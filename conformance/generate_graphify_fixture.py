"""Run with the installed Graphify 0.9.75 interpreter, not a runtime dependency."""

from copy import deepcopy
from hashlib import sha256
from importlib.metadata import version
import json
import os
from pathlib import Path

import graphify
from graphify.build import build_from_json
from graphify.export import to_json
from graphify.extract import extract_python


def generate() -> None:
    if version("graphifyy") != "0.9.75":
        raise ValueError("Fixture generator is pinned to Graphify 0.9.75.")
    root = Path(__file__).resolve().parents[1]
    folder = root / "examples/graphify"
    code_root = folder / "code"
    os.chdir(code_root)
    raw = extract_python(Path("reservation.py"), root=code_root)
    if "error" in raw:
        raise ValueError(raw)
    (folder / "raw.ast.json").write_text(
        json.dumps(raw, ensure_ascii=False, indent=2) + "\n"
    )
    for directed, name in (
        (False, "export.undirected.json"),
        (True, "export.directed.json"),
    ):
        data = deepcopy(raw)
        graph = build_from_json(data, directed=directed)
        communities = {0: list(graph.nodes)}
        assert to_json(
            graph,
            communities,
            str(folder / name),
            force=True,
            original_links=data["edges"],
            community_labels={0: "Reservations"},
        )
        if not directed:
            assert to_json(
                graph, communities, str(folder / "export.collapsed.json"), force=True
            )
    package = Path(graphify.__file__).parent
    files = [
        folder / name
        for name in (
            "raw.ast.json",
            "export.undirected.json",
            "export.directed.json",
            "export.collapsed.json",
        )
    ]
    evidence = {
        "producer": {"name": "graphify", "version": version("graphifyy")},
        "dependencies": {
            n: version(n) for n in ("networkx", "tree-sitter", "tree-sitter-python")
        },
        "input_files": [
            {
                "path": "code/reservation.py",
                "sha256": sha256(
                    (code_root / "reservation.py").read_bytes()
                ).hexdigest(),
            }
        ],
        "code_revision": None,
        "producer_sources": [
            {"path": name, "sha256": sha256((package / name).read_bytes()).hexdigest()}
            for name in ("extract.py", "build.py", "export.py")
        ],
        "artifacts": [
            {
                "path": p.name,
                "sha256": sha256(p.read_bytes()).hexdigest(),
                "nodes": len(json.loads(p.read_text())["nodes"]),
                "edges": len(
                    json.loads(p.read_text()).get(
                        "edges", json.loads(p.read_text()).get("links", [])
                    )
                ),
            }
            for p in files
        ],
        "note": "Actually extracted from the included Python fixture, then exported with the installed producer. No LLM pipeline, code acceptance or invented Git revision.",
    }
    (folder / "producer-evidence.json").write_text(
        json.dumps(evidence, ensure_ascii=False, indent=2) + "\n"
    )
    print(json.dumps(evidence["artifacts"], indent=2))


if __name__ == "__main__":
    generate()
