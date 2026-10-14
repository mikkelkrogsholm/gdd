"""Small hostile directed multigraph for renderer and real browser checks."""

import json
from pathlib import Path

HOSTILE = '</script><script>window.__gddPwned=1</script><img src=x onerror="window.__gddPwned=1"> & \u2028\u2029 javascript:alert(1)'


def fixture() -> dict:
    nodes = {
        "constructor": {
            "kind": "action",
            "description": HOSTILE,
            "label": HOSTILE,
            "behavior": HOSTILE,
            "source_refs": [{"source": "source.test", "purpose": "provenance"}],
        },
        "action.second": {"kind": "action", "description": "Second action"},
        "component.orphan": {"kind": "component", "description": "Isolated node"},
        "entity.precision": {
            "kind": "entity",
            "description": "Exact integers",
            "fields": {"huge": {"type": "integer", "minimum": 9007199254740993}},
        },
        "community.backend": {"kind": "community", "description": "Backend"},
        "community.frontend": {"kind": "community", "description": "Frontend"},
        "scenario.hidden": {"kind": "scenario", "description": "Toggle this scenario"},
    }
    triples = [
        ("edge.call_one", "constructor", "calls", "action.second"),
        ("edge.call_two", "constructor", "calls", "action.second"),
        ("edge.depends", "constructor", "depends_on", "action.second"),
        ("edge.reverse", "action.second", "calls", "constructor"),
        ("edge.loop", "constructor", "calls", "constructor"),
        ("edge.member_backend", "constructor", "member_of", "community.backend"),
        ("edge.member_frontend", "constructor", "member_of", "community.frontend"),
        ("edge.second_frontend", "action.second", "member_of", "community.frontend"),
        ("edge.scenario", "scenario.hidden", "exercises", "constructor"),
    ]
    return {
        "format": "gdd/0.2",
        "project": {
            "id": "viewer-fixture",
            "title": "Viewer fixture",
            "goal": "Inspect without losing identity or executing data",
        },
        "nodes": nodes,
        "edges": [
            {"id": i, "from": f, "relation": r, "to": t} for i, f, r, t in triples
        ],
        "sources": {
            "source.test": {
                "role": "observation",
                "title": HOSTILE,
                "uri": "javascript:alert(1)",
                "format": "text/plain",
                "normative": False,
            }
        },
    }


def write_fixture(path: Path) -> Path:
    path.write_text(
        json.dumps(fixture(), ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    return path
