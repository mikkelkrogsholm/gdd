"""Finite numeric input must retain its declared value or remain unresolved."""

from hashlib import sha256
import json
from pathlib import Path
import subprocess
import sys

import pytest

from gdd_validator import exit_code, validate
from gdd_validator.loading import (
    InputError,
    canonical,
    load_bundle,
    parse,
    parse_json,
    schema_checks,
)
from gdd_validator.report import aggregate

ROOT = Path(__file__).resolve().parents[1]
UNSUPPORTED = [
    "1e-4000",
    "0.10000000000000001",
    "1.0000000000000001",
    "1e4000",
    "1e999999999999999999999999",
    "1e-999999999999999999999999",
]


def model():
    return {
        "format": "gdd/0.2",
        "project": {"id": "review", "title": "Review", "goal": "Preserve numbers"},
        "nodes": {"entity.x": {"kind": "entity", "description": "Value", "fields": {}}},
        "edges": [],
    }


def write(path, data):
    path.write_text(json.dumps(data), encoding="utf-8")


def codes(report):
    return {d["code"] for c in report["checks"] for d in c["diagnostics"]}


def encoded(token, form):
    if form == "json":
        return '{"value":' + token + "}"
    if form == "yaml":
        return "value: " + token + "\n"
    if form == "tag":
        return "value: !!float " + token + "\n"
    return "%TAG !num! tag:yaml.org,2002:\n---\nvalue: !num!float " + token + "\n"


@pytest.mark.parametrize("token", UNSUPPORTED)
@pytest.mark.parametrize("form", ["json", "yaml", "tag", "handle"])
def test_unsupported_finite_precision_is_not_rounded(token, form):
    with pytest.raises(InputError) as error:
        parse(encoded(token, form))
    assert error.value.code == "FORMAT_UNSUPPORTED"


@pytest.mark.parametrize("token", ["0.1", "1e-300", "1.5", "-0.0", "1e300"])
@pytest.mark.parametrize("form", ["json", "yaml", "tag", "handle"])
def test_roundtrippable_floats_stay_builtin_and_canonical(token, form):
    value = parse(encoded(token, form))["value"]
    assert type(value) is float
    assert value == float(token)
    assert json.loads(canonical(value)) == value


@pytest.mark.parametrize(
    "text",
    ['{"value": NaN}', '{"value": Infinity}', "value: .nan", "value: !!float .inf"],
)
def test_nonfinite_literals_remain_forbidden(text):
    with pytest.raises(InputError) as error:
        parse(text)
    assert error.value.code == "YAML_FEATURE_FORBIDDEN"


@pytest.mark.parametrize("form", ["json", "yaml"])
def test_large_exact_integers_stay_builtin(form):
    value = parse(encoded("9007199254740993", form))["value"]
    assert type(value) is int
    assert value == 9007199254740993


@pytest.mark.parametrize("form", ["json", "yaml", "tag"])
def test_runtime_integer_capacity_is_unknown(form):
    limit = sys.get_int_max_str_digits()
    if not limit:
        pytest.skip("This runtime has no integer conversion limit")
    token = "1" * (limit + 1)
    text = encoded(token, form) if form != "tag" else "value: !!int " + token
    with pytest.raises(InputError) as error:
        parse(text)
    assert error.value.code == "FORMAT_UNSUPPORTED"


@pytest.mark.parametrize("token", UNSUPPORTED)
def test_primary_unsupported_number_has_valid_unknown_report(tmp_path, token):
    data = model()
    data["nodes"]["entity.x"]["fields"] = {
        "v": {"type": "number", "minimum": "TOKEN", "maximum": 0}
    }
    path = tmp_path / "model.json"
    path.write_text(json.dumps(data).replace('"TOKEN"', token))
    value = validate(path)
    assert aggregate(value["checks"], "model") == "UNKNOWN"
    assert exit_code(value) == 2
    assert "FORMAT_UNSUPPORTED" in codes(value)
    assert schema_checks(value, "report.json")[0]["result"] == "PASS"
    assert load_bundle(path).revision is None


@pytest.mark.parametrize(
    "minimum,maximum,result",
    [
        ("0.1", "0.1", "PASS"),
        ("1e-300", "1e-300", "PASS"),
        ("1e-299", "1e-300", "FAIL"),
    ],
)
def test_supported_bounds_keep_real_semantic_validation(
    tmp_path, minimum, maximum, result
):
    data = model()
    data["nodes"]["entity.x"]["fields"] = {
        "v": {"type": "number", "minimum": "MIN", "maximum": "MAX"}
    }
    path = tmp_path / "model.json"
    path.write_text(
        json.dumps(data).replace('"MIN"', minimum).replace('"MAX"', maximum)
    )
    assert aggregate(validate(path)["checks"], "model") == result


@pytest.mark.parametrize("kind", ["fragment", "records", "extension"])
def test_unsupported_secondary_bundle_asset_stays_unknown(tmp_path, kind):
    data = model()
    if kind == "fragment":
        data["includes"] = ["fragment.json"]
        asset = {
            "format": "gdd-fragment/0.2",
            "nodes": {
                "entity.y": {
                    "kind": "entity",
                    "description": "Value",
                    "annotations": {"example.number": "TOKEN"},
                }
            },
            "edges": [],
        }
    elif kind == "records":
        data["record_files"] = ["records.json"]
        # Parsing fails before structural record schema can be established.
        asset = {
            "format": "gdd-records/0.2",
            "project_id": "review",
            "records": {},
            "unknown": "TOKEN",
        }
    else:
        asset = {
            "format": "gdd-extension/0.2",
            "namespace": "example.test",
            "version": "1.0.0",
            "node_kinds": {},
            "relations": {},
            "unknown": "TOKEN",
        }
    raw = json.dumps(asset).replace('"TOKEN"', "1e-4000").encode()
    (tmp_path / (kind + ".json")).write_bytes(raw)
    if kind == "extension":
        data["extensions"] = {
            "extension.test": {
                "namespace": "example.test",
                "version": "1.0.0",
                "path": "extension.json",
                "sha256": sha256(raw).hexdigest(),
            }
        }
    path = tmp_path / "model.json"
    write(path, data)
    value = validate(path)
    assert aggregate(value["checks"], "model") == "UNKNOWN"
    assert "FORMAT_UNSUPPORTED" in codes(value)
    assert (load_bundle(path).revision is not None) == (kind == "records")


@pytest.mark.parametrize("format", ["json", "yaml"])
@pytest.mark.parametrize(
    "token,result",
    [
        ("0.1", "PASS"),
        ("1e-300", "PASS"),
        ("1e-4000", "UNKNOWN"),
        ("0.10000000000000001", "UNKNOWN"),
    ],
)
def test_contract_source_precision_is_scoped_unknown(tmp_path, format, token, result):
    raw = encoded(token, format).encode()
    (tmp_path / "contract.txt").write_bytes(raw)
    data = model()
    data["sources"] = {
        "source.contract": {
            "role": "knowledge",
            "title": "Contract",
            "format": format,
            "uri": "contract.txt",
            "normative": True,
            "revision": {"sha256": sha256(raw).hexdigest()},
        }
    }
    data["nodes"]["entity.x"]["source_refs"] = [
        {"source": "source.contract", "purpose": "contract"}
    ]
    path = tmp_path / "model.json"
    write(path, data)
    value = validate(path, profiles=["readiness"], scope=["entity.x"])
    assert aggregate(value["checks"], "model") == "PASS"
    assert aggregate(value["checks"], "readiness") == result
    if result == "UNKNOWN":
        assert "FORMAT_UNSUPPORTED" in codes(value)


@pytest.mark.parametrize(
    "kind,result",
    [
        ("unsupported", "UNKNOWN"),
        ("missing", "UNKNOWN"),
        ("wrong", "FAIL"),
        ("valid", "PASS"),
    ],
)
def test_explicit_secondary_input_has_no_fabricated_format_failure(
    tmp_path, kind, result
):
    path = tmp_path / "model.json"
    write(path, model())
    secondary = tmp_path / "secondary.json"
    if kind == "unsupported":
        secondary.write_text(
            json.dumps(model()).replace(
                '"fields": {}',
                '"fields": {"v": {"type": "number", "minimum": 1e-4000}}',
            )
        )
    elif kind == "wrong":
        write(secondary, model())
    elif kind == "valid":
        write(
            secondary,
            {"format": "gdd-records/0.2", "project_id": "review", "records": {}},
        )
    value = validate(secondary, model_path=path, profiles=["representation"])
    assert aggregate(value["checks"], "model") == result
    if kind in {"unsupported", "missing"}:
        assert "SCHEMA_INVALID" not in codes(value)


@pytest.mark.parametrize(
    "option", ["host-context", "package", "parallel-plan", "import-audit"]
)
def test_cli_explicit_numeric_unsupported_input_exits_two(tmp_path, option):
    path = tmp_path / "model.json"
    write(path, model())
    supplied = tmp_path / "option.json"
    supplied.write_text('{"value": 1e-4000}')
    output = tmp_path / "report.json"
    process = subprocess.run(
        [
            sys.executable,
            "-B",
            "-m",
            "gdd_validator",
            "validate",
            str(path),
            "--" + option,
            str(supplied),
            "--output",
            str(output),
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    assert process.returncode == 2
    assert "gdd:" in process.stderr
    assert not output.exists()


def test_cli_proposal_numeric_unsupported_mapping_creates_no_artifact(tmp_path):
    mapping = tmp_path / "mapping.json"
    mapping.write_text('{"value": 1e-4000}')
    output = tmp_path / "proposal.json"
    process = subprocess.run(
        [
            sys.executable,
            "-B",
            "-m",
            "gdd_validator",
            "graphify",
            "propose",
            str(tmp_path / "observations"),
            "--mapping",
            str(mapping),
            "--output",
            str(output),
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    assert process.returncode == 2
    assert not output.exists()
