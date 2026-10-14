"""Known ledger hash violations are reported even outside the selected check."""

import pytest

from gdd_validator import validate
from gdd_validator.loading import digest
from gdd_validator.report import aggregate, exit_code
from test_review_protocol_loop4 import case as proof_case, run, rule_result, write


@pytest.mark.parametrize("profiles", [["model"], ["readiness"], ["records"]])
@pytest.mark.parametrize("matching", [True, False])
@pytest.mark.parametrize("trusted", [True, False])
def test_unrelated_evidence_hash_is_checked_across_profiles(
    proof_case, profiles, matching, trusted
):
    path, records, host = proof_case
    if not matching:
        records["evidence.y"]["check_revision"] = "sha256:" + "a" * 64
    if not trusted:
        host["trusted_evidence"].remove("evidence.y")
    write(path, records)
    report = validate(path, profiles=profiles, scope=["entity.x"], host_context=host)
    assert aggregate(report["checks"], "model") == ("PASS" if matching else "FAIL")
    mismatches = [
        d
        for check in report["checks"]
        for d in check["diagnostics"]
        if d["code"] == "CHECK_REVISION_MISMATCH"
    ]
    assert bool(mismatches) is not matching
    if mismatches:
        assert any(d["related_ids"] == ["evidence.y"] for d in mismatches)
    if profiles == ["records"]:
        assert rule_result(report) == "PASS"
        assert exit_code(report) == (0 if matching else 1)


@pytest.mark.parametrize("independent_result", ["PASS", "FAIL"])
@pytest.mark.parametrize("evidence_result", ["PASS", "FAIL", "UNKNOWN"])
def test_unrelated_hash_violation_preserves_independent_implementation_result(
    proof_case, independent_result, evidence_result
):
    _, records, _ = proof_case
    records["evidence.x"]["result"] = independent_result
    records["evidence.y"].update(
        check_revision="sha256:" + "a" * 64, result=evidence_result
    )
    report = run(proof_case)
    assert aggregate(report["checks"], "model") == "FAIL"
    assert rule_result(report) == independent_result
    assert exit_code(report) == 1


@pytest.mark.parametrize("requirement", ["explicit", "standing", "none"])
@pytest.mark.parametrize("denied", [True, False])
def test_unrelated_hash_violation_preserves_applicable_authorization(
    proof_case, requirement, denied
):
    _, records, host = proof_case
    records["evidence.y"]["check_revision"] = "sha256:" + "a" * 64
    host["policy"]["requirement"] = requirement
    if denied:
        records["decision.x"]["outcome"] = "rejected"
    report = run(proof_case, activity="implementation")
    assert aggregate(report["checks"], "model") == "FAIL"
    assert aggregate(report["checks"], "authorization") == (
        "DENIED" if denied else "GRANTED"
    )


@pytest.mark.parametrize("matching", [True, False])
def test_representation_profile_does_not_claim_semantic_hash_validation(
    proof_case, matching
):
    path, records, host = proof_case
    if not matching:
        records["evidence.y"]["check_revision"] = "sha256:" + "a" * 64
    write(path, records)
    report = validate(path, profiles=["representation"], host_context=host)
    assert aggregate(report["checks"], "model") == "PASS"
    assert not any(
        d["code"] == "CHECK_REVISION_MISMATCH"
        for check in report["checks"]
        for d in check["diagnostics"]
    )


@pytest.mark.parametrize("target", ["check.missing", "binding.y"])
def test_missing_or_wrong_kind_check_is_diagnosed_without_digest_comparison(
    proof_case, target
):
    path, records, host = proof_case
    records["evidence.y"].update(check=target, check_revision="sha256:" + "a" * 64)
    write(path, records)
    report = validate(path, profiles=["model"], host_context=host)
    diagnostics = [d for check in report["checks"] for d in check["diagnostics"]]
    assert aggregate(report["checks"], "model") == "FAIL"
    assert any(
        d["code"] == "RECORD_INVALID" and "evidence.y" in d["related_ids"]
        for d in diagnostics
    )
    assert not any(d["code"] == "CHECK_REVISION_MISMATCH" for d in diagnostics)


@pytest.mark.parametrize("matching", [True, False])
def test_historical_evidence_digest_remains_locally_decidable(proof_case, matching):
    path, records, host = proof_case
    old = "sha256:" + "b" * 64
    records["check.y"].update(spec_revision=old, scope=["rule.deleted"])
    records["evidence.y"].update(
        spec_revision=old,
        scope=["rule.deleted"],
        check_revision=digest(records["check.y"]) if matching else "sha256:" + "a" * 64,
    )
    write(path, records)
    report = validate(path, profiles=["model"], host_context=host)
    assert aggregate(report["checks"], "model") == ("UNKNOWN" if matching else "FAIL")
    # Historical targets are not rebound, but the referenced immutable check
    # record is present and its exact digest needs no historical model snapshot.
    assert not any(
        d["code"] == "RECORD_INVALID" and "rule.deleted" in d["message"]
        for check in report["checks"]
        for d in check["diagnostics"]
    )
