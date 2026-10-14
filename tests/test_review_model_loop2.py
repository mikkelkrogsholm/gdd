"""Regressions from the second ordinary/adversarial model review."""

from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path

import pytest

from gdd_validator import validate
from gdd_validator.loading import Bundle, load_bundle, schema_checks
from gdd_validator.model import validate_model
from gdd_validator.readiness import validate_readiness
from gdd_validator.report import aggregate


def minimal_model():
    return {
        "format": "gdd/0.2",
        "project": {"id": "review", "title": "Review", "goal": "Check contracts"},
        "nodes": {"entity.x": {"kind": "entity", "description": "Value", "fields": {}}},
        "edges": [],
    }


def decision(revision, scope, topic="readiness_review", **extra):
    return {
        "kind": "decision",
        "topic": topic,
        "scope": scope,
        "spec_revision": revision,
        "policy_revision": "policy.1",
        "actor": {"id": "actor.human", "kind": "human"},
        "outcome": "accepted",
        "rationale": "Reviewed the selected contract",
        **extra,
    }


def write_screen(directory, *, purpose="contract", normative=True):
    model = minimal_model()
    raw = b"# Screen design\nShow content in a readable layout.\n"
    (directory / "design.md").write_bytes(raw)
    model["nodes"].update(
        {
            "screen.x": {
                "kind": "screen",
                "description": "Content screen",
                "interaction": {
                    "states": ["normal"],
                    "accessibility": ["Keyboard access"],
                    "responsive": "Adapt to screen width",
                    "behavior": "Show content",
                },
                "design_refs": [{"source": "source.design", "purpose": purpose}],
            },
            "scenario.x": {
                "kind": "scenario",
                "description": "Normal display",
                "scenario": {
                    "category": "normal",
                    "given": "Content",
                    "when": "Open",
                    "then": "Visible",
                },
            },
        }
    )
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
            "title": "Screen design",
            "uri": "design.md" if normative else "missing.md",
            "format": "text/markdown",
            "normative": normative,
            **({"revision": {"sha256": sha256(raw).hexdigest()}} if normative else {}),
        }
    }
    model["record_files"] = ["records.json"]
    path = directory / "model.json"
    path.write_text(json.dumps(model), encoding="utf-8")
    write_records(directory, {})
    revision = load_bundle(path).revision
    write_records(directory, {"decision.review": decision(revision, ["screen.x"])})
    return path


def write_records(directory, records):
    (directory / "records.json").write_text(
        json.dumps(
            {
                "format": "gdd-records/0.2",
                "project_id": "review",
                "records": records,
            }
        ),
        encoding="utf-8",
    )


def codes(report):
    return {d["code"] for c in report["checks"] for d in c["diagnostics"]}


@pytest.mark.parametrize("format", ["gdd/0.2", "gdd-fragment/0.2"])
def test_design_ref_purpose_is_contract_in_both_schemas(format):
    model = minimal_model()
    model["nodes"] = {
        "screen.x": {
            "kind": "screen",
            "description": "Screen",
            "design_refs": [{"source": "source.design", "purpose": "provenance"}],
        }
    }
    model["format"] = format
    if format == "gdd-fragment/0.2":
        model.pop("project")
    assert aggregate(schema_checks(model, "<memory>"), "model") == "FAIL"


def test_provenance_design_cannot_bypass_resolution(tmp_path):
    path = write_screen(tmp_path, purpose="provenance", normative=False)
    report = validate(
        path,
        profiles=["readiness"],
        scope=["screen.x"],
        host_context={"trusted_decisions": ["decision.review"]},
    )
    assert aggregate(report["checks"], "model") == "FAIL"
    assert aggregate(report["checks"], "readiness") != "PASS"
    # Public low-level functions also enforce the semantic contract.
    model = json.loads(path.read_text())
    bundle = Bundle(path, model=model, revision="sha256:" + "a" * 64)
    bundle.records = {"decision.review": decision(bundle.revision, ["screen.x"])}
    validate_model(bundle)
    assert aggregate(bundle.checks, "model") == "FAIL"
    validate_readiness(bundle, ["screen.x"], {"trusted_decisions": ["decision.review"]})
    assert aggregate(bundle.checks, "readiness") == "FAIL"


def test_resolved_pinned_design_is_ready(tmp_path):
    path = write_screen(tmp_path)
    report = validate(
        path,
        profiles=["readiness"],
        scope=["screen.x"],
        host_context={"trusted_decisions": ["decision.review"]},
    )
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == "PASS"


@pytest.mark.parametrize("mode", ["screen", "question"])
@pytest.mark.parametrize(
    "mutation",
    [
        {"topic": "source_promotion"},
        {"scope": ["entity.x"]},
        {"spec_revision": "sha256:" + "a" * 64},
        {"policy_revision": "policy.other"},
        {"activity": "model_changes"},
        {"implementation": "implementation.other"},
        {"code_revision": "other"},
        {"outcome": "proposed"},
    ],
)
def test_incompatible_decision_does_not_retire_current_receipt(
    tmp_path, mode, mutation
):
    if mode == "screen":
        path = write_screen(tmp_path)
        scope = ["screen.x"]
        topic = "readiness_review"
    else:
        model = minimal_model()
        model["record_files"] = ["records.json"]
        model["questions"] = {
            "question.x": {
                "statement": "Choose the value contract",
                "targets": ["entity.x"],
                "blocks": ["implementation"],
                "status": "resolved",
                "resolution": {"decision": "decision.review"},
            }
        }
        path = tmp_path / "model.json"
        path.write_text(json.dumps(model), encoding="utf-8")
        write_records(tmp_path, {})
        scope = ["entity.x"]
        topic = "model_change"
    revision = load_bundle(path).revision
    original = decision(revision, scope, topic)
    replacement = {**deepcopy(original), "supersedes": "decision.review", **mutation}
    # For the question case use a different, known current target.
    if mode == "question" and "scope" in mutation:
        replacement["scope"] = ["question.x"]
    write_records(
        tmp_path, {"decision.review": original, "decision.other": replacement}
    )
    report = validate(
        path,
        profiles=["readiness"],
        scope=scope,
        host_context={"trusted_decisions": ["decision.review", "decision.other"]},
    )
    assert aggregate(report["checks"], "readiness") == "PASS"


@pytest.mark.parametrize(
    "outcome,result", [("accepted", "PASS"), ("rejected", "UNKNOWN")]
)
def test_compatible_screen_decision_supersedes(tmp_path, outcome, result):
    path = write_screen(tmp_path)
    revision = load_bundle(path).revision
    original = decision(revision, ["screen.x"])
    replacement = {
        **deepcopy(original),
        "outcome": outcome,
        "supersedes": "decision.review",
    }
    write_records(
        tmp_path, {"decision.review": original, "decision.other": replacement}
    )
    report = validate(
        path,
        profiles=["readiness"],
        scope=["screen.x"],
        host_context={"trusted_decisions": ["decision.review", "decision.other"]},
    )
    assert aggregate(report["checks"], "readiness") == result


@pytest.mark.parametrize(
    "start,end,result",
    [
        ("1" * 5000, "2" * 5000, "PASS"),
        ("2" * 5000, "1" * 5000, "FAIL"),
        ("9" * 5000, "1" * 5001, "PASS"),
    ],
)
def test_long_decimal_line_selector_has_controlled_result(tmp_path, start, end, result):
    model = minimal_model()
    model["sources"] = {
        "source.x": {
            "role": "input",
            "title": "Source",
            "uri": "missing.txt",
            "format": "text/plain",
            "normative": False,
            "revision": {"sha256": "a" * 64},
        }
    }
    model["nodes"]["entity.x"]["source_refs"] = [
        {
            "source": "source.x",
            "purpose": "provenance",
            "selector": {"kind": "line-range", "value": "L" + start + "-L" + end},
        }
    ]
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model), encoding="utf-8")
    report = validate(path)
    assert aggregate(report["checks"], "model") == result


def write_extension(directory, data_schema, data):
    extension = {
        "format": "gdd-extension/0.2",
        "namespace": "example.test",
        "version": "1.0.0",
        "node_kinds": {
            "example.test.widget": {"description": "Widget", "data_schema": data_schema}
        },
        "relations": {},
    }
    raw = json.dumps(extension).encode("utf-8")
    (directory / "extension.json").write_bytes(raw)
    model = minimal_model()
    model["nodes"] = {
        "node.x": {"kind": "example.test.widget", "description": "Widget", "data": data}
    }
    model["extensions"] = {
        "extension.test": {
            "namespace": "example.test",
            "version": "1.0.0",
            "path": "extension.json",
            "sha256": sha256(raw).hexdigest(),
        }
    }
    path = directory / "model.json"
    path.write_text(json.dumps(model), encoding="utf-8")
    return path


@pytest.mark.parametrize("reference", ["$ref", "$dynamicRef"])
@pytest.mark.parametrize(
    "hidden",
    [
        {"$schema": "http://json-schema.org/draft-07/schema#", "type": "object"},
        {"$ref": "https://example.org/remote"},
        {"$id": "https://example.org/mutable", "type": "object"},
        {"x-executable": "run code", "type": "object"},
        {"type": 23},
    ],
)
def test_local_reference_targets_receive_schema_guards(tmp_path, reference, hidden):
    path = write_extension(tmp_path, {"hidden": hidden, reference: "#/hidden"}, {})
    report = validate(path)
    assert aggregate(report["checks"], "model") == "FAIL"
    assert "EXTENSION_DATA_INVALID" in codes(report)


def test_hidden_dialect_cannot_disable_2020_constraints(tmp_path):
    target = {
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "items": {"type": "array", "prefixItems": [{"type": "integer"}]}
        },
    }
    path = write_extension(
        tmp_path, {"legacy": target, "$ref": "#/legacy"}, {"items": ["INVALID INTEGER"]}
    )
    assert aggregate(validate(path)["checks"], "model") == "FAIL"


@pytest.mark.parametrize(
    "data_schema,data,result",
    [
        (
            {"hidden": {"type": "object", "required": ["value"]}, "$ref": "#/hidden"},
            {"value": 1},
            "PASS",
        ),
        (
            {"hidden": {"type": "object", "required": ["value"]}, "$ref": "#/hidden"},
            {},
            "FAIL",
        ),
        (
            {"$defs": {"x": {"$anchor": "value", "type": "object"}}, "$ref": "#value"},
            {},
            "PASS",
        ),
        (
            {
                "$defs": {"x": {"$dynamicAnchor": "value", "type": "object"}},
                "$dynamicRef": "#value",
            },
            {},
            "PASS",
        ),
        (
            {"const": {"$ref": "https://example.org/inert", "$schema": "other"}},
            {"$ref": "https://example.org/inert", "$schema": "other"},
            "PASS",
        ),
        (
            {
                "metadata": {"$schema": "other", "$ref": "https://example.org/inert"},
                "type": "object",
            },
            {},
            "PASS",
        ),
        ({"hidden": False, "$ref": "#/hidden"}, {}, "FAIL"),
        ({"hidden": True, "$ref": "#/hidden"}, {}, "PASS"),
        (
            {
                "$defs": {
                    "node": {
                        "type": "object",
                        "properties": {"child": {"$ref": "#/$defs/node"}},
                    }
                },
                "$ref": "#/$defs/node",
            },
            {"child": {"child": {}}},
            "PASS",
        ),
    ],
)
def test_useful_local_references_and_inert_data_survive(
    tmp_path, data_schema, data, result
):
    assert (
        aggregate(
            validate(write_extension(tmp_path, data_schema, data))["checks"], "model"
        )
        == result
    )


def test_extension_metaschema_depth_is_unknown(tmp_path):
    spec = {"type": "string"}
    for _ in range(220):
        spec = {"type": "array", "items": spec}
    report = validate(write_extension(tmp_path, spec, {}))
    assert aggregate(report["checks"], "model") == "UNKNOWN"
    assert "EXTENSION_UNSUPPORTED" in codes(report)


def test_extension_recursive_data_depth_is_unknown(tmp_path):
    spec = {
        "$defs": {
            "node": {
                "type": "object",
                "properties": {"child": {"$ref": "#/$defs/node"}},
            }
        },
        "$ref": "#/$defs/node",
    }
    data = {}
    for _ in range(400):
        data = {"child": data}
    report = validate(write_extension(tmp_path, spec, data))
    assert aggregate(report["checks"], "model") == "UNKNOWN"
    assert "EXTENSION_UNSUPPORTED" in codes(report)


def test_nonprogressing_local_reference_cycle_is_unknown(tmp_path):
    report = validate(write_extension(tmp_path, {"$ref": "#"}, {}))
    assert aggregate(report["checks"], "model") == "UNKNOWN"
    assert "EXTENSION_UNSUPPORTED" in codes(report)
