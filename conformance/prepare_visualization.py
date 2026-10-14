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


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="New fixture directory")
    prepare(parser.parse_args().output)
