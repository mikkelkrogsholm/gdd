"""Product perspectives preserve canonical identity without evidence claims."""
from hashlib import sha256
import json
from pathlib import Path
import shutil
import subprocess
import sys

from jsonschema import Draft202012Validator
import pytest

from gdd_validator import perspectives, perspective_data, load_perspective_bundle
from gdd_validator.loading import InputError, schema

ROOT = Path(__file__).resolve().parents[1]
EXAMPLES = ROOT / "examples/product"


def copied(tmp_path, name="greenfield"):
    shutil.copytree(EXAMPLES, tmp_path / "product")
    return tmp_path / "product" / f"{name}.gdd.json"


def change(path, update):
    model = json.loads(path.read_text())
    update(model)
    path.write_text(json.dumps(model, indent=2) + "\n")


def test_connected_views_and_canonical_witnesses():
    result = perspectives(EXAMPLES / "greenfield.gdd.json")
    Draft202012Validator(schema("perspectives")).validate(result)
    assert result["advisory"] is True
    assert not result["gaps"]
    assert result["product_extension"]["version"] == "0.1.0"
    model = json.loads((EXAMPLES / "greenfield.gdd.json").read_text())
    all_nodes = set(model["nodes"])
    all_edges = {e["id"] for e in model["edges"]}
    canonical_edges = {e["id"]: e for e in model["edges"]}
    for name, view in result["views"].items():
        assert set(view["shown"]["node_ids"]) | set(view["omitted"]["node_ids"]) == all_nodes
        assert set(view["shown"]["node_ids"]) & set(view["omitted"]["node_ids"]) == set()
        assert set(view["shown"]["edge_ids"]) | set(view["omitted"]["edge_ids"]) == all_edges
        assert all(n == model["nodes"][ident] for ident, n in view["nodes"].items())
        assert all(edge == canonical_edges[edge["id"]] for edge in view["edges"])
        assert view["cross_links"]
    assert {"component": "component.booking", "entity": "entity.reservation", "via": "creates",
            "edge_ids": ["edge.reserve.creates", "edge.reserve.owner"]} in result["component_data_links"]
    assert {"component": "component.booking", "entity": "entity.reservation", "via": "uses",
            "edge_ids": ["edge.booking.reservation"]} in result["component_data_links"]
    assert "PASS" not in json.dumps(result)


def test_refactoring_keeps_journey_and_rule_with_changed_assignment():
    before = perspectives(EXAMPLES / "greenfield.gdd.json")
    after = perspectives(EXAMPLES / "refactoring.gdd.json")
    assert before["spec_revision"] != after["spec_revision"]
    assert before["views"]["journey"]["primary_node_ids"] == after["views"]["journey"]["primary_node_ids"]
    for ident in before["views"]["journey"]["primary_node_ids"]:
        assert before["views"]["journey"]["nodes"][ident] == after["views"]["journey"]["nodes"][ident]
    assert before["views"]["data"]["nodes"]["rule.capacity"] == after["views"]["data"]["nodes"]["rule.capacity"]
    assert any(link["component"] == "component.reservations" and link["entity"] == "entity.reservation"
               for link in after["component_data_links"])


def test_partial_is_valid_and_advisory():
    result = perspectives(EXAMPLES / "partial.gdd.json")
    assert {g["code"] for g in result["gaps"]} == {
        "MISSING_ACTOR", "MISSING_USER_ACTION", "MISSING_SYSTEM_RESPONSE",
        "MISSING_RESPONSIBLE_COMPONENT", "MISSING_DATA_COMPONENT"}
    Draft202012Validator(schema("perspectives")).validate(result)


@pytest.mark.parametrize("format", ["gdd/0.2", "gdd/0.3"])
def test_plain_core_and_unbound_global_rule(tmp_path, format):
    model = {"format": format, "project": {"id": "example.core", "title": "Core", "goal": "Inspect data."},
             "nodes": {"entity.item": {"kind": "entity", "description": "An item."},
                       "component.store": {"kind": "component", "description": "Owns items."},
                       "rule.valid": {"kind": "rule", "description": "Valid data.",
                                      "rule": {"category": "invariant", "scope": "global", "statement": "Items must be valid.",
                                               "blocking": False, "verification": ["Review."]}}},
             "edges": [{"id": "edge.owner", "from": "entity.item", "relation": "owned_by", "to": "component.store"}]}
    path = tmp_path / "core.json"
    path.write_text(json.dumps(model))
    result = perspectives(path)
    assert result["views"]["journey"]["nodes"] == {}
    assert result["product_extension"] is None
    assert result["component_data_links"][0]["edge_ids"] == ["edge.owner"]
    assert [g["code"] for g in result["gaps"]] == ["MISSING_RULE_TARGET"]


@pytest.mark.parametrize("mutation", ["endpoint", "reference", "hash", "blank", "cardinality"])
def test_invalid_bundle_never_projects(tmp_path, mutation):
    path = copied(tmp_path)
    def update(model):
        if mutation == "endpoint": model["edges"][0]["from"] = "entity.reservation"
        if mutation == "reference": model["edges"][0]["from"] = "actor.missing"
        if mutation == "hash": model["extensions"]["extension.product"]["sha256"] = "0" * 64
        if mutation == "blank": model["nodes"]["step.reserve"]["data"]["user_action"] = ""
        if mutation == "cardinality": model["edges"][-1]["data"]["to_cardinality"] = "many"
    change(path, update)
    with pytest.raises(ValueError, match="valid"):
        perspectives(path)


@pytest.mark.parametrize("mutation", ["version", "definition"])
def test_spoofed_product_definition_is_explicitly_unsupported(tmp_path, mutation):
    path = copied(tmp_path)
    asset = path.parent / "extensions/product-0.1.json"
    definition = json.loads(asset.read_text())
    if mutation == "version": definition["version"] = "0.2.0"
    if mutation == "definition": definition["relations"]["gdd.product.next"]["dependency"] = True
    raw = json.dumps(definition).encode()
    asset.write_bytes(raw)
    def update(model):
        declaration = model["extensions"]["extension.product"]
        declaration.update(namespace=definition["namespace"], version=definition["version"], sha256=sha256(raw).hexdigest())
    change(path, update)
    with pytest.raises(InputError, match="Unsupported gdd.product semantics") as error:
        perspectives(path)
    assert error.value.code == "FORMAT_UNSUPPORTED"
    command = subprocess.run([sys.executable, "-m", "gdd_validator", "perspectives", str(path)], capture_output=True, text=True)
    assert command.returncode == 2
    assert not command.stdout


def test_other_extensions_are_explicit_and_not_product():
    result = perspectives(ROOT / "examples/extension.gdd.yaml")
    assert result["unsupported_extensions"] == ["example.sensors"]
    assert result["views"]["journey"]["nodes"] == {}
    assert result["product_extension"] is None


def test_branch_cycle_multiple_relations_and_cardinalities(tmp_path):
    path = copied(tmp_path)
    def update(model):
        model["edges"].extend([
            {"id": "edge.loop", "from": "step.reserve", "to": "step.choose", "relation": "gdd.product.next", "data": {}},
            {"id": "edge.branch", "from": "step.choose", "to": "step.choose", "relation": "gdd.product.next", "data": {}},
            {"id": "edge.collaborator", "from": "step.reserve", "to": "component.reservations", "relation": "gdd.product.realized_by", "data": {}},
            {"id": "edge.related.again", "from": "entity.reservation", "to": "entity.performance", "relation": "gdd.product.relates_to",
             "data": {"from_cardinality": "1..*", "to_cardinality": "0..1"}}])
    change(path, update)
    result = perspectives(path)
    assert {"edge.loop", "edge.branch", "edge.collaborator"} <= set(result["views"]["journey"]["shown"]["edge_ids"])
    assert {"edge.reservation.performance", "edge.related.again"} <= set(result["views"]["data"]["shown"]["edge_ids"])


def test_pure_projection_does_not_read_model_paths_or_alias_input(tmp_path):
    path = copied(tmp_path)
    bundle = load_perspective_bundle(path)
    expected = perspective_data(bundle)
    path.unlink()
    projected = perspective_data(bundle)
    assert projected == expected
    projected["views"]["journey"]["nodes"]["step.reserve"]["data"]["user_action"] = "Changed."
    assert perspective_data(bundle) == expected


def test_cli_is_deterministic_and_output_is_exclusive(tmp_path):
    args = [sys.executable, "-m", "gdd_validator", "perspectives", str(EXAMPLES / "greenfield.gdd.json")]
    first = subprocess.run(args, capture_output=True, text=True)
    second = subprocess.run(args, capture_output=True, text=True)
    assert first.returncode == second.returncode == 0
    assert first.stdout == second.stdout
    assert not first.stderr
    output = tmp_path / "projection.json"
    written = subprocess.run(args + ["--output", str(output)], capture_output=True, text=True)
    assert written.returncode == 0 and not written.stdout
    assert output.read_text() == first.stdout
    refused = subprocess.run(args + ["--output", str(output)], capture_output=True, text=True)
    assert refused.returncode == 1
    assert output.read_text() == first.stdout


def test_extension_copy_is_byte_exact():
    assert (EXAMPLES / "extensions/product-0.1.json").read_bytes() == (ROOT / "gdd_validator/extensions/product-0.1.json").read_bytes()


@pytest.mark.parametrize("format", ["gdd/0.2", "gdd/0.3"])
def test_product_extension_on_both_core_formats(tmp_path, format):
    from gdd_validator import validate, exit_code

    path = copied(tmp_path)
    change(path, lambda model: model.update(format=format))
    assert exit_code(validate(path, profiles=["model"])) == 0
    result = perspectives(path)
    assert result["views"]["journey"]["nodes"]["actor.guest"]["kind"] == "gdd.product.actor"
    assert "step.reserve" in result["views"]["journey"]["primary_node_ids"]
    assert result["views"]["data"]["nodes"]["rule.capacity"]["rule"]["scope"] == "targeted"
    assert "edge.capacity.reservation" in result["views"]["data"]["shown"]["edge_ids"]
    assert any(link["edge_ids"] == ["edge.reserve.creates", "edge.reserve.owner"]
               for link in result["component_data_links"])
    assert not result["gaps"]
