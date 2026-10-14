"""Readiness decisions share the same integrity boundary as other used receipts."""

import json

import pytest

from gdd_validator import validate
from gdd_validator.loading import load_bundle
from gdd_validator.report import aggregate
from test_review_model_loop2 import decision, minimal_model, write_records, write_screen


@pytest.fixture(params=["screen", "question"])
def case(tmp_path, request):
    if request.param == "screen":
        path, scope = write_screen(tmp_path), ["screen.x"]
    else:
        model = minimal_model()
        model["record_files"] = ["records.json"]
        model["questions"] = {
            "question.x": {
                "statement": "Choose value",
                "targets": ["entity.x"],
                "blocks": ["implementation"],
                "status": "resolved",
                "resolution": {"decision": "decision.review"},
            }
        }
        path, scope = tmp_path / "model.json", ["entity.x"]
        path.write_text(json.dumps(model))
        write_records(tmp_path, {})
        write_records(
            tmp_path,
            {
                "decision.review": decision(
                    load_bundle(path).revision, scope, "model_change"
                )
            },
        )
    records = json.loads((tmp_path / "records.json").read_text())["records"]
    return path, scope, records


@pytest.mark.parametrize("variant", ["valid", "dangling_scope", "dangling_check"])
def test_used_readiness_decision_requires_record_integrity(case, variant):
    path, scope, records = case
    if variant == "dangling_scope":
        records["decision.review"]["scope"].append("entity.deleted")
    elif variant == "dangling_check":
        records["decision.review"]["checks"] = ["check.missing"]
    write_records(path.parent, records)
    report = validate(
        path,
        profiles=["readiness"],
        scope=scope,
        host_context={"trusted_decisions": ["decision.review"]},
    )
    if variant == "valid":
        assert aggregate(report["checks"], "model") == "PASS"
        assert aggregate(report["checks"], "readiness") == "PASS"
    else:
        assert aggregate(report["checks"], "model") == "FAIL"
        assert aggregate(report["checks"], "readiness") != "PASS"
