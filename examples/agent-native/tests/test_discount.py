import unittest

from quote_demo.discount import discount


class LeafTests(unittest.TestCase):
    def test_normal(self):
        """scenario.discount.normal: exactly divisible positive control."""
        self.assertEqual(discount(10000, 1000), 1000)

    def test_floor_half(self):
        """scenario.discount.floor: accepted floor value, old half-up must fail."""
        self.assertEqual(discount(101, 5000), 50)

    def test_floor_fraction(self):
        self.assertEqual(discount(103, 2500), 25)

    def test_boundaries(self):
        """scenario.discount.floor: endpoints and exact large-integer control."""
        self.assertEqual(discount(0, 10000), 0)
        self.assertEqual(discount(101, 0), 0)
        self.assertEqual(discount(101, 10000), 101)
        self.assertEqual(discount(10**30, 5000), 5 * 10**29)

    def test_invalid(self):
        """scenario.discount.invalid: integers and inclusive rates are mandatory."""
        for args in [(-1, 0), (1, -1), (1, 10001), (True, 0), (1, False),
                     (1.0, 0), (1, 0.0), ("1", 0), (1, "0"), (None, 0)]:
            with self.subTest(args=args), self.assertRaisesRegex(ValueError, "^InvalidInput$"):
                discount(*args)
