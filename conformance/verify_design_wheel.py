"""Verify the installed release outside the checkout with fixed CLI operations."""

import argparse
from hashlib import sha256
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[1]


def run(wheel: Path) -> dict:
    wheel = wheel.resolve()
    checks = []
    environment = {key: value for key, value in os.environ.items() if key != "PYTHONPATH"}
    with tempfile.TemporaryDirectory(prefix="gdd-0.3-installed-") as temporary:
        folder = Path(temporary)
        python = folder / "venv/bin/python"
        for argv in (["uv", "venv", "--python", sys.executable, str(folder / "venv")],
                     ["uv", "pip", "install", "--python", str(python), str(wheel)]):
            subprocess.run(argv, check=True, capture_output=True, text=True, env=environment)

        def command(name: str, arguments: list[str], expected: int = 0) -> dict:
            execution = subprocess.run([str(python), "-m", "gdd_validator", *arguments],
                                       cwd=folder, env=environment, capture_output=True, text=True)
            if execution.returncode != expected:
                raise AssertionError(f"{name}: {execution.returncode}, {execution.stderr}, {execution.stdout}")
            output = json.loads(execution.stdout)
            checks.append({"name": name, "exit_code": execution.returncode})
            return output

        old = str(ROOT / "examples/concert.gdd.yaml")
        new = str(ROOT / "examples/concert-0.3.gdd.yaml")
        for name, model in [("legacy", old), ("current", new)]:
            command(name + "-model", ["validate", model])
            command(name + "-readiness", ["validate", model, "--profile", "readiness", "--scope", "action.reserve"])
            command(name + "-snapshot", ["snapshot", model])
            html = folder / (name + ".html")
            command(name + "-view", ["view", model, "--output", str(html)])
            command(name + "-verify-view", ["validate", model, "--profile", "visualization", "--view", str(html)])
            source = ROOT / ("examples/concert.graph.html" if name == "legacy" else "examples/concert-0.3.graph.html")
            if source.read_bytes() != html.read_bytes():
                raise AssertionError("Installed renderer differs from the shipped current artifact.")
        boundaries = command("current-boundaries", ["boundaries", new])
        if boundaries != json.loads((ROOT / "examples/concert-0.3.boundaries.json").read_text()):
            raise AssertionError("Installed derived boundaries differ from source.")
        review = command("planned-review-unknown", ["design-review", new, "--review", str(ROOT / "examples/design-review.planned.json"),
                         "--implementation", "implementation.concert.python", "--code-revision", "planned", "--scope", "action.reserve"], 2)
        if review["status"] != "UNKNOWN":
            raise AssertionError("A planned review must remain UNKNOWN.")
        command("legacy-native-acceptance", ["validate", str(ROOT / "examples/agent-native/model.gdd.json"),
                "--profile", "records", "--activity", "acceptance", "--scope", "action.quote",
                "--host-context", str(ROOT / "examples/agent-native/host.acceptance.json")])
        replay = subprocess.run([str(python), str(ROOT / "examples/agent-native/replay.py"), "--output", str(folder / "replay")],
                                cwd=folder, env=environment, capture_output=True, text=True, check=True)
        summary = json.loads(replay.stdout)
        if not summary["acceptance_current"] or summary["baseline_exit"] == 0 or summary["final_exit"] != 0:
            raise AssertionError("Installed legacy replay failed its actual red/green checks.")
        files = []
        with ZipFile(wheel) as archive:
            for member in archive.namelist():
                if member.startswith("gdd_validator/") and not member.endswith("/"):
                    relative = member.replace("gdd_validator/schemas/", "schemas/")
                    if archive.read(member) != (ROOT / relative).read_bytes():
                        raise AssertionError(f"Wheel/source mismatch: {member}")
                    files.append(member)
    return {"format": "gdd-installed-wheel-check/0.3", "wheel": wheel.name,
            "wheel_sha256": sha256(wheel.read_bytes()).hexdigest(),
            "runtime_hashes": {member: sha256((ROOT / member.replace("gdd_validator/schemas/", "schemas/")).read_bytes()).hexdigest() for member in files},
            "checks": checks, "cli_checks": len(checks), "legacy_replay": summary,
            "matching_runtime_files": files, "passed": True}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wheel", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Output already exists.")
    result = run(args.wheel)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(f"{result['cli_checks']} installed CLI checks, legacy replay and {len(result['matching_runtime_files'])} runtime files passed")
