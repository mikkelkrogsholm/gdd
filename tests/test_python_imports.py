"""Behavioral controls for the bounded, non-executing Python import observer."""

import pytest

from gdd_validator.python_imports import analyze_imports


def analyze(sources, bindings=None):
    return analyze_imports(
        {path: text.encode() for path, text in sources.items()},
        bindings if bindings is not None else {path: path for path in sources},
    )


def pairs(result):
    return {(edge["from"], edge["to"]) for edge in result["edges"]}


def codes(result):
    return {entry["code"] for entry in result["diagnostics"]}


def test_imports_are_static_and_do_not_execute_source():
    result = analyze({"a.py": "raise RuntimeError('never execute')\nif False:\n import b\n", "b.py": ""})
    assert pairs(result) == {("a.py", "b.py")}
    assert not result["diagnostics"]
    assert result["edges"][0]["line"] == 3


def test_regular_package_import_observes_every_parent_initializer():
    result = analyze({"main.py": "import pkg.nested.leaf", "pkg/__init__.py": "",
                      "pkg/nested/__init__.py": "pass", "pkg/nested/leaf.py": ""})
    assert pairs(result) == {("main.py", path) for path in
                             ("pkg/__init__.py", "pkg/nested/__init__.py", "pkg/nested/leaf.py")}
    assert not result["diagnostics"]


def test_relative_imports_resolve_regular_package_and_parent_levels():
    result = analyze({"pkg/__init__.py": "", "pkg/peer.py": "", "pkg/nested/__init__.py": "",
                      "pkg/nested/main.py": "from .. import peer\nfrom . import child",
                      "pkg/nested/child.py": ""})
    assert ("pkg/nested/main.py", "pkg/peer.py") in pairs(result)
    assert ("pkg/nested/main.py", "pkg/nested/child.py") in pairs(result)
    assert not result["diagnostics"]


@pytest.mark.parametrize("source", ["from . import peer", "from ..pkg import peer"])
def test_relative_import_beyond_package_is_unknown(source):
    result = analyze({"main.py": source, "peer.py": ""})
    assert codes(result) == {"CODE_IMPORT_UNRESOLVED"}
    assert not result["edges"]


def test_noninert_initializer_keeps_parent_but_does_not_guess_child():
    result = analyze({"main.py": "from pkg import child", "pkg/__init__.py": "value = 1",
                      "pkg/child.py": ""})
    assert pairs(result) == {("main.py", "pkg/__init__.py")}
    assert codes(result) == {"CODE_IMPORT_UNRESOLVED"}


def test_from_regular_module_import_member_needs_only_module_dependency():
    result = analyze({"main.py": "from other import value", "other.py": "value = 1"})
    assert pairs(result) == {("main.py", "other.py")}
    assert not result["diagnostics"]
    assert result["imports"][0]["imported"] == "value"


@pytest.mark.parametrize("source", ["from pkg import absent", "from pkg import *", "import pkg.absent"])
def test_missing_package_member_and_star_are_unresolved(source):
    result = analyze({"main.py": source, "pkg/__init__.py": ""})
    assert codes(result) == {"CODE_IMPORT_UNRESOLVED"}
    assert ("main.py", "pkg/__init__.py") in pairs(result)


def test_module_package_collision_is_ambiguous():
    result = analyze({"main.py": "import pkg.child", "pkg.py": "", "pkg/__init__.py": "",
                      "pkg/child.py": ""})
    assert not result["edges"]
    assert codes(result) == {"CODE_IMPORT_UNRESOLVED"}


def test_namespace_package_is_outside_profile():
    result = analyze({"main.py": "import pkg.child", "pkg/child.py": ""})
    assert not result["edges"]
    assert codes(result) == {"CODE_IMPORT_UNRESOLVED"}
    assert result["imports"][0]["external"] is False


def test_external_imports_are_reported_without_installation_checks():
    result = analyze({"main.py": "import missing_library\nfrom other_missing import thing"})
    assert not result["edges"]
    assert not result["diagnostics"]
    assert all(row["external"] for row in result["imports"])


@pytest.mark.parametrize("source", [
    "__import__('target')", "import builtins as b\nb.__import__('target')",
    "from builtins import __import__ as load\nload('target')",
    "import importlib as il\nil.import_module('target')",
    "from importlib import import_module as load\nload('target')",
    "import importlib.util as util\nutil.spec_from_file_location('x', 'x.py')",
    "import runpy as run\nrun.run_module('target')",
    "eval('import target')", "exec('import target')",
    "getattr(object, name)('target')",
    "import importlib as il\nload = il.import_module\nload('target')",
    "import importlib\nnamespace = importlib\nnamespace.import_module('target')",
    "import sys as system\nsystem.modules['target']",
])
def test_dynamic_loader_aliases_and_reflection_are_unknown(source):
    result = analyze({"main.py": source})
    assert "CODE_DYNAMIC_IMPORT" in codes(result)


def test_unbound_importer_reaching_bound_code_is_unknown():
    result = analyze({"main.py": "import target", "target.py": ""}, {"target.py": "component.target"})
    assert not result["edges"]
    assert codes(result) == {"CODE_UNBOUND_IMPORT"}


def test_unbound_plain_external_importer_is_advisory_only():
    result = analyze({"main.py": "import os"}, {})
    assert not result["diagnostics"]


def test_unbound_inert_initializer_can_bootstrap_without_edge():
    result = analyze({"main.py": "import pkg.child", "pkg/__init__.py": '"Documentation"\npass',
                      "pkg/child.py": ""}, {"main.py": "a", "pkg/child.py": "b"})
    assert pairs(result) == {("a", "b")}
    assert not result["diagnostics"]
    assert result["inert_files"] == ["pkg/__init__.py"]


def test_unbound_noninert_target_prevents_complete_coverage():
    result = analyze({"main.py": "import helper", "helper.py": "value = 1"}, {"main.py": "a"})
    assert codes(result) == {"CODE_UNBOUND_IMPORT"}


def test_real_component_self_edges_are_preserved():
    result = analyze({"a.py": "import b", "b.py": ""}, {"a.py": "same", "b.py": "same"})
    assert pairs(result) == {("same", "same")}


def test_parse_errors_are_coverage_diagnostics_and_others_still_analyzed():
    result = analyze({"broken.py": "def (", "a.py": "import b", "b.py": ""})
    assert codes(result) == {"CODE_SOURCE_UNPARSEABLE"}
    assert pairs(result) == {("a.py", "b.py")}
    assert result["diagnostics"][0]["location"]["file"] == "broken.py"
    assert result["diagnostics"][0]["location"]["line"] == 1
    assert result["diagnostics"][0]["location"]["column"] is not None


def test_observation_is_deterministic_and_does_not_mutate_input():
    sources = {"b.py": b"import a", "a.py": b"import b\nimport os"}
    components = {"b.py": "b", "a.py": "a"}
    first = analyze_imports(sources, components)
    assert first == analyze_imports(dict(reversed(list(sources.items()))), components)
    assert sources["b.py"] == b"import a"
    assert components == {"b.py": "b", "a.py": "a"}


@pytest.mark.parametrize("source", [
    "import importlib; loader=getattr(importlib, 'import_module'); loader('app.storage')",
    "from importlib import import_module; loaders=[import_module]",
    "import importlib", "import builtins", "import runpy",
    "loader=__builtins__['__import__']; loader('app.storage')",
    "from builtins import __import__ as loader; loaders={'load': loader}",
    "import builtins as b; loader=b.__dict__['__import__']",
    "import sys; registry=getattr(sys, 'modules')",
    "from sys import modules as registry",
    "import sys; hooks=sys.meta_path",
])
def test_recognized_loader_values_and_namespaces_cannot_escape_coverage(source):
    assert "CODE_DYNAMIC_IMPORT" in codes(analyze({"main.py": source}))


@pytest.mark.parametrize("source", ["import sys", "import sys; version=sys.version", "from builtins import len; count=len([])"])
def test_harmless_sys_and_specific_builtin_imports_remain_ordinary_external_dependencies(source):
    assert not analyze({"main.py": source})["diagnostics"]


@pytest.mark.parametrize("module", ["sys", "builtins"])
def test_builtin_finder_prevents_false_source_root_collision_edge(module):
    result = analyze({"main.py": f"import {module}", f"{module}.py": ""})
    assert not result["edges"]
    assert result["imports"][0]["external"] is True
    assert result["imports"][0]["target_file"] is None


def test_invalid_encoding_never_reports_nonpositive_source_positions():
    result = analyze_imports({"bad.py": b"# coding: unknown-codec\n"}, {"bad.py": "bad"})
    assert codes(result) == {"CODE_SOURCE_UNPARSEABLE"}
    position = result["diagnostics"][0]["location"]
    assert position["line"] is None or position["line"] >= 1
    assert position["column"] is None or position["column"] >= 0
