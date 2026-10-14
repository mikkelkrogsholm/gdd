"""Replay archived delivered patches and real checks into a fresh experiment.

This does not call agents, execute pseudocode, or reinterpret graph check labels.
The host selects fixed app/test/checker commands and creates new GDD run journals.
"""

import argparse
from copy import deepcopy
from hashlib import sha256
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import pytest

from gdd_validator import create_run, prepare_workflow, run_status
from gdd_validator.loading import load_bundle
import experiment
from experiment import write, snapshot, execute
import import_check

SOURCE = Path(__file__).resolve().parents[1]


def check_phase(root, phase, database, node):
    evidence = root / "evidence" / ("phase-" + phase)
    evidence.mkdir(parents=True, exist_ok=True)
    tests = execute([sys.executable, "-B", "-m", "pytest", "tests", "tools/test_import_check.py", "-q"], evidence / "pytest.run.json", root)
    assert tests["exit_code"] == 0, tests
    controls = import_check.run_controls(root)
    write(evidence / "import.controls.json", controls)
    assert controls["matched"], controls
    with tempfile.TemporaryDirectory(prefix="concert-capacity-negative-") as temp:
        copy = Path(temp)
        for folder in ("concert_app", "tests"):
            shutil.copytree(root / folder, copy / folder)
        path = copy / "concert_app/reservations.py"
        text = path.read_text()
        guard = 'if storage.count_active(connection, concert_id=concert_id) >= concert["capacity"]:'
        assert guard in text
        path.write_text(text.replace(guard, "if False:"))
        negative = execute([sys.executable, "-B", "-m", "pytest", "tests/test_domain.py", "-k", "zero_capacity or sold_out or concurrent_last_seat", "-q"], evidence / "capacity.bypass.run.json", copy)
        assert negative["exit_code"] == 1, negative
    server = subprocess.Popen([sys.executable, "-B", "run.py", "--database", str(database), "--port", "0"], cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        line = server.stdout.readline().strip()
        assert line.startswith("Local Sessions: "), (line, server.stderr.read())
        url = line.split(" ")[-1]
        browser_run = execute([node, "tools/browser.cjs", "--url", url, "--phase", phase, "--output", str(evidence)], evidence / "browser.command.run.json", root)
        assert browser_run["exit_code"] == 0, browser_run
        browser = json.loads((evidence / "browser.run.json").read_text())
    finally:
        server.terminate()
        server.wait(timeout=10)
    receipts = {
        "pytest_concert": {"checker": {"name": "pytest+SQLite+HTTP", "version": pytest.__version__}, "reason": "Actual replay suite passed; real SQLite rollback faults and unchanged assertions rejected a capacity bypass.", "uri": f"evidence/phase-{phase}/pytest.run.json"},
        "python_import_graph": {"checker": import_check.CHECKER, "reason": "Actual clean/injected/repaired/missing import controls yielded PASS/FAIL/PASS/UNKNOWN.", "uri": f"evidence/phase-{phase}/import.controls.json"},
        "browser_concert": {"checker": {"name": "Playwright/Chrome", "version": browser["browser"]}, "reason": "Real local browser controls passed, including committed reservation with lost response and same-key retry.", "uri": f"evidence/phase-{phase}/browser.run.json", "browser": browser["browser"]},
    }
    host = experiment.attest(phase, receipts)
    accepted = experiment.event(phase, host, "accept")
    status = run_status(root / "gdd.json", root / "runs" / ("phase-" + phase), host_context=host)
    assert status["acceptance_current"], status
    write(evidence / "acceptance.json", accepted)
    write(evidence / "status.json", status)
    write(evidence / "code.snapshot.json", snapshot(root))
    return {"spec_revision": host["policy"]["spec_revision"], "code_revision": host["code_revision"], "acceptance_current": True, "browser_cases": len(browser["cases"])}


def replay(output, node=None):
    node = node or os.environ.get("GDD_NODE", "node")
    root = Path(output).resolve()
    root.mkdir()  # Never overwrite a past run or the repository delivery.
    experiment.ROOT = root
    write(root / "replay.provenance.json", {
        "format": "concert-replay-provenance/1",
        "notice": "New execution of archived delivered app/test bytes through the listed current host tools. Historical records are not selected as new evidence.",
        "archived_delivery_git_commit": "7e27757c34b6fa57ecc69660e263c99e1420649b",
        "source": str(SOURCE),
        "tools": {path.relative_to(SOURCE).as_posix(): sha256(path.read_bytes()).hexdigest()
                  for path in sorted((SOURCE / "tools").iterdir()) if path.is_file()},
        "python": sys.version, "python_executable": sys.executable,
        "node": node, "node_executable": shutil.which(node),
        "playwright_module": os.environ.get("GDD_PLAYWRIGHT", "playwright"),
        "browser_executable": os.environ.get("GDD_CHROME", "Playwright-managed Chromium"),
    })
    (root / "tools").mkdir()
    for name in ("browser.cjs", "import_check.py", "test_import_check.py"):
        shutil.copy2(SOURCE / "tools" / name, root / "tools" / name)
    (root / "concert_app").mkdir()
    shutil.copy2(SOURCE / "history/phase-a/concert_app/__init__.py", root / "concert_app/__init__.py")
    (root / "runs").mkdir()
    (root / ".data").mkdir()
    database = root / ".data/concert.sqlite"
    summaries = {}
    for phase in ("a", "b"):
        delivery = SOURCE / "history/phase-a" if phase == "a" else SOURCE
        for name in ("gdd.json", "DESIGN.md", "HTTP.md", "REQUIREMENTS.md"):
            shutil.copy2(delivery / name, root / name)
        records = json.loads((delivery / "gdd.records.json").read_text())
        records["records"] = {key: value for key, value in records["records"].items() if value["kind"] in ("check", "decision") and f".app.{phase}." in key}
        write(root / "gdd.records.json", records)
        if phase == "b":
            shutil.copy2(SOURCE / "tests/test_requirement_change.py", root / "tests/test_requirement_change.py")
            red = execute([sys.executable, "-B", "-m", "pytest", "tests/test_requirement_change.py", "-q"], root / "evidence/phase-b/requirements.red.run.json", root)
            assert red["exit_code"] == 1, red
        request = deepcopy(json.loads((SOURCE / f"workflow.{phase}.request.json").read_text()))
        request["objective"] = "Replay archived delivered patches through new real checks; no new agent generation is claimed."
        request["base_code_revision"] = snapshot(root)["revision"]
        for package in request["packages"]:
            workspace = root / "workspaces" / (phase + "." + package["id"])
            workspace.mkdir(parents=True)
            for file in package["write_resources"]:
                target = workspace / file
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(delivery / file, target)
            package["workspace"] = str(workspace)
        host = {"implementation": request["implementation"], "code_revision": request["base_code_revision"],
                "trusted_decisions": [f"decision.app.{phase}.readiness"],
                "policy": deepcopy(json.loads((SOURCE / f"host.acceptance.{phase}.json").read_text())["policy"])}
        host["policy"]["activity"] = "implementation"
        plan = prepare_workflow(root / "gdd.json", request, host_context=host)
        write(root / f"workflow.{phase}.request.json", request)
        write(root / f"workflow.{phase}.plan.json", plan["artifact"])
        write(root / "host.implementation.json", host)
        create_run(root / "gdd.json", plan["artifact"], root / "runs" / ("phase-" + phase), host_context=host)
        for package in request["packages"]:
            dispatched = experiment.event(phase, host, "dispatch", package=package["id"], attempt=f"replay.{phase}.{package['id']}")
            write(root / f"dispatches/phase-{phase}.{package['id']}.json", dispatched["artifact"])
        for package in request["packages"]:
            review_path = f"evidence/phase-{phase}/{package['id']}.review.json"
            write(root / review_path, {"reviewer": "actor.concert.lead", "result": "PASS", "basis": "Replay exact archived lead-reviewed delivered files. Full actual integration checks are required before acceptance.", "files": {file: sha256((delivery / file).read_bytes()).hexdigest() for file in package["write_resources"]}})
            experiment.integrate(phase, package["id"], Path(package["workspace"]), review_path)
        expected_path = SOURCE / "history/phase-a/code.snapshot.json" if phase == "a" else SOURCE / "evidence/phase-b/code.snapshot.json"
        assert snapshot(root)["revision"] == json.loads(expected_path.read_text())["revision"]
        summaries[phase] = check_phase(root, phase, database, node)
        if phase == "a":
            history = root / "history/phase-a"
            history.mkdir(parents=True)
            for name in ("gdd.json", "gdd.records.json", "DESIGN.md", "HTTP.md", "REQUIREMENTS.md", "host.acceptance.a.json"):
                shutil.copy2(root / name, history / name)
            for folder in ("concert_app", "tests"):
                shutil.copytree(root / folder, history / folder, ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    result = {"format": "concert-replay/0.1", "new_agent_generation": False, "phases": summaries}
    write(root / "summary.json", result)
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--node", default=os.environ.get("GDD_NODE", "node"))
    args = parser.parse_args()
    print(json.dumps(replay(args.output, args.node), indent=2))
