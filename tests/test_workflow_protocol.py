"""Protocol regressions and independent controls for the local workflow layer."""

from concurrent.futures import ThreadPoolExecutor
from copy import deepcopy
import json

import pytest

from gdd_validator import (
    advance_run,
    create_run,
    prepare_workflow,
    run_status,
    validate,
    verify_workflow,
)
from gdd_validator.loading import digest, load_bundle
from gdd_validator.report import aggregate


def make_case(directory, multi=False, question=False):
    directory.mkdir()
    names = ["x", "y"] if multi else ["x"]
    model = {
        "format": "gdd/0.2",
        "project": {
            "id": "workflow.probe",
            "title": "Workflow",
            "goal": "Bounded protocol",
        },
        "nodes": {},
        "edges": [],
        "record_files": ["records.json"],
    }
    for name in names:
        model["nodes"]["entity." + name] = {
            "kind": "entity",
            "description": "Value",
            "fields": {},
        }
        model["nodes"]["rule." + name] = {
            "kind": "rule",
            "description": "Invariant",
            "rule": {
                "category": "invariant",
                "statement": "Required behavior",
                "scope": "targeted",
                "blocking": True,
                "verification": ["probe"],
            },
        }
        model["nodes"]["scenario." + name] = {
            "kind": "scenario",
            "description": "Verify invariant",
            "scenario": {
                "category": "normal",
                "given": "Value",
                "when": "Observed",
                "then": "Valid",
            },
        }
        model["edges"].extend(
            [
                {
                    "id": "edge.constraint." + name,
                    "from": "rule." + name,
                    "relation": "constrains",
                    "to": "entity." + name,
                },
                {
                    "id": "edge.verify." + name,
                    "from": "scenario." + name,
                    "relation": "verifies",
                    "to": "rule." + name,
                },
            ]
        )
    if question:
        model["questions"] = {
            "question.accept": {
                "statement": "Resolve acceptance",
                "targets": ["entity.x"],
                "blocks": ["acceptance"],
                "status": "open",
            }
        }
    path = directory / "model.json"
    path.write_text(json.dumps(model))
    records = {}

    def write():
        (directory / "records.json").write_text(
            json.dumps(
                {
                    "format": "gdd-records/0.2",
                    "project_id": "workflow.probe",
                    "records": records,
                }
            )
        )

    write()
    revision = load_bundle(path).revision
    assert revision is not None
    for name in names:
        records["check." + name] = {
            "kind": "check",
            "spec_revision": revision,
            "scope": ["rule." + name],
            "method": "probe",
            "required_environment": [],
            "negative_controls": [],
        }
    write()
    request = {
        "format": "gdd-work-request/0.1",
        "id": "run.probe",
        "objective": "Implement entities",
        "implementation": "implementation.probe",
        "base_code_revision": "code.base",
        "packages": [
            {
                "id": "package." + name,
                "scope": ["entity." + name],
                "write_resources": [name + ".py"],
                "read_resources": [],
                "workspace": "workspaces/" + name,
                "deliverables": ["Entity implementation"],
                "integration_plan": "Merge and check",
            }
            for name in names
        ],
        "serialized": [],
        "integration": {
            "owner": "actor.probe",
            "checks": ["check." + name for name in names],
        },
    }
    host = {
        "implementation": "implementation.probe",
        "code_revision": "code.base",
        "policy": {
            "revision": "policy.probe",
            "actor": "actor.probe",
            "activity": "implementation",
            "spec_revision": revision,
            "scope": [],
            "requirement": "standing",
            "authorities": ["actor.probe"],
            "basis": "Synthetic bounded host mandate",
        },
    }
    return {
        "path": path,
        "records": records,
        "write": write,
        "request": request,
        "host": host,
        "directory": directory / "run",
    }


def start(case):
    prepared = prepare_workflow(
        case["path"], case["request"], host_context=case["host"]
    )
    case["plan"] = prepared["artifact"]
    return create_run(
        case["path"], case["plan"], case["directory"], host_context=case["host"]
    )


def step(case, kind, **payload):
    status = run_status(case["path"], case["directory"], host_context=case["host"])
    event = {
        "type": kind,
        "idempotency_key": kind + "." + str(status["event_count"]),
        "expected_revision": status["revision"],
        **payload,
    }
    return advance_run(
        case["path"], case["directory"], event, host_context=case["host"]
    )


def submit(case):
    return step(
        case,
        "submit",
        package="package.x",
        attempt="attempt.1",
        summary="Untrusted completed report",
        artifact_uri="fixture://patch",
        changed_resources=["x.py"],
    )


def integrate(case, **overrides):
    payload = {
        "package": "package.x",
        "attempt": "attempt.1",
        "base_code_revision": "code.base",
        "code_revision": "code.result",
        "review": {
            "reviewer": "reviewer.probe",
            "verdict": "PASS",
            "evidence_uri": "fixture://review",
        },
        "verified_resources": ["x.py"],
        **overrides,
    }
    return step(case, "integrate", **payload)


def completed(case):
    start(case)
    assert step(case, "dispatch", package="package.x", attempt="attempt.1")["applied"]
    assert submit(case)["applied"]
    case["host"]["code_revision"] = "code.result"
    assert integrate(case)["applied"]


def attach_evidence(case):
    records, host = case["records"], case["host"]
    revision = host["policy"]["spec_revision"]
    records["binding.x"] = {
        "kind": "binding",
        "spec_revision": revision,
        "targets": ["rule.x"],
        "implementation": "implementation.probe",
        "role": "test",
        "resource": "x.py",
        "selector": {"kind": "whole-document"},
        "status": "checked",
        "revision": "resource.result",
        "code_revision": "code.result",
    }
    records["evidence.x"] = {
        "kind": "evidence",
        "check": "check.x",
        "check_revision": digest(records["check.x"]),
        "implementation": "implementation.probe",
        "spec_revision": revision,
        "code_revision": "code.result",
        "checker": {"name": "probe", "version": "1"},
        "scope": ["rule.x"],
        "result": "PASS",
        "executed": True,
        "reason": "Synthetic test assertion",
        "environment": {},
        "evidence_uri": "fixture://raw",
    }
    case["write"]()
    host.update(
        supported_methods=["probe"],
        trusted_evidence=["evidence.x"],
        verified_bindings=["binding.x"],
        environment={},
        negative_controls=[],
    )
    host["policy"]["activity"] = "acceptance"


@pytest.mark.parametrize("multi", [False, True])
@pytest.mark.parametrize("defect", ["historical", "unused-current-invalid", "none"])
def test_unrelated_records_preserve_scoped_dispatch(tmp_path, multi, defect):
    case = make_case(tmp_path / "case", multi=multi)
    if defect != "none":
        case["records"]["decision.unrelated"] = {
            "kind": "decision",
            "topic": "model_change",
            "scope": ["entity.deleted"],
            "spec_revision": (
                "sha256:" + "a" * 64
                if defect == "historical"
                else case["host"]["policy"]["spec_revision"]
            ),
            "policy_revision": "policy.old",
            "actor": {"id": "actor.probe", "kind": "human"},
            "outcome": "accepted",
            "rationale": "Unrelated record",
        }
        case["write"]()
    status = start(case)
    assert aggregate(status["report"]["checks"], "readiness") == "PASS"
    assert status["available_packages"] == (
        ["package.x", "package.y"] if multi else ["package.x"]
    )
    assert step(case, "dispatch", package="package.x", attempt="attempt.1")["applied"]


@pytest.mark.parametrize(
    "tamper", ["missing-packet-fields", "altered-slice", "stale-model"]
)
def test_create_rejects_invalid_packet_before_publication(tmp_path, tamper):
    case = make_case(tmp_path / "case")
    plan = prepare_workflow(case["path"], case["request"], host_context=case["host"])[
        "artifact"
    ]
    if tamper == "missing-packet-fields":
        plan["packages"] = [{}]
    elif tamper == "altered-slice":
        plan["packages"][0]["model_slice"]["nodes"]["entity.x"][
            "description"
        ] = "Altered intention"
    else:
        model = json.loads(case["path"].read_text())
        model["nodes"]["entity.x"]["description"] = "New model revision"
        case["path"].write_text(json.dumps(model))
    plan["revision"] = digest({k: v for k, v in plan.items() if k != "revision"})
    with pytest.raises(ValueError):
        create_run(case["path"], plan, case["directory"], host_context=case["host"])
    assert not case["directory"].exists()


@pytest.mark.parametrize("defect", ["edge", "type"])
def test_semantically_dangling_model_is_controlled_before_slice_derivation(
    tmp_path, defect
):
    case = make_case(tmp_path / "case")
    model = json.loads(case["path"].read_text())
    if defect == "edge":
        model["edges"].append(
            {
                "id": "edge.bad",
                "from": "rule.x",
                "relation": "constrains",
                "to": "entity.missing",
            }
        )
    else:
        model["nodes"]["entity.x"]["fields"] = {
            "v": {"type": "reference", "target": "entity.missing"}
        }
    case["path"].write_text(json.dumps(model))
    assert aggregate(validate(case["path"])["checks"], "model") == "FAIL"
    with pytest.raises(ValueError):
        prepare_workflow(case["path"], case["request"], host_context=case["host"])


def test_valid_unready_draft_can_be_created_without_dispatch(tmp_path):
    case = make_case(tmp_path / "case")
    model = json.loads(case["path"].read_text())
    del model["nodes"]["entity.x"]["fields"]
    case["path"].write_text(json.dumps(model))
    revision = load_bundle(case["path"]).revision
    case["records"]["check.x"]["spec_revision"] = revision
    case["host"]["policy"]["spec_revision"] = revision
    case["write"]()
    status = start(case)
    assert case["directory"].exists()
    assert not status["available_packages"]
    assert not step(case, "dispatch", package="package.x", attempt="attempt.1")[
        "applied"
    ]


@pytest.mark.parametrize(
    "variant",
    [
        "valid",
        "no-evidence",
        "untrusted",
        "wrong-code",
        "wrong-implementation",
        "wrong-activity",
        "denied",
        "check-selection",
        "hash-mismatch",
        "acceptance-question",
    ],
)
def test_current_acceptance_requires_separate_bound_gates(tmp_path, variant):
    case = make_case(tmp_path / "case", question=variant == "acceptance-question")
    completed(case)
    if variant != "no-evidence":
        attach_evidence(case)
    else:
        case["host"]["policy"]["activity"] = "acceptance"
    host = case["host"]
    if variant == "untrusted":
        host["trusted_evidence"] = []
    if variant == "wrong-code":
        host["code_revision"] = "code.other"
    if variant == "wrong-implementation":
        host["implementation"] = "implementation.other"
    if variant == "wrong-activity":
        host["policy"]["activity"] = "implementation"
    if variant == "denied":
        host["policy"]["denied"] = True
    if variant == "check-selection":
        host["check_selection"] = {"rule.x": {"probe": "check.other"}}
    if variant == "hash-mismatch":
        case["records"]["evidence.x"]["check_revision"] = "sha256:" + "a" * 64
        case["write"]()
    result = step(case, "accept")
    assert result["applied"] == (variant == "valid")
    if variant == "valid":
        status = run_status(case["path"], case["directory"], host_context=host)
        assert status["state"]["status"] == "accepted" and status["acceptance_current"]
        host["code_revision"] = "code.changed"
        status = run_status(case["path"], case["directory"], host_context=host)
        assert (
            status["state"]["status"] == "accepted" and not status["acceptance_current"]
        )


@pytest.mark.parametrize(
    "variant",
    [
        "valid",
        "denied",
        "wrong-code",
        "wrong-implementation",
        "wrong-base",
        "out-of-scope",
    ],
)
def test_integration_has_fresh_authority_code_and_scope_guards(tmp_path, variant):
    case = make_case(tmp_path / "case")
    start(case)
    step(case, "dispatch", package="package.x", attempt="attempt.1")
    submit(case)
    host = case["host"]
    host["code_revision"] = "code.result"
    if variant == "denied":
        host["policy"]["denied"] = True
    if variant == "wrong-code":
        host["code_revision"] = "code.other"
    if variant == "wrong-implementation":
        host["implementation"] = "implementation.other"
    if variant in {"wrong-base", "out-of-scope"}:
        with pytest.raises(ValueError):
            integrate(
                case,
                **(
                    {"base_code_revision": "code.wrong"}
                    if variant == "wrong-base"
                    else {"verified_resources": ["outside.py"]}
                )
            )
    else:
        assert integrate(case)["applied"] == (variant == "valid")


def test_idempotency_cancellation_and_attempt_generation(tmp_path):
    case = make_case(tmp_path / "case")
    status = start(case)
    dispatch = {
        "type": "dispatch",
        "package": "package.x",
        "attempt": "attempt.1",
        "idempotency_key": "stable.dispatch",
        "expected_revision": status["revision"],
    }
    assert advance_run(
        case["path"], case["directory"], dispatch, host_context=case["host"]
    )["applied"]
    step(case, "cancel", reason="User cancellation")
    replay = advance_run(
        case["path"], case["directory"], dispatch, host_context=case["host"]
    )
    assert replay["replayed"] and not replay["applied"] and "artifact" not in replay
    assert replay["status"]["state"]["status"] == "cancelled"
    assert submit(case)["state"]["status"] == "cancelled"
    step(case, "resume", reason="Explicit resume")
    step(case, "retry", package="package.x", reason="Fresh attempt")
    with pytest.raises(ValueError):
        step(case, "dispatch", package="package.x", attempt="attempt.1")
    step(case, "dispatch", package="package.x", attempt="attempt.2")
    with pytest.raises(ValueError):
        submit(case)


def test_compare_and_publish_has_one_concurrent_winner(tmp_path):
    case = make_case(tmp_path / "case")
    status = start(case)

    def cancel(number):
        try:
            return advance_run(
                case["path"],
                case["directory"],
                {
                    "type": "cancel",
                    "reason": "Race",
                    "idempotency_key": "cancel." + str(number),
                    "expected_revision": status["revision"],
                },
                host_context=case["host"],
            )["applied"]
        except (ValueError, OSError):
            return False

    with ThreadPoolExecutor(max_workers=2) as executor:
        outcomes = list(executor.map(cancel, [1, 2]))
    assert sum(outcomes) == 1
    status = run_status(case["path"], case["directory"], host_context=case["host"])
    assert status["event_count"] == 1 and status["state"]["status"] == "cancelled"


def test_cancel_remains_available_when_model_is_unavailable(tmp_path):
    case = make_case(tmp_path / "case")
    start(case)
    dispatched = step(case, "dispatch", package="package.x", attempt="attempt.1")
    case["path"].unlink()
    result = advance_run(
        case["path"],
        case["directory"],
        {
            "type": "cancel",
            "reason": "User cancellation",
            "idempotency_key": "cancel",
            "expected_revision": dispatched["receipt"]["revision"],
        },
        host_context=case["host"],
    )
    assert result["applied"] and result["state"]["status"] == "cancelled"
    assert len(list((case["directory"] / "events").glob("*.json"))) == 2
