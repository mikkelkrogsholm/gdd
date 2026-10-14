"""Fixed contract A assertions: no handling fee."""
from pathlib import Path
import sys
import unittest
from shared_contract import ContractTests, configure, cli

configure(sys.argv[1] if len(sys.argv)>1 else Path(__file__).resolve().parents[1]/"src")


class ContractA(unittest.TestCase):
    def test_remote_amounts(self):
        from parcel.pricing import quote
        for weight, expected in [(1,150),(999,150),(1000,150),(1001,300),(2000,300),(2001,450)]:
            with self.subTest(weight=weight):
                self.assertEqual(quote(weight,"remote"),expected)
                result=cli(weight,"remote")
                self.assertEqual((result.returncode,result.stdout,result.stderr),(0,f"{expected}\n",""))


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]],verbosity=2)
