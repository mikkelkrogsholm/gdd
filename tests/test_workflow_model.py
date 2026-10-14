"""Revision-bound packet and graph scope regressions; no executed application claims."""

from copy import deepcopy
from hashlib import sha256
import json
import os
from pathlib import Path
import shutil

import pytest

from gdd_validator import validate
from gdd_validator.loading import canonical, load_bundle, parse
from gdd_validator.readiness import context_closure
from gdd_validator.report import aggregate

ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture
def api():
    # Lazy import permits fixture preflight before the implementation exists.
    from gdd_validator.workflow import prepare_workflow, verify_workflow

    return prepare_workflow, verify_workflow


# Adapt only these helpers if the published artifact names differ.
def packet(plan, ident="package.reserve"):
    return next(p for p in plan["packages"] if p["id"] == ident)


def context(work):
    return work["context"]


def implementation_targets(work):
    return work["implementation_targets"]


def serialization(plan):
    return plan["request"]["serialized"]


def artifact_result(report):
    checks = [
        c for c in report["checks"] if c["check_id"] == "workflow.packet_integrity"
    ]
    assert (
        len(checks) == 1
    ), "Expose one stable artifact-integrity check independently of execution/authority"
    assert checks[0]["dimension"] == "readiness"
    return checks[0]["result"]


def objects(value):
    pending = [value]
    while pending:
        item = pending.pop()
        if isinstance(item, dict):
            yield item
            pending.extend(item.values())
        elif isinstance(item, list):
            pending.extend(item)


def node_payloads(work, ident):
    return [
        item[ident]
        for item in objects(work)
        if isinstance(item.get(ident), dict) and "kind" in item[ident]
    ]


def write(path, value):
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


@pytest.fixture
def concert(tmp_path):
    directory = tmp_path / "bundle"
    shutil.copytree(ROOT / "examples", directory)
    path = directory / "concert.gdd.yaml"
    report = validate(path, profiles=["readiness"], scope=["action.reserve"])
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == "PASS"
    return path


def rebase_planned_records(path):
    """Keep test mutations isolated from unrelated historical-record UNKNOWN."""
    bundle = load_bundle(path)
    assert bundle.revision is not None
    records_path = path.parent / "concert.records.yaml"
    data = parse(records_path.read_bytes())
    for record in data["records"].values():
        assert record["kind"] in {
            "check",
            "binding",
        }, "Never fabricate/rebase executed evidence"
        record["spec_revision"] = bundle.revision
    write(records_path, data)


def rewrite_model(path, change):
    model = parse(path.read_bytes())
    change(model)
    write(path, model)
    rebase_planned_records(path)


def host(path):
    return {
        "implementation": "implementation.synthetic",
        "code_revision": "code.synthetic.base",
        "policy": {
            "revision": "policy.synthetic",
            "actor": "actor.synthetic",
            "activity": "implementation",
            "spec_revision": load_bundle(path).revision,
            "scope": [],
            "requirement": "standing",
            "authorities": ["actor.synthetic"],
            "basis": "Synthetic fixture mandate; no external authority or execution claim",
        },
    }


def request(path, scopes=("action.reserve",)):
    bundle = load_bundle(path)
    packages = []
    for scope in scopes:
        name = scope.rsplit(".", 1)[-1] if scope.startswith("action.") else scope
        workspace = path.parent / "workspaces" / name
        workspace.mkdir(parents=True, exist_ok=True)
        packages.append(
            {
                "id": "package." + name,
                "scope": [scope],
                "write_resources": ["src/" + name, "tests/" + name],
                "read_resources": ["src/contracts"],
                "workspace": str(workspace),
                "deliverables": ["Implement the scoped contract and its checks"],
                "integration_plan": "Integrate against the selected shared-rule checkplans",
            }
        )
    return {
        "format": "gdd-work-request/0.1",
        "id": "workflow.synthetic",
        "objective": "Implement the selected concert contracts",
        "implementation": "implementation.synthetic",
        "base_code_revision": "code.synthetic.base",
        "packages": packages,
        "serialized": [],
        "integration": {
            "owner": "actor.synthetic",
            "checks": sorted(
                i for i, r in bundle.records.items() if r["kind"] == "check"
            ),
        },
    }


def parallel_projection(path, work_request):
    """Independent existing-validator oracle for the request's declarations."""
    bundle = load_bundle(path)
    packages = []
    for declared in work_request["packages"]:
        closure = context_closure(bundle, declared["scope"])
        packages.append(
            {
                **{
                    k: declared[k]
                    for k in (
                        "id",
                        "scope",
                        "write_resources",
                        "read_resources",
                        "workspace",
                        "integration_plan",
                    )
                },
                "base_revision": bundle.revision,
                "locked_contracts": sorted(
                    n
                    for n in closure
                    if bundle.model["nodes"][n]["kind"] in {"action", "screen"}
                ),
            }
        )
    return {
        "format": "gdd-parallel-plan/0.2",
        "packages": packages,
        "serialized": deepcopy(work_request["serialized"]),
        "integration": deepcopy(work_request["integration"]),
    }


def prepare(api, path, work_request=None):
    value = api[0](path, work_request or request(path), host_context=host(path))
    assert value["format"] == "gdd-workflow-result/0.1"
    return value


def current_plan(api, path, work_request=None):
    result = prepare(api, path, work_request)
    assert aggregate(result["report"]["checks"], "model") == "PASS"
    assert aggregate(result["report"]["checks"], "readiness") == "PASS"
    assert result["artifact"] is not None
    assert (
        artifact_result(api[1](path, result["artifact"], host_context=host(path)))
        == "PASS"
    )
    return result["artifact"]


def assert_current_backend_ready(path):
    report = validate(path, profiles=["readiness"], scope=["action.reserve"])
    assert aggregate(report["checks"], "model") == "PASS"
    assert aggregate(report["checks"], "readiness") == "PASS"


def test_scope_is_not_closure_and_complete_contract_payloads_are_preserved(
    api, concert
):
    plan = current_plan(api, concert)
    work = packet(plan)
    bundle = load_bundle(concert)
    closure = context_closure(bundle, ["action.reserve"])
    assert work["scope"] == ["action.reserve"]
    assert set(implementation_targets(work)) == {"action.reserve"}
    assert "action.cancel" in closure
    assert set(context(work)["nodes"]) == closure
    for ident in closure:
        values = node_payloads(work, ident)
        assert values and all(v == bundle.model["nodes"][ident] for v in values)
    payload_edges = [
        o for o in objects(work) if {"id", "from", "to", "relation"} <= o.keys()
    ]
    for edge in bundle.model["edges"]:
        if edge["from"] in closure and edge["to"] in closure:
            assert edge in payload_edges


def test_preparation_is_deterministic_and_never_rewrites_inputs(api, concert):
    declared = request(concert)
    before_request = deepcopy(declared)
    before_files = {
        p: p.read_bytes() for p in (concert, concert.parent / "concert.records.yaml")
    }
    first = prepare(api, concert, declared)
    second = prepare(api, concert, declared)
    assert canonical(first["artifact"]) == canonical(second["artifact"])
    assert declared == before_request
    assert all(p.read_bytes() == content for p, content in before_files.items())
    assert not any(
        c["dimension"] == "implementation" and c["result"] == "PASS"
        for c in first["report"]["checks"]
    )


def test_raw_model_byte_change_invalidates_the_old_plan(api, concert):
    plan = current_plan(api, concert)
    concert.write_bytes(concert.read_bytes() + b"\n")
    rebase_planned_records(concert)
    assert_current_backend_ready(concert)
    assert artifact_result(api[1](concert, plan, host_context=host(concert))) != "PASS"


def test_contract_change_invalidates_the_old_plan(api, concert):
    plan = current_plan(api, concert)
    rewrite_model(
        concert,
        lambda m: m["nodes"]["action.reserve"].update(
            behavior=m["nodes"]["action.reserve"]["behavior"]
            + "\nAlso preserve an explicit audit marker."
        ),
    )
    assert_current_backend_ready(concert)
    assert artifact_result(api[1](concert, plan, host_context=host(concert))) != "PASS"


def test_changed_pinned_contract_asset_invalidates_old_plan(api, concert):
    source = concert.parent / "contract.txt"
    source.write_bytes(b"Contract revision one\n")

    def add_source(model):
        model["sources"]["source.contract.synthetic"] = {
            "role": "knowledge",
            "title": "Synthetic contract",
            "uri": "contract.txt",
            "format": "text/plain",
            "normative": True,
            "revision": {"sha256": sha256(source.read_bytes()).hexdigest()},
        }
        model["nodes"]["action.reserve"].setdefault("source_refs", []).append(
            {"source": "source.contract.synthetic", "purpose": "contract"}
        )

    rewrite_model(concert, add_source)
    plan = current_plan(api, concert)
    source.write_bytes(b"Contract revision two\n")
    rewrite_model(
        concert,
        lambda m: m["sources"]["source.contract.synthetic"]["revision"].update(
            sha256=sha256(source.read_bytes()).hexdigest()
        ),
    )
    assert_current_backend_ready(concert)
    assert artifact_result(api[1](concert, plan, host_context=host(concert))) != "PASS"


def test_tampered_packet_contract_cannot_verify_against_unchanged_model(api, concert):
    plan = deepcopy(current_plan(api, concert))
    node_payloads(packet(plan), "action.reserve")[0]["behavior"] = "Skip authorization"
    assert_current_backend_ready(concert)
    assert artifact_result(api[1](concert, plan, host_context=host(concert))) != "PASS"


def test_parallel_pass_cannot_hide_missing_structural_readiness(api, concert):
    declared = request(concert, ("action.reserve", "screen.reserve"))
    oracle = validate(
        concert,
        profiles=["readiness"],
        parallel_plan=parallel_projection(concert, declared),
    )
    assert aggregate(oracle["checks"], "readiness") == "PASS"
    result = prepare(api, concert, declared)
    assert aggregate(result["report"]["checks"], "readiness") == "FAIL"


@pytest.mark.parametrize("conflict", ["same_file", "directory", "same_workspace"])
def test_parallel_ownership_conflicts_remain_known_failures(api, concert, conflict):
    declared = request(concert, ("action.reserve", "action.cancel"))
    current_plan(api, concert, declared)
    left, right = declared["packages"]
    if conflict == "same_file":
        left["write_resources"] = right["write_resources"] = ["src/shared"]
    elif conflict == "directory":
        left["write_resources"], right["write_resources"] = ["src/"], ["src/cancel"]
    else:
        right["workspace"] = left["workspace"]
    result = prepare(api, concert, declared)
    assert aggregate(result["report"]["checks"], "readiness") == "FAIL"
    assert any(
        d["code"] == "WRITE_OWNERSHIP_CONFLICT"
        for c in result["report"]["checks"]
        for d in c["diagnostics"]
    )


def test_explicit_serialization_can_resolve_declared_write_overlap(api, concert):
    declared = request(concert, ("action.reserve", "action.cancel"))
    for work in declared["packages"]:
        work["write_resources"] = ["src/shared"]
    declared["serialized"] = [["package.reserve", "package.cancel"]]
    plan = current_plan(api, concert, declared)
    assert serialization(plan) == declared["serialized"]


def test_calls_and_depends_on_do_not_invent_code_execution_order(api, concert):
    def add_dependencies(model):
        for relation in ("calls", "depends_on"):
            model["edges"].append(
                {
                    "id": "edge.synthetic." + relation,
                    "from": "action.reserve",
                    "relation": relation,
                    "to": "action.cancel",
                }
            )

    rewrite_model(concert, add_dependencies)
    declared = request(concert, ("action.reserve", "action.cancel"))
    assert declared["serialized"] == []
    plan = current_plan(api, concert, declared)
    assert serialization(plan) == []
