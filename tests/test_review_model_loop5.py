"""Declared allowed imports cannot contradict explicit forbidden model paths."""

from copy import deepcopy
import json

import pytest

from gdd_validator.loading import load_bundle
from gdd_validator.report import aggregate
from gdd_validator.validator import validate


def model_graph(
    paths,
    *,
    reach="direct",
    source="component.a",
    target="component.b",
    blocking=True,
    targeted=False,
    constraint=True
):
    model = {
        "format": "gdd/0.2",
        "project": {
            "id": "loop5",
            "title": "Loop5",
            "goal": "Synthetic intended import constraints",
        },
        "nodes": {
            "component."
            + name: {"kind": "component", "description": name, "interfaces": []}
            for name in ("a", "b", "c", "d")
        },
        "edges": [
            {
                "id": "edge." + str(n),
                "from": "component." + a,
                "relation": "imports",
                "to": "component." + b,
            }
            for n, (a, b) in enumerate(paths)
        ],
    }
    if constraint:
        model["nodes"]["rule.forbid"] = {
            "kind": "rule",
            "description": "Forbidden intended import",
            "rule": {
                "category": "architecture",
                "statement": "This import path is forbidden",
                "scope": "targeted" if targeted else "global",
                "blocking": blocking,
                "verification": ["imports_checker"],
                "constraint": {
                    "relation": "imports",
                    "from": source,
                    "to": target,
                    "reach": reach,
                    "policy": "forbid",
                },
            },
        }
        if targeted:
            model["edges"].append(
                {
                    "id": "edge.constraint",
                    "from": "rule.forbid",
                    "relation": "constrains",
                    "to": "component.a",
                }
            )
    return model


def run(tmp_path, model, **options):
    path = tmp_path / "model.json"
    raw = json.dumps(model)
    path.write_text(raw)
    report = validate(path, **options)
    # Validation never collapses parallel edge IDs or edits the authoritative input.
    assert path.read_text() == raw
    assert load_bundle(path).model["edges"] == model["edges"]
    return report


def forbidden_diagnostics(report):
    return [
        d
        for c in report["checks"]
        for d in c["diagnostics"]
        if d["code"] == "CONSTRAINT_INVALID" and "forbidden" in d["message"]
    ]


@pytest.mark.parametrize(
    "paths,reach,source,target,expected",
    [
        ([("a", "b")], "direct", "a", "b", "FAIL"),
        ([("a", "b")], "transitive", "a", "b", "FAIL"),
        ([("b", "a")], "direct", "a", "b", "PASS"),
        ([("b", "a")], "transitive", "a", "b", "PASS"),
        ([("a", "c"), ("c", "b")], "direct", "a", "b", "PASS"),
        ([("a", "c"), ("c", "b")], "transitive", "a", "b", "FAIL"),
        ([], "direct", "a", "a", "PASS"),
        ([], "transitive", "a", "a", "PASS"),
        ([("a", "a")], "direct", "a", "a", "FAIL"),
        ([("a", "a")], "transitive", "a", "a", "FAIL"),
        ([("a", "c"), ("c", "a")], "direct", "a", "a", "PASS"),
        ([("a", "c"), ("c", "a")], "transitive", "a", "a", "FAIL"),
        ([("c", "d"), ("d", "c")], "transitive", "a", "b", "PASS"),
        ([("a", "c"), ("c", "c"), ("c", "b")], "transitive", "a", "b", "FAIL"),
        (
            [("a", "c"), ("c", "a"), ("a", "d"), ("d", "b")],
            "transitive",
            "a",
            "b",
            "FAIL",
        ),
        ([("a", "b"), ("a", "b")], "direct", "a", "b", "FAIL"),
        ([("a", "c"), ("a", "c"), ("c", "b")], "direct", "a", "b", "PASS"),
        ([("a", "c"), ("a", "c"), ("c", "b")], "transitive", "a", "b", "FAIL"),
    ],
)
def test_forbidden_declared_import_path_semantics(
    tmp_path, paths, reach, source, target, expected
):
    model = model_graph(
        paths, reach=reach, source="component." + source, target="component." + target
    )
    report = run(tmp_path, model)
    assert aggregate(report["checks"], "model") == expected
    diagnostics = forbidden_diagnostics(report)
    assert len(diagnostics) == (1 if expected == "FAIL" else 0)
    if diagnostics:
        assert diagnostics[0]["related_ids"] == ["rule.forbid"]
        assert diagnostics[0]["location"]["pointer"].endswith("/rule/constraint")


@pytest.mark.parametrize("reach", ["direct", "transitive"])
@pytest.mark.parametrize("blocking", [True, False])
@pytest.mark.parametrize("targeted", [True, False])
def test_all_declared_forbids_are_model_constraints(
    tmp_path, reach, blocking, targeted
):
    model = model_graph([("a", "b")], reach=reach, blocking=blocking, targeted=targeted)
    assert aggregate(run(tmp_path, model)["checks"], "model") == "FAIL"


@pytest.mark.parametrize("profile", ["model", "readiness"])
def test_constraint_outside_selected_scope_still_fails_model(tmp_path, profile):
    model = model_graph([("a", "b")], targeted=True)
    report = run(tmp_path, model, profiles=[profile], scope=["component.d"])
    assert aggregate(report["checks"], "model") == "FAIL"
    assert forbidden_diagnostics(report)


def test_representation_profile_does_not_claim_model_semantics(tmp_path):
    report = run(tmp_path, model_graph([("a", "b")]), profiles=["representation"])
    assert aggregate(report["checks"], "model") == "PASS"
    assert not forbidden_diagnostics(report)


def test_cycles_and_parallel_edges_without_forbid_remain_legal(tmp_path):
    report = run(
        tmp_path, model_graph([("a", "b"), ("b", "a"), ("a", "b")], constraint=False)
    )
    assert aggregate(report["checks"], "model") == "PASS"


@pytest.mark.parametrize("with_import,expected", [(False, "PASS"), (True, "FAIL")])
def test_other_relations_do_not_become_import_paths(tmp_path, with_import, expected):
    model = model_graph([("a", "b")] if with_import else [], reach="transitive")
    model["edges"].extend(
        [
            {
                "id": "edge.dependency.one",
                "from": "component.a",
                "relation": "depends_on",
                "to": "component.b",
            },
            {
                "id": "edge.dependency.two",
                "from": "component.a",
                "relation": "depends_on",
                "to": "component.b",
            },
        ]
    )
    assert aggregate(run(tmp_path, model)["checks"], "model") == expected


@pytest.mark.parametrize(
    "defect,code",
    [
        ("relation", "CONSTRAINT_INVALID"),
        ("category", "CONSTRAINT_INVALID"),
        ("missing_endpoint", "REFERENCE_UNRESOLVED"),
        ("endpoint_kind", "ENDPOINT_KIND_INVALID"),
    ],
)
def test_unsupported_constraint_context_keeps_existing_guards(tmp_path, defect, code):
    model = model_graph([("a", "b")])
    rule = model["nodes"]["rule.forbid"]["rule"]
    if defect == "relation":
        rule["constraint"]["relation"] = "depends_on"
    elif defect == "category":
        rule["category"] = "invariant"
    elif defect == "missing_endpoint":
        rule["constraint"]["to"] = "component.deleted"
    else:
        model["nodes"]["entity.x"] = {
            "kind": "entity",
            "description": "Data",
            "fields": {},
        }
        rule["constraint"]["to"] = "entity.x"
    report = run(tmp_path, model)
    assert aggregate(report["checks"], "model") == "FAIL"
    assert any(d["code"] == code for c in report["checks"] for d in c["diagnostics"])
    assert not forbidden_diagnostics(report)


def test_multiple_forbids_are_evaluated_without_changing_edge_identity(tmp_path):
    model = model_graph([("a", "b")], blocking=False)
    model["nodes"]["rule.other"] = deepcopy(model["nodes"]["rule.forbid"])
    model["nodes"]["rule.other"]["rule"]["constraint"]["reach"] = "transitive"
    report = run(tmp_path, model)
    assert aggregate(report["checks"], "model") == "FAIL"
    assert {tuple(d["related_ids"]) for d in forbidden_diagnostics(report)} == {
        ("rule.forbid",),
        ("rule.other",),
    }
