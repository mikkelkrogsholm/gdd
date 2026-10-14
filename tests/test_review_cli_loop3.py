"""Model asset paths must fail through report diagnostics, before filesystem IO."""

import json

import pytest

from gdd_validator import exit_code, validate
from gdd_validator.loading import InputError, resolve_asset, schema_checks


@pytest.mark.parametrize("asset", ["include", "records", "source", "extension"])
def test_model_embedded_nul_asset_path_returns_diagnostic(tmp_path, asset):
    model = {
        "format": "gdd/0.2",
        "project": {
            "id": "project.nul",
            "title": "NUL",
            "goal": "Reject invalid paths",
        },
        "nodes": {},
        "edges": [],
    }
    if asset == "include":
        model["includes"] = ["fragment\x00.json"]
    elif asset == "records":
        model["record_files"] = ["records\x00.json"]
    elif asset == "source":
        model["sources"] = {
            "source.test": {
                "role": "input",
                "title": "Invalid asset path",
                "format": "text/plain",
                "uri": "source\x00.txt",
                "normative": True,
                "revision": {"sha256": "a" * 64},
            }
        }
    else:
        model["extensions"] = {
            "extension.test": {
                "namespace": "org.test",
                "version": "1.0.0",
                "path": "extension\x00.json",
                "sha256": "a" * 64,
            }
        }
    assert schema_checks(model, "model.json")[0]["result"] == "PASS"
    path = tmp_path / "model.json"
    path.write_text(json.dumps(model))
    report = validate(path)
    assert exit_code(report) == 1
    assert "ASSET_PATH_INVALID" in {
        d["code"] for c in report["checks"] for d in c["diagnostics"]
    }
    assert schema_checks(report, "report.json")[0]["result"] == "PASS"


def test_asset_path_rejects_nul_as_input_error(tmp_path):
    with pytest.raises(InputError) as error:
        resolve_asset(tmp_path, "file\x00.txt")
    assert error.value.code == "ASSET_PATH_INVALID"
