"""Static Python observations against explicit GDD architecture constraints.

This consumer observes caller-selected source bytes. It never executes project
code, trusts a recorded execution, or writes authorization/evidence records.
"""

from collections import defaultdict, deque
from fnmatch import fnmatchcase
from functools import lru_cache
from hashlib import sha256
import os
from pathlib import Path, PurePosixPath
import platform
import re
import stat

from . import __version__
from .boundaries import owners
from .loading import digest, load_bundle, resolve_asset, resolve_local_path
from .model import validate_model
from .record_integrity import invalid_record_ids
from .report import check, exit_code

__all__ = ["check_code"]
PROFILE = "python-imports/0.1"
IGNORED_DIRECTORIES = frozenset({".git", ".venv", ".tox", ".nox", "__pycache__", "node_modules"})
ID = re.compile(r"[a-z][a-z0-9_]*(?:[.-][a-z0-9_]+)*\Z")


def _diagnostic(code, message, *, file=None, ids=()):
    return {"code": code, "severity": "warning", "message": message,
            "location": {"file": file, "pointer": "", "line": None, "column": None},
            "related_ids": list(dict.fromkeys(ids))}


def _resource(value):
    if (not isinstance(value, str) or not value or any(c in value for c in ("\\", ":", "\0"))
            or PurePosixPath(value).is_absolute()):
        raise ValueError("Use a normalized relative POSIX resource or glob.")
    parts = value.rstrip("/").split("/")
    if any(part in {"", ".", ".."} or "**" in part and part != "**" for part in parts):
        raise ValueError("Resources cannot contain empty, dot or parent segments; ** must be a whole segment.")
    return value


def _matches(name, pattern):
    if pattern.endswith("/"):
        return name.startswith(pattern)
    names, patterns = name.split("/"), pattern.split("/")

    @lru_cache(None)
    def match(i, j):
        if j == len(patterns):
            return i == len(names)
        if patterns[j] == "**":
            return match(i, j + 1) or i < len(names) and match(i + 1, j)
        return i < len(names) and fnmatchcase(names[i], patterns[j]) and match(i + 1, j + 1)

    return match(0, 0)


def _prefix(pattern):
    parts = []
    for part in pattern.rstrip("/").split("/"):
        if any(c in part for c in "*?["):
            break
        parts.append(part)
    return "/".join(parts)


def _overlaps_omission(resource, omitted):
    prefix = _prefix(resource)
    for name in omitted:
        if _matches(name.rstrip("/"), resource) or _matches(name + "placeholder.py", resource):
            return True
        if name.endswith("/") and (not prefix or prefix.startswith(name) or name.startswith(prefix.rstrip("/") + "/")):
            return True
    return False


def _inventory(root, exclude):
    sources, diagnostics, omitted, identities = {}, [], [], {}

    def failed(error):
        diagnostics.append(_diagnostic("CODE_SOURCE_UNAVAILABLE", str(error)))

    for directory, folders, files in os.walk(root, topdown=True, followlinks=False, onerror=failed):
        current = Path(directory)
        kept = []
        for name in sorted(folders):
            path = current / name
            relative = path.relative_to(root).as_posix()
            if name in IGNORED_DIRECTORIES or any(_matches(relative, p) or _matches(relative + "/", p) for p in exclude):
                omitted.append(relative + "/")
            elif path.is_symlink():
                diagnostics.append(_diagnostic("CODE_SOURCE_UNAVAILABLE", "Source directories cannot be symlinks.", file=relative))
                omitted.append(relative + "/")
            else:
                kept.append(name)
        folders[:] = kept
        for name in sorted(files):
            if not name.endswith(".py"):
                continue
            path = current / name
            relative = path.relative_to(root).as_posix()
            if any(_matches(relative, p) for p in exclude):
                omitted.append(relative)
                continue
            try:
                identity = path.lstat()
                if not stat.S_ISREG(identity.st_mode):
                    raise ValueError("Python source must be a regular file without symlinks.")
                physical = (identity.st_dev, identity.st_ino)
                if physical in identities:
                    previous = identities[physical]
                    sources.pop(previous, None)
                    omitted.extend((previous, relative))
                    diagnostics.append(_diagnostic("CODE_BINDING_AMBIGUOUS", "Multiple source paths identify the same physical file.", file=relative))
                    continue
                identities[physical] = relative
                sources[relative] = path.read_bytes()
            except (OSError, ValueError) as error:
                diagnostics.append(_diagnostic("CODE_SOURCE_UNAVAILABLE", str(error), file=relative))
    return sources, diagnostics, sorted(omitted)


def _load_records(bundle, path):
    if path is None:
        return
    selected = resolve_local_path(Path(path))
    declared = {resolve_asset(bundle.root, relative) for relative in bundle.model.get("record_files", [])}
    if selected in declared:
        return
    extra = load_bundle(selected)
    if (extra.model.get("format") != "gdd-records/0.2"
            or any(c["result"] != "PASS" for c in extra.checks)):
        raise ValueError("Additional records must be a supported, schema-valid records document.")
    if extra.model["project_id"] != bundle.model["project"]["id"]:
        raise ValueError("Additional records belong to a different project.")
    if bundle.records.keys() & extra.records.keys():
        raise ValueError("Additional records duplicate existing record IDs.")
    bundle.records.update(extra.records)


def _bindings(bundle, implementation, sources, omitted):
    selected, claims, diagnostics, advisory = {}, defaultdict(list), [], []
    invalid = invalid_record_ids(bundle)
    nodes, owned = bundle.model["nodes"], owners(bundle.model)
    for ident, record in sorted(bundle.records.items()):
        if record["kind"] != "binding" or record["role"] != "code" or record["implementation"] != implementation:
            continue
        if record["spec_revision"] != bundle.revision:
            advisory.append({"code": "CODE_BINDING_HISTORICAL", "binding": ident,
                             "message": "Historical binding was not rebound to the current model."})
            continue
        targets = {target if nodes.get(target, {}).get("kind") == "component" else owned.get(target)
                   for target in record["targets"]}
        entry = {"component": None, "resource": record["resource"], "status": record["status"], "files": [],
                 "record_revision": digest(record),
                 "declared_revision": record.get("revision"), "declared_code_revision": record.get("code_revision")}
        selected[ident] = entry
        if ident in invalid or None in targets or len(targets) != 1:
            diagnostics.append(_diagnostic("CODE_BINDING_AMBIGUOUS", "Binding does not resolve to one unambiguous component.", ids=(ident,)))
            continue
        component = entry["component"] = next(iter(targets))
        if record["selector"]["kind"] != "whole-document":
            diagnostics.append(_diagnostic("CODE_SELECTOR_UNSUPPORTED", "The Python profile requires whole-document bindings.", ids=(ident, component)))
            continue
        try:
            resource = _resource(record["resource"])
        except ValueError as error:
            diagnostics.append(_diagnostic("CODE_BINDING_RESOURCE_INVALID", str(error), ids=(ident, component)))
            continue
        exact = not resource.endswith("/") and not any(c in resource for c in "*?[")
        if exact and not resource.endswith(".py"):
            advisory.append({"code": "CODE_RESOURCE_OUTSIDE_PROFILE", "binding": ident,
                             "message": "Non-Python binding is outside this observer profile."})
            continue
        matched = entry["files"] = sorted(name for name in sources if _matches(name, resource))
        if not matched or _overlaps_omission(resource, omitted):
            diagnostics.append(_diagnostic("CODE_BINDING_SOURCE_MISSING", "Binding has missing, excluded or unavailable Python coverage.", ids=(ident, component)))
        hashes = {name: sha256(sources[name]).hexdigest() for name in matched}
        if hashes and record["status"] in {"observed", "checked"}:
            actual = "sha256:" + hashes[resource] if exact else digest(hashes)
            if record["revision"] != actual:
                diagnostics.append(_diagnostic("CODE_BINDING_STALE", "Declared resource revision differs from current observed bytes; mapping remains explicit intent only.", ids=(ident, component)))
        for name in matched:
            claims[name].append((ident, component))
    components = {}
    for name, candidates in sorted(claims.items()):
        if len(candidates) != 1:
            diagnostics.append(_diagnostic("CODE_BINDING_AMBIGUOUS", "Multiple binding records claim the same Python source.", file=name,
                                           ids=tuple(ident for ident, _ in candidates)))
        else:
            components[name] = candidates[0][1]
    return selected, components, diagnostics, advisory


def _shortest_path(edges, source, target, reach):
    adjacency = defaultdict(list)
    for edge in edges:
        adjacency[edge["from"]].append(edge)
    for values in adjacency.values():
        values.sort(key=lambda e: (e["to"], e["source_file"], e["line"], e["module"]))
    queue, visited = deque([(source, [])]), {source}
    while queue:
        current, path = queue.popleft()
        for edge in adjacency[current]:
            if edge["to"] == target:
                return path + [edge]
            if reach == "transitive" and edge["to"] not in visited:
                visited.add(edge["to"])
                queue.append((edge["to"], path + [edge]))
    return []


def check_code(path, *, root, implementation, profile, records=None, scope=None, exclude=()):
    """Observe static Python imports; return a separate, non-authoritative envelope."""
    if not isinstance(implementation, str) or not ID.fullmatch(implementation):
        raise ValueError("Select an explicit implementation ID.")
    if scope is not None and (not isinstance(scope, (list, tuple)) or any(not isinstance(i, str) or not ID.fullmatch(i) for i in scope)):
        raise ValueError("Scope must contain rule IDs.")
    if not isinstance(profile, str) or not profile:
        raise ValueError("Select an explicit observer profile.")
    if not isinstance(exclude, (list, tuple)):
        raise ValueError("Exclusions must be a list of relative POSIX globs.")
    exclude = sorted(set(_resource(p) for p in exclude))
    requested = sorted(set(scope or []))
    result = {"format": "gdd-code-observation/0.1", "tool": {"name": "gdd-validator", "version": __version__},
              "runtime": {"python": platform.python_version(), "implementation": platform.python_implementation()},
              "profile": profile, "implementation": implementation, "spec_revision": None, "code_revision": None,
              "scope": requested, "source_root": str(Path(root).absolute()), "exclude": exclude,
              "ignored_directories": sorted(IGNORED_DIRECTORIES), "omitted": [], "files": {}, "bindings": {},
              "imports": [], "edges": [], "constraints": [], "checks": [], "result": "UNKNOWN", "advisory": [],
              "notice": "Static source dependencies under an explicit local import-root assumption; no project code is executed. This observation grants no authority or trusted implementation evidence."}

    def unresolved(code, message):
        result["checks"].append(check("code.inputs", "integration", "UNKNOWN", message, requested, [_diagnostic(code, message)]))
        return result

    if profile != PROFILE:
        return unresolved("CODE_PROFILE_UNSUPPORTED", "Unsupported code observer profile: " + profile)
    bundle = load_bundle(Path(path))
    if bundle.model.get("format") not in {"gdd/0.2", "gdd/0.3"}:
        return unresolved("CODE_MODEL_UNRESOLVED", "A supported model is required.")
    validate_model(bundle)
    result["spec_revision"] = bundle.revision
    if bundle.revision is None or any(c["result"] != "PASS" for c in bundle.checks):
        return unresolved("CODE_MODEL_UNRESOLVED", "The model and its declared assets must resolve and validate before code observation.")
    try:
        _load_records(bundle, records)
        source_root = resolve_local_path(Path(root))
        if not source_root.is_dir():
            raise ValueError("Select an existing Python source root directory.")
    except (OSError, ValueError) as error:
        return unresolved("CODE_INPUT_UNAVAILABLE", str(error))
    result["source_root"] = str(source_root)
    nodes = bundle.model["nodes"]
    candidates = {ident: node["rule"]["constraint"] for ident, node in nodes.items()
                  if node["kind"] == "rule" and "constraint" in node.get("rule", {})}
    if set(requested) - candidates.keys():
        return unresolved("CODE_SCOPE_UNRESOLVED", "Scope must select existing rules with explicit architecture constraints.")
    constraints = [{"rule": ident, **constraint} for ident, constraint in sorted(candidates.items()) if not requested or ident in requested]
    result["scope"] = [c["rule"] for c in constraints]
    result["constraints"] = constraints
    if not constraints:
        return unresolved("CODE_CONSTRAINT_MISSING", "No explicit architecture constraint was selected; no vacuous PASS.")
    sources, inventory_diagnostics, omitted = _inventory(source_root, exclude)
    diagnostics = list(inventory_diagnostics)
    result["omitted"] = omitted
    result["files"] = {name: sha256(raw).hexdigest() for name, raw in sorted(sources.items())}
    result["code_revision"] = digest(result["files"]) if sources else None
    bindings, components, binding_diagnostics, advisory = _bindings(bundle, implementation, sources, omitted)
    result["bindings"] = bindings
    diagnostics.extend(binding_diagnostics)
    result["advisory"].extend(advisory)
    if not sources:
        diagnostics.append(_diagnostic("CODE_SOURCE_UNAVAILABLE", "No Python source bytes were available."))
    for constraint in constraints:
        for endpoint in (constraint["from"], constraint["to"]):
            if endpoint not in components.values():
                diagnostics.append(_diagnostic("CODE_BINDING_SOURCE_MISSING", "Constraint endpoint has no unambiguous Python binding.", ids=(constraint["rule"], endpoint)))
    from .python_imports import analyze_imports, runtime_module
    observed = analyze_imports(sources, components)
    result["imports"], result["edges"] = observed["imports"], observed["edges"]
    diagnostics.extend(observed["diagnostics"])
    omitted_roots = {name.split("/")[0].removesuffix(".py") for name in omitted}
    for observation in result["imports"]:
        module = observation["module"]
        if (observation["external"] and module.split(".")[0] in omitted_roots
                and not runtime_module(module)):
            observation["external"] = False
            uncertain = _diagnostic("CODE_IMPORT_UNRESOLVED", "Import intersects omitted local source; external resolution is not established.",
                                    file=observation["source_file"])
            uncertain["location"].update(line=observation["line"], column=observation["column"])
            diagnostics.append(uncertain)
    after, after_diagnostics, after_omitted = _inventory(source_root, exclude)
    drift = after != sources or after_omitted != omitted or after_diagnostics != inventory_diagnostics
    if drift:
        diagnostics.append(_diagnostic("CODE_SOURCE_CHANGED", "Source inventory/bytes became unresolved or changed during observation."))
    result["advisory"].extend({"code": "CODE_UNCLAIMED_SOURCE", "file": name, "message": "No unambiguous component binding claims this Python source."}
                               for name in sorted(sources.keys() - components.keys()))
    declared = {(edge["from"], edge["to"]) for edge in bundle.model["edges"] if edge["relation"] == "imports"}
    forbidden = {(c["from"], c["to"]) for c in constraints}
    for edge in result["edges"]:
        if edge["from"] != edge["to"] and (edge["from"], edge["to"]) not in declared | forbidden:
            result["advisory"].append({"code": "CODE_UNDECLARED_IMPORT", "from": edge["from"], "to": edge["to"],
                                       "file": edge["source_file"], "line": edge["line"],
                                       "message": "Observed import is not declared; missing imports edges are not a prohibition."})
    for number, constraint in enumerate(constraints):
        supported = constraint["relation"] == "imports" and constraint["policy"] == "forbid" and constraint["reach"] in {"direct", "transitive"}
        path_edges = _shortest_path(result["edges"], constraint["from"], constraint["to"], constraint["reach"]) if supported and not drift else []
        rule_diagnostics = list(diagnostics)
        if not supported:
            rule_diagnostics.append(_diagnostic("CODE_CONSTRAINT_UNSUPPORTED", "Constraint has no supported Python import meaning.", ids=(constraint["rule"],)))
        if path_edges:
            first = path_edges[0]
            violation = _diagnostic("ARCHITECTURE_VIOLATION", f"{constraint['rule']} forbids this {constraint['reach']} import path: " + " -> ".join([constraint["from"]] + [e["to"] for e in path_edges]),
                                    file=first["source_file"], ids=(constraint["rule"], constraint["from"], constraint["to"]))
            violation["severity"] = "error"
            violation["location"].update(line=first["line"], column=first["column"])
            rule_diagnostics.append(violation)
        verdict = "FAIL" if path_edges else "UNKNOWN" if rule_diagnostics else "PASS"
        result["checks"].append(check(f"code.rule.{number}", "integration", verdict,
                                      "Observed static Python imports against an explicit forbid constraint.", [constraint["rule"]],
                                      rule_diagnostics, path=path_edges))
    result["result"] = {0: "PASS", 1: "FAIL", 2: "UNKNOWN"}[exit_code(result)]
    return result
