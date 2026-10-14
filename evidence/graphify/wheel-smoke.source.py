"""Verify the built wheel from an isolated environment and outside repository cwd."""

from hashlib import sha256
import json
from pathlib import Path
import subprocess
import tempfile


def main() -> None:
    root = Path(__file__).resolve().parents[2]
    wheel = root / "dist/gdd_validator-0.3.0-py3-none-any.whl"
    fixture = root / "examples/graphify/raw.ast.json"
    with tempfile.TemporaryDirectory(prefix="gdd-graphify-wheel-") as temporary:
        directory = Path(temporary)
        environment = directory / "env"
        for command in (
            [
                "uv",
                "venv",
                str(environment),
                "--python",
                str(root / ".venv/bin/python"),
            ],
            [
                "uv",
                "pip",
                "install",
                "--python",
                str(environment / "bin/python"),
                str(wheel),
            ],
        ):
            result = subprocess.run(
                command, capture_output=True, text=True, cwd=directory
            )
            assert result.returncode == 0, result.stderr
        executable = environment / "bin/gdd"
        bundle = directory / "observations"
        steps = [
            (
                "import",
                [
                    "graphify",
                    "import",
                    str(fixture),
                    "--profile",
                    "graphify-0.9.75-ast",
                    "--output",
                    str(bundle),
                ],
            ),
            ("validate", ["validate", str(bundle), "--profile", "import.graphify"]),
            (
                "export",
                [
                    "graphify",
                    "export",
                    str(bundle),
                    "--output",
                    str(directory / "recovered.json"),
                ],
            ),
            (
                "propose",
                [
                    "graphify",
                    "propose",
                    str(bundle),
                    "--mapping",
                    str(root / "examples/graphify/mapping.json"),
                    "--output",
                    str(bundle / "proposal.json"),
                ],
            ),
        ]
        results = []
        for name, arguments in steps:
            completed = subprocess.run(
                [str(executable), *arguments],
                capture_output=True,
                text=True,
                cwd=directory,
            )
            assert completed.returncode == 0, (name, completed.stderr, completed.stdout)
            result = json.loads(completed.stdout)
            if name != "propose":
                assert all(c["result"] == "PASS" for c in result["checks"]), result
            else:
                assert result["status"] == "proposed"
            results.append({"step": name, "exit_code": completed.returncode})
        assert (directory / "recovered.json").read_bytes() == fixture.read_bytes()
        probe = subprocess.run(
            [
                str(environment / "bin/python"),
                "-c",
                "import importlib.util,json; from importlib.metadata import version; import gdd_validator; from pathlib import Path; print(json.dumps({'version':version('gdd-validator'),'schemas':sorted(p.name for p in (Path(gdd_validator.__file__).parent/'schemas').glob('*.schema.json')),'graphify_dependency_installed':importlib.util.find_spec('graphify') is not None}))",
            ],
            cwd=directory,
            capture_output=True,
            text=True,
            check=True,
        )
        package = json.loads(probe.stdout)
        assert package["version"] == "0.3.0"
        assert len(package["schemas"]) == 9
        assert not package["graphify_dependency_installed"]
        output = {
            "format": "gdd-graphify-wheel-smoke/0.2",
            "wheel": wheel.name,
            "wheel_sha256": sha256(wheel.read_bytes()).hexdigest(),
            "outside_workspace": True,
            "package": package,
            "steps": results,
            "byte_roundtrip": "PASS",
        }
        (root / "evidence/graphify/wheel-smoke.json").write_text(
            json.dumps(output, indent=2) + "\n"
        )
        print(
            "Installed wheel: import / validate / byte export / explicit proposal PASS; no Graphify runtime dependency."
        )


if __name__ == "__main__":
    main()
