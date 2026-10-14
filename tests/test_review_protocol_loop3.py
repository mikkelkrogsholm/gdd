"""Current binding retirement and evidence-scope integrity regressions."""

from copy import deepcopy
import json

import pytest

from gdd_validator import validate
from gdd_validator.loading import digest, load_bundle
from gdd_validator.report import aggregate


@pytest.fixture
def fixture(tmp_path):
    model = {
        "format": "gdd/0.2",
        "project": {
            "id": "review",
            "title": "Review",
            "goal": "Test record applicability",
        },
        "nodes": {},
        "edges": [],
        "record_files": ["records.json"],
    }
    for name in ("x", "y"):
        model["nodes"][f"entity.{name}"] = {
            "kind": "entity",
            "description": "Value",
            "fields": {},
        }
        model["nodes"][f"rule.{name}"] = {
            "kind": "rule",
            "description": "Required check",
            "rule": {
                "category": "invariant",
                "statement": "Required behavior",
                "scope": "targeted",
                "blocking": True,
                "verification": ["synthetic"],
            },
        }
        model["edges"].append(
            {
                "id": f"edge.constraint.{name}",
                "from": f"rule.{name}",
                "relation": "constrains",
                "to": f"entity.{name}",
            }
        )
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model), encoding="utf-8")
    write_records(path, {})
    revision = load_bundle(path).revision
    records = {}
    for name in ("x", "y"):
        plan = {
            "kind": "check",
            "spec_revision": revision,
            "scope": [f"rule.{name}"],
            "method": "synthetic",
            "required_environment": [],
            "negative_controls": [],
        }
        records[f"check.{name}"] = plan
        records[f"evidence.{name}"] = {
            "kind": "evidence",
            "check": f"check.{name}",
            "check_revision": digest(plan),
            "implementation": "implementation.x",
            "spec_revision": revision,
            "code_revision": "code.x",
            "checker": {"name": "synthetic", "version": "1"},
            "scope": [f"rule.{name}"],
            "result": "PASS",
            "executed": True,
            "reason": "Synthetic host claim; no application execution",
            "environment": {},
            "evidence_uri": "fixture://synthetic-result",
        }
        records[f"binding.{name}"] = {
            "kind": "binding",
            "spec_revision": revision,
            "targets": [f"rule.{name}"],
            "implementation": "implementation.x",
            "role": "test",
            "resource": "test.py",
            "selector": {"kind": "whole-document"},
            "status": "checked",
            "revision": "resource.x",
            "code_revision": "code.x",
        }
    host = {
        "implementation": "implementation.x",
        "code_revision": "code.x",
        "supported_methods": ["synthetic"],
        "trusted_evidence": ["evidence.x", "evidence.y"],
        "verified_bindings": ["binding.x", "binding.y"],
        "environment": {},
        "negative_controls": [],
    }
    return path, records, host


def write_records(path, records):
    (path.parent / "records.json").write_text(
        json.dumps(
            {
                "format": "gdd-records/0.2",
                "project_id": "review",
                "records": records,
            }
        ),
        encoding="utf-8",
    )


def report(fixture, scope=None):
    path, records, host = fixture
    write_records(path, records)
    return validate(
        path, profiles=["records"], scope=scope or ["entity.x"], host_context=host
    )


def implementation_result(value, rule="rule.x"):
    return next(
        c["result"]
        for c in value["checks"]
        if c["dimension"] == "implementation" and c["scope"] == [rule]
    )


def diagnostics(value):
    return [d for c in value["checks"] for d in c["diagnostics"]]


@pytest.mark.parametrize(
    "change",
    [
        {"code_revision": "code.other"},
        {"implementation": "implementation.other"},
        {"spec_revision": "sha256:" + "a" * 64},
        {"targets": ["rule.y"]},
    ],
)
def test_incompatible_verified_binding_does_not_retire_current_proof(fixture, change):
    _, records, host = fixture
    assert implementation_result(report(fixture)) == "PASS"
    records["binding.other"] = {
        **deepcopy(records["binding.x"]),
        "supersedes": "binding.x",
        **change,
    }
    host["verified_bindings"].append("binding.other")
    value = report(fixture)
    assert implementation_result(value) == "PASS"
    assert not any(d["code"] == "BINDING_REVISION_MISSING" for d in diagnostics(value))


def test_narrow_binding_cannot_retire_wider_current_mapping(fixture):
    _, records, host = fixture
    records["binding.x"]["targets"].append("rule.y")
    records.pop("binding.y")
    host["verified_bindings"] = ["binding.x"]
    records["binding.other"] = {
        **deepcopy(records["binding.x"]),
        "supersedes": "binding.x",
        "targets": ["rule.x"],
    }
    host["verified_bindings"].append("binding.other")
    value = report(fixture, ["entity.x", "entity.y"])
    assert implementation_result(value, "rule.x") == "PASS"
    assert implementation_result(value, "rule.y") == "PASS"


def test_unverified_compatible_binding_does_not_retire_current_mapping(fixture):
    _, records, _ = fixture
    records["binding.other"] = {
        **deepcopy(records["binding.x"]),
        "supersedes": "binding.x",
        "role": "design",
    }
    assert implementation_result(report(fixture)) == "PASS"


def test_compatible_binding_may_change_resource_selector_revision_and_role(fixture):
    _, records, host = fixture
    records["binding.other"] = {
        **deepcopy(records["binding.x"]),
        "supersedes": "binding.x",
        "resource": "renamed.py",
        "selector": {"kind": "opaque", "value": "new symbol"},
        "revision": "resource.new",
        "role": "code",
        "status": "observed",
        "targets": ["rule.x", "entity.x"],
    }
    host["verified_bindings"].append("binding.other")
    value = report(fixture)
    assert aggregate(value["checks"], "model") == "PASS"
    assert implementation_result(value) == "PASS"


def test_compatible_verified_design_successor_retires_old_code_mapping(fixture):
    _, records, host = fixture
    records["binding.other"] = {
        **deepcopy(records["binding.x"]),
        "supersedes": "binding.x",
        "role": "design",
    }
    host["verified_bindings"].append("binding.other")
    assert implementation_result(report(fixture)) == "UNKNOWN"


@pytest.mark.parametrize("result", ["PASS", "FAIL"])
def test_dangling_current_evidence_scope_is_invalid_and_not_usable(fixture, result):
    _, records, _ = fixture
    records["evidence.x"].update(scope=["rule.x", "entity.deleted"], result=result)
    value = report(fixture)
    assert aggregate(value["checks"], "model") == "FAIL"
    assert implementation_result(value) == "UNKNOWN"
    assert any(
        d["code"] == "RECORD_INVALID" and "entity.deleted" in d["message"]
        for d in diagnostics(value)
    )


@pytest.mark.parametrize("extra", ["entity.y", "rule.y", "edge.constraint.y"])
def test_known_extra_evidence_scope_resolves_without_implicit_extra_proof(
    fixture, extra
):
    _, records, _ = fixture
    records["evidence.x"]["scope"].append(extra)
    value = report(fixture)
    assert aggregate(value["checks"], "model") == "PASS"
    assert implementation_result(value) == "PASS"
    checked_scopes = [
        c["scope"] for c in value["checks"] if c["dimension"] == "implementation"
    ]
    assert checked_scopes == [["rule.x"]]


def test_record_id_is_not_an_extra_model_scope_target(fixture):
    _, records, _ = fixture
    records["evidence.x"]["scope"].append("binding.x")
    value = report(fixture)
    assert aggregate(value["checks"], "model") == "FAIL"
    assert implementation_result(value) == "UNKNOWN"


def test_historical_evidence_scope_is_not_rebound_to_current_model(fixture):
    _, records, _ = fixture
    old = "sha256:" + "b" * 64
    records["check.x"].update(spec_revision=old, scope=["rule.deleted"])
    records["evidence.x"].update(
        spec_revision=old,
        scope=["rule.deleted"],
        check_revision=digest(records["check.x"]),
    )
    value = report(fixture)
    assert aggregate(value["checks"], "model") == "UNKNOWN"
    assert implementation_result(value) == "UNKNOWN"
    assert not any(d["code"] == "RECORD_INVALID" for d in diagnostics(value))


def test_unknown_evidence_revision_cannot_resolve_or_prove_current_scope(fixture):
    _, records, _ = fixture
    records["evidence.x"].update(
        spec_revision=None,
        scope=["rule.x", "entity.deleted"],
        result="UNKNOWN",
        executed=False,
    )
    value = report(fixture)
    assert aggregate(value["checks"], "model") == "UNKNOWN"
    assert implementation_result(value) == "UNKNOWN"
    assert not any(d["code"] == "RECORD_INVALID" for d in diagnostics(value))


@pytest.mark.parametrize("independent_result", ["PASS", "FAIL"])
def test_invalid_evidence_does_not_hide_independent_valid_result(
    fixture, independent_result
):
    _, records, _ = fixture
    records["evidence.x"]["scope"].append("entity.deleted")
    records["evidence.y"]["result"] = independent_result
    value = report(fixture, ["entity.x", "entity.y"])
    assert aggregate(value["checks"], "model") == "FAIL"
    assert implementation_result(value, "rule.x") == "UNKNOWN"
    assert implementation_result(value, "rule.y") == independent_result
    expected = "FAIL" if independent_result == "FAIL" else "UNKNOWN"
    assert aggregate(value["checks"], "implementation") == expected
