"""Real AST inputs and injected source controls; no application execution claims."""

from hashlib import sha256
import importlib.util
import json
import os
from pathlib import Path

import pytest


CHECKER_PATH = Path(__file__).resolve().parents[1] / "tools/import_check.py"
spec = importlib.util.spec_from_file_location("concert_import_checker", CHECKER_PATH)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


@pytest.fixture
def app(tmp_path):
    package = tmp_path / "concert_app"
    package.mkdir()
    sources = {
        "__init__.py": '"""Empty bootstrap."""\n',
        "api.py": "import json\nfrom . import reservations, identity\n",
        "__main__.py": "from .api import make_server\n",
        "identity.py": "import secrets\n",
        "reservations.py": "from .storage import transaction\n",
        "storage.py": "import sqlite3\n",
    }
    for filename, text in sources.items():
        (package / filename).write_text(text, encoding="utf-8")
    model = {
        "format": "gdd/0.3", "project": {"id": "example.imports", "title": "Import checks", "goal": "Observe component imports."},
        "nodes": {component: {"kind": "component", "description": component} for component in checker.DEFAULT_BINDINGS},
        "edges": [{"id": "edge.constraint", "from": "rule.api_boundary", "relation": "constrains", "to": "component.api"}],
    }
    model["nodes"]["rule.api_boundary"] = {
        "kind": "rule", "description": "API must not directly import storage.",
        "rule": {"category": "architecture", "statement": "API must not directly import storage.", "scope": "targeted", "blocking": True,
                 "verification": ["python_import_graph"], "constraint": {"relation": "imports", "from": "component.api", "to": "component.storage", "reach": "direct", "policy": "forbid"}},
    }
    (tmp_path / "gdd.json").write_text(json.dumps(model), encoding="utf-8")
    return tmp_path


def append(app, source, filename="api.py"):
    path = app / "concert_app" / filename
    path.write_text(path.read_text() + "\n" + source + "\n", encoding="utf-8")


def modify_model(app, change):
    path = app / "gdd.json"
    model = json.loads(path.read_text())
    change(model)
    path.write_text(json.dumps(model), encoding="utf-8")


def test_allowed_indirect_path_and_multiple_api_files(app):
    result = checker.scan(app)
    assert result["result"] == "PASS", result
    assert {(edge["from"], edge["to"]) for edge in result["edges"]} >= {
        ("component.api", "component.reservations"), ("component.reservations", "component.storage"),
        ("component.api", "component.api"),
    }
    assert result["spec_revision"] and result["code_revision"]
    assert result["violations"] == result["unknown"] == []
    for edge in result["edges"]:
        assert edge["source_sha256"] == sha256((app / edge["source_file"]).read_bytes()).hexdigest()
        assert edge["line"] > 0
    assert not any(e["module"] in {"json", "sqlite3", "secrets"} for e in result["edges"])


@pytest.mark.parametrize("statement", [
    "import concert_app.storage", "import concert_app.storage as db", "from concert_app import storage",
    "from . import storage", "from .storage import transaction", "from concert_app.storage import transaction as tx",
    "def hidden():\n    from . import storage", "if False:\n    from .storage import transaction",
])
def test_forbidden_import_forms_and_nested_imports(app, statement):
    append(app, statement)
    result = checker.scan(app)
    assert result["result"] == "FAIL"
    assert any(v["rule"] == "rule.api_boundary" and v["path"][-1]["to"] == "component.storage" for v in result["violations"])


def test_main_is_same_api_component_not_unmapped_file(app):
    append(app, "from .storage import transaction", "__main__.py")
    result = checker.scan(app)
    assert result["result"] == "FAIL"
    assert any(v["path"][0]["source_file"].endswith("__main__.py") for v in result["violations"])


def test_transitive_constraint_checks_observed_path(app):
    modify_model(app, lambda m: m["nodes"]["rule.api_boundary"]["rule"]["constraint"].update(reach="transitive"))
    result = checker.scan(app)
    assert result["result"] == "FAIL"
    assert any([e["to"] for e in v["path"]] == ["component.reservations", "component.storage"] for v in result["violations"])


@pytest.mark.parametrize("source", [
    "__import__('concert_app.storage')", "exec('import concert_app.storage')", "eval('1')",
    "import importlib as il\nil.import_module('concert_app.storage')",
    "from importlib import import_module as load\nload('concert_app.storage')",
    "from builtins import __import__ as load\nload('concert_app.storage')",
    "from builtins import exec as run\nrun('import concert_app.storage')",
    "import builtins as b\nb.__import__('concert_app.storage')",
    "alias = __import__\nalias('concert_app.storage')",
    "import importlib as il\nfirst = il\nsecond = first.import_module\nsecond('concert_app.storage')",
    "import importlib\ngetattr(importlib, 'import_module')('concert_app.storage')",
    "__builtins__['__import__']('concert_app.storage')",
    "def hidden():\n    import importlib as il\n    il.import_module('concert_app.storage')",
    "loaders = {'load': __import__}\nloaders['load']('concert_app.storage')",
    "from importlib import *\nimport_module('concert_app.storage')",
])
def test_unsupported_dynamic_import_and_execution_aliases_are_unknown(app, source):
    append(app, source)
    result = checker.scan(app)
    assert result["result"] == "UNKNOWN"
    assert any("Dynamic" in item["reason"] for item in result["unknown"])


def test_known_violation_wins_over_unknown(app):
    append(app, "from . import storage\n__import__('unresolved')")
    result = checker.scan(app)
    assert result["result"] == "FAIL" and result["violations"] and result["unknown"]


@pytest.mark.parametrize("missing", ["storage.py", "__main__.py", "__init__.py"])
def test_missing_existing_source_never_passes(app, missing):
    (app / "concert_app" / missing).unlink()
    assert checker.scan(app)["result"] == "UNKNOWN"


def test_missing_model_never_passes(app):
    assert checker.scan(app, Path("missing.json"))["result"] == "UNKNOWN"


def test_no_constraints_is_not_vacuous_pass(app):
    modify_model(app, lambda m: (m["nodes"].pop("rule.api_boundary"), m.update(edges=[])))
    assert checker.scan(app)["result"] == "UNKNOWN"


def test_invalid_model_remains_unknown(app):
    modify_model(app, lambda m: m["nodes"]["rule.api_boundary"]["rule"]["constraint"].update(reach="magic"))
    assert checker.scan(app)["result"] == "UNKNOWN"


def test_missing_component_mapping_is_unknown(app):
    bindings = dict(checker.DEFAULT_BINDINGS)
    bindings.pop("component.storage")
    assert checker.scan(app, bindings=bindings)["result"] == "UNKNOWN"


def test_overlapping_mapping_does_not_choose_provider(app):
    bindings = {key: list(value) for key, value in checker.DEFAULT_BINDINGS.items()}
    bindings["component.identity"].append("concert_app/storage.py")
    result = checker.scan(app, bindings=bindings)
    assert result["result"] == "UNKNOWN"
    assert any("overlapping" in item["reason"] for item in result["unknown"])


def test_unseen_local_python_file_and_import_are_unknown(app):
    (app / "concert_app" / "helper.py").write_text("import json\n")
    append(app, "from . import helper")
    assert checker.scan(app)["result"] == "UNKNOWN"


def test_nonempty_unbound_bootstrap_is_unknown(app):
    append(app, "from . import storage", "__init__.py")
    append(app, "import concert_app")
    assert checker.scan(app)["result"] == "UNKNOWN"


def test_missing_local_import_is_unknown(app):
    append(app, "import concert_app.missing")
    assert checker.scan(app)["result"] == "UNKNOWN"


def test_package_wildcard_is_unknown(app):
    append(app, "from . import *")
    assert checker.scan(app)["result"] == "UNKNOWN"


def test_unparseable_source_is_unknown(app):
    append(app, "def broken(")
    assert checker.scan(app)["result"] == "UNKNOWN"


@pytest.mark.parametrize("resource", ["../outside.py", "/tmp/outside.py", "concert_app/*.py"])
def test_invalid_binding_paths_are_unknown(app, resource):
    bindings = dict(checker.DEFAULT_BINDINGS)
    bindings["component.storage"] = [resource]
    assert checker.scan(app, bindings=bindings)["result"] == "UNKNOWN"


def test_symlink_binding_cannot_read_outside_root(app):
    outside = app.parent / "outside.py"
    outside.write_text("from . import storage\n")
    (app / "concert_app/storage.py").unlink()
    (app / "concert_app/storage.py").symlink_to(outside)
    result = checker.scan(app)
    assert result["result"] == "UNKNOWN"
    assert "concert_app/storage.py" not in result["files"]


def test_symlink_bindings_cannot_assign_one_physical_file_twice(app):
    (app / "concert_app/api.py").unlink()
    (app / "concert_app/api.py").symlink_to(app / "concert_app/storage.py")
    result = checker.scan(app)
    assert result["result"] == "UNKNOWN"
    assert any("physical" in item["reason"] for item in result["unknown"])


def test_hardlink_bindings_cannot_assign_one_physical_file_twice(app):
    (app / "concert_app/api.py").unlink()
    os.link(app / "concert_app/storage.py", app / "concert_app/api.py")
    result = checker.scan(app)
    assert result["result"] == "UNKNOWN"
    assert any("physical" in item["reason"] for item in result["unknown"])


def test_negative_controls_scan_actual_copied_bytes_and_preserve_input(app):
    before = {p.relative_to(app).as_posix(): p.read_bytes() for p in app.rglob("*") if p.is_file()}
    result = checker.run_controls(app)
    assert result["matched"], result
    assert {key: report["result"] for key, report in result["controls"].items()} == {
        "clean": "PASS", "injected": "FAIL", "repaired": "PASS", "missing": "UNKNOWN"}
    assert before == {p.relative_to(app).as_posix(): p.read_bytes() for p in app.rglob("*") if p.is_file()}
