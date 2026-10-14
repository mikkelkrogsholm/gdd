from journey_tools import write
write('src/parcel/__init__.py','"""Parcel pricing components."""\n')
write('src/parcel/tariffs.py','''"""Per-begun-kilogram tariff table, in integer cents."""

def rate(zone: str) -> int:
    if zone == "domestic":
        return 100
    if zone == "remote":
        return 150
    raise ValueError("zone must be domestic or remote")
''')
write('src/parcel/pricing.py','''"""Complete parcel quote policy."""
from .tariffs import rate


def quote(weight_grams: int, zone: str) -> int:
    if isinstance(weight_grams, bool) or not isinstance(weight_grams, int) or weight_grams <= 0:
        raise ValueError("weight must be a positive integer in grams")
    begun_kg = (weight_grams + 999) // 1000
    return begun_kg * rate(zone)
''')
write('src/parcel/cli.py','''"""Parse parcel command arguments and present one quote."""
import argparse
from .pricing import quote


def positive_grams(text: str) -> int:
    try:
        value = int(text)
    except ValueError:
        raise argparse.ArgumentTypeError("weight must be a positive integer in grams") from None
    if value <= 0:
        raise argparse.ArgumentTypeError("weight must be a positive integer in grams")
    return value


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="Quote a parcel in integer cents.")
    parser.add_argument("weight", type=positive_grams, help="positive integer weight in grams")
    parser.add_argument("zone", choices=("domestic", "remote"))
    args = parser.parse_args(argv)
    print(quote(args.weight, args.zone))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
''')
write('tests/shared_contract.py','''"""Fixed common assertions; the model does not execute this file."""
import os
from pathlib import Path
import subprocess
import sys
import unittest

SOURCE = None


def configure(source):
    global SOURCE
    SOURCE = Path(source).resolve()
    sys.path.insert(0, str(SOURCE))


def cli(*args):
    env = dict(os.environ, PYTHONPATH=str(SOURCE), PYTHONDONTWRITEBYTECODE="1")
    return subprocess.run([sys.executable, "-B", "-m", "parcel.cli", *map(str, args)],
                          text=True, capture_output=True, env=env)


class ContractTests(unittest.TestCase):
    def test_domestic_boundaries(self):
        from parcel.pricing import quote
        for weight, expected in [(1,100),(999,100),(1000,100),(1001,200),(2000,200),(2001,300),(10**18,10**17)]:
            with self.subTest(weight=weight):
                self.assertEqual(quote(weight,"domestic"),expected)
                result=cli(weight,"domestic")
                self.assertEqual((result.returncode,result.stdout,result.stderr),(0,f"{expected}\\n",""))

    def test_api_rejects_invalid_weights(self):
        from parcel.pricing import quote
        for weight in [0,-1,1.5,"1",None,True,False]:
            with self.subTest(weight=weight), self.assertRaises(ValueError):
                quote(weight,"domestic")

    def test_api_rejects_invalid_zones(self):
        from parcel.pricing import quote
        from parcel.tariffs import rate
        for zone in ["", "abroad", None]:
            with self.subTest(zone=zone):
                with self.assertRaises(ValueError):quote(1,zone)
                with self.assertRaises(ValueError):rate(zone)

    def test_cli_rejects_invalid_arguments(self):
        for args in [("0","domestic"),("-1","remote"),("1.5","domestic"),("bad","remote"),("1","abroad"),(),("1",)]:
            with self.subTest(args=args):
                result=cli(*args)
                self.assertEqual(result.returncode,2)
                self.assertEqual(result.stdout,"")
                self.assertIn("error:",result.stderr)

    def test_tariff_values_and_repeatability(self):
        from parcel.tariffs import rate
        from parcel.pricing import quote
        self.assertEqual(rate("domestic"),100)
        self.assertEqual(rate("remote"),150)
        self.assertEqual(quote(1001,"domestic"),quote(1001,"domestic"))
''')
write('tests/test_a.py','''"""Fixed contract A assertions: no handling fee."""
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
                self.assertEqual((result.returncode,result.stdout,result.stderr),(0,f"{expected}\\n",""))


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]],verbosity=2)
''')
write('tests/check_architecture.py','''"""Concrete static-import checker for the fixed parcel package.

Exit 0 means PASS, 1 means a known violation, and 2 means UNKNOWN.
Dynamic import syntax or missing/parsing-failed source remains UNKNOWN.
"""
import ast
import json
from pathlib import Path
import sys


def check(source):
    source = Path(source)
    edges=set()
    for module in ("cli","pricing","tariffs"):
        path=source/"parcel"/(module+".py")
        if not path.is_file():
            print(json.dumps({"result":"UNKNOWN","reason":"missing actual code","resource":str(path)}))
            return 2
        try:tree=ast.parse(path.read_text(),filename=str(path))
        except (SyntaxError,OSError) as exc:
            print(json.dumps({"result":"UNKNOWN","reason":str(exc)}));return 2
        for node in ast.walk(tree):
            if isinstance(node,ast.Call) and ((isinstance(node.func,ast.Name) and node.func.id=="__import__") or (isinstance(node.func,ast.Attribute) and node.func.attr=="import_module")):
                print(json.dumps({"result":"UNKNOWN","reason":"dynamic import outside static checker profile"}));return 2
            targets=[]
            if isinstance(node,ast.Import):targets=[x.name for x in node.names]
            elif isinstance(node,ast.ImportFrom):
                if node.level == 1:targets=[node.module] if node.module else [x.name for x in node.names]
                elif node.module == "parcel":targets=[x.name for x in node.names]
                elif node.module:targets=[node.module]
            for target in targets:
                if target.startswith("parcel."):target=target.removeprefix("parcel.").split(".")[0]
                if target in {"cli","pricing","tariffs"}:edges.add((module,target))
    forbidden=("cli","tariffs") in edges
    missing={('cli','pricing'),('pricing','tariffs')}-edges
    result="FAIL" if forbidden or missing else "PASS"
    print(json.dumps({"result":result,"actual_imports":sorted(edges),"forbidden_cli_tariffs":forbidden,"missing_required_imports":sorted(missing)}))
    return 1 if result=="FAIL" else 0


if __name__ == "__main__":
    raise SystemExit(check(sys.argv[1] if len(sys.argv)>1 else "src"))
''')
