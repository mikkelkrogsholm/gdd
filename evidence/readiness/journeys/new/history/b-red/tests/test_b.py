"""Fixed contract B assertions: remote has exactly one 50-cent fee.

These expectations are authored before the fee implementation and never edited
between the preserved red execution against A and green execution against B.
"""
from pathlib import Path
import sys
import unittest
from shared_contract import ContractTests, configure, cli

configure(sys.argv[1] if len(sys.argv)>1 else Path(__file__).resolve().parents[1]/"src")


class ContractB(unittest.TestCase):
    def test_remote_amounts_with_one_flat_fee(self):
        from parcel.pricing import quote
        for weight, expected in [(1,200),(999,200),(1000,200),(1001,350),(2000,350),(2001,500)]:
            with self.subTest(weight=weight):
                self.assertEqual(quote(weight,"remote"),expected)
                result=cli(weight,"remote")
                self.assertEqual((result.returncode,result.stdout,result.stderr),(0,f"{expected}\n",""))

    def test_remote_cli_has_fee(self):
        for weight, expected in [(1,200),(1001,350),(2001,500)]:
            with self.subTest(weight=weight):
                result=cli(weight,"remote")
                self.assertEqual((result.returncode,result.stdout,result.stderr),(0,f"{expected}\n",""))

    def test_handling_fee_once_per_quote(self):
        from parcel.pricing import quote
        self.assertEqual(quote(1,"remote"),200)
        self.assertEqual(quote(1000,"remote"),200)
        self.assertEqual(quote(1001,"remote"),350)
        self.assertEqual(quote(3000,"remote"),500)
        self.assertEqual(quote(3001,"remote"),650)
        self.assertEqual(quote(3001,"domestic"),400)


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]],verbosity=2)
