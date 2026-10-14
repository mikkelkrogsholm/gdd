"""Readiness regressions for global reviews, custom targets and ID collisions."""

from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path
import shutil

import pytest
import yaml

from gdd_validator.loading import load_bundle
from gdd_validator.report import aggregate, exit_code
from gdd_validator.validator import validate

ROOT = Path(__file__).resolve().parents[1]


def write(path, data):
    path.write_text(json.dumps(data), encoding="utf-8")


def minimal_model():
    return {
        "format": "gdd/0.2",
        "project": {"id": "review.loop3", "title": "Review", "goal": "Synthetic probe"},
        "nodes": {},
        "edges": [],
    }


def screen_fixture(directory, review_scope):
    raw = b"# Synthetic screen design\nUse readable spacing.\n"
    (directory / "design.md").write_bytes(raw)
    model = minimal_model()
    model["nodes"] = {
        "screen.x": {
            "kind": "screen",
            "description": "Synthetic screen",
            "interaction": {
                "states": ["normal"],
                "accessibility": ["Keyboard access"],
                "responsive": "Fluid layout",
                "behavior": "Show content",
            },
            "design_refs": [{"source": "source.design", "purpose": "contract"}],
        },
        "scenario.x": {
            "kind": "scenario",
            "description": "Display content",
            "scenario": {
                "category": "normal",
                "given": "Content",
                "when": "Open",
                "then": "Visible",
            },
        },
    }
    model["edges"] = [
        {
            "id": "edge.exercise",
            "from": "scenario.x",
            "relation": "exercises",
            "to": "screen.x",
        }
    ]
    model["sources"] = {
        "source.design": {
            "role": "design",
            "title": "Design",
            "uri": "design.md",
            "format": "text/markdown",
            "normative": True,
            "revision": {"sha256": sha256(raw).hexdigest()},
        }
    }
    model["record_files"] = ["records.json"]
    path = directory / "model.json"
    write(path, model)
    ledger = {
        "format": "gdd-records/0.2",
        "project_id": model["project"]["id"],
        "records": {},
    }
    write(directory / "records.json", ledger)
    ledger["records"]["decision.review"] = {
        "kind": "decision",
        "topic": "readiness_review",
        "scope": review_scope,
        "spec_revision": load_bundle(path).revision,
        "policy_revision": "policy.loop3",
        "actor": {"id": "actor.loop3", "kind": "human"},
        "outcome": "accepted",
        "rationale": "Synthetic current review",
    }
    write(directory / "records.json", ledger)
    return path, ledger


@pytest.mark.parametrize("review_scope", [[], ["screen.x"]])
@pytest.mark.parametrize("request_scope", [[], ["screen.x"]])
@pytest.mark.parametrize("trusted", [True, False])
def test_global_and_targeted_screen_reviews_use_the_same_trust_boundary(
    tmp_path, review_scope, request_scope, trusted
):
    path, _ = screen_fixture(tmp_path, review_scope)
    report = validate(
        path,
        profiles=["readiness"],
        scope=request_scope,
        host_context={"trusted_decisions": ["decision.review"] if trusted else []},
    )
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == (
        "PASS" if trusted else "UNKNOWN"
    )
    assert exit_code(report) == (0 if trusted else 2)


@pytest.mark.parametrize("variant", ["historical", "disjoint", "rejected"])
def test_global_review_support_does_not_expand_invalid_receipts(tmp_path, variant):
    path, ledger = screen_fixture(tmp_path, [])
    receipt = ledger["records"]["decision.review"]
    if variant == "historical":
        receipt["spec_revision"] = "sha256:" + "0" * 64
    elif variant == "disjoint":
        # The scenario is a known model ID, but does not cover the screen.
        receipt["scope"] = ["scenario.x"]
    else:
        receipt["outcome"] = "rejected"
    write(tmp_path / "records.json", ledger)
    report = validate(
        path,
        profiles=["readiness"],
        scope=["screen.x"],
        host_context={"trusted_decisions": ["decision.review"]},
    )
    assert aggregate(report["checks"], "readiness") == "UNKNOWN"


@pytest.mark.parametrize(
    "replacement_scope,expected", [([], "UNKNOWN"), (["screen.x"], "PASS")]
)
def test_global_review_supersession_preserves_coverage_guard(
    tmp_path, replacement_scope, expected
):
    path, ledger = screen_fixture(tmp_path, [])
    ledger["records"]["decision.replacement"] = {
        **deepcopy(ledger["records"]["decision.review"]),
        "scope": replacement_scope,
        "outcome": "rejected",
        "supersedes": "decision.review",
    }
    write(tmp_path / "records.json", ledger)
    report = validate(
        path,
        profiles=["readiness"],
        scope=["screen.x"],
        host_context={"trusted_decisions": ["decision.review", "decision.replacement"]},
    )
    assert aggregate(report["checks"], "readiness") == expected


def custom_fixture(directory, mixed=False):
    shutil.copytree(ROOT / "examples/extensions", directory / "extensions")
    model = yaml.safe_load((ROOT / "examples/extension.gdd.yaml").read_text())
    model["nodes"].pop("entity.room")
    model["edges"] = []
    if mixed:
        model["nodes"]["component.core"] = {
            "kind": "component",
            "description": "Disconnected ready core target",
            "interfaces": [],
        }
    path = directory / "model.json"
    write(path, model)
    return path


@pytest.mark.parametrize("mixed", [True, False])
@pytest.mark.parametrize("scope", [None, [], ["sensor.room"]])
def test_whole_project_readiness_includes_custom_implementation_targets(
    tmp_path, mixed, scope
):
    path = custom_fixture(tmp_path, mixed)
    report = validate(path, profiles=["readiness"], scope=scope)
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == "UNKNOWN"
    assert exit_code(report) == 2
    assert any(
        d["code"] == "EXTENSION_UNSUPPORTED" and "sensor.room" in d["related_ids"]
        for c in report["checks"]
        for d in c["diagnostics"]
    )


def test_disconnected_custom_target_does_not_expand_explicit_core_scope(tmp_path):
    path = custom_fixture(tmp_path, mixed=True)
    report = validate(path, profiles=["readiness"], scope=["component.core"])
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == "PASS"
    assert exit_code(report) == 0


def test_unsupported_custom_semantics_remain_unknown_for_whole_project(tmp_path):
    path = custom_fixture(tmp_path, mixed=True)
    report = validate(
        path,
        profiles=["readiness"],
        unsupported_extensions=frozenset({"example.sensors"}),
    )
    assert aggregate(report["checks"], "model") == "UNKNOWN"
    assert aggregate(report["checks"], "readiness") == "UNKNOWN"
    assert exit_code(report) == 2


def concert_fixture(directory):
    shutil.copytree(ROOT / "examples", directory / "examples")
    path = directory / "examples/concert.gdd.yaml"
    bundle = load_bundle(path)
    ledger_path = path.parent / "concert.records.yaml"
    ledger = yaml.safe_load(ledger_path.read_text())
    return path, bundle, ledger_path, ledger


@pytest.mark.parametrize("registry", ["node", "source", "edge", "transition"])
def test_record_collisions_with_model_ids_cannot_be_ignored_by_readiness(
    tmp_path, registry
):
    path, bundle, ledger_path, ledger = concert_fixture(tmp_path)
    if registry == "node":
        collision = "action.reserve"
    elif registry == "source":
        collision = "source.legacy"
    elif registry == "edge":
        collision = next(
            e["id"]
            for e in bundle.model["edges"]
            if e["from"] == "action.reserve" and e["relation"] == "owned_by"
        )
    else:
        collision = next(
            t["id"]
            for n in bundle.model["nodes"].values()
            for t in n.get("machine", {}).get("transitions", [])
        )
    ledger["records"][collision] = deepcopy(ledger["records"]["check.rule.capacity"])
    ledger_path.write_text(yaml.safe_dump(ledger, sort_keys=False))
    report = validate(path, profiles=["readiness"], scope=["action.reserve"])
    assert aggregate(report["checks"], "model") == "FAIL"
    assert aggregate(report["checks"], "readiness") == "FAIL"
    assert any(
        d["code"] == "ID_COLLISION" and collision in d["related_ids"]
        for c in report["checks"]
        for d in c["diagnostics"]
    )


@pytest.mark.parametrize("historical", [True, False])
def test_record_only_defects_do_not_become_model_context_defects(tmp_path, historical):
    path, _, ledger_path, ledger = concert_fixture(tmp_path)
    record = deepcopy(ledger["records"]["check.rule.capacity"])
    if historical:
        record["spec_revision"] = "sha256:" + "0" * 64
    else:
        record["scope"] = ["action.missing"]
    ledger["records"]["check.unrelated"] = record
    ledger_path.write_text(yaml.safe_dump(ledger, sort_keys=False))
    report = validate(path, profiles=["readiness"], scope=["action.reserve"])
    assert aggregate(report["checks"], "model") == ("UNKNOWN" if historical else "FAIL")
    assert aggregate(report["checks"], "readiness") == "PASS"


@pytest.mark.parametrize("support_nodes", [False, True])
def test_empty_or_support_only_model_remains_valid_without_new_targets(
    tmp_path, support_nodes
):
    model = minimal_model()
    if support_nodes:
        model["nodes"]["community.x"] = {
            "kind": "community",
            "description": "Navigation only",
        }
    path = tmp_path / "model.json"
    write(path, model)
    report = validate(path, profiles=["readiness"])
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == "PASS"
    assert exit_code(report) == 0
