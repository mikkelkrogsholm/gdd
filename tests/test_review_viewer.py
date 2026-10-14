"""Regression checks for exact source and design reference inspector text."""

import json

from conformance.viewer_fixture import fixture
from gdd_validator.visualization import model_bundle, view_data


def test_all_reference_inspectors_preserve_exact_selector_ordinals(tmp_path):
    model = fixture()
    ref = {
        "source": "source.test",
        "purpose": "provenance",
        "selector": {
            "kind": "markdown-heading",
            "value": "Title",
            "ordinal": 9007199254740993,
        },
    }
    model["nodes"]["constructor"]["source_refs"] = [ref]
    model["nodes"]["screen.references"] = {
        "kind": "screen",
        "description": "Exact design reference",
        "source_refs": [ref],
        "design_refs": [{**ref, "purpose": "contract"}],
    }
    model["edges"][0]["source_refs"] = [ref]
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model), encoding="utf-8")
    data = view_data(model_bundle(path))
    for ident in ("constructor", "screen.references", "edge.call_one"):
        refs = data["text"]["references"][ident]
        assert len(refs) == (2 if ident == "screen.references" else 1)
        for index, text in enumerate(refs):
            expected = {**ref, "purpose": "contract"} if index == 1 else ref
            assert json.loads(text) == expected
            assert "9007199254740993" in text
            assert "9007199254740992" not in text
