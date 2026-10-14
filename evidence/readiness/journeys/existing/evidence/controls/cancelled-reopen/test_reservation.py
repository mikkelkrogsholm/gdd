import inspect
import unittest

import reservation


class ReservationLifecycleTests(unittest.TestCase):
    def test_default_is_draft(self):
        self.assertEqual(reservation.Reservation().state, 'draft')

    def test_public_reserve_signature(self):
        signature = inspect.signature(reservation.reserve)
        self.assertEqual(list(signature.parameters), ['value'])
        self.assertIs(signature.parameters['value'].annotation, reservation.Reservation)
        self.assertEqual(signature.parameters['value'].kind, inspect.Parameter.POSITIONAL_OR_KEYWORD)
        self.assertIs(signature.return_annotation, reservation.Reservation)

    def test_reserve_returns_input_and_changes_draft(self):
        value = reservation.Reservation()
        self.assertIs(reservation.reserve(value), value)
        self.assertEqual(value.state, 'reserved')

    def test_repeated_reserve_is_harmless(self):
        value = reservation.Reservation()
        reservation.reserve(value)
        self.assertIs(reservation.reserve(value), value)
        self.assertIs(reservation.reserve(value), value)
        self.assertEqual(value.state, 'reserved')

    def test_cancel_changes_reserved_and_returns_input(self):
        value = reservation.Reservation()
        reservation.reserve(value)
        self.assertIs(reservation.cancel(value), value)
        self.assertEqual(value.state, 'cancelled')

    def test_repeated_cancel_is_harmless(self):
        value = reservation.Reservation()
        reservation.reserve(value)
        reservation.cancel(value)
        self.assertIs(reservation.cancel(value), value)
        self.assertIs(reservation.cancel(value), value)
        self.assertEqual(value.state, 'cancelled')

    def test_cancelled_reserve_rejects_without_mutation(self):
        value = reservation.Reservation()
        reservation.reserve(value)
        reservation.cancel(value)
        with self.assertRaises(reservation.InvalidTransition):
            reservation.reserve(value)
        self.assertEqual(value.state, 'cancelled')

    def test_draft_cancel_rejects_without_mutation(self):
        value = reservation.Reservation()
        with self.assertRaises(reservation.InvalidTransition):
            reservation.cancel(value)
        self.assertEqual(value.state, 'draft')

    def test_unsupported_reserve_state_rejects_without_mutation(self):
        value = reservation.Reservation()
        value.state = 'unsupported'
        with self.assertRaises(reservation.InvalidTransition):
            reservation.reserve(value)
        self.assertEqual(value.state, 'unsupported')

    def test_unsupported_cancel_state_rejects_without_mutation(self):
        value = reservation.Reservation()
        value.state = 'unsupported'
        with self.assertRaises(reservation.InvalidTransition):
            reservation.cancel(value)
        self.assertEqual(value.state, 'unsupported')


if __name__ == '__main__':
    unittest.main()
