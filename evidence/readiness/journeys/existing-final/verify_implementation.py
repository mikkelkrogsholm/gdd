"""Run actual checks and isolated defects through the installed host API."""
import json
from pathlib import Path
import shutil
import sys
from gdd_validator.host import execute_check, verification_records, append_records

root = Path.cwd()
files = ["reservation.py", "test_reservation.py"]
code = (root / "reservation.py").read_text()
mutations = {
    "replacement-return": code.replace("return value", "return Reservation()", 1),
    "resurrection": code.replace('(\"draft\", \"reserved\")', '(\"draft\", \"reserved\", \"cancelled\")', 1),
    "repeat-cancel-error": code.replace('(\"reserved\", \"cancelled\")', '(\"reserved\",)', 1),
}
controls = []
for name, mutated in mutations.items():
    assert mutated != code, name
    isolated = root / "evidence" / "negative-controls" / name
    isolated.mkdir(parents=True)
    (isolated / "reservation.py").write_text(mutated)
    shutil.copyfile(root / "test_reservation.py", isolated / "test_reservation.py")
    receipt = execute_check(isolated, [sys.executable, "-B", "-m", "unittest", "-v", "test_reservation"], files, output=isolated / "run.json")
    assert receipt["result"] == "FAIL", (name, receipt)
    controls.append(name)
    print(name, receipt["result"])

receipt = execute_check(root, [sys.executable, "-B", "-m", "unittest", "-v", "test_reservation"], files, output=root / "evidence" / "integrated-test.run.json")
assert receipt["result"] == "PASS", receipt
environment = {"python": sys.version}
fragment = verification_records(root / "gdd.json", "check.lifecycle.v3", root / "evidence" / "integrated-test.run.json", root=root, implementation="implementation.reservations", binding_id="binding.lifecycle.v1", evidence_id="evidence.lifecycle.v1", test_resource="test_reservation.py", checker={"name":"stdlib.unittest", "version":sys.version.split()[0]}, environment=environment, verified_negative_controls=controls)
append_records(root / "records.json", fragment)
host = json.loads((root / "host-integrated.json").read_text())
host.update(fragment["host"])
host["policy"]["activity"] = "acceptance"
(root / "host-acceptance.json").write_text(json.dumps(host, indent=2) + "\n")
(root / "evidence" / "verified-fragment.json").write_text(json.dumps(fragment, indent=2) + "\n")
print("Combined revision evidence:", receipt["result"])
print("Attested controls:", controls)
