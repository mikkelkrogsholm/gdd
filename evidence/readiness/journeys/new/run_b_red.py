from journey_tools import ROOT,write
from gdd_validator.host import execute_check,snapshot_code
import json,sys,shutil,hashlib
write('tests/test_b.py','''"""Fixed contract B assertions: remote has exactly one 50-cent fee.

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
                self.assertEqual((result.returncode,result.stdout,result.stderr),(0,f"{expected}\\n",""))

    def test_remote_cli_has_fee(self):
        for weight, expected in [(1,200),(1001,350),(2001,500)]:
            with self.subTest(weight=weight):
                result=cli(weight,"remote")
                self.assertEqual((result.returncode,result.stdout,result.stderr),(0,f"{expected}\\n",""))

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
''')
files=list(json.loads((ROOT/'evidence/a-code.json').read_text())['files'])+['tests/test_b.py']
code=snapshot_code(ROOT,files);write('evidence/b-red-code.json',code)
receipt=execute_check(ROOT,[sys.executable,'-B','tests/test_b.py','src'],files,output=ROOT/'evidence/b-fixed-assertions-against-a.run.json')
write('evidence/b-fixed-test-hash.json',{'resource':'tests/test_b.py','sha256':hashlib.sha256((ROOT/'tests/test_b.py').read_bytes()).hexdigest(),'authored_before_implementation':True,'red_receipt':'evidence/b-fixed-assertions-against-a.run.json'})
print(receipt['result']);print(receipt['stderr']);assert receipt['result']=='FAIL'
for path in ['src','tests']:shutil.copytree(ROOT/path,ROOT/'history/b-red'/path)
