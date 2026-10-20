"""Stable-ID differences are exact, conservative under partial loads and advisory."""
from copy import deepcopy
import json
from pathlib import Path

from jsonschema import Draft202012Validator
import pytest

from gdd_validator.model_diff import diff_models, diff_text
from scripts.concert_archive import restore

ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture
def model():
    return json.loads((ROOT / "docs/tutorial/refined.gdd.json").read_text())


def pair(tmp_path, base, head):
    paths = [tmp_path / "base.json", tmp_path / "head.json"]
    for path, value in zip(paths, (base, head)):
        path.write_text(json.dumps(value))
    return paths


def compare(tmp_path, model, change):
    head = deepcopy(model)
    change(head)
    return diff_models(*pair(tmp_path, model, head))


def codes(result):
    return {item["code"] for item in result["advisories"]}


def test_identity_reformat_and_edge_reorder(tmp_path, model):
    paths = pair(tmp_path, model, model)
    paths[1].write_text(json.dumps(model, indent=4, sort_keys=True))
    result = diff_models(*paths)
    assert result["exit_code"] == 0
    assert result["base"]["spec_revision"] != result["head"]["spec_revision"]
    assert not result["comparison"]["semantic_changes"]
    model["edges"].reverse()
    paths[1].write_text(json.dumps(model))
    assert not diff_models(*paths)["comparison"]["semantic_changes"]


def test_fragment_relocation_is_nonsemantic(tmp_path, model):
    paths = pair(tmp_path, model, model)
    moved = deepcopy(model)
    fragment = {"format": "gdd-fragment/0.3", "nodes": moved.pop("nodes"), "edges": moved.pop("edges")}
    moved.update(nodes={}, edges=[], includes=["part.json"])
    (tmp_path / "part.json").write_text(json.dumps(fragment))
    paths[1].write_text(json.dumps(moved))
    result = diff_models(*paths)
    assert result["exit_code"] == 0
    assert not result["changes"] and not result["metadata_changes"]
    assert result["relocated"]


def test_exact_fields_prose_types_and_escaped_pointer(tmp_path, model):
    def change(value):
        value["nodes"]["action.add"]["behavior"] += "\nReturn exactly."
        value["nodes"]["action.add"]["contract"]["input"]["left"]["type"] = "number"
        value["nodes"]["action.add"]["contract"]["input"]["a/b~c"] = {"type": "integer"}
    result = compare(tmp_path, model, change)
    fields = result["changes"][0]["fields"]
    assert {item["pointer"] for item in fields} == {"/behavior", "/contract/input/left/type", "/contract/input/a~1b~0c"}
    assert result["exit_code"] == 0 and not result["advisories"]
    assert "Return exactly." in diff_text(result)


def test_registry_add_remove_change(tmp_path, model):
    def change(value):
        value["nodes"]["action.extra"] = deepcopy(value["nodes"]["action.add"])
        value["nodes"].pop("scenario.add")
        value["edges"] = [edge for edge in value["edges"] if edge["from"] != "scenario.add"]
        value["questions"] = {"question.next": {"statement": "Should output be transported?", "status": "open", "blocks": ["implementation"], "targets": ["action.add"]}}
        value["project"]["goal"] += " Review transport separately."
    result = compare(tmp_path, model, change)
    assert {(c["registry"], c["id"], c["change"]) for c in result["changes"]} >= {("nodes", "action.extra", "added"), ("nodes", "scenario.add", "removed"), ("questions", "question.next", "added")}
    assert result["metadata_changes"][0]["pointer"] == "/project/goal"


def rule_model(model):
    model["nodes"]["rule.boundary"] = {"kind": "rule", "description": "No import.", "rule": {"category": "architecture", "statement": "No dependency.", "scope": "targeted", "blocking": True, "verification": ["static_analysis"], "constraint": {"relation": "depends_on", "from": "component.arithmetic", "to": "action.add", "reach": "transitive", "policy": "forbid"}}}
    model["edges"].append({"id": "edge.verifier", "from": "scenario.add", "to": "rule.boundary", "relation": "verifies"})
    return model


@pytest.mark.parametrize("mutation,expected", [
    (lambda m: m["nodes"]["rule.boundary"]["rule"].update(blocking=False), "BLOCKING_RULE_RELAXED"),
    (lambda m: m["nodes"].pop("rule.boundary"), "BLOCKING_RULE_RELAXED"),
    (lambda m: m["nodes"]["rule.boundary"]["rule"]["constraint"].update(reach="direct"), "FORBID_REACH_NARROWED"),
    (lambda m: m["nodes"]["rule.boundary"]["rule"]["constraint"].update(to="component.arithmetic"), "FORBID_ENDPOINT_CHANGED"),
    (lambda m: m.update(edges=[e for e in m["edges"] if e["id"] != "edge.verifier"]), "LAST_VERIFIER_REMOVED"),
    (lambda m: m["nodes"]["component.arithmetic"].update(interfaces=[]), "INTERFACE_SHRUNK"),
    (lambda m: m["nodes"].update({"action.add": {"kind": "component", "description": "An incompatible concept."}}), "IDENTITY_REVIEW"),
])
def test_structural_advisories_positive_and_noop_negative(tmp_path, model, mutation, expected):
    rule_model(model)
    assert not codes(compare(tmp_path, model, lambda m: None))
    assert expected in codes(compare(tmp_path, model, mutation))


@pytest.mark.parametrize("field,value", [("errors", {"Unavailable": "Return an unavailable error."}), ("preconditions", ["Inputs are integers."]), ("postconditions", ["Result equals left plus right."])])
def test_contract_removal_advisory_and_addition_negative(tmp_path, model, field, value):
    model["nodes"]["action.add"]["contract"][field] = value
    result = compare(tmp_path, model, lambda m: m["nodes"]["action.add"]["contract"].update({field: {} if field == "errors" else []}))
    assert "CONTRACT_REQUIREMENT_REMOVED" in codes(result)
    model["nodes"]["action.add"]["contract"][field] = {} if field == "errors" else []
    assert "CONTRACT_REQUIREMENT_REMOVED" not in codes(compare(tmp_path, model, lambda m: m["nodes"]["action.add"]["contract"].update({field: value})))


def test_retained_verifier_and_expanding_forbid_not_warned(tmp_path, model):
    rule_model(model)
    model["edges"].append({"id": "edge.other_verifier", "from": "scenario.add", "to": "rule.boundary", "relation": "verifies"})
    model["nodes"]["rule.boundary"]["rule"]["constraint"]["reach"] = "direct"
    result = compare(tmp_path, model, lambda m: (m.update(edges=[e for e in m["edges"] if e["id"] != "edge.verifier"]), m["nodes"]["rule.boundary"]["rule"]["constraint"].update(reach="transitive")))
    assert not codes(result)


def test_partial_inputs_do_not_fabricate_absence(tmp_path, model):
    head = deepcopy(model)
    head["includes"] = ["missing.json"]
    head["nodes"].pop("scenario.add")
    head["nodes"]["action.add"]["behavior"] += " Exact text."
    result = diff_models(*pair(tmp_path, model, head))
    assert result["comparison"]["coverage"] == "partial"
    assert result["exit_code"] in {1, 2}
    assert all(item["change"] == "changed" for item in result["changes"])
    assert not result["advisories"] and not result["metadata_changes"]
    assert "absence claims" in diff_text(result)


def test_unknown_and_invalid_inputs_visible(tmp_path, model):
    valid, _ = pair(tmp_path, model, model)
    missing = diff_models(valid, tmp_path / "missing.json")
    assert missing["exit_code"] == 2 and not missing["changes"]
    malformed = tmp_path / "bad.json"
    malformed.write_text('{"format":"gdd/0.3"}')
    result = diff_models(valid, malformed)
    assert result["exit_code"] == 1 and not result["changes"]


@pytest.mark.parametrize("invalid", [False, True])
def test_semantic_unknown_is_partial_and_does_not_establish_absence(tmp_path, model, invalid):
    source = {"role": "input", "title": "Source", "uri": "unused.txt",
              "format": "text/plain", "normative": False}
    model["sources"] = {"source.parent": source,
                        "source.derived": {**source, "derived_from": [{"source": "source.parent"}]}}
    model["nodes"]["component.unused"] = deepcopy(model["nodes"]["component.arithmetic"])
    model["nodes"]["component.unused"]["interfaces"] = []
    head = deepcopy(model)
    head["nodes"].pop("component.unused")
    head["nodes"]["action.add"]["behavior"] += " Exact change."
    if invalid:
        head["edges"].append({"id": "edge.bad", "from": "action.add",
                              "to": "action.absent", "relation": "calls"})
    result = diff_models(*pair(tmp_path, model, head))
    assert result["exit_code"] == (1 if invalid else 2)
    assert result["comparison"]["coverage"] == "partial"
    assert any(diag["code"] == "REVISION_UNRESOLVED"
               for item in result["diagnostics"] for diag in item["diagnostics"])
    assert [item["id"] for item in result["changes"]] == ["action.add"]
    assert result["changes"][0]["change"] == "changed"
    assert not result["advisories"]
    Draft202012Validator(json.loads((ROOT / "schemas/model-diff.schema.json").read_text())).validate(result)


def test_known_invalid_semantics_still_show_exact_loaded_change(tmp_path, model):
    result = compare(tmp_path, model, lambda m: m["edges"].append({"id": "edge.bad", "from": "action.add", "to": "action.missing", "relation": "calls"}))
    assert result["exit_code"] == 1
    assert any(item["id"] == "edge.bad" and item["change"] == "added" for item in result["changes"])


def test_legacy_to_current_format(tmp_path):
    legacy = ROOT / "archive/gdd-0.2/examples/concert.gdd.yaml"
    current = ROOT / "examples/concert-0.3.gdd.yaml"
    result = diff_models(legacy, current)
    assert result["base"]["format"] == "gdd/0.2" and result["head"]["format"] == "gdd/0.3"
    assert any(item["pointer"] == "/format" for item in result["metadata_changes"])


def test_schema_determinism_and_read_only(tmp_path, model):
    paths = pair(tmp_path, model, model)
    original = {path: path.read_bytes() for path in paths}
    result = diff_models(*paths)
    assert result == diff_models(*paths)
    assert original == {path: path.read_bytes() for path in paths}
    Draft202012Validator(json.loads((ROOT / "schemas/model-diff.schema.json").read_text())).validate(result)
    assert diff_text(result).endswith("\n")


def test_archived_requirement_change_exposes_limit(tmp_path):
    restore(ROOT / "archive/experiments/concert-7e27757.tar.gz",
            ROOT / "archive/experiments/concert-7e27757.index.json", tmp_path.resolve() / "restored")
    project = tmp_path / "restored/examples/concert-app"
    result = diff_models(project / "history/phase-a/gdd.json", project / "gdd.json")
    assert result["comparison"]["coverage"] == "complete"
    text = diff_text(result)
    assert "LimitReached" in text and "principal" in text
    assert result["comparison"]["semantic_changes"]


def test_source_role_change_has_identity_review(tmp_path, model):
    model["sources"] = {"source.context": {"role": "input", "title": "Context", "uri": "context.txt", "format": "text/plain", "normative": False}}
    result = compare(tmp_path, model, lambda m: m["sources"]["source.context"].update(role="knowledge"))
    assert result["exit_code"] == 0
    assert "IDENTITY_REVIEW" in codes(result)
    assert "historical ID reuse" in result["advisories"][0]["message"]


def test_extension_semantics_not_asset_packaging(tmp_path, model):
    from hashlib import sha256
    extension = {"format": "gdd-extension/0.2", "namespace": "example.links", "version": "1.0.0", "node_kinds": {}, "relations": {"example.links.related": {"description": "Declared relation.", "from": ["action"], "to": ["action"], "context": "both", "dependency": False, "effect": "none", "data_schema": {"type": "object"}}}}
    paths = []
    for name, indent in (("base", None), ("head", 2)):
        directory = tmp_path / name
        directory.mkdir()
        raw = json.dumps(extension, indent=indent).encode()
        (directory / (name + ".json")).write_bytes(raw)
        value = deepcopy(model)
        value["extensions"] = {"extension.links": {"namespace": "example.links", "version": "1.0.0", "path": name + ".json", "sha256": sha256(raw).hexdigest()}}
        manifest = directory / "gdd.json"
        manifest.write_text(json.dumps(value))
        paths.append(manifest)
    result = diff_models(*paths)
    assert result["exit_code"] == 0 and not result["comparison"]["semantic_changes"]
    extension["relations"]["example.links.related"]["dependency"] = True
    raw = json.dumps(extension).encode()
    (paths[1].parent / "head.json").write_bytes(raw)
    value = json.loads(paths[1].read_text())
    value["extensions"]["extension.links"]["sha256"] = sha256(raw).hexdigest()
    paths[1].write_text(json.dumps(value))
    result = diff_models(*paths)
    assert result["exit_code"] == 0 and result["comparison"]["semantic_changes"]
    assert result["metadata_changes"][0]["pointer"].endswith("/dependency")


def test_result_schema_covers_partial_invalid_and_changed(tmp_path, model):
    validator = Draft202012Validator(json.loads((ROOT / "schemas/model-diff.schema.json").read_text()))
    paths = pair(tmp_path, model, model)
    for result in (diff_models(paths[0], tmp_path / "missing.json"),
                   compare(tmp_path, model, lambda m: m["nodes"]["action.add"].update(behavior="Return exact sum."))):
        validator.validate(result)
    paths[1].write_text('{"format":"gdd/0.3"}')
    validator.validate(diff_models(*paths))
