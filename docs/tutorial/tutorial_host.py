"""Explicit local tutorial operations; read this file before selecting execution.

Only stdlib and the installed GDD package are used. This is a worked host script,
not a validator command. It neither authenticates externally supplied receipts
nor decides operational authority. Run stages in the documented fresh directory.
"""

import argparse
import json
from pathlib import Path
import shutil
import sys

from gdd_validator.host import append_records, execute_check, snapshot_code, verification_records
from gdd_validator.loading import load_bundle

FILES = ["integer_add.py", "test_integer_add.py"]
CONTROL = "isolated-subtraction-mutant"
IMPLEMENTATION = "implementation.integer-addition.python"


def write_new(path, value):
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")


def fragment(root, *, binding_id="binding.sum.v1", evidence_id="evidence.sum.v1"):
    # The caller has inspected the assertions and performed the isolated control.
    return verification_records(
        root / "evidence.gdd.json", "check.sum", root / "pass.receipt.json",
        root=root, implementation=IMPLEMENTATION, binding_id=binding_id,
        evidence_id=evidence_id, test_resource="test_integer_add.py",
        checker={"name": "stdlib.unittest", "version": sys.version.split()[0]},
        environment={"python": sys.version}, verified_negative_controls=[CONTROL],
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stage", choices=["prepare", "negative", "check", "record", "drift"])
    parser.add_argument("root", type=Path)
    parser.add_argument("--attest-reviewed-checks", action="store_true",
                        help="Explicit local caller attestation after reviewing code, assertions and actual receipts.")
    args = parser.parse_args()
    if args.stage == "record" and not args.attest_reviewed_checks:
        parser.error("record requires --attest-reviewed-checks after actual local review")
    root = args.root.resolve(strict=True)
    model = root / "evidence.gdd.json"
    command = [sys.executable, "-B", "-m", "unittest", "-v", "test_integer_add"]

    if args.stage == "prepare":
        # record_files is already in the finalized model; records do not change it.
        project = json.loads(model.read_text())["project"]["id"]
        write_new(root / "records.json", {"format": "gdd-records/0.2", "project_id": project, "records": {}})
        revision = load_bundle(model).revision
        if revision is None:
            raise ValueError("The finalized model revision must resolve.")
        append_records(root / "records.json", {"records": {"check.sum": {
            "kind": "check", "spec_revision": revision, "scope": ["rule.sum", "scenario.add"],
            "method": "python_unittest", "required_environment": ["python"],
            "negative_controls": [CONTROL],
        }}})
        print("Created current immutable check.sum; no command has been executed.")

    elif args.stage == "negative":
        mutant = root / "mutant"
        mutant.mkdir()  # Refuse accidental reuse of an earlier experiment.
        for name in FILES:
            shutil.copyfile(root / name, mutant / name)
        source = mutant / "integer_add.py"
        original = source.read_text()
        if original.count("return left + right") != 1:
            raise ValueError("Select the documented source before applying this control.")
        source.write_text(original.replace("return left + right", "return left - right"))
        receipt = execute_check(mutant, command, FILES, output=root / "negative.receipt.json")
        if (receipt["result"] != "FAIL" or receipt["exit_code"] != 1
                or "AssertionError" not in receipt["stderr"]
                or "FAILED (failures=" not in receipt["stderr"]):
            raise ValueError("The actual subtraction mutant must fail the selected assertions.")
        print("Actual mutant FAIL preserved; current implementation is untouched.")

    elif args.stage == "check":
        receipt = execute_check(root, command, FILES, output=root / "pass.receipt.json")
        if receipt["result"] != "PASS":
            raise ValueError("The current implementation did not pass; inspect its actual receipt.")
        print("Actual current-code PASS preserved in pass.receipt.json.")

    elif args.stage == "record":
        # This check is a guard against mistakes, not authentication of a file.
        # The trusted local caller owns the preceding execution and coverage review.
        negative = json.loads((root / "negative.receipt.json").read_text())
        if (negative["executed"] is not True or negative["result"] != "FAIL"
                or negative["exit_code"] != 1 or negative["command"] != command
                or negative["cwd"] != str(root / "mutant")
                or negative["snapshot"] != snapshot_code(root / "mutant", FILES)):
            raise ValueError("Review the actual isolated negative control before attesting it.")
        result = fragment(root)
        append_records(root / "records.json", result)
        write_new(root / "host-context.json", result["host"])
        print("Appended binding.sum.v1 and evidence.sum.v1; explicit local evidence trust only.")

    else:
        # A behavior-preserving edit still changes selected source identity.
        changed = root / "changed"
        changed.mkdir()
        for name in [*FILES, "evidence.gdd.json", "records.json", "pass.receipt.json"]:
            shutil.copyfile(root / name, changed / name)
        source = changed / "integer_add.py"
        source.write_text(source.read_text() + "\n# Later edit: old evidence cannot cover these new bytes.\n")
        host = json.loads((root / "host-context.json").read_text())
        host["code_revision"] = snapshot_code(changed, FILES)["revision"]
        write_new(root / "stale-host-context.json", host)
        try:
            fragment(changed, binding_id="binding.sum.v2", evidence_id="evidence.sum.v2")
        except ValueError as error:
            if "stale" not in str(error):
                raise
            write_new(root / "stale-authoring.json", {"result": "REFUSED", "reason": str(error)})
        else:
            raise AssertionError("Stale receipt unexpectedly authorized new evidence.")
        print("Selected source changed; old receipt cannot author new current evidence.")


if __name__ == "__main__":
    main()
