"""Independent public-API controls for bounded Python source observation.

These fixtures contain mapping intent, never trusted execution evidence. The
concert control copies only application files and preserves historical inputs.
"""

from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path

import pytest

from gdd_validator.code_check import check_code
from gdd_validator.loading import load_bundle

ROOT = Path(__file__).resolve().parents[1]
PROFILE = "python-imports/0.1"
IMPLEMENTATION = "implementation.control"


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def control(tmp_path, sources=None, *, reach="direct", origin="component.front", target="component.store"):
    root = tmp_path / "source"
    root.mkdir()
    sources = sources if sources is not None else {"pkg/__init__.py": '"""Inert bootstrap."""\n',
                                                  "pkg/front.py": "VALUE = 1\n", "pkg/store.py": "VALUE = 2\n"}
    for name, source in sources.items():
        destination = root / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(source)
    model = {"format": "gdd/0.3", "project": {"id": "project.import-control", "title": "Import control", "goal": "Observe explicit Python import boundaries."},
             "nodes": {}, "edges": [], "record_files": ["mapping.records.json"]}
    for name in ("front", "middle", "store"):
        model["nodes"]["component." + name] = {"kind": "component", "description": "Own " + name + " operations."}
    model["nodes"]["rule.boundary"] = {"kind": "rule", "description": "Enforce an explicit import boundary.",
        "rule": {"category": "architecture", "statement": "The selected import dependency is forbidden.",
                 "scope": "global", "blocking": True, "verification": ["python_import_graph"],
                 "constraint": {"relation": "imports", "from": origin, "to": target, "reach": reach, "policy": "forbid"}}}
    path = tmp_path / "model.json"
    write(path, model)
    records = {"format": "gdd-records/0.2", "project_id": model["project"]["id"], "records": {}}
    # Records are not part of the model revision; every mapping is authored here.
    revision = load_bundle(path).revision
    for name in ("front", "middle", "store"):
        resource = "pkg/" + name + ".py"
        if resource in sources:
            records["records"]["binding." + name] = binding(revision, "component." + name, resource)
    write(tmp_path / "mapping.records.json", records)
    return path, root, model, records


def binding(revision, component, resource):
    return {"kind": "binding", "spec_revision": revision, "targets": [component],
            "implementation": IMPLEMENTATION, "role": "code", "resource": resource,
            "selector": {"kind": "whole-document"}, "status": "planned"}


def observe(path, root, **options):
    return check_code(path, root=root, implementation=IMPLEMENTATION, profile=PROFILE, **options)


def save_records(path, records):
    write(path.parent / "mapping.records.json", records)


def codes(report):
    return {diagnostic["code"] for check in report["checks"] for diagnostic in check["diagnostics"]}


def test_independent_package_clean_injected_repaired(tmp_path):
    path, root, _, _ = control(tmp_path)
    clean = observe(path, root)
    assert clean["result"] == "PASS", clean
    assert clean["format"] == "gdd-code-observation/0.1"
    (root / "pkg/front.py").write_text("from . import store\n")
    broken = observe(path, root)
    assert broken["result"] == "FAIL", broken
    assert broken["code_revision"] != clean["code_revision"]
    assert any(edge["from"] == "component.front" and edge["to"] == "component.store" for edge in broken["edges"])
    assert any(check.get("path") for check in broken["checks"] if check["result"] == "FAIL")
    (root / "pkg/front.py").write_text("VALUE = 1\n")
    assert observe(path, root)["result"] == "PASS"
    assert observe(path, root)["code_revision"] == clean["code_revision"]
    assert not {"evidence_refs", "trusted_evidence", "authorization"} & clean.keys()


def test_copied_concert_clean_injected_repaired_missing(tmp_path):
    original = ROOT / "examples/concert-app/concert_app"
    baseline = {p.relative_to(original).as_posix(): sha256(p.read_bytes()).hexdigest() for p in original.rglob("*") if p.is_file()}
    sources = {"concert_app/" + name: (original / name).read_text() for name in baseline if name.endswith(".py")}
    path, root, model, records = control(tmp_path, sources, origin="component.front", target="component.store")
    revision = load_bundle(path).revision
    records["records"] = {"binding." + name: binding(revision, component, "concert_app/" + name + ".py")
                          for name, component in {"api": "component.front", "storage": "component.store",
                                                 "identity": "component.middle", "reservations": "component.middle",
                                                 "__main__": "component.middle"}.items()}
    save_records(path, records)
    assert observe(path, root)["result"] == "PASS"
    api = root / "concert_app/api.py"
    clean = api.read_bytes()
    api.write_bytes(clean + b"\nfrom . import storage\n")
    assert observe(path, root)["result"] == "FAIL"
    api.write_bytes(clean)
    assert observe(path, root)["result"] == "PASS"
    (root / "concert_app/storage.py").unlink()
    assert observe(path, root)["result"] == "UNKNOWN"
    assert baseline == {p.relative_to(original).as_posix(): sha256(p.read_bytes()).hexdigest() for p in original.rglob("*") if p.is_file()}


@pytest.mark.parametrize("reach,expected", [("direct", "PASS"), ("transitive", "FAIL")])
def test_direct_and_shortest_transitive_paths(tmp_path, reach, expected):
    path, root, _, _ = control(tmp_path, {"pkg/__init__.py": "pass\n", "pkg/front.py": "from . import middle\n",
        "pkg/middle.py": "from . import store\n", "pkg/store.py": "VALUE=1\n"}, reach=reach)
    report = observe(path, root)
    assert report["result"] == expected, report
    if expected == "FAIL":
        witnesses = [check["path"] for check in report["checks"] if check.get("path")]
        assert witnesses
        assert len(witnesses[0]) == 2


def test_self_import_is_a_real_cycle(tmp_path):
    path, root, _, _ = control(tmp_path, {"pkg/__init__.py": "pass\n", "pkg/front.py": "from . import front\n",
        "pkg/store.py": "VALUE=1\n"}, origin="component.front", target="component.front", reach="transitive")
    assert observe(path, root)["result"] == "FAIL"


@pytest.mark.parametrize("change", ["missing", "ambiguous", "stale", "other-implementation", "selector"])
def test_unresolved_bindings_prevent_pass(tmp_path, change):
    path, root, _, records = control(tmp_path)
    selected = records["records"]["binding.front"]
    if change == "missing":
        del records["records"]["binding.front"]
    elif change == "ambiguous":
        records["records"]["binding.duplicate"] = deepcopy(selected)
    elif change == "stale":
        selected["spec_revision"] = "sha256:" + "0" * 64
    elif change == "other-implementation":
        selected["implementation"] = "implementation.other"
    else:
        selected["selector"] = {"kind": "opaque", "value": "VALUE"}
    save_records(path, records)
    assert observe(path, root)["result"] == "UNKNOWN"


def test_unrelated_historical_binding_does_not_block_current_mapping(tmp_path):
    path, root, _, records = control(tmp_path)
    historical = deepcopy(records["records"]["binding.front"])
    historical["spec_revision"] = "sha256:" + "0" * 64
    historical["implementation"] = "implementation.history"
    records["records"]["binding.history"] = historical
    save_records(path, records)
    assert observe(path, root)["result"] == "PASS"


@pytest.mark.parametrize("resource", ["pkg/", "pkg/*.py", "pkg/**/front.py"])
def test_supported_resource_expansion(tmp_path, resource):
    path, root, model, records = control(tmp_path, target="component.front")
    records["records"] = {"binding.front": binding(load_bundle(path).revision, "component.front", resource)}
    save_records(path, records)
    assert observe(path, root)["result"] == "PASS"


@pytest.mark.parametrize("resource", ["../pkg/front.py", "/tmp/front.py", "pkg\\front.py", "pkg/./front.py"])
def test_unsafe_binding_resource_is_unknown(tmp_path, resource):
    path, root, _, records = control(tmp_path)
    records["records"]["binding.front"]["resource"] = resource
    save_records(path, records)
    assert observe(path, root)["result"] == "UNKNOWN"


def test_excluded_endpoint_is_explicit_and_unresolved(tmp_path):
    path, root, _, _ = control(tmp_path)
    report = observe(path, root, exclude=["pkg/store.py"])
    assert report["result"] == "UNKNOWN"
    assert report["exclude"] == ["pkg/store.py"]
    assert "pkg/store.py" not in report["files"]


def test_source_symlink_is_not_followed(tmp_path):
    path, root, _, _ = control(tmp_path)
    target = tmp_path / "outside.py"
    target.write_text("raise RuntimeError('Must not execute')\n")
    source = root / "pkg/store.py"
    source.unlink()
    source.symlink_to(target)
    assert observe(path, root)["result"] == "UNKNOWN"


@pytest.mark.parametrize("source,expected_code", [
    ("def broken(:\n", "CODE_SOURCE_UNPARSEABLE"),
    ("import importlib\nimportlib.import_module('pkg.store')\n", "CODE_DYNAMIC_IMPORT"),
    ("from . import absent\n", "CODE_IMPORT_UNRESOLVED"),
])
def test_source_uncertainty_is_unknown(tmp_path, source, expected_code):
    path, root, _, _ = control(tmp_path)
    (root / "pkg/front.py").write_text(source)
    report = observe(path, root)
    assert report["result"] == "UNKNOWN", report
    assert expected_code in codes(report)


def test_unbound_import_into_bound_component_is_unknown(tmp_path):
    path, root, _, _ = control(tmp_path)
    (root / "pkg/unmapped.py").write_text("from . import store\n")
    report = observe(path, root)
    assert report["result"] == "UNKNOWN"
    assert "CODE_UNBOUND_IMPORT" in codes(report)


def test_witnessed_violation_wins_over_unrelated_parse_uncertainty(tmp_path):
    path, root, _, _ = control(tmp_path)
    (root / "pkg/front.py").write_text("from . import store\n")
    (root / "pkg/broken.py").write_text("def broken(:\n")
    report = observe(path, root)
    assert report["result"] == "FAIL", report
    assert "CODE_SOURCE_UNPARSEABLE" in codes(report)


def test_observation_never_executes_project_code(tmp_path):
    path, root, _, _ = control(tmp_path)
    marker = tmp_path / "executed"
    (root / "pkg/front.py").write_text("from pathlib import Path\nPath(" + repr(str(marker)) + ").write_text('executed')\n")
    report = observe(path, root)
    assert report["result"] == "PASS", report
    assert not marker.exists()
    assert not list(root.rglob("__pycache__"))


@pytest.mark.parametrize("option", ["profile", "scope", "constraint"])
def test_unsupported_selection_has_no_false_pass(tmp_path, option):
    path, root, model, records = control(tmp_path)
    if option == "profile":
        report = check_code(path, root=root, implementation=IMPLEMENTATION, profile="python-imports/9.9")
    elif option == "scope":
        report = observe(path, root, scope=["component.front"])
    else:
        model["nodes"]["rule.boundary"]["rule"]["constraint"]["relation"] = "calls"
        write(path, model)
        for record in records["records"].values():
            record["spec_revision"] = load_bundle(path).revision
        save_records(path, records)
        report = observe(path, root)
    assert report["result"] == "UNKNOWN", report


def test_checked_resource_revision_must_match_current_bytes(tmp_path):
    path, root, _, records = control(tmp_path)
    selected = records["records"]["binding.front"]
    selected.update(status="checked", revision="sha256:" + "0" * 64, code_revision="historical-observation")
    save_records(path, records)
    assert observe(path, root)["result"] == "UNKNOWN"
    selected["revision"] = "sha256:" + sha256((root / "pkg/front.py").read_bytes()).hexdigest()
    save_records(path, records)
    assert observe(path, root)["result"] == "PASS"


def test_scoped_transitive_rule_retains_intermediate_inventory(tmp_path):
    path, root, _, _ = control(tmp_path, {"pkg/__init__.py": "pass\n", "pkg/front.py": "from . import middle\n",
        "pkg/middle.py": "from . import store\n", "pkg/store.py": "VALUE=1\n"}, reach="transitive")
    report = observe(path, root, scope=["rule.boundary"])
    assert report["result"] == "FAIL", report
    assert "pkg/middle.py" in report["files"]
    assert any(edge["from"] == "component.middle" for edge in report["edges"])


@pytest.mark.parametrize("ambiguous", [False, True])
def test_noncomponent_binding_requires_unique_explicit_owner(tmp_path, ambiguous):
    path, root, model, records = control(tmp_path)
    model["nodes"]["action.front"] = {"kind": "action", "description": "Represent front operations."}
    model["edges"].append({"id": "edge.owner.front", "from": "action.front", "to": "component.front", "relation": "owned_by"})
    if ambiguous:
        model["edges"].append({"id": "edge.owner.middle", "from": "action.front", "to": "component.middle", "relation": "owned_by"})
    write(path, model)
    revision = load_bundle(path).revision
    for record in records["records"].values():
        record["spec_revision"] = revision
    records["records"]["binding.front"]["targets"] = ["action.front"]
    save_records(path, records)
    report = observe(path, root)
    assert report["result"] == ("UNKNOWN" if ambiguous else "PASS"), report


@pytest.mark.parametrize("defect", ["wrong-project", "duplicate-record"])
def test_additional_records_cannot_override_declared_mapping(tmp_path, defect):
    path, root, _, records = control(tmp_path)
    extra = tmp_path / "extra.records.json"
    additional = deepcopy(records)
    if defect == "wrong-project":
        additional["project_id"] = "project.other"
        additional["records"] = {}
    write(extra, additional)
    assert observe(path, root, records=extra)["result"] == "UNKNOWN"


def test_successful_observation_is_deterministic_and_preserves_inputs(tmp_path):
    path, root, _, _ = control(tmp_path)
    before = {file: file.read_bytes() for file in tmp_path.rglob("*") if file.is_file()}
    first = observe(path, root)
    second = observe(path, root)
    assert first == second
    assert first["result"] == "PASS"
    assert before == {file: file.read_bytes() for file in tmp_path.rglob("*") if file.is_file()}
    assert all(check["dimension"] == "integration" for check in first["checks"])
    assert all(set(("file", "pointer", "line", "column")) <= diagnostic["location"].keys()
               for check in first["checks"] for diagnostic in check["diagnostics"])


def test_transitive_witness_chooses_shortest_available_path(tmp_path):
    path, root, _, _ = control(tmp_path, {"pkg/__init__.py": "pass\n", "pkg/front.py": "from . import middle, store\n",
        "pkg/middle.py": "from . import store\n", "pkg/store.py": "VALUE=1\n"}, reach="transitive")
    report = observe(path, root)
    assert report["result"] == "FAIL"
    paths = [check["path"] for check in report["checks"] if check.get("path")]
    assert len(paths[0]) == 1
    assert paths[0][0]["source_file"] == "pkg/front.py"


def test_no_selected_constraints_cannot_produce_vacuous_pass(tmp_path):
    path, root, model, records = control(tmp_path)
    del model["nodes"]["rule.boundary"]
    write(path, model)
    for record in records["records"].values():
        record["spec_revision"] = load_bundle(path).revision
    save_records(path, records)
    assert observe(path, root)["result"] == "UNKNOWN"


def test_fixed_infrastructure_is_excluded_from_source_inventory(tmp_path):
    path, root, _, _ = control(tmp_path)
    ignored = root / ".venv/broken.py"
    ignored.parent.mkdir()
    ignored.write_text("def invalid(:\n")
    report = observe(path, root)
    assert report["result"] == "PASS"
    assert ".venv/broken.py" not in report["files"]


def test_already_declared_records_are_not_loaded_twice(tmp_path):
    path, root, _, _ = control(tmp_path)
    report = observe(path, root, records=path.parent / "mapping.records.json")
    assert report["result"] == "PASS"
    assert len(report["bindings"]) == 2
