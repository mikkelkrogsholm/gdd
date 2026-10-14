"""Regressions for LOOP 1 protocol and report fail-closed boundaries."""

from copy import deepcopy
from pathlib import Path

import pytest

from gdd_validator.loading import digest, load_bundle
from gdd_validator.readiness import context_closure
from gdd_validator.records import authorize, evaluate_implementation
from gdd_validator.report import aggregate, exit_code
from gdd_validator.validator import validate

ROOT = Path(__file__).resolve().parents[1]
MODEL = ROOT / "examples/concert.gdd.yaml"


def policy(bundle, requirement="standing", scope=None):
    return {
        "revision": "synthetic-policy",
        "actor": "actor.fixture",
        "activity": "implementation",
        "spec_revision": bundle.revision,
        "scope": [] if scope is None else scope,
        "requirement": requirement,
        "authorities": ["actor.fixture"],
        "basis": "Synthetic test mandate; no external authority",
    }


def approval(bundle, outcome, scope):
    return {
        "kind": "decision",
        "topic": "approval",
        "activity": "implementation",
        "spec_revision": bundle.revision,
        "policy_revision": "synthetic-policy",
        "scope": scope,
        "actor": {"id": "actor.fixture", "kind": "human"},
        "outcome": outcome,
        "rationale": "Synthetic protocol regression",
    }


@pytest.mark.parametrize("requirement", ["explicit", "standing", "none"])
@pytest.mark.parametrize(
    "scope", [["action.reserve"], ["action.reserve", "action.cancel"], []]
)
def test_partial_rejection_denies_overlapping_and_whole_project_scope(
    requirement, scope
):
    bundle = load_bundle(MODEL)
    bundle.records["decision.reject"] = approval(bundle, "rejected", ["action.reserve"])
    bundle.records["decision.accept"] = approval(bundle, "accepted", [])
    host = {
        "policy": policy(bundle, requirement),
        "trusted_decisions": ["decision.reject", "decision.accept"],
    }
    authorize(bundle, "implementation", scope, host)
    assert aggregate(bundle.checks, "authorization") == "DENIED"


def test_rejection_for_disjoint_scope_does_not_deny_authorized_slice():
    bundle = load_bundle(MODEL)
    bundle.records["decision.reject"] = approval(bundle, "rejected", ["action.reserve"])
    bundle.records["decision.accept"] = approval(bundle, "accepted", ["action.cancel"])
    host = {
        "policy": policy(bundle, "explicit"),
        "trusted_decisions": ["decision.reject", "decision.accept"],
    }
    authorize(bundle, "implementation", ["action.cancel"], host)
    assert aggregate(bundle.checks, "authorization") == "GRANTED"


@pytest.mark.parametrize("scope", [["action.reserve", "action.cancel"], []])
def test_partial_approval_does_not_authorize_larger_scope(scope):
    bundle = load_bundle(MODEL)
    bundle.records["decision.accept"] = approval(bundle, "accepted", ["action.reserve"])
    host = {
        "policy": policy(bundle, "explicit"),
        "trusted_decisions": ["decision.accept"],
    }
    authorize(bundle, "implementation", scope, host)
    assert aggregate(bundle.checks, "authorization") == "UNRESOLVED"


def test_covering_trusted_approval_can_supersede_partial_rejection():
    bundle = load_bundle(MODEL)
    bundle.records["decision.reject"] = approval(bundle, "rejected", ["action.reserve"])
    bundle.records["decision.accept"] = {
        **approval(bundle, "accepted", []),
        "supersedes": "decision.reject",
    }
    host = {
        "policy": policy(bundle, "explicit"),
        "trusted_decisions": ["decision.reject", "decision.accept"],
    }
    authorize(bundle, "implementation", ["action.reserve", "action.cancel"], host)
    assert aggregate(bundle.checks, "authorization") == "GRANTED"


def evidence(bundle, check_id, result):
    plan = bundle.records[check_id]
    return {
        "kind": "evidence",
        "check": check_id,
        "check_revision": digest(plan),
        "implementation": "implementation.fixture",
        "spec_revision": bundle.revision,
        "code_revision": "synthetic-current",
        "checker": {"name": "synthetic", "version": "1"},
        "scope": list(plan["scope"]),
        "result": result,
        "executed": True,
        "reason": "Synthetic applicability observation; no application execution",
        "environment": {name: "synthetic-env" for name in plan["required_environment"]},
        "evidence_uri": "fixture://synthetic-only",
    }


def evidence_host(bundle, evidence_ids):
    plan = bundle.records["check.rule.capacity"]
    bundle.records["binding.fixture"] = {
        "kind": "binding",
        "spec_revision": bundle.revision,
        "targets": ["rule.capacity"],
        "implementation": "implementation.fixture",
        "role": "test",
        "resource": "synthetic.py",
        "selector": {"kind": "whole-document"},
        "status": "checked",
        "revision": "synthetic-resource",
        "code_revision": "synthetic-current",
    }
    return {
        "implementation": "implementation.fixture",
        "code_revision": "synthetic-current",
        "trusted_evidence": evidence_ids,
        "supported_methods": [plan["method"]],
        "environment": {name: "synthetic-env" for name in plan["required_environment"]},
        "negative_controls": list(plan["negative_controls"]),
        "verified_bindings": ["binding.fixture"],
    }


def capacity_result(bundle, host):
    evaluate_implementation(bundle, ["rule.capacity"], host)
    return next(
        c["result"]
        for c in reversed(bundle.checks)
        if c["dimension"] == "implementation" and c["scope"] == ["rule.capacity"]
    )


@pytest.mark.parametrize("incompatibility", ["spec_revision", "method", "scope"])
def test_incompatible_check_supersession_preserves_selected_failed_plan(
    incompatibility,
):
    bundle = load_bundle(MODEL)
    original = bundle.records["check.rule.capacity"]
    if incompatibility == "scope":
        original["scope"].append("rule.authorization")
    replacement = {**deepcopy(original), "supersedes": "check.rule.capacity"}
    if incompatibility == "spec_revision":
        replacement["spec_revision"] = "sha256:" + "0" * 64
    elif incompatibility == "method":
        replacement["method"] = "unrelated_method"
    else:
        replacement["scope"] = ["rule.capacity"]
    bundle.records["check.replacement"] = replacement
    bundle.records["evidence.failure"] = evidence(bundle, "check.rule.capacity", "FAIL")
    host = evidence_host(bundle, ["evidence.failure"])
    host["check_selection"] = {
        "rule.capacity": {original["method"]: "check.rule.capacity"}
    }
    assert capacity_result(bundle, host) == "FAIL"


def test_historical_unrelated_check_cannot_turn_ambiguous_plans_into_pass():
    bundle = load_bundle(MODEL)
    bundle.records["check.alternative"] = deepcopy(
        bundle.records["check.rule.capacity"]
    )
    bundle.records["evidence.failure"] = evidence(bundle, "check.rule.capacity", "FAIL")
    bundle.records["evidence.pass"] = evidence(bundle, "check.alternative", "PASS")
    host = evidence_host(bundle, ["evidence.failure", "evidence.pass"])
    assert capacity_result(bundle, host) == "UNKNOWN"
    bundle.records["check.historical"] = {
        "kind": "check",
        "spec_revision": "sha256:" + "0" * 64,
        "scope": ["rule.authorization"],
        "method": "authorization_tests",
        "required_environment": [],
        "negative_controls": [],
        "supersedes": "check.rule.capacity",
    }
    assert capacity_result(bundle, host) == "UNKNOWN"


def test_compatible_check_supersession_selects_covering_current_plan():
    bundle = load_bundle(MODEL)
    bundle.records["check.replacement"] = {
        **deepcopy(bundle.records["check.rule.capacity"]),
        "scope": ["rule.capacity", "rule.authorization"],
        "supersedes": "check.rule.capacity",
    }
    bundle.records["evidence.failure"] = evidence(bundle, "check.rule.capacity", "FAIL")
    bundle.records["evidence.pass"] = evidence(bundle, "check.replacement", "PASS")
    host = evidence_host(bundle, ["evidence.failure", "evidence.pass"])
    assert capacity_result(bundle, host) == "PASS"


def context_package(bundle, scope):
    closure = context_closure(bundle, scope)
    # Supplying complete registries is valid and isolates scope propagation here.
    return {
        "format": "gdd-context/0.2",
        "spec_revision": bundle.revision,
        "scope": scope,
        "nodes": sorted(closure),
        "edges": [e["id"] for e in bundle.model["edges"]],
        "sources": list(bundle.model.get("sources", {})),
        "questions": list(bundle.model.get("questions", {})),
        "extensions": list(bundle.model.get("extensions", {})),
    }


def parallel_plan(bundle):
    return {
        "format": "gdd-parallel-plan/0.2",
        "packages": [
            {
                "id": "package." + str(n),
                "scope": [node],
                "write_resources": [str(n) + ".py"],
                "read_resources": [],
                "locked_contracts": list(bundle.model["nodes"]),
                "workspace": "workspace." + str(n),
                "base_revision": bundle.revision,
                "integration_plan": "Synthetic integration plan",
            }
            for n, node in enumerate(("action.reserve", "action.cancel"))
        ],
        "serialized": [],
        "integration": {
            "owner": "actor.fixture",
            "checks": [i for i, r in bundle.records.items() if r["kind"] == "check"],
        },
    }


@pytest.mark.parametrize("explicit_scope", [None, [], ["action.reserve"]])
def test_package_effective_scope_drives_readiness_authority_and_report(explicit_scope):
    bundle = load_bundle(MODEL)
    report = validate(
        MODEL,
        profiles=["readiness"],
        scope=explicit_scope,
        package=context_package(bundle, ["action.reserve"]),
        activity="implementation",
        host_context={"policy": policy(bundle, scope=["action.cancel"])},
    )
    assert report["scope"] == ["action.reserve"]
    assert aggregate(report["checks"], "readiness") == "PASS"
    assert aggregate(report["checks"], "authorization") == "DENIED"
    authorization = next(
        c for c in report["checks"] if c["dimension"] == "authorization"
    )
    assert authorization["scope"] == ["action.reserve"]


def test_package_conflicting_explicit_scope_is_rejected():
    bundle = load_bundle(MODEL)
    with pytest.raises(ValueError, match="conflicts"):
        validate(
            MODEL,
            scope=["action.cancel"],
            package=context_package(bundle, ["action.reserve"]),
        )


@pytest.mark.parametrize(
    "explicit_scope", [None, [], ["action.cancel", "action.reserve"]]
)
def test_parallel_effective_union_drives_authority_and_report(explicit_scope):
    bundle = load_bundle(MODEL)
    report = validate(
        MODEL,
        profiles=["readiness"],
        scope=explicit_scope,
        parallel_plan=parallel_plan(bundle),
        activity="implementation",
        host_context={"policy": policy(bundle, scope=["action.reserve"])},
    )
    assert report["scope"] == ["action.reserve", "action.cancel"]
    assert aggregate(report["checks"], "authorization") == "DENIED"
    authorization = next(
        c for c in report["checks"] if c["dimension"] == "authorization"
    )
    assert authorization["scope"] == report["scope"]


def test_parallel_conflicting_explicit_scope_is_rejected():
    bundle = load_bundle(MODEL)
    with pytest.raises(ValueError, match="conflicts"):
        validate(MODEL, scope=["action.reserve"], parallel_plan=parallel_plan(bundle))


def test_both_scope_envelopes_are_rejected():
    bundle = load_bundle(MODEL)
    with pytest.raises(ValueError, match="cannot be supplied together"):
        validate(
            MODEL,
            package=context_package(bundle, ["action.reserve"]),
            parallel_plan=parallel_plan(bundle),
        )


@pytest.mark.parametrize(
    "value", ["action.reserve", 1, {}, [1], [["action.reserve"]], [None]]
)
def test_malformed_scope_type_raises_controlled_value_error(value):
    with pytest.raises(ValueError, match="Scope must be a list of strings"):
        validate(MODEL, scope=value)


@pytest.mark.parametrize("value", ["model", 1, {}, [1], [["model"]], [None]])
def test_malformed_profile_type_raises_controlled_value_error(value):
    with pytest.raises(ValueError, match="Profiles must be a list of strings"):
        validate(MODEL, profiles=value)


@pytest.mark.parametrize("activity", [[], {}])
def test_malformed_activity_type_raises_controlled_value_error(activity):
    with pytest.raises(ValueError, match="Unknown activity"):
        validate(MODEL, activity=activity)


def test_valid_scope_and_profile_deduplication_is_preserved():
    report = validate(
        MODEL,
        profiles=["model", "model"],
        scope=["action.reserve", "action.reserve"],
    )
    assert report["profiles"] == ["model"]
    assert report["scope"] == ["action.reserve"]


@pytest.mark.parametrize(
    "checks",
    [
        [{"dimension": "model", "result": "BROKEN"}],
        [{"dimension": "model", "result": "GRANTED"}],
        [{"dimension": "authorization", "result": "PASS"}],
        [{"dimension": "alien", "result": "PASS"}],
        [{"dimension": [], "result": "PASS"}],
        [{"dimension": "model", "result": []}],
        [{"dimension": "model"}],
        [None],
        {},
        None,
    ],
)
def test_report_helpers_reject_malformed_or_dimension_invalid_results(checks):
    with pytest.raises(ValueError):
        aggregate(checks, "model")
    with pytest.raises(ValueError):
        exit_code({"checks": checks})


@pytest.mark.parametrize("report", [None, [], {}])
def test_exit_code_requires_report_checks(report):
    with pytest.raises(ValueError):
        exit_code(report)


@pytest.mark.parametrize("dimension", [None, [], "alien"])
def test_aggregate_rejects_unknown_requested_dimension(dimension):
    with pytest.raises(ValueError):
        aggregate([], dimension)


def test_empty_checks_are_unassessed():
    assert aggregate([], "model") == "UNKNOWN"
    assert aggregate([], "authorization") == "UNRESOLVED"
    assert exit_code({"checks": []}) == 2


@pytest.mark.parametrize(
    "dimension,result,expected",
    [
        ("model", "PASS", 0),
        ("readiness", "PASS", 0),
        ("authorization", "GRANTED", 0),
        ("implementation", "UNKNOWN", 2),
        ("authorization", "UNRESOLVED", 2),
        ("integration", "FAIL", 1),
        ("authorization", "DENIED", 1),
    ],
)
def test_report_helpers_keep_known_result_semantics(dimension, result, expected):
    checks = [{"dimension": dimension, "result": result}]
    assert aggregate(checks, dimension) == result
    assert exit_code({"checks": checks}) == expected


def test_report_failure_dominates_unknown_result():
    checks = [
        {"dimension": "model", "result": "UNKNOWN"},
        {"dimension": "authorization", "result": "DENIED"},
    ]
    assert exit_code({"checks": checks}) == 1


def test_declared_record_loop_preserves_loader_diagnostic(monkeypatch, tmp_path):
    # Isolate orchestration after a loader has diagnosed a looping record_file.
    import importlib

    validator_module = importlib.import_module("gdd_validator.validator")
    bundle = load_bundle(MODEL)
    bundle.path = tmp_path / "model.yaml"
    loop = tmp_path / "loop.yaml"
    loop.symlink_to(loop.name)
    bundle.model["record_files"] = [loop.name]
    bundle.issue(
        "ASSET_PATH_INVALID", "Synthetic existing loader diagnosis.", loop.name
    )
    monkeypatch.setattr(validator_module, "load_bundle", lambda *args: bundle)
    report = validate(bundle.path)
    assert exit_code(report) == 1
    assert any(
        d["code"] == "ASSET_PATH_INVALID"
        and d["message"] == "Synthetic existing loader diagnosis."
        for c in report["checks"]
        for d in c["diagnostics"]
    )


def test_secondary_record_loop_returns_controlled_diagnostic(tmp_path):
    loop = tmp_path / "loop.yaml"
    loop.symlink_to(loop.name)
    report = validate(loop, model_path=MODEL)
    assert exit_code(report) == 1
    assert any(
        d["code"] == "ASSET_PATH_INVALID"
        and "Secondary record input cannot be resolved" in d["message"]
        for c in report["checks"]
        for d in c["diagnostics"]
    )
