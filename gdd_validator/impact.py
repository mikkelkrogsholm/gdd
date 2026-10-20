"""Explain potential change impact in declared model dependencies, without executing code."""

from collections import deque
from copy import deepcopy
from pathlib import Path
import re

from .boundaries import boundary_view
from .loading import digest, load_bundle
from .model import CORE_KINDS, type_references, validate_model
from .readiness import context_paths
from .record_integrity import invalid_record_ids
from .report import diagnostic

__all__ = ["analyze_impact", "impact_text"]
_ID = re.compile(r"[a-z][a-z0-9_]*(?:[.-][a-z0-9_]+)*\Z")


def _dependencies(bundle):
    """Only declared dependency semantics spread potential changes to consumers."""
    links = []
    definitions = {name: definition for ext in bundle.extensions.values()
                   for name, definition in ext["relations"].items()}
    for edge in bundle.model["edges"]:
        if edge["relation"] in {"calls", "depends_on", "imports", "reads", "creates", "changes", "navigates_to", "state_of"} or definitions.get(edge["relation"], {}).get("dependency"):
            links.append({"from": edge["to"], "to": edge["from"], "edge": edge["id"],
                          "relation": edge["relation"], "orientation": "reverse",
                          "derivation": "declared-dependency-consumer"})
    for ident, node in bundle.model["nodes"].items():
        contract = node.get("contract", {})
        specs = list(node.get("fields", {}).values())
        for fieldmap in ("input", "context"):
            specs.extend(contract.get(fieldmap, {}).values())
        if "output" in contract:
            specs.append(contract["output"])
        references = {target for spec in specs for target in type_references(spec)}
        for target in sorted(references):
            links.append({"from": target, "to": ident, "edge": None, "relation": None,
                          "orientation": "semantic", "derivation": "type-reference-consumer"})
        for transition in node.get("machine", {}).get("transitions", []):
            links.append({"from": transition["trigger"], "to": ident, "edge": None,
                          "relation": None, "orientation": "semantic",
                          "derivation": "machine-trigger-consumer"})
    return sorted(links, key=lambda link: (link["from"], link["to"], link["edge"] or "", link["derivation"]))


def analyze_impact(path, *, scope, implementation=None):
    """Return deterministic advisory impact rooted in explicit current-model node IDs.

    Calls, depends_on, imports, reads, creates, changes, navigates_to, state_of,
    declared extension dependencies, contract type references and machine triggers
    propagate potential change impact from provider to consumer. A changed writer
    does not propagate forward to its entity or sideways to other readers.
    Ownership, communities, constraints, scenarios and all other read context
    remain supporting. Historical records are never applied to this revision.
    """
    diagnostics = []
    result = {"format": "gdd-impact/0.1", "spec_revision": None, "scope": [],
              "implementation": implementation, "coverage": {
                  "result": "unavailable", "basis": "declared-model", "runtime_callers": "unassessed",
                  "unlisted_dependencies": "unassessed", "unspecified_declarations": []},
              "roots": [], "records": [], "records_omitted": [],
              "record_coverage": {"basis": "declared-records-only", "code_bindings": "unassessed"}, "boundaries": None, "diagnostics": diagnostics, "exit_code": 0}

    def issue(code, message, ids=(), unknown=False):
        diagnostics.append(diagnostic(code, message, ids=tuple(ids), result="UNKNOWN" if unknown else "FAIL"))
        result["exit_code"] = max(result["exit_code"], 2) if unknown and result["exit_code"] != 1 else 1

    if (not isinstance(scope, list) or not scope
            or any(not isinstance(ident, str) or not _ID.fullmatch(ident) for ident in scope)):
        issue("IMPACT_SCOPE_INVALID", "Select one or more syntactically valid current-model node IDs.")
        return result
    result["scope"] = sorted(set(scope))
    if implementation is not None and (not isinstance(implementation, str) or not _ID.fullmatch(implementation)):
        issue("IMPACT_IMPLEMENTATION_INVALID", "Implementation must be a GDD ID.")
        return result
    bundle = load_bundle(Path(path))
    result["spec_revision"] = bundle.revision
    representation_ok = not any(check["result"] != "PASS" for check in bundle.checks)
    if bundle.model.get("format") not in {"gdd/0.2", "gdd/0.3"}:
        issue("FORMAT_UNSUPPORTED", "Impact requires a supported current GDD model manifest.", unknown=True)
        representation_ok = False
    if representation_ok:
        validate_model(bundle)
    for check in bundle.checks:
        if check["result"] != "PASS":
            diagnostics.extend(check["diagnostics"])
            if check["result"] == "FAIL":
                result["exit_code"] = 1
            elif result["exit_code"] != 1:
                result["exit_code"] = 2
    if not representation_ok:
        return result
    nodes = bundle.model["nodes"]
    model_invalid = result["exit_code"] == 1
    for ident in result["scope"]:
        if ident not in nodes:
            issue("IMPACT_SCOPE_UNKNOWN", "Impact seeds must identify current-model nodes, not edges, sources or records.", [ident])
    if model_invalid:
        return result
    definitions = {name for ext in bundle.extensions.values() for name in ext["node_kinds"]}
    for ident, node in nodes.items():
        if node["kind"] not in CORE_KINDS and node["kind"] not in definitions:
            issue("EXTENSION_UNSUPPORTED", "Impact cannot establish semantics for this extension node kind.", [ident], True)
    adjacency = {}
    for link in _dependencies(bundle):
        adjacency.setdefault(link["from"], []).append(link)
    affected_roots = {}
    support = set()
    for root in (ident for ident in result["scope"] if ident in nodes):
        paths = {root: []}
        pending = deque([root])
        while pending:
            source = pending.popleft()
            for link in adjacency.get(source, []):
                if link["to"] not in paths:
                    paths[link["to"]] = paths[source] + [link]
                    pending.append(link["to"])
        contexts = {}
        # Each impacted consumer brings its established read context, retaining
        # the dependency witness back to the originally selected root.
        for ident, prefix in sorted(paths.items()):
            for target, suffix in context_paths(bundle, [ident]).items():
                candidate = prefix + suffix
                if target not in contexts or (len(candidate), repr(candidate)) < (len(contexts[target]), repr(contexts[target])):
                    contexts[target] = candidate
        items = []
        for target in sorted(contexts.keys() | paths.keys()):
            witness = paths[target] if target in paths else contexts[target]
            category = ("selected" if target == root else "direct" if target in paths and len(witness) == 1
                        else "transitive" if target in paths else "supporting")
            items.append({"id": target, "kind": nodes[target]["kind"], "category": category,
                          "reason": "Selected change seed." if category == "selected" else
                          "Declared dependency consumer may need review." if target in paths else
                          "Established supporting read context; no change propagation inferred.",
                          "witness": witness})
            affected_roots.setdefault(target, []).append(root)
        support.update(item["id"] for item in items)
        result["roots"].append({"id": root, "items": items})
    for ident in sorted(support):
        node = nodes[ident]
        declared = node.get("contract", {}).get("dependencies_complete")
        if declared is False:
            issue("IMPACT_DEPENDENCIES_INCOMPLETE",
                  "Declared action dependencies are explicitly incomplete; omitted consumers cannot be excluded.",
                  [ident], True)
        elif node["kind"] == "action" and declared is None:
            result["coverage"]["unspecified_declarations"].append(ident)
    if bundle.model["format"] == "gdd/0.3":
        result["boundaries"] = boundary_view(bundle, sorted(support))
    invalid = invalid_record_ids(bundle)
    for ident, record in sorted(bundle.records.items()):
        if record.get("kind") not in {"binding", "check"}:
            continue
        targets = sorted(set(record.get("targets", record.get("scope", []))) & affected_roots.keys())
        if not targets:
            continue
        if record.get("spec_revision") != bundle.revision or ident in invalid:
            result["records_omitted"].append({"id": ident, "reason": "Historical revision or invalid record; never rebound to the selected model."})
            continue
        if implementation is not None and record.get("implementation") not in ({None, implementation} if record["kind"] == "check" else {implementation}):
            result["records_omitted"].append({"id": ident, "reason": "Different implementation."})
            continue
        result["records"].append({"id": ident, "kind": record["kind"],
            "action": "rerun" if record["kind"] == "check" else "inspect",
            "targets": targets, "roots": sorted({root for target in targets for root in affected_roots[target]}),
            "spec_revision": record["spec_revision"], "implementation": record.get("implementation"),
            "record_revision": digest(record), "declaration": deepcopy(record),
            "provenance": "Declared inspection/rerun candidate only; code freshness and execution unassessed."})
    result["record_coverage"]["code_bindings"] = "present" if any(record["kind"] == "binding" and record["declaration"].get("role") == "code" for record in result["records"]) else "absent"
    result["coverage"]["result"] = "complete" if result["exit_code"] == 0 else "partial"
    return result


def impact_text(result):
    """Project the same assessed JSON result without recomputing semantics."""
    lines = [f"Impact: {result['coverage']['result']} (exit {result['exit_code']}; declared model; runtime callers unassessed)",
             "Scope: " + ", ".join(result["scope"])]
    for root in result["roots"]:
        lines.append(f"Seed {root['id']}:")
        for item in root["items"]:
            witness = " -> ".join([root["id"]] + [step["to"] for step in item["witness"]])
            lines.append(f"  {item['category']}: {item['id']} — {item['reason']} [{witness}]")
            for step in item["witness"]:
                lines.append(f"    {step['orientation']}: {step['edge'] or step['derivation']}"
                             + (f" ({step['relation']})" if step["relation"] else ""))
    if result["coverage"]["unspecified_declarations"]:
        lines.append("Dependency completeness unspecified: " + ", ".join(result["coverage"]["unspecified_declarations"]))
    lines.append("Code bindings: " + result["record_coverage"]["code_bindings"] + " (declared records only)")
    for record in result["records"]:
        lines.append(f"{record['action']}: {record['id']} ({', '.join(record['targets'])})")
    lines.extend(f"Omitted record {item['id']}: {item['reason']}" for item in result["records_omitted"])
    lines.extend(f"{item['code']}: {item['message']}" for item in result["diagnostics"])
    return "\n".join(lines) + "\n"
