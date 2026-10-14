"""Host-selected Python AST import check; never execute graph or source text.

This bounded profile covers the concert_app package with exact component/file
bindings. It establishes observed static module imports, not every dependency
that arbitrary Python reflection could create at runtime.
"""

import argparse
import ast
from collections import defaultdict, deque
from hashlib import sha256
import json
from pathlib import Path
import shutil
import tempfile

from gdd_validator.loading import digest, load_bundle
from gdd_validator.model import validate_model


CHECKER = {"name": "concert-python-import-graph", "version": "0.1.0"}
PROFILE = "concert-python-component-imports/0.1"
DEFAULT_BINDINGS = {
    "component.api": ["concert_app/api.py", "concert_app/__main__.py"],
    "component.identity": ["concert_app/identity.py"],
    "component.reservations": ["concert_app/reservations.py"],
    "component.storage": ["concert_app/storage.py"],
}


def _module(path):
    parts = list(path.with_suffix("").parts)
    if parts[-1] == "__init__":
        parts.pop()
    return ".".join(parts)


def _dynamic_calls(tree):
    """Recognize direct calls and ordinary imported/assigned callable aliases."""
    functions = {"__import__", "exec", "eval"}
    module_aliases = {"__builtins__": "builtins"}
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name.split(".")[0] in {"importlib", "builtins"}:
                    module_aliases[alias.asname or alias.name.split(".")[0]] = alias.name.split(".")[0]
        elif isinstance(node, ast.ImportFrom) and node.level == 0:
            for alias in node.names:
                if ((node.module == "importlib" and alias.name == "import_module")
                        or (node.module == "builtins" and alias.name in {"__import__", "exec", "eval"})):
                    functions.add(alias.asname or alias.name)

    def module_ref(value):
        return module_aliases.get(value.id) if isinstance(value, ast.Name) else None

    def dynamic_ref(value):
        if isinstance(value, ast.Name):
            return value.id in functions
        if isinstance(value, ast.Attribute):
            module = module_ref(value.value)
            return ((module == "importlib" and value.attr == "import_module")
                    or (module == "builtins" and value.attr in {"__import__", "exec", "eval"}))
        if isinstance(value, ast.Subscript) and module_ref(value.value) == "builtins":
            return isinstance(value.slice, ast.Constant) and value.slice.value in {"__import__", "exec", "eval"}
        if isinstance(value, ast.Call) and isinstance(value.func, ast.Name) and value.func.id == "getattr" and value.args:
            return module_ref(value.args[0]) in {"importlib", "builtins"}
        return False

    while True:
        before = (set(functions), dict(module_aliases))
        for node in ast.walk(tree):
            targets, value = [], None
            if isinstance(node, ast.Assign):
                targets, value = node.targets, node.value
            elif isinstance(node, (ast.AnnAssign, ast.NamedExpr)):
                targets, value = [node.target], node.value
            for target in targets:
                if isinstance(target, ast.Name) and value is not None:
                    if dynamic_ref(value):
                        functions.add(target.id)
                    if module_ref(value):
                        module_aliases[target.id] = module_ref(value)
        if before == (set(functions), dict(module_aliases)):
            break
    sites = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Call) and dynamic_ref(node.func):
            sites.append((node, ast.unparse(node.func)))
        elif isinstance(node, ast.Name) and isinstance(node.ctx, ast.Load) and node.id in functions:
            # Passing/packing a loader escapes simple alias analysis. Its presence
            # cannot establish absence of dynamic imports in this bounded profile.
            sites.append((node, node.id))
        elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module in {"importlib", "builtins"} and any(a.name == "*" for a in node.names):
            sites.append((node, ast.unparse(node)))
    return sites


def scan(root: Path, model_path: Path | None = None, bindings: dict | None = None) -> dict:
    """Scan current bytes and explicit forbid-import constraints without imports."""
    root = Path(root).resolve()
    model_path = Path(model_path) if model_path is not None else root / "gdd.json"
    if not model_path.is_absolute():
        model_path = root / model_path
    report = {
        "format": "concert-import-scan/0.1", "checker": dict(CHECKER), "profile": PROFILE,
        "method": "python_import_graph", "result": "UNKNOWN", "executed": True,
        "spec_revision": None, "code_revision": None, "scope": [], "bindings": {},
        "files": {}, "imports": [], "edges": [], "constraints": [], "violations": [], "unknown": [],
        "notice": "Observed static imports only; graph/source text is never executed and this scan alone grants no GDD acceptance.",
    }

    def unknown(reason, **location):
        report["unknown"].append({"reason": reason, **location})

    nodes = {}
    try:
        bundle = load_bundle(model_path)
        report["spec_revision"] = bundle.revision
        nodes = bundle.model.get("nodes", {})
        if bundle.model.get("format") not in {"gdd/0.2", "gdd/0.3"}:
            unknown("A supported GDD model is required.")
        else:
            validate_model(bundle)
        for check in bundle.checks:
            if check["dimension"] == "model" and check["result"] != "PASS":
                unknown("Model input is unresolved or invalid: " + check["reason"])
        for ident, node in nodes.items():
            rule = node.get("rule", {})
            constraint = rule.get("constraint")
            if not constraint:
                continue
            if (rule.get("category") == "architecture" and constraint.get("relation") == "imports"
                    and constraint.get("policy") == "forbid" and constraint.get("reach") in {"direct", "transitive"}
                    and all(nodes.get(constraint.get(k), {}).get("kind") == "component" for k in ("from", "to"))):
                report["constraints"].append({"rule": ident, **constraint})
            else:
                unknown("Constraint has no supported Python component-import meaning.", rule=ident)
    except (OSError, ValueError, KeyError, TypeError) as error:
        unknown("Model could not be resolved: " + str(error))
    report["scope"] = sorted(c["rule"] for c in report["constraints"])
    if report["spec_revision"] is None:
        unknown("The model snapshot revision is unresolved.")
    if not report["constraints"]:
        unknown("No supported explicit forbid-import constraint was supplied.")

    raw_bindings = DEFAULT_BINDINGS if bindings is None else bindings
    file_candidates = defaultdict(list)
    physical_candidates = defaultdict(list)
    if not isinstance(raw_bindings, dict):
        unknown("Bindings must be an explicit component-to-file mapping.")
        raw_bindings = {}
    for component, resources in raw_bindings.items():
        if nodes.get(component, {}).get("kind") != "component":
            unknown("Binding does not name a model component.", component=str(component))
            continue
        if isinstance(resources, str):
            resources = [resources]
        if not isinstance(resources, (list, tuple)) or not resources or any(not isinstance(p, str) for p in resources):
            unknown("Binding requires at least one exact Python path.", component=component)
            continue
        report["bindings"][component] = list(resources)
        for relative in resources:
            path = Path(relative)
            if path.is_absolute() or ".." in path.parts or path.suffix != ".py":
                unknown("Binding must be an exact in-root Python path.", component=component, source_file=relative)
                continue
            resolved = (root / path).resolve()
            if not resolved.is_relative_to(root):
                unknown("Binding resolves outside the source root.", component=component, source_file=relative)
                continue
            if not resolved.is_file():
                unknown("A bound source file is missing.", component=component, source_file=relative)
                continue
            if not path.parts or path.parts[0] != "concert_app":
                unknown("This profile binds only concert_app package files.", component=component, source_file=relative)
                continue
            file_candidates[path.as_posix()].append(component)
            try:
                identity = resolved.stat()
                physical_candidates[(identity.st_dev, identity.st_ino)].append(path.as_posix())
            except OSError as error:
                unknown("Bound source identity is unavailable: " + str(error), source_file=relative)

    components = {}
    overlapping_files = set()
    for paths in physical_candidates.values():
        if len(paths) > 1:
            overlapping_files.update(paths)
            unknown("Bindings overlap the same physical source file.", source_files=paths)
    for relative, candidates in file_candidates.items():
        if len(candidates) != 1 or relative in overlapping_files:
            unknown("Source file has duplicate or overlapping bindings.", source_file=relative, components=candidates)
        else:
            components[relative] = candidates[0]
    for constraint in report["constraints"]:
        for component in (constraint["from"], constraint["to"]):
            if component not in components.values():
                unknown("Constraint endpoint has no unambiguous existing binding.", component=component, rule=constraint["rule"])

    package = root / "concert_app"
    if not package.is_dir():
        unknown("The concert_app package is unavailable.")
    trees, modules = {}, {}
    for path in sorted(package.rglob("*.py")) if package.is_dir() else []:
        relative = path.relative_to(root).as_posix()
        if not path.resolve().is_relative_to(root):
            unknown("Package source resolves outside the source root.", source_file=relative)
            continue
        try:
            raw = path.read_bytes()
            report["files"][relative] = sha256(raw).hexdigest()
            tree = ast.parse(raw, filename=relative)
            trees[relative] = tree
            module = _module(Path(relative))
            if module in modules:
                unknown("Python module name has multiple source files.", module=module)
                modules[module] = None
            else:
                modules[module] = relative
            if relative not in components:
                bootstrap = relative == "concert_app/__init__.py" and all(
                    isinstance(n, ast.Pass) or (isinstance(n, ast.Expr) and isinstance(n.value, ast.Constant)
                                               and isinstance(n.value.value, str)) for n in tree.body)
                if not bootstrap:
                    unknown("Local Python source has no unambiguous component binding.", source_file=relative)
        except (OSError, SyntaxError, ValueError) as error:
            unknown("Source cannot be parsed: " + str(error), source_file=relative)
    if "concert_app/__init__.py" not in trees:
        unknown("Known package bootstrap __init__.py is missing or unparseable.")
    if report["files"]:
        report["code_revision"] = digest(report["files"])

    def record_import(relative, node, target, imported):
        local = target == "concert_app" or target.startswith("concert_app.")
        observed = {"source_file": relative, "line": node.lineno,
                    "source_sha256": report["files"][relative], "module": target, "imported": imported,
                    "statement": ast.unparse(node), "local": local}
        report["imports"].append(observed)
        if not local:
            # Bare sibling imports depend on host sys.path; do not call them external proof.
            if target in {name.rsplit(".", 1)[-1] for name in modules if name != "concert_app"}:
                unknown("Bare import may name a local sibling; resolution is outside this profile.", source_file=relative, line=node.lineno, module=target)
            return
        destination = modules.get(target)
        if destination is None:
            unknown("Local imported module is missing or ambiguous.", source_file=relative, line=node.lineno, module=target)
            return
        if target == "concert_app" and destination not in components:
            return  # Its inspected, unbound bootstrap must be empty; otherwise UNKNOWN was recorded.
        consumer, provider = components.get(relative), components.get(destination)
        if not consumer or not provider:
            unknown("Local import lacks unambiguous endpoint bindings.", source_file=relative, line=node.lineno, module=target)
            return
        report["edges"].append({"from": consumer, "to": provider, **observed})

    for relative, tree in trees.items():
        current = _module(Path(relative))
        package_name = current if relative.endswith("/__init__.py") else current.rsplit(".", 1)[0]
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    record_import(relative, node, alias.name, alias.name)
            elif isinstance(node, ast.ImportFrom):
                if node.level:
                    parts = package_name.split(".")
                    if node.level > len(parts):
                        unknown("Relative import escapes the known package.", source_file=relative, line=node.lineno)
                        continue
                    base = ".".join(parts[:len(parts) - node.level + 1])
                    if node.module:
                        base += "." + node.module
                else:
                    base = node.module or ""
                if base == "concert_app" or (modules.get(base) or "").endswith("/__init__.py"):
                    for alias in node.names:
                        if alias.name == "*":
                            unknown("Package wildcard imports are outside this profile.", source_file=relative, line=node.lineno)
                            record_import(relative, node, base, alias.name)
                        else:
                            record_import(relative, node, base + "." + alias.name, alias.name)
                else:
                    record_import(relative, node, base, ", ".join(a.name for a in node.names))
        for node, expression in _dynamic_calls(tree):
            unknown("Dynamic importing or source execution is outside this static profile.", source_file=relative,
                    line=node.lineno, expression=expression)

    adjacency = defaultdict(list)
    for edge in report["edges"]:
        adjacency[edge["from"]].append(edge)
    for constraint in report["constraints"]:
        if constraint["reach"] == "direct":
            paths = [[e] for e in adjacency[constraint["from"]] if e["to"] == constraint["to"]]
        else:
            paths, queue, visited = [], deque([(constraint["from"], [])]), {constraint["from"]}
            while queue:
                component, path = queue.popleft()
                for edge in adjacency[component]:
                    if edge["to"] == constraint["to"]:
                        paths.append(path + [edge])
                    elif edge["to"] not in visited:
                        visited.add(edge["to"])
                        queue.append((edge["to"], path + [edge]))
        report["violations"].extend({"rule": constraint["rule"], "constraint": constraint, "path": path} for path in paths)
    report["result"] = "FAIL" if report["violations"] else "UNKNOWN" if report["unknown"] else "PASS"
    return report


def run_controls(root: Path) -> dict:
    """Inject and repair actual copied source bytes; never mutate the input app."""
    root = Path(root).resolve()
    with tempfile.TemporaryDirectory(prefix="concert-import-controls-") as temporary:
        copied = Path(temporary) / "app"
        shutil.copytree(root, copied, ignore=shutil.ignore_patterns("__pycache__", ".pytest_cache", ".git", ".venv", "evidence", "runs"))
        api, storage = copied / "concert_app/api.py", copied / "concert_app/storage.py"
        original = api.read_bytes()
        controls = {"clean": scan(copied)}
        api.write_bytes(original + b"\nfrom . import storage\n")
        controls["injected"] = scan(copied)
        api.write_bytes(original)
        controls["repaired"] = scan(copied)
        storage.unlink()
        controls["missing"] = scan(copied)
    expected = {"clean": "PASS", "injected": "FAIL", "repaired": "PASS", "missing": "UNKNOWN"}
    matched = (all(controls[key]["result"] == value for key, value in expected.items())
               and controls["clean"]["code_revision"] == controls["repaired"]["code_revision"]
               and controls["clean"]["code_revision"] != controls["injected"]["code_revision"]
               and controls["clean"]["code_revision"] != controls["missing"]["code_revision"])
    return {"format": "concert-import-controls/0.1", "checker": dict(CHECKER), "profile": PROFILE,
            "matched": matched, "expected": expected, "controls": controls}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--model", type=Path)
    parser.add_argument("--controls", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = run_controls(args.root) if args.controls else scan(args.root, args.model)
    output = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(output, encoding="utf-8")
    else:
        print(output, end="")
    raise SystemExit((0 if result["matched"] else 1) if args.controls else {"PASS": 0, "FAIL": 1, "UNKNOWN": 2}[result["result"]])
