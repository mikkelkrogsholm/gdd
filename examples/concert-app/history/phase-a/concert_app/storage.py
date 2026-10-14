"""File SQLite persistence; domain policy belongs to Reservations."""

from contextlib import contextmanager
from pathlib import Path
import sqlite3
from uuid import uuid4


PersistenceError = sqlite3.Error


class PolicyMigrationError(ValueError):
    """Existing active reservations do not satisfy the proposed policy."""


SEED_CONCERTS = (
    ("midnight", "Midnight Sessions", "2026-11-14", "Harbor Hall", 8),
    ("northern", "Northern Lights", "2026-11-21", "North Stage", 3),
    ("matinee", "Sunday Matinee", "2026-11-29", "Studio 03", 2),
)


@contextmanager
def transaction(database, write=False):
    """Use a fresh connection and rollback every failure before commit.

    BEGIN IMMEDIATE reserves the writer before domain reads, so concurrent
    capacity checks, key binding and insertion form a serializable operation.
    """
    connection = None
    try:
        connection = sqlite3.connect(database, timeout=5.0, isolation_level=None)
        connection.row_factory = sqlite3.Row
        connection.execute("PRAGMA foreign_keys = ON")
        connection.execute("BEGIN IMMEDIATE" if write else "BEGIN")
        yield connection
        connection.commit()
    except BaseException:
        if connection is not None:
            try:
                connection.rollback()
            except sqlite3.Error:
                pass
        raise
    finally:
        if connection is not None:
            connection.close()


def initialize(database):
    """Create the local schema and insert missing fictional events only."""
    with transaction(database, write=True) as connection:
        connection.execute(
            """CREATE TABLE IF NOT EXISTS concerts (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                date TEXT NOT NULL,
                venue TEXT NOT NULL,
                capacity INTEGER NOT NULL CHECK (capacity >= 0)
            )"""
        )
        connection.execute(
            """CREATE TABLE IF NOT EXISTS reservations (
                id TEXT PRIMARY KEY,
                concert_id TEXT NOT NULL REFERENCES concerts(id),
                principal_id TEXT NOT NULL,
                request_key TEXT NOT NULL,
                state TEXT NOT NULL CHECK (state IN ('reserved', 'cancelled')),
                UNIQUE (principal_id, request_key)
            )"""
        )
        connection.executemany(
            """INSERT OR IGNORE INTO concerts (id, title, date, venue, capacity)
               VALUES (?, ?, ?, ?, ?)""",
            SEED_CONCERTS,
        )


def get_concert(connection, concert_id):
    row = connection.execute(
        "SELECT id, title, date, venue, capacity FROM concerts WHERE id = ?",
        (concert_id,),
    ).fetchone()
    return dict(row) if row is not None else None


def list_concerts(connection):
    return [
        dict(row)
        for row in connection.execute(
            "SELECT id, title, date, venue, capacity FROM concerts ORDER BY date, id"
        )
    ]


def find_by_key(connection, principal_id, request_key):
    row = connection.execute(
        "SELECT * FROM reservations WHERE principal_id = ? AND request_key = ?",
        (principal_id, request_key),
    ).fetchone()
    return dict(row) if row is not None else None


def get_reservation(connection, reservation_id):
    row = connection.execute(
        "SELECT * FROM reservations WHERE id = ?", (reservation_id,)
    ).fetchone()
    return dict(row) if row is not None else None


def list_for_principal(connection, principal_id):
    return [
        dict(row)
        for row in connection.execute(
            "SELECT * FROM reservations WHERE principal_id = ? ORDER BY rowid DESC",
            (principal_id,),
        )
    ]


def count_active(connection, concert_id=None, principal_id=None):
    conditions, parameters = ["state = 'reserved'"], []
    if concert_id is not None:
        conditions.append("concert_id = ?")
        parameters.append(concert_id)
    if principal_id is not None:
        conditions.append("principal_id = ?")
        parameters.append(principal_id)
    return connection.execute(
        "SELECT COUNT(*) FROM reservations WHERE " + " AND ".join(conditions),
        parameters,
    ).fetchone()[0]


def insert_reservation(connection, principal_id, concert_id, request_key):
    reservation_id = uuid4().hex
    connection.execute(
        """INSERT INTO reservations
           (id, concert_id, principal_id, request_key, state)
           VALUES (?, ?, ?, ?, 'reserved')""",
        (reservation_id, concert_id, principal_id, request_key),
    )
    return get_reservation(connection, reservation_id)


def cancel_reservation(connection, reservation_id):
    connection.execute(
        "UPDATE reservations SET state = 'cancelled' WHERE id = ? AND state = 'reserved'",
        (reservation_id,),
    )
    return get_reservation(connection, reservation_id)


def validate_active_limit(database, limit):
    """Inspect compatibility without creating or changing persisted data."""
    if limit is None:
        return
    if isinstance(limit, bool) or not isinstance(limit, int) or limit < 0:
        raise ValueError("The active reservation limit must be a nonnegative integer or None.")
    if not Path(database).exists():
        return
    with transaction(database) as connection:
        if connection.execute(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'reservations'"
        ).fetchone() is None:
            return
        incompatible = connection.execute(
            """SELECT 1 FROM reservations WHERE state = 'reserved'
               GROUP BY principal_id HAVING COUNT(*) > ? LIMIT 1""",
            (limit,),
        ).fetchone()
        if incompatible:
            raise PolicyMigrationError(
                "Existing active reservations exceed the proposed limit; explicit reconciliation is required."
            )
