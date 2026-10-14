"""Meaningful checks of projection fidelity, trust boundaries and CLI delivery."""

from copy import deepcopy
import json
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

from conformance.viewer_fixture import HOSTILE, fixture, write_fixture
from gdd_validator import exit_code, render_view, validate, verify_view
from gdd_validator.loading import load_bundle, schema_checks
from gdd_validator.visualization import PAYLOAD, html_document, model_bundle, view_data

ROOT = Path(__file__).resolve().parents[1]


def embedded(path):
    return json.loads(PAYLOAD.findall(path.read_text(encoding="utf-8"))[0])


@pytest.mark.parametrize(
    "name", ["concert", "sketch", "refinement", "extension", "fragmented"]
)
def test_examples_roundtrip_and_reports(tmp_path, name):
    path = ROOT / "examples" / f"{name}.gdd.yaml"
    output = tmp_path / "graph.html"
    assert exit_code(render_view(path, output)) == 0
    data = embedded(output)
    assert data["model"] == load_bundle(path).model
    assert data["spec_revision"] == load_bundle(path).revision
    assert set(data["manifest"]["shown_nodes"]) == set(data["model"]["nodes"])
    assert set(data["manifest"]["shown_edges"]) == {
        e["id"] for e in data["model"]["edges"]
    }
    report = verify_view(path, output)
    assert exit_code(report) == 0
    assert all(c["result"] == "PASS" for c in schema_checks(report, "report"))
    assert exit_code(validate(path, profiles=["visualization"], view_path=output)) == 0


def test_multigraph_and_exact_numeric_text_are_preserved(tmp_path):
    path = write_fixture(tmp_path / "model.json")
    output = tmp_path / "graph.html"
    render_view(path, output)
    data = embedded(output)
    assert data["model"] == fixture()
    assert len(data["model"]["edges"]) == 9
    assert (
        "9007199254740993"
        in data["text"]["nodes"]["entity.precision"]["fields"]["fields"]
    )
    assert data["text"]["nodes"]["constructor"]["fields"]["behavior"] == HOSTILE
    assert data["observation_inputs"] == []
    assert set(data["locations"]) >= set(data["model"]["nodes"])


def test_html_escaping_and_offline_assets(tmp_path):
    path = write_fixture(tmp_path / "model.json")
    output = tmp_path / "graph.html"
    render_view(path, output)
    html = output.read_text(encoding="utf-8")
    assert HOSTILE not in html
    assert "\\u003c/script\\u003e" in html
    assert "<script src=" not in html and "<link " not in html
    assert "connect-src 'none'" in html
    assert len(PAYLOAD.findall(html)) == 1
    assert embedded(output)["model"]["nodes"]["constructor"]["behavior"] == HOSTILE


def test_context_scope_partitions_without_dropping_records(tmp_path):
    path = ROOT / "examples/concert.gdd.yaml"
    output = tmp_path / "scoped.html"
    render_view(path, output, scope=["action.reserve", "action.reserve"])
    data = embedded(output)
    assert data["scope"] == ["action.reserve"]
    manifest = data["manifest"]
    assert len(manifest["shown_nodes"]) == 34
    assert len(manifest["shown_edges"]) == 73
    assert len(manifest["omitted_nodes"]) == 4
    assert len(manifest["omitted_edges"]) == 12
    assert not set(manifest["shown_nodes"]) & set(manifest["omitted_nodes"])
    assert len(data["model"]["nodes"]) == 38  # Scope is not redaction.
    assert exit_code(verify_view(path, output)) == 0


@pytest.mark.parametrize(
    "operation",
    ["text", "direction", "manifest", "script", "blank", "duplicate_payload"],
)
def test_tampered_artifact_fails(tmp_path, operation):
    path = write_fixture(tmp_path / "model.json")
    output = tmp_path / "graph.html"
    render_view(path, output)
    html = output.read_text(encoding="utf-8")
    if operation in {"text", "direction", "manifest"}:
        data = embedded(output)
        if operation == "text":
            data["model"]["nodes"]["action.second"]["description"] = "Changed"
        if operation == "direction":
            data["model"]["edges"][0]["to"] = "constructor"
        if operation == "manifest":
            data["manifest"]["shown_edges"] = []
        html = html_document(data)
    elif operation == "script":
        html = html.replace('"use strict";', '"use strict";window.hacked=1;')
    elif operation == "blank":
        html = "<html></html>"
    else:
        html += PAYLOAD.search(html)[0]
    output.write_text(html, encoding="utf-8")
    assert exit_code(verify_view(path, output)) == 1


def test_stale_revision_and_unknown_renderer_are_distinct(tmp_path):
    path = write_fixture(tmp_path / "model.json")
    output = tmp_path / "graph.html"
    render_view(path, output)
    data = embedded(output)
    data["generator"]["version"] = "99.0"
    output.write_text(html_document(data), encoding="utf-8")
    assert exit_code(verify_view(path, output)) == 2
    data["generator"]["version"] = "0.1.3"
    data["spec_revision"] = "sha256:" + "0" * 64
    output.write_text(html_document(data), encoding="utf-8")
    report = verify_view(path, output)
    assert exit_code(report) == 1
    assert report["checks"][0]["diagnostics"][0]["code"] == "VIEW_REVISION_MISMATCH"


def test_deterministic_and_read_only(tmp_path):
    path = write_fixture(tmp_path / "model.json")
    before = path.read_bytes()
    a, b = tmp_path / "a.html", tmp_path / "b.html"
    render_view(path, a)
    render_view(path, b)
    assert a.read_bytes() == b.read_bytes()
    assert path.read_bytes() == before


@pytest.mark.parametrize("symlink", [False, True])
def test_existing_output_never_clobbered(tmp_path, symlink):
    path = write_fixture(tmp_path / "model.json")
    output = tmp_path / "graph.html"
    target = tmp_path / "important"
    target.write_bytes(b"preserve")
    if symlink:
        output.symlink_to(target)
    else:
        output.write_bytes(b"preserve")
    with pytest.raises(FileExistsError):
        render_view(path, output)
    assert output.read_bytes() == target.read_bytes() == b"preserve"


@pytest.mark.parametrize("change", ["format", "dangling", "unknown_scope"])
def test_unresolved_input_has_no_output(tmp_path, change):
    path = write_fixture(tmp_path / "model.json")
    data = fixture()
    if change == "format":
        data["format"] = "gdd/99"
    if change == "dangling":
        data["edges"][0]["to"] = "action.missing"
    path.write_text(json.dumps(data))
    output = tmp_path / "graph.html"
    with pytest.raises(ValueError):
        render_view(
            path, output, scope=["missing"] if change == "unknown_scope" else []
        )
    assert not output.exists()


def test_missing_artifact_is_unknown(tmp_path):
    path = write_fixture(tmp_path / "model.json")
    assert exit_code(validate(path, profiles=["visualization"])) == 2
    assert exit_code(verify_view(path, tmp_path / "missing.html")) == 2


def test_cli_view_and_verification(tmp_path):
    path = write_fixture(tmp_path / "model.json")
    output = tmp_path / "graph.html"
    command = [sys.executable, "-m", "gdd_validator"]
    result = subprocess.run(
        command + ["view", str(path), "--output", str(output)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0 and output.exists()
    assert json.loads(result.stdout)["profiles"] == ["visualization"]
    result = subprocess.run(
        command
        + ["validate", str(path), "--profile", "visualization", "--view", str(output)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0
    before = output.read_bytes()
    result = subprocess.run(
        command + ["view", str(path), "--output", str(output)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1 and output.read_bytes() == before


def test_deep_payload_and_newlines_are_controlled(tmp_path):
    path = write_fixture(tmp_path / "model.json")
    output = tmp_path / "graph.html"
    render_view(path, output)
    original = output.read_bytes()
    output.write_bytes(original.replace(b"\n", b"\r\n"))
    assert exit_code(verify_view(path, output)) == 1
    output.write_text(
        '<script id="gdd-data" type="application/json">'
        + "[" * 2000
        + "0"
        + "]" * 2000
        + "</script>"
    )
    assert exit_code(verify_view(path, output)) == 2


def test_empty_model_and_fragment_edge_locations(tmp_path):
    data = fixture()
    data.update(nodes={}, edges=[], sources={})
    path = tmp_path / "empty.json"
    path.write_text(json.dumps(data))
    output = tmp_path / "empty.html"
    assert exit_code(render_view(path, output)) == 0
    assert embedded(output)["manifest"]["shown_nodes"] == []
    fragment = view_data(model_bundle(ROOT / "examples/fragmented.gdd.yaml"))
    assert fragment["locations"]["edge.note.owner"] == [
        "fragmented.gdd.yaml",
        "/edges/0",
    ]
    assert fragment["locations"]["entity.note"] == [
        "note.fragment.yaml",
        "/nodes/entity.note",
    ]


def test_literal_json_unicode_is_not_folded():
    from gdd_validator.loading import parse

    value = "before \u2028 \u2029 after"
    assert parse(json.dumps({"value": value}, ensure_ascii=False))["value"] == value


def test_renderer_conformance_cases():
    from conformance.run_visualization import run

    report = run()
    assert report["matched"] == report["total"] == 16


def test_view_payload_schema_is_packaged_and_offline(tmp_path):
    from jsonschema import Draft202012Validator
    from referencing import Registry, Resource
    from gdd_validator.loading import schema

    resources = []
    for path in (ROOT / "schemas").glob("*.json"):
        value = json.loads(path.read_text())
        Draft202012Validator.check_schema(value)
        if "$id" in value:
            resources.append((value["$id"], Resource.from_contents(value)))
    registry = Registry().with_resources(resources)
    path = write_fixture(tmp_path / "model.json")
    data = view_data(model_bundle(path))
    Draft202012Validator(schema("view"), registry=registry).validate(data)


def test_shipped_demo_matches_current_renderer():
    assert (
        exit_code(
            verify_view(
                ROOT / "examples/concert.gdd.yaml", ROOT / "examples/concert.graph.html"
            )
        )
        == 0
    )
