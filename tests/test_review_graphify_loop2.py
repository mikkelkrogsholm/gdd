"""Depth, Unicode and proposal API regressions from review loop two."""

from copy import deepcopy
import json

import pytest

from gdd_validator import exit_code, validate
from gdd_validator.graphify_io import import_graphify, load_observations
from gdd_validator.graphify_proposal import propose_graphify
from gdd_validator.loading import parse_json, schema_checks

PROFILE = "graphify-0.9.75-node-link"


def model_with_type(depth, leaf="string"):
    value = {"type": leaf}
    for _ in range(depth):
        value = {"type": "array", "items": value}
    return {
        "format": "gdd/0.2",
        "project": {"id": "project.depth", "title": "Depth", "goal": "Depth"},
        "nodes": {
            "entity.depth": {
                "kind": "entity",
                "description": "Nested data",
                "fields": {"nested": value},
            }
        },
        "edges": [],
    }


def test_actual_schema_library_depth_exhaustion_is_scoped_unknown():
    checks = schema_checks(model_with_type(600), "deep-model.json")
    assert checks[0]["result"] == "UNKNOWN"
    diagnostic = checks[0]["diagnostics"][0]
    assert diagnostic["code"] == "FORMAT_UNSUPPORTED"
    assert diagnostic["severity"] == "warning"
    assert diagnostic["location"] == {"file": "deep-model.json", "pointer": ""}
    report = {
        "format": "gdd-report/0.2",
        "tool": {"name": "review", "version": "1"},
        "spec_revision": None,
        "profiles": ["representation"],
        "scope": [],
        "checks": checks,
    }
    assert exit_code(report) == 2
    assert schema_checks(report, "report.json")[0]["result"] == "PASS"


def test_public_validation_of_valid_deep_types_returns_a_report(tmp_path):
    path = tmp_path / "deep-model.json"
    path.write_text(json.dumps(model_with_type(600)))
    report = validate(path)
    assert exit_code(report) == 2
    assert report["spec_revision"] is None
    assert any(
        diagnostic["code"] == "FORMAT_UNSUPPORTED"
        for check in report["checks"]
        for diagnostic in check["diagnostics"]
    )
    assert schema_checks(report, "report.json")[0]["result"] == "PASS"


def test_schema_depth_guard_keeps_valid_and_invalid_shallow_results():
    assert schema_checks(model_with_type(5), "valid.json")[0]["result"] == "PASS"
    assert (
        schema_checks(model_with_type(5, "not_a_type"), "invalid.json")[0]["result"]
        == "FAIL"
    )


def test_deep_opaque_annotation_is_not_rejected_by_a_blanket_depth_limit():
    model = model_with_type(0)
    annotation = "leaf"
    for _ in range(600):
        annotation = {"next": annotation}
    model["nodes"]["entity.depth"]["annotations"] = {"org.review": annotation}
    assert schema_checks(model, "opaque.json")[0]["result"] == "PASS"


@pytest.mark.parametrize("version", ["١.٢.٣", "１.２.３", "1.٢.3"])
@pytest.mark.parametrize("format", ["extension", "manifest"])
def test_extension_versions_reject_non_ascii_digits(version, format):
    assert version_result(version, format) == "FAIL"


@pytest.mark.parametrize("version", ["0.0.0", "1.2.3", "10.20.30-alpha.1"])
@pytest.mark.parametrize("format", ["extension", "manifest"])
def test_extension_versions_keep_ascii_digits(version, format):
    assert version_result(version, format) == "PASS"


def version_result(version, format):
    if format == "extension":
        value = {
            "format": "gdd-extension/0.2",
            "namespace": "org.review",
            "version": version,
            "node_kinds": {},
            "relations": {},
        }
    else:
        value = model_with_type(0)
        value["extensions"] = {
            "extension.review": {
                "namespace": "org.review",
                "version": version,
                "path": "extension.json",
                "sha256": "0" * 64,
            }
        }
    return schema_checks(value, "version.json")[0]["result"]


@pytest.fixture
def proposal_inputs(tmp_path):
    raw = {
        "directed": True,
        "multigraph": False,
        "graph": {},
        "nodes": [
            {"id": key, "label": key, "file_type": "code", "source_file": "app.py"}
            for key in ("a", "b")
        ],
        "links": [{"source": "a", "target": "b", "relation": "calls"}],
    }
    source = tmp_path / "input.json"
    source.write_text(json.dumps(raw))
    folder = tmp_path / "observations"
    assert exit_code(import_graphify(source, folder, profile=PROFILE)) == 0
    index, _, _ = load_observations(folder)
    mapping = {
        "format": "gdd-graphify-mapping/0.2",
        "project": {"id": "project.review", "title": "Review", "goal": "Review"},
        "nodes": [
            {
                "observation": node["id"],
                "target": "action." + node["producer_id"],
                "kind": "action",
                "description": "Explicit interpretation",
                "rationale": "Explicit review",
            }
            for node in index["nodes"]
        ],
        "edges": [
            {
                "observation": index["edges"][0]["id"],
                "id": "edge.review",
                "from": "action.a",
                "to": "action.b",
                "relation": "calls",
                "rationale": "Explicit interpretation",
            }
        ],
    }
    return folder, mapping


@pytest.mark.parametrize("text", ["bad\ud800text", "\ud83d\ude00"])
@pytest.mark.parametrize(
    "field", ["title", "description", "node_rationale", "edge_rationale", "source_uri"]
)
def test_proposal_api_rejects_every_unencodable_string(proposal_inputs, field, text):
    folder, mapping = proposal_inputs
    source_uri = "graphify.raw.json"
    if field == "title":
        mapping["project"]["title"] = text
    elif field == "description":
        mapping["nodes"][0]["description"] = text
    elif field == "node_rationale":
        mapping["nodes"][0]["rationale"] = text
    elif field == "edge_rationale":
        mapping["edges"][0]["rationale"] = text
    else:
        source_uri = text
    with pytest.raises(ValueError, match="JSON"):
        propose_graphify(folder, mapping, source_uri=source_uri, format="gdd/0.2")


@pytest.mark.parametrize("number", [float("nan"), float("inf"), float("-inf")])
def test_proposal_api_rejects_nonfinite_values_before_schema_or_io(
    proposal_inputs, monkeypatch, number
):
    folder, mapping = proposal_inputs
    mapping["nodes"][0]["rationale"] = number
    monkeypatch.setattr(
        "gdd_validator.graphify_proposal.load_observations",
        lambda _: pytest.fail("Invalid JSON must be rejected before bundle IO"),
    )
    with pytest.raises(ValueError, match="JSON"):
        propose_graphify(folder, mapping, format="gdd/0.2")


@pytest.mark.parametrize("value", [b"\xff", object()])
def test_proposal_source_uri_rejects_non_json_values_before_io(
    proposal_inputs, monkeypatch, value
):
    folder, mapping = proposal_inputs
    monkeypatch.setattr(
        "gdd_validator.graphify_proposal.load_observations",
        lambda _: pytest.fail("Invalid JSON must be rejected before bundle IO"),
    )
    with pytest.raises(ValueError, match="JSON"):
        propose_graphify(folder, mapping, source_uri=value, format="gdd/0.2")


def test_proposal_api_rejects_circular_input_with_controlled_error(proposal_inputs):
    folder, mapping = proposal_inputs
    mapping["project"]["title"] = mapping
    with pytest.raises(ValueError, match="JSON"):
        propose_graphify(folder, mapping, format="gdd/0.2")


def test_proposal_api_depth_exhaustion_is_a_controlled_error(proposal_inputs):
    folder, mapping = proposal_inputs
    nested = "leaf"
    for _ in range(2000):
        nested = [nested]
    mapping["extra"] = nested
    with pytest.raises(ValueError, match="JSON"):
        propose_graphify(folder, mapping, format="gdd/0.2")


def test_proposal_api_keeps_valid_unicode_and_materializes_a_valid_candidate(
    proposal_inputs,
):
    folder, mapping = proposal_inputs
    text = "Review \u00e6\u00f8\u00e5 😀\u2028next\u2029last"
    mapping["project"]["title"] = text
    mapping["nodes"][0]["description"] = text
    mapping["nodes"][0]["rationale"] = text
    mapping["edges"][0]["rationale"] = text
    before = deepcopy(mapping)
    proposal = propose_graphify(folder, mapping, format="gdd/0.2")
    assert proposal["mapping"] == mapping == before
    assert proposal["status"] == "proposed"
    candidate = proposal["model_candidate"]
    encoded = json.dumps(candidate, ensure_ascii=False, allow_nan=False).encode("utf-8")
    assert parse_json(encoded) == candidate
    path = folder / "candidate.gdd.json"
    path.write_bytes(encoded)
    assert exit_code(validate(path)) == 0
    assert candidate["sources"]["source.graphify"]["normative"] is False
    assert all(node["maturity"] == "sketch" for node in candidate["nodes"].values())
