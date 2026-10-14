"""Execute artifact conformance for gdd-viewer-html/0.1; browser tests are separate."""

import argparse
import json
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from conformance.viewer_fixture import write_fixture
from gdd_validator import render_view, verify_view
from gdd_validator.loading import parse
from gdd_validator.report import aggregate
from gdd_validator.visualization import PAYLOAD, html_document


def run() -> dict:
    cases = parse((ROOT / "conformance/visualization-cases.yaml").read_bytes())["cases"]
    results = []
    for case in cases:
        with tempfile.TemporaryDirectory(prefix="gdd-view-case-") as temporary:
            directory = Path(temporary)
            model = (
                write_fixture(directory / "model.json")
                if case["base"] == "fixture"
                else ROOT / "examples" / f'{case["base"]}.gdd.yaml'
            )
            output = directory / "graph.html"
            operation = case["operation"]
            render_view(
                model, output, scope=["action.reserve"] if operation == "scope" else []
            )
            html = output.read_text(encoding="utf-8")
            data = json.loads(PAYLOAD.search(html)[1])
            if operation == "renderer":
                data["generator"]["version"] = "unsupported"
            elif operation == "revision":
                data["spec_revision"] = "sha256:" + "0" * 64
            elif operation == "direction":
                data["model"]["edges"][0]["to"] = "constructor"
            elif operation == "omit_edge":
                data["model"]["edges"].pop(1)
            elif operation == "manifest":
                data["manifest"]["shown_edges"] = []
            if operation in {
                "renderer",
                "revision",
                "direction",
                "omit_edge",
                "manifest",
            }:
                html = html_document(data)
            elif operation == "script":
                html = html.replace('"use strict";', '"use strict";window.hacked=1;')
            elif operation == "blank":
                html = "<html></html>"
            elif operation == "crlf":
                html = html.replace("\n", "\r\n")
            elif operation == "deep":
                html = (
                    '<script id="gdd-data" type="application/json">'
                    + "[" * 2000
                    + "0"
                    + "]" * 2000
                    + "</script>"
                )
            output.write_bytes(html.encode("utf-8"))
            if operation == "unavailable":
                output.unlink()
            report = verify_view(model, None if operation == "missing" else output)
            actual = aggregate(report["checks"], "integration")
            results.append(
                {
                    "id": case["id"],
                    "expected": case["expected"],
                    "actual": actual,
                    "matched": actual == case["expected"],
                    "report": report,
                }
            )
    return {
        "format": "gdd-view-conformance/0.1",
        "profile": "gdd-viewer-html/0.1",
        "matched": sum(r["matched"] for r in results),
        "total": len(results),
        "results": results,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = run()
    text = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        with args.output.open("x", encoding="utf-8") as file:
            file.write(text)
    else:
        sys.stdout.write(text)
    raise SystemExit(0 if result["matched"] == result["total"] else 1)
