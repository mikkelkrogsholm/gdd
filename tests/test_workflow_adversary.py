"""Independent workflow adversarial regressions; writes only to pytest tmp_path."""

from concurrent.futures import ThreadPoolExecutor
from copy import deepcopy
from pathlib import Path
from threading import Barrier
import json
import subprocess
import sys

import pytest
from gdd_validator.loading import digest, load_bundle
from gdd_validator.workflow import prepare_workflow, verify_workflow
import gdd_validator.workflow_run as run


@pytest.fixture
def case(tmp_path):
    path = tmp_path / "model.json"
    path.write_text(
        json.dumps(
            {
                "format": "gdd/0.2",
                "project": {"id": "probe", "title": "Probe", "goal": "Workflow"},
                "nodes": {
                    "entity.x": {
                        "kind": "entity",
                        "description": "Value",
                        "maturity": "specified",
                        "fields": {},
                    }
                },
                "edges": [],
            }
        )
    )
    revision = load_bundle(path).revision
    request = {
        "format": "gdd-work-request/0.1",
        "id": "work.probe",
        "objective": "Implement value",
        "implementation": "impl.good",
        "base_code_revision": "code.base",
        "packages": [
            {
                "id": "package.a",
                "scope": ["entity.x"],
                "write_resources": ["src/a.py"],
                "read_resources": [],
                "workspace": "worktree/a",
                "deliverables": ["Implement value"],
                "integration_plan": "Review and integrate",
            }
        ],
        "serialized": [],
        "integration": {"owner": "host", "checks": []},
    }
    host = {
        "implementation": "impl.good",
        "code_revision": "code.base",
        "policy": {
            "revision": "policy.x",
            "actor": "host",
            "activity": "implementation",
            "spec_revision": revision,
            "scope": ["entity.x"],
            "requirement": "none",
            "authorities": ["host"],
            "basis": "Explicit trusted host mandate",
        },
    }
    plan = prepare_workflow(path, request, host_context=host)["artifact"]
    return path, request, host, plan


def transition(path, directory, host, kind, key, **fields):
    return {
        "type": kind,
        "idempotency_key": key,
        "expected_revision": run.run_status(path, directory, host_context=host)[
            "revision"
        ],
        **fields,
    }


@pytest.mark.parametrize("bad", [[], False, "", 0])
@pytest.mark.parametrize(
    "operation", ["prepare", "verify", "create", "status", "event"]
)
def test_falsy_nonobject_host_is_rejected(case, bad, operation):
    path, request, host, plan = case
    directory = path.parent / "run"
    if operation in {"status", "event"}:
        run.create_run(path, plan, directory, host_context=host)
    with pytest.raises(ValueError):
        if operation == "prepare":
            prepare_workflow(path, request, host_context=bad)
        elif operation == "verify":
            verify_workflow(path, plan, host_context=bad)
        elif operation == "create":
            run.create_run(path, plan, directory, host_context=bad)
        elif operation == "status":
            run.run_status(path, directory, host_context=bad)
        else:
            run.advance_run(
                path,
                directory,
                transition(path, directory, host, "cancel", "cancel", reason="Stop"),
                host_context=bad,
            )
    if operation == "create":
        assert not directory.exists()
    if operation == "event":
        assert run.run_status(path, directory, host_context=host)["event_count"] == 0


@pytest.mark.parametrize("tamper", ["slice", "missing_packet", "revision", "scope"])
def test_altered_plan_never_publishes(case, tamper):
    path, request, host, plan = case
    bad = deepcopy(plan)
    if tamper == "slice":
        bad["packages"][0]["model_slice"]["nodes"]["entity.x"][
            "description"
        ] = "Injected"
    elif tamper == "missing_packet":
        bad["packages"] = [{}]
    elif tamper == "revision":
        bad["spec_revision"] = "sha256:" + "0" * 64
    else:
        bad["packages"][0]["scope"] = []
    bad["revision"] = digest(
        {key: value for key, value in bad.items() if key != "revision"}
    )
    directory = path.parent / "bad-run"
    with pytest.raises(ValueError):
        run.create_run(path, bad, directory, host_context=host)
    assert not directory.exists()


def test_hash_consistent_malformed_stored_packet_is_controlled(case):
    path, request, host, plan = case
    bad = deepcopy(plan)
    bad["packages"] = [{}]
    bad["revision"] = digest(
        {key: value for key, value in bad.items() if key != "revision"}
    )
    directory = path.parent / "bad-log"
    directory.mkdir()
    (directory / "events").mkdir()
    manifest = {"format": "gdd-run/0.1", "plan": bad}
    manifest["revision"] = digest(manifest)
    (directory / "run.json").write_text(json.dumps(manifest))
    with pytest.raises(ValueError):
        run.run_status(path, directory, host_context=host)
    result = subprocess.run(
        [
            sys.executable,
            "-B",
            "-m",
            "gdd_validator",
            "workflow",
            "status",
            str(path),
            "--run",
            str(directory),
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1 and "Traceback" not in result.stderr


@pytest.mark.parametrize(
    "change", [{"implementation": "impl.other"}, {"code_revision": "code.other"}]
)
@pytest.mark.parametrize("kind", ["dispatch", "resume"])
def test_work_transitions_require_current_host_binding(case, change, kind):
    path, request, host, plan = case
    directory = path.parent / "run"
    run.create_run(path, plan, directory, host_context=host)
    if kind == "resume":
        run.advance_run(
            path,
            directory,
            transition(path, directory, host, "cancel", "cancel", reason="Stop"),
            host_context=host,
        )
    before = run.run_status(path, directory, host_context=host)
    event = transition(
        path,
        directory,
        host,
        kind,
        "operation",
        **(
            {"package": "package.a", "attempt": "attempt.a"}
            if kind == "dispatch"
            else {"reason": "Continue"}
        ),
    )
    result = run.advance_run(path, directory, event, host_context={**host, **change})
    assert not result["applied"]
    after = run.run_status(path, directory, host_context=host)
    assert after["revision"] == before["revision"] and after["state"] == before["state"]


def test_dangling_model_is_controlled_before_packet_derivation(case):
    path, request, host, plan = case
    model = json.loads(path.read_text())
    model["nodes"]["entity.x"]["fields"] = {
        "v": {"type": "reference", "target": "entity.missing"}
    }
    path.write_text(json.dumps(model))
    request_path = path.parent / "request.json"
    request_path.write_text(json.dumps(request))
    with pytest.raises(ValueError):
        prepare_workflow(path, request, host_context=host)
    output = path.parent / "plan.json"
    result = subprocess.run(
        [
            sys.executable,
            "-B",
            "-m",
            "gdd_validator",
            "workflow",
            "plan",
            str(path),
            "--request",
            str(request_path),
            "--output",
            str(output),
        ],
        capture_output=True,
        text=True,
    )
    assert (
        result.returncode == 1
        and "Traceback" not in result.stderr
        and not output.exists()
    )


@pytest.mark.parametrize("field", ["id", "package", "serialized"])
def test_newline_is_not_a_workflow_identifier(case, field):
    path, request, host, plan = case
    bad = deepcopy(request)
    if field == "id":
        bad["id"] += "\n"
    elif field == "package":
        bad["packages"][0]["id"] += "\n"
    else:
        bad["serialized"] = [["package.a\n", "package.a"]]
    with pytest.raises(ValueError):
        prepare_workflow(path, bad, host_context=host)


def test_cancel_retains_submission_replay_resume_and_integration(case):
    path, request, host, plan = case
    directory = path.parent / "run"
    run.create_run(path, plan, directory, host_context=host)
    run.advance_run(
        path,
        directory,
        transition(
            path,
            directory,
            host,
            "dispatch",
            "dispatch",
            package="package.a",
            attempt="attempt.a",
        ),
        host_context=host,
    )
    run.advance_run(
        path,
        directory,
        transition(path, directory, host, "cancel", "cancel", reason="Stop"),
        host_context=host,
    )
    submission = transition(
        path,
        directory,
        host,
        "submit",
        "submit",
        package="package.a",
        attempt="attempt.a",
        summary="Completed before cancel",
        artifact_uri="result.json",
        changed_resources=["src/a.py"],
    )
    result = run.advance_run(path, directory, submission, host_context=host)
    assert (
        result["applied"]
        and result["state"]["status"] == "cancelled"
        and result["state"]["packages"]["package.a"]["status"] == "submitted"
    )
    assert run.advance_run(path, directory, submission, host_context=host)["replayed"]
    run.advance_run(
        path,
        directory,
        transition(path, directory, host, "resume", "resume", reason="Continue"),
        host_context=host,
    )
    event = transition(
        path,
        directory,
        host,
        "integrate",
        "integrate",
        package="package.a",
        attempt="attempt.a",
        base_code_revision="code.base",
        code_revision="code.new",
        review={"reviewer": "host", "verdict": "PASS", "evidence_uri": "review.json"},
        verified_resources=["src/a.py"],
    )
    result = run.advance_run(
        path, directory, event, host_context={**host, "code_revision": "code.new"}
    )
    assert (
        result["applied"]
        and result["state"]["packages"]["package.a"]["status"] == "integrated"
        and result["state"]["code_revision"] == "code.new"
    )


def test_concurrent_publication_is_one_append_then_replay(case, monkeypatch):
    path, request, host, plan = case
    directory = path.parent / "run"
    initial = run.create_run(path, plan, directory, host_context=host)
    events = [
        {
            "type": "cancel",
            "idempotency_key": f"cancel.{n}",
            "expected_revision": initial["revision"],
            "reason": "Concurrent stop",
        }
        for n in (1, 2)
    ]
    barrier = Barrier(2)
    original = run._load

    def synchronized(directory):
        loaded = original(directory)
        barrier.wait(timeout=10)
        return loaded

    monkeypatch.setattr(run, "_load", synchronized)

    def publish(event):
        try:
            return run.advance_run(path, directory, event, host_context=host)
        except FileExistsError:
            return {"conflict": True}

    with ThreadPoolExecutor(max_workers=2) as pool:
        results = list(pool.map(publish, events))
    monkeypatch.setattr(run, "_load", original)
    assert sum(result.get("applied", False) for result in results) == 1
    assert run.run_status(path, directory, host_context=host)["event_count"] == 1
    assert not list((directory / "events").glob(".pending-*"))
    winner = next(i for i, result in enumerate(results) if result.get("applied"))
    assert run.advance_run(path, directory, events[winner], host_context=host)[
        "replayed"
    ]
