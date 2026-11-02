"""Generate bounded offline product-viewer fixtures in a new directory."""
from pathlib import Path
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


if __name__ == "__main__":
    prepare(Path(sys.argv[1]))
