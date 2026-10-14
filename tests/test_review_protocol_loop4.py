"""Used-record integrity guards positive claims and retirement, not the whole log."""

from copy import deepcopy
import json

import pytest

from gdd_validator.loading import digest, load_bundle
from gdd_validator.report import aggregate
from gdd_validator.validator import validate


@pytest.fixture
def case(tmp_path):
    model = {
        "format": "gdd/0.2",
        "project": {"id": "loop4", "title": "Loop4", "goal": "Synthetic record checks"},
        "nodes": {},
        "edges": [],
        "record_files": ["records.json"],
        "sources": {
            "source.input": {
                "role": "input",
                "title": "Provenance",
                "uri": "missing.md",
                "format": "text/markdown",
                "normative": False,
            }
        },
    }
    for name in ("x", "y"):
        model["nodes"]["entity." + name] = {
            "kind": "entity",
            "description": "Value",
            "fields": {},
        }
        model["nodes"]["rule." + name] = {
            "kind": "rule",
            "description": "Required check",
            "rule": {
                "category": "invariant",
                "statement": "Required behavior",
                "scope": "targeted",
                "blocking": True,
                "verification": ["probe"],
            },
        }
        model["edges"].append(
            {
                "id": "edge." + name,
                "from": "rule." + name,
                "relation": "constrains",
                "to": "entity." + name,
            }
        )
    model["nodes"].update(
        {
            "action.trigger": {"kind": "action", "description": "Trigger"},
            "machine.x": {
                "kind": "machine",
                "description": "Lifecycle",
                "machine": {
                    "states": ["a", "b"],
                    "initial": "a",
                    "terminal": ["b"],
                    "transitions": [
                        {
                            "id": "transition.x",
                            "from": "a",
                            "to": "b",
                            "trigger": "action.trigger",
                            "guard": "true",
                            "effects": "change",
                        }
                    ],
                },
            },
        }
    )
    model["edges"].append(
        {
            "id": "edge.state",
            "from": "machine.x",
            "relation": "state_of",
            "to": "entity.x",
        }
    )
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model))
    records = {}
    write(path, records)
    revision = load_bundle(path).revision
    assert revision is not None
    for name in ("x", "y"):
        plan = {
            "kind": "check",
            "spec_revision": revision,
            "scope": ["rule." + name],
            "method": "probe",
            "required_environment": [],
            "negative_controls": [],
        }
        records["check." + name] = plan
        records["evidence." + name] = {
            "kind": "evidence",
            "check": "check." + name,
            "check_revision": digest(plan),
            "implementation": "implementation.x",
            "spec_revision": revision,
            "code_revision": "code.x",
            "checker": {"name": "synthetic", "version": "1"},
            "scope": ["rule." + name],
            "result": "PASS",
            "executed": True,
            "reason": "Synthetic observation; no app execution",
            "environment": {},
            "evidence_uri": "fixture://loop4",
        }
        records["binding." + name] = {
            "kind": "binding",
            "spec_revision": revision,
            "targets": ["rule." + name],
            "implementation": "implementation.x",
            "role": "test",
            "resource": "test.py",
            "selector": {"kind": "whole-document"},
            "status": "checked",
            "revision": "resource.x",
            "code_revision": "code.x",
        }
    records["decision.x"] = {
        "kind": "decision",
        "topic": "approval",
        "activity": "implementation",
        "spec_revision": revision,
        "policy_revision": "policy.x",
        "scope": ["entity.x"],
        "actor": {"id": "actor.x", "kind": "human"},
        "outcome": "accepted",
        "rationale": "Synthetic authority receipt",
    }
    host = {
        "implementation": "implementation.x",
        "code_revision": "code.x",
        "supported_methods": ["probe"],
        "trusted_evidence": ["evidence.x", "evidence.y"],
        "verified_bindings": ["binding.x", "binding.y"],
        "environment": {},
        "negative_controls": [],
        "trusted_decisions": ["decision.x"],
        "policy": {
            "revision": "policy.x",
            "actor": "actor.x",
            "activity": "implementation",
            "spec_revision": revision,
            "scope": [],
            "requirement": "explicit",
            "authorities": ["actor.x"],
            "basis": "Synthetic host mandate",
        },
    }
    return path, records, host


def write(path, records):
    (path.parent / "records.json").write_text(
        json.dumps(
            {
                "format": "gdd-records/0.2",
                "project_id": "loop4",
                "records": records,
            }
        )
    )


def run(case, *, activity=None, scope=None):
    path, records, host = case
    write(path, records)
    return validate(
        path,
        profiles=["records"],
        scope=scope or ["entity.x"],
        activity=activity,
        host_context=host,
    )


def rule_result(report, name="x"):
    return next(
        c["result"]
        for c in report["checks"]
        if c["dimension"] == "implementation" and c["scope"] == ["rule." + name]
    )


@pytest.mark.parametrize("kind", ["check", "evidence", "binding"])
@pytest.mark.parametrize(
    "model_id", ["entity.x", "source.input", "edge.x", "transition.x"]
)
def test_used_record_id_collision_cannot_support_implementation_pass(
    case, kind, model_id
):
    _, records, host = case
    records[model_id] = records.pop(kind + ".x")
    if kind == "check":
        records["evidence.x"]["check"] = model_id
    elif kind == "evidence":
        host["trusted_evidence"] = [model_id, "evidence.y"]
    else:
        host["verified_bindings"] = [model_id, "binding.y"]
    report = run(case)
    assert aggregate(report["checks"], "model") == "FAIL"
    assert rule_result(report) == "UNKNOWN"


@pytest.mark.parametrize(
    "model_id", ["entity.x", "source.input", "edge.x", "transition.x"]
)
def test_used_approval_id_collision_cannot_support_grant(case, model_id):
    _, records, host = case
    records[model_id] = records.pop("decision.x")
    host["trusted_decisions"] = [model_id]
    report = run(case, activity="implementation")
    assert aggregate(report["checks"], "model") == "FAIL"
    assert aggregate(report["checks"], "authorization") == "UNRESOLVED"


@pytest.mark.parametrize(
    "kind,field", [("binding", "targets"), ("evidence", "scope"), ("check", "scope")]
)
def test_unknown_current_target_cannot_support_positive_evidence(case, kind, field):
    _, records, _ = case
    records[kind + ".x"][field].append("entity.deleted")
    if kind == "check":
        records["evidence.x"].update(
            scope=list(records["check.x"]["scope"]),
            check_revision=digest(records["check.x"]),
        )
    report = run(case)
    assert aggregate(report["checks"], "model") == "FAIL"
    assert rule_result(report) == "UNKNOWN"


def test_unknown_current_approval_scope_cannot_support_grant(case):
    _, records, _ = case
    records["decision.x"]["scope"].append("entity.deleted")
    assert (
        aggregate(run(case, activity="implementation")["checks"], "authorization")
        == "UNRESOLVED"
    )


@pytest.mark.parametrize("reference", ["check.deleted", "evidence.x", "entity.x"])
def test_invalid_approval_check_reference_cannot_support_grant(case, reference):
    _, records, _ = case
    records["decision.x"]["checks"] = [reference]
    assert (
        aggregate(run(case, activity="implementation")["checks"], "authorization")
        == "UNRESOLVED"
    )


@pytest.mark.parametrize("variant", ["self", "pair"])
@pytest.mark.parametrize("requirement", ["standing", "none"])
def test_cyclic_supersession_cannot_erase_current_rejection(case, variant, requirement):
    _, records, host = case
    host["policy"]["requirement"] = requirement
    records["decision.x"].update(
        outcome="rejected",
        supersedes="decision.x" if variant == "self" else "decision.other",
    )
    if variant == "pair":
        records["decision.other"] = {
            **deepcopy(records["decision.x"]),
            "outcome": "accepted",
            "supersedes": "decision.x",
        }
        host["trusted_decisions"].append("decision.other")
    report = run(case, activity="implementation")
    assert aggregate(report["checks"], "model") == "FAIL"
    assert aggregate(report["checks"], "authorization") == "DENIED"


@pytest.mark.parametrize("reference", ["decision.x", "decision.deleted", "binding.x"])
def test_malformed_approval_supersession_cannot_support_explicit_grant(case, reference):
    _, records, _ = case
    records["decision.x"]["supersedes"] = reference
    assert (
        aggregate(run(case, activity="implementation")["checks"], "authorization")
        == "UNRESOLVED"
    )


@pytest.mark.parametrize("defect", ["scope", "checkref", "collision"])
def test_invalid_accepted_successor_cannot_retire_valid_rejection(case, defect):
    _, records, host = case
    records["decision.x"]["outcome"] = "rejected"
    replacement = {
        **deepcopy(records["decision.x"]),
        "outcome": "accepted",
        "supersedes": "decision.x",
    }
    ident = "entity.x" if defect == "collision" else "decision.other"
    if defect == "scope":
        replacement["scope"].append("entity.deleted")
    elif defect == "checkref":
        replacement["checks"] = ["check.deleted"]
    records[ident] = replacement
    host["trusted_decisions"].append(ident)
    host["policy"]["requirement"] = "standing"
    assert (
        aggregate(run(case, activity="implementation")["checks"], "authorization")
        == "DENIED"
    )


@pytest.mark.parametrize("kind", ["check", "evidence", "binding"])
@pytest.mark.parametrize("reference", ["self", "missing", "wrongkind"])
def test_malformed_used_supersession_cannot_support_positive_proof(
    case, kind, reference
):
    _, records, _ = case
    ident = kind + ".x"
    records[ident]["supersedes"] = (
        ident
        if reference == "self"
        else "record.missing" if reference == "missing" else "decision.x"
    )
    if kind == "check":
        records["evidence.x"]["check_revision"] = digest(records[ident])
    assert rule_result(run(case)) == "UNKNOWN"


@pytest.mark.parametrize("kind", ["check", "evidence"])
def test_invalid_successor_cannot_retire_selected_valid_failure(case, kind):
    _, records, host = case
    records["evidence.x"]["result"] = "FAIL"
    replacement = {**deepcopy(records[kind + ".x"]), "supersedes": kind + ".x"}
    replacement["scope"].append("entity.deleted")
    if kind == "evidence":
        replacement["result"] = "PASS"
        host["trusted_evidence"].append("evidence.other")
    else:
        host["check_selection"] = {"rule.x": {"probe": "check.x"}}
    records[kind + ".other"] = replacement
    host["evidence_selection"] = {"check.x": "evidence.x"}
    assert rule_result(run(case)) == "FAIL"


def test_invalid_binding_successor_preserves_valid_current_mapping(case):
    _, records, host = case
    records["binding.other"] = {
        **deepcopy(records["binding.x"]),
        "supersedes": "binding.x",
        "role": "design",
        "targets": ["rule.x", "entity.deleted"],
    }
    host["verified_bindings"].append("binding.other")
    assert rule_result(run(case)) == "PASS"


@pytest.mark.parametrize("defect", ["collision", "scope", "cycle"])
def test_valid_independent_failure_remains_fail(case, defect):
    _, records, host = case
    records["evidence.y"]["result"] = "FAIL"
    if defect == "collision":
        records["entity.x"] = records.pop("evidence.x")
        host["trusted_evidence"] = ["entity.x", "evidence.y"]
    elif defect == "scope":
        records["evidence.x"]["scope"].append("entity.deleted")
    else:
        records["evidence.x"]["supersedes"] = "evidence.x"
    report = run(case, scope=["entity.x", "entity.y"])
    assert rule_result(report, "x") == "UNKNOWN"
    assert rule_result(report, "y") == "FAIL"
    assert aggregate(report["checks"], "implementation") == "FAIL"


@pytest.mark.parametrize("requirement", ["explicit", "standing"])
def test_unrelated_invalid_decision_cycle_does_not_block_valid_authority(
    case, requirement
):
    _, records, host = case
    host["policy"]["requirement"] = requirement
    records["decision.unrelated"] = {
        **deepcopy(records["decision.x"]),
        "topic": "readiness_review",
        "scope": ["entity.y"],
        "supersedes": "decision.unrelated",
    }
    host["trusted_decisions"].append("decision.unrelated")
    report = run(case, activity="implementation")
    assert aggregate(report["checks"], "model") == "FAIL"
    assert aggregate(report["checks"], "authorization") == "GRANTED"


def test_invalid_unused_proof_does_not_block_explicit_valid_selections(case):
    _, records, host = case
    records["evidence.other"] = {
        **deepcopy(records["evidence.x"]),
        "supersedes": "evidence.other",
    }
    host["trusted_evidence"].append("evidence.other")
    host["evidence_selection"] = {"check.x": "evidence.x"}
    assert rule_result(run(case)) == "PASS"


def test_historical_targets_are_not_rebound_and_current_proof_is_independent(case):
    _, records, _ = case
    records["binding.historical"] = {
        **deepcopy(records["binding.x"]),
        "spec_revision": "sha256:" + "a" * 64,
        "targets": ["entity.deleted"],
    }
    report = run(case)
    assert aggregate(report["checks"], "model") == "UNKNOWN"
    assert rule_result(report) == "PASS"
    assert not any(
        d["code"] == "RECORD_INVALID"
        for c in report["checks"]
        for d in c["diagnostics"]
    )


def test_current_positive_control_and_known_extra_binding_targets(case):
    _, records, _ = case
    records["binding.x"]["targets"].extend(["source.input", "edge.x", "transition.x"])
    report = run(case)
    assert aggregate(report["checks"], "model") == "PASS"
    assert rule_result(report) == "PASS"
    assert (
        aggregate(run(case, activity="implementation")["checks"], "authorization")
        == "GRANTED"
    )


def test_wrong_check_digest_successor_cannot_retire_selected_valid_failure(case):
    _, records, host = case
    records["evidence.x"]["result"] = "FAIL"
    records["evidence.other"] = {
        **deepcopy(records["evidence.x"]),
        "result": "PASS",
        "supersedes": "evidence.x",
        "check_revision": "sha256:" + "a" * 64,
    }
    host["trusted_evidence"].append("evidence.other")
    host["evidence_selection"] = {"check.x": "evidence.x"}
    assert rule_result(run(case)) == "FAIL"


@pytest.mark.parametrize("kind", ["check", "evidence", "binding"])
def test_two_member_used_record_cycle_cannot_support_positive_proof(case, kind):
    _, records, host = case
    records[kind + ".x"]["supersedes"] = kind + ".other"
    records[kind + ".other"] = {
        **deepcopy(records[kind + ".x"]),
        "supersedes": kind + ".x",
    }
    if kind == "check":
        records["evidence.x"]["check_revision"] = digest(records["check.x"])
        host["check_selection"] = {"rule.x": {"probe": "check.x"}}
    elif kind == "evidence":
        host["trusted_evidence"].append("evidence.other")
        host["evidence_selection"] = {"check.x": "evidence.x"}
    else:
        host["verified_bindings"].append("binding.other")
    assert rule_result(run(case)) == "UNKNOWN"
