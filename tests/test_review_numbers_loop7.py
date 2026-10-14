"""Exact decimal numeric semantics across core types and local JSON Schemas."""

from copy import deepcopy
from hashlib import sha256
import json

import pytest

from gdd_validator import validate
from gdd_validator.loading import canonical, load_bundle, parse_json, schema_checks
from gdd_validator.model import validate_model
from gdd_validator.report import aggregate

INTEGER = "100000000000000000000000"
PREVIOUS = "99999999999999999999999"


def base_model():
    return {
        "format": "gdd/0.2",
        "project": {"id": "review", "title": "Review", "goal": "Exact numbers"},
        "nodes": {},
        "edges": [],
    }


def raw_json(value, **tokens):
    text = json.dumps(value)
    for key, raw in tokens.items():
        text = text.replace(json.dumps(key), raw)
    return text


def run_core(tmp_path, spec, **tokens):
    model = base_model()
    model["nodes"]["entity.x"] = {
        "kind": "entity",
        "description": "Value",
        "fields": {"v": spec},
    }
    path = tmp_path / "model.json"
    path.write_text(raw_json(model, **tokens), encoding="utf-8")
    return validate(path)


@pytest.mark.parametrize("kind", ["number", "integer"])
@pytest.mark.parametrize(
    "minimum,maximum,result",
    [
        ("1e23", INTEGER, "PASS"),
        (INTEGER, "1e23", "PASS"),
        ("1e23", PREVIOUS, "FAIL"),
        (PREVIOUS, "1e23", "PASS"),
        ("-1e23", "-" + INTEGER, "PASS"),
        ("-" + INTEGER, "-1e23", "PASS"),
        ("-" + PREVIOUS, "-1e23", "FAIL"),
        ("0.1", "0.2", "NUMBER_ONLY"),
    ],
)
def test_core_mixed_numeric_bounds(tmp_path, kind, minimum, maximum, result):
    report = run_core(
        tmp_path,
        {"type": kind, "minimum": "MIN", "maximum": "MAX"},
        MIN=minimum,
        MAX=maximum,
    )
    if result == "NUMBER_ONLY":
        result = "PASS" if kind == "number" else "FAIL"
    assert aggregate(report["checks"], "model") == result
    assert schema_checks(report, "report.json")[0]["result"] == "PASS"


@pytest.mark.parametrize("optional", [{}, {"optional": True}, {"nullable": True}])
def test_real_contradiction_is_invalid_in_every_type_context(tmp_path, optional):
    report = run_core(
        tmp_path,
        {"type": "integer", "minimum": "MIN", "maximum": "MAX", **optional},
        MIN="1e23",
        MAX=PREVIOUS,
    )
    assert aggregate(report["checks"], "model") == "FAIL"


@pytest.mark.parametrize("optional", [{"optional": True}, {"nullable": True}])
def test_optional_and_nullable_empty_integer_domains_still_allowed(tmp_path, optional):
    report = run_core(
        tmp_path,
        {"type": "integer", "minimum": 0.1, "maximum": 0.2, **optional},
    )
    assert aggregate(report["checks"], "model") == "PASS"


@pytest.mark.parametrize(
    "values,result",
    [
        ("[1e23," + INTEGER + "]", "FAIL"),
        ("[1e23," + PREVIOUS + "]", "PASS"),
        ("[true,1]", "PASS"),
        ("[false,0.0]", "PASS"),
        ("[1,1.0]", "FAIL"),
        ('["100000000000000000000000",1e23]', "PASS"),
    ],
)
def test_core_enum_uses_exact_json_numeric_equality(tmp_path, values, result):
    report = run_core(tmp_path, {"type": "enum", "values": "VALUES"}, VALUES=values)
    assert aggregate(report["checks"], "model") == result


def run_extension(tmp_path, constraint, value, *, schema_tokens=None, mode="inline"):
    definition = {
        "type": "object",
        "properties": {"v": constraint},
        "required": ["v"],
    }
    if mode != "inline":
        definition["properties"]["v"] = {
            "$dynamicRef" if mode == "dynamic" else "$ref": (
                "#number" if mode == "dynamic" else "#/$defs/number"
            )
        }
        definition["$defs"] = {"number": deepcopy(constraint)}
        definition["$defs"]["number"][
            "$schema"
        ] = "https://json-schema.org/draft/2020-12/schema"
        if mode == "dynamic":
            definition["$defs"]["number"]["$dynamicAnchor"] = "number"
    extension = {
        "format": "gdd-extension/0.2",
        "namespace": "example.numeric",
        "version": "1.0.0",
        "node_kinds": {
            "example.numeric.value": {
                "description": "Exact numeric value",
                "data_schema": definition,
            }
        },
        "relations": {},
    }
    raw = raw_json(extension, **(schema_tokens or {})).encode()
    (tmp_path / "extension.json").write_bytes(raw)
    model = base_model()
    model["extensions"] = {
        "extension.numeric": {
            "namespace": "example.numeric",
            "version": "1.0.0",
            "path": "extension.json",
            "sha256": sha256(raw).hexdigest(),
        }
    }
    model["nodes"]["node.x"] = {
        "kind": "example.numeric.value",
        "description": "Value",
        "data": {"v": "VALUE", "integer": int(INTEGER)},
    }
    path = tmp_path / "model.json"
    path.write_text(raw_json(model, VALUE=value), encoding="utf-8")
    return validate(path), path


@pytest.mark.parametrize("mode", ["inline", "ref", "dynamic"])
@pytest.mark.parametrize(
    "keyword,bound,value,result",
    [
        ("minimum", INTEGER, "1e23", "PASS"),
        ("minimum", "1e23", PREVIOUS, "FAIL"),
        ("maximum", PREVIOUS, "1e23", "FAIL"),
        ("maximum", "1e23", INTEGER, "PASS"),
        ("exclusiveMinimum", PREVIOUS, "1e23", "PASS"),
        ("exclusiveMinimum", "1e23", INTEGER, "FAIL"),
        ("exclusiveMaximum", INTEGER, "1e23", "FAIL"),
        ("exclusiveMaximum", "1e23", PREVIOUS, "PASS"),
        ("minimum", "-1e23", "-" + INTEGER, "PASS"),
        ("maximum", "-1e23", "-" + PREVIOUS, "FAIL"),
    ],
)
def test_extension_exact_comparators(tmp_path, mode, keyword, bound, value, result):
    report, _ = run_extension(
        tmp_path,
        {keyword: "BOUND"},
        value,
        schema_tokens={"BOUND": bound},
        mode=mode,
    )
    assert aggregate(report["checks"], "model") == result
    assert schema_checks(report, "report.json")[0]["result"] == "PASS"


@pytest.mark.parametrize("mode", ["inline", "ref"])
@pytest.mark.parametrize(
    "divisor,value,result",
    [
        ("0.1", "0.3", "PASS"),
        ("0.1", "0.31", "FAIL"),
        ("1e-300", "1e300", "PASS"),
        ("0.1", "1" + "0" * 400, "PASS"),
        ("1e23", "2" + "0" * 23, "PASS"),
        ("1e23", "2" + "0" * 22 + "1", "FAIL"),
    ],
)
def test_extension_exact_multiple_of(tmp_path, mode, divisor, value, result):
    report, _ = run_extension(
        tmp_path,
        {"multipleOf": "DIVISOR"},
        value,
        schema_tokens={"DIVISOR": divisor},
        mode=mode,
    )
    assert aggregate(report["checks"], "model") == result


@pytest.mark.parametrize("mode", ["inline", "ref"])
@pytest.mark.parametrize(
    "constraint,value,result",
    [
        ({"const": "NUMBER"}, "1e23", "PASS"),
        ({"const": "NUMBER"}, PREVIOUS, "FAIL"),
        ({"enum": ["NUMBER"]}, "1e23", "PASS"),
        ({"enum": ["NUMBER"]}, PREVIOUS, "FAIL"),
        ({"const": {"x": ["NUMBER"]}}, '{"x":[1e23]}', "PASS"),
        ({"const": True}, "1", "FAIL"),
        ({"enum": [False]}, "0.0", "FAIL"),
        ({"uniqueItems": True}, "[1e23," + INTEGER + "]", "FAIL"),
        ({"uniqueItems": True}, "[1e23," + PREVIOUS + "]", "PASS"),
        ({"uniqueItems": True}, "[true,1]", "PASS"),
        ({"uniqueItems": True}, '[{"x":[1e23]},{"x":[' + INTEGER + "]}]", "FAIL"),
        ({"type": "integer"}, "1e23", "PASS"),
        ({"type": "integer"}, "0.1", "FAIL"),
        ({"type": "number"}, "0.1", "PASS"),
    ],
)
def test_extension_exact_equality_and_integer_type(
    tmp_path, mode, constraint, value, result
):
    report, _ = run_extension(
        tmp_path, constraint, value, schema_tokens={"NUMBER": INTEGER}, mode=mode
    )
    assert aggregate(report["checks"], "model") == result


@pytest.mark.parametrize(
    "constraint",
    [{"multipleOf": 0}, {"multipleOf": -0.1}, {"maxLength": 0.1}],
)
def test_extension_metaschema_still_rejects_invalid_numbers(tmp_path, constraint):
    report, _ = run_extension(tmp_path, constraint, '"text"')
    assert aggregate(report["checks"], "model") == "FAIL"
    assert any(
        d["code"] == "EXTENSION_DATA_INVALID"
        for c in report["checks"]
        for d in c["diagnostics"]
    )


def test_numeric_validation_never_changes_model_assets_or_canonical_bytes(tmp_path):
    report, path = run_extension(
        tmp_path,
        {"const": "NUMBER"},
        "1e23",
        schema_tokens={"NUMBER": INTEGER},
    )
    assert aggregate(report["checks"], "model") == "PASS"
    bundle = load_bundle(path)
    before = (
        canonical(bundle.model),
        canonical(bundle.extensions),
        deepcopy(bundle.assets),
        deepcopy(bundle.snapshot),
    )
    data = bundle.model["nodes"]["node.x"]["data"]
    assert type(data["v"]) is float
    assert type(data["integer"]) is int
    validate_model(bundle)
    assert before == (
        canonical(bundle.model),
        canonical(bundle.extensions),
        bundle.assets,
        bundle.snapshot,
    )
    assert type(data["v"]) is float
    assert type(data["integer"]) is int
    assert parse_json(path.read_bytes())["nodes"]["node.x"]["data"]["v"] == 1e23


def test_fragment_enum_and_yaml_have_same_numeric_semantics(tmp_path):
    fragment = (
        "format: gdd-fragment/0.2\n"
        "nodes:\n"
        "  entity.x:\n"
        "    kind: entity\n"
        "    description: Value\n"
        "    fields:\n"
        "      v:\n"
        "        type: enum\n"
        "        values: [!!float 1e23, " + INTEGER + "]\n"
        "edges: []\n"
    )
    (tmp_path / "fragment.yaml").write_text(fragment)
    model = base_model()
    model["includes"] = ["fragment.yaml"]
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model))
    assert aggregate(validate(path)["checks"], "model") == "FAIL"
