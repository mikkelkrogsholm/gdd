"""A secondary project mismatch requires a known primary model identity."""

import json

import pytest

from gdd_validator import validate
from gdd_validator.report import aggregate, exit_code
from test_review_protocol_loop4 import case as proof_case, write


PROFILES = [["representation"], ["model"], ["records"], ["readiness"]]
ACTIVITIES = [None, "model_changes", "implementation", "acceptance"]


def prepare(proof_case, activity):
    path, records, host = proof_case
    if activity:
        records["decision.x"]["activity"] = activity
        host["policy"]["activity"] = activity
        if activity == "acceptance":
            records["decision.x"].update(
                implementation=host["implementation"],
                code_revision=host["code_revision"],
            )
    write(path, records)
    return path, path.parent / "records.json", host


def unavailable_primary(path, variant):
    model = json.loads(path.read_text())
    if variant == "missing":
        path.unlink()
    elif variant == "unsupported":
        model["format"] = "gdd/99.0"
        path.write_text(json.dumps(model))
    elif variant == "deep-json":
        model["nodes"]["entity.x"]["annotations"] = {"example.deep": "RAW"}
        path.write_text(
            json.dumps(model).replace('"RAW"', "[" * 2000 + "0" + "]" * 2000)
        )
    elif variant == "deep-yaml":
        path.write_text(
            "format: gdd/0.2\n"
            "project: {id: loop4, title: Probe, goal: Unknown parsing capacity}\n"
            "nodes:\n  entity.x:\n    kind: entity\n    description: Value\n"
            "    fields: {}\n    annotations:\n      example.deep: "
            + "[" * 2000
            + "0"
            + "]" * 2000
            + "\nedges: []\n"
        )


@pytest.mark.parametrize(
    "variant", ["missing", "unsupported", "deep-json", "deep-yaml"]
)
@pytest.mark.parametrize("profiles", PROFILES)
@pytest.mark.parametrize("activity", ACTIVITIES)
def test_unknown_primary_does_not_fabricate_secondary_project_mismatch(
    proof_case, variant, profiles, activity
):
    path, secondary, host = prepare(proof_case, activity)
    unavailable_primary(path, variant)
    report = validate(
        secondary,
        model_path=path,
        profiles=profiles,
        scope=["entity.x"],
        activity=activity,
        host_context=host,
    )
    assert aggregate(report["checks"], "model") == "UNKNOWN"
    assert exit_code(report) == 2
    assert report["spec_revision"] is None
    assert not any(
        d["code"] == "RECORD_INVALID"
        for check in report["checks"]
        for d in check["diagnostics"]
    )
    if activity:
        assert aggregate(report["checks"], "authorization") == "UNRESOLVED"
    elif profiles == ["records"]:
        assert aggregate(report["checks"], "implementation") == "UNKNOWN"
    if profiles == ["readiness"]:
        assert aggregate(report["checks"], "readiness") == "UNKNOWN"


@pytest.mark.parametrize("matching", [True, False])
@pytest.mark.parametrize("profiles", PROFILES)
@pytest.mark.parametrize("activity", ACTIVITIES)
def test_known_primary_preserves_matching_merge_and_real_project_mismatch(
    proof_case, matching, profiles, activity
):
    path, _, host = prepare(proof_case, activity)
    secondary = path.parent / "secondary.json"
    secondary.write_text(
        json.dumps(
            {
                "format": "gdd-records/0.2",
                "project_id": "loop4" if matching else "other",
                "records": {},
            }
        )
    )
    report = validate(
        secondary,
        model_path=path,
        profiles=profiles,
        scope=["entity.x"],
        activity=activity,
        host_context=host,
    )
    assert aggregate(report["checks"], "model") == ("PASS" if matching else "FAIL")
    mismatches = [
        d
        for check in report["checks"]
        for d in check["diagnostics"]
        if d["code"] == "RECORD_INVALID" and "another project" in d["message"]
    ]
    assert bool(mismatches) is not matching
    if activity:
        assert aggregate(report["checks"], "authorization") == (
            "GRANTED" if matching else "UNRESOLVED"
        )
    elif profiles == ["records"]:
        assert aggregate(report["checks"], "implementation") == (
            "PASS" if matching else "UNKNOWN"
        )


@pytest.mark.parametrize("matching", [True, False])
def test_known_primary_identity_survives_unavailable_fragment(proof_case, matching):
    path, _, host = prepare(proof_case, None)
    model = json.loads(path.read_text())
    model["includes"] = ["missing-fragment.json"]
    path.write_text(json.dumps(model))
    secondary = path.parent / "secondary.json"
    secondary.write_text(
        json.dumps(
            {
                "format": "gdd-records/0.2",
                "project_id": "loop4" if matching else "other",
                "records": {},
            }
        )
    )
    report = validate(
        secondary, model_path=path, profiles=["records"], host_context=host
    )
    assert aggregate(report["checks"], "model") == ("UNKNOWN" if matching else "FAIL")
    assert exit_code(report) == (2 if matching else 1)


@pytest.mark.parametrize("secondary_kind", ["invalid-schema", "known-wrong-kind"])
def test_secondary_known_violations_remain_visible_with_unavailable_primary(
    proof_case, secondary_kind
):
    path, secondary, host = prepare(proof_case, None)
    unavailable_primary(path, "missing")
    if secondary_kind == "invalid-schema":
        secondary.write_text(
            '{"format":"gdd-records/0.2","project_id":"loop4","records":[]}'
        )
    else:
        secondary.write_text(
            json.dumps(
                {
                    "format": "gdd/0.2",
                    "project": {
                        "id": "loop4",
                        "title": "Other",
                        "goal": "Known wrong input kind",
                    },
                    "nodes": {},
                    "edges": [],
                }
            )
        )
    report = validate(
        secondary, model_path=path, profiles=["records"], host_context=host
    )
    assert aggregate(report["checks"], "model") == "FAIL"
    assert exit_code(report) == 1
    assert any(
        d["code"] == "SCHEMA_INVALID"
        for check in report["checks"]
        for d in check["diagnostics"]
    )
    assert not any(
        d["code"] == "RECORD_INVALID"
        for check in report["checks"]
        for d in check["diagnostics"]
    )
