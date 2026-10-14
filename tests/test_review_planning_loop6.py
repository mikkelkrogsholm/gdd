"""Parallel integration must name valid active plans for every required method."""

from copy import deepcopy
import json

import pytest

from gdd_validator import validate
from gdd_validator.loading import load_bundle
from gdd_validator.report import aggregate


@pytest.fixture
def case(tmp_path):
    model = {
        "format": "gdd/0.2",
        "project": {"id": "review", "title": "Review", "goal": "Plan checks"},
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
            "description": "Required behavior",
            "rule": {
                "category": "invariant",
                "statement": "Required behavior",
                "scope": "targeted",
                "blocking": True,
                "verification": (
                    ["synthetic", "synthetic.two"] if name == "x" else ["synthetic"]
                ),
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
    path.write_text(json.dumps(model))
    (tmp_path / "records.json").write_text(
        json.dumps({"format": "gdd-records/0.2", "project_id": "review", "records": {}})
    )
    revision = load_bundle(path).revision
    records = {
        f"check.{name}": {
            "kind": "check",
            "spec_revision": revision,
            "scope": [f"rule.{name}"],
            "method": "synthetic",
            "required_environment": [],
            "negative_controls": [],
        }
        for name in ("x", "y")
    }
    records["check.two"] = {**deepcopy(records["check.x"]), "method": "synthetic.two"}
    plan = {
        "format": "gdd-parallel-plan/0.2",
        "packages": [
            {
                "id": f"package.{name}",
                "scope": [f"entity.{name}"],
                "write_resources": [f"{name}.py"],
                "read_resources": [],
                "locked_contracts": [],
                "workspace": f"workspace.{name}",
                "base_revision": revision,
                "integration_plan": "Run required selected checks after integration",
            }
            for name in ("x", "y")
        ],
        "serialized": [],
        "integration": {"owner": "actor.integration", "checks": list(records)},
    }
    return path, records, plan


def run(case, *, check_model=True):
    path, records, plan = case
    (path.parent / "records.json").write_text(
        json.dumps(
            {"format": "gdd-records/0.2", "project_id": "review", "records": records}
        )
    )
    if check_model:
        value = validate(path, profiles=["readiness"], parallel_plan=plan)
    else:
        from gdd_validator.planning import validate_parallel

        bundle = load_bundle(path)
        validate_parallel(bundle, plan)
        value = {"checks": bundle.checks}
    return value, next(
        c["result"] for c in value["checks"] if c["check_id"] == "parallel.independence"
    )


@pytest.mark.parametrize(
    "checks,result",
    [
        (["check.x", "check.y"], "UNKNOWN"),
        (["check.two", "check.y"], "UNKNOWN"),
        (["check.x", "check.y", "check.two"], "PASS"),
    ],
)
def test_integration_covers_each_rule_method(case, checks, result):
    case[2]["integration"]["checks"] = checks
    value, observed = run(case)
    assert aggregate(value["checks"], "model") == "PASS"
    assert observed == result


@pytest.mark.parametrize(
    "variant",
    [
        "missing",
        "historical",
        "wrong_kind",
        "dangling_scope",
        "cycle",
        "dangling_supersedes",
        "collision",
    ],
)
def test_every_enumerated_check_must_be_valid_current(case, variant):
    _, records, plan = case
    if variant == "missing":
        plan["integration"]["checks"].append("check.missing")
    else:
        record = deepcopy(records["check.x"])
        if variant == "historical":
            record["spec_revision"] = "sha256:" + "a" * 64
        elif variant == "wrong_kind":
            record = {
                "kind": "binding",
                "spec_revision": record["spec_revision"],
                "targets": ["rule.x"],
                "implementation": "implementation.x",
                "role": "test",
                "resource": "test.py",
                "selector": {"kind": "whole-document"},
                "status": "planned",
            }
        elif variant == "dangling_scope":
            record["scope"].append("rule.missing")
        elif variant == "cycle":
            record["supersedes"] = "check.extra"
        elif variant == "dangling_supersedes":
            record["supersedes"] = "check.missing"
        ident = "rule.x" if variant == "collision" else "check.extra"
        records[ident] = record
        plan["integration"]["checks"].append(ident)
    _, result = run(case)
    assert result == "UNKNOWN"


def test_valid_extra_plan_is_declarative_and_needs_no_execution(case):
    _, records, plan = case
    records["check.extra"] = {
        **deepcopy(records["check.x"]),
        "method": "future.integration",
    }
    plan["integration"]["checks"].append("check.extra")
    value, result = run(case)
    assert aggregate(value["checks"], "model") == "PASS"
    assert result == "PASS"
    assert all(r["kind"] == "check" for r in records.values())


@pytest.mark.parametrize(
    "selected,result", [("check.x", "UNKNOWN"), ("check.next", "PASS")]
)
def test_selected_check_must_not_be_retired(case, selected, result):
    _, records, plan = case
    records["check.next"] = {**deepcopy(records["check.x"]), "supersedes": "check.x"}
    plan["integration"]["checks"] = [selected, "check.y", "check.two"]
    assert run(case)[1] == result


@pytest.mark.parametrize(
    "change",
    [
        {"spec_revision": "sha256:" + "a" * 64},
        {"method": "future.integration"},
        {"scope": ["rule.y"]},
    ],
)
def test_incompatible_supersession_keeps_selected_current_plan(case, change):
    _, records, _ = case
    records["check.next"] = {
        **deepcopy(records["check.x"]),
        "supersedes": "check.x",
        **change,
    }
    # Isolate retirement from the independent historical-revision diagnosis.
    assert run(case, check_model=False)[1] == "PASS"


def test_invalid_unselected_replacement_cannot_retire_plan(case):
    # Pure consumer check: unrelated invalid records remain separately diagnosed.
    from gdd_validator.planning import validate_parallel

    _, records, plan = case
    records["check.next"] = {
        **deepcopy(records["check.x"]),
        "supersedes": "check.x",
        "scope": ["rule.x", "rule.missing"],
    }
    path = case[0]
    (path.parent / "records.json").write_text(
        json.dumps(
            {"format": "gdd-records/0.2", "project_id": "review", "records": records}
        )
    )
    bundle = load_bundle(path)
    validate_parallel(bundle, plan)
    assert (
        next(
            c["result"]
            for c in bundle.checks
            if c["check_id"] == "parallel.independence"
        )
        == "PASS"
    )
