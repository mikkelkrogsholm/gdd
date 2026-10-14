"""GDD 0.3 boundaries and advisory review; test inputs claim no app execution."""

from copy import deepcopy
import json
from pathlib import Path
import subprocess
import sys

from jsonschema import Draft202012Validator
import pytest

from gdd_validator import validate
from gdd_validator.boundaries import PRINCIPLES, boundary_view
from gdd_validator.design_review import review_design
from gdd_validator.loading import load_bundle, parse, schema
from gdd_validator.readiness import context_closure
from gdd_validator.report import aggregate
from gdd_validator.visualization import render_view, verify_view
from gdd_validator.workflow import prepare_workflow, verify_workflow

ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture
def model(tmp_path):
    data = parse((ROOT / "examples/concert-0.3.gdd.yaml").read_bytes())
    data.pop("record_files")
    path = tmp_path / "model.gdd.json"
    write(path, data)
    return path


def write(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")


def edit(path, operation):
    data = parse(path.read_bytes())
    operation(data)
    write(path, data)


def result(path, dimension="model", scope=None):
    report = validate(path, profiles=["readiness" if dimension == "readiness" else "model"], scope=scope or [])
    return aggregate(report["checks"], dimension)


def diagnostic_codes(path, **options):
    return {d["code"] for c in validate(path, **options)["checks"] for d in c["diagnostics"]}


def review_for(path):
    return {"format": "gdd-design-review/0.3", "spec_revision": load_bundle(path).revision,
            "implementation": "implementation.synthetic", "code_revision": "synthetic-code",
            "scope": ["action.reserve"], "reviewer": {"name": "synthetic-reviewer", "version": "1"},
            "executed": True, "assessments": [
                {"principle": p, "status": "reviewed", "rationale": "Synthetic assessment for envelope testing only.", "findings": []}
                for p in PRINCIPLES]}


def review_status(path, review, **options):
    return review_design(load_bundle(path), review, implementation="implementation.synthetic",
                         code_revision="synthetic-code", **options)


def workplan(path):
    request = {"format": "gdd-work-request/0.1", "id": "workflow.synthetic",
               "objective": "Exercise revision-bound packets; no code execution.",
               "implementation": "implementation.synthetic", "base_code_revision": "synthetic-code",
               "packages": [{"id": "package.reserve", "scope": ["action.reserve"],
                             "read_resources": ["model.gdd.json"], "write_resources": ["src/reservations.py"],
                             "workspace": "workspaces/reservations", "deliverables": ["src/reservations.py"],
                             "integration_plan": "Run actual reservation integration checks before acceptance."}],
               "serialized": [], "integration": {"owner": "actor.synthetic", "checks": []}}
    return prepare_workflow(path, request)["artifact"]


def test_complete_boundary_readiness(model):
    assert result(model) == "PASS"
    assert result(model, "readiness", ["action.reserve"]) == "PASS"


@pytest.mark.parametrize("field", ["responsibility", "rationale", "excludes"])
def test_partial_boundary_sketch_and_readiness(model, field):
    edit(model, lambda m: m["nodes"]["component.reservations"]["boundary"].pop(field))
    assert result(model) == "PASS"
    assert result(model, "readiness", ["action.reserve"]) == "FAIL"
    assert "BOUNDARY_RATIONALE_MISSING" in diagnostic_codes(model, profiles=["readiness"], scope=["action.reserve"])


@pytest.mark.parametrize("field", ["responsibility", "rationale"])
def test_blank_boundary_is_invalid(model, field):
    edit(model, lambda m: m["nodes"]["component.reservations"]["boundary"].update({field: " \n\t"}))
    assert result(model) == "FAIL"


def test_explicit_empty_exclusions(model):
    edit(model, lambda m: m["nodes"]["component.reservations"]["boundary"].update(excludes=[]))
    assert result(model, "readiness", ["action.reserve"]) == "PASS"


def test_unrelated_incomplete_component(model):
    edit(model, lambda m: m["nodes"].update({"component.island": {"kind": "component", "description": "Unrelated sketch."}}))
    assert result(model, "readiness", ["action.reserve"]) == "PASS"
    assert result(model, "readiness", ["component.island"]) == "FAIL"


def test_legacy_semantics_preserved():
    path = ROOT / "examples/concert.gdd.yaml"
    assert result(path, "readiness", ["action.reserve"]) == "PASS"
    old = parse(path.read_bytes())
    assert "boundary" not in old["nodes"]["component.api"]
    assert "action.reserve" in old["nodes"]["component.api"]["interfaces"]


def test_new_fields_cannot_silently_enter_legacy_format(model):
    edit(model, lambda m: m.update(format="gdd/0.2"))
    assert result(model) == "FAIL"


def test_fragment_version_must_match(model):
    fragment = model.parent / "fragment.json"
    write(fragment, {"format": "gdd-fragment/0.2", "nodes": {}, "edges": []})
    edit(model, lambda m: m.update(includes=[fragment.name]))
    assert result(model) == "FAIL"
    data = parse(fragment.read_bytes()); data["format"] = "gdd-fragment/0.3"; write(fragment, data)
    assert result(model) == "PASS"


def test_public_interface_requires_matching_owner(model):
    edit(model, lambda m: m["nodes"]["component.api"].update(interfaces=["action.reserve"]))
    assert result(model) == "FAIL"
    assert "INTERFACE_OWNER_MISMATCH" in diagnostic_codes(model)


def test_cross_component_private_call(model):
    edit(model, lambda m: m["nodes"]["component.reservations"].update(interfaces=[]))
    assert result(model) == "FAIL"
    assert "PRIVATE_ACTION_CALL" in diagnostic_codes(model)


def test_internal_private_call_allowed(model):
    def mutate(m):
        m["nodes"]["component.reservations"]["interfaces"] = []
        m["edges"] = [e for e in m["edges"] if e["relation"] != "calls"]
        m["edges"].append({"id": "edge.internal", "from": "action.cancel", "relation": "calls", "to": "action.reserve"})
    edit(model, mutate)
    assert result(model) == "PASS"
    assert boundary_view(load_bundle(model))["calls"][0]["status"] == "internal"


@pytest.mark.parametrize("missing", ["provider_owner", "consumer_owner", "interfaces"])
def test_unresolved_call_valid_sketch_unready(model, missing):
    def mutate(m):
        if missing == "interfaces": m["nodes"]["component.reservations"].pop("interfaces")
        else:
            target = "action.reserve" if missing == "provider_owner" else "screen.reserve"
            m["edges"] = [e for e in m["edges"] if not (e["from"] == target and e["relation"] == "owned_by")]
    edit(model, mutate)
    assert result(model) == "PASS"
    scope = ["screen.reserve"] if missing == "consumer_owner" else ["action.reserve"]
    assert result(model, "readiness", scope) == "FAIL"
    assert "PUBLIC_CONTRACT_UNRESOLVED" in diagnostic_codes(model, profiles=["readiness"], scope=scope)


def test_postconditions_required_in_read_context(model):
    edit(model, lambda m: m["nodes"]["action.cancel"]["contract"].pop("postconditions"))
    assert result(model) == "PASS"
    assert result(model, "readiness", ["action.reserve"]) == "FAIL"


def test_unrelated_private_call_does_not_block_readiness(model):
    def mutate(m):
        m["nodes"].update({"component.island": {"kind": "component", "description": "Isolated caller."},
                           "component.private": {"kind": "component", "description": "Isolated provider.", "interfaces": []},
                           "action.private": {"kind": "action", "description": "Isolated private action."}})
        m["edges"].extend([{"id": "edge.private.owner", "from": "action.private", "relation": "owned_by", "to": "component.private"},
                           {"id": "edge.island.call", "from": "component.island", "relation": "calls", "to": "action.private"}])
    edit(model, mutate)
    report = validate(model, profiles=["readiness"], scope=["action.reserve"])
    assert aggregate(report["checks"], "model") == "FAIL"
    assert aggregate(report["checks"], "readiness") == "PASS"


def test_community_overlap_and_owner_derived_membership(model):
    view = boundary_view(load_bundle(model))
    backend = view["communities"]["community.backend"]
    feature = view["communities"]["community.reservation_feature"]
    assert "edge.screen.reserve.calls" in backend["calls"]["inbound"]
    assert "edge.screen.reserve.calls" in feature["calls"]["internal"]
    assert feature["membership_basis"]["component.reservations"] == ["action.reserve"]
    assert all(c["provider"] == "component.reservations" for c in feature["public_contracts"])
    Draft202012Validator(schema("boundaries")).validate(view)


def test_community_membership_does_not_grant_access(model):
    edit(model, lambda m: m["nodes"]["component.reservations"].update(interfaces=[]))
    assert result(model) == "FAIL"


def test_scoped_boundary_view_declares_omitted_calls(model):
    bundle = load_bundle(model)
    view = boundary_view(bundle, sorted(context_closure(bundle, ["action.reserve"])))
    assert "edge.screen.reserve.calls" in view["omitted_calls"]
    assert "edge.screen.reserve.calls" in view["communities"]["community.backend"]["omitted_calls"]
    assert "component.frontend" not in view["components"]


def test_unrelated_unresolved_call_not_in_every_community(model):
    def mutate(m):
        m["nodes"].update({"component.island": {"kind": "component", "description": "Unrelated caller.", "interfaces": []},
                           "action.island": {"kind": "action", "description": "Unrelated ownerless action."}})
        m["edges"].append({"id": "edge.island", "from": "component.island", "relation": "calls", "to": "action.island"})
    edit(model, mutate)
    view = boundary_view(load_bundle(model))
    assert "edge.island" in view["unresolved"]
    assert all("edge.island" not in c["calls"]["unresolved"] for c in view["communities"].values())


def test_boundary_and_contract_changes_change_revision(model):
    before = load_bundle(model).revision
    edit(model, lambda m: m["nodes"]["component.reservations"]["boundary"].update(rationale="A newly accepted boundary reason."))
    after = load_bundle(model).revision
    assert before != after
    edit(model, lambda m: m["nodes"]["action.reserve"]["contract"]["postconditions"].append("An additional accepted obligation."))
    assert load_bundle(model).revision != after


@pytest.mark.parametrize("rationale_present", [True, False])
def test_refinement_preserves_parent_boundary_context(model, rationale_present):
    def mutate(m):
        m["nodes"]["component.parent"] = {
            "kind": "component", "description": "Reservation feature boundary.", "interfaces": [],
            "boundary": {"responsibility": "Preserve the reservation feature obligations.",
                         "excludes": [], "rationale": "Keep the feature's public obligations visible during decomposition."}}
        if not rationale_present:
            m["nodes"]["component.parent"]["boundary"].pop("rationale")
        m["edges"].append({"id": "edge.parent", "from": "component.reservations", "relation": "refines", "to": "component.parent"})
    edit(model, mutate)
    assert "component.parent" in context_closure(load_bundle(model), ["action.reserve"])
    assert result(model, "readiness", ["action.reserve"]) == ("PASS" if rationale_present else "FAIL")


def test_packet_contains_canonical_boundaries_and_review_topics(model):
    plan = workplan(model); packet = plan["packages"][0]
    assert packet["model_slice"]["nodes"]["action.reserve"]["contract"]["postconditions"]
    assert packet["boundaries"]["components"]["component.reservations"]["boundary"]["rationale"]
    assert {p["principle"] for p in packet["design_review"]} == set(PRINCIPLES)
    assert "component.reservations" in packet["locked_contracts"]


@pytest.mark.parametrize("field", ["boundaries", "design_review"])
def test_packet_omission_rejected(model, field):
    plan = workplan(model); plan["packages"][0].pop(field)
    report = verify_workflow(model, plan)
    check = next(c for c in report["checks"] if c["check_id"] == "workflow.packet_integrity")
    assert check["result"] == "FAIL"


def test_boundary_change_invalidates_plan(model):
    plan = workplan(model)
    edit(model, lambda m: m["nodes"]["component.reservations"]["boundary"].update(rationale="A changed accepted reason."))
    assert any(c["result"] != "PASS" for c in verify_workflow(model, plan)["checks"])


@pytest.mark.parametrize("category,blocking", [("design", True), ("architecture", False)])
def test_broad_principle_is_advisory(model, category, blocking):
    edit(model, lambda m: m["nodes"]["rule.design.kiss"]["rule"].update(category=category, blocking=blocking))
    assert "DESIGN_PRINCIPLE_INVALID" in diagnostic_codes(model)


def test_precise_design_rule_can_still_block(model):
    def mutate(m):
        rule = m["nodes"]["rule.design.kiss"]["rule"]
        rule.pop("principle"); rule["blocking"] = True
    edit(model, mutate)
    assert result(model) == "PASS"


def test_advisory_review_complete_even_with_findings(model):
    review = review_for(model)
    review["assessments"][0]["findings"] = [{"targets": ["action.reserve"],
          "code_locations": [{"resource": "src/reservations.py", "selector": "reserve"}],
          "consequence": "An unused layer adds navigation cost.", "recommendation": "Consider a direct function."}]
    status = review_status(model, review)
    assert status["status"] == "COMPLETE"
    assert status["findings"] and "result" not in status
    assert status["scope"] == ["action.reserve"]
    assert status["review_identity"]["code_revision"] == "synthetic-code"


@pytest.mark.parametrize("condition", ["absent", "planned", "missing_principle", "unknown", "stale_model", "stale_code", "unsupported", "narrow_scope"])
def test_review_unknown_is_explicit(model, condition):
    review = review_for(model); options = {}
    if condition == "absent": review = None
    elif condition == "planned": review["executed"] = False
    elif condition == "missing_principle": review["assessments"].pop()
    elif condition == "unknown": review["assessments"][0]["status"] = "unknown"
    elif condition == "stale_model": review["spec_revision"] = "sha256:" + "0" * 64
    elif condition == "stale_code": review["code_revision"] = "synthetic-old"
    elif condition == "unsupported": review["format"] = "gdd-design-review/9.0"
    elif condition == "narrow_scope": options["expected_scope"] = ["action.reserve", "action.cancel"]
    status = review_status(model, review, **options)
    assert status["status"] == "UNKNOWN" and status["reasons"]


@pytest.mark.parametrize("condition", ["duplicate", "unknown_scope", "unknown_target", "outside_context", "missing_location", "not_applicable_findings"])
def test_invalid_review_cannot_be_complete(model, condition):
    review = review_for(model)
    finding = {"targets": ["action.reserve"], "code_locations": [{"resource": "src/reservations.py", "selector": "reserve"}],
               "consequence": "Synthetic consequence.", "recommendation": "Synthetic recommendation."}
    if condition == "duplicate": review["assessments"][1]["principle"] = "kiss"
    elif condition == "unknown_scope": review["scope"] = ["action.unknown"]
    else:
        review["assessments"][0]["findings"] = [finding]
        if condition == "unknown_target": finding["targets"] = ["action.unknown"]
        elif condition == "outside_context": finding["targets"] = ["screen.reserve"]
        elif condition == "missing_location": finding["code_locations"] = []
        elif condition == "not_applicable_findings": review["assessments"][0]["status"] = "not_applicable"
    assert review_status(model, review)["status"] == "INVALID"


def test_invalid_model_cannot_supply_complete_review(model):
    review = review_for(model)
    edit(model, lambda m: m["nodes"]["component.reservations"].update(interfaces=[]))
    assert review_status(model, review)["status"] == "UNKNOWN"


def test_complete_review_does_not_replace_blocking_evidence(model):
    assert review_status(model, review_for(model))["status"] == "COMPLETE"
    report = validate(model, profiles=["records"], scope=["action.reserve"])
    assert aggregate(report["checks"], "implementation") == "UNKNOWN"


def test_new_viewer_and_cli_operations(model, tmp_path):
    html = tmp_path / "graph.html"
    assert aggregate(render_view(model, html)["checks"], "integration") == "PASS"
    assert aggregate(verify_view(model, html)["checks"], "integration") == "PASS"
    assert "Component responsibility and boundary rationale" in html.read_text()
    assert "Derived public boundary" in html.read_text()
    output = subprocess.run([sys.executable, "-m", "gdd_validator", "boundaries", str(model)], capture_output=True, text=True)
    assert output.returncode == 0
    Draft202012Validator(schema("boundaries")).validate(json.loads(output.stdout))
    review = tmp_path / "review.json"; write(review, review_for(model))
    output = subprocess.run([sys.executable, "-m", "gdd_validator", "design-review", str(model), "--review", str(review),
                             "--implementation", "implementation.synthetic", "--code-revision", "synthetic-code", "--scope", "action.reserve"], capture_output=True, text=True)
    assert output.returncode == 0 and json.loads(output.stdout)["status"] == "COMPLETE"


def test_versioned_schema_definitions_align():
    assert schema("gdd")["$defs"] == schema("fragment")["$defs"]
    assert schema("gdd-0.2")["$defs"] == schema("fragment-0.2")["$defs"]
    assert (ROOT / "schemas/gdd-0.2.schema.json").read_bytes() == (ROOT / "archive/gdd-0.2/schemas/gdd.schema.json").read_bytes()
