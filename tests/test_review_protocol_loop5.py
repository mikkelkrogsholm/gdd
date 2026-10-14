"""Supersedes references must have an unambiguous record identity."""

from copy import deepcopy
import json

import pytest

from gdd_validator import validate
from gdd_validator.loading import digest, load_bundle
from gdd_validator.record_integrity import invalid_record_ids
from gdd_validator.report import aggregate
from test_review_model_loop2 import write_records
from test_review_protocol_loop4 import case as proof_case, run, rule_result
from test_review_readiness_integrity import case as readiness_case


def predecessor(proof_case, kind, previous):
    _, records, host = proof_case
    used = kind + ".x"
    records[previous] = deepcopy(records[used])
    records[used]["supersedes"] = previous
    if kind == "check":
        records["evidence.x"]["check_revision"] = digest(records[used])
        host["check_selection"] = {"rule.x": {"probe": used}}
    elif kind == "evidence":
        host["evidence_selection"] = {"check.x": used}
    return used


@pytest.mark.parametrize("kind", ["check", "evidence", "binding", "decision"])
@pytest.mark.parametrize(
    "previous", ["entity.y", "source.input", "edge.x", "transition.x"]
)
def test_ambiguous_supersedes_target_invalidates_the_used_record(
    proof_case, kind, previous
):
    used = predecessor(proof_case, kind, previous)
    report = run(proof_case, activity="implementation" if kind == "decision" else None)
    assert aggregate(report["checks"], "model") == "FAIL"
    assert {used, previous} <= invalid_record_ids(load_bundle(proof_case[0]))
    if kind == "decision":
        assert aggregate(report["checks"], "authorization") == "UNRESOLVED"
    else:
        assert rule_result(report) == "UNKNOWN"


@pytest.mark.parametrize("kind", ["check", "evidence", "binding", "decision"])
@pytest.mark.parametrize("negative", [False, True])
def test_valid_record_predecessor_preserves_positive_and_negative_claims(
    proof_case, kind, negative
):
    _, records, _ = proof_case
    predecessor(proof_case, kind, kind + ".previous")
    if negative:
        if kind == "decision":
            records["decision.x"]["outcome"] = "rejected"
        else:
            records["evidence.x"]["result"] = "FAIL"
    report = run(proof_case, activity="implementation" if kind == "decision" else None)
    assert aggregate(report["checks"], "model") == "PASS"
    assert not invalid_record_ids(load_bundle(proof_case[0]))
    if kind == "decision":
        assert aggregate(report["checks"], "authorization") == (
            "DENIED" if negative else "GRANTED"
        )
    else:
        assert rule_result(report) == ("FAIL" if negative else "PASS")


def test_invalid_approval_history_does_not_erase_an_explicit_rejection(proof_case):
    _, records, host = proof_case
    predecessor(proof_case, "decision", "entity.y")
    records["decision.x"]["outcome"] = "rejected"
    host["policy"]["requirement"] = "standing"
    report = run(proof_case, activity="implementation")
    assert aggregate(report["checks"], "model") == "FAIL"
    assert aggregate(report["checks"], "authorization") == "DENIED"


def test_invalid_used_history_preserves_an_independent_valid_failure(proof_case):
    _, records, _ = proof_case
    predecessor(proof_case, "binding", "source.input")
    records["evidence.y"]["result"] = "FAIL"
    report = run(proof_case, scope=["entity.x", "entity.y"])
    assert rule_result(report, "x") == "UNKNOWN"
    assert rule_result(report, "y") == "FAIL"
    assert aggregate(report["checks"], "implementation") == "FAIL"


@pytest.mark.parametrize("ambiguous", [False, True])
@pytest.mark.parametrize("accepted", [False, True])
def test_used_readiness_history_has_the_same_identity_boundary(
    readiness_case, ambiguous, accepted
):
    path, scope, records = readiness_case
    model = json.loads(path.read_text())
    model["nodes"]["entity.y"] = {
        "kind": "entity",
        "description": "Unrelated value",
        "fields": {},
    }
    path.write_text(json.dumps(model), encoding="utf-8")
    records["decision.review"]["spec_revision"] = load_bundle(path).revision
    previous = "entity.y" if ambiguous else "decision.previous"
    records[previous] = deepcopy(records["decision.review"])
    records["decision.review"].update(
        supersedes=previous, outcome="accepted" if accepted else "rejected"
    )
    write_records(path.parent, records)
    report = validate(
        path,
        profiles=["readiness"],
        scope=scope,
        host_context={"trusted_decisions": ["decision.review"]},
    )
    assert aggregate(report["checks"], "model") == ("FAIL" if ambiguous else "PASS")
    expected = "PASS" if accepted and not ambiguous else "UNKNOWN"
    assert aggregate(report["checks"], "readiness") == expected
    assert ("decision.review" in invalid_record_ids(load_bundle(path))) is ambiguous
