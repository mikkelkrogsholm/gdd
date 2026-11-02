"""Generate bounded offline product-viewer fixtures in a new directory."""
from pathlib import Path
import json
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from gdd_validator import render_view


def prepare(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=False)
    for name in ("greenfield", "refactoring", "partial"):
        source = ROOT / f"examples/product/{name}.gdd.json"
        render_view(source, directory / f"{name}.html")
    render_view(ROOT / "examples/product/greenfield.gdd.json", directory / "scoped.html", scope=["step.choose"])
    render_view(ROOT / "examples/product/partial.gdd.json", directory / "partial-scoped.html", scope=["step.reserve"])
    render_view(ROOT / "examples/concert.gdd.yaml", directory / "legacy.html")
    # Deliberately hostile human text and directed parallel edges remain canonical.
    shutil.copytree(ROOT / "examples/product/extensions", directory / "extensions")
    model = json.loads((ROOT / "examples/product/greenfield.gdd.json").read_text())
    hostile = '</script><script>globalThis.gddInjected=true</script><img src="https://invalid.example/pixel" onerror="globalThis.gddInjected=true"> & Å漢字 👩🏽‍💻 \u2028\u2029'
    model["nodes"]["actor.guest"]["data"]["goal"] = "Goal " + hostile
    model["nodes"]["step.choose"]["data"] = {"user_action": "Action " + hostile, "system_response": "Response " + hostile}
    model["nodes"]["community.product"] = {"kind": "community", "description": "Product review group."}
    for ident in ["actor.guest", "step.choose", "step.reserve", "component.booking", "entity.reservation"]:
        model["edges"].append({"id": "edge.member." + ident, "from": ident, "relation": "member_of", "to": "community.product"})
    model["edges"].extend([
        {"id": "edge.parallel", "from": "step.choose", "relation": "gdd.product.next", "to": "step.reserve", "data": {}},
        {"id": "edge.reverse", "from": "step.reserve", "relation": "gdd.product.next", "to": "step.choose", "data": {}},
    ])
    source = directory / "adversarial.gdd.json"
    source.write_text(json.dumps(model, ensure_ascii=False), encoding="utf-8")
    render_view(source, directory / "adversarial.html")
    render_view(source, directory / "adversarial-scoped.html", scope=["step.choose"])


if __name__ == "__main__":
    prepare(Path(sys.argv[1]))
