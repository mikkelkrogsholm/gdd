"""Concrete examples for rule.sum and scenario.add; not a universal proof."""

import unittest

from integer_add import add


class IntegerAdditionTests(unittest.TestCase):
    def test_examples(self):
        for left, right, expected in [(-7, 7, 0), (2, 3, 5), (0, 0, 0),
                                      (-9, -4, -13), (10**100, 1, 10**100 + 1)]:
            with self.subTest(left=left, right=right):
                self.assertEqual(add(left, right), expected)


if __name__ == "__main__":
    unittest.main()
