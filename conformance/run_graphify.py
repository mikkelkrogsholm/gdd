"""Executable adapter cases, preserving the original 58 standard expectations."""

from copy import deepcopy
from datetime import datetime, timezone
import json
from pathlib import Path
import sys
import tempfile

# The executable harness also works when Python starts in conformance/.
ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from conformance.run import patch
from gdd_validator.graphify import decode_json
from gdd_validator.graphify_io import import_graphify
from gdd_validator.loading import parse
from gdd_validator.report import aggregate


def run() -> dict:
    document = parse((ROOT / "conformance/graphify-cases.yaml").read_bytes())
    results = []
    for case in document["cases"]:
        try:
            with tempfile.TemporaryDirectory(prefix="gdd-graphify-case-") as temporary:
                directory = Path(temporary)
                source = directory / "graph.json"
                data = decode_json((ROOT / "conformance" / case["base"]).read_bytes())
                patch(data, case.get("patches", []))
                condition = case.get("condition")
                if condition == "duplicate_edge":
                    data["links"].append(deepcopy(data["links"][0]))
                elif condition == "both_arrays":
                    data["edges"] = deepcopy(data["links"])
                elif condition == "docs_only":
                    for node in data["nodes"]:
                        node.update(file_type="document", source_file="SPEC.md")
                elif condition == "hyperedges":
                    data["hyperedges"] = [
                        {
                            "members": [n["id"] for n in data["nodes"]],
                            "unknown": {"preserve": True},
                        }
                    ]
                    data["graph"]["hyperedges"] = [
                        {"different": "preserve independently"}
                    ]
                elif condition in {"partial_markers", "conflicting_markers"}:
                    edge = data["links"][0]
                    edge["_src"] = (
                        edge["source"]
                        if condition == "partial_markers"
                        else edge["target"]
                    )
                    if condition == "conflicting_markers":
                        edge["_tgt"] = edge["source"]
                content = (
                    json.dumps(data, ensure_ascii=False, indent=2).encode() + b"\n"
                )
                if condition == "precision":
                    content = b'{"directed":true,"multigraph":false,"graph":{},"nodes":[{"id":9007199254740992,"file_type":"code","source_file":"a.py","label":"A"}],"links":[{"source":9007199254740993.0,"target":9007199254740992,"relation":"calls"}]}'
                source.write_bytes(content)
                bundle = directory / "bundle"
                report = import_graphify(source, bundle, profile=case["profile"])
                index = decode_json((bundle / "observations.json").read_bytes())
                expected = case["expected"]
                checks = (
                    [
                        c
                        for c in report["checks"]
                        if c["check_id"] == expected["check_id"]
                    ]
                    if expected.get("check_id")
                    else report["checks"]
                )
                actual = aggregate(checks, "integration")
                codes = {d["code"] for c in checks for d in c["diagnostics"]}
                assertions = case.get("assertions", {})
                assertions_ok = (
                    (
                        "nodes" not in assertions
                        or len(index["nodes"]) == assertions["nodes"]
                    )
                    and (
                        "edges" not in assertions
                        or len(index["edges"]) == assertions["edges"]
                    )
                    and (bundle / "graphify.raw.json").read_bytes() == content
                )
                matched = (
                    actual == expected["result"]
                    and (
                        not expected.get("diagnostic")
                        or expected["diagnostic"] in codes
                    )
                    and assertions_ok
                )
                results.append(
                    {
                        "id": case["id"],
                        "matched": matched,
                        "expected": expected,
                        "actual": actual,
                        "diagnostics": sorted(codes),
                        "raw_preserved": (bundle / "graphify.raw.json").read_bytes()
                        == content,
                        "report": report,
                    }
                )
        except Exception as error:
            results.append(
                {
                    "id": case["id"],
                    "matched": False,
                    "error": f"{type(error).__name__}: {error}",
                }
            )
    return {
        "format": "gdd-graphify-conformance-run/0.2",
        "executed_at": datetime.now(timezone.utc).isoformat(),
        "total": len(results),
        "matched": sum(r["matched"] for r in results),
        "cases": results,
    }


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = run()
    if args.output:
        args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    for case in result["cases"]:
        if not case["matched"]:
            print(json.dumps(case, ensure_ascii=False))
    print(
        f"{result['matched']}/{result['total']} Graphify adapter expectations matched"
    )
    raise SystemExit(0 if result["matched"] == result["total"] else 1)
