import json
from pathlib import Path
import subprocess
import sys
import unittest

from quote_demo.quote import quote

ROOT = Path(__file__).resolve().parents[1]


class QuoteTests(unittest.TestCase):
    def test_normal(self):
        """scenario.quote.normal: exactly divisible positive control."""
        self.assertEqual(quote(10000, 1000, 2500),
                         {"subtotal": 10000, "discount": 1000, "net": 9000,
                          "tax": 2250, "total": 11250})

    def test_floor_after_discount(self):
        """scenario.quote.floor: composition must preserve each floor boundary."""
        self.assertEqual(quote(101, 5000, 5000),
                         {"subtotal": 101, "discount": 50, "net": 51,
                          "tax": 25, "total": 76})

    def test_tax_floor_without_discount(self):
        self.assertEqual(quote(101, 0, 5000),
                         {"subtotal": 101, "discount": 0, "net": 101,
                          "tax": 50, "total": 151})

    def test_zero_and_full_discount(self):
        """scenario.quote.floor: endpoint controls."""
        self.assertEqual(quote(0, 0, 10000),
                         {"subtotal": 0, "discount": 0, "net": 0, "tax": 0, "total": 0})
        self.assertEqual(quote(100, 10000, 10000),
                         {"subtotal": 100, "discount": 100, "net": 0, "tax": 0, "total": 0})

    def test_invalid(self):
        """scenario.quote.invalid: reject before calling leaf functions."""
        for args in [(-1, 0, 0), (1, -1, 0), (1, 10001, 0), (1, 0, -1),
                     (1, 0, 10001), (True, 0, 0), (1, False, 0), (1, 0, True),
                     (1.0, 0, 0), (1, "0", 0), (1, 0, None)]:
            with self.subTest(args=args), self.assertRaisesRegex(ValueError, "^InvalidInput$"):
                quote(*args)

    def test_cli_normal(self):
        """scenario.quote.cli: actual process, exactly divisible control."""
        result = subprocess.run([sys.executable, "-m", "quote_demo", "10000", "1000", "2500"],
                                cwd=ROOT, capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        self.assertEqual(json.loads(result.stdout),
                         {"subtotal": 10000, "discount": 1000, "net": 9000,
                          "tax": 2250, "total": 11250})

    def test_cli_floor(self):
        """scenario.quote.cli: the defect must be visible through the real CLI."""
        result = subprocess.run([sys.executable, "-m", "quote_demo", "101", "5000", "5000"],
                                cwd=ROOT, capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        self.assertEqual(json.loads(result.stdout),
                         {"subtotal": 101, "discount": 50, "net": 51,
                          "tax": 25, "total": 76})

    def test_cli_invalid(self):
        """scenario.quote.invalid: stderr and exit status are observable contract."""
        for args in [("-1", "0", "0"), ("1", "10001", "0"),
                     ("1", "0", "-1"), ("1.5", "0", "0"), ("bad", "0", "0")]:
            with self.subTest(args=args):
                result = subprocess.run([sys.executable, "-m", "quote_demo", *args],
                                        cwd=ROOT, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, "")
                self.assertEqual(result.stderr, "InvalidInput\n")
