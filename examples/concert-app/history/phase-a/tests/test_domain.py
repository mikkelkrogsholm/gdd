"""Actual file SQLite tests for the Phase A reservation contracts."""

from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import sqlite3
import tempfile
from threading import Barrier, Event
import unittest
from unittest.mock import patch

from concert_app import reservations, storage


class ReservationTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.database = Path(self.directory.name) / "concert.sqlite"
        reservations.initialize(self.database)

    def assert_error(self, code, operation, *arguments):
        with self.assertRaises(reservations.DomainError) as raised:
            operation(*arguments)
        self.assertEqual(raised.exception.code, code)
        return raised.exception

    def capacity(self, concert_id, capacity):
        with storage.transaction(self.database, write=True) as connection:
            connection.execute("UPDATE concerts SET capacity = ? WHERE id = ?", (capacity, concert_id))

    def active(self, concert_id=None, principal_id=None):
        with storage.transaction(self.database) as connection:
            return storage.count_active(connection, concert_id, principal_id)

    def persisted(self, reservation_id):
        with storage.transaction(self.database) as connection:
            return storage.get_reservation(connection, reservation_id)

    def key_row(self, principal_id, request_key):
        with storage.transaction(self.database) as connection:
            return storage.find_by_key(connection, principal_id, request_key)

    def reserve(self, key="first", principal="alice", concert="midnight"):
        return reservations.reserve(self.database, principal, concert, key)

    def test_seed_metadata_and_availability_are_public(self):
        concerts = reservations.catalog(self.database)["concerts"]
        self.assertEqual(concerts, [
            {"id": "midnight", "title": "Midnight Sessions", "date": "2026-11-14", "venue": "Harbor Hall", "capacity": 8, "available": 8},
            {"id": "northern", "title": "Northern Lights", "date": "2026-11-21", "venue": "North Stage", "capacity": 3, "available": 3},
            {"id": "matinee", "title": "Sunday Matinee", "date": "2026-11-29", "venue": "Studio 03", "capacity": 2, "available": 2},
        ])

    def test_initialize_preserves_existing_capacity_metadata_and_history(self):
        reservation = self.reserve()
        with storage.transaction(self.database, write=True) as connection:
            connection.execute("UPDATE concerts SET capacity = 17, title = 'Retained title' WHERE id = 'midnight'")
        reservations.initialize(self.database)
        with storage.transaction(self.database) as connection:
            concert = storage.get_concert(connection, "midnight")
        self.assertEqual((concert["capacity"], concert["title"]), (17, "Retained title"))
        self.assertEqual(self.persisted(reservation["id"])["request_key"], "first")

    def test_reserve_returns_public_fields_and_occupies_one_seat(self):
        result = self.reserve()
        self.assertEqual(set(result), {"id", "state"})
        self.assertEqual(result["state"], "reserved")
        self.assertEqual(self.active("midnight"), 1)
        self.assertEqual(reservations.catalog(self.database)["concerts"][0]["available"], 7)

    def test_phase_a_has_no_per_person_limit(self):
        self.assertIsNone(reservations.ACTIVE_LIMIT)
        for key in ("one", "two", "three"):
            self.reserve(key)
        response = reservations.list_own(self.database, "alice")
        self.assertEqual(response["active_count"], 3)
        self.assertIsNone(response["active_limit"])

    def test_identity_is_validated_before_lookup_or_mutation(self):
        with patch.object(storage, "transaction", side_effect=AssertionError("Persistence was accessed")):
            for principal in (None, "", " ", 4, False):
                with self.subTest(principal=principal):
                    self.assert_error("Unauthenticated", reservations.reserve, self.database, principal, None, None)
                    self.assert_error("Unauthenticated", reservations.cancel, self.database, principal, None)
                    self.assert_error("Unauthenticated", reservations.list_own, self.database, principal)

    def test_malformed_input_is_rejected_before_lookup(self):
        with patch.object(storage, "transaction", side_effect=AssertionError("Persistence was accessed")):
            for invalid in (None, "", " ", 7, {}, True):
                with self.subTest(value=invalid):
                    self.assert_error("InvalidInput", reservations.reserve, self.database, "alice", invalid, "key")
                    self.assert_error("InvalidInput", reservations.reserve, self.database, "alice", "midnight", invalid)
                    self.assert_error("InvalidInput", reservations.cancel, self.database, "alice", invalid)

    def test_missing_concert_binds_no_key_and_can_be_retried(self):
        self.assert_error("ConcertNotFound", reservations.reserve, self.database, "alice", "missing", "retry")
        self.assertIsNone(self.key_row("alice", "retry"))
        self.reserve("retry")
        self.assertEqual(self.active(principal_id="alice"), 1)

    def test_sold_out_binds_no_key_and_can_succeed_after_cancellation(self):
        self.capacity("midnight", 1)
        original = self.reserve("occupant")
        self.assert_error("SoldOut", reservations.reserve, self.database, "bob", "midnight", "retry")
        self.assertIsNone(self.key_row("bob", "retry"))
        reservations.cancel(self.database, "alice", original["id"])
        result = self.reserve("retry", principal="bob")
        self.assertEqual(result["state"], "reserved")
        self.assertEqual(self.active("midnight"), 1)

    def test_zero_capacity_rejects_new_reservation(self):
        self.capacity("midnight", 0)
        self.assert_error("SoldOut", reservations.reserve, self.database, "alice", "midnight", "empty")
        self.assertEqual(self.active("midnight"), 0)
        self.assertIsNone(self.key_row("alice", "empty"))

    def test_replay_precedes_capacity_and_returns_same_id(self):
        self.capacity("midnight", 1)
        original = self.reserve("same")
        with patch.object(storage, "get_concert", side_effect=AssertionError("Replay reached capacity lookup")):
            self.assertEqual(self.reserve("same"), original)
        self.assertEqual(self.active("midnight"), 1)

    def test_key_conflict_precedes_concert_lookup(self):
        self.reserve("same")
        with patch.object(storage, "get_concert", side_effect=AssertionError("Conflict reached concert lookup")):
            self.assert_error("IdempotencyConflict", reservations.reserve, self.database, "alice", "missing", "same")
        self.assertEqual(self.active(), 1)

    def test_request_keys_are_scoped_to_principal(self):
        alice = self.reserve("same", principal="alice")
        bob = self.reserve("same", principal="bob")
        self.assertNotEqual(alice["id"], bob["id"])
        self.assertEqual(self.active(), 2)

    def test_cancel_replay_keeps_history_key_and_available_capacity(self):
        result = self.reserve("keep")
        cancelled = reservations.cancel(self.database, "alice", result["id"])
        self.assertEqual(cancelled, {"id": result["id"], "state": "cancelled"})
        self.assertEqual(reservations.cancel(self.database, "alice", result["id"]), cancelled)
        self.assertEqual(self.reserve("keep"), cancelled)
        self.assertEqual(self.active(), 0)
        self.assertEqual(self.key_row("alice", "keep")["state"], "cancelled")
        self.assertEqual(reservations.catalog(self.database)["concerts"][0]["available"], 8)

    def test_foreign_and_missing_cancellation_have_identical_errors(self):
        result = self.reserve()
        missing = self.assert_error("ReservationNotFound", reservations.cancel, self.database, "bob", "missing")
        foreign = self.assert_error("ReservationNotFound", reservations.cancel, self.database, "bob", result["id"])
        self.assertEqual(foreign.message, missing.message)
        self.assertEqual(self.persisted(result["id"])["state"], "reserved")
        reservations.cancel(self.database, "alice", result["id"])
        foreign_cancelled = self.assert_error("ReservationNotFound", reservations.cancel, self.database, "bob", result["id"])
        self.assertEqual(foreign_cancelled.message, missing.message)

    def test_history_is_principal_private_and_omits_private_fields(self):
        active = self.reserve("active")
        old = self.reserve("old")
        reservations.cancel(self.database, "alice", old["id"])
        bob = self.reserve("bob", principal="bob")
        alice_rows = reservations.list_own(self.database, "alice")
        self.assertEqual({row["id"] for row in alice_rows["reservations"]}, {active["id"], old["id"]})
        self.assertTrue(all(set(row) == {"id", "concert_id", "state"} for row in alice_rows["reservations"]))
        self.assertEqual(alice_rows["active_count"], 1)
        self.assertEqual(reservations.list_own(self.database, "bob")["reservations"][0]["id"], bob["id"])

    def test_concurrent_last_seat_uses_multiple_connections_without_overbooking(self):
        self.capacity("midnight", 1)
        barrier = Barrier(8)
        def attempt(index):
            barrier.wait(timeout=5)
            try:
                return self.reserve(f"request-{index}", principal=f"person-{index}")
            except reservations.DomainError as error:
                return error.code
        with ThreadPoolExecutor(max_workers=8) as executor:
            results = list(executor.map(attempt, range(8)))
        self.assertEqual(sum(isinstance(result, dict) for result in results), 1)
        self.assertEqual(results.count("SoldOut"), 7)
        self.assertEqual(self.active("midnight"), 1)

    def test_concurrent_same_key_returns_one_reservation(self):
        barrier = Barrier(8)
        def attempt(_):
            barrier.wait(timeout=5)
            return self.reserve("shared")
        with ThreadPoolExecutor(max_workers=8) as executor:
            results = list(executor.map(attempt, range(8)))
        self.assertEqual(len({result["id"] for result in results}), 1)
        self.assertEqual(self.active(), 1)

    def test_concurrent_same_key_different_concerts_binds_only_one(self):
        barrier = Barrier(2)
        def attempt(concert):
            barrier.wait(timeout=5)
            try:
                return self.reserve("shared", concert=concert)
            except reservations.DomainError as error:
                return error.code
        with ThreadPoolExecutor(max_workers=2) as executor:
            results = list(executor.map(attempt, ["midnight", "northern"]))
        self.assertEqual(sum(isinstance(result, dict) for result in results), 1)
        self.assertEqual(results.count("IdempotencyConflict"), 1)
        self.assertEqual(self.active(), 1)

    def test_concurrent_cancellation_releases_exactly_one_seat(self):
        result = self.reserve()
        barrier = Barrier(6)
        def attempt(_):
            barrier.wait(timeout=5)
            return reservations.cancel(self.database, "alice", result["id"])
        with ThreadPoolExecutor(max_workers=6) as executor:
            results = list(executor.map(attempt, range(6)))
        self.assertTrue(all(row == {"id": result["id"], "state": "cancelled"} for row in results))
        self.assertEqual(self.active(), 0)
        self.assertEqual(reservations.catalog(self.database)["concerts"][0]["available"], 8)

    def test_held_writer_is_released_before_waiting_reservation_proceeds(self):
        attempted = Event()
        original = storage.transaction
        def watched_transaction(database, write=False):
            if write:
                attempted.set()
            return original(database, write)
        with ThreadPoolExecutor(max_workers=1) as executor:
            with original(self.database, write=True):
                with patch.object(storage, "transaction", side_effect=watched_transaction):
                    future = executor.submit(self.reserve, "waiting")
                    self.assertTrue(attempted.wait(timeout=2))
                    self.assertFalse(future.done())
            result = future.result(timeout=5)
        self.assertEqual(result["state"], "reserved")
        self.assertEqual(self.active(), 1)

    def test_fault_after_real_insert_rolls_back_and_does_not_bind_key(self):
        original = storage.insert_reservation
        def failing_insert(*arguments):
            original(*arguments)
            raise sqlite3.OperationalError("Injected failure after real insert")
        with patch.object(storage, "insert_reservation", side_effect=failing_insert):
            error = self.assert_error("PersistenceUnavailable", reservations.reserve, self.database, "alice", "midnight", "fault")
        self.assertNotIn("Injected", error.message)
        self.assertEqual(self.active(), 0)
        self.assertIsNone(self.key_row("alice", "fault"))
        self.reserve("fault")
        self.assertEqual(self.active(), 1)

    def test_fault_after_real_cancel_rolls_back_state_change(self):
        result = self.reserve("keep")
        original = storage.cancel_reservation
        def failing_cancel(*arguments):
            original(*arguments)
            raise sqlite3.OperationalError("Injected failure after real cancellation")
        with patch.object(storage, "cancel_reservation", side_effect=failing_cancel):
            self.assert_error("PersistenceUnavailable", reservations.cancel, self.database, "alice", result["id"])
        self.assertEqual(self.persisted(result["id"])["state"], "reserved")
        self.assertEqual(self.active(), 1)
        self.assertEqual(self.reserve("keep"), result)
        reservations.cancel(self.database, "alice", result["id"])
        self.assertEqual(self.active(), 0)

    def test_read_and_initialization_database_failures_are_translated(self):
        with patch.object(storage, "transaction", side_effect=sqlite3.OperationalError("Private SQL detail")):
            for operation, arguments in (
                (reservations.catalog, (self.database,)),
                (reservations.list_own, (self.database, "alice")),
                (reservations.initialize, (self.database,)),
            ):
                error = self.assert_error("PersistenceUnavailable", operation, *arguments)
                self.assertNotIn("SQL", error.message)

    def test_transaction_rolls_back_non_sql_failure_and_closes_connection(self):
        connection = None
        with self.assertRaisesRegex(RuntimeError, "Injected"):
            with storage.transaction(self.database, write=True) as connection:
                storage.insert_reservation(connection, "alice", "midnight", "rollback")
                raise RuntimeError("Injected failure before commit")
        self.assertEqual(self.active(), 0)
        with self.assertRaises(sqlite3.ProgrammingError):
            connection.execute("SELECT 1")

    def test_commit_failure_rolls_back_creation_and_key_binding(self):
        original_connect = sqlite3.connect
        class FailedCommitConnection(sqlite3.Connection):
            def commit(self):
                raise sqlite3.OperationalError("Injected failure before actual commit")
        def connect(*arguments, **keywords):
            return original_connect(*arguments, factory=FailedCommitConnection, **keywords)
        with patch.object(sqlite3, "connect", side_effect=connect):
            self.assert_error("PersistenceUnavailable", reservations.reserve, self.database, "alice", "midnight", "commit-fault")
        self.assertEqual(self.active(), 0)
        self.assertIsNone(self.key_row("alice", "commit-fault"))
        self.reserve("commit-fault")
        self.assertEqual(self.active(), 1)

    def test_count_filters_intersect_and_cancelled_rows_are_excluded(self):
        alice_midnight = self.reserve("midnight")
        self.reserve("north", concert="northern")
        self.reserve("bob", principal="bob")
        self.assertEqual(self.active(), 3)
        self.assertEqual(self.active("midnight"), 2)
        self.assertEqual(self.active(principal_id="alice"), 2)
        self.assertEqual(self.active("midnight", "alice"), 1)
        reservations.cancel(self.database, "alice", alice_midnight["id"])
        self.assertEqual(self.active("midnight", "alice"), 0)

    def test_database_uniqueness_retains_cancelled_keys(self):
        result = self.reserve("bound")
        reservations.cancel(self.database, "alice", result["id"])
        with self.assertRaises(storage.PersistenceError):
            with storage.transaction(self.database, write=True) as connection:
                storage.insert_reservation(connection, "alice", "northern", "bound")
        self.assertEqual(self.key_row("alice", "bound")["id"], result["id"])
        self.assertEqual(self.active(), 0)

    def test_future_limit_compatibility_inspection_is_read_only(self):
        for key in ("one", "two", "three"):
            self.reserve(key)
        before = self.database.read_bytes()
        with self.assertRaisesRegex(storage.PolicyMigrationError, "explicit reconciliation"):
            storage.validate_active_limit(self.database, 2)
        self.assertEqual(self.database.read_bytes(), before)
        self.assertEqual(self.active(principal_id="alice"), 3)
        storage.validate_active_limit(self.database, None)

    def test_future_policy_initialize_rejects_incompatible_data_before_mutation(self):
        for key in ("one", "two", "three"):
            self.reserve(key)
        with storage.transaction(self.database, write=True) as connection:
            connection.execute("DELETE FROM concerts WHERE id = 'matinee'")
        before = self.database.read_bytes()
        with patch.object(reservations, "ACTIVE_LIMIT", 2):
            self.assert_error("PolicyMigrationRequired", reservations.initialize, self.database)
        self.assertEqual(self.database.read_bytes(), before)
        with storage.transaction(self.database) as connection:
            self.assertIsNone(storage.get_concert(connection, "matinee"))
        self.assertEqual(self.active(principal_id="alice"), 3)

    def test_compatible_validation_preserves_history_and_missing_path(self):
        result = self.reserve()
        reservations.cancel(self.database, "alice", result["id"])
        before = self.database.read_bytes()
        storage.validate_active_limit(self.database, 0)
        self.assertEqual(self.database.read_bytes(), before)
        missing = Path(self.directory.name) / "missing.sqlite"
        storage.validate_active_limit(missing, 2)
        self.assertFalse(missing.exists())


if __name__ == "__main__":
    unittest.main()
