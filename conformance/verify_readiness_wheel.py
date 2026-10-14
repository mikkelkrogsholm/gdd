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
