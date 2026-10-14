"""Executable fixtures for the declarative GDD 0.2 standard cases.

All observations and mandates generated here are synthetic checker inputs.
They are never application evidence or real authority.
"""

from copy import deepcopy
from datetime import datetime, timezone
from hashlib import sha256
import json
from pathlib import Path
import shutil
import tempfile

from gdd_validator import __version__, validate
from gdd_validator.loading import digest, load_bundle, parse
from gdd_validator.readiness import context_closure
from gdd_validator.report import aggregate

ROOT = Path(__file__).resolve().parent.parent


def write(path: Path, value: dict) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


def patch(data: dict, operations: list[dict]) -> None:
    for operation in operations:
        tokens = [
            t.replace("~1", "/").replace("~0", "~")
            for t in operation["path"].split("/")[1:]
        ]
        parent = data
        for token in tokens[:-1]:
            parent = parent[int(token)] if isinstance(parent, list) else parent[token]
        last = tokens[-1]
        if operation["op"] == "remove":
            if isinstance(parent, list):
                parent.pop(int(last))
            else:
                del parent[last]
        elif isinstance(parent, list):
            if last == "-":
                parent.append(deepcopy(operation["value"]))
            elif operation["op"] == "add":
                parent.insert(int(last), deepcopy(operation["value"]))
            else:
                parent[int(last)] = deepcopy(operation["value"])
        else:
            parent[last] = deepcopy(operation["value"])


def package(bundle, scope: list[str]) -> dict:
    closure = context_closure(bundle, scope)
    return {
        "format": "gdd-context/0.2",
        "spec_revision": bundle.revision,
        "scope": scope,
        "nodes": sorted(closure),
        "edges": [
            e["id"]
            for e in bundle.model["edges"]
            if e["from"] in closure and e["to"] in closure
        ],
        "sources": sorted(
            {
                r["source"]
                for n in closure
                for key in ("source_refs", "design_refs")
                for r in bundle.model["nodes"][n].get(key, [])
            }
        ),
        "questions": [
            i
            for i, q in bundle.model.get("questions", {}).items()
            if set(q["targets"]) & closure
        ],
        "extensions": list(bundle.model.get("extensions", {})),
    }


def evidence_fixture(directory: Path) -> tuple[Path, dict]:
    model = directory / "concert.gdd.yaml"
    bundle = load_bundle(model)
    records = parse((directory / "concert.records.yaml").read_bytes())
    for record in records["records"].values():
        record["spec_revision"] = bundle.revision
    plan = records["records"]["check.rule.capacity"]
    evidence = {
        "kind": "evidence",
        "check": "check.rule.capacity",
        "check_revision": digest(plan),
        "implementation": "implementation.fixture",
        "spec_revision": bundle.revision,
        "code_revision": "synthetic-current",
        "checker": {"name": "synthetic", "version": "1"},
        "scope": plan["scope"],
        "result": "PASS",
        "executed": True,
        "reason": "Synthetic observation for validator testing; no application execution.",
        "environment": {
            name: "synthetic-actual-environment"
            for name in plan["required_environment"]
        },
        "evidence_uri": "fixture://synthetic-only",
    }
    records["records"]["evidence.fixture"] = evidence
    records["records"]["binding.fixture"] = {
        "kind": "binding",
        "spec_revision": bundle.revision,
        "targets": ["rule.capacity"],
        "implementation": "implementation.fixture",
        "role": "test",
        "resource": "synthetic.py",
        "selector": {"kind": "whole-document"},
        "status": "checked",
        "revision": "synthetic-resource",
        "code_revision": "synthetic-current",
    }
    host = {
        "implementation": "implementation.fixture",
        "code_revision": "synthetic-current",
        "trusted_evidence": ["evidence.fixture"],
        "supported_methods": [plan["method"]],
        "environment": dict(evidence["environment"]),
        "negative_controls": plan["negative_controls"],
        "verified_bindings": ["binding.fixture"],
    }
    path = directory / "concert.records.yaml"
    write(path, records)
    return path, host


def execute_case(case: dict, directory: Path) -> dict:
    shutil.copytree(ROOT / "examples", directory, dirs_exist_ok=True)
    path = directory / Path(case["base"]).name
    data = parse(path.read_bytes())
    if case.get("patches"):
        patch(data, case["patches"])
        write(path, data)
    options = {"profiles": [case["profile"]], "scope": case["scope"]}
    ident = case["id"]
    if ident == "case.parse.duplicate_key":
        path.write_text("format: gdd/0.2\nformat: gdd/0.2\n")
    elif ident == "case.parse.alias":
        path.write_text("format: gdd/0.2\na: &shared {}\nb: *shared\n")
    elif ident == "case.auth.absent":
        options["activity"] = "implementation"
    elif ident == "case.auth.standing":
        revision = load_bundle(path).revision
        options.update(
            activity="implementation",
            host_context={
                "policy": {
                    "revision": "synthetic-policy-1",
                    "actor": "actor.fixture",
                    "activity": "implementation",
                    "spec_revision": revision,
                    "scope": ["action.reserve"],
                    "requirement": "standing",
                    "authorities": ["actor.fixture"],
                    "basis": "Synthetic host mandate; no real authority.",
                }
            },
        )
    elif ident == "case.extension.unsupported":
        options["unsupported_extensions"] = frozenset({"example.sensors"})
    elif ident in {"case.extension.override", "case.extension.identity"}:
        extpath = directory / "extensions/sensors.extension.json"
        extension = parse(extpath.read_bytes())
        if ident.endswith("override"):
            extension["relations"]["calls"] = extension["relations"].pop(
                "example.sensors.measures"
            )
        else:
            extension["version"] = "0.1.1"
        write(extpath, extension)
        data["extensions"]["extension.sensors"]["sha256"] = sha256(
            extpath.read_bytes()
        ).hexdigest()
        write(path, data)
    elif ident == "case.include.collision":
        data["nodes"]["entity.note"] = {
            "kind": "entity",
            "description": "Collision fixture",
        }
        write(path, data)
    elif (
        ident.startswith("case.evidence.") or ident == "case.records.check_hash_changed"
    ):
        path, host = evidence_fixture(directory)
        records = parse(path.read_bytes())
        if ident == "case.evidence.stale":
            records["records"]["evidence.fixture"]["code_revision"] = "synthetic-old"
        elif ident == "case.evidence.environment":
            records["records"]["evidence.fixture"]["environment"] = {
                "stub_database": "synthetic-stub"
            }
        else:
            records["records"]["evidence.fixture"]["check_revision"] = (
                "sha256:" + "0" * 64
            )
        write(path, records)
        options.update(model_path=directory / "concert.gdd.yaml", host_context=host)
        # Model already lists the record file; use it as primary instead of reading it twice.
        path = directory / "concert.gdd.yaml"
        options.pop("model_path")
    elif ident == "case.records.check_plan_missing":
        data["records"].pop("check.rule.capacity")
        write(path, data)
        path = directory / "concert.gdd.yaml"
    elif ident == "case.integration.loss":
        data = {
            "directed": True,
            "multigraph": False,
            "graph": {},
            "nodes": [
                {"id": n, "label": n, "file_type": "code", "source_file": n + ".py"}
                for n in ("a", "b")
            ],
            "links": [
                {"source": "a", "target": "b", "relation": r}
                for r in ("calls", "imports")
            ],
        }
        path = directory / "graphify.json"
        write(path, data)
        options["graphify_profile"] = "graphify-0.9.75-node-link"
        options["import_audit"] = {
            "format": "gdd-import-audit/0.2",
            "original": data,
            "reconstructed": {**data, "links": data["links"][:-1]},
        }
    elif ident == "case.integration.unsupported":
        path = directory / "graphify-unknown.json"
        write(path, {"nodes": [], "links": [], "schema_version": "99"})
        options["graphify_profile"] = "graphify-0.9.75-node-link"
    elif ident == "case.context.global":
        data["nodes"]["rule.capacity"]["rule"]["scope"] = "global"
        write(path, data)
        candidate = package(load_bundle(path), ["action.reserve"])
        candidate["nodes"].remove("rule.capacity")
        options["package"] = candidate
    elif ident.startswith("case.parallel."):
        if ident.endswith("unknown_effects"):
            data = {
                "format": "gdd/0.2",
                "project": {
                    "id": "example.parallel",
                    "title": "Synthetic",
                    "goal": "Test unknown declarations",
                },
                "nodes": {
                    "action.a": {"kind": "action", "description": "A"},
                    "action.b": {"kind": "action", "description": "B"},
                },
                "edges": [],
            }
            write(path, data)
            scopes = ["action.a", "action.b"]
        else:
            scopes = ["action.reserve", "action.cancel"]
        revision = load_bundle(path).revision
        options["parallel_plan"] = {
            "format": "gdd-parallel-plan/0.2",
            "packages": [
                {
                    "id": f"package.{i}",
                    "scope": [s],
                    "write_resources": [
                        (
                            "src/contracts.py"
                            if ident.endswith("write_overlap")
                            else f"src/{i}.py"
                        )
                    ],
                    "base_revision": revision,
                    "integration_plan": "Synthetic integration plan",
                }
                for i, s in enumerate(scopes)
            ],
            "serialized": [],
        }
    return validate(path, **options)


def run() -> dict:
    cases = parse((ROOT / "conformance/cases.yaml").read_bytes())["cases"]
    results = []
    for case in cases:
        with tempfile.TemporaryDirectory(prefix="gdd-conformance-") as temporary:
            try:
                report = execute_case(case, Path(temporary) / "examples")
                expected = case["expected"]
                actual = aggregate(report["checks"], expected["dimension"])
                codes = {
                    d["code"]
                    for c in report["checks"]
                    if c["dimension"] == expected["dimension"]
                    for d in c["diagnostics"]
                }
                matched = actual == expected["result"] and (
                    expected["diagnostic"] is None or expected["diagnostic"] in codes
                )
                results.append(
                    {
                        "id": case["id"],
                        "matched": matched,
                        "expected": expected,
                        "actual": actual,
                        "diagnostics": sorted(codes),
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
        "format": "gdd-conformance-run/0.2",
        "tool": "gdd-validator/" + __version__,
        "executed_at": datetime.now(timezone.utc).isoformat(),
        "synthetic_inputs": True,
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
        write(args.output, result)
    for case in result["cases"]:
        if not case["matched"]:
            print(json.dumps(case, ensure_ascii=False))
    print(f"{result['matched']}/{result['total']} conformance expectations matched")
    raise SystemExit(0 if result["matched"] == result["total"] else 1)
