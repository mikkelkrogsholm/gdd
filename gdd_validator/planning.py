"""Work-package completeness and declarative parallel-plan checks."""

import posixpath
from glob import has_magic
from pathlib import Path, PurePosixPath

from .loading import Bundle
from .model import CORE_KINDS, has_cycle
from .readiness import context_closure
from .record_integrity import invalid_record_ids, model_ids
from .report import check, diagnostic

__all__ = ["validate_package", "validate_parallel"]


def _model_uncertain(bundle: Bundle) -> bool:
    """Record-only historical diagnostics do not make model semantics unknown."""
    record_only = bundle.records.keys() - model_ids(bundle.model)
    return any(
        c["dimension"] == "model"
        and c["result"] != "PASS"
        and not (
            (related := {i for d in c["diagnostics"] for i in d["related_ids"]})
            and related <= record_only
        )
        for c in bundle.checks
    )


def _write_resource(value: str) -> tuple[str, bool] | None:
    """Logical POSIX resource; terminal '/', '.' or '..' owns a directory subtree.

    Globs, absolute/escaping paths, Windows paths and symbol/URI syntax are not
    assigned disjointness by this profile. They require a different ownership profile.
    """
    if (
        not value.strip()
        or "\x00" in value
        or has_magic(value)
        or "\\" in value
        or ":" in value
        or PurePosixPath(value).is_absolute()
    ):
        return None
    normalized = posixpath.normpath(value)
    if normalized == ".." or normalized.startswith("../"):
        return None
    directory = value.endswith("/") or value.rsplit("/", 1)[-1] in {".", ".."}
    if normalized == "." and not directory:
        return None
    return normalized, directory


def _resource_overlap(left: tuple[str, bool], right: tuple[str, bool]) -> bool:
    a, a_directory = left
    b, b_directory = right
    return (
        a == b
        or a_directory
        and (a == "." or b.startswith(a + "/"))
        or b_directory
        and (b == "." or a.startswith(b + "/"))
    )


def _workspace(bundle: Bundle, value: str) -> Path | None:
    """Resolve a declared POSIX workspace path without requiring it to exist."""
    if (
        not value.strip()
        or "\x00" in value
        or "\\" in value
        or ":" in value
        or has_magic(value)
    ):
        return None
    try:
        path = Path(value)
        return (path if path.is_absolute() else bundle.root / path).resolve()
    except (OSError, RuntimeError, ValueError):
        return None


def validate_package(bundle: Bundle, package: dict) -> None:
    scope = package["scope"]
    closure = context_closure(bundle, scope)
    missing = closure - set(package["nodes"])
    missing_rules = {
        n for n in missing if bundle.model["nodes"].get(n, {}).get("kind") == "rule"
    }
    diagnostics = [
        diagnostic(
            (
                "APPLICABLE_RULE_OMITTED"
                if n in missing_rules
                else "READINESS_CONTRACT_MISSING"
            ),
            f"Context omits {n}.",
            ids=(n,),
        )
        for n in sorted(missing)
    ]
    required_edges = {
        e["id"]
        for e in bundle.model["edges"]
        if e["from"] in closure and e["to"] in closure
    }
    required_sources = {
        r["source"]
        for n in closure
        for field in ("source_refs", "design_refs")
        for r in bundle.model["nodes"].get(n, {}).get(field, [])
    }
    required_sources.update(
        r["source"]
        for e in bundle.model["edges"]
        if e["id"] in required_edges
        for r in e.get("source_refs", [])
    )
    while True:
        before = set(required_sources)
        required_sources.update(
            p["source"]
            for ident in before
            for p in bundle.model.get("sources", {})
            .get(ident, {})
            .get("derived_from", [])
        )
        if before == required_sources:
            break
    required_questions = {
        i
        for i, q in bundle.model.get("questions", {}).items()
        if set(q["targets"]) & closure
    }
    for registry, required, known in (
        ("edges", required_edges, {e["id"] for e in bundle.model["edges"]}),
        ("sources", required_sources, set(bundle.model.get("sources", {}))),
        ("questions", required_questions, set(bundle.model.get("questions", {}))),
        (
            "extensions",
            set(bundle.model.get("extensions", {})),
            set(bundle.model.get("extensions", {})),
        ),
        ("nodes", closure, set(bundle.model["nodes"])),
    ):
        absent = required - set(package[registry])
        extra = set(package[registry]) - known
        if registry != "nodes" and absent:
            diagnostics.append(
                diagnostic(
                    "READINESS_CONTRACT_MISSING",
                    f"Context omits {registry}: {sorted(absent)}.",
                )
            )
        if extra:
            diagnostics.append(
                diagnostic(
                    "REFERENCE_UNRESOLVED",
                    f"Context includes unknown {registry}: {sorted(extra)}.",
                )
            )
    result = (
        "FAIL"
        if diagnostics
        else (
            "UNKNOWN"
            if package["spec_revision"] != bundle.revision or bundle.revision is None
            else "PASS"
        )
    )
    if result == "PASS" and _model_uncertain(bundle):
        result = "UNKNOWN"
    if result == "UNKNOWN":
        diagnostics.append(
            diagnostic(
                "REVISION_UNRESOLVED",
                "Package base revision or required model semantics are unknown.",
                result="UNKNOWN",
            )
        )
    bundle.checks.append(
        check(
            "context.package",
            "readiness",
            result,
            "Candidate work package compared with fixed-point closure.",
            scope,
            diagnostics,
        )
    )


def validate_parallel(bundle: Bundle, plan: dict) -> None:
    packages = plan["packages"]
    ordered = {tuple(pair) for pair in plan.get("serialized", [])}
    ids = [p["id"] for p in packages]
    order_graph = {i: [b for a, b in ordered if a == i] for i in ids}

    errors, unknown = [], []
    if (
        len(set(ids)) != len(ids)
        or any(a not in ids or b not in ids for a, b in ordered)
        or has_cycle(order_graph)
    ):
        errors.append(
            diagnostic(
                "WRITE_OWNERSHIP_CONFLICT", "Invalid or cyclic serialization plan."
            )
        )

    def precedes(a: str, b: str) -> bool:
        seen, todo = set(), [a]
        while todo:
            current = todo.pop()
            if current == b:
                return True
            if current not in seen:
                seen.add(current)
                todo.extend(order_graph.get(current, []))
        return False

    write_resources = []
    workspaces = []
    for package in packages:
        resources = [_write_resource(value) for value in package["write_resources"]]
        write_resources.append([value for value in resources if value is not None])
        workspace = _workspace(bundle, package.get("workspace", ""))
        workspaces.append(workspace)
        if None in resources or workspace is None:
            unknown.append(
                diagnostic(
                    "INDEPENDENCE_UNRESOLVED",
                    "Ownership needs project-relative POSIX files/directories and a resolvable POSIX workspace; patterns or unsupported selectors are unknown.",
                    result="UNKNOWN",
                )
            )
        closure = context_closure(bundle, package["scope"])
        for ident in closure:
            node = bundle.model["nodes"].get(ident)
            if (
                node is None
                or (
                    node["kind"] == "action"
                    and any(
                        node.get("contract", {}).get(k) is not True
                        for k in ("effects_complete", "dependencies_complete")
                    )
                )
                or node["kind"] not in CORE_KINDS
            ):
                unknown.append(
                    diagnostic(
                        "INDEPENDENCE_UNRESOLVED",
                        f"Unknown effects/dependencies for {ident}.",
                        result="UNKNOWN",
                    )
                )
        if (
            not package["integration_plan"].strip()
            or package["base_revision"] != bundle.revision
            or bundle.revision is None
        ):
            unknown.append(
                diagnostic(
                    "INDEPENDENCE_UNRESOLVED",
                    "Integration plan or compatible contract revision missing.",
                    result="UNKNOWN",
                )
            )
        needed_contracts = {
            n
            for n in closure
            if bundle.model["nodes"].get(n, {}).get("kind") in {"action", "screen"}
        }
        if (
            not package.get("workspace")
            or "read_resources" not in package
            or not needed_contracts <= set(package.get("locked_contracts", []))
        ):
            unknown.append(
                diagnostic(
                    "INDEPENDENCE_UNRESOLVED",
                    "Workspace, read ownership or locked boundary contracts are undeclared.",
                    result="UNKNOWN",
                )
            )
        if any(
            bundle.model["nodes"].get(n, {}).get("kind")
            in {"rule", "scenario", "community"}
            for n in package["scope"]
        ):
            unknown.append(
                diagnostic(
                    "INDEPENDENCE_UNRESOLVED",
                    "Package scope is not an implementation target.",
                    result="UNKNOWN",
                )
            )
    integration = plan.get("integration", {})
    needed_rules = {
        n
        for p in packages
        for n in context_closure(bundle, p["scope"])
        if bundle.model["nodes"].get(n, {}).get("rule", {}).get("blocking")
    }
    invalid_records = invalid_record_ids(bundle)
    current_checks = {
        ident: record
        for ident, record in bundle.records.items()
        if record["kind"] == "check"
        and bundle.revision is not None
        and record["spec_revision"] == bundle.revision
        and ident not in invalid_records
    }
    # Checkplans retire only compatible current plans. Integration IDs explicitly
    # select declarations; no execution, evidence trust or host selection is inferred.
    retired_checks = {
        record["supersedes"]
        for record in current_checks.values()
        if record.get("supersedes") in current_checks
        and record["method"] == current_checks[record["supersedes"]]["method"]
        and set(current_checks[record["supersedes"]]["scope"]) <= set(record["scope"])
    }
    selected = integration.get("checks", [])
    invalid_selected = {
        ident
        for ident in selected
        if ident not in current_checks or ident in retired_checks
    }
    if invalid_selected:
        unknown.append(
            diagnostic(
                "INDEPENDENCE_UNRESOLVED",
                f"Integration references unknown, invalid, historical or retired checks: {sorted(invalid_selected)}.",
                result="UNKNOWN",
            )
        )
    needed_pairs = {
        (rule, method)
        for rule in needed_rules
        for method in bundle.model["nodes"][rule]["rule"].get("verification", [])
    }
    covered_pairs = {
        (target, current_checks[ident]["method"])
        for ident in selected
        if ident not in invalid_selected
        for target in current_checks[ident]["scope"]
    }
    if not integration.get("owner") or not needed_pairs <= covered_pairs:
        unknown.append(
            diagnostic(
                "INDEPENDENCE_UNRESOLVED",
                "Integration owner/checks do not cover every required blocking rule/method pair.",
                result="UNKNOWN",
            )
        )
    if _model_uncertain(bundle):
        unknown.append(
            diagnostic(
                "INDEPENDENCE_UNRESOLVED",
                "Model context has unresolved validation checks.",
                result="UNKNOWN",
            )
        )
    for i, left in enumerate(packages):
        for j, right in enumerate(packages[i + 1 :], start=i + 1):
            overlap = {
                (a[0], b[0])
                for a in write_resources[i]
                for b in write_resources[j]
                if _resource_overlap(a, b)
            }
            if (
                overlap
                and not precedes(left["id"], right["id"])
                and not precedes(right["id"], left["id"])
            ):
                errors.append(
                    diagnostic(
                        "WRITE_OWNERSHIP_CONFLICT",
                        f"Unserialized write overlap: {sorted(overlap)}.",
                    )
                )
            if (
                workspaces[i] is not None
                and workspaces[i] == workspaces[j]
                and not precedes(left["id"], right["id"])
                and not precedes(right["id"], left["id"])
            ):
                errors.append(
                    diagnostic(
                        "WRITE_OWNERSHIP_CONFLICT",
                        "Concurrent writers declare the same workspace.",
                    )
                )
    scope = sorted({n for p in packages for n in p["scope"]})
    if errors:
        bundle.checks.append(
            check(
                "parallel.ownership",
                "readiness",
                "FAIL",
                "Parallel write ownership conflicts.",
                scope,
                errors,
            )
        )
    if unknown:
        bundle.checks.append(
            check(
                "parallel.independence",
                "readiness",
                "UNKNOWN",
                "Independence requires known declarations and compatible integration plan.",
                scope,
                unknown,
            )
        )
    if not errors and not unknown:
        bundle.checks.append(
            check(
                "parallel.independence",
                "readiness",
                "PASS",
                "Declared write resources, effects, revisions and integration plans checked; runtime safety is not proven.",
                scope,
            )
        )
