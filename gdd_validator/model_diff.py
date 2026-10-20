"""Read-only stable-identity model comparison and structural review advisories."""
from copy import deepcopy
import json
from pathlib import Path

from .loading import canonical, load_bundle
from .model import validate_model
from .report import pointer

__all__ = ["diff_models", "diff_text"]


def _fields(before, after, path=""):
    if canonical(before) == canonical(after):
        return []
    if isinstance(before, dict) and isinstance(after, dict):
        changes = []
        for key in sorted(before.keys() | after.keys()):
            here = path + pointer(key)
            if key not in before:
                changes.append({"pointer": here, "change": "added", "after": deepcopy(after[key])})
            elif key not in after:
                changes.append({"pointer": here, "change": "removed", "before": deepcopy(before[key])})
            else:
                changes.extend(_fields(before[key], after[key], here))
        return changes
    # Ordered arrays and prose remain exact values; no inferred prose semantics.
    return [{"pointer": path, "change": "changed", "before": deepcopy(before), "after": deepcopy(after)}]


def _side(path):
    bundle = load_bundle(Path(path))
    supported = bundle.model.get("format") in {"gdd/0.2", "gdd/0.3"}
    # A valid manifest and loaded fragments are schema-shaped, even if other
    # assets are missing. Semantic validation is safe on that partial graph.
    representation_complete = supported and all(item["result"] == "PASS" for item in bundle.checks)
    if supported:
        validate_model(bundle)
    checks = deepcopy(bundle.checks)
    if not supported:
        checks.append({"check_id": "diff.manifest", "dimension": "model", "result": "UNKNOWN",
                       "reason": "A supported GDD model manifest is unavailable.", "scope": [],
                       "diagnostics": []})
    complete = representation_complete and not any(
        diagnostic["code"] == "ID_COLLISION"
        for item in checks for diagnostic in item.get("diagnostics", [])
    )
    side = {"path": str(path), "spec_revision": bundle.revision,
            "format": bundle.model.get("format"),
            "coverage": "complete" if complete else "partial" if supported else "unavailable",
            "checks": checks}
    model = bundle.model if supported else {}
    registries = {name: deepcopy(model.get(name, {})) for name in ("nodes", "sources", "questions")}
    # Colliding identities cannot be compared by silently selecting one value.
    duplicates = set()
    edges = {}
    for edge in model.get("edges", []):
        if edge["id"] in edges:
            duplicates.add(edge["id"])
        edges[edge["id"]] = deepcopy(edge)
    for item in checks:
        for diagnostic in item.get("diagnostics", []):
            if diagnostic["code"] == "ID_COLLISION":
                duplicates.update(diagnostic.get("related_ids", []))
    registries["edges"] = edges
    for registry in registries.values():
        for ident in duplicates:
            registry.pop(ident, None)
    metadata = {key: deepcopy(value) for key, value in bundle.model.items()
                if key not in {"nodes", "edges", "sources", "questions", "includes", "record_files", "extensions"}}
    # Compare extension identity and loaded semantics, not packaging paths/hash.
    metadata["extensions"] = {ident: {"namespace": decl["namespace"], "version": decl["version"],
                                       "definition": deepcopy(bundle.extensions.get(decl["namespace"]))}
                              for ident, decl in bundle.model.get("extensions", {}).items()}
    return bundle, side, registries, metadata


def _advisories(base, head, changes):
    advisories = []

    def add(code, ident, path, message, before=None, after=None):
        advisories.append({"code": code, "id": ident, "pointer": path, "message": message,
                           "before": deepcopy(before), "after": deepcopy(after)})

    for change in changes:
        if change["registry"] == "sources" and change["change"] == "changed":
            previous, current = change["before"].get("role"), change["after"].get("role")
            if previous != current:
                add("IDENTITY_REVIEW", change["id"], "/role", "A source identity's role changed. Two snapshots cannot establish historical ID reuse.", previous, current)
        if change["registry"] != "nodes" or change["change"] == "added":
            continue
        ident = change["id"]
        old, new = base.get(ident, {}), head.get(ident, {})
        rule, updated = old.get("rule", {}), new.get("rule", {})
        if rule.get("blocking") is True and (change["change"] == "removed" or updated.get("blocking") is not True):
            add("BLOCKING_RULE_RELAXED", ident, "/rule/blocking", "A blocking rule was removed or is no longer explicitly blocking.", True, updated.get("blocking"))
        old_constraint, new_constraint = rule.get("constraint", {}), updated.get("constraint", {})
        if old_constraint.get("policy") == "forbid" and new_constraint.get("policy") != "forbid":
            add("FORBID_POLICY_RELAXED", ident, "/rule/constraint/policy", "A forbid constraint was removed or replaced by another policy.", "forbid", new_constraint.get("policy"))
        if old_constraint.get("policy") == "forbid" and new_constraint:
            if old_constraint.get("reach") == "transitive" and new_constraint.get("reach") == "direct":
                add("FORBID_REACH_NARROWED", ident, "/rule/constraint/reach", "A forbid constraint now checks direct paths only.", "transitive", "direct")
            for key in ("from", "to"):
                if canonical(old_constraint.get(key)) != canonical(new_constraint.get(key)):
                    add("FORBID_ENDPOINT_CHANGED", ident, "/rule/constraint/" + key, "A forbid endpoint changed; review whether the restriction narrowed.", old_constraint.get(key), new_constraint.get(key))
        if change["change"] != "changed":
            continue
        for key in ("kind", "role"):
            if canonical(old.get(key)) != canonical(new.get(key)):
                add("IDENTITY_REVIEW", ident, "/" + key, "An identity's kind or role changed. Two snapshots cannot establish historical ID reuse.", old.get(key), new.get(key))
        for key in ("errors", "preconditions", "postconditions"):
            previous, current = old.get("contract", {}).get(key, {} if key == "errors" else []), new.get("contract", {}).get(key, {} if key == "errors" else [])
            removed = sorted(set(previous) - set(current)) if isinstance(previous, dict) and isinstance(current, dict) else [value for value in previous if canonical(value) not in {canonical(item) for item in current}] if isinstance(previous, list) and isinstance(current, list) else []
            if removed:
                add("CONTRACT_REQUIREMENT_REMOVED", ident, "/contract/" + key, "Contract entries disappeared or were replaced; review exact text without assuming semantic weakening.", removed, current)
        removed = sorted(set(old.get("interfaces", [])) - set(new.get("interfaces", [])))
        if removed:
            add("INTERFACE_SHRUNK", ident, "/interfaces", "Previously declared interface actions are absent.", removed, new.get("interfaces", []))
    return advisories


def diff_models(base, head):
    """Compare loaded stable IDs; incomplete sides never justify absence claims."""
    old_bundle, old_side, old, old_meta = _side(base)
    new_bundle, new_side, new, new_meta = _side(head)
    complete = old_side["coverage"] == new_side["coverage"] == "complete"
    available = all(side["coverage"] != "unavailable" for side in (old_side, new_side))
    changes, relocated = [], []
    counts = {name: {kind: 0 for kind in ("added", "removed", "changed")} for name in old}
    for registry in old:
        for ident in sorted(old[registry].keys() | new[registry].keys()):
            before, after = old[registry].get(ident), new[registry].get(ident)
            if (before is None or after is None) and not complete:
                continue
            change = "added" if before is None else "removed" if after is None else "changed"
            fields = _fields(before, after)
            if fields:
                item = {"registry": registry, "id": ident, "change": change, "fields": fields}
                if before is not None:
                    item["before"] = before
                if after is not None:
                    item["after"] = after
                changes.append(item)
                counts[registry][change] += 1
            if before is not None and after is not None:
                start, end = old_bundle.locations.get(ident), new_bundle.locations.get(ident)
                # Array position and manifest filename are packaging noise; only
                # a move between manifest/fragment containers is relocation.
                def container(bundle, location):
                    return "manifest" if location and location[0] == bundle.path.name else location[0] if location else None
                if container(old_bundle, start) != container(new_bundle, end):
                    relocated.append({"registry": registry, "id": ident, "base": list(start) if start else None, "head": list(end) if end else None})
    # Partial metadata cannot establish absence or equality of expanded assets.
    metadata_changes = _fields(old_meta, new_meta) if complete else []
    advisories = _advisories(old["nodes"], new["nodes"], changes)
    if complete:
        old_verified = {edge["to"] for edge in old["edges"].values() if edge["relation"] == "verifies"}
        new_verified = {edge["to"] for edge in new["edges"].values() if edge["relation"] == "verifies"}
        for ident in sorted(old_verified - new_verified):
            advisories.append({"code": "LAST_VERIFIER_REMOVED", "id": ident, "pointer": "/edges", "message": "The rule lost its last declared verifying scenario edge.", "before": True, "after": False})
    diagnostics = [{"side": name, "check_id": item["check_id"], "result": item["result"], "reason": item["reason"], "diagnostics": item["diagnostics"]}
                   for name, side in (("base", old_side), ("head", new_side)) for item in side["checks"] if item["result"] != "PASS"]
    exit_code = 1 if any(item["result"] == "FAIL" for item in diagnostics) else 0 if complete else 2
    return {"format": "gdd-model-diff/0.1", "base": old_side, "head": new_side,
            "comparison": {"coverage": "complete" if complete else "partial" if available else "unavailable", "semantic_changes": bool(changes or metadata_changes), "counts": counts},
            "changes": changes, "metadata_changes": metadata_changes, "relocated": relocated,
            "advisories": sorted(advisories, key=lambda item: (item["id"], item["code"], item["pointer"])),
            "diagnostics": diagnostics, "exit_code": exit_code}


def diff_text(result):
    """Render exact differences and clearly advisory structural concerns."""
    lines = ["GDD model diff — " + result["comparison"]["coverage"],
             f"Base {result['base']['spec_revision']} ({result['base']['format']}) → Head {result['head']['spec_revision']} ({result['head']['format']})"]
    if result["comparison"]["coverage"] != "complete":
        lines.append("UNKNOWN comparison coverage: absence claims and metadata comparison are suppressed.")
    for registry, counts in result["comparison"]["counts"].items():
        lines.append(f"{registry}: +{counts['added']} added -{counts['removed']} removed ~{counts['changed']} changed")
    for item in result["changes"]:
        lines.append(f"{item['change']} {item['registry']} {item['id']}")
        for field in item["fields"]:
            before = json.dumps(field.get("before"), ensure_ascii=False, sort_keys=True)
            after = json.dumps(field.get("after"), ensure_ascii=False, sort_keys=True)
            lines.append(f"  {field['pointer'] or '/'} {field['change']}: {before} → {after}")
    for field in result["metadata_changes"]:
        lines.append("metadata " + json.dumps(field, ensure_ascii=False, sort_keys=True))
    for item in result["relocated"]:
        lines.append(f"relocated {item['registry']} {item['id']}: {item['base']} → {item['head']}")
    for item in result["advisories"]:
        lines.append(f"Advisory {item['code']} {item['id']}{item['pointer']}: {item['message']}")
    for item in result["diagnostics"]:
        lines.append(f"{item['side']} {item['result']} {item['check_id']}: {item['reason']}")
    lines.append("Structural advisories do not prove prose weakening or historical ID reuse.")
    return "\n".join(lines) + "\n"
