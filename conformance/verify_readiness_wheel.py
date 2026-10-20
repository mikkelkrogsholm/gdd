"""Verify a wheel's installed CLI and complete assets outside the source checkout.

This fixed harness reuses the current/legacy operations and native replay from
verify_design_wheel.py. It runs no commands supplied by models or skill content.
"""

import argparse
from hashlib import sha256
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[1]


def source_assets() -> dict[str, Path]:
    """Map every shipped asset to its authoritative source file."""
    assets = {}
    for source, destination in (
        (ROOT / "gdd_validator", "gdd_validator"),
        (ROOT / "schemas", "gdd_validator/schemas"),
        (ROOT / "templates", "gdd_validator/templates"),
        (ROOT / "skills/gdd", "gdd_validator/skill"),
    ):
        if not source.is_dir():
            raise AssertionError(f"Required source asset directory is missing: {source}")
        for file in sorted(source.rglob("*")):
            relative = file.relative_to(source)
            if file.is_file() and "__pycache__" not in relative.parts and file.suffix not in {".pyc", ".pyo"}:
                member = f"{destination}/{relative.as_posix()}"
                if member in assets:
                    raise AssertionError(f"Asset mapping is ambiguous: {member}")
                assets[member] = file
    return assets


def file_hashes(folder: Path) -> dict[str, str]:
    return {
        file.relative_to(folder).as_posix(): sha256(file.read_bytes()).hexdigest()
        for file in sorted(folder.rglob("*"))
        if file.is_file()
    }


def run(wheel: Path) -> dict:
    wheel = wheel.resolve()
    assets = source_assets()
    with ZipFile(wheel) as archive:
        members = [name for name in archive.namelist() if name.startswith("gdd_validator/") and not name.endswith("/")]
        if len(members) != len(set(members)) or set(members) != set(assets):
            raise AssertionError(
                f"Wheel asset inventory differs: missing={sorted(set(assets) - set(members))}, "
                f"unexpected={sorted(set(members) - set(assets))}"
            )
        for member, source in assets.items():
            if archive.read(member) != source.read_bytes():
                raise AssertionError(f"Wheel/source byte mismatch: {member}")
        metadata_names = [name for name in archive.namelist() if name.endswith(".dist-info/METADATA")]
        if len(metadata_names) != 1:
            raise AssertionError("The wheel must contain one distribution metadata document.")
        metadata = archive.read(metadata_names[0]).decode("utf-8")
        license_member = metadata_names[0].removesuffix("METADATA") + "licenses/LICENSE"
        if ("License-Expression: MIT\n" not in metadata or "License-File: LICENSE\n" not in metadata
                or archive.read(license_member) != (ROOT / "LICENSE").read_bytes()):
            raise AssertionError("The installed distribution must include the maintainer-selected MIT license.")

    environment = {
        key: value for key, value in os.environ.items()
        if key not in {"PYTHONPATH", "PYTHONHOME", "VIRTUAL_ENV"}
    }
    environment.update(PYTHONDONTWRITEBYTECODE="1", PYTHONNOUSERSITE="1")
    checks = []
    with tempfile.TemporaryDirectory(prefix="gdd-readiness-installed-") as temporary:
        folder = Path(temporary).resolve()
        if folder.is_relative_to(ROOT):
            raise AssertionError("Installed verification must run outside the checkout.")
        venv = folder / "venv"
        bin_directory = venv / ("Scripts" if os.name == "nt" else "bin")
        python = bin_directory / ("python.exe" if os.name == "nt" else "python")
        cli = bin_directory / ("gdd.exe" if os.name == "nt" else "gdd")
        for argv in (
            [sys.executable, "-m", "venv", str(venv)],
            [str(python), "-I", "-m", "pip", "install", "--disable-pip-version-check", str(wheel)],
        ):
            subprocess.run(argv, cwd=folder, env=environment, check=True, capture_output=True, text=True)

        identity = subprocess.run(
            [str(python), "-I", "-c", "import json, gdd_validator; from importlib.metadata import version; "
             "print(json.dumps({'module': gdd_validator.__file__, 'version': version('gdd-validator')}))"],
            cwd=folder, env=environment, check=True, capture_output=True, text=True,
        )
        installed = json.loads(identity.stdout)
        installed_root = Path(installed["module"]).resolve().parent
        if not installed_root.is_relative_to(venv) or installed_root.is_relative_to(ROOT):
            raise AssertionError("The verifier imported code outside its fresh wheel environment.")
        expected_version = tomllib.loads((ROOT / "pyproject.toml").read_text())["project"]["version"]
        if installed["version"] != expected_version:
            raise AssertionError("Installed package version differs from pyproject.toml.")
        for member, source in assets.items():
            target = installed_root / member.removeprefix("gdd_validator/")
            if not target.is_file() or target.read_bytes() != source.read_bytes():
                raise AssertionError(f"Installed/source byte mismatch: {member}")
        # Match the documented extraction route even from the source checkout:
        # isolated Python prevents the checkout from shadowing the installed wheel.
        skill_copy = folder / "standalone-skill"
        subprocess.run([
            str(python), "-I", "-c",
            "from importlib.resources import files; import shutil, sys; "
            "shutil.copytree(str(files('gdd_validator').joinpath('skill')), sys.argv[1])",
            str(skill_copy),
        ], cwd=ROOT, env=environment, check=True, capture_output=True, text=True)
        subprocess.run([
            str(python), "-I", str(skill_copy / "scripts/verify_assets.py"),
        ], cwd=skill_copy, env=environment, check=True, capture_output=True, text=True)

        def execute(argv: list[str], cwd: Path) -> subprocess.CompletedProcess:
            return subprocess.run(argv, cwd=cwd, env=environment, capture_output=True, text=True)

        def command(name: str, arguments: list[str], expected: int = 0, *, parity: bool = True, json_output: bool = True):
            execution = execute([str(python), "-I", "-m", "gdd_validator", *arguments], folder)
            if execution.returncode != expected:
                raise AssertionError(f"{name}: {execution.returncode}, {execution.stderr}, {execution.stdout}")
            output = json.loads(execution.stdout) if json_output else None
            if parity:
                source = execute([sys.executable, "-m", "gdd_validator", *arguments], ROOT)
                if source.returncode != expected or json.loads(source.stdout) != output:
                    raise AssertionError(f"Installed/source CLI mismatch: {name}")
                console = execute([str(cli), *arguments], folder)
                if console.returncode != expected or json.loads(console.stdout) != output:
                    raise AssertionError(f"Module/console CLI mismatch: {name}")
            checks.append({"name": name, "exit_code": execution.returncode, "source_and_console_parity": parity})
            return output

        for name, model_name, view_name in (
            ("legacy", "concert.gdd.yaml", "concert.graph.html"),
            ("current", "concert-0.3.gdd.yaml", "concert-0.3.graph.html"),
        ):
            model = str(ROOT / "examples" / model_name)
            command(name + "-model", ["validate", model])
            command(name + "-readiness", ["validate", model, "--profile", "readiness", "--scope", "action.reserve"])
            command(name + "-snapshot", ["snapshot", model])
            command(name + "-context", ["context", model, "--scope", "action.reserve"])
            html = folder / (name + ".html")
            command(name + "-view", ["view", model, "--output", str(html)], parity=False)
            command(name + "-verify-view", ["validate", model, "--profile", "visualization", "--view", str(html)])
            if (ROOT / "examples" / view_name).read_bytes() != html.read_bytes():
                raise AssertionError(f"Installed renderer differs from the shipped {name} artifact.")

        current = str(ROOT / "examples/concert-0.3.gdd.yaml")
        boundaries = command("current-boundaries", ["boundaries", current])
        if boundaries != json.loads((ROOT / "examples/concert-0.3.boundaries.json").read_text()):
            raise AssertionError("Installed derived boundaries differ from the shipped artifact.")
        review = command("planned-review-unknown", [
            "design-review", current, "--review", str(ROOT / "examples/design-review.planned.json"),
            "--implementation", "implementation.concert.python", "--code-revision", "planned", "--scope", "action.reserve",
        ], 2)
        if review["status"] != "UNKNOWN":
            raise AssertionError("A planned review must remain UNKNOWN.")
        command("legacy-native-acceptance", [
            "validate", str(ROOT / "examples/agent-native/model.gdd.json"), "--profile", "records",
            "--activity", "acceptance", "--scope", "action.quote",
            "--host-context", str(ROOT / "examples/agent-native/host.acceptance.json"),
        ])

        project = folder / "fresh-project"
        init_args = ["init", str(project), "--project-id", "project.wheel", "--title", "Installed wheel starter"]
        command("fresh-project-init", init_args, parity=False)
        if set(file_hashes(project)) != {"gdd.json", "SOURCES.md", "records.json"}:
            raise AssertionError("The scaffold did not create the documented project files.")
        command("fresh-project-model", ["validate", str(project / "gdd.json")])
        starter_status = command("fresh-project-status", [
            "status", str(project / "gdd.json"), "--scope", "action.main",
        ], 1)
        if starter_status["assessments"]["readiness"]["result"] != "FAIL":
            raise AssertionError("Installed status must preserve starter readiness failures.")
        ready_model = str(ROOT / "docs/tutorial/refined.gdd.json")
        ready_before = Path(ready_model).read_bytes()
        before_inspection = file_hashes(project)
        ready_status = command("ready-status", ["status", ready_model, "--scope", "action.add"])
        if ready_status["assessments"]["authorization"] != {
            "assessed": False, "result": None, "check_refs": [],
        }:
            raise AssertionError("Installed status inferred authorization without host input.")
        ready_impact = command("ready-impact", ["impact", ready_model, "--scope", "action.add"])
        if ready_impact["coverage"]["basis"] != "declared-model":
            raise AssertionError("Installed impact must identify its declared-model coverage.")
        if file_hashes(project) != before_inspection or Path(ready_model).read_bytes() != ready_before:
            raise AssertionError("Read-only inspection changed a model or scaffold.")
        unchanged = command("equal-model-diff", ["diff", ready_model, ready_model])
        if unchanged["comparison"]["semantic_changes"]:
            raise AssertionError("Installed comparison invented a change between equal models.")
        changed_model = json.loads(ready_before)
        changed_model["nodes"]["action.add"]["contract"]["postconditions"] = []
        changed_path = folder / "changed-model.json"
        changed_path.write_text(json.dumps(changed_model), encoding="utf-8")
        changed_before = changed_path.read_bytes()
        changed = command("weakened-contract-diff", ["diff", ready_model, str(changed_path)])
        if not changed["advisories"] or not changed["comparison"]["semantic_changes"]:
            raise AssertionError("Installed comparison omitted the removed postcondition.")
        if Path(ready_model).read_bytes() != ready_before or changed_path.read_bytes() != changed_before:
            raise AssertionError("Installed comparison changed its inputs.")
        readiness = command("fresh-project-unready", [
            "validate", str(project / "gdd.json"), "--profile", "readiness", "--scope", "action.main",
        ], 1)
        if not any(check["dimension"] == "readiness" and check["result"] == "FAIL" for check in readiness["checks"]):
            raise AssertionError("A new draft scaffold must remain unready.")
        before = file_hashes(project)
        command("repeated-init-preserves-files", init_args, 1, parity=False, json_output=False)
        if file_hashes(project) != before:
            raise AssertionError("Repeated initialization changed existing project files.")
        occupied = folder / "occupied"
        occupied.mkdir()
        (occupied / "existing.txt").write_bytes(b"Existing work must survive.\n")
        before = file_hashes(occupied)
        command("occupied-init-preserves-files", [
            "init", str(occupied), "--project-id", "project.occupied", "--title", "Occupied directory",
        ], 1, parity=False, json_output=False)
        if file_hashes(occupied) != before:
            raise AssertionError("Initialization changed a nonempty directory.")

        # Exercise the installed observer with caller-authored binding records.
        code_root = folder / "python-source"
        code_root.mkdir()
        api, storage = code_root / "api.py", code_root / "storage.py"
        api.write_text("pass\n", encoding="utf-8")
        storage.write_text("pass\n", encoding="utf-8")
        code_model = folder / "python-model.json"
        code_records = folder / "python-records.json"
        code_document = {
            "format": "gdd/0.3", "project": {"id": "project.python", "title": "Python", "goal": "Check imports"},
            "record_files": [code_records.name], "edges": [],
            "nodes": {
                "component.api": {"kind": "component", "description": "API"},
                "component.storage": {"kind": "component", "description": "Storage"},
                "rule.boundary": {"kind": "rule", "description": "Boundary", "rule": {
                    "category": "architecture", "statement": "API must not import storage.",
                    "scope": "global", "blocking": True, "verification": ["python_imports"],
                    "constraint": {"relation": "imports", "from": "component.api", "to": "component.storage",
                                   "reach": "direct", "policy": "forbid"}}},
            },
        }
        code_model.write_text(json.dumps(code_document), encoding="utf-8")
        mapping = {"format": "gdd-records/0.2", "project_id": "project.python", "records": {}}
        code_records.write_text(json.dumps(mapping), encoding="utf-8")
        code_spec = command("code-model-snapshot", ["snapshot", str(code_model)])["revision"]
        mapping["records"] = {
            "binding." + name: {"kind": "binding", "spec_revision": code_spec,
                               "targets": ["component." + name], "implementation": "implementation.python",
                               "role": "code", "resource": name + ".py", "selector": {"kind": "whole-document"},
                               "status": "planned"}
            for name in ("api", "storage")
        }
        code_records.write_text(json.dumps(mapping), encoding="utf-8")
        code_args = ["check-code", str(code_model), "--root", str(code_root),
                     "--implementation", "implementation.python", "--profile", "python-imports/0.1"]
        clean = command("code-clean", code_args)
        api.write_text("import storage\n", encoding="utf-8")
        injected = command("code-forbidden-import", code_args, 1)
        api.write_text("pass\n", encoding="utf-8")
        repaired = command("code-repaired", code_args)
        storage.unlink()
        missing = command("code-missing-source", code_args, 2)
        code_controls = {name: result["result"] for name, result in
                         (("clean", clean), ("injected", injected), ("repaired", repaired), ("missing", missing))}
        if (code_controls != {"clean": "PASS", "injected": "FAIL", "repaired": "PASS", "missing": "UNKNOWN"}
                or clean["code_revision"] != repaired["code_revision"]
                or clean["code_revision"] == injected["code_revision"]
                or not injected["checks"][0]["path"]):
            raise AssertionError("Installed code observer controls did not establish the expected source behavior.")

        replay = execute([
            str(python), "-I", str(ROOT / "examples/agent-native/replay.py"), "--output", str(folder / "replay"),
        ], folder)
        if replay.returncode:
            raise AssertionError(f"Installed native replay failed: {replay.stderr}, {replay.stdout}")
        summary = json.loads(replay.stdout)
        if not summary["acceptance_current"] or summary["baseline_exit"] == 0 or summary["final_exit"] != 0:
            raise AssertionError("Installed legacy replay failed its actual red/green checks.")

    return {
        "format": "gdd-readiness-wheel-check/0.1",
        "wheel": wheel.name,
        "wheel_sha256": sha256(wheel.read_bytes()).hexdigest(),
        "package_version": installed["version"],
        "python": sys.version,
        "outside_checkout": True,
        "standalone_skill_extracted_and_verified": True,
        "license": "MIT",
        "license_file_sha256": sha256((ROOT / "LICENSE").read_bytes()).hexdigest(),
        "pythonpath_removed": "PYTHONPATH" not in environment,
        "runtime_hashes": {member: sha256(source.read_bytes()).hexdigest() for member, source in assets.items()},
        "checks": checks,
        "cli_checks": len(checks),
        "legacy_replay": summary,
        "code_observation_controls": code_controls,
        "matching_runtime_files": sorted(assets),
        "passed": True,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wheel", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Output already exists.")
    result = run(args.wheel)
    with args.output.open("x", encoding="utf-8") as output:
        output.write(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"{result['cli_checks']} installed CLI checks, legacy replay and {len(result['matching_runtime_files'])} exact assets passed")
