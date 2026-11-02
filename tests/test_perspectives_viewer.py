"""Product lenses share canonical Python projections without changing model identity."""
from copy import deepcopy
from pathlib import Path

import pytest
from gdd_validator import render_view, verify_view
from gdd_validator.perspectives import perspective_data
from gdd_validator.visualization import GENERATOR, html_document, model_bundle, view_data

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.parametrize("name", ["greenfield", "refactoring", "partial"])
def test_view_embeds_same_python_perspectives(name):
    bundle = model_bundle(ROOT / f"examples/product/{name}.gdd.json")
    before = deepcopy(bundle.model)
    data = view_data(bundle, [next(iter(bundle.model["nodes"]))])
    assert data["format"] == "gdd-view/0.3"
    assert data["perspectives"] == perspective_data(bundle)
    assert data["model"] == before == bundle.model
    assert data["perspectives"]["spec_revision"] == data["spec_revision"]
    assert data["observation_inputs"] == []
    assert set(data["manifest"]["shown_nodes"]).isdisjoint(data["manifest"]["omitted_nodes"])


def test_prior_renderer_is_explicitly_unknown(tmp_path):
    source = ROOT / "examples/product/greenfield.gdd.json"
    data = view_data(model_bundle(source))
    data["generator"]["version"] = "0.3.0"
    path = tmp_path / "old.html"
    path.write_text(html_document(data))
    check = verify_view(source, path)["checks"][0]
    assert check["result"] == "UNKNOWN"
    assert "Unsupported renderer" in check["reason"]
    assert GENERATOR["version"] == "0.4.0"


@pytest.mark.parametrize("name", ["concert", "concert-0.3", "product/greenfield"])
def test_current_demos_match_renderer(name):
    suffix = ".gdd.json" if name.startswith("product/") else ".gdd.yaml"
    assert verify_view(ROOT / f"examples/{name}{suffix}", ROOT / f"examples/{name}.graph.html")["checks"][0]["result"] == "PASS"
