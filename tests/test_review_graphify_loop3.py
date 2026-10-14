"""Bounded projection, exact recovery and locator regressions from loop three."""

from copy import deepcopy
from hashlib import sha256
import json

import pytest
from jsonschema import Draft202012Validator

from gdd_validator import exit_code
from gdd_validator.graphify import (
    compact_observations,
    decode_json,
    observe_graphify,
)
from gdd_validator.graphify_io import (
    export_graphify,
    import_graphify,
    load_observations,
    verify_graphify,
)
from gdd_validator.graphify_proposal import propose_graphify

AST = "graphify-0.9.75-ast"
PROFILE = "graphify-0.9.75-node-link"


def raw_at_depth(depth):
    # Root object=1, nodes array=2, node object=3; scalar leaves add no depth.
    assert depth >= 3
    return (
        b'{"directed":true,"multigraph":false,"graph":{},"nodes":[{"id":"a","label":"A","file_type":"code","source_file":"app.py","vendor":'
        + b"[" * (depth - 3)
        + b"0"
        + b"]" * (depth - 3)
        + b'}],"links":[]}'
    )


def compact_mapping(index):
    return {
        "format": "gdd-graphify-mapping/0.2",
        "project": {"id": "project.review", "title": "Review", "goal": "Review"},
        "nodes": [
            {
                "observation": "observation.node.missing",
                "target": "component.a",
                "kind": "component",
                "description": "Review",
                "rationale": "Review",
            }
        ],
        "edges": [],
    }


def make_bundle(tmp_path, raw, name="bundle"):
    source = tmp_path / (name + ".raw.json")
    source.write_bytes(raw)
    folder = tmp_path / name
    report = import_graphify(source, folder, profile=PROFILE)
    return folder, report


def results(report):
    return {item["check_id"]: item["result"] for item in report["checks"]}


@pytest.mark.parametrize(
    "depth,expected",
    [(255, 0), (256, 0), (257, 2), (600, 2), (990, 2), (992, 2), (1200, 2)],
)
def test_raw_depth_boundary_is_consistent_and_exactly_recoverable(
    tmp_path, depth, expected
):
    raw = raw_at_depth(depth)
    observed_index, observed_report = observe_graphify(raw, profile=PROFILE)
    assert exit_code(observed_report) == expected
    folder, report = make_bundle(tmp_path, raw)
    assert exit_code(report) == expected
    index, recovered_raw, reloaded = load_observations(folder)
    assert recovered_raw == raw
    assert exit_code(reloaded) == expected
    assert index["artifact"]["revision"] == "sha256:" + sha256(raw).hexdigest()
    assert index["importer"]["version"] == "0.1.2"
    assert index["nodes"] == observed_index["nodes"]
    assert results(reloaded)["graphify.integrity"] == "PASS"
    recovered = tmp_path / "recovered.json"
    export_graphify(folder, recovered)
    assert recovered.read_bytes() == raw
    if expected == 2:
        assert index == compact_observations(index)
        assert results(reloaded)["graphify.projection"] == "UNKNOWN"
        assert any(
            d["code"] == "FORMAT_UNSUPPORTED"
            for c in reloaded["checks"]
            for d in c["diagnostics"]
        )
        with pytest.raises(ValueError, match="intact"):
            propose_graphify(folder, compact_mapping(index))
    else:
        assert len(index["nodes"]) == 1
        expected_id = (
            "observation.node.h"
            + sha256((index["artifact"]["revision"] + "/nodes/0").encode()).hexdigest()
        )
        assert index["nodes"][0]["id"] == expected_id


def test_index_embedding_depth_is_not_the_raw_cap(tmp_path):
    raw = raw_at_depth(256)
    folder, report = make_bundle(tmp_path, raw)
    assert exit_code(report) == 0
    index = decode_json((folder / "observations.json").read_bytes())
    assert len(index["nodes"]) == 1
    alias = folder / "custom-manifest.json"
    alias.write_bytes((folder / "observations.json").read_bytes())
    assert exit_code(verify_graphify(alias, profile=PROFILE)) == 0


@pytest.mark.parametrize("version", ["0.1.0", "0.1.1"])
def test_legacy_full_deep_index_is_unassessed_not_corrupt(tmp_path, version):
    raw = raw_at_depth(600)
    index, _ = observe_graphify(raw, profile=PROFILE)
    shallow, _ = observe_graphify(raw_at_depth(3), profile=PROFILE)
    node = deepcopy(shallow["nodes"][0])
    node["raw"] = json.loads(raw)["nodes"][0]
    node["identity"]["artifact_revision"] = index["artifact"]["revision"]
    node["id"] = (
        "observation.node.h"
        + sha256((index["artifact"]["revision"] + "/nodes/0").encode()).hexdigest()
    )
    binding = deepcopy(shallow["binding_candidates"][0])
    binding["observation"] = node["id"]
    index["nodes"] = [node]
    index["binding_candidates"] = [binding]
    index["importer"]["version"] = version
    folder = tmp_path / "legacy"
    folder.mkdir()
    (folder / "graphify.raw.json").write_bytes(raw)
    (folder / "observations.json").write_text(json.dumps(index))
    report = verify_graphify(folder)
    assert exit_code(report) == 2
    assert "FAIL" not in results(report).values()
    assert results(report)["graphify.integrity"] == "PASS"
    assert results(report)["graphify.projection"] == "UNKNOWN"
    export_graphify(folder, tmp_path / "recovered.json")
    assert (tmp_path / "recovered.json").read_bytes() == raw
    with pytest.raises(ValueError, match="intact"):
        propose_graphify(folder, compact_mapping(index))


@pytest.mark.parametrize("version", ["0.1.0", "0.1.1"])
def test_legacy_safe_projection_still_passes(tmp_path, version):
    folder, _ = make_bundle(tmp_path, raw_at_depth(20))
    manifest = folder / "observations.json"
    index = json.loads(manifest.read_text())
    index["importer"]["version"] = version
    manifest.write_text(json.dumps(index))
    assert exit_code(verify_graphify(folder)) == 0


@pytest.mark.parametrize("tamper", ["hash", "insert_record"])
def test_current_compact_bundle_cannot_hide_known_corruption(tmp_path, tamper):
    folder, _ = make_bundle(tmp_path, raw_at_depth(600))
    manifest = folder / "observations.json"
    index = json.loads(manifest.read_text())
    if tamper == "hash":
        index["artifact"]["revision"] = "sha256:" + "0" * 64
    else:
        index["nodes"] = observe_graphify(raw_at_depth(3), profile=PROFILE)[0]["nodes"]
    manifest.write_text(json.dumps(index))
    assert exit_code(verify_graphify(folder)) == 1
    with pytest.raises(ValueError, match="corrupt"):
        export_graphify(folder, tmp_path / "recovered.json")


def test_full_index_encoder_exhaustion_falls_back_without_a_crash(
    tmp_path, monkeypatch
):
    original = json.dumps

    def exhausted(value, *args, **kwargs):
        if (
            isinstance(value, dict)
            and value.get("format") == "gdd-observations/0.2"
            and value.get("nodes")
        ):
            raise RecursionError("simulated encoder capacity")
        return original(value, *args, **kwargs)

    monkeypatch.setattr("gdd_validator.graphify.json.dumps", exhausted)
    raw = raw_at_depth(3)
    assert exit_code(observe_graphify(raw, profile=PROFILE)[1]) == 2
    folder, report = make_bundle(tmp_path, raw)
    assert exit_code(report) == 2
    export_graphify(folder, tmp_path / "recovered.json")
    assert (tmp_path / "recovered.json").read_bytes() == raw


def test_full_index_schema_exhaustion_falls_back_without_a_crash(tmp_path, monkeypatch):
    original = Draft202012Validator.iter_errors

    def exhausted(self, instance, *args, **kwargs):
        if (
            self.schema.get("title", "").startswith("GDD observations")
            and isinstance(instance, dict)
            and instance.get("nodes")
        ):
            raise RecursionError("simulated schema capacity")
        return original(self, instance, *args, **kwargs)

    monkeypatch.setattr(Draft202012Validator, "iter_errors", exhausted)
    raw = raw_at_depth(3)
    assert exit_code(observe_graphify(raw, profile=PROFILE)[1]) == 2
    folder, report = make_bundle(tmp_path, raw)
    assert exit_code(report) == 2
    export_graphify(folder, tmp_path / "recovered.json")
    assert (tmp_path / "recovered.json").read_bytes() == raw


@pytest.mark.parametrize(
    "raw",
    [
        b'{"x":NaN}',
        b'{"x":Infinity}',
        b'{"x":-Infinity}',
        b'{"x":01}',
        b'{"nodes":[],"nodes":[]}',
        raw_at_depth(257)[:-1],
    ],
)
def test_known_malformed_json_stays_fail_and_recoverable(tmp_path, raw):
    folder, report = make_bundle(tmp_path, raw)
    assert exit_code(report) == 1
    export_graphify(folder, tmp_path / "recovered.json")
    assert (tmp_path / "recovered.json").read_bytes() == raw


@pytest.mark.parametrize("number", [b"1e309", b"9" * 5000])
def test_finite_numeric_runtime_limits_are_unknown_and_recoverable(tmp_path, number):
    raw = (
        b'{"nodes":[{"id":'
        + number
        + b',"label":"A","file_type":"code","source_file":"app.py"}],"edges":[]}'
    )
    index, report = observe_graphify(raw, profile=AST)
    assert exit_code(report) == 2 and not index["nodes"]
    assert any(
        d["code"] == "FORMAT_UNSUPPORTED"
        for c in report["checks"]
        for d in c["diagnostics"]
    )
    source = tmp_path / "raw.json"
    source.write_bytes(raw)
    folder = tmp_path / "bundle"
    assert exit_code(import_graphify(source, folder, profile=AST)) == 2
    export_graphify(folder, tmp_path / "recovered.json")
    assert (tmp_path / "recovered.json").read_bytes() == raw


@pytest.mark.parametrize("profile", [AST, PROFILE])
@pytest.mark.parametrize("filename", ["graph.json", "observations.json", "custom.json"])
def test_explicit_raw_profile_is_independent_of_filename(tmp_path, profile, filename):
    source = tmp_path / filename
    raw = raw_at_depth(3)
    if profile == AST:
        raw = raw.replace(b'"links":[]', b'"edges":[]')
    source.write_bytes(raw)
    assert exit_code(verify_graphify(source, profile=profile)) == 0


def test_actual_manifest_profile_mismatch_is_still_unknown(tmp_path):
    folder, _ = make_bundle(tmp_path, raw_at_depth(3))
    report = verify_graphify(folder / "observations.json", profile=AST)
    assert exit_code(report) == 2
    assert results(report)["graphify.profile"] == "UNKNOWN"


@pytest.mark.parametrize(
    "raw,expected",
    [
        (b"broken json", 1),
        (b'{"format":"gdd-observations/99"}', 2),
        (b'{"format":"gdd-observations/0.2"}', 1),
    ],
)
def test_manifest_filename_without_explicit_profile_keeps_manifest_handling(
    tmp_path, raw, expected
):
    source = tmp_path / "observations.json"
    source.write_bytes(raw)
    assert exit_code(verify_graphify(source)) == expected
