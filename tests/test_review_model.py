"""Regression cases from the first ordinary/adversarial model review."""

from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path
import shutil

import pytest

from gdd_validator import validate
from gdd_validator.loading import Bundle, load_bundle
from gdd_validator.model import type_references, validate_model
from gdd_validator.readiness import context_closure
from gdd_validator.report import aggregate

ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture
def examples(tmp_path):
    directory = tmp_path / "examples"
    shutil.copytree(ROOT / "examples", directory)
    return directory


def write_model(examples, operation):
    original = load_bundle(examples / "concert.gdd.yaml").model
    model = deepcopy(original)
    model.pop("record_files", None)
    operation(model)
    path = examples / "review.gdd.json"
    path.write_text(json.dumps(model, ensure_ascii=False), encoding="utf-8")
    return path


def diagnostics(report):
    return {d["code"] for c in report["checks"] for d in c["diagnostics"]}


@pytest.mark.parametrize(
    "metadata",
    [
        {"type": "reference"},
        {"type": "reference", "target": "entity.unrelated"},
        {"type": "reference", "target": ["not", "an", "ID"]},
    ],
)
def test_annotations_cannot_supply_type_references(examples, metadata):
    path = write_model(
        examples,
        lambda m: m["nodes"]["action.reserve"].update(
            annotations={"example.metadata": metadata}
        ),
    )
    report = validate(path, profiles=["readiness"], scope=["action.reserve"])
    assert aggregate(report["checks"], "readiness") == "PASS"
    assert "entity.unrelated" not in context_closure(
        load_bundle(path), ["action.reserve"]
    )


def test_type_reference_traversal_uses_only_type_positions():
    spec = {
        "type": "object",
        "properties": {
            "reservation": {
                "type": "array",
                "items": {"type": "reference", "target": "entity.reservation"},
            }
        },
        # Arbitrary metadata is not a typespec, even if an external caller supplies it.
        "default": {"type": "reference", "target": "entity.not_a_dependency"},
        "annotations": {"type": "reference"},
    }
    assert type_references(spec) == {"entity.reservation"}


@pytest.mark.parametrize("support", ["rule.capacity", "scenario.reserve"])
def test_support_refinement_descendants_remain_read_context(examples, support):
    path = write_model(
        examples,
        lambda m: m["edges"].append(
            {
                "id": "edge.support.refines",
                "from": support,
                "relation": "refines",
                "to": "action.reserve",
            }
        ),
    )
    report = validate(path, profiles=["readiness"], scope=["action.reserve"])
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == "PASS"
    assert support in context_closure(load_bundle(path), ["action.reserve"])
    direct = validate(path, profiles=["readiness"], scope=[support])
    assert aggregate(direct["checks"], "readiness") == "FAIL"


def parallel_plan(bundle):
    return {
        "format": "gdd-parallel-plan/0.2",
        "packages": [
            {
                "id": f"package.{i}",
                "scope": [scope],
                "write_resources": [f"src/{i}.py"],
                "read_resources": ["src/contracts.py"],
                "locked_contracts": ["action.reserve", "action.cancel"],
                "workspace": f"workspace.{i}",
                "base_revision": bundle.revision,
                "integration_plan": "Integrate with the declared shared-rule checks.",
            }
            for i, scope in enumerate(("action.reserve", "action.cancel"))
        ],
        "serialized": [],
        "integration": {
            "owner": "actor.integration",
            "checks": [i for i, r in bundle.records.items() if r["kind"] == "check"],
        },
    }


@pytest.mark.parametrize(
    "left,right,result",
    [
        ("src/[ab].py", "src/a.py", "UNKNOWN"),
        ("src/*.py", "src/a.py", "UNKNOWN"),
        ("src/", "src/a.py", "FAIL"),
        ("src/.", "src/a.py", "FAIL"),
        ("src/dir/..", "src/a.py", "FAIL"),
        ("src//.", "src/a.py", "FAIL"),
        ("src/dir/..", "src-other/a.py", "PASS"),
        ("src/a/", "src/a/nested/file.py", "FAIL"),
        ("./", "src/a.py", "FAIL"),
        ("src/a/", "src/ab.py", "PASS"),
        ("src/a/", "src/b/", "PASS"),
        ("src/a.py::symbol", "src/a.py", "UNKNOWN"),
        ("src\\a.py", "src/a.py", "UNKNOWN"),
        ("../outside.py", "src/a.py", "UNKNOWN"),
    ],
)
def test_parallel_posix_resource_ownership(left, right, result):
    path = ROOT / "examples/concert.gdd.yaml"
    plan = parallel_plan(load_bundle(path))
    plan["packages"][0]["write_resources"] = [left]
    plan["packages"][1]["write_resources"] = [right]
    report = validate(path, profiles=["readiness"], parallel_plan=plan)
    assert aggregate(report["checks"], "readiness") == result


@pytest.mark.parametrize("directory", ["src/", "src/.", "src/dir/.."])
def test_serialization_allows_known_directory_overlap(directory):
    path = ROOT / "examples/concert.gdd.yaml"
    plan = parallel_plan(load_bundle(path))
    plan["packages"][0]["write_resources"] = [directory]
    plan["packages"][1]["write_resources"] = ["src/a.py"]
    plan["serialized"] = [["package.0", "package.1"]]
    assert (
        aggregate(
            validate(path, profiles=["readiness"], parallel_plan=plan)["checks"],
            "readiness",
        )
        == "PASS"
    )


@pytest.mark.parametrize("alias", ["workspace.shared/.", "temp/../workspace.shared"])
def test_parallel_workspace_aliases_conflict(alias):
    path = ROOT / "examples/concert.gdd.yaml"
    plan = parallel_plan(load_bundle(path))
    plan["packages"][0]["workspace"] = "workspace.shared"
    plan["packages"][1]["workspace"] = alias
    report = validate(path, profiles=["readiness"], parallel_plan=plan)
    assert "WRITE_OWNERSHIP_CONFLICT" in diagnostics(report)


def test_parallel_absolute_and_relative_workspace_aliases_conflict():
    path = ROOT / "examples/concert.gdd.yaml"
    bundle = load_bundle(path)
    plan = parallel_plan(bundle)
    plan["packages"][0]["workspace"] = "workspace.shared"
    plan["packages"][1]["workspace"] = str(bundle.root / "workspace.shared")
    report = validate(path, profiles=["readiness"], parallel_plan=plan)
    assert "WRITE_OWNERSHIP_CONFLICT" in diagnostics(report)


@pytest.mark.parametrize(
    "raw,format,selector,result",
    [
        (
            b"requirements: do X\n",
            "application/json",
            {"kind": "json-pointer", "value": "/requirements"},
            "UNKNOWN",
        ),
        (b"{", "json", {"kind": "whole-document"}, "UNKNOWN"),
        (b"\xff\xfe\x80", "text/plain", {"kind": "whole-document"}, "UNKNOWN"),
        (
            b'{"requirements":"do X"}',
            "application/json",
            {"kind": "json-pointer", "value": "/requirements"},
            "PASS",
        ),
        (
            b"requirements: do X\n",
            "application/yaml",
            {"kind": "json-pointer", "value": "/requirements"},
            "PASS",
        ),
        (
            "Læs kravet\u2028præcist".encode("utf-8"),
            "text/plain",
            {"kind": "whole-document"},
            "PASS",
        ),
    ],
)
def test_contract_source_format_and_encoding(examples, raw, format, selector, result):
    (examples / "contract.asset").write_bytes(raw)

    def add_source(model):
        model.setdefault("sources", {})["source.contract"] = {
            "role": "knowledge",
            "title": "Contract input",
            "uri": "contract.asset",
            "format": format,
            "normative": True,
            "revision": {"sha256": sha256(raw).hexdigest()},
        }
        model["nodes"]["action.reserve"].setdefault("source_refs", []).append(
            {"source": "source.contract", "purpose": "contract", "selector": selector}
        )

    report = validate(
        write_model(examples, add_source),
        profiles=["readiness"],
        scope=["action.reserve"],
    )
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == result


@pytest.mark.parametrize(
    "extra,result",
    [({}, "FAIL"), ({"nullable": True}, "PASS"), ({"optional": True}, "PASS")],
)
def test_integer_interval_respects_null_and_omission(extra, result):
    model = {
        "format": "gdd/0.2",
        "project": {
            "id": "example.interval",
            "title": "Interval",
            "goal": "Check domain consistency",
        },
        "nodes": {
            "entity.value": {
                "kind": "entity",
                "description": "Domain value",
                "fields": {
                    "x": {"type": "integer", "minimum": 0.2, "maximum": 0.8, **extra}
                },
            }
        },
        "edges": [],
    }
    bundle = Bundle(ROOT / "interval.fixture.json", model=model)
    validate_model(bundle)
    assert aggregate(bundle.checks, "model") == result


def test_integer_interval_with_one_integer_is_valid():
    model = {
        "format": "gdd/0.2",
        "project": {
            "id": "example.interval",
            "title": "Interval",
            "goal": "Check domain consistency",
        },
        "nodes": {
            "entity.value": {
                "kind": "entity",
                "description": "Value",
                "fields": {"x": {"type": "integer", "minimum": 0.2, "maximum": 1.2}},
            }
        },
        "edges": [],
    }
    bundle = Bundle(ROOT / "interval.fixture.json", model=model)
    validate_model(bundle)
    assert aggregate(bundle.checks, "model") == "PASS"
