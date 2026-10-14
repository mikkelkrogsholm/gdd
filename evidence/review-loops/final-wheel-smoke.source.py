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
    wheel = root / "dist/gdd_validator-0.4.1-py3-none-any.whl"
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        assert not any(name.endswith((".orig", ".rej")) for name in names)
        assets = [name for name in names if name.startswith("gdd_validator/viewer/")]
        assert len(assets) == 5, assets
        assert "gdd_validator/schemas/view.schema.json" in names
        assert "gdd_validator/schemas/view-state.schema.json" in names
        assert "gdd_validator/decision.py" in names
        assert "gdd_validator/record_integrity.py" in names
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
        installed = subprocess.run(
            [
                str(environment / "bin/python"),
                "-c",
                "import json, gdd_validator; print(json.dumps({'path': gdd_validator.__file__, 'version': gdd_validator.__version__}))",
            ],
            capture_output=True,
            text=True,
            cwd=directory,
        )
        assert installed.returncode == 0, installed.stderr
        installed_package = json.loads(installed.stdout)
        assert installed_package["version"] == "0.4.1"
        assert (
            Path(installed_package["path"])
            .resolve()
            .is_relative_to(environment.resolve())
        )
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
            (
                "graphify-import",
                [
                    "graphify",
                    "import",
                    str(model / "graphify/raw.ast.json"),
                    "--profile",
                    "graphify-0.9.75-ast",
                    "--output",
                    str(directory / "observations"),
                ],
            ),
            (
                "graphify-verify",
                ["graphify", "validate", str(directory / "observations")],
            ),
            (
                "graphify-export",
                [
                    "graphify",
                    "export",
                    str(directory / "observations"),
                    "--output",
                    str(directory / "recovered.json"),
                ],
            ),
            (
                "graphify-propose",
                [
                    "graphify",
                    "propose",
                    str(directory / "observations"),
                    "--mapping",
                    str(model / "graphify/mapping.json"),
                    "--output",
                    str(directory / "proposal.json"),
                ],
            ),
        ]
        results = []

        def run_step(name, arguments, expected=0):
            completed = subprocess.run(
                [str(executable), *arguments],
                capture_output=True,
                text=True,
                cwd=directory,
            )
            assert completed.returncode == expected, (
                name,
                completed.stdout,
                completed.stderr,
            )
            assert "Traceback" not in completed.stderr
            results.append(
                {
                    "step": name,
                    "exit_code": completed.returncode,
                    "report": (
                        json.loads(completed.stdout) if completed.stdout else None
                    ),
                    "stderr": completed.stderr,
                }
            )

        for name, arguments in steps:
            run_step(name, arguments)
        raw_alias = directory / "observations.json"
        shutil.copy2(model / "graphify/raw.ast.json", raw_alias)
        run_step(
            "raw-filename-profile",
            [
                "graphify",
                "validate",
                str(raw_alias),
                "--profile",
                "graphify-0.9.75-ast",
            ],
        )
        deep = directory / "deep.json"
        deep.write_bytes(
            b'{"nodes":[],"edges":[],"vendor":' + b"[" * 300 + b"0" + b"]" * 300 + b"}"
        )
        deep_bundle = directory / "deep-observations"
        run_step(
            "unsupported-depth-import",
            [
                "graphify",
                "import",
                str(deep),
                "--profile",
                "graphify-0.9.75-ast",
                "--output",
                str(deep_bundle),
            ],
            expected=2,
        )
        deep_recovered = directory / "deep-recovered.json"
        run_step(
            "unsupported-depth-export",
            ["graphify", "export", str(deep_bundle), "--output", str(deep_recovered)],
            expected=2,
        )
        assert deep_recovered.read_bytes() == deep.read_bytes()
        refused_proposal = directory / "refused.json"
        run_step(
            "unsupported-depth-proposal-refused",
            [
                "graphify",
                "propose",
                str(deep_bundle),
                "--mapping",
                str(model / "graphify/mapping.json"),
                "--output",
                str(refused_proposal),
            ],
            expected=1,
        )
        assert not refused_proposal.exists()
        unsupported_number = directory / "unsupported-number.json"
        unsupported_number.write_text(
            '{"format":"gdd/0.2","project":{"id":"smoke","title":"Smoke",'
            '"goal":"Preserve declared numbers"},"nodes":{"entity.x":'
            '{"kind":"entity","description":"Value","fields":{"value":'
            '{"type":"number","minimum":1e-4000,"maximum":0}}}},"edges":[]}'
        )
        run_step(
            "unsupported-model-number",
            ["validate", str(unsupported_number)],
            expected=2,
        )
        numeric_report = results[-1]["report"]
        assert any(
            diagnostic["code"] == "FORMAT_UNSUPPORTED"
            for check in numeric_report["checks"]
            for diagnostic in check["diagnostics"]
        )
        assert not any(check["result"] == "FAIL" for check in numeric_report["checks"])
        for name, bounds, expected in (
            (
                "contradictory-mixed-number-bounds",
                '"minimum":1e23,"maximum":99999999999999995000000',
                1,
            ),
            (
                "equal-mixed-number-bounds",
                '"minimum":100000000000000000000000,"maximum":1e23',
                0,
            ),
        ):
            numeric_model = directory / (name + ".json")
            numeric_model.write_text(
                unsupported_number.read_text().replace(
                    '"minimum":1e-4000,"maximum":0', bounds
                )
            )
            run_step(name, ["validate", str(numeric_model)], expected=expected)
        run_step(
            "unavailable-secondary-input",
            [
                "validate",
                str(directory / "missing-records.json"),
                "--model",
                str(model / "concert.gdd.yaml"),
                "--profile",
                "representation",
            ],
            expected=2,
        )
        deep_model = directory / "unsupported-model-depth.json"
        deep_model.write_text(
            json.dumps(
                {
                    "format": "gdd/0.2",
                    "project": {
                        "id": "smoke",
                        "title": "Smoke",
                        "goal": "Classify parser capacity",
                    },
                    "nodes": {
                        "entity.x": {
                            "kind": "entity",
                            "description": "Value",
                            "fields": {},
                            "annotations": {"smoke.data": "DEEP"},
                        }
                    },
                    "edges": [],
                }
            ).replace('"DEEP"', "[" * 2000 + "0" + "]" * 2000)
        )
        run_step(
            "unsupported-model-parser-depth",
            ["validate", str(deep_model)],
            expected=2,
        )
        assert any(
            diagnostic["code"] == "FORMAT_UNSUPPORTED"
            for check in results[-1]["report"]["checks"]
            for diagnostic in check["diagnostics"]
        )
        secondary_records = directory / "secondary-records.json"
        secondary_records.write_text(
            json.dumps(
                {"format": "gdd-records/0.2", "project_id": "smoke", "records": {}}
            )
        )
        run_step(
            "unsupported-primary-with-secondary-records",
            [
                "validate",
                str(secondary_records),
                "--model",
                str(deep_model),
                "--profile",
                "records",
            ],
            expected=2,
        )
        assert not any(
            diagnostic["code"] == "RECORD_INVALID"
            for check in results[-1]["report"]["checks"]
            for diagnostic in check["diagnostics"]
        )
        assert (
            output.read_bytes() == (root / "examples/concert.graph.html").read_bytes()
        )
        assert (directory / "recovered.json").read_bytes() == (
            model / "graphify/raw.ast.json"
        ).read_bytes()
        proposal = json.loads((directory / "proposal.json").read_text())
        assert proposal["status"] == "proposed"
        raw_source = next(iter(proposal["model_candidate"]["sources"].values()))
        assert (directory / raw_source["uri"]).resolve() == (
            directory / "observations/graphify.raw.json"
        ).resolve()
        result = {
            "wheel": wheel.name,
            "sha256": sha256(wheel.read_bytes()).hexdigest(),
            "assets": assets,
            "outside_checkout": True,
            "import_from_installed_wheel": True,
            "html_matches_checkout_renderer": True,
            "graphify_roundtrip_exact": True,
            "proposal_source_resolves": True,
            "steps": results,
        }
        (root / "evidence/review-loops/final-wheel-smoke.json").write_text(
            json.dumps(result, indent=2) + "\n"
        )


if __name__ == "__main__":
    main()
