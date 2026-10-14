"""Derive reproducible development packets. Never execute graph or source text."""

from copy import deepcopy
from pathlib import Path

from jsonschema import Draft202012Validator

from .loading import Bundle, canonical, digest, load_bundle, schema
from .model import validate_model
from .planning import _write_resource, _workspace
from .readiness import context_closure, implementation_targets
from .record_integrity import invalid_record_ids
from .report import check
from .validator import input_schema, validate
from .boundaries import boundary_view, design_checklist

__all__ = ["prepare_workflow", "verify_workflow", "context_manifest"]


def require(value: object, definition: dict, label: str) -> None:
    """Reject malformed tool envelopes before using their contents."""
    canonical(value)
    errors = sorted(Draft202012Validator(definition).iter_errors(value), key=str)
    if errors:
        raise ValueError(f"Invalid {label}: {errors[0].message}")


def workflow_schema(kind: str) -> dict:
    document = schema("workflow")
    return {"$ref": f"#/$defs/{kind}", "$defs": document["$defs"]}


def context_manifest(bundle: Bundle, scope: list[str]) -> dict:
    """The fixed-point read context is deliberately distinct from write scope."""
    if set(scope) - bundle.model.get("nodes", {}).keys():
        raise ValueError("Unknown context scope.")
    closure = context_closure(bundle, scope)
    edges = [
        e for e in bundle.model["edges"] if e["from"] in closure and e["to"] in closure
    ]
    sources = {
        ref["source"]
        for ident in closure
        for field in ("source_refs", "design_refs")
        for ref in bundle.model["nodes"][ident].get(field, [])
    }
    sources.update(ref["source"] for e in edges for ref in e.get("source_refs", []))
    while True:
        before = set(sources)
        sources.update(
            ref["source"]
            for ident in before
            for ref in bundle.model.get("sources", {})
            .get(ident, {})
            .get("derived_from", [])
        )
        if sources == before:
            break
    return {
        "format": "gdd-context/0.2",
        "spec_revision": bundle.revision,
        "scope": sorted(set(scope)),
        "nodes": sorted(closure),
        "edges": sorted(e["id"] for e in edges),
        "sources": sorted(sources),
        "questions": sorted(
            ident
            for ident, q in bundle.model.get("questions", {}).items()
            if set(q["targets"]) & closure
        ),
        "extensions": sorted(bundle.model.get("extensions", {})),
    }


def _check_plans(bundle, scope: list[str], selected: list[str]) -> tuple[list, dict]:
    """Each blocking (rule, method) needs an explicitly selected active check."""
    invalid = invalid_record_ids(bundle)
    current = {
        ident: r
        for ident, r in bundle.records.items()
        if r.get("kind") == "check"
        and r.get("spec_revision") == bundle.revision
        and ident not in invalid
        and bundle.revision is not None
    }
    retired = {
        r["supersedes"]
        for r in current.values()
        if r.get("supersedes") in current
        and r["method"] == current[r["supersedes"]]["method"]
        and set(current[r["supersedes"]]["scope"]) <= set(r["scope"])
    }
    active = {
        ident: current[ident]
        for ident in selected
        if ident in current and ident not in retired
    }
    requirements = [
        {
            "rule": ident,
            "method": method,
            "checks": sorted(
                cid
                for cid, c in active.items()
                if ident in c["scope"] and c["method"] == method
            ),
        }
        for ident in sorted(context_closure(bundle, scope))
        if bundle.model["nodes"][ident].get("rule", {}).get("blocking")
        for method in bundle.model["nodes"][ident]["rule"].get("verification", [])
    ]
    return requirements, active


def _packet(bundle, package: dict, request: dict) -> dict:
    context = context_manifest(bundle, package["scope"])
    ids = {
        ident
        for key in ("nodes", "edges", "sources", "questions", "extensions")
        for ident in context[key]
    }
    requirements, checks = _check_plans(
        bundle, package["scope"], request["integration"]["checks"]
    )
    packet = {
        **deepcopy(package),
        "scope": context["scope"],
        "implementation_targets": sorted(
            implementation_targets(bundle, package["scope"])
        ),
        "context": context,
        "model_slice": {
            **{
                key: {
                    ident: deepcopy(bundle.model.get(key, {})[ident])
                    for ident in context[key]
                }
                for key in ("nodes", "sources", "questions", "extensions")
            },
            "edges": [
                deepcopy(e)
                for e in sorted(bundle.model["edges"], key=lambda e: e["id"])
                if e["id"] in context["edges"]
            ],
        },
        "locations": {
            ident: list(bundle.locations[ident])
            for ident in sorted(ids)
            if ident in bundle.locations
        },
        "locked_contracts": sorted(
            ident
            for ident in context["nodes"]
            if bundle.model["nodes"][ident]["kind"] in (
                {"action", "screen", "component"} if bundle.model["format"] == "gdd/0.3" else {"action", "screen"}
            )
        ),
        "required_checks": requirements,
        "check_plans": checks,
        "extension_semantics": deepcopy(bundle.extensions),
        "base_revision": bundle.revision,
    }
    if bundle.model["format"] == "gdd/0.3":
        packet["boundaries"] = boundary_view(bundle, context["nodes"])
        packet["design_review"] = design_checklist(bundle, context["nodes"])
    return packet


def _derive(path: Path, request: dict) -> dict:
    bundle = load_bundle(path)
    if bundle.revision is None or not bundle.model.get("nodes"):
        raise ValueError("A known model snapshot is required to derive a workplan.")
    before = len(bundle.checks)
    validate_model(bundle)
    if any(c["result"] != "PASS" for c in bundle.checks[before:]):
        raise ValueError(
            "Cannot derive packets from unresolved model semantics; run gdd validate first."
        )
    packages = [_packet(bundle, p, request) for p in request["packages"]]
    return {
        "format": "gdd-workplan/0.1",
        "request": deepcopy(request),
        "project_id": bundle.model["project"]["id"],
        "spec_revision": bundle.revision,
        "snapshot": deepcopy(bundle.snapshot),
        "scope": sorted({ident for p in packages for ident in p["scope"]}),
        "packages": packages,
        "notice": "Derived product data; not authorization, executed evidence or an instruction to launch tools.",
    }


def _plan_report(path: Path, plan: dict, host: dict) -> dict:
    report = validate(path, profiles=["model"], scope=plan["scope"], host_context=host)
    report["profiles"] = ["model", "readiness"]
    bundle = load_bundle(path)
    request = plan["request"]
    packages = plan["packages"]
    ids = [p["id"] for p in packages]
    if len(set(ids)) != len(ids):
        raise ValueError("Package IDs must be unique.")
    # The existing parallel validator does not assess individual target readiness.
    for p in packages:
        readiness = validate(
            path, profiles=["readiness"], package=p["context"], host_context=host
        )
        report["checks"].extend(
            {**c, "check_id": p["id"] + "." + c["check_id"]}
            for c in readiness["checks"]
            if c["dimension"] == "readiness"
        )
        valid_resources = all(
            _write_resource(r) is not None
            for r in p["write_resources"] + p["read_resources"]
        )
        report["checks"].append(
            check(
                p["id"] + ".ownership",
                "readiness",
                (
                    "PASS"
                    if valid_resources
                    and _workspace(bundle, p["workspace"]) is not None
                    else "UNKNOWN"
                ),
                "Explicit logical file ownership and workspace declarations; actual isolation must be verified by the host.",
                p["scope"],
            )
        )
    required, checks = _check_plans(
        bundle, plan["scope"], request["integration"]["checks"]
    )
    used = {ident for r in required for ident in r["checks"]}
    complete = (
        all(len(r["checks"]) == 1 for r in required)
        and set(checks) == set(request["integration"]["checks"]) == used
    )
    report["checks"].append(
        check(
            "workflow.check_plans",
            "readiness",
            "PASS" if complete else "UNKNOWN",
            "Selected active checks must cover every blocking rule and method.",
            plan["scope"],
        )
    )
    if len(packages) > 1:
        parallel = {
            "format": "gdd-parallel-plan/0.2",
            "packages": [
                {
                    key: p[key]
                    for key in (
                        "id",
                        "scope",
                        "write_resources",
                        "read_resources",
                        "locked_contracts",
                        "workspace",
                        "base_revision",
                        "integration_plan",
                    )
                }
                for p in packages
            ],
            "serialized": request["serialized"],
            "integration": request["integration"],
        }
        parallel_report = validate(
            path, profiles=["readiness"], parallel_plan=parallel, host_context=host
        )
        report["checks"].extend(
            c for c in parallel_report["checks"] if c["dimension"] == "readiness"
        )
    elif request["serialized"]:
        raise ValueError("A singleton plan cannot contain serialized pairs.")
    return report


def prepare_workflow(
    path: Path, request: dict, *, host_context: dict | None = None
) -> dict:
    """Prepare a draft plan and diagnostics; a plan never authorizes execution."""
    require(request, workflow_schema("request"), "workflow request")
    require(
        host_context if host_context is not None else {},
        input_schema("host"),
        "host context",
    )
    plan = _derive(Path(path), request)
    plan["revision"] = digest(plan)
    report = _plan_report(
        Path(path), plan, host_context if host_context is not None else {}
    )
    return {"format": "gdd-workflow-result/0.1", "report": report, "artifact": plan}


def verify_workflow(
    path: Path, plan: dict, *, host_context: dict | None = None
) -> dict:
    """Re-derive complete payload from authoritative model, not just packet IDs."""
    require(plan, workflow_schema("plan"), "workplan")
    require(plan["request"], workflow_schema("request"), "workflow request")
    require(
        host_context if host_context is not None else {},
        input_schema("host"),
        "host context",
    )
    expected = _derive(Path(path), plan["request"])
    expected["revision"] = digest(expected)
    report = _plan_report(
        Path(path), expected, host_context if host_context is not None else {}
    )
    report["checks"].append(
        check(
            "workflow.packet_integrity",
            "readiness",
            "PASS" if canonical(plan) == canonical(expected) else "FAIL",
            "Complete packet must match the current model, assets, selected check declarations and original request.",
            expected["scope"],
        )
    )
    return report
