"""Public CLI, observation schema and source integrity controls."""

from hashlib import sha256
import json
from pathlib import Path
import subprocess
import sys

from jsonschema import Draft202012Validator
import pytest

from gdd_validator import check_code
from gdd_validator.loading import load_bundle, schema


@pytest.fixture
def project(tmp_path):
    root = tmp_path / "source"
    root.mkdir()
    (root / "api.py").write_text("pass\n")
    (root / "store.py").write_text("pass\n")
    model = tmp_path / "gdd.json"
    document = {
        "format": "gdd/0.3", "project": {"id": "project.cli", "title": "CLI", "goal": "Observe imports"},
        "record_files": ["records.json"],
        "nodes": {
            "component.api": {"kind": "component", "description": "API"},
            "component.store": {"kind": "component", "description": "Store"},
            "rule.boundary": {"kind": "rule", "description": "Boundary", "rule": {
                "category": "architecture", "statement": "API must not import storage directly.",
                "scope": "global", "blocking": True, "verification": ["python_imports"],
                "constraint": {"relation": "imports", "from": "component.api", "to": "component.store",
                               "reach": "direct", "policy": "forbid"}}},
        }, "edges": [],
    }
    model.write_text(json.dumps(document))
    records = tmp_path / "records.json"
    records.write_text(json.dumps({"format": "gdd-records/0.2", "project_id": "project.cli", "records": {}}))
    revision = load_bundle(model).revision
    records.write_text(json.dumps({"format": "gdd-records/0.2", "project_id": "project.cli", "records": {
        "binding." + name: {"kind": "binding", "spec_revision": revision,
                           "targets": ["component." + name], "implementation": "implementation.cli", "role": "code",
                           "resource": name + ".py", "selector": {"kind": "whole-document"}, "status": "planned"}
        for name in ("api", "store")}}))
    return model, root


def observe(project, **options):
    model, root = project
    return check_code(model, root=root, implementation="implementation.cli", profile="python-imports/0.1", **options)


def cli(project, *args):
    model, root = project
    return subprocess.run([sys.executable, "-m", "gdd_validator", "check-code", str(model), "--root", str(root),
                           "--implementation", "implementation.cli", "--profile", "python-imports/0.1", *args],
                          capture_output=True, text=True)


@pytest.mark.parametrize("source,expected", [("pass\n", "PASS"), ("import store\n", "FAIL"),
                                             ("from . import store\n", "UNKNOWN"),
                                             ("def invalid(\n", "UNKNOWN")])
def test_observations_follow_their_own_schema(project, source, expected):
    (project[1] / "api.py").write_text(source)
    observation = observe(project)
    Draft202012Validator(schema("code-observation")).validate(observation)
    assert observation["result"] == expected
    assert observation["format"] != "gdd-report/0.2"
    assert observation["runtime"]["python"]
    assert all("evidence_refs" not in c for c in observation["checks"])


@pytest.mark.parametrize("source,expected", [("pass\n", 0), ("import store\n", 1), ("import importlib\n", 2)])
def test_cli_result_and_exclusive_output(project, tmp_path, source, expected):
    (project[1] / "api.py").write_text(source)
    output = tmp_path / "observation.json"
    execution = cli(project, "--output", str(output))
    assert execution.returncode == expected, execution.stderr
    assert not execution.stdout
    observation = json.loads(output.read_text())
    assert observation["result"] == {0: "PASS", 1: "FAIL", 2: "UNKNOWN"}[expected]
    original = output.read_bytes()
    repeated = cli(project, "--output", str(output))
    assert repeated.returncode == 1 and output.read_bytes() == original


def test_cli_requires_profile_and_unknown_profile_is_structured(project):
    model, root = project
    command = [sys.executable, "-m", "gdd_validator", "check-code", str(model), "--root", str(root),
               "--implementation", "implementation.cli"]
    missing = subprocess.run(command, capture_output=True, text=True)
    assert missing.returncode == 2 and "--profile" in missing.stderr
    unknown = subprocess.run(command + ["--profile", "unsupported/9"], capture_output=True, text=True)
    assert unknown.returncode == 2
    observation = json.loads(unknown.stdout)
    Draft202012Validator(schema("code-observation")).validate(observation)
    assert observation["checks"][0]["diagnostics"][0]["code"] == "CODE_PROFILE_UNSUPPORTED"


def test_cli_refuses_dangling_output_symlink(project, tmp_path):
    target = tmp_path / "missing.json"
    output = tmp_path / "alias.json"
    output.symlink_to(target)
    execution = cli(project, "--output", str(output))
    assert execution.returncode == 1 and output.is_symlink() and not target.exists()


def test_source_inventory_drift_cannot_establish_a_witness(project, monkeypatch):
    import gdd_validator.python_imports as observer
    original = observer.analyze_imports

    def changed(sources, components):
        result = original(sources, components)
        (project[1] / "api.py").write_text("pass\n")
        return result

    (project[1] / "api.py").write_text("import store\n")
    monkeypatch.setattr(observer, "analyze_imports", changed)
    observation = observe(project)
    assert observation["result"] == "UNKNOWN"
    assert all(not c.get("path") for c in observation["checks"])
    assert "CODE_SOURCE_CHANGED" in {d["code"] for c in observation["checks"] for d in c["diagnostics"]}


def test_hardlinked_binding_alias_is_ambiguous(project):
    root = project[1]
    (root / "store.py").unlink()
    (root / "store.py").hardlink_to(root / "api.py")
    observation = observe(project)
    assert observation["result"] == "UNKNOWN"
    assert "CODE_BINDING_AMBIGUOUS" in {d["code"] for c in observation["checks"] for d in c["diagnostics"]}


def test_source_snapshot_hashes_current_bytes_and_binding_contents(project):
    observation = observe(project)
    assert observation["files"]["api.py"] == sha256((project[1] / "api.py").read_bytes()).hexdigest()
    assert observation["bindings"]["binding.api"]["record_revision"].startswith("sha256:")
    assert not list(project[1].glob("__pycache__"))


def test_excluding_a_known_local_import_cannot_turn_unknown_into_pass(project):
    root = project[1]
    (root / "api.py").write_text("import helper\n")
    (root / "helper.py").write_text("import store\n")
    assert observe(project)["result"] == "UNKNOWN"
    excluded = observe(project, exclude=["helper.py"])
    assert excluded["result"] == "UNKNOWN"
    assert "CODE_IMPORT_UNRESOLVED" in {d["code"] for c in excluded["checks"] for d in c["diagnostics"]}


def test_unrelated_exclusion_does_not_prevent_profile_coverage(project):
    (project[1] / "unrelated.py").write_text("pass\n")
    assert observe(project, exclude=["unrelated.py"])["result"] == "PASS"


def test_explicit_legacy_model_uses_the_same_observer_without_migration(project):
    model, root = project
    document = json.loads(model.read_text())
    document['format'] = 'gdd/0.2'
    model.write_text(json.dumps(document))
    records_path = model.parent / 'records.json'
    records = json.loads(records_path.read_text())
    revision = load_bundle(model).revision
    for record in records['records'].values():
        record['spec_revision'] = revision
    records_path.write_text(json.dumps(records))
    assert observe(project)['result'] == 'PASS'
    (root / 'api.py').write_text('import store\n')
    assert observe(project)['result'] == 'FAIL'
