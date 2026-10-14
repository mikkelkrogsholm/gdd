"""Current decision retirement precedes request-specific approval coverage."""

from copy import deepcopy
from pathlib import Path
import shutil

import pytest
import yaml

from gdd_validator.decision import compatible_decision_supersession
from gdd_validator.loading import load_bundle
from gdd_validator.records import authorize
from gdd_validator.report import aggregate, exit_code
from gdd_validator.validator import validate

ROOT = Path(__file__).resolve().parents[1]
MODEL = ROOT / "examples/concert.gdd.yaml"


def fixture(requirement="standing", activity="implementation"):
    bundle = load_bundle(MODEL)
    receipt = {
        "kind": "decision",
        "topic": "approval",
        "activity": activity,
        "spec_revision": bundle.revision,
        "policy_revision": "policy.loop2",
        "scope": ["action.reserve"],
        "actor": {"id": "actor.loop2", "kind": "human"},
        "outcome": "rejected",
        "rationale": "Synthetic decision retirement regression",
    }
    host = {
        "policy": {
            "revision": "policy.loop2",
            "actor": "actor.loop2",
            "activity": activity,
            "spec_revision": bundle.revision,
            "scope": [],
            "requirement": requirement,
            "authorities": ["actor.loop2"],
            "basis": "Synthetic broad loop2 mandate",
        },
        "trusted_decisions": ["decision.reject", "decision.accept"],
    }
    if activity == "acceptance":
        host.update(implementation="implementation.loop2", code_revision="code.loop2")
        receipt.update(
            implementation=host["implementation"], code_revision=host["code_revision"]
        )
    bundle.records["decision.reject"] = receipt
    bundle.records["decision.accept"] = {
        **deepcopy(receipt),
        "outcome": "accepted",
        "supersedes": "decision.reject",
    }
    return bundle, host


def result(bundle, host, scope):
    authorize(bundle, host["policy"]["activity"], scope, host)
    return aggregate(bundle.checks, "authorization")


@pytest.mark.parametrize("requirement", ["standing", "none"])
@pytest.mark.parametrize(
    "scope", [["action.reserve"], ["action.reserve", "action.cancel"], []]
)
def test_narrow_terminal_approval_retires_rejection_under_broad_mandate(
    requirement, scope
):
    bundle, host = fixture(requirement)
    assert result(bundle, host, scope) == "GRANTED"
    assert bundle.checks[-1]["authority_basis"] == host["policy"]["basis"]
    assert bundle.checks[-1]["scope"] == scope


@pytest.mark.parametrize(
    "scope,expected",
    [
        (["action.reserve"], "GRANTED"),
        (["action.reserve", "action.cancel"], "UNRESOLVED"),
        ([], "UNRESOLVED"),
    ],
)
def test_narrow_approval_retires_rejection_without_authorizing_broader_work(
    scope, expected
):
    bundle, host = fixture("explicit")
    assert result(bundle, host, scope) == expected


@pytest.mark.parametrize("scope", [["action.reserve", "action.cancel"], []])
def test_explicit_broad_approval_remains_usable_after_narrow_receipt_retirement(scope):
    bundle, host = fixture("explicit")
    bundle.records["decision.broad"] = {
        **deepcopy(bundle.records["decision.accept"]),
        "scope": [],
    }
    bundle.records["decision.broad"].pop("supersedes")
    host["trusted_decisions"].append("decision.broad")
    assert result(bundle, host, scope) == "GRANTED"
    assert bundle.checks[-1]["authority_basis"] == "decision.broad"


@pytest.mark.parametrize(
    "variant",
    [
        "untrusted",
        "historical",
        "other_policy",
        "other_activity",
        "other_topic",
        "other_actor",
        "disjoint_scope",
        "insufficient_scope",
        "global_rejection",
        "proposed",
    ],
)
@pytest.mark.parametrize("scope", [["action.reserve", "action.cancel"], []])
def test_incompatible_narrow_successor_cannot_hide_overlapping_rejection(
    variant, scope
):
    bundle, host = fixture()
    replacement = bundle.records["decision.accept"]
    previous = bundle.records["decision.reject"]
    if variant == "untrusted":
        host["trusted_decisions"].remove("decision.accept")
    elif variant == "historical":
        replacement["spec_revision"] = "sha256:" + "0" * 64
    elif variant == "other_policy":
        replacement["policy_revision"] = "policy.other"
    elif variant == "other_activity":
        replacement["activity"] = "model_changes"
    elif variant == "other_topic":
        replacement["topic"] = "readiness_review"
    elif variant == "other_actor":
        replacement["actor"]["id"] = "actor.untrusted"
    elif variant == "disjoint_scope":
        replacement["scope"] = ["action.cancel"]
    elif variant == "insufficient_scope":
        previous["scope"] = ["action.reserve", "action.cancel"]
    elif variant == "global_rejection":
        previous["scope"] = []
    else:
        replacement["outcome"] = "proposed"
    assert result(bundle, host, scope) == "DENIED"


@pytest.mark.parametrize("field", ["implementation", "code_revision"])
def test_other_acceptance_result_cannot_retire_current_rejection(field):
    bundle, host = fixture(activity="acceptance")
    bundle.records["decision.accept"][field] = "other.result"
    assert result(bundle, host, []) == "DENIED"


def test_current_rejection_after_narrow_approval_still_denies_broad_request():
    bundle, host = fixture()
    bundle.records["decision.reject_again"] = {
        **deepcopy(bundle.records["decision.reject"]),
        "supersedes": "decision.accept",
    }
    host["trusted_decisions"].append("decision.reject_again")
    assert result(bundle, host, []) == "DENIED"


def test_global_successor_can_retire_global_rejection():
    bundle, host = fixture("explicit")
    bundle.records["decision.reject"]["scope"] = []
    bundle.records["decision.accept"]["scope"] = []
    assert result(bundle, host, []) == "GRANTED"


@pytest.mark.parametrize(
    "field,value",
    [
        ("kind", "binding"),
        ("outcome", "proposed"),
        ("topic", "readiness_review"),
        ("spec_revision", "sha256:" + "0" * 64),
        ("policy_revision", "policy.other"),
        ("activity", "acceptance"),
        ("implementation", "implementation.other"),
        ("code_revision", "code.other"),
    ],
)
def test_shared_decision_helper_requires_same_terminal_context(field, value):
    bundle, _ = fixture()
    previous = bundle.records["decision.reject"]
    replacement = deepcopy(bundle.records["decision.accept"])
    assert compatible_decision_supersession(replacement, previous)
    replacement[field] = value
    assert not compatible_decision_supersession(replacement, previous)


@pytest.mark.parametrize(
    "old_scope,new_scope,expected",
    [
        (["action.reserve"], ["action.reserve"], True),
        (["action.reserve"], ["action.reserve", "action.cancel"], True),
        (["action.reserve"], [], True),
        ([], [], True),
        ([], ["action.reserve"], False),
        (["action.reserve", "action.cancel"], ["action.reserve"], False),
        (["action.reserve"], ["action.cancel"], False),
    ],
)
def test_shared_decision_helper_covers_prior_scope(old_scope, new_scope, expected):
    bundle, _ = fixture()
    previous = bundle.records["decision.reject"]
    replacement = bundle.records["decision.accept"]
    previous["scope"], replacement["scope"] = old_scope, new_scope
    assert compatible_decision_supersession(replacement, previous) is expected


@pytest.mark.parametrize(
    "scope", [["action.reserve"], ["action.reserve", "action.cancel"], []]
)
def test_public_api_retires_narrow_receipt_before_broad_authorization(tmp_path, scope):
    directory = tmp_path / "examples"
    shutil.copytree(ROOT / "examples", directory)
    model = directory / "concert.gdd.yaml"
    bundle, host = fixture()
    records = {
        "format": "gdd-records/0.2",
        "project_id": bundle.model["project"]["id"],
        "records": bundle.records,
    }
    (directory / "concert.records.yaml").write_text(
        yaml.safe_dump(records, sort_keys=False), encoding="utf-8"
    )
    report = validate(
        model,
        profiles=["records"],
        scope=scope,
        activity="implementation",
        host_context=host,
    )
    assert aggregate(report["checks"], "authorization") == "GRANTED"
    assert aggregate(report["checks"], "model") == "PASS"
    assert report["scope"] == scope
    assert exit_code(report) == 0
