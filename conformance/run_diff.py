"""Run the Stable-identity model diff suite's named behavioral assertions in one isolated pytest process.

Requires the project's dev dependencies. This fixed development harness never
executes commands supplied by a GDD model or a worker result.
"""

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

from gdd_validator import __version__
ROOT = Path(__file__).resolve().parents[1]


def run():
    cases = json.loads((ROOT / "conformance/diff.cases.json").read_text())
    with tempfile.TemporaryDirectory(prefix="gdd-model-diff-conformance-") as temp:
        junit = Path(temp) / "results.xml"
        argv = [
            sys.executable,
            "-B",
            "-m",
            "pytest",
            "-q",
            "-p",
            "no:cacheprovider",
            "--junitxml",
            str(junit),
        ] + [case["test"] for case in cases]
        executed = subprocess.run(
            argv, cwd=ROOT, capture_output=True, text=True, check=False
        )
        observed = ET.parse(junit).findall(".//testcase") if junit.exists() else []
    results = []
    for case in cases:
        file, function = case["test"].split("::")
        module = file.removesuffix(".py").replace("/", ".")
        matches = [
            test
            for test in observed
            if test.get("classname") == module
            and (
                test.get("name") == function
                or test.get("name", "").startswith(function + "[")
            )
        ]
        passed = bool(matches) and all(not list(test) for test in matches)
        results.append(
            {
                "id": case["id"],
                "assertion": case["assertion"],
                "result": "PASS" if passed else "FAIL",
                "test_instances": len(matches),
            }
        )
    return {
        "format": "gdd-model-diff-conformance/0.1",
        "tool": "gdd-validator/" + __version__,
        "cases": results,
        "case_count": len(results),
        "test_instances": sum(r["test_instances"] for r in results),
        "pytest_exit": executed.returncode,
        "passed": executed.returncode == 0
        and all(r["result"] == "PASS" for r in results),
        "stdout": executed.stdout,
        "stderr": executed.stderr,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.output and args.output.exists():
        parser.error("Output already exists.")
    result = run()
    text = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        with args.output.open("x", encoding="utf-8") as file:
            file.write(text)
    else:
        sys.stdout.write(text)
    raise SystemExit(0 if result["passed"] else 1)
