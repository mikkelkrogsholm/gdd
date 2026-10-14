"""Fixed common assertions; the model does not execute this file."""
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
                self.assertEqual((result.returncode,result.stdout,result.stderr),(0,f"{expected}\n",""))

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
