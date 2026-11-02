"""Independent permutation and canonical-witness controls for product projections."""
from hashlib import sha256
import json
from pathlib import Path
import random
import shutil

import pytest

from gdd_validator import load_perspective_bundle, perspective_data
from gdd_validator.visualization import view_data

ROOT = Path(__file__).resolve().parents[1]


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")


def fixture(tmp_path):
    shutil.copytree(ROOT / "examples/product/extensions", tmp_path / "extensions")
    sensor = ROOT / "examples/extensions/sensors.extension.json"
    shutil.copyfile(sensor, tmp_path / "extensions/sensors.json")
    model = json.loads((ROOT / "examples/product/greenfield.gdd.json").read_text())
    model["extensions"]["extension.sensor"] = {
        "namespace": "example.sensors", "version": "0.1.0",
        "path": "extensions/sensors.json", "sha256": sha256(sensor.read_bytes()).hexdigest()}
    renames = {"actor.guest": "constructor", "component.booking": "prototype"}
    model["nodes"] = {renames.get(i, i): n for i, n in model["nodes"].items()}
    for edge in model["edges"]:
        for end in ("from", "to"):
            edge[end] = renames.get(edge[end], edge[end])
    model["nodes"]["constructor"]["data"]["goal"] = "Å漢字 👩🏽‍💻 e\u0301 \u2028\u2029 </script>"
    model["nodes"]["entity.performance"]["fields"]["capacity"]["minimum"] = 9007199254740993
    model["nodes"]["sensor.room"] = {
        "kind": "example.sensors.sensor", "description": "Temperature concept.",
        "data": {"unit": "°C 漢字"}, "annotations": {"review.exact": {"large": 9007199254740993}}}
    model["nodes"]["step.incomplete"] = {
        "kind": "gdd.product.journey_step", "description": "An intentionally partial step.", "data": {}}
    model["nodes"]["entity.unassigned"] = {"kind": "entity", "description": "An unassigned entity."}
    model["edges"].extend([
        {"id": "edge.sensor", "from": "sensor.room", "relation": "example.sensors.measures", "to": "entity.performance", "data": {}},
        {"id": "edge.usage.second", "from": "prototype", "relation": "gdd.product.uses", "to": "entity.reservation", "data": {}},
        {"id": "edge.read", "from": "action.reserve", "relation": "reads", "to": "entity.reservation"},
        {"id": "edge.change", "from": "action.reserve", "relation": "changes", "to": "entity.reservation"},
        {"id": "edge.entity.owner", "from": "entity.reservation", "relation": "owned_by", "to": "prototype"},
        {"id": "edge.entity.storage", "from": "entity.reservation", "relation": "stored_in", "to": "prototype"},
        {"id": "edge.next.parallel", "from": "step.choose", "relation": "gdd.product.next", "to": "step.reserve", "data": {}},
        {"id": "edge.next.reverse", "from": "step.reserve", "relation": "gdd.product.next", "to": "step.choose", "data": {}},
    ])
    fragment = {"format": "gdd-fragment/0.3", "nodes": {}, "edges": []}
    for ident in ("constructor", "sensor.room", "step.incomplete"):
        fragment["nodes"][ident] = model["nodes"].pop(ident)
    fragment["edges"] = [model["edges"].pop()]
    model["includes"] = ["slice.json"]
    path = tmp_path / "model.json"
    write(path, model)
    write(tmp_path / "slice.json", fragment)
    return path, model, fragment


def test_fragment_identity_direction_multiedges_and_distinct_witnesses(tmp_path):
    path, model, fragment = fixture(tmp_path)
    bundle = load_perspective_bundle(path)
    result = perspective_data(bundle)
    nodes = model["nodes"] | fragment["nodes"]
    edges = {e["id"]: e for e in model["edges"] + fragment["edges"]}
    expected_primary = {
        "journey": {"constructor", "step.choose", "step.reserve", "step.incomplete"},
        "components": {"prototype", "component.reservations", "action.reserve"},
        "data": {"entity.performance", "entity.reservation", "entity.unassigned", "rule.capacity"}}
    for name, view in result["views"].items():
        assert set(view["primary_node_ids"]) == expected_primary[name]
        for registry, universe in (("node_ids", nodes), ("edge_ids", edges)):
            shown, omitted = map(set, (view["shown"][registry], view["omitted"][registry]))
            assert shown.isdisjoint(omitted)
            assert shown | omitted == set(universe)
        assert view["nodes"] == {i: nodes[i] for i in view["shown"]["node_ids"]}
        assert view["edges"] == [edges[i] for i in view["shown"]["edge_ids"]]
    assert "sensor.room" in result["views"]["data"]["shown"]["node_ids"]
    assert "sensor.room" in result["views"]["journey"]["omitted"]["node_ids"]
    assert result["unsupported_extensions"] == ["example.sensors"]
    # Both steps are contextual neighbors in Components; their ordering edges
    # belong to Journey and must not leak through the shared endpoint records.
    ordering = {"edge.choose.reserve", "edge.next.parallel", "edge.next.reverse"}
    assert ordering <= set(result["views"]["journey"]["shown"]["edge_ids"])
    assert {"step.choose", "step.reserve"} <= set(result["views"]["components"]["shown"]["node_ids"])
    assert ordering <= set(result["views"]["components"]["omitted"]["edge_ids"])
    assert "edge.booking.reservation" in result["views"]["journey"]["omitted"]["edge_ids"]
    for name, view in result["views"].items():
        for link in view["cross_links"]:
            assert set(link["views"]) == {
                other for other, primary in expected_primary.items()
                if other != name and link["node_id"] in primary}
            assert link["edge_ids"] == sorted(
                edge["id"] for edge in view["edges"]
                if link["node_id"] in (edge["from"], edge["to"]))
    # All seven authored routes between the same pair retain distinct witnesses.
    pair = [link for link in result["component_data_links"]
            if (link["component"], link["entity"]) == ("prototype", "entity.reservation")]
    assert {(link["via"], tuple(link["edge_ids"])) for link in pair} == {
        ("uses", ("edge.booking.reservation",)), ("uses", ("edge.usage.second",)),
        ("owned_by", ("edge.entity.owner",)), ("stored_in", ("edge.entity.storage",)),
        ("reads", ("edge.read", "edge.reserve.owner")),
        ("creates", ("edge.reserve.creates", "edge.reserve.owner")),
        ("changes", ("edge.change", "edge.reserve.owner"))}
    assert len(pair) == 7
    assert {g["node_id"] for g in result["gaps"]} == {"step.incomplete", "entity.unassigned"}
    # Browser inspector strings preserve integers that JavaScript cannot represent.
    payload = view_data(bundle)
    assert "9007199254740993" in payload["text"]["nodes"]["entity.performance"]["full"]
    assert json.loads(payload["text"]["nodes"]["sensor.room"]["full"]) == nodes["sensor.room"]
    result["views"]["data"]["nodes"]["sensor.room"]["annotations"]["review.exact"]["large"] = 0
    assert perspective_data(bundle)["views"]["data"]["nodes"]["sensor.room"] == nodes["sensor.room"]


@pytest.mark.parametrize("seed", range(6))
def test_byte_order_changes_revision_without_semantic_changes(tmp_path, seed):
    path, model, fragment = fixture(tmp_path)
    before_bundle = load_perspective_bundle(path)
    before = perspective_data(before_bundle)
    randomizer = random.Random(seed)
    for document in (model, fragment):
        for key in ("nodes", "extensions"):
            if key in document:
                items = list(document[key].items())
                randomizer.shuffle(items)
                document[key] = dict(items)
        randomizer.shuffle(document["edges"])
    write(path, model)
    write(tmp_path / "slice.json", fragment)
    after_bundle = load_perspective_bundle(path)
    after = perspective_data(after_bundle)
    assert before_bundle.revision != after_bundle.revision
    assert before.pop("spec_revision") != after.pop("spec_revision")
    assert after == before


@pytest.mark.parametrize("command", ["perspectives", "view"])
@pytest.mark.parametrize("destination", ["manifest", "symlink", "extension"])
def test_cli_cannot_overwrite_source_or_asset_alias(tmp_path, command, destination):
    import subprocess
    import sys

    path, _, _ = fixture(tmp_path)
    asset = tmp_path / "extensions/product-0.1.json"
    before = {p: p.read_bytes() for p in (path, asset, tmp_path / "slice.json")}
    output = path if destination == "manifest" else asset
    if destination == "symlink":
        output = tmp_path / "alias.json"
        output.symlink_to(path)
    process = subprocess.run(
        [sys.executable, "-m", "gdd_validator", command, str(path), "--output", str(output)],
        capture_output=True, text=True)
    assert process.returncode == 1 and not process.stdout, process.stderr
    assert "Traceback" not in process.stderr
    assert {p: p.read_bytes() for p in before} == before
    if destination == "symlink":
        assert output.is_symlink() and output.resolve() == path


@pytest.mark.parametrize("command", ["perspectives", "view"])
def test_semantically_equal_product_bytes_are_explicitly_unsupported(tmp_path, command):
    import subprocess
    import sys

    path, model, _ = fixture(tmp_path)
    asset = tmp_path / "extensions/product-0.1.json"
    original = asset.read_bytes()
    reformatted = json.dumps(json.loads(original), sort_keys=True).encode()
    assert reformatted != original and json.loads(reformatted) == json.loads(original)
    asset.write_bytes(reformatted)
    model["extensions"]["extension.product"]["sha256"] = sha256(reformatted).hexdigest()
    write(path, model)
    output = tmp_path / "new-output.json"
    process = subprocess.run(
        [sys.executable, "-m", "gdd_validator", command, str(path), "--output", str(output)],
        capture_output=True, text=True)
    assert process.returncode == 2 and not process.stdout
    assert "Unsupported gdd.product semantics" in process.stderr
    assert "Traceback" not in process.stderr and not output.exists()
    assert asset.read_bytes() == reformatted
