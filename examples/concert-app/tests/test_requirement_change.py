"""Fixed phase-B assertions: run unchanged against A first, then B."""

from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import sqlite3
from threading import Barrier

import pytest

from concert_app import reservations, storage


@pytest.fixture
def database(tmp_path):
    path = tmp_path / "requirements.sqlite"
    reservations.initialize(path)
    return path


def error(code, operation, *args):
    with pytest.raises(reservations.DomainError) as raised:
        operation(*args)
    assert raised.value.code == code


def reserve(database, key, concert="midnight", principal="alice"):
    return reservations.reserve(database, principal, concert, key)


def test_global_limit_rejects_third_across_concerts_and_leaves_key_unbound(database):
    reserve(database, "one")
    reserve(database, "two", "northern")
    error("ActiveReservationLimitReached", reserve, database, "three", "matinee")
    assert reservations.list_own(database, "alice")["active_count"] == 2
    with storage.transaction(database) as connection:
        assert storage.find_by_key(connection, "alice", "three") is None
        assert storage.count_active(connection, concert_id="matinee") == 0


def test_principal_allowances_are_independent(database):
    reserve(database, "a1")
    reserve(database, "a2", "northern")
    reserve(database, "b1", principal="bob")
    reserve(database, "b2", "northern", principal="bob")
    assert reservations.list_own(database, "alice")["active_count"] == 2
    assert reservations.list_own(database, "bob")["active_count"] == 2
    error("ActiveReservationLimitReached", reserve, database, "b3", "midnight", "bob")


def test_cancellation_frees_allowance_and_failed_key_can_then_succeed(database):
    first = reserve(database, "one")
    reserve(database, "two", "northern")
    error("ActiveReservationLimitReached", reserve, database, "failed", "matinee")
    reservations.cancel(database, "alice", first["id"])
    created = reserve(database, "failed", "matinee")
    history = reservations.list_own(database, "alice")
    assert history["active_count"] == 2 and len(history["reservations"]) == 3
    assert history["active_limit"] == 2 and created["state"] == "reserved"


def test_active_and_cancelled_successful_replay_precede_limit_and_capacity(database):
    first = reserve(database, "one")
    reserve(database, "two", "northern")
    with storage.transaction(database, write=True) as connection:
        connection.execute("UPDATE concerts SET capacity=1 WHERE id='midnight'")
    assert reserve(database, "one") == first
    cancelled = reservations.cancel(database, "alice", first["id"])
    reserve(database, "three", "matinee")
    assert reserve(database, "one") == cancelled
    assert reservations.list_own(database, "alice")["active_count"] == 2


def test_precedence_is_input_then_replay_then_existence_then_limit_then_capacity(database):
    reserve(database, "one")
    reserve(database, "two", "northern")
    error("InvalidInput", reserve, database, "", "midnight")
    error("IdempotencyConflict", reserve, database, "one", "missing")
    error("ConcertNotFound", reserve, database, "new", "missing")
    with storage.transaction(database, write=True) as connection:
        connection.execute("UPDATE concerts SET capacity=0 WHERE id='matinee'")
    error("ActiveReservationLimitReached", reserve, database, "new", "matinee")
    error("SoldOut", reserve, database, "new", "matinee", "bob")


def test_concurrent_new_keys_across_concerts_share_one_remaining_allowance(database):
    reserve(database, "existing")
    barrier = Barrier(2)
    def attempt(pair):
        key, concert = pair
        barrier.wait(timeout=5)
        try:
            return reserve(database, key, concert)
        except reservations.DomainError as failure:
            return {"error": failure.code}
    with ThreadPoolExecutor(max_workers=2) as pool:
        outcomes = list(pool.map(attempt, [("competing-one", "northern"), ("competing-two", "matinee")]))
    assert sum(outcome.get("state") == "reserved" for outcome in outcomes) == 1
    assert sum(outcome.get("error") == "ActiveReservationLimitReached" for outcome in outcomes) == 1
    assert reservations.list_own(database, "alice")["active_count"] == 2
    with storage.transaction(database) as connection:
        assert sum(storage.find_by_key(connection, "alice", key) is not None for key in ("competing-one", "competing-two")) == 1


def test_concurrent_replay_remains_one_reservation_at_limit(database):
    first = reserve(database, "existing")
    reserve(database, "second", "northern")
    with ThreadPoolExecutor(max_workers=8) as pool:
        outputs = list(pool.map(lambda _: reserve(database, "existing"), range(8)))
    assert outputs == [first] * 8
    assert reservations.list_own(database, "alice")["active_count"] == 2


def test_server_exposes_single_current_policy_value(database):
    assert reservations.ACTIVE_LIMIT == 2
    assert reservations.list_own(database, "alice")["active_limit"] == 2


def test_incompatible_existing_a_data_is_rejected_without_history_mutation(tmp_path):
    path = tmp_path / "incompatible.sqlite"
    storage.initialize(path)
    with storage.transaction(path, write=True) as connection:
        for key, concert in [("one", "midnight"), ("two", "northern"), ("three", "matinee")]:
            storage.insert_reservation(connection, "alice", concert, key)
    before = path.read_bytes()
    error("PolicyMigrationRequired", reservations.initialize, path)
    assert path.read_bytes() == before
    with storage.transaction(path) as connection:
        assert storage.count_active(connection, principal_id="alice") == 3


def test_compatible_a_history_and_cancelled_keys_survive_activation(tmp_path):
    path = tmp_path / "compatible.sqlite"
    storage.initialize(path)
    with storage.transaction(path, write=True) as connection:
        cancelled = storage.insert_reservation(connection, "alice", "midnight", "old-cancelled")
        storage.cancel_reservation(connection, cancelled["id"])
        storage.insert_reservation(connection, "alice", "northern", "old-one")
        storage.insert_reservation(connection, "alice", "matinee", "old-two")
    reservations.initialize(path)
    assert reserve(path, "old-cancelled") == {"id": cancelled["id"], "state": "cancelled"}
    history = reservations.list_own(path, "alice")
    assert history["active_count"] == 2 and history["active_limit"] == 2 and len(history["reservations"]) == 3
