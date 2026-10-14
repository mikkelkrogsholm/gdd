"""Projection capacity stops are controlled and never invented corruption."""

from pathlib import Path
import json
import subprocess
import sys

import pytest

from gdd_validator import exit_code, render_view, verify_view
from gdd_validator.loading import InputError, schema
from gdd_validator import visualization as view
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]


def model_file(directory, depth=0):
    model = {
        "format": "gdd/0.2",
        "project": {
            "id": "review.capacity",
            "title": "Capacity",
            "goal": "Honest stop",
        },
        "nodes": {
            "entity.x": {
                "kind": "entity",
                "description": "Capacity fixture",
                "fields": {},
                "annotations": {"example.data": "NESTED"},
            }
        },
        "edges": [],
    }
    raw = json.dumps(model).replace('"NESTED"', "[" * depth + "0" + "]" * depth)
    path = directory / "model.json"
    path.write_text(raw, encoding="utf-8")
    return path


def exhaust(monkeypatch, phase):
    if phase == "canonical":

        def failed(value):
            raise RecursionError("simulated canonical capacity")

        monkeypatch.setattr(view, "canonical", failed)
    elif phase == "decode":
        original = view.json.loads

        def failed(value, *args, **kwargs):
            if isinstance(value, str) and '"gdd-view/0.2"' in value:
                raise RecursionError("simulated payload decoding capacity")
            return original(value, *args, **kwargs)

        monkeypatch.setattr(view.json, "loads", failed)
    else:
        original = view.json.dumps

        def failed(value, *args, **kwargs):
            selected = isinstance(value, dict) and (
                phase == "text"
                and value.get("description") == "Capacity fixture"
                or phase == "payload"
                and value.get("format") == "gdd-view/0.2"
            )
            if selected:
                raise RecursionError("simulated serialization capacity")
            return original(value, *args, **kwargs)

        monkeypatch.setattr(view.json, "dumps", failed)


def assert_capacity(error):
    assert isinstance(error.value, InputError)
    assert error.value.code == "FORMAT_UNSUPPORTED"


@pytest.mark.parametrize("phase", ["text", "payload"])
def test_exported_projection_helpers_control_serialization_capacity(
    tmp_path, monkeypatch, phase
):
    bundle = view.model_bundle(model_file(tmp_path))
    data = view.view_data(bundle)
    exhaust(monkeypatch, phase)
    with pytest.raises(InputError) as error:
        view.view_data(bundle) if phase == "text" else view.html_document(data)
    assert_capacity(error)


@pytest.mark.parametrize("phase", ["text", "payload", "decode"])
@pytest.mark.parametrize("existing", [False, True])
def test_render_capacity_stops_before_output_is_opened(
    tmp_path, monkeypatch, phase, existing
):
    source = model_file(tmp_path)
    output = tmp_path / "graph.html"
    if existing:
        output.write_bytes(b"existing artifact")
    exhaust(monkeypatch, phase)
    with pytest.raises(InputError) as error:
        render_view(source, output)
    assert_capacity(error)
    if existing:
        assert output.read_bytes() == b"existing artifact"
    else:
        assert not output.exists()


@pytest.mark.parametrize("phase", ["decode", "canonical", "text", "payload"])
def test_verification_capacity_is_unknown_not_artifact_corruption(
    tmp_path, monkeypatch, phase
):
    source = model_file(tmp_path)
    output = tmp_path / "graph.html"
    render_view(source, output)
    original = output.read_bytes()
    exhaust(monkeypatch, phase)
    report = verify_view(source, output)
    assert exit_code(report) == 2
    assert report["checks"][0]["result"] == "UNKNOWN"
    assert report["checks"][0]["diagnostics"][0]["code"] == "FORMAT_UNSUPPORTED"
    Draft202012Validator(schema("report")).validate(report)
    assert output.read_bytes() == original


@pytest.mark.parametrize("tamper", ["malformed", "newline", "duplicate", "content"])
def test_known_artifact_violations_remain_fail(tmp_path, tamper):
    source = model_file(tmp_path)
    output = tmp_path / "graph.html"
    render_view(source, output)
    html = output.read_text()
    if tamper == "malformed":
        html = '<script id="gdd-data" type="application/json">{invalid}</script>'
    elif tamper == "newline":
        html = html.replace("\n", "\r\n")
    elif tamper == "duplicate":
        html += '<script id="gdd-data" type="application/json">{}</script>'
    else:
        html += "<!-- changed artifact -->"
    output.write_bytes(html.encode())
    assert exit_code(verify_view(source, output)) == 1


@pytest.mark.parametrize("depth", [0, 20, 300])
def test_generated_supported_artifact_reloads_exactly(tmp_path, depth):
    source = model_file(tmp_path, depth)
    before = source.read_bytes()
    output = tmp_path / "graph.html"
    assert exit_code(render_view(source, output)) == 0
    assert exit_code(verify_view(source, output)) == 0
    data = json.loads(view.PAYLOAD.findall(output.read_text())[0])
    assert output.read_text() == view.html_document(data)
    assert source.read_bytes() == before


@pytest.mark.parametrize("depth", [980, 991, 995, 1200])
def test_real_near_capacity_cli_has_no_traceback_or_partial_output(tmp_path, depth):
    source = model_file(tmp_path, depth)
    output = tmp_path / "graph.html"
    command = [sys.executable, "-m", "gdd_validator"]
    result = subprocess.run(
        command + ["view", str(source), "--output", str(output)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert "Traceback" not in result.stderr
    assert result.returncode in {0, 1, 2}
    if result.returncode == 0:
        assert output.is_file()
        verified = subprocess.run(
            command
            + [
                "validate",
                str(source),
                "--profile",
                "visualization",
                "--view",
                str(output),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        assert "Traceback" not in verified.stderr
        assert verified.returncode in {0, 2}
    else:
        assert not output.exists()


def test_cli_serialization_capacity_exit_is_unknown(tmp_path):
    source = model_file(tmp_path)
    output = tmp_path / "graph.html"
    code = """
import json, sys
from gdd_validator.cli import main
original = json.dumps
def exhausted(value, *args, **kwargs):
    if isinstance(value, dict) and value.get('description') == 'Capacity fixture':
        raise RecursionError('simulated serialization capacity')
    return original(value, *args, **kwargs)
json.dumps = exhausted
sys.argv = ['gdd', 'view', sys.argv[1], '--output', sys.argv[2]]
raise SystemExit(main())
"""
    result = subprocess.run(
        [sys.executable, "-c", code, str(source), str(output)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 2
    assert "Traceback" not in result.stderr
    assert "nesting capacity" in result.stderr
    assert not output.exists()
