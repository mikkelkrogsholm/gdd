#!/usr/bin/env python3
"""Run proportionate local checks with fresh logs and explicit interpreter coverage."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
from time import monotonic

ROOT = Path(__file__).resolve().parents[1]
MATRIX_PYTHONS = ("3.11", "3.12", "3.13", "3.14")
QUICK_TESTS = (
    "tests/test_consumer_limits.py", "tests/test_readiness_diagnostics.py",
    "tests/test_tutorial.py", "tests/test_graphify.py",
    "tests/test_visualization.py", "tests/test_skill_assets.py",
    "tests/test_local_check.py",
    "tests/test_code_check.py", "tests/test_code_check_cli.py", "tests/test_python_imports.py",
    "tests/test_project_status.py", "tests/test_impact.py", "tests/test_inspection_cli.py",
    "tests/test_model_diff.py", "tests/test_model_diff_cli.py",
)
PYTEST_BOOTSTRAP = (
    "import json, platform, sys; "
    "print('GDD_CHECK_RUNTIME=' + json.dumps({'python': platform.python_version(), "
    "'executable': sys.executable}), flush=True); "
    "import pytest; raise SystemExit(pytest.main(sys.argv[1:]))"
)


def pytest_command(version: str | None, quick: bool) -> list[str]:
    # Keep an explicitly selected host patch for its matching matrix line.
    selected = sys.executable if version == f"{sys.version_info.major}.{sys.version_info.minor}" else version
    prefix = [sys.executable] if version is None else [
        "uv", "run", "--isolated", "--locked", "--group", "dev",
        "--no-python-downloads", "--python", selected, "python",
    ]
    return prefix + ["-B", "-c", PYTEST_BOOTSTRAP, "-q", "-p", "no:cacheprovider",
                     "-o", "tmp_path_retention_policy=failed"] + (
        list(QUICK_TESTS) if quick else []
    )


def run_step(name: str, argv: list[str], output: Path, *, root: Path = ROOT) -> dict:
    for stream in ("stdout", "stderr"):
        path = output / f"{name}.{stream}.txt"
        if path.exists() or path.is_symlink():
            raise FileExistsError(f"Refusing to reuse an earlier check log: {path}")
    started = monotonic()
    environment = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1", "UV_NO_PROGRESS": "1"}
    try:
        result = subprocess.run(argv, cwd=root, env=environment, capture_output=True,
                                text=True, encoding="utf-8", errors="replace", check=False)
        code, stdout, stderr = result.returncode, result.stdout, result.stderr
    except OSError as error:
        code, stdout, stderr = 127, "", str(error)
    record = {"name": name, "command": argv, "exit_code": code,
              "result": "PASS" if code == 0 else "FAIL",
              "seconds": round(monotonic() - started, 3)}
    for stream, value in (("stdout", stdout), ("stderr", stderr)):
        path = output / f"{name}.{stream}.txt"
        with path.open("x", encoding="utf-8") as file:
            file.write(value)
        record[stream] = path.name
    for line in stdout.splitlines():
        if line.startswith("GDD_CHECK_RUNTIME="):
            record["runtime"] = json.loads(line.removeprefix("GDD_CHECK_RUNTIME="))
            break
    print(f"{record['result']:4} {name}: {record['seconds']:.2f}s", flush=True)
    if code:
        print(f"     Logs: {output / record['stdout']} and {output / record['stderr']}", flush=True)
    return record


def run_checks(output: Path, *, quick: bool = False, release: bool = False,
               matrix: bool = False, root: Path = ROOT) -> dict:
    # The caller creates one fresh directory; never reuse a historical receipt.
    report = {"mode": "quick" if quick else "release" if release else "full",
              "matrix": matrix, "started_at": datetime.now(timezone.utc).isoformat(),
              "host_python": platform.python_version(), "platform": platform.platform(),
              "output": str(output), "steps": [], "passed": False}

    def step(name: str, argv: list[str]) -> bool:
        record = run_step(name, argv, output, root=root)
        report["steps"].append(record)
        return record["exit_code"] == 0

    try:
        if not step("skill-sync", [sys.executable, "scripts/sync_skill.py", "--check"]):
            return report
        for version in MATRIX_PYTHONS if matrix else (None,):
            if not step("pytest-" + (version or "current"), pytest_command(version, quick)):
                return report
        if release:
            for name in ("standard", "graphify", "visualization", "workflow", "design", "code", "status", "impact", "diff"):
                script = "run.py" if name == "standard" else f"run_{name}.py"
                if not step(name, [sys.executable, f"conformance/{script}",
                                   "--output", str(output / f"{name}.json")]):
                    return report
            dist = output / "dist"
            if not step("wheel-build", ["uv", "build", "--wheel", "--out-dir", str(dist)]):
                return report
            wheels = list(dist.glob("*.whl"))
            if len(wheels) != 1:
                raise ValueError("Release verification requires exactly one freshly built wheel.")
            if not step("installed-wheel", [sys.executable, "conformance/verify_readiness_wheel.py",
                                             str(wheels[0]), "--output", str(output / "installed-wheel.json")]):
                return report
        report["passed"] = True
        return report
    finally:
        report["finished_at"] = datetime.now(timezone.utc).isoformat()
        with (output / "summary.json").open("x", encoding="utf-8") as file:
            json.dump(report, file, indent=2)
            file.write("\n")
        print(f"Local verification: {'PASS' if report['passed'] else 'FAIL'}; report: {output / 'summary.json'}", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--quick", action="store_true", help="Focused CLI, parser, Graphify, viewer and skill checks")
    modes.add_argument("--release", action="store_true", help="Full suite, separate conformance receipts and installed wheel")
    parser.add_argument("--matrix", action="store_true", help="Run pytest sequentially on installed Python 3.11–3.14 via isolated uv environments")
    parser.add_argument("--output", type=Path, help="New output directory; default: a fresh system temporary directory")
    args = parser.parse_args()
    try:
        if args.output:
            output = args.output.resolve()
            output.mkdir(parents=True, exist_ok=False)
        else:
            output = Path(tempfile.mkdtemp(prefix="gdd-local-check-"))
        result = run_checks(output, quick=args.quick, release=args.release, matrix=args.matrix)
        return 0 if result["passed"] else 1
    except (OSError, ValueError) as error:
        parser.exit(1, f"Local verification failed: {error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
