"""Declared reverse dependencies are distinct from supporting read context."""
from copy import deepcopy
import json
from pathlib import Path

from jsonschema import Draft202012Validator
import pytest

from gdd_validator.impact import analyze_impact, impact_text
from gdd_validator.loading import load_bundle
from gdd_validator.readiness import context_closure, context_paths

ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture
def model():
    value = json.loads((ROOT / "docs/tutorial/refined.gdd.json").read_text())
    action = deepcopy(value["nodes"]["action.add"])
    for name in ("caller", "outer", "sibling"):
        value["nodes"]["action." + name] = deepcopy(action)
    value["edges"] += [
        {"id": "edge.caller", "from": "action.caller", "to": "action.add", "relation": "calls"},
        {"id": "edge.outer", "from": "action.outer", "to": "action.caller", "relation": "calls"},
        {"id": "edge.sibling", "from": "action.sibling", "to": "component.arithmetic", "relation": "owned_by"},
    ]
    return value


def write(tmp_path, model):
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model))
    return path


def items(result, root=0):
    return {item["id"]: item for item in result["roots"][root]["items"]}


def test_reverse_paths_and_supporting_owner(tmp_path, model):
    path = write(tmp_path, model)
    result = analyze_impact(path, scope=["action.add"])
    found = items(result)
    assert result["exit_code"] == 0
    assert found["action.caller"]["category"] == "direct"
    assert found["action.outer"]["category"] == "transitive"
    assert [step["edge"] for step in found["action.outer"]["witness"]] == ["edge.caller", "edge.outer"]
    assert all(step["orientation"] == "reverse" for step in found["action.outer"]["witness"])
    assert found["component.arithmetic"]["category"] == "supporting"
    assert found["scenario.add"]["category"] == "supporting"
    assert "action.sibling" not in found
    assert items(analyze_impact(path, scope=["action.caller"]))["action.add"]["category"] == "supporting"


def test_cycles_and_multiple_roots_are_deterministic(tmp_path, model):
    model["edges"].append({"id": "edge.cycle", "from": "action.add", "to": "action.outer", "relation": "calls"})
    path = write(tmp_path, model)
    result = analyze_impact(path, scope=["action.outer", "action.add", "action.add"])
    assert result == analyze_impact(path, scope=["action.add", "action.outer"])
    assert len(result["roots"]) == 2
    assert all(items(result, n)[root["id"]]["category"] == "selected" for n, root in enumerate(result["roots"]))
    assert max(len(item["witness"]) for root in result["roots"] for item in root["items"]) < 8


@pytest.mark.parametrize("scope", [[], ["unknown.id"], ["action.add\n"], "action.add", [42], ["edge.caller"]])
def test_invalid_scopes_visible(tmp_path, model, scope):
    result = analyze_impact(write(tmp_path, model), scope=scope)
    assert result["exit_code"] == 1 and result["diagnostics"]
    assert result["coverage"]["result"] != "complete"


def test_mixed_scope_retains_valid_root(tmp_path, model):
    result = analyze_impact(write(tmp_path, model), scope=["action.add", "unknown.id"])
    assert result["exit_code"] == 1
    assert result["roots"][0]["id"] == "action.add"


def test_read_only_text_and_schema(tmp_path, model):
    path = write(tmp_path, model)
    before = path.read_bytes()
    result = analyze_impact(path, scope=["action.add"])
    assert path.read_bytes() == before
    assert set(tmp_path.iterdir()) == {path}
    text = impact_text(result)
    assert text.endswith("\n") and "runtime callers unassessed" in text
    assert all(item["id"] in text for item in result["roots"][0]["items"])
    Draft202012Validator(json.loads((ROOT / "schemas/impact.schema.json").read_text())).validate(result)


def test_context_paths_match_existing_closure(tmp_path, model):
    model["edges"].append({"id": "edge.refine", "from": "action.sibling", "to": "action.outer", "relation": "refines"})
    bundle = load_bundle(write(tmp_path, model))
    for seed in model["nodes"]:
        assert set(context_paths(bundle, [seed])) == context_closure(bundle, [seed])


def test_unsupported_and_malformed_inputs(tmp_path):
    path = tmp_path / "bad.json"
    path.write_text('{"format": "gdd/future"}')
    result = analyze_impact(path, scope=["action.add"])
    assert result["exit_code"] in {1, 2} and result["coverage"]["result"] == "unavailable"
    path.write_text('{')
    assert analyze_impact(path, scope=["action.add"])["diagnostics"]


@pytest.mark.parametrize("dependency", [False, True])
def test_extension_context_does_not_invent_dependency(tmp_path, model, dependency):
    from hashlib import sha256
    extension = {"format": "gdd-extension/0.2", "namespace": "example.links", "version": "1.0.0",
        "node_kinds": {}, "relations": {"example.links.related": {
            "description": "Explicit read context with separately declared dependency.",
            "from": ["action"], "to": ["action"], "context": "both", "dependency": dependency,
            "effect": "none", "data_schema": {"type": "object", "additionalProperties": False}}}}
    raw = json.dumps(extension).encode()
    (tmp_path / "extension.json").write_bytes(raw)
    model["extensions"] = {"extension.links": {"namespace": "example.links", "version": "1.0.0",
                                              "path": "extension.json", "sha256": sha256(raw).hexdigest()}}
    model["edges"] = [edge for edge in model["edges"] if edge["id"] != "edge.caller"]
    model["edges"].append({"id": "edge.custom", "from": "action.caller", "to": "action.add",
                           "relation": "example.links.related", "data": {}})
    result = analyze_impact(write(tmp_path, model), scope=["action.add"])
    assert result["exit_code"] == 0, result["diagnostics"]
    item = items(result)["action.caller"]
    assert item["category"] == ("direct" if dependency else "supporting")
    assert item["witness"][0]["edge"] == "edge.custom"


def test_unknown_extension_is_visible_partial_semantics(tmp_path, model):
    model["extensions"] = {"extension.links": {"namespace": "example.links", "version": "1.0.0",
                                              "path": "missing.json", "sha256": "0" * 64}}
    result = analyze_impact(write(tmp_path, model), scope=["action.add"])
    assert result["exit_code"] == 2 and result["coverage"]["result"] != "complete"


def test_current_records_selected_history_never_rebound(tmp_path, model):
    from tests.test_code_check import binding
    model["record_files"] = ["records.json"]
    path = write(tmp_path, model)
    records_path = tmp_path / "records.json"
    records = {"format": "gdd-records/0.2", "project_id": model["project"]["id"], "records": {}}
    records_path.write_text(json.dumps(records))
    revision = load_bundle(path).revision
    records["records"] = {
        "binding.current": binding(revision, "action.add", "module.py"),
        "binding.history": binding("sha256:" + "0" * 64, "action.add", "old.py"),
        "check.current": {"kind": "check", "scope": ["scenario.add"], "spec_revision": revision,
                          "method": "unit_test", "required_environment": [], "negative_controls": []}}
    records_path.write_text(json.dumps(records))
    result = analyze_impact(path, scope=["action.add"], implementation=records["records"]["binding.current"]["implementation"])
    assert result["exit_code"] == 0, result["diagnostics"]
    assert {record["id"] for record in result["records"]} == {"binding.current", "check.current"}
    assert result["records_omitted"][0]["id"] == "binding.history"
    assert all(record["spec_revision"] == revision for record in result["records"])


def test_type_reference_consumers_have_semantic_witness(tmp_path, model):
    model["nodes"]["entity.input"] = {"kind": "entity", "description": "Input", "fields": {}}
    model["nodes"]["action.add"]["contract"]["input"]["left"] = {"type": "reference", "target": "entity.input"}
    result = analyze_impact(write(tmp_path, model), scope=["entity.input"])
    assert result["exit_code"] == 0, result["diagnostics"]
    assert items(result)["action.add"]["witness"][0]["derivation"] == "type-reference-consumer"


@pytest.mark.parametrize("relation", ["reads", "creates", "changes"])
def test_entity_consumers_reverse_only(tmp_path, model, relation):
    model["nodes"]["entity.shared"] = {"kind": "entity", "description": "Shared entity", "fields": {}}
    model["edges"].append({"id": "edge.data", "from": "action.add", "to": "entity.shared", "relation": relation})
    path = write(tmp_path, model)
    result = analyze_impact(path, scope=["entity.shared"])
    assert result["exit_code"] == 0, result["diagnostics"]
    assert items(result)["action.add"]["category"] == "direct"
    reverse = analyze_impact(path, scope=["action.add"])
    assert items(reverse)["entity.shared"]["category"] == "supporting"


def test_navigation_consumer_reverse_only(tmp_path, model):
    for name in ("entry", "destination"):
        model["nodes"]["screen." + name] = {"kind": "screen", "description": name}
    model["edges"].append({"id": "edge.navigation", "from": "screen.entry", "to": "screen.destination", "relation": "navigates_to"})
    path = write(tmp_path, model)
    assert items(analyze_impact(path, scope=["screen.destination"]))["screen.entry"]["category"] == "direct"
    assert items(analyze_impact(path, scope=["screen.entry"]))["screen.destination"]["category"] == "supporting"


def test_explicitly_incomplete_dependencies_partial(tmp_path, model):
    model["nodes"]["action.caller"]["contract"]["dependencies_complete"] = False
    result = analyze_impact(write(tmp_path, model), scope=["action.add"])
    assert result["exit_code"] == 2
    assert result["coverage"]["result"] == "partial"
    assert any(item["code"] == "IMPACT_DEPENDENCIES_INCOMPLETE" for item in result["diagnostics"])
    assert items(result)["action.caller"]["category"] == "direct"


def test_missing_completeness_does_not_require_readiness(tmp_path, model):
    model["nodes"]["action.add"].pop("contract")
    result = analyze_impact(write(tmp_path, model), scope=["action.add"])
    assert result["exit_code"] == 0
    assert "action.add" in result["coverage"]["unspecified_declarations"]
    assert result["coverage"]["unlisted_dependencies"] == "unassessed"
