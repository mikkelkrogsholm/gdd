"""Fixed acceptance assertions for the requested Reservation lifecycle."""
import inspect
import unittest
import reservation


class ReservationAcceptance(unittest.TestCase):
    def test_default_is_draft(self):
        self.assertEqual(reservation.Reservation().state, "draft")

    def test_reserve_signature_is_preserved(self):
        signature = inspect.signature(reservation.reserve)
        self.assertEqual(list(signature.parameters), ["value"])
        self.assertIs(signature.parameters["value"].annotation, reservation.Reservation)
        self.assertIs(signature.return_annotation, reservation.Reservation)

    def test_each_reservation_keeps_its_own_state(self):
        first = reservation.Reservation()
        second = reservation.Reservation()
        reservation.reserve(first)
        reservation.cancel(first)
        self.assertEqual(first.state, "cancelled")
        self.assertEqual(second.state, "draft")

    def test_reserve_preserves_identity_and_sets_reserved(self):
        value = reservation.Reservation()
        self.assertIs(reservation.reserve(value), value)
        self.assertEqual(value.state, "reserved")

    def test_repeated_reserve_is_harmless(self):
        value = reservation.Reservation()
        first = reservation.reserve(value)
        value.marker = object()
        marker = value.marker
        self.assertIs(reservation.reserve(value), first)
        self.assertIs(first, value)
        self.assertEqual(value.state, "reserved")
        self.assertIs(value.marker, marker)

    def test_cancel_sets_cancelled_and_preserves_identity(self):
        value = reservation.Reservation()
        reservation.reserve(value)
        self.assertIs(reservation.cancel(value), value)
        self.assertEqual(value.state, "cancelled")

    def test_repeated_cancel_is_harmless(self):
        value = reservation.Reservation()
        reservation.reserve(value)
        reservation.cancel(value)
        value.marker = object()
        marker = value.marker
        self.assertIs(reservation.cancel(value), value)
        self.assertIs(reservation.cancel(value), value)
        self.assertEqual(value.state, "cancelled")
        self.assertIs(value.marker, marker)

    def test_cancelled_cannot_be_reserved(self):
        value = reservation.Reservation()
        reservation.reserve(value)
        reservation.cancel(value)
        before = vars(value).copy()
        with self.assertRaises(reservation.InvalidTransition):
            reservation.reserve(value)
        self.assertEqual(vars(value), before)
        self.assertEqual(value.state, "cancelled")

    def test_draft_cancel_is_rejected_without_mutation(self):
        value = reservation.Reservation()
        before = vars(value).copy()
        with self.assertRaises(reservation.InvalidTransition):
            reservation.cancel(value)
        self.assertEqual(vars(value), before)


if __name__ == "__main__":
    unittest.main()
