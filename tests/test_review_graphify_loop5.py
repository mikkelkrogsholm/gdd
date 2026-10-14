"""UTF-8-safe diagnostics for duplicate JSON keys containing lone surrogates."""

import json
from pathlib import Path
import subprocess
import sys

import pytest
from jsonschema import Draft202012Validator

from gdd_validator import exit_code, validate
from gdd_validator.graphify import observe_graphify
from gdd_validator.graphify_io import (
    export_graphify,
    import_graphify,
    verify_graphify,
)
from gdd_validator.loading import schema

ROOT = Path(__file__).resolve().parents[1]
PROFILE = "graphify-0.9.75-node-link"
INVALID_KEYS = [b"\\ud800", b"\\udfff"]


def assert_utf8_fail(report):
    encoded = json.dumps(report, ensure_ascii=False).encode("utf-8")
    Draft202012Validator(schema("report")).validate(json.loads(encoded))
    assert exit_code(report) == 1
    return [d["message"] for c in report["checks"] for d in c["diagnostics"]]


def duplicate_raw(key):
    return b'{"' + key + b'":1,"' + key + b'":2}'


@pytest.mark.parametrize("key", INVALID_KEYS)
def test_core_duplicate_surrogate_key_has_utf8_fail_report(tmp_path, key):
    source = tmp_path / "model.json"
    source.write_bytes(duplicate_raw(key))
    messages = assert_utf8_fail(validate(source))
    assert any("Duplicate mapping key" in text for text in messages)
    assert any(key.decode("ascii") in text for text in messages)


@pytest.mark.parametrize("key", INVALID_KEYS)
def test_duplicate_surrogate_key_remains_fail_and_exactly_recoverable(tmp_path, key):
    raw = duplicate_raw(key)
    _, observed = observe_graphify(raw, profile=PROFILE)
    assert_utf8_fail(observed)
    source = tmp_path / "graph.json"
    source.write_bytes(raw)
    output = tmp_path / "bundle"
    assert_utf8_fail(import_graphify(source, output, profile=PROFILE))
    assert (output / "graphify.raw.json").read_bytes() == raw
    stored = json.loads((output / "import.report.json").read_bytes())
    assert_utf8_fail(stored)
    assert_utf8_fail(verify_graphify(output))
    recovered = tmp_path / "recovered.json"
    assert_utf8_fail(export_graphify(output, recovered))
    assert recovered.read_bytes() == raw


@pytest.mark.parametrize("key", INVALID_KEYS)
@pytest.mark.parametrize("adapter", [False, True])
def test_cli_duplicate_surrogate_key_returns_json_fail(tmp_path, key, adapter):
    source = tmp_path / "graph.json"
    source.write_bytes(duplicate_raw(key))
    arguments = (
        ["graphify", "validate", str(source), "--profile", PROFILE]
        if adapter
        else ["validate", str(source)]
    )
    result = subprocess.run(
        [sys.executable, "-m", "gdd_validator", *arguments],
        cwd=ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    assert result.returncode == 1
    assert result.stderr == ""
    assert "Traceback" not in result.stdout
    assert_utf8_fail(json.loads(result.stdout))


@pytest.mark.parametrize("adapter", [False, True])
def test_valid_unicode_duplicate_key_remains_readable_failure(tmp_path, adapter):
    key = "café/🔥"
    raw = duplicate_raw(key.encode("utf-8"))
    if adapter:
        report = observe_graphify(raw, profile=PROFILE)[1]
    else:
        source = tmp_path / "model.json"
        source.write_bytes(raw)
        report = validate(source)
    messages = assert_utf8_fail(report)
    assert any("Duplicate" in text and key in text for text in messages)


@pytest.mark.parametrize("key", INVALID_KEYS)
@pytest.mark.parametrize("adapter", [False, True])
def test_nonduplicate_surrogate_scalar_stays_utf8_failure(tmp_path, key, adapter):
    raw = b'{"key":"' + key + b'"}'
    if adapter:
        report = observe_graphify(raw, profile=PROFILE)[1]
    else:
        source = tmp_path / "model.json"
        source.write_bytes(raw)
        report = validate(source)
    messages = assert_utf8_fail(report)
    assert any("Invalid Unicode scalar" in text for text in messages)


def test_ordinary_unicode_graph_import_is_unchanged(tmp_path):
    graph = {
        "directed": True,
        "multigraph": False,
        "graph": {},
        "nodes": [
            {
                "id": "🔥",
                "label": "café",
                "file_type": "code",
                "source_file": "app.py",
            }
        ],
        "links": [],
    }
    raw = json.dumps(graph, ensure_ascii=False).encode("utf-8")
    source = tmp_path / "graph.json"
    source.write_bytes(raw)
    output = tmp_path / "bundle"
    assert exit_code(import_graphify(source, output, profile=PROFILE)) == 0
    assert exit_code(verify_graphify(output)) == 0
    recovered = tmp_path / "recovered.json"
    assert exit_code(export_graphify(output, recovered)) == 0
    assert recovered.read_bytes() == raw
