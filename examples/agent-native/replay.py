"""Replay archived native-agent patches through real checks and a new GDD run.

This host-owned demo script executes only its fixed stdlib unittest commands.
It transports no agent tasks and never executes graph/source/check-label text.
"""

import argparse
from hashlib import sha256
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from gdd_validator import advance_run, create_run, prepare_workflow, run_status
from gdd_validator.loading import digest, load_bundle, parse

SOURCE = Path(__file__).resolve().parent


def write(path, value):
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


def code_snapshot(root):
    files = {
        p.relative_to(root).as_posix(): sha256(p.read_bytes()).hexdigest()
        for folder in ("quote_demo", "tests")
        for p in sorted((root / folder).glob("*.py"))
    }
    return {
        "format": "demo-code-snapshot/0.1",
        "files": files,
        "revision": digest(files),
    }


def run_tests(root, name, module=None):
    argv = [sys.executable, "-B", "-m", "unittest"] + (
        [module, "-v"] if module else ["discover", "-s", "tests", "-v"]
    )
    result = subprocess.run(
        argv,
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
        env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
    )
    receipt = {
        "argv": argv,
        "exit_code": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "python": sys.version,
        "code_revision": code_snapshot(root)["revision"],
    }
    write(root / "evidence" / (name + ".run.json"), receipt)
    return receipt


def record_verification(root, revision, result):
    """Host attests the actual run and exact local code/test bytes, not worker PASS."""
    model = root / "model.gdd.json"
    records_path = root / "model.records.json"
    records = parse(records_path.read_bytes())
    check = records["records"]["check.demo.all"]
    binding_id, evidence_id = "binding.demo.verified", "evidence.demo.final"
    if binding_id in records["records"] or evidence_id in records["records"]:
        raise ValueError(
            "Demo verification record IDs already exist; preserve history."
        )
    implementation = "implementation.quote_demo.python"
    records["records"][binding_id] = {
        "kind": "binding",
        "spec_revision": load_bundle(model).revision,
        "targets": check["scope"],
        "implementation": implementation,
        "role": "test",
        "resource": "tests/",
        "selector": {"kind": "whole-document"},
        "status": "checked",
        "revision": digest(
            {
                p.name: sha256(p.read_bytes()).hexdigest()
                for p in sorted((root / "tests").glob("*.py"))
            }
        ),
        "code_revision": revision,
    }
    records["records"][evidence_id] = {
        "kind": "evidence",
        "check": "check.demo.all",
        "check_revision": digest(check),
        "implementation": implementation,
        "spec_revision": load_bundle(model).revision,
        "code_revision": revision,
        "checker": {"name": "stdlib.unittest", "version": sys.version.split()[0]},
        "scope": check["scope"],
        "result": "PASS" if result["exit_code"] == 0 else "FAIL",
        "executed": True,
        "reason": "Actual integrated stdlib suite and real CLI executed; archived half-up negative control failed the unchanged floor assertions.",
        "environment": {"python": sys.version},
        "evidence_uri": "evidence/final.run.json",
    }
    write(records_path, records)
    return {
        "implementation": implementation,
        "code_revision": revision,
        "trusted_evidence": [evidence_id],
        "verified_bindings": [binding_id],
        "supported_methods": [check["method"]],
        "environment": {"python": sys.version},
        "negative_controls": check["negative_controls"],
    }


def replay(output):
    output = Path(output)
    output.mkdir()  # Exclusive destination: never replace a previous demonstration.
    (output / "evidence").mkdir()
    for name in ("model.gdd.json", "REQUIREMENTS.md"):
        shutil.copy2(SOURCE / name, output / name)
    shutil.copy2(SOURCE / "model.records.plan.json", output / "model.records.json")
    for name in ("quote_demo", "tests"):
        shutil.copytree(SOURCE / "baseline" / name, output / name)
    baseline = run_tests(output, "baseline")
    assert baseline["exit_code"] == 1 and "FAIL:" in baseline["stderr"], baseline
    request = parse((SOURCE / "workflow.request.json").read_bytes())
    request["base_code_revision"] = code_snapshot(output)["revision"]
    plan = prepare_workflow(output / "model.gdd.json", request)["artifact"]
    write(output / "plan.json", plan)
    host = parse((SOURCE / "host.implementation.json").read_bytes())
    host["code_revision"] = request["base_code_revision"]
    model, directory = output / "model.gdd.json", output / "run"
    create_run(model, plan, directory)

    def event(kind, **fields):
        value = {
            "type": kind,
            "idempotency_key": kind + "." + fields.get("package", "run"),
            "expected_revision": run_status(model, directory)["revision"],
            **fields,
        }
        result = advance_run(model, directory, value, host_context=host)
        assert result["applied"], result
        return result

    for name in ("discount", "tax"):
        event("dispatch", package="package." + name, attempt="replay." + name)
    for name, changed in (
        ("discount", "quote_demo/discount.py"),
        ("tax", "quote_demo/tax.py"),
        ("quote", "tests/test_quote.py"),
    ):
        if name == "quote":
            assert run_status(model, directory)["available_packages"] == [
                "package.quote"
            ]
            event("dispatch", package="package.quote", attempt="replay.quote")
        event(
            "submit",
            package="package." + name,
            attempt="replay." + name,
            summary="Replay the archived native-agent result; no new LLM execution.",
            artifact_uri=str(SOURCE / changed),
            changed_resources=[changed],
        )
        base = run_status(model, directory)["state"]["code_revision"]
        shutil.copy2(SOURCE / changed, output / changed)
        reviewed = run_tests(output, name, "tests.test_" + name)
        assert reviewed["exit_code"] == 0, reviewed
        host["code_revision"] = code_snapshot(output)["revision"]
        event(
            "integrate",
            package="package." + name,
            attempt="replay." + name,
            base_code_revision=base,
            code_revision=host["code_revision"],
            verified_resources=[changed],
            review={
                "reviewer": "actor.demo_lead",
                "verdict": "PASS",
                "evidence_uri": "evidence/" + name + ".run.json",
            },
        )
    final = run_tests(output, "final")
    assert final["exit_code"] == 0, final
    verified = record_verification(output, host["code_revision"], final)
    host = {**host, **verified, "policy": {**host["policy"], "activity": "acceptance"}}
    write(output / "host.acceptance.json", host)
    accepted = event("accept")
    status = run_status(model, directory, host_context=host)
    assert status["acceptance_current"], status
    write(output / "code.snapshot.json", code_snapshot(output))
    write(output / "acceptance.json", accepted)
    write(output / "status.json", status)
    summary = {
        "format": "gdd-workflow-demo/0.1",
        "baseline_exit": baseline["exit_code"],
        "final_exit": final["exit_code"],
        "acceptance_current": status["acceptance_current"],
        "events": status["event_count"],
        "spec_revision": plan["spec_revision"],
        "code_revision": host["code_revision"],
    }
    write(output / "summary.json", summary)
    return summary


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(replay(args.output), indent=2))
