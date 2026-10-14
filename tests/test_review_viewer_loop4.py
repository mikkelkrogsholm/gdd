"""Scope strings must not silently select unrelated character-sized IDs."""

import json

import pytest

from gdd_validator import render_view
from gdd_validator.visualization import model_bundle, view_data


@pytest.fixture
def model_path(tmp_path):
    model = {
        "format": "gdd/0.2",
        "project": {"id": "project.scope", "title": "Scope", "goal": "Exact selection"},
        "nodes": {
            key: {"kind": "entity", "description": key, "fields": {}}
            for key in ("a", "b", "ab")
        },
        "edges": [],
    }
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model))
    return path


@pytest.mark.parametrize("scope", ["ab", "", 3, False, {}, [{}], [3]])
def test_view_scope_rejects_invalid_types_without_publishing(model_path, scope):
    with pytest.raises(ValueError, match="list of strings"):
        view_data(model_bundle(model_path), scope=scope)
    output = model_path.parent / "output.html"
    with pytest.raises(ValueError, match="list of strings"):
        render_view(model_path, output, scope=scope)
    assert not output.exists()


@pytest.mark.parametrize(
    "scope,expected",
    [
        (None, ["a", "ab", "b"]),
        ([], ["a", "ab", "b"]),
        (["ab"], ["ab"]),
        (["a", "a"], ["a"]),
    ],
)
def test_view_scope_keeps_exact_valid_list_selection(model_path, scope, expected):
    data = view_data(model_bundle(model_path), scope=scope)
    assert data["manifest"]["shown_nodes"] == expected
