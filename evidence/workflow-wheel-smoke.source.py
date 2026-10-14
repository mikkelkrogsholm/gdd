"""Verify the installed 0.5.0 wheel from outside the source checkout."""

import argparse
from hashlib import sha256
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

import gdd_validator
from gdd_validator.loading import parse


def smoke(repository):
    assert gdd_validator.__version__ == "0.5.0"
    installed = Path(gdd_validator.__file__).resolve()
    assert not installed.is_relative_to(repository)
    steps = []
    environment = {k: v for k, v in os.environ.items() if k != "PYTHONPATH"}
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    with tempfile.TemporaryDirectory(prefix="gdd-wheel-external-") as temporary:
        working = Path(temporary)
        case = working / "case"
        case.mkdir()
        source = repository / "examples/agent-native"
        for name in (
            "model.gdd.json",
            "REQUIREMENTS.md",
            "workflow.request.json",
            "host.implementation.json",
        ):
            shutil.copy2(source / name, case / name)
        shutil.copy2(source / "model.records.plan.json", case / "model.records.json")
        for name in ("quote_demo", "tests"):
            shutil.copytree(source / "baseline" / name, case / name)
        host = parse((case / "host.implementation.json").read_bytes())
        demo = runpy.run_path(str(source / "replay.py"))
        assert demo["code_snapshot"](case)["revision"] == host["code_revision"]
        model = str(case / "model.gdd.json")

        def cli(label, *arguments, expected=0):
            result = subprocess.run(
                [sys.executable, "-B", "-m", "gdd_validator", *arguments],
                cwd=working,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            assert result.returncode == expected, (
                label,
                result.returncode,
                result.stderr,
                result.stdout,
            )
            assert "Traceback" not in result.stderr
            steps.append(
                {
                    "step": label,
                    "exit": result.returncode,
                    "stdout_sha256": sha256(result.stdout.encode()).hexdigest(),
                }
            )
            return json.loads(result.stdout) if result.stdout else None

        plan_path = str(case / "plan.json")
        run_path = str(case / "run")
        cli(
            "plan",
            "workflow",
            "plan",
            model,
            "--request",
            str(case / "workflow.request.json"),
            "--output",
            plan_path,
        )
        cli("verify", "workflow", "verify", model, "--plan", plan_path)
        cli(
            "create",
            "workflow",
            "create",
            model,
            "--plan",
            plan_path,
            "--run",
            run_path,
        )
        status = cli("status", "workflow", "status", model, "--run", run_path)
        assert status["available_packages"] == ["package.discount", "package.tax"]

        def event(label, kind, **fields):
            nonlocal status
            request = {
                "type": kind,
                "expected_revision": status["revision"],
                "idempotency_key": label,
                **fields,
            }
            path = case / (label + ".event.json")
            path.write_text(json.dumps(request))
            (case / "host.json").write_text(json.dumps(host))
            result = cli(
                label,
                "workflow",
                "event",
                model,
                "--run",
                run_path,
                "--event",
                str(path),
                "--host-context",
                str(case / "host.json"),
            )
            assert result["applied"]
            status = cli(
                label + ".status", "workflow", "status", model, "--run", run_path
            )
            return path, result

        dispatch_path, _ = event(
            "dispatch", "dispatch", package="package.discount", attempt="wheel.discount"
        )
        event("cancel", "cancel", reason="Wheel retry/cancellation control")
        repeated = cli(
            "idempotent.retry",
            "workflow",
            "event",
            model,
            "--run",
            run_path,
            "--event",
            str(dispatch_path),
            "--host-context",
            str(case / "host.json"),
        )
        assert (
            repeated["replayed"]
            and not repeated["applied"]
            and "artifact" not in repeated
        )
        assert repeated["status"]["state"]["status"] == "cancelled"
        event("resume", "resume", reason="Explicit wheel resume")
        event(
            "submit",
            "submit",
            package="package.discount",
            attempt="wheel.discount",
            summary="Archived native patch",
            artifact_uri=str(source / "quote_demo/discount.py"),
            changed_resources=["quote_demo/discount.py"],
        )
        base = host["code_revision"]
        shutil.copy2(source / "quote_demo/discount.py", case / "quote_demo/discount.py")
        checked = demo[
            "run_tests"
        ]  # Same fixed, host-owned check; create its evidence directory first.
        (case / "evidence").mkdir()
        result = checked(case, "discount", "tests.test_discount")
        assert result["exit_code"] == 0
        host["code_revision"] = demo["code_snapshot"](case)["revision"]
        event(
            "integrate",
            "integrate",
            package="package.discount",
            attempt="wheel.discount",
            base_code_revision=base,
            code_revision=host["code_revision"],
            verified_resources=["quote_demo/discount.py"],
            review={
                "reviewer": "actor.demo_lead",
                "verdict": "PASS",
                "evidence_uri": "evidence/discount.run.json",
            },
        )
        assert status["available_packages"] == ["package.tax"]
        cli("context", "context", model, "--scope", "action.discount")
        cli("snapshot", "snapshot", model)

        replayed = subprocess.run(
            [
                sys.executable,
                "-B",
                str(source / "replay.py"),
                "--output",
                str(working / "replayed"),
            ],
            cwd=working,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )
        assert replayed.returncode == 0, replayed.stderr + replayed.stdout
        summary = json.loads(replayed.stdout)
        assert (
            summary["acceptance_current"]
            and summary["baseline_exit"] == 1
            and summary["final_exit"] == 0
        )
        accepted = cli(
            "accepted.current",
            "workflow",
            "status",
            str(working / "replayed/model.gdd.json"),
            "--run",
            str(working / "replayed/run"),
            "--host-context",
            str(working / "replayed/host.acceptance.json"),
        )
        assert accepted["acceptance_current"]
    runtime = {}
    with zipfile.ZipFile(
        repository / "dist/gdd_validator-0.5.0-py3-none-any.whl"
    ) as wheel:
        for name in wheel.namelist():
            if not name.startswith("gdd_validator/") or name.endswith("/"):
                continue
            local = repository / name
            if name.startswith("gdd_validator/schemas/"):
                local = repository / "schemas" / Path(name).name
            assert local.read_bytes() == wheel.read(name), name
            assert (
                installed.parent.parent / name
            ).read_bytes() == local.read_bytes(), name
            runtime[name] = sha256(local.read_bytes()).hexdigest()
    with tarfile.open(repository / "dist/gdd_validator-0.5.0.tar.gz") as archive:
        names = {name.split("/", 1)[1] for name in archive.getnames() if "/" in name}
        for name in (
            "WORKFLOW.md",
            "agents/gdd-lead.md",
            "agents/gdd-worker.md",
            "schemas/workflow.schema.json",
            "conformance/run_workflow.py",
            "conformance/workflow.cases.json",
            "examples/agent-native/replay.py",
        ):
            assert name in names, name
    return {
        "format": "gdd-workflow-wheel-smoke/0.1",
        "package_version": gdd_validator.__version__,
        "installed_module": str(installed),
        "python": sys.version,
        "outside_checkout": True,
        "cli_steps": steps,
        "replayed_demo": summary,
        "runtime_files": runtime,
        "all_runtime_bytes_match": True,
        "sdist_delivery_files_present": True,
        "passed": True,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repository", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    result = smoke(args.repository.resolve())
    with args.output.open("x", encoding="utf-8") as file:
        file.write(json.dumps(result, indent=2) + "\n")
    print(
        json.dumps(
            {
                "passed": result["passed"],
                "cli_steps": len(result["cli_steps"]),
                "runtime_files": len(result["runtime_files"]),
            }
        )
    )
