"""Actual host execution, source freshness and immutable record authoring."""

from copy import deepcopy
import json
from pathlib import Path
import sys

import pytest

from gdd_validator import validate
from gdd_validator.host import append_records, execute_check, snapshot_code, verification_records
from gdd_validator.loading import load_bundle
from gdd_validator.report import aggregate


@pytest.fixture
def project(tmp_path):
    (tmp_path / "domain.py").write_text("VALUE = 2\n")
    (tmp_path / "check.py").write_text("from domain import VALUE\nassert VALUE == 2\n")
    model = {"format": "gdd/0.3", "project": {"id": "host.example", "title": "Host utility fixture", "goal": "Check actual execution provenance."},
             "nodes": {"component.domain": {"kind": "component", "description": "Fixture domain."},
                       "rule.value": {"kind": "rule", "description": "VALUE equals two.",
                                      "rule": {"category": "invariant", "scope": "targeted", "statement": "VALUE equals two.", "blocking": True, "verification": ["python_tests"]}}},
             "edges": [{"id": "edge.value", "from": "rule.value", "relation": "constrains", "to": "component.domain"}]}
    model["record_files"] = ["records.json"]
    (tmp_path / "records.json").write_text(json.dumps({"format": "gdd-records/0.2", "project_id": "host.example", "records": {}}))
    manifest = tmp_path / "gdd.json"
    manifest.write_text(json.dumps(model))
    revision = load_bundle(manifest).revision
    records = {"format": "gdd-records/0.2", "project_id": "host.example", "records": {
        "check.value": {"kind": "check", "spec_revision": revision, "scope": ["rule.value"], "method": "python_tests", "required_environment": ["python"], "negative_controls": []}}}
    (tmp_path / "records.json").write_text(json.dumps(records))
    return tmp_path


def run(project, command=None, output="receipt.json", timeout=120):
    return execute_check(project, command or [sys.executable, "-B", "check.py"],
                         ["domain.py", "check.py"], output=project / output, timeout=timeout)


def attest(project, **overrides):
    args = {"root": project, "implementation": "implementation.host", "binding_id": "binding.value", "evidence_id": "evidence.value",
            "test_resource": "check.py", "checker": {"name": "python.assertion", "version": sys.version.split()[0]}, "environment": {"python": sys.version}}
    args.update(overrides)
    return verification_records(project / "gdd.json", "check.value", project / "receipt.json", **args)


def test_real_execution_and_current_evidence(project):
    receipt = run(project)
    assert receipt["result"] == "PASS" and receipt["executed"] and receipt["exit_code"] == 0
    fragment = attest(project)
    append_records(project / "records.json", fragment)
    report = validate(project / "gdd.json", profiles=["records"], scope=["component.domain"], host_context=fragment["host"])
    assert aggregate(report["checks"], "implementation") == "PASS"
    assert "policy" not in fragment["host"] and "trusted_decisions" not in fragment["host"]


def test_real_failure_is_not_promoted(project):
    (project / "domain.py").write_text("VALUE = 3\n")
    receipt = run(project)
    assert receipt["result"] == "FAIL" and receipt["exit_code"] == 1
    fragment = attest(project)
    append_records(project / "records.json", fragment)
    report = validate(project / "gdd.json", profiles=["records"], scope=["component.domain"], host_context=fragment["host"])
    assert aggregate(report["checks"], "implementation") == "FAIL"


@pytest.mark.parametrize("command", [[sys.executable, "-c", "raise SystemExit(2)"], ["/nonexistent-gdd-checker"]])
def test_unknown_and_unavailable_cannot_attest(project, command):
    receipt = run(project, command)
    assert receipt["result"] == "UNKNOWN"
    with pytest.raises(ValueError, match="completed actual"):
        attest(project)


def test_timeout_preserves_actual_partial_output(project):
    receipt = run(project, [sys.executable, "-u", "-c", "import time; print('started'); time.sleep(10)"], timeout=0.2)
    assert receipt["result"] == "UNKNOWN" and receipt["executed"] and "started" in receipt["stdout"]


def test_code_changed_during_execution_is_unknown(project):
    receipt = run(project, [sys.executable, "-c", "from pathlib import Path; Path('domain.py').write_text('VALUE=9\\n')"])
    assert receipt["exit_code"] == 0 and receipt["result"] == "UNKNOWN"
    with pytest.raises(ValueError):
        attest(project)


def test_later_code_drift_rejects_old_receipt(project):
    run(project)
    (project / "domain.py").write_text("VALUE = 9\n")
    with pytest.raises(ValueError, match="stale"):
        attest(project)


def test_existing_receipt_prevents_reexecuting_effects(project):
    run(project)
    with pytest.raises(FileExistsError):
        run(project, [sys.executable, "-c", "from pathlib import Path; Path('effect').write_text('ran')"])
    assert not (project / "effect").exists()


@pytest.mark.parametrize("resource", ["../domain.py", "/domain.py", "./domain.py", "domain.py/", "a\\b", "domain.py\0", ""])
def test_snapshot_refuses_ambiguous_or_escaping_paths(project, resource):
    with pytest.raises(ValueError):
        snapshot_code(project, [resource])


def test_snapshot_requires_regular_unique_files(project):
    for files in [["domain.py", "domain.py"], ["missing.py"], ["."]]:
        with pytest.raises(ValueError):
            snapshot_code(project, files)
    (project / "link.py").symlink_to(project / "domain.py")
    with pytest.raises(ValueError, match="symlinks"):
        snapshot_code(project, ["link.py"])


def test_snapshot_is_order_independent(project):
    assert snapshot_code(project, ["domain.py", "check.py"]) == snapshot_code(project, ["check.py", "domain.py"])


def test_empty_inventory_is_an_explicit_initial_code_base(project):
    empty = snapshot_code(project, [])
    assert empty["files"] == {} and empty["revision"].startswith("sha256:")


@pytest.mark.parametrize("defect", ["environment", "controls", "stale-plan", "unexecuted", "unknown-target", "collision", "absent-test"])
def test_incomplete_host_attestation_is_rejected(project, defect):
    run(project)
    overrides = {}
    if defect == "environment":
        overrides["environment"] = {}
    elif defect in {"controls", "stale-plan", "unknown-target"}:
        path = project / "records.json"
        document = json.loads(path.read_text())
        check = document["records"]["check.value"]
        if defect == "controls":
            check["negative_controls"] = ["actual-mutation"]
        elif defect == "stale-plan":
            check["spec_revision"] = "sha256:" + "0" * 64
        else:
            check["scope"] = ["rule.missing"]
        path.write_text(json.dumps(document))
    elif defect == "unexecuted":
        path = project / "receipt.json"
        receipt = json.loads(path.read_text()); receipt["executed"] = False
        path.write_text(json.dumps(receipt))
    elif defect == "collision":
        overrides["binding_id"] = "component.domain"
    else:
        overrides["test_resource"] = "other-test.py"
    with pytest.raises(ValueError):
        attest(project, **overrides)


def test_records_are_immutable_and_locked(project):
    run(project)
    fragment = attest(project)
    path = project / "records.json"
    append_records(path, fragment)
    before = path.read_bytes()
    with pytest.raises(ValueError, match="immutable"):
        append_records(path, fragment)
    assert path.read_bytes() == before and not (project / "records.json.lock").exists()
    (project / "records.json.lock").write_text("other writer")
    with pytest.raises(FileExistsError):
        append_records(path, {"records": {"other": {}}})
    assert (project / "records.json.lock").read_text() == "other writer"
    assert path.read_bytes() == before


def test_invalid_fragment_preserves_record_bytes(project):
    path = project / "records.json"
    before = path.read_bytes()
    with pytest.raises(Exception):
        append_records(path, {"records": {"evidence.invalid": {"kind": "evidence"}}})
    assert path.read_bytes() == before and not (project / "records.json.lock").exists()


def test_untrusted_receipt_alone_supplies_no_core_trust(project):
    run(project)
    append_records(project / "records.json", attest(project))
    report = validate(project / "gdd.json", profiles=["records"], scope=["component.domain"])
    assert aggregate(report["checks"], "implementation") == "UNKNOWN"
