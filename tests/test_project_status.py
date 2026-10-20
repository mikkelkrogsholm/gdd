"""Read-only status composition over real assessors and explicit inputs."""

from copy import deepcopy
import importlib
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator
from referencing import Registry, Resource

from gdd_validator.loading import load_bundle, schema
from gdd_validator.project_status import project_status, status_text
from gdd_validator.scaffold import init_project
from gdd_validator.workflow import prepare_workflow
from gdd_validator.workflow_run import create_run, run_status

ROOT = Path(__file__).resolve().parents[1]
READY = ROOT / "docs/tutorial/refined.gdd.json"


def validate_status(result):
    definition = schema("status")
    registry = Registry().with_resource(schema("report")["$id"], Resource.from_contents(schema("report")))
    Draft202012Validator(definition, registry=registry).validate(result)


def follow(result, pointer):
    item = result
    for token in pointer.removeprefix("/").split("/"):
        item = item[int(token)] if isinstance(item, list) else item[token.replace("~1", "/").replace("~0", "~")]
    return item


def test_starter_questions_and_blockers_are_usable_without_host(tmp_path):
    init_project(tmp_path / "project")
    path = tmp_path / "project/gdd.json"
    before = path.read_bytes()
    result = project_status(path, scope=["action.main"])
    validate_status(result)
    assert result["assessments"]["model"]["result"] == "PASS"
    assert result["assessments"]["readiness"]["result"] == "FAIL"
    assert result["open_questions"] and result["open_questions"][0]["id"]
    assert result["workflow"] is None
    assert result["exit_code"] == 1
    assert path.read_bytes() == before


def test_ready_scope_does_not_imply_authority_or_evidence():
    result = project_status(READY, scope=["action.add"])
    validate_status(result)
    for dimension in ("model", "readiness"):
        assert result["assessments"][dimension]["result"] == "PASS"
    for dimension in ("authorization", "implementation"):
        assert result["assessments"][dimension] == {"assessed": False, "result": None, "check_refs": []}
    assert result["exit_code"] == 0
    assert "complete" not in result and "result" not in result


def test_explicit_empty_host_is_assessed_unresolved():
    result = project_status(READY, scope=["action.add"], host_context={})
    validate_status(result)
    assert result["assessments"]["authorization"]["result"] == "UNRESOLVED"
    assert result["assessments"]["implementation"]["result"] == "UNKNOWN"
    assert result["exit_code"] == 2


def test_known_failure_outranks_unknown(tmp_path):
    init_project(tmp_path / "project")
    result = project_status(tmp_path / "project/gdd.json", scope=["action.main"], host_context={})
    assert result["assessments"]["readiness"]["result"] == "FAIL"
    assert result["assessments"]["authorization"]["result"] == "UNRESOLVED"
    assert result["exit_code"] == 1


@pytest.mark.parametrize("uri,expected", [("https://example.invalid/spec", 2), ("../outside.txt", 1)])
def test_assessed_integration_checks_affect_exit_without_erasing_readiness(tmp_path, uri, expected):
    model = json.loads(READY.read_text())
    model.setdefault("sources", {})["source.normative"] = {
        "role": "knowledge", "title": "Normative input", "uri": uri,
        "format": "text/plain", "normative": True, "revision": {"sha256": "0" * 64},
    }
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model))
    result = project_status(path, scope=["action.add"])
    validate_status(result)
    assert result["assessments"]["readiness"]["result"] == "PASS"
    assert result["spec_revision"] is None
    assert result["exit_code"] == expected
    assert any(item["dimension"] == "integration" and item["result"] != "PASS"
               for item in result["reports"]["model"]["checks"])


def test_inputs_remain_explicit_and_cannot_silently_override_trust():
    host = {"implementation": "implementation.example", "code_revision": "revision-a"}
    original = deepcopy(host)
    result = project_status(READY, scope=["action.add"], host_context=host, implementation="implementation.example")
    assert result["inputs"]["code_revision"] == "revision-a"
    assert host == original
    with pytest.raises(ValueError, match="conflicts"):
        project_status(READY, host_context=host, code_revision="revision-b")
    with pytest.raises(ValueError):
        project_status(READY, scope=[1, "action.add"])
    with pytest.raises(ValueError, match="host input"):
        project_status(READY, host_context=[], implementation="implementation.example")


def test_actions_are_deterministic_bounded_and_traceable(tmp_path):
    init_project(tmp_path / "project")
    path = tmp_path / "project/gdd.json"
    first = project_status(path, scope=["action.main"])
    assert first == project_status(path, scope=["action.main", "action.main"])
    assert 0 < len(first["next_actions"]) <= 12
    for action in first["next_actions"]:
        source = follow(first, action["source"])
        if action["diagnostic_code"]:
            assert source["code"] == action["diagnostic_code"]
            assert source["message"] == action["reason"]
        else:
            assert source in first["inputs"]["missing"]
        assert action["argv"][0] == "gdd"
    text = status_text(first)
    assert text.endswith("\n")
    for name, assessment in first["assessments"].items():
        assert f"{name}: {assessment['result'] if assessment['assessed'] else 'UNASSESSED'}" in text
    for action in first["next_actions"]:
        assert action["source"] in text and action["reason"] in text
    assert "Exit code: 1" in text


@pytest.fixture
def workflow(tmp_path):
    request = {
        "format": "gdd-work-request/0.1", "id": "work.add",
        "objective": "Implement addition", "implementation": "implementation.example",
        "base_code_revision": "synthetic-0",
        "packages": [{"id": "work.arithmetic", "scope": ["action.add"],
                      "workspace": "../workspace", "write_resources": ["src/add.py"],
                      "read_resources": [], "deliverables": ["Addition implementation"],
                      "integration_plan": "Review and run checks"}],
        "serialized": [], "integration": {"owner": "actor.integrator", "checks": []},
    }
    plan = prepare_workflow(READY, request)["artifact"]
    directory = tmp_path / "run"
    create_run(READY, plan, directory)
    return directory


def test_optional_run_exactly_projects_existing_status_without_journal_writes(workflow):
    before = {str(p): p.read_bytes() for p in workflow.rglob("*") if p.is_file()}
    result = project_status(READY, scope=["action.add"], run=workflow)
    validate_status(result)
    expected = run_status(READY, workflow)
    assert result["workflow"]["state"] == expected["state"]
    assert result["workflow"]["acceptance_current"] == expected["acceptance_current"]
    assert result["workflow"]["revision"] == expected["revision"]
    assert result["workflow"]["available_packages"] == expected["available_packages"]
    assert before == {str(p): p.read_bytes() for p in workflow.rglob("*") if p.is_file()}
    assert "work.arithmetic: pending" in status_text(result)


@pytest.mark.parametrize("current", [True, False])
def test_recorded_acceptance_is_only_current_when_existing_assessor_says_so(monkeypatch, workflow, current):
    # The existing workflow suite owns acceptance evidence construction. Here a
    # synthetic assessor response verifies composition does not reinterpret it.
    observed = run_status(READY, workflow)
    observed["state"]["status"] = "accepted"
    observed["acceptance_current"] = current
    monkeypatch.setattr(importlib.import_module("gdd_validator.project_status"), "run_status",
                        lambda *a, **kw: deepcopy(observed))
    result = project_status(READY, run=workflow)
    assert result["workflow"]["acceptance_current"] is current
    assert f"acceptance_current={str(current).lower()}" in status_text(result)
    validate_status(result)


def test_missing_run_is_unknown_and_malformed_journal_is_fail(tmp_path):
    missing = project_status(READY, scope=["action.add"], run=tmp_path / "absent")
    assert missing["exit_code"] == 2
    assert missing["workflow_error"]["check"]["result"] == "UNKNOWN"
    validate_status(missing)
    directory = tmp_path / "invalid"
    directory.mkdir()
    (directory / "run.json").write_text('{}')
    malformed = project_status(READY, scope=["action.add"], run=directory)
    assert malformed["exit_code"] == 1
    assert malformed["workflow_error"]["check"]["result"] == "FAIL"
    validate_status(malformed)


def test_unrelated_historical_record_does_not_erase_readiness(tmp_path):
    model = json.loads(READY.read_text())
    model["record_files"] = ["records.json"]
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model))
    record = {"kind": "binding", "implementation": "implementation.old",
              "spec_revision": "sha256:" + "0" * 64, "targets": ["action.retired"],
              "role": "code", "resource": "old.py", "selector": {"kind": "whole-document"}, "status": "planned"}
    (tmp_path / "records.json").write_text(json.dumps({"format": "gdd-records/0.2", "project_id": model["project"]["id"], "records": {"binding.old": record}}))
    result = project_status(path, scope=["action.add"])
    assert result["assessments"]["model"]["result"] == "UNKNOWN"
    assert result["assessments"]["readiness"]["result"] == "PASS"
    assert result["exit_code"] == 2
    assert any(d["related_ids"] == ["binding.old"] for c in result["reports"]["model"]["checks"] for d in c["diagnostics"])


def test_run_model_revision_is_not_silently_rebound(workflow, tmp_path):
    changed = json.loads(READY.read_text())
    changed["project"]["title"] = "Changed current model"
    path = tmp_path / "changed.json"
    path.write_text(json.dumps(changed))
    result = project_status(path, scope=["action.add"], run=workflow)
    assert result["assessments"]["readiness"]["result"] == "PASS"
    assert result["workflow"]["acceptance_current"] is False
    assert any(c["result"] == "FAIL" for c in result["workflow"]["report"]["checks"])
    assert result["exit_code"] == 1
    validate_status(result)


def test_explicit_code_identity_never_manufactures_implementation_evidence():
    result = project_status(READY, scope=["action.add"],
                            implementation="implementation.example", code_revision="claimed")
    assert result["assessments"]["implementation"]["result"] == "UNKNOWN"
    assert result["assessments"]["authorization"]["result"] == "UNRESOLVED"
    assert result["inputs"]["implementation"] == "implementation.example"
    assert result["inputs"]["code_revision"] == "claimed"
    assert "host_context" in result["inputs"]["missing"]
