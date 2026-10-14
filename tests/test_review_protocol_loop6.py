"""Current model references must remain globally unambiguous when consumed."""

from copy import deepcopy
import json

import pytest

from gdd_validator import validate
from gdd_validator.loading import digest, load_bundle
from gdd_validator.record_integrity import invalid_record_ids
from gdd_validator.report import aggregate
from test_review_protocol_loop4 import case as proof_case, run, rule_result
from test_review_readiness_integrity import case as readiness_case


def write_records(directory, records):
    model = json.loads((directory / "model.json").read_text())
    (directory / "records.json").write_text(
        json.dumps(
            {
                "format": "gdd-records/0.2",
                "project_id": model["project"]["id"],
                "records": records,
            }
        )
    )


def source():
    return {
        "role": "input",
        "title": "Synthetic input",
        "uri": "missing.md",
        "format": "text/markdown",
        "normative": False,
    }


def collide(case, ident, registry="source"):
    path, records, _ = case
    model = json.loads(path.read_text())
    if registry == "source":
        model.setdefault("sources", {})[ident] = source()
    elif registry == "edge":
        model["edges"].append(
            {
                "id": ident,
                "from": "rule.y",
                "relation": "constrains",
                "to": "entity.y",
            }
        )
    elif registry == "transition":
        transition = model["nodes"]["machine.x"]["machine"]["transitions"][0]
        transition["id"] = ident
    elif registry == "question":
        model["questions"] = {
            ident: {
                "statement": "Synthetic unrelated question",
                "targets": ["entity.y"],
                "blocks": ["modeling"],
                "status": "open",
            }
        }
    path.write_text(json.dumps(model))
    revision = load_bundle(path).revision
    assert revision is not None
    for record in records.values():
        record["spec_revision"] = revision
    for record in records.values():
        if record["kind"] == "evidence":
            record["check_revision"] = digest(records[record["check"]])
    case[2]["policy"]["spec_revision"] = revision
    return model


@pytest.mark.parametrize("registry", ["source", "edge", "transition", "question"])
@pytest.mark.parametrize("kind", ["check", "evidence", "binding", "decision"])
def test_ambiguous_current_targets_cannot_support_positive_use(
    proof_case, registry, kind
):
    path, records, _ = proof_case
    collide(proof_case, "rule.x", registry)
    record = records[kind + ".x"]
    field = "targets" if kind == "binding" else "scope"
    if kind == "decision":
        record[field].append("rule.x")
    report = run(proof_case, activity="implementation" if kind == "decision" else None)
    assert kind + ".x" in invalid_record_ids(load_bundle(path))
    assert aggregate(report["checks"], "model") == "FAIL"
    assert any(
        d["code"] == "RECORD_INVALID"
        and "ambiguous current target rule.x" in d["message"]
        and kind + ".x" in d["related_ids"]
        for check in report["checks"]
        for d in check["diagnostics"]
    )
    if kind == "decision":
        assert aggregate(report["checks"], "authorization") == "UNRESOLVED"
    else:
        assert rule_result(report) == "UNKNOWN"


@pytest.mark.parametrize("registry", ["nodes", "sources", "questions", "extensions"])
def test_registry_ids_are_counted_even_without_a_node_at_the_collision(registry):
    from gdd_validator.record_integrity import ambiguous_model_ids

    # Identity traversal is independent of representation payloads; dict values
    # are not counters. The schema/model consumer owns the payload validation.
    model = {"edges": [{"id": "shared.id"}], registry: {"shared.id": {}}}
    assert ambiguous_model_ids(model) == {"shared.id"}


@pytest.mark.parametrize("kind", ["edges", "transitions"])
def test_duplicate_nonregistry_id_occurrences_are_detected(kind):
    from gdd_validator.record_integrity import ambiguous_model_ids

    model = {"nodes": {}, "edges": []}
    values = [{"id": "shared.id"}, {"id": "shared.id"}]
    if kind == "edges":
        model["edges"] = values
    else:
        model["nodes"] = {"machine.x": {"machine": {"transitions": values}}}
    assert ambiguous_model_ids(model) == {"shared.id"}


@pytest.mark.parametrize("request_scope", [["entity.x"], []])
@pytest.mark.parametrize("requirement", ["explicit", "standing", "none"])
def test_ambiguous_request_node_cannot_be_authorized(
    proof_case, request_scope, requirement
):
    path, records, host = proof_case
    collide(proof_case, "entity.x")
    host["policy"]["requirement"] = requirement
    records["decision.x"][
        "scope"
    ] = []  # Full project approval covers either request_scope.
    write_records(path.parent, records)
    report = validate(
        path,
        profiles=["records"],
        activity="implementation",
        scope=request_scope,
        host_context=host,
    )
    assert aggregate(report["checks"], "authorization") == "UNRESOLVED"


@pytest.mark.parametrize("request_scope", [["entity.x"], []])
@pytest.mark.parametrize("denial", ["host", "receipt"])
def test_explicit_denial_precedes_ambiguous_request(proof_case, request_scope, denial):
    path, records, host = proof_case
    collide(proof_case, "entity.x")
    host["policy"]["requirement"] = "standing"
    if denial == "host":
        host["policy"]["denied"] = True
    else:
        records["decision.x"]["outcome"] = "rejected"
    write_records(path.parent, records)
    report = validate(
        path,
        profiles=["records"],
        activity="implementation",
        scope=request_scope,
        host_context=host,
    )
    assert aggregate(report["checks"], "authorization") == "DENIED"


@pytest.mark.parametrize("requirement", ["explicit", "standing", "none"])
def test_unrelated_ambiguity_does_not_block_known_authorized_scope(
    proof_case, requirement
):
    _, _, host = proof_case
    collide(proof_case, "entity.y")
    host["policy"]["requirement"] = requirement
    report = run(proof_case, activity="implementation")
    assert aggregate(report["checks"], "authorization") == "GRANTED"


@pytest.mark.parametrize("result", ["PASS", "FAIL"])
def test_unrelated_ambiguity_preserves_known_implementation_result(proof_case, result):
    _, records, _ = proof_case
    collide(proof_case, "rule.y")
    records["evidence.x"]["result"] = result
    report = run(proof_case)
    assert rule_result(report) == result


def test_ambiguous_requested_entity_prevents_dependent_rule_pass(proof_case):
    collide(proof_case, "entity.x")
    # Records reference the unique rule, but the rule/request_scope uses ambiguous entity.x.
    report = run(proof_case)
    assert rule_result(report) == "UNKNOWN"


@pytest.mark.parametrize("ambiguous", ["rule.x", "entity.x"])
def test_ambiguity_preserves_independent_failure_in_combined_scope(
    proof_case, ambiguous
):
    _, records, _ = proof_case
    collide(proof_case, ambiguous)
    records["evidence.y"]["result"] = "FAIL"
    report = run(proof_case, scope=["entity.x", "entity.y"])
    assert rule_result(report, "x") == "UNKNOWN"
    assert rule_result(report, "y") == "FAIL"
    assert aggregate(report["checks"], "implementation") == "FAIL"


@pytest.mark.parametrize("kind", ["check", "evidence", "binding", "decision"])
def test_historical_targets_are_not_rebound_to_current_ambiguities(proof_case, kind):
    path, records, _ = proof_case
    collide(proof_case, "rule.x")
    record = records[kind + ".x"]
    record["spec_revision"] = "sha256:" + "a" * 64
    record["targets" if kind == "binding" else "scope"] = ["rule.x"]
    if kind == "evidence":
        records["check.x"]["spec_revision"] = record["spec_revision"]
        record["check_revision"] = digest(records["check.x"])
    write_records(path.parent, records)
    bundle = load_bundle(path)
    assert kind + ".x" not in invalid_record_ids(bundle)


@pytest.mark.parametrize("kind", ["check", "evidence"])
def test_ambiguous_successor_cannot_retire_valid_current_failure(proof_case, kind):
    _, records, host = proof_case
    collide(proof_case, "entity.y")
    records["evidence.x"]["result"] = "FAIL"
    replacement = {**deepcopy(records[kind + ".x"]), "supersedes": kind + ".x"}
    replacement["scope"].append("entity.y")
    records[kind + ".other"] = replacement
    if kind == "evidence":
        replacement["result"] = "PASS"
        host["trusted_evidence"].append("evidence.other")
    host["check_selection"] = {"rule.x": {"probe": "check.x"}}
    host["evidence_selection"] = {"check.x": "evidence.x"}
    assert rule_result(run(proof_case)) == "FAIL"


def test_ambiguous_successor_cannot_erase_explicit_rejection(proof_case):
    _, records, host = proof_case
    collide(proof_case, "entity.y")
    records["decision.x"]["outcome"] = "rejected"
    records["decision.other"] = {
        **deepcopy(records["decision.x"]),
        "scope": ["entity.x", "entity.y"],
        "supersedes": "decision.x",
        "outcome": "accepted",
    }
    host["trusted_decisions"].append("decision.other")
    host["policy"]["requirement"] = "standing"
    assert (
        aggregate(run(proof_case, activity="implementation")["checks"], "authorization")
        == "DENIED"
    )


def test_used_readiness_receipts_share_ambiguous_target_guard(readiness_case):
    path, scope, records = readiness_case
    model = json.loads(path.read_text())
    model["nodes"]["entity.unrelated"] = {
        "kind": "entity",
        "description": "Value",
        "fields": {},
    }
    model.setdefault("sources", {})["entity.unrelated"] = source()
    path.write_text(json.dumps(model))
    records["decision.review"].update(
        spec_revision=load_bundle(path).revision,
        scope=scope + ["entity.unrelated"],
    )
    write_records(path.parent, records)
    report = validate(
        path,
        profiles=["readiness"],
        scope=scope,
        host_context={"trusted_decisions": ["decision.review"]},
    )
    assert "decision.review" in invalid_record_ids(load_bundle(path))
    assert aggregate(report["checks"], "readiness") != "PASS"


@pytest.mark.parametrize("result", ["PASS", "FAIL"])
def test_rule_context_ambiguity_preserves_valid_failure(proof_case, result):
    _, records, _ = proof_case
    collide(proof_case, "entity.x")
    records["evidence.x"]["result"] = result
    # The plan/evidence have the unique rule.x in their scope.
    # Ambiguous context prevents positive completeness, but does not erase
    # a known independent failure reported for that rule.
    assert rule_result(run(proof_case)) == ("UNKNOWN" if result == "PASS" else "FAIL")


@pytest.mark.parametrize("model_id", ["edge.x", "transition.x", "source.input"])
@pytest.mark.parametrize("request_scope", [[], ["entity.x"]])
def test_global_authority_scope_includes_nonnode_model_identities(
    proof_case, model_id, request_scope
):
    path, records, host = proof_case
    if model_id == "source.input":
        # A source/edge collision contains no ambiguous node ID.
        collide(proof_case, model_id, "edge")
    else:
        collide(proof_case, model_id)
    host["policy"]["requirement"] = "standing"
    write_records(path.parent, records)
    report = validate(
        path,
        profiles=["records"],
        scope=request_scope,
        activity="implementation",
        host_context=host,
    )
    assert aggregate(report["checks"], "authorization") == (
        "GRANTED" if request_scope else "UNRESOLVED"
    )


@pytest.mark.parametrize("kind", ["check", "binding", "evidence", "decision"])
def test_nonnode_ambiguous_current_record_reference_is_invalid(proof_case, kind):
    path, records, _ = proof_case
    collide(proof_case, "edge.x")
    record = records[kind + ".x"]
    record["targets" if kind == "binding" else "scope"].append("edge.x")
    if kind == "check":
        records["evidence.x"]["scope"] = list(record["scope"])
        records["evidence.x"]["check_revision"] = digest(record)
    write_records(path.parent, records)
    assert kind + ".x" in invalid_record_ids(load_bundle(path))


@pytest.mark.parametrize("result", ["PASS", "FAIL"])
def test_ambiguous_extra_request_does_not_claim_complete_implementation(
    proof_case, result
):
    path, records, _ = proof_case
    model = json.loads(path.read_text())
    model["nodes"]["entity.extra"] = {
        "kind": "entity",
        "description": "Extra",
        "fields": {},
    }
    path.write_text(json.dumps(model))
    collide(proof_case, "entity.extra")
    records["evidence.x"]["result"] = result
    report = run(proof_case, scope=["entity.x", "entity.extra"])
    assert rule_result(report) == result
    assert aggregate(report["checks"], "implementation") == (
        "UNKNOWN" if result == "PASS" else "FAIL"
    )
    assert any(
        c["check_id"] == "implementation.scope" and c["result"] == "UNKNOWN"
        for c in report["checks"]
    )


def test_global_implementation_request_includes_nonnode_identities(proof_case):
    path, records, host = proof_case
    collide(proof_case, "edge.x")
    write_records(path.parent, records)
    report = validate(path, profiles=["records"], scope=[], host_context=host)
    assert rule_result(report, "x") == "PASS"
    assert rule_result(report, "y") == "PASS"
    assert aggregate(report["checks"], "implementation") == "UNKNOWN"
