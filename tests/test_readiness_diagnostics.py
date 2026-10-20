"""Actionable readiness diagnostics preserve result codes and real locations."""

from copy import deepcopy
import json
from pathlib import Path

import pytest

from gdd_validator import validate
from gdd_validator.loading import parse
from gdd_validator.report import aggregate, exit_code

ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture
def model():
    return json.loads((ROOT / "docs/tutorial/refined.gdd.json").read_text())


def diagnostics(tmp_path, model, fragment=False):
    model = deepcopy(model)
    if fragment:
        part = {"format": "gdd-fragment/0.3", "nodes": model.pop("nodes"), "edges": model.pop("edges")}
        (tmp_path / "part.json").write_text(json.dumps(part))
        model.update(nodes={}, edges=[], includes=["part.json"])
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model))
    report = validate(path, profiles=["readiness"], scope=["action.add", "component.arithmetic"])
    assert aggregate(report["checks"], "readiness") == "FAIL"
    assert exit_code(report) == 1
    found = [d for c in report["checks"] if c["dimension"] == "readiness" for d in c["diagnostics"]]
    for item in found:
        # Every reported pointer must resolve against the actual owning document.
        value = parse((tmp_path / item["location"]["file"]).read_bytes())
        for token in item["location"]["pointer"].split("/")[1:]:
            token = token.replace("~1", "/").replace("~0", "~")
            value = value[int(token)] if isinstance(value, list) else value[token]
    return found


@pytest.mark.parametrize("fragment", [False, True])
def test_missing_contract_and_behavior_stop_at_node(tmp_path, model, fragment):
    action = model["nodes"]["action.add"]
    action.pop("contract")
    action.pop("behavior")
    found = diagnostics(tmp_path, model, fragment)
    missing = next(d for d in found if "contract is missing fields:" in d["message"])
    assert missing["code"] == "READINESS_CONTRACT_MISSING"
    assert "input" in missing["message"] and "postconditions" in missing["message"]
    assert missing["location"] == {"file": "part.json" if fragment else "model.json", "pointer": "/nodes/action.add"}
    behavior = next(d for d in found if "behavior is missing or blank" in d["message"])
    assert behavior["location"] == missing["location"]


def test_partial_contract_lists_only_missing_fields(tmp_path, model):
    contract = model["nodes"]["action.add"]["contract"]
    contract.pop("output")
    contract.pop("input")
    found = diagnostics(tmp_path, model)
    item = next(d for d in found if "contract is missing fields:" in d["message"])
    assert item["message"] == "Action contract is missing fields: input, output."
    assert item["location"]["pointer"] == "/nodes/action.add/contract"


def test_existing_blank_behavior_points_to_field(tmp_path, model):
    model["nodes"]["action.add"]["behavior"] = " "
    found = diagnostics(tmp_path, model)
    item = next(d for d in found if "behavior is missing or blank" in d["message"])
    assert item["location"]["pointer"] == "/nodes/action.add/behavior"


@pytest.mark.parametrize("absent", [False, True])
def test_completeness_flags_name_exact_problem(tmp_path, model, absent):
    contract = model["nodes"]["action.add"]["contract"]
    if absent:
        contract.pop("dependencies_complete")
    else:
        contract["dependencies_complete"] = False
    found = diagnostics(tmp_path, model)
    item = next(d for d in found if d["code"] == "EFFECTS_INCOMPLETE")
    assert item["message"] == "Action contract must declare true: dependencies_complete."
    assert item["location"]["pointer"] == "/nodes/action.add/contract" + ("" if absent else "/dependencies_complete")


@pytest.mark.parametrize("field", ["responsibility", "rationale", "excludes"])
def test_boundary_missing_field_points_to_existing_boundary(tmp_path, model, field):
    model["nodes"]["component.arithmetic"]["boundary"].pop(field)
    found = diagnostics(tmp_path, model)
    item = next(d for d in found if d["code"] == "BOUNDARY_RATIONALE_MISSING")
    assert f"needs fields: {field};" in item["message"]
    assert item["location"]["pointer"] == "/nodes/component.arithmetic/boundary"


def test_interfaces_absent_and_invalid_ownership(tmp_path, model):
    component = model["nodes"]["component.arithmetic"]
    component.pop("interfaces")
    item = next(d for d in diagnostics(tmp_path, model) if "interfaces are missing" in d["message"])
    assert item["location"]["pointer"] == "/nodes/component.arithmetic"
    component["interfaces"] = ["action.add"]
    model["edges"] = [e for e in model["edges"] if e["relation"] != "owned_by"]
    item = next(d for d in diagnostics(tmp_path, model) if "Public interface actions" in d["message"])
    assert "action.add" in item["message"]
    assert item["location"]["pointer"] == "/nodes/component.arithmetic/interfaces"


def test_missing_boundary_lists_all_fields_at_component(tmp_path, model):
    model["nodes"]["component.arithmetic"].pop("boundary")
    item = next(d for d in diagnostics(tmp_path, model) if d["code"] == "BOUNDARY_RATIONALE_MISSING")
    assert "needs fields: responsibility, rationale, excludes;" in item["message"]
    assert item["location"]["pointer"] == "/nodes/component.arithmetic"


def test_all_incomplete_flags_have_stable_order(tmp_path, model):
    contract = model["nodes"]["action.add"]["contract"]
    contract["effects_complete"] = False
    contract["dependencies_complete"] = False
    item = next(d for d in diagnostics(tmp_path, model) if d["code"] == "EFFECTS_INCOMPLETE")
    assert item["message"] == "Action contract must declare true: effects_complete, dependencies_complete."
    assert item["location"]["pointer"] == "/nodes/action.add/contract/effects_complete"


def test_missing_risk_points_to_existing_risks(tmp_path, model):
    model["nodes"]["action.add"]["contract"]["risks"].pop("authorization")
    item = next(d for d in diagnostics(tmp_path, model) if "Risk assessment authorization" in d["message"])
    assert item["code"] == "READINESS_CONTRACT_MISSING"
    assert item["location"]["pointer"] == "/nodes/action.add/contract/risks"
