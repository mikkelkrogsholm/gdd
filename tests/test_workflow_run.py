"""Operational regressions; receipts here are explicitly synthetic test data."""

from concurrent.futures import ThreadPoolExecutor
from copy import deepcopy
import json
from pathlib import Path
import shutil
import subprocess
import sys

import pytest
from jsonschema import Draft202012Validator

from gdd_validator import (
    prepare_workflow,
    verify_workflow,
    create_run,
    run_status,
    advance_run,
)
from gdd_validator.loading import load_bundle, parse, schema
from gdd_validator.report import aggregate

ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture
def workflow(tmp_path):
    directory = tmp_path / "examples"
    shutil.copytree(ROOT / "examples", directory)
    model = directory / "concert.gdd.yaml"
    bundle = load_bundle(model)
    request = {
        "format": "gdd-work-request/0.1",
        "id": "work.concert",
        "objective": "Implement two actions",
        "implementation": "implementation.test",
        "base_code_revision": "synthetic-c0",
        "packages": [
            {
                "id": "work." + name,
                "scope": ["action." + name],
                "workspace": "../workspace/" + name,
                "write_resources": ["src/" + name + ".py"],
                "read_resources": [],
                "deliverables": ["Action implementation and tests"],
                "integration_plan": "Review patch and rerun shared checks",
            }
            for name in ("reserve", "cancel")
        ],
        "serialized": [],
        "integration": {
            "owner": "actor.integrator",
            "checks": [i for i, r in bundle.records.items() if r["kind"] == "check"],
        },
    }
    host = {
        "implementation": request["implementation"],
        "code_revision": "synthetic-c0",
        "policy": {
            "revision": "test-policy",
            "actor": "actor.integrator",
            "activity": "implementation",
            "spec_revision": bundle.revision,
            "scope": [],
            "requirement": "none",
            "authorities": ["actor.integrator"],
            "basis": "Synthetic test mandate",
        },
    }
    prepared = prepare_workflow(model, request)
    assert aggregate(prepared["report"]["checks"], "readiness") == "PASS"
    run = tmp_path / "run"
    create_run(model, prepared["artifact"], run)
    return model, prepared["artifact"], run, host


def publish(workflow, kind, *, host=None, **fields):
    model, _, directory, default_host = workflow
    event = {
        "idempotency_key": "event." + str(run_status(model, directory)["event_count"]),
        "expected_revision": run_status(model, directory)["revision"],
        "type": kind,
        **fields,
    }
    return (
        advance_run(
            model,
            directory,
            event,
            host_context=(
                {
                    **default_host,
                    "code_revision": run_status(model, directory)["state"][
                        "code_revision"
                    ],
                }
                if host is None
                else host
            ),
        ),
        event,
    )


def submit(workflow, name="reserve", attempt="attempt.1"):
    publish(workflow, "dispatch", package="work." + name, attempt=attempt)
    return publish(
        workflow,
        "submit",
        package="work." + name,
        attempt=attempt,
        summary="Synthetic code claim",
        artifact_uri="fixture://patch",
        changed_resources=["src/" + name + ".py"],
    )


def integrate(workflow, name="reserve", attempt="attempt.1", revision="synthetic-c1"):
    model, _, directory, host = workflow
    return publish(
        workflow,
        "integrate",
        package="work." + name,
        attempt=attempt,
        base_code_revision=run_status(model, directory)["state"]["code_revision"],
        code_revision=revision,
        review={
            "reviewer": "actor.integrator",
            "verdict": "PASS",
            "evidence_uri": "fixture://review",
        },
        verified_resources=["src/" + name + ".py"],
        host={**host, "code_revision": revision},
    )


def test_parallel_code_base_is_distinct_from_spec_revision(workflow):
    model, plan, directory, _ = workflow
    assert plan["packages"][0]["base_revision"] == plan["spec_revision"]
    assert run_status(model, directory)["available_packages"] == [
        "work.cancel",
        "work.reserve",
    ]


def test_dispatch_requires_current_authority_and_never_executes(workflow):
    model, _, directory, _ = workflow
    result, _ = publish(
        workflow, "dispatch", package="work.reserve", attempt="a", host={}
    )
    assert result["applied"] is False
    assert run_status(model, directory)["event_count"] == 0
    assert "artifact" not in result


def test_retry_after_commit_returns_receipt_without_redispatch(workflow):
    model, _, directory, host = workflow
    first, event = publish(workflow, "dispatch", package="work.reserve", attempt="a")
    publish(workflow, "cancel", reason="Synthetic stop")
    retry = advance_run(model, directory, event, host_context=host)
    assert first["applied"] and retry["replayed"] and not retry["applied"]
    assert "artifact" not in retry
    assert retry["status"]["state"]["status"] == "cancelled"
    assert retry["status"]["event_count"] == 2


def test_idempotency_key_cannot_change_request(workflow):
    model, _, directory, host = workflow
    _, event = publish(workflow, "dispatch", package="work.reserve", attempt="a")
    with pytest.raises(ValueError, match="different request"):
        advance_run(model, directory, {**event, "attempt": "b"}, host_context=host)


def test_one_concurrent_event_wins(workflow):
    model, _, directory, host = workflow
    head = run_status(model, directory)["revision"]

    def writer(name):
        event = {
            "idempotency_key": name,
            "expected_revision": head,
            "type": "dispatch",
            "package": "work." + name,
            "attempt": name,
        }
        try:
            return advance_run(model, directory, event, host_context=host)["applied"]
        except (ValueError, FileExistsError):
            return False

    with ThreadPoolExecutor(max_workers=2) as pool:
        assert sum(pool.map(writer, ["reserve", "cancel"])) == 1
    assert run_status(model, directory)["event_count"] == 1


def test_cancellation_preserves_late_result_but_blocks_integration(workflow):
    model, _, directory, _ = workflow
    publish(workflow, "dispatch", package="work.reserve", attempt="a")
    publish(workflow, "cancel", reason="Synthetic cancellation")
    publish(
        workflow,
        "submit",
        package="work.reserve",
        attempt="a",
        summary="Late claim",
        artifact_uri="fixture://late",
        changed_resources=["src/reserve.py"],
    )
    with pytest.raises(ValueError, match="Cancelled"):
        integrate(workflow, attempt="a")
    status = run_status(model, directory)
    assert status["state"]["status"] == "cancelled"
    assert status["state"]["packages"]["work.reserve"]["status"] == "submitted"
    publish(workflow, "resume", reason="Explicit resume")
    assert integrate(workflow, attempt="a")[0]["applied"]


def test_retry_does_not_reuse_attempt_or_promote_old_results(workflow):
    submit(workflow)
    publish(workflow, "retry", package="work.reserve", reason="Synthetic failed review")
    with pytest.raises(ValueError, match="cannot be reused"):
        publish(workflow, "dispatch", package="work.reserve", attempt="attempt.1")
    publish(workflow, "dispatch", package="work.reserve", attempt="attempt.2")
    with pytest.raises(ValueError, match="current dispatched"):
        publish(
            workflow,
            "submit",
            package="work.reserve",
            attempt="attempt.1",
            summary="Old claim",
            artifact_uri="fixture://old",
            changed_resources=["src/reserve.py"],
        )


def test_worker_cannot_import_host_trust(workflow):
    publish(workflow, "dispatch", package="work.reserve", attempt="a")
    with pytest.raises(ValueError, match="Invalid workflow event"):
        publish(
            workflow,
            "submit",
            package="work.reserve",
            attempt="a",
            summary="Claim",
            artifact_uri="fixture://claim",
            changed_resources=["src/reserve.py"],
            trusted_evidence=["evil"],
        )


@pytest.mark.parametrize(
    "resource", ["../escape.py", "src/cancel.py", "src/", "src/*.py", "/tmp/escape"]
)
def test_submit_must_stay_inside_declared_files(workflow, resource):
    publish(workflow, "dispatch", package="work.reserve", attempt="a")
    with pytest.raises(ValueError, match="ownership"):
        publish(
            workflow,
            "submit",
            package="work.reserve",
            attempt="a",
            summary="Claim",
            artifact_uri="fixture://claim",
            changed_resources=[resource],
        )


def test_integration_rechecks_policy_and_code_binding(workflow):
    _, _, _, host = workflow
    submit(workflow)
    denied = {**host, "policy": {**host["policy"], "denied": True}}
    result, _ = publish(
        workflow,
        "integrate",
        package="work.reserve",
        attempt="attempt.1",
        base_code_revision="synthetic-c0",
        code_revision="synthetic-c1",
        review={
            "reviewer": "lead",
            "verdict": "PASS",
            "evidence_uri": "fixture://review",
        },
        verified_resources=["src/reserve.py"],
        host=denied,
    )
    assert not result["applied"]
    assert aggregate(result["report"]["checks"], "authorization") == "DENIED"
    assert integrate(workflow)[0]["applied"]
    with pytest.raises(ValueError, match="unfinished"):
        publish(workflow, "retry", package="work.reserve", reason="Too late")


def test_dependency_released_after_integration_not_submit(workflow, tmp_path):
    model, plan, _, host = workflow
    request = deepcopy(plan["request"])
    request["serialized"] = [["work.reserve", "work.cancel"]]
    plan = prepare_workflow(model, request)["artifact"]
    directory = tmp_path / "ordered"
    create_run(model, plan, directory)
    ordered = model, plan, directory, host
    assert run_status(model, directory)["available_packages"] == ["work.reserve"]
    submit(ordered)
    assert run_status(model, directory)["available_packages"] == []
    integrate(ordered)
    assert run_status(model, directory)["available_packages"] == ["work.cancel"]


def test_acceptance_requires_current_actual_evidence(workflow):
    _, _, _, host = workflow
    submit(workflow)
    integrate(workflow)
    submit(workflow, "cancel", "attempt.2")
    integrate(workflow, "cancel", "attempt.2", "synthetic-c2")
    acceptance = {
        **host,
        "code_revision": "synthetic-c2",
        "policy": {**host["policy"], "activity": "acceptance"},
    }
    result, _ = publish(workflow, "accept", host=acceptance)
    assert not result["applied"]
    assert aggregate(result["report"]["checks"], "authorization") == "GRANTED"
    assert aggregate(result["report"]["checks"], "implementation") == "UNKNOWN"


def test_log_tamper_is_rejected(workflow):
    model, _, directory, _ = workflow
    publish(workflow, "dispatch", package="work.reserve", attempt="a")
    path = directory / "events/000001.json"
    value = parse(path.read_bytes())
    value["event"]["attempt"] = "tampered"
    path.write_text(json.dumps(value))
    with pytest.raises(ValueError, match="hash chain"):
        run_status(model, directory)


def test_report_matches_core_report_schema(workflow):
    model, plan, _, _ = workflow
    Draft202012Validator(schema("report")).validate(verify_workflow(model, plan))


def test_cli_plan_is_consumable_and_never_overwrites(workflow, tmp_path):
    model, plan, _, _ = workflow
    request = tmp_path / "request.json"
    request.write_text(json.dumps(plan["request"]))
    output = tmp_path / "plan.json"
    command = [
        sys.executable,
        "-m",
        "gdd_validator",
        "workflow",
        "plan",
        str(model),
        "--request",
        str(request),
        "--output",
        str(output),
    ]
    first = subprocess.run(command, capture_output=True, text=True)
    assert first.returncode == 0, first.stderr
    assert parse(output.read_bytes())["format"] == "gdd-workplan/0.1"
    before = output.read_bytes()
    assert subprocess.run(command, capture_output=True).returncode == 1
    assert output.read_bytes() == before
