"""Observe a bounded, static Python import graph without loading project code."""

from __future__ import annotations

import ast
import json
from collections import defaultdict


def analyze_imports(sources: dict[str, bytes], components: dict[str, str]) -> dict:
    """Return source dependencies for regular packages rooted at the inventory root.

    Imports in every AST branch count as static dependencies. External module
    availability is deliberately not checked. Diagnostics describe uncertainty,
    not a claim that an import executes or that arbitrary reflection is detected.
    """
    trees: dict[str, ast.Module] = {}
    inert_files: list[str] = []
    imports: list[dict] = []
    edges: list[dict] = []
    diagnostics: list[dict] = []

    def diagnostic(code: str, file: str, node=None, message: str = "") -> None:
        diagnostics.append({
            "code": code, "severity": "warning", "message": message,
            "location": {"file": file, "pointer": "",
                         "line": getattr(node, "lineno", None),
                         "column": getattr(node, "col_offset", None)},
            "related_ids": sorted({components[file]} if file in components else set()),
        })

    for file, data in sorted(sources.items()):
        try:
            tree = ast.parse(data, filename=file)
        except (SyntaxError, ValueError, RecursionError) as exc:
            diagnostic("CODE_SOURCE_UNPARSEABLE", file, exc, str(exc))
            if isinstance(exc, SyntaxError):
                diagnostics[-1]["location"]["column"] = exc.offset - 1 if exc.offset else None
            continue
        trees[file] = tree
        if file.split("/")[-1] == "__init__.py" and all(
            isinstance(node, ast.Pass) or
            (index == 0 and isinstance(node, ast.Expr) and isinstance(node.value, ast.Constant)
             and isinstance(node.value.value, str))
            for index, node in enumerate(tree.body)
        ):
            inert_files.append(file)

    # A single import root, regular packages only. Retain conflicting candidates
    # so a module/package collision cannot silently select one filesystem entry.
    modules: dict[str, list[str]] = defaultdict(list)
    local_roots: set[str] = set()
    file_modules: dict[str, str] = {}
    for file in sorted(sources):
        parts = file.split("/")
        local_roots.add(parts[0][:-3] if len(parts) == 1 else parts[0])
        is_package = parts[-1] == "__init__.py"
        name_parts = parts[:-1] if is_package else parts[:-1] + [parts[-1][:-3]]
        if not name_parts or any(not part.isidentifier() for part in name_parts):
            continue
        if any("/".join(parts[:index] + ["__init__.py"]) not in sources
               for index in range(1, len(parts))):
            continue
        name = ".".join(name_parts)
        modules[name].append(file)
        file_modules[file] = name

    def observe(file: str, node, module: str, imported: str | None,
                target: str | None, external: bool) -> None:
        statement = ast.unparse(node)
        row = {"source_file": file, "line": node.lineno,
               "column": node.col_offset, "module": module,
               "imported": imported, "statement": statement,
               "target_file": target, "external": external}
        imports.append(row)
        if target is None:
            return
        source_component = components.get(file)
        target_component = components.get(target)
        if source_component and target_component:
            edges.append({"from": source_component, "to": target_component,
                          **{key: row[key] for key in
                             ("source_file", "line", "column", "module", "target_file", "statement")}})
        elif target_component or (source_component and target not in inert_files):
            diagnostic("CODE_UNBOUND_IMPORT", file, node,
                       f"Local dependency {target!r} has no complete component mapping.")

    def resolve(file: str, node, module: str, imported: str | None = None,
                *, relative: bool = False) -> str | None:
        if not module:
            diagnostic("CODE_IMPORT_UNRESOLVED", file, node, "Relative import has no regular package context.")
            observe(file, node, module, imported, None, False)
            return None
        if not relative and module.split(".")[0] not in local_roots:
            observe(file, node, module, imported, None, True)
            return None
        parts = module.split(".")
        final = None
        for index in range(1, len(parts) + 1):
            name = ".".join(parts[:index])
            candidates = modules.get(name, [])
            if len(candidates) != 1 or (index < len(parts) and not candidates[0].endswith("/__init__.py")):
                diagnostic("CODE_IMPORT_UNRESOLVED", file, node,
                           f"Local module {name!r} is missing, ambiguous, or outside the regular-package profile.")
                observe(file, node, module, imported, None, False)
                return None
            final = candidates[0]
            observe(file, node, module, imported, final, False)
        return final

    def dotted(node) -> str | None:
        if isinstance(node, ast.Name):
            return node.id
        if isinstance(node, ast.Attribute):
            base = dotted(node.value)
            return f"{base}.{node.attr}" if base else None
        return None

    for file, tree in sorted(trees.items()):
        # Alias recognition is deliberately flow-insensitive: imports in a branch
        # still describe potential static dependencies, even after reassignment.
        aliases = {"__import__": "builtins.__import__", "eval": "builtins.eval", "exec": "builtins.exec"}
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    aliases[alias.asname or alias.name.split(".")[0]] = alias.name if alias.asname else alias.name.split(".")[0]
            elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
                for alias in node.names:
                    aliases[alias.asname or alias.name] = f"{node.module}.{alias.name}"

        def expanded_name(node):
            name = dotted(node)
            if not name:
                return None
            first, *rest = name.split(".")
            return ".".join([aliases.get(first, first), *rest])

        # Follow simple assignment aliases without evaluating expressions. Repeat
        # to support aliases declared after a function that uses them.
        assignments = [node for node in ast.walk(tree) if isinstance(node, (ast.Assign, ast.AnnAssign))]
        for _ in range(len(assignments)):
            changed = False
            for assignment in assignments:
                name = expanded_name(assignment.value)
                targets = assignment.targets if isinstance(assignment, ast.Assign) else [assignment.target]
                if name and name.startswith(("importlib", "builtins", "runpy", "sys.modules")):
                    for target in targets:
                        if isinstance(target, ast.Name) and aliases.get(target.id) != name:
                            aliases[target.id] = name
                            changed = True
            if not changed:
                break

        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    resolve(file, node, alias.name)
            elif isinstance(node, ast.ImportFrom):
                module = node.module or ""
                if node.level:
                    own = file_modules.get(file, "")
                    package = own if file.endswith("/__init__.py") else own.rpartition(".")[0]
                    parts = package.split(".") if package else []
                    if node.level > len(parts):
                        module = ""
                    else:
                        module = ".".join(parts[:len(parts) - node.level + 1] + ([module] if module else []))
                for alias in node.names:
                    target = resolve(file, node, module, alias.name, relative=bool(node.level))
                    if target and target.endswith("/__init__.py"):
                        child = f"{module}.{alias.name}"
                        if target in inert_files and alias.name != "*" and len(modules.get(child, [])) == 1:
                            resolve(file, node, child, alias.name, relative=True)
                        else:
                            diagnostic("CODE_IMPORT_UNRESOLVED", file, node,
                                       f"Package member {child!r} cannot be resolved without executing its initializer.")
            elif isinstance(node, ast.Call):
                name = expanded_name(node.func)
                reflected = isinstance(node.func, ast.Call) and isinstance(node.func.func, ast.Name) and node.func.func.id == "getattr"
                if (name in {"builtins.__import__", "builtins.eval", "builtins.exec", "importlib.import_module", "importlib.reload", "importlib.util.module_from_spec", "importlib.util.spec_from_file_location", "runpy.run_module", "runpy.run_path"}
                    or (name and (name.startswith("importlib.machinery.") or name.endswith(".exec_module") or name.endswith(".load_module")))
                    or reflected):
                    diagnostic("CODE_DYNAMIC_IMPORT", file, node,
                               "Dynamic loading or reflection requires analysis outside this static import profile.")
            elif isinstance(node, ast.Attribute) and expanded_name(node) == "sys.modules":
                diagnostic("CODE_DYNAMIC_IMPORT", file, node,
                           "Access to the module registry is outside this static import profile.")

    def unique_sorted(rows: list[dict]) -> list[dict]:
        return [row for _, row in sorted({json.dumps(row, sort_keys=True): row for row in rows}.items())]

    return {"imports": unique_sorted(imports), "edges": unique_sorted(edges),
            "diagnostics": unique_sorted(diagnostics), "inert_files": sorted(inert_files)}
