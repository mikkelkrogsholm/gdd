"""Regression cases from the first adversarial framework review."""

from copy import deepcopy
import json
from pathlib import Path
import re
import subprocess
import sys

import pytest
from jsonschema import Draft202012Validator

from gdd_validator import exit_code, validate
from gdd_validator.graphify import observe_graphify
from gdd_validator.graphify_io import (
    export_graphify,
    import_graphify,
    load_observations,
    verify_graphify,
)
from gdd_validator.graphify_proposal import propose_graphify
from gdd_validator.loading import InputError, parse, parse_json, resolve_asset

PROFILE = "graphify-0.9.75-node-link"
ROOT = Path(__file__).resolve().parents[1]


def graph(node_ids=("a", "b"), source="a"):
    return {
        "directed": True,
        "multigraph": False,
        "graph": {},
        "nodes": [
            {
                "id": ident,
                "label": str(ident),
                "file_type": "code",
                "source_file": "app.py",
            }
            for ident in node_ids
        ],
        "links": [{"source": source, "target": node_ids[-1], "relation": "calls"}],
    }


def make_bundle(tmp_path):
    source = tmp_path / "graph.json"
    source.write_text(json.dumps(graph()))
    folder = tmp_path / "observations"
    assert exit_code(import_graphify(source, folder, profile=PROFILE)) == 0
    index, raw, report = load_observations(folder)
    assert raw is not None and exit_code(report) == 0
    mapping = {
        "format": "gdd-graphify-mapping/0.2",
        "project": {"id": "project.review", "title": "Review", "goal": "Review"},
        "nodes": [
            {
                "observation": node["id"],
                "target": "component." + node["producer_id"],
                "kind": "component",
                "description": "Explicit unaccepted interpretation",
                "rationale": "Review the observed code",
            }
            for node in index["nodes"]
        ],
        "edges": [],
    }
    return folder, index, raw, report, mapping


@pytest.mark.parametrize(
    "endpoint, expected",
    [(100000000000000000000000, 0), (99999999999999991611392, 1)],
)
def test_decimal_endpoint_identity_never_uses_binary_float_alias(endpoint, expected):
    index, report = observe_graphify(
        json.dumps(graph((1e23, "b"), endpoint)).encode(), profile=PROFILE
    )
    assert exit_code(report) == expected
    resolved = index["edges"][0]["source_observation"]
    assert resolved == (index["nodes"][0]["id"] if expected == 0 else None)


def test_equal_decimal_numeric_nodes_are_ambiguous():
    _, report = observe_graphify(
        json.dumps(graph((1e23, 100000000000000000000000), 1e23)).encode(),
        profile=PROFILE,
    )
    assert exit_code(report) == 1
    assert any(
        diagnostic["code"] == "ID_COLLISION"
        for check in report["checks"]
        for diagnostic in check["diagnostics"]
    )


def test_distinct_decimal_numeric_nodes_are_not_collapsed():
    index, report = observe_graphify(
        json.dumps(graph((1e23, 99999999999999991611392), 1e23)).encode(),
        profile=PROFILE,
    )
    assert exit_code(report) == 0
    assert index["edges"][0]["source_observation"] == index["nodes"][0]["id"]
    assert index["edges"][0]["target_observation"] == index["nodes"][1]["id"]


def test_direction_markers_compare_exact_decimal_identities():
    data = graph((1e23, "b"), 100000000000000000000000)
    data["directed"] = False
    data["links"][0].update(_src=1e23, _tgt="b")
    index, report = observe_graphify(json.dumps(data).encode(), profile=PROFILE)
    assert exit_code(report) == 0
    assert index["edges"][0]["direction"]["value"] == "source_to_target"


def test_missing_original_cannot_support_a_proposal(tmp_path):
    folder, _, _, _, mapping = make_bundle(tmp_path)
    (folder / "graphify.raw.json").unlink()
    assert exit_code(verify_graphify(folder)) == 2
    with pytest.raises(ValueError, match="intact"):
        propose_graphify(folder, mapping, format="gdd/0.2")


@pytest.mark.parametrize("required", ["graphify.integrity", "graphify.projection"])
@pytest.mark.parametrize("result", [None, "UNKNOWN"])
def test_proposal_requires_explicit_positive_integrity_and_projection(
    tmp_path, monkeypatch, required, result
):
    folder, index, raw, report, mapping = make_bundle(tmp_path)
    report = deepcopy(report)
    if result is None:
        report["checks"] = [
            item for item in report["checks"] if item["check_id"] != required
        ]
    else:
        for item in report["checks"]:
            if item["check_id"] == required:
                item["result"] = result
    monkeypatch.setattr(
        "gdd_validator.graphify_proposal.load_observations",
        lambda _: (index, raw, report),
    )
    with pytest.raises(ValueError, match="intact"):
        propose_graphify(folder, mapping, format="gdd/0.2")


def test_looping_raw_symlink_is_a_controlled_invalid_asset(tmp_path):
    folder, _, _, _, mapping = make_bundle(tmp_path)
    original = folder / "graphify.raw.json"
    original.unlink()
    original.symlink_to(original.name)
    report = verify_graphify(folder)
    assert exit_code(report) == 1
    assert report["checks"][0]["diagnostics"][0]["code"] == "ASSET_PATH_INVALID"
    recovered = tmp_path / "recovered.json"
    with pytest.raises(ValueError, match="corrupt/unavailable"):
        export_graphify(folder, recovered)
    assert not recovered.exists()
    with pytest.raises(ValueError, match="intact"):
        propose_graphify(folder, mapping, format="gdd/0.2")


def test_looping_bundle_root_is_a_controlled_invalid_asset(tmp_path):
    root = tmp_path / "loop"
    root.symlink_to(root.name)
    with pytest.raises(InputError) as caught:
        resolve_asset(root, "asset.json")
    assert caught.value.code == "ASSET_PATH_INVALID"


def test_manifest_identifier_trailing_newline_is_rejected_without_crash(tmp_path):
    folder, index, _, _, _ = make_bundle(tmp_path)
    index["source_id"] += "\n"
    (folder / "observations.json").write_text(json.dumps(index))
    report = verify_graphify(folder)
    assert exit_code(report) == 1
    assert report["checks"][0]["diagnostics"][0]["code"] == "SCHEMA_INVALID"


@pytest.mark.parametrize(
    "text", ["color: blue", "blue", "{color: blue}", '{"color": "blue",}']
)
def test_strict_json_never_uses_yaml_fallback(text):
    with pytest.raises(json.JSONDecodeError):
        parse_json(text)
    assert parse(text) is not None


@pytest.mark.parametrize(
    "text",
    [
        '{"a": 1, "a": 2}',
        '{"<<": {}}',
        '{"a": NaN}',
        '{"a": Infinity}',
        '{"a": -Infinity}',
        '{"a": 1e999}',
        '"\\ud800"',
        b'"\xff"',
    ],
)
def test_strict_json_preserves_existing_profile_safeguards(text):
    for loader in (parse_json, parse):
        with pytest.raises(InputError):
            loader(text)


def test_strict_json_preserves_literal_unicode_separators_and_json_values():
    expected = {
        "text": "first\u2028second\u2029third",
        "values": [None, True, 123, 1.25],
    }
    raw = json.dumps(expected, ensure_ascii=False).encode()
    assert parse_json(raw) == parse(raw) == expected


def test_strict_json_deep_nesting_has_a_controlled_error():
    with pytest.raises(InputError):
        parse_json("[" * 2000 + "0" + "]" * 2000)


def test_all_schema_end_anchors_reject_trailing_newlines():
    patterns = set()

    def visit(value):
        if isinstance(value, dict):
            # Nonblank prose can span lines; identity patterns must reject suffixes.
            if "pattern" in value and value["pattern"] != r"\S":
                patterns.add(value["pattern"])
            for item in value.values():
                visit(item)
        elif isinstance(value, list):
            for item in value:
                visit(item)

    for path in (ROOT / "schemas").glob("*.json"):
        document = json.loads(path.read_text())
        Draft202012Validator.check_schema(document)
        visit(document)
    witnesses = [
        "source.graphify",
        "source.graphify.extra",
        "1.2.3",
        "a" * 64,
        "sha256:" + "a" * 64,
    ]
    assert patterns
    for pattern in patterns:
        assert not pattern.endswith("$")
        witness = next(value for value in witnesses if re.search(pattern, value))
        validator = Draft202012Validator({"type": "string", "pattern": pattern})
        assert validator.is_valid(witness)
        assert not validator.is_valid(witness + "\n")
        assert not validator.is_valid(witness + "\r\n")


def test_correct_legacy_importer_bundle_uses_current_checks(tmp_path):
    folder, index, raw, _, mapping = make_bundle(tmp_path)
    assert index["importer"]["version"] == "0.1.2"
    index["importer"]["version"] = "0.1.0"
    (folder / "observations.json").write_text(json.dumps(index))
    report = verify_graphify(folder)
    assert exit_code(report) == 0
    assert report["tool"]["version"] == "0.1.2"
    assert propose_graphify(folder, mapping, format="gdd/0.2")["status"] == "proposed"
    recovered = tmp_path / "recovered.json"
    export_graphify(folder, recovered)
    assert recovered.read_bytes() == raw


def test_legacy_float_alias_projection_is_not_grandfathered(tmp_path):
    raw = json.dumps(graph((1e23, "b"), 99999999999999991611392)).encode()
    index, _ = observe_graphify(raw, profile=PROFILE)
    index["importer"]["version"] = "0.1.0"
    # Version 0.1.0 incorrectly linked this distinct integer to the float ID.
    index["edges"][0]["source_observation"] = index["nodes"][0]["id"]
    folder = tmp_path / "observations"
    folder.mkdir()
    (folder / "graphify.raw.json").write_bytes(raw)
    (folder / "observations.json").write_text(json.dumps(index))
    report = verify_graphify(folder)
    assert exit_code(report) == 1
    assert (
        next(
            check
            for check in report["checks"]
            if check["check_id"] == "graphify.projection"
        )["result"]
        == "FAIL"
    )
    with pytest.raises(ValueError, match="corrupt/unavailable"):
        export_graphify(folder, tmp_path / "recovered.json")


def test_unknown_importer_version_cannot_use_legacy_normalization(tmp_path):
    folder, index, _, _, mapping = make_bundle(tmp_path)
    index["importer"]["version"] = "0.1.3"
    (folder / "observations.json").write_text(json.dumps(index))
    report = verify_graphify(folder)
    assert exit_code(report) == 2
    assert report["checks"][0]["diagnostics"][0]["code"] == "FORMAT_UNSUPPORTED"
    with pytest.raises(ValueError, match="intact"):
        propose_graphify(folder, mapping, format="gdd/0.2")


def test_looping_model_manifest_has_a_structured_public_report(tmp_path):
    manifest = tmp_path / "model.gdd.json"
    manifest.symlink_to(manifest.name)
    report = validate(manifest)
    assert exit_code(report) == 1
    assert report["spec_revision"] is None
    diagnostic = report["checks"][0]["diagnostics"][0]
    assert diagnostic["code"] == "ASSET_PATH_INVALID"
    assert diagnostic["location"]["file"] == str(manifest.absolute())
    result = subprocess.run(
        [sys.executable, "-m", "gdd_validator", "validate", str(manifest)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    assert "Traceback" not in result.stderr
    assert (
        json.loads(result.stdout)["checks"][0]["diagnostics"][0]["code"]
        == "ASSET_PATH_INVALID"
    )
