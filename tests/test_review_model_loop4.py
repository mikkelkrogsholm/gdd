"""Resolved YAML tags and context-sensitive inhabited types."""

from hashlib import sha256
import json

import pytest

from gdd_validator import validate
from gdd_validator.loading import InputError, parse
from gdd_validator.report import aggregate


EMPTY_INTEGER = {"type": "integer", "minimum": 0.2, "maximum": 0.8}


def object_type(child, *, required=True, **extra):
    return {
        "type": "object",
        "properties": {"value": child},
        **({"required": ["value"]} if required else {}),
        **extra,
    }


def model_with_type(spec, position="fields"):
    model = {
        "format": "gdd/0.2",
        "project": {"id": "review", "title": "Review", "goal": "Type domains"},
        "nodes": {},
        "edges": [],
    }
    if position == "fields":
        model["nodes"]["entity.x"] = {
            "kind": "entity",
            "description": "Value",
            "fields": {"value": spec},
        }
    else:
        contract = {position: spec if position == "output" else {"value": spec}}
        model["nodes"]["action.x"] = {
            "kind": "action",
            "description": "Action",
            "contract": contract,
        }
    return model


def validate_model(tmp_path, spec, position="fields"):
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model_with_type(spec, position)), encoding="utf-8")
    return validate(path)


@pytest.mark.parametrize("position", ["fields", "input", "context", "output"])
@pytest.mark.parametrize(
    "spec,result",
    [
        (EMPTY_INTEGER, "FAIL"),
        ({**EMPTY_INTEGER, "nullable": True}, "PASS"),
        (object_type(EMPTY_INTEGER, required=False), "PASS"),
        (object_type(EMPTY_INTEGER), "FAIL"),
        (object_type(EMPTY_INTEGER, nullable=True), "PASS"),
        (object_type(object_type(EMPTY_INTEGER)), "FAIL"),
        (object_type(object_type(EMPTY_INTEGER), required=False), "PASS"),
        (object_type(object_type(EMPTY_INTEGER, required=False)), "PASS"),
        ({"type": "array", "items": EMPTY_INTEGER}, "PASS"),
        ({"type": "array", "items": object_type(EMPTY_INTEGER)}, "PASS"),
        (object_type({"type": "array", "items": EMPTY_INTEGER}), "PASS"),
    ],
)
def test_integer_domain_requires_an_unavoidable_value(tmp_path, position, spec, result):
    assert (
        aggregate(validate_model(tmp_path, spec, position)["checks"], "model") == result
    )


@pytest.mark.parametrize("position", ["fields", "input", "context"])
def test_optional_field_can_omit_an_otherwise_uninhabited_object(tmp_path, position):
    spec = object_type(EMPTY_INTEGER, optional=True)
    assert (
        aggregate(validate_model(tmp_path, spec, position)["checks"], "model") == "PASS"
    )


@pytest.mark.parametrize(
    "wrapper",
    [
        lambda child: object_type(child, required=False),
        lambda child: object_type(child, nullable=True),
        lambda child: {"type": "array", "items": child},
    ],
)
@pytest.mark.parametrize(
    "child,code",
    [
        ({"type": "integer", "minimum": 3, "maximum": 2}, "TYPE_CONSTRAINT_INVALID"),
        ({"type": "reference", "target": "entity.missing"}, "REFERENCE_UNRESOLVED"),
        ({"type": "integer", "optional": False}, "TYPE_CONSTRAINT_INVALID"),
        (
            {"type": "object", "properties": {}, "required": ["missing"]},
            "TYPE_CONSTRAINT_INVALID",
        ),
    ],
)
def test_avoidable_values_still_have_structural_and_reference_checks(
    tmp_path, wrapper, child, code
):
    report = validate_model(tmp_path, wrapper(child))
    assert aggregate(report["checks"], "model") == "FAIL"
    assert code in {d["code"] for c in report["checks"] for d in c["diagnostics"]}


@pytest.mark.parametrize(
    "tag,value",
    [
        ("str", "text"),
        ("int", "2"),
        ("float", "2.5"),
        ("bool", "true"),
        ("null", "null"),
        ("seq", "[one, two]"),
        ("map", "{key: value}"),
    ],
)
def test_rebound_double_exclamation_is_not_necessarily_a_builtin(tag, value):
    raw = f"%TAG !! tag:example.org,2026:custom-\n---\nvalue: !!{tag} {value}\n"
    with pytest.raises(InputError, match="built-in tags"):
        parse(raw)


@pytest.mark.parametrize(
    "directive,tag",
    [
        ("", "!!"),
        ("%TAG !! tag:yaml.org,2002:\n", "!!"),
        ("%TAG !b! tag:yaml.org,2002:\n", "!b!"),
        ("%TAG ! tag:yaml.org,2002:\n", "!"),
    ],
)
def test_actual_builtin_tags_survive_handle_aliases(directive, tag):
    raw = (
        directive
        + "---\n"
        + "\n".join(
            [
                f"s: {tag}str 2",
                f"i: {tag}int 2",
                f"f: {tag}float 2.5",
                f"b: {tag}bool true",
                f"n: {tag}null null",
                f"a: {tag}seq [one, two]",
                f"o: {tag}map {{key: value}}",
            ]
        )
    )
    assert parse(raw) == {
        "s": "2",
        "i": 2,
        "f": 2.5,
        "b": True,
        "n": None,
        "a": ["one", "two"],
        "o": {"key": "value"},
    }


def test_verbatim_builtin_tag_is_supported():
    assert parse("value: !<tag:yaml.org,2002:int> 2") == {"value": 2}


def test_yaml_json_scalars_do_not_change_when_tags_are_checked():
    raw = 'a: true\nb: false\nc: null\nd: 2\ne: 2.5\nf: "2"\ng: yes\nh: 2026-01-01\n'
    assert parse(raw) == {
        "a": True,
        "b": False,
        "c": None,
        "d": 2,
        "e": 2.5,
        "f": "2",
        "g": "yes",
        "h": "2026-01-01",
    }


def test_rebound_tag_cannot_make_a_model_representation_pass(tmp_path):
    raw = "%TAG !! tag:example.org,2026:custom-\n---\nformat: !!str gdd/0.2\nproject: {id: p, title: P, goal: P}\nnodes: {}\nedges: []\n"
    path = tmp_path / "model.yaml"
    path.write_text(raw, encoding="utf-8")
    report = validate(path, profiles=["representation"])
    assert aggregate(report["checks"], "model") == "FAIL"
    assert "YAML_FEATURE_FORBIDDEN" in {
        d["code"] for c in report["checks"] for d in c["diagnostics"]
    }


@pytest.mark.parametrize(
    "raw,result",
    [
        (
            b"%TAG !! tag:example.org,2026:custom-\n---\nvalue: !!str requirement\n",
            "UNKNOWN",
        ),
        (b"%TAG !s! tag:yaml.org,2002:\n---\nvalue: !s!str requirement\n", "PASS"),
    ],
)
def test_contract_yaml_uses_the_same_resolved_tag_policy(tmp_path, raw, result):
    (tmp_path / "contract.yaml").write_bytes(raw)
    model = model_with_type({"type": "string"})
    model["sources"] = {
        "source.contract": {
            "role": "knowledge",
            "title": "Contract",
            "uri": "contract.yaml",
            "format": "yaml",
            "normative": True,
            "revision": {"sha256": sha256(raw).hexdigest()},
        }
    }
    model["nodes"]["entity.x"]["source_refs"] = [
        {
            "source": "source.contract",
            "purpose": "contract",
            "selector": {"kind": "json-pointer", "value": "/value"},
        }
    ]
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model), encoding="utf-8")
    report = validate(path, profiles=["readiness"], scope=["entity.x"])
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == result
