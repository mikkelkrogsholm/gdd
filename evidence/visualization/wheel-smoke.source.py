"""Installed wheel verification outside the checkout, including embedded assets."""

from hashlib import sha256
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile


def main() -> None:
    root = Path(__file__).resolve().parents[2]
    wheel = root / "dist/gdd_validator-0.4.0-py3-none-any.whl"
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        assets = [name for name in names if name.startswith("gdd_validator/viewer/")]
        assert len(assets) == 5, assets
        assert "gdd_validator/schemas/view.schema.json" in names
        assert "gdd_validator/schemas/view-state.schema.json" in names
    with tempfile.TemporaryDirectory(prefix="gdd-view-wheel-") as temporary:
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
        model = directory / "examples"
        shutil.copytree(
            root / "examples", model, ignore=shutil.ignore_patterns("*.html")
        )
        executable = environment / "bin/gdd"
        output = directory / "graph.html"
        steps = [
            (
                "render",
                ["view", str(model / "concert.gdd.yaml"), "--output", str(output)],
            ),
            (
                "verify",
                [
                    "validate",
                    str(model / "concert.gdd.yaml"),
                    "--profile",
                    "visualization",
                    "--view",
                    str(output),
                ],
            ),
            (
                "readiness",
                [
                    "validate",
                    str(model / "concert.gdd.yaml"),
                    "--profile",
                    "readiness",
                    "--scope",
                    "action.reserve",
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
            assert completed.returncode == 0, (name, completed.stdout, completed.stderr)
            results.append(
                {
                    "step": name,
                    "exit_code": completed.returncode,
                    "report": json.loads(completed.stdout),
                }
            )
        assert (
            output.read_bytes() == (root / "examples/concert.graph.html").read_bytes()
        )
        result = {
            "wheel": wheel.name,
            "sha256": sha256(wheel.read_bytes()).hexdigest(),
            "assets": assets,
            "outside_checkout": True,
            "html_matches_checkout_renderer": True,
            "steps": results,
        }
        (root / "evidence/visualization/wheel-smoke.json").write_text(
            json.dumps(result, indent=2) + "\n"
        )


if __name__ == "__main__":
    main()
