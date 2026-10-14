import unittest

from quote_demo.tax import tax


class LeafTests(unittest.TestCase):
    def test_normal(self):
        """scenario.tax.normal: exactly divisible positive control."""
        self.assertEqual(tax(10000, 1000), 1000)

    def test_floor_half(self):
        """scenario.tax.floor: accepted floor value, old half-up must fail."""
        self.assertEqual(tax(101, 5000), 50)

    def test_floor_fraction(self):
        self.assertEqual(tax(103, 2500), 25)

    def test_boundaries(self):
        """scenario.tax.floor: endpoints and exact large-integer control."""
        self.assertEqual(tax(0, 10000), 0)
        self.assertEqual(tax(101, 0), 0)
        self.assertEqual(tax(101, 10000), 101)
        self.assertEqual(tax(10**30, 5000), 5 * 10**29)

    def test_invalid(self):
        """scenario.tax.invalid: integers and inclusive rates are mandatory."""
        for args in [(-1, 0), (1, -1), (1, 10001), (True, 0), (1, False),
                     (1.0, 0), (1, 0.0), ("1", 0), (1, "0"), (None, 0)]:
            with self.subTest(args=args), self.assertRaisesRegex(ValueError, "^InvalidInput$"):
                tax(*args)
