"""Generate browser fixtures into a new directory, without clobbering files."""

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from conformance.viewer_fixture import fixture, write_fixture
from gdd_validator import render_view


def prepare(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=False)
    path = write_fixture(directory / "fixture.gdd.json")
    render_view(path, directory / "fixture.html")
    long_title = fixture()
    long_title["project"]["title"] = "A" * 500
    long_title_path = directory / "long-title.gdd.json"
    long_title_path.write_text(json.dumps(long_title), encoding="utf-8")
    render_view(long_title_path, directory / "long-title.html")
    references = fixture()
    selector = {
        "kind": "markdown-heading",
        "value": "Title",
        "ordinal": 9007199254740993,
    }
    ref = {"source": "source.test", "purpose": "provenance", "selector": selector}
    references["nodes"]["constructor"]["source_refs"] = [ref]
    references["nodes"]["screen.references"] = {
        "kind": "screen",
        "description": "Inspect exact design references",
        "design_refs": [{**ref, "purpose": "contract"}],
    }
    references["edges"][0]["source_refs"] = [ref]
    references_path = directory / "references.gdd.json"
    references_path.write_text(json.dumps(references), encoding="utf-8")
    render_view(references_path, directory / "references.html")
    duplicate = fixture()
    duplicate["edges"].append(
        {
            "id": "edge.member_again",
            "from": "action.second",
            "relation": "member_of",
            "to": "community.frontend",
        }
    )
    duplicate_path = directory / "duplicate.gdd.json"
    duplicate_path.write_text(json.dumps(duplicate), encoding="utf-8")
    render_view(duplicate_path, directory / "duplicate.html")
    for name, model, scope in [
        ("scoped", "concert", ["action.reserve"]),
        ("extension", "extension", []),
        ("concert", "concert", []),
        ("concert-0.3", "concert-0.3", []),
    ]:
        render_view(
            ROOT / "examples" / f"{model}.gdd.yaml",
            directory / f"{name}.html",
            scope=scope,
        )
    empty = fixture()
    empty.update(nodes={}, edges=[], sources={})
    path = directory / "empty.gdd.json"
    path.write_text(json.dumps(empty), encoding="utf-8")
    render_view(path, directory / "empty.html")
    # Dense real projections exercise label collisions and community separation.
    clustered = cluster_fixture(8)
    ownership = json.loads(json.dumps(clustered))
    ownership["nodes"] = {
        ident: node
        for ident, node in ownership["nodes"].items()
        if node["kind"] != "community"
    }
    ownership["edges"] = [
        edge for edge in ownership["edges"] if edge["relation"] != "member_of"
    ]
    overlap = json.loads(json.dumps(clustered))
    overlap["edges"].extend(
        [
            {
                "id": "edge.shared.first",
                "from": "action.group0.0",
                "to": "community.group1",
                "relation": "member_of",
            },
            {
                "id": "edge.shared.second",
                "from": "action.group0.0",
                "to": "community.group2",
                "relation": "member_of",
            },
            {
                "id": "edge.shared.other",
                "from": "action.group1.0",
                "to": "community.group0",
                "relation": "member_of",
            },
        ]
    )
    # Exactly 300 nodes, with bounded sparse connectivity rather than a scale claim.
    large = cluster_fixture(98, dense=False)
    for name, model in [
        ("clusters", clustered),
        ("ownership", ownership),
        ("overlap", overlap),
        ("large", large),
    ]:
        path = directory / (name + ".gdd.json")
        path.write_text(json.dumps(model), encoding="utf-8")
        render_view(path, directory / (name + ".html"))


def cluster_fixture(member_count: int, *, dense: bool = True) -> dict:
    model = {
        "format": "gdd/0.3",
        "project": {
            "id": "project.clusters",
            "title": "Cluster layout",
            "goal": "Keep groups readable without changing graph identity",
        },
        "nodes": {},
        "edges": [],
    }
    for group in range(3):
        community = f"community.group{group}"
        component = f"component.group{group}"
        model["nodes"][community] = {
            "kind": "community",
            "label": f"Group {group}",
            "description": "Declared community",
        }
        model["nodes"][component] = {
            "kind": "component",
            "label": f"Group {group} service",
            "description": "Owns the group actions",
        }
        model["edges"].append(
            {
                "id": f"edge.component{group}.member",
                "from": component,
                "to": community,
                "relation": "member_of",
            }
        )
        for index in range(member_count):
            ident = f"action.group{group}.{index}"
            model["nodes"][ident] = {
                "kind": "action",
                "label": f"Group {group} operation {index}",
                "description": "Connected action",
            }
            model["edges"].extend(
                [
                    {
                        "id": f"edge.{group}.{index}.member",
                        "from": ident,
                        "to": community,
                        "relation": "member_of",
                    },
                    {
                        "id": f"edge.{group}.{index}.owner",
                        "from": ident,
                        "to": component,
                        "relation": "owned_by",
                    },
                ]
            )
            neighbors = range(index) if dense else range(max(0, index - 2), index)
            for other in neighbors:
                model["edges"].append(
                    {
                        "id": f"edge.{group}.{index}.{other}",
                        "from": ident,
                        "to": f"action.group{group}.{other}",
                        "relation": "calls",
                    }
                )
    for group in range(3):
        model["edges"].append(
            {
                "id": f"edge.bridge.{group}",
                "from": f"action.group{group}.0",
                "to": f"action.group{(group + 1) % 3}.0",
                "relation": "calls",
            }
        )
    return model


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="New fixture directory")
    prepare(parser.parse_args().output)
