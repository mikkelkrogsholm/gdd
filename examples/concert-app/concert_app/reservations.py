"""Reservation decisions against explicit Storage transaction contracts."""

from . import storage


ACTIVE_LIMIT = 2


class DomainError(Exception):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code
        self.message = message


def _principal(principal_id):
    if not isinstance(principal_id, str) or not principal_id.strip():
        raise DomainError("Unauthenticated", "Sign in to continue.")


def _input(*values):
    if any(not isinstance(value, str) or not value.strip() for value in values):
        raise DomainError("InvalidInput", "Provide a valid identifier and request key.")


def _public(row):
    return {"id": row["id"], "state": row["state"]}


def _unavailable():
    return DomainError("PersistenceUnavailable", "Reservations are temporarily unavailable. Please retry.")


def initialize(database):
    """Refuse incompatible existing data before initializing or seeding."""
    try:
        storage.validate_active_limit(database, ACTIVE_LIMIT)
        storage.initialize(database)
        storage.validate_active_limit(database, ACTIVE_LIMIT)
    except storage.PolicyMigrationError as error:
        raise DomainError(
            "PolicyMigrationRequired",
            "Existing reservations require explicit reconciliation before this policy can be activated.",
        ) from error
    except storage.PersistenceError as error:
        raise _unavailable() from error


def catalog(database):
    try:
        with storage.transaction(database) as connection:
            return {
                "concerts": [
                    {**concert, "available": concert["capacity"] - storage.count_active(connection, concert_id=concert["id"])}
                    for concert in storage.list_concerts(connection)
                ]
            }
    except storage.PersistenceError as error:
        raise _unavailable() from error


def list_own(database, principal_id):
    _principal(principal_id)
    try:
        with storage.transaction(database) as connection:
            history = storage.list_for_principal(connection, principal_id)
            return {
                "reservations": [
                    {"id": row["id"], "concert_id": row["concert_id"], "state": row["state"]}
                    for row in history
                ],
                "active_count": sum(row["state"] == "reserved" for row in history),
                "active_limit": ACTIVE_LIMIT,
            }
    except storage.PersistenceError as error:
        raise _unavailable() from error


def reserve(database, principal_id, concert_id, request_key):
    _principal(principal_id)
    _input(concert_id, request_key)
    try:
        with storage.transaction(database, write=True) as connection:
            previous = storage.find_by_key(connection, principal_id, request_key)
            if previous is not None:
                if previous["concert_id"] != concert_id:
                    raise DomainError("IdempotencyConflict", "This request key belongs to another concert.")
                return _public(previous)
            concert = storage.get_concert(connection, concert_id)
            if concert is None:
                raise DomainError("ConcertNotFound", "Concert not found.")
            if ACTIVE_LIMIT is not None and storage.count_active(connection, principal_id=principal_id) >= ACTIVE_LIMIT:
                raise DomainError("ActiveReservationLimitReached", "Your active reservation limit has been reached.")
            if storage.count_active(connection, concert_id=concert_id) >= concert["capacity"]:
                raise DomainError("SoldOut", "No seats are available.")
            return _public(storage.insert_reservation(connection, principal_id, concert_id, request_key))
    except storage.PersistenceError as error:
        raise _unavailable() from error


def cancel(database, principal_id, reservation_id):
    _principal(principal_id)
    _input(reservation_id)
    try:
        with storage.transaction(database, write=True) as connection:
            reservation = storage.get_reservation(connection, reservation_id)
            if reservation is None or reservation["principal_id"] != principal_id:
                raise DomainError("ReservationNotFound", "Reservation not found.")
            if reservation["state"] == "cancelled":
                return _public(reservation)
            return _public(storage.cancel_reservation(connection, reservation_id))
    except storage.PersistenceError as error:
        raise _unavailable() from error
