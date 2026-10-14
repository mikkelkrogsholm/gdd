"""Runtime parser capacity is unresolved input, not a forbidden feature."""

from hashlib import sha256
import json
from pathlib import Path
import subprocess
import sys

import pytest

from gdd_validator import exit_code, validate
from gdd_validator.loading import (
    InputError,
    load_bundle,
    parse,
    parse_json,
    schema_checks,
)
from gdd_validator.report import aggregate

ROOT = Path(__file__).resolve().parents[1]
DEEP = 2000


def raw_value(depth, kind="annotation"):
    if kind == "annotation":
        return "[" * depth + "0" + "]" * depth
    return '{"type":"array","items":' * depth + '{"type":"string"}' + "}" * depth


def model():
    return {
        "format": "gdd/0.2",
        "project": {"id": "review", "title": "Review", "goal": "Parser depth"},
        "nodes": {"entity.x": {"kind": "entity", "description": "Value", "fields": {}}},
        "edges": [],
    }


def raw_document(depth, form="json", kind="annotation", *, fragment=False):
    data = model()
    node = data["nodes"]["entity.x"]
    if kind == "annotation":
        node["annotations"] = {"example.data": "RAW"}
    else:
        node["fields"] = {"v": "RAW"}
    if fragment:
        data["format"] = "gdd-fragment/0.2"
        data.pop("project")
    if form == "json":
        return json.dumps(data).replace('"RAW"', raw_value(depth, kind))
    # Keep the deeply nested value in valid flow syntax within an ordinary YAML manifest.
    header = "format: " + data["format"] + "\n"
    if not fragment:
        header += "project: {id: review, title: Review, goal: Parser depth}\n"
    header += "nodes:\n  entity.x:\n    kind: entity\n    description: Value\n"
    if kind == "annotation":
        header += "    fields: {}\n    annotations:\n      example.data: "
    else:
        header += "    fields:\n      v: "
    return header + raw_value(depth, kind) + "\nedges: []\n"


def codes(report):
    return {d["code"] for c in report["checks"] for d in c["diagnostics"]}


@pytest.mark.parametrize(
    "loader,raw",
    [
        (parse_json, raw_value(DEEP)),
        (parse, raw_value(DEEP)),
        (parse, "value: " + raw_value(DEEP)),
    ],
)
def test_actual_parser_recursion_is_format_unsupported(loader, raw):
    with pytest.raises(InputError) as error:
        loader(raw)
    assert error.value.code == "FORMAT_UNSUPPORTED"


@pytest.mark.parametrize("form", ["json", "yaml"])
@pytest.mark.parametrize("kind", ["annotation", "type"])
def test_valid_deep_document_has_unknown_report(tmp_path, form, kind):
    path = tmp_path / ("model." + form)
    path.write_text(raw_document(DEEP, form, kind))
    report = validate(path)
    assert aggregate(report["checks"], "model") == "UNKNOWN"
    assert exit_code(report) == 2
    assert "FORMAT_UNSUPPORTED" in codes(report)
    assert "YAML_FEATURE_FORBIDDEN" not in codes(report)
    assert report["spec_revision"] is None
    assert load_bundle(path).revision is None
    assert schema_checks(report, "report.json")[0]["result"] == "PASS"


@pytest.mark.parametrize("form", ["json", "yaml"])
@pytest.mark.parametrize("kind", ["annotation", "type"])
def test_lower_depth_supported_json_and_yaml_are_valid(tmp_path, form, kind):
    path = tmp_path / ("model." + form)
    path.write_text(raw_document(20, form, kind))
    report = validate(path, profiles=["readiness"], scope=["entity.x"])
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == "PASS"
    assert exit_code(report) == 0
    assert load_bundle(path).revision is not None


@pytest.mark.parametrize("form", ["json", "yaml"])
def test_deep_fragment_capacity_does_not_create_known_model_failure(tmp_path, form):
    (tmp_path / ("fragment." + form)).write_text(
        raw_document(DEEP, form, fragment=True)
    )
    data = model()
    data["nodes"] = {}
    data["includes"] = ["fragment." + form]
    path = tmp_path / "model.json"
    path.write_text(json.dumps(data))
    report = validate(path)
    assert aggregate(report["checks"], "model") == "UNKNOWN"
    assert exit_code(report) == 2
    assert "YAML_FEATURE_FORBIDDEN" not in codes(report)
    assert load_bundle(path).revision is None


@pytest.mark.parametrize("form", ["json", "yaml"])
@pytest.mark.parametrize("depth,result", [(20, "PASS"), (DEEP, "UNKNOWN")])
def test_contract_source_parser_capacity_stays_scoped_unknown(
    tmp_path, form, depth, result
):
    raw = (
        raw_value(depth) if form == "json" else "value: " + raw_value(depth)
    ).encode()
    (tmp_path / "contract.txt").write_bytes(raw)
    data = model()
    data["nodes"]["entity.x"]["source_refs"] = [
        {"source": "source.contract", "purpose": "contract"}
    ]
    data["sources"] = {
        "source.contract": {
            "role": "knowledge",
            "title": "Contract",
            "uri": "contract.txt",
            "format": form,
            "normative": True,
            "revision": {"sha256": sha256(raw).hexdigest()},
        }
    }
    path = tmp_path / "model.json"
    path.write_text(json.dumps(data))
    report = validate(path, profiles=["readiness"], scope=["entity.x"])
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == result
    assert load_bundle(path).revision is not None
    if result == "UNKNOWN":
        assert "FORMAT_UNSUPPORTED" in codes(report)
        assert "YAML_FEATURE_FORBIDDEN" not in codes(report)


@pytest.mark.parametrize(
    "raw,code",
    [
        ('{"value":1,"value":2}', "YAML_DUPLICATE_KEY"),
        ("value: 1\nvalue: 2", "YAML_DUPLICATE_KEY"),
        ("value: &item [1]\ncopy: *item", "YAML_FEATURE_FORBIDDEN"),
        ("value: !custom 1", "YAML_FEATURE_FORBIDDEN"),
        ('{"value": NaN}', "YAML_FEATURE_FORBIDDEN"),
        ('{"value":"\\ud800"}', "YAML_FEATURE_FORBIDDEN"),
        (b"\xff", "YAML_FEATURE_FORBIDDEN"),
        ('{"value":[}', "YAML_FEATURE_FORBIDDEN"),
    ],
)
def test_actual_forbidden_features_remain_failures(raw, code):
    with pytest.raises(InputError) as error:
        parse(raw)
    assert error.value.code == code


def test_shallow_json_syntax_error_remains_json_decode_error():
    with pytest.raises(json.JSONDecodeError):
        parse_json('{"value":[}')


@pytest.mark.parametrize("form", ["json", "yaml"])
@pytest.mark.parametrize("kind", ["annotation", "type"])
def test_cli_deep_input_reports_unknown_without_traceback_or_pass(tmp_path, form, kind):
    path = tmp_path / ("model." + form)
    path.write_text(raw_document(DEEP, form, kind))
    output = tmp_path / "report.json"
    process = subprocess.run(
        [
            sys.executable,
            "-B",
            "-m",
            "gdd_validator",
            "validate",
            str(path),
            "--output",
            str(output),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert process.returncode == 2
    assert "Traceback" not in process.stderr
    report = parse_json(output.read_bytes())
    assert schema_checks(report, "report.json")[0]["result"] == "PASS"
    assert aggregate(report["checks"], "model") == "UNKNOWN"
    assert "FORMAT_UNSUPPORTED" in codes(report)
    assert "YAML_FEATURE_FORBIDDEN" not in codes(report)
    assert not any(c["result"] == "PASS" for c in report["checks"])


@pytest.mark.parametrize("form", ["json", "yaml"])
def test_cli_explicit_deep_import_audit_exits_two_without_output_claim(tmp_path, form):
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model()))
    audit = tmp_path / ("audit." + form)
    if form == "json":
        raw = '{"format":"gdd-import-audit/0.2","original":RAW,"reconstructed":RAW}'
    else:
        raw = "format: gdd-import-audit/0.2\noriginal: RAW\nreconstructed: RAW\n"
    audit.write_text(raw.replace("RAW", raw_value(DEEP)))
    output = tmp_path / "report.json"
    process = subprocess.run(
        [
            sys.executable,
            "-B",
            "-m",
            "gdd_validator",
            "validate",
            str(path),
            "--import-audit",
            str(audit),
            "--output",
            str(output),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert process.returncode == 2
    assert "gdd:" in process.stderr and "Traceback" not in process.stderr
    assert not output.exists()
    assert not process.stdout


@pytest.mark.parametrize("raw", ['{"format":"gdd/0.2",}', "format: !custom gdd/0.2"])
def test_cli_shallow_invalid_input_still_exits_one(tmp_path, raw):
    path = tmp_path / "bad.json"
    path.write_text(raw)
    process = subprocess.run(
        [sys.executable, "-B", "-m", "gdd_validator", "validate", str(path)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert process.returncode == 1
    assert "Traceback" not in process.stderr
