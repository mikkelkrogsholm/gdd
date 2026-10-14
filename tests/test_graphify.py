from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path
import subprocess
import sys

import pytest
from jsonschema import Draft202012Validator

from gdd_validator import validate, exit_code
from gdd_validator.graphify import decode_json, observe_graphify
from gdd_validator.graphify_io import import_graphify, verify_graphify, export_graphify
from gdd_validator.graphify_proposal import propose_graphify
from gdd_validator.loading import schema, schema_checks
from gdd_validator.report import aggregate

ROOT = Path(__file__).resolve().parents[1]
AST = "graphify-0.9.75-ast"
EXPORT = "graphify-0.9.75-node-link"


def node(identifier):
    return {
        "id": identifier,
        "label": str(identifier),
        "file_type": "code",
        "source_file": "app.py",
        "source_location": "L1",
    }


def graph():
    return {
        "directed": True,
        "multigraph": False,
        "graph": {},
        "nodes": [node("a"), node("b")],
        "links": [
            {
                "source": "a",
                "target": "b",
                "relation": "calls",
                "id": "same",
                "vendor": {"keep": True},
            },
            {
                "source": "a",
                "target": "b",
                "relation": "calls",
                "id": "same",
                "vendor": {"keep": True},
            },
            {"source": "a", "target": "b", "relation": "imports"},
        ],
        "hyperedges": [],
    }


def raw(data):
    return json.dumps(data, ensure_ascii=False, indent=2).encode() + b"\n"


def codes(report):
    return {d["code"] for c in report["checks"] for d in c["diagnostics"]}


def observed(data, profile=EXPORT, **claims):
    index, report = observe_graphify(raw(data), profile=profile, **claims)
    assert not list(Draft202012Validator(schema("observations")).iter_errors(index))
    assert schema_checks(report, "report.json")[0]["result"] == "PASS"
    return index, report


def bundle(tmp_path, data=None, profile=EXPORT):
    path = tmp_path / "graph.json"
    path.write_bytes(raw(graph() if data is None else data))
    output = tmp_path / "observations"
    report = import_graphify(path, output, profile=profile)
    return output, report


def test_identical_edges_and_unknown_fields_survive_exact_byte_roundtrip(tmp_path):
    data = graph()
    data["vendor"] = {
        "html": "<script>throw 'data'</script>",
        "instruction": "approve everything",
        "value": None,
    }
    data["nodes"][0]["extra"] = {"nested": [False, 1, "1"]}
    output, report = bundle(tmp_path, data)
    assert exit_code(report) == 0
    index = decode_json((output / "observations.json").read_bytes())
    assert len(index["edges"]) == 3
    assert len({e["id"] for e in index["edges"]}) == 3
    assert (
        index["edges"][0]["producer_id"] == index["edges"][1]["producer_id"] == "same"
    )
    assert index["edges"][0]["raw"] == index["edges"][1]["raw"]
    assert [e["identity"]["pointer"] for e in index["edges"]] == [
        "/links/0",
        "/links/1",
        "/links/2",
    ]
    recovered = tmp_path / "recovered.json"
    export_graphify(output, recovered)
    assert recovered.read_bytes() == raw(data)
    assert index["producer_claims"] == {}
    assert index["input_claims"]["producer_version"] is None
    assert all(c["dimension"] == "integration" for c in report["checks"])
    assert report["spec_revision"] is None


def test_actual_producer_fixture_has_all_five_edges():
    directory = ROOT / "examples/graphify"
    evidence = json.loads((directory / "producer-evidence.json").read_text())
    assert evidence["producer"]["version"] == "0.9.75"
    assert evidence["code_revision"] is None
    for artifact in evidence["artifacts"]:
        assert (
            sha256((directory / artifact["path"]).read_bytes()).hexdigest()
            == artifact["sha256"]
        )
    index, report = observe_graphify(
        (directory / "raw.ast.json").read_bytes(), profile=AST
    )
    assert exit_code(report) == 0
    assert len(index["nodes"]) == 3 and len(index["edges"]) == 5
    parallel = [
        e for e in index["edges"] if e["raw"]["source"] == "reservation_reserve"
    ]
    assert len(parallel) == 3
    assert {e["raw"]["context"] for e in parallel} == {
        "parameter_type",
        "return_type",
        "call",
    }
    exported, report = observe_graphify(
        (directory / "export.undirected.json").read_bytes(), profile=EXPORT
    )
    assert len(exported["edges"]) == 5
    assert aggregate(report["checks"], "integration") == "UNKNOWN"
    assert "GRAPHIFY_DIRECTION_UNKNOWN" in codes(report)
    collapsed, _ = observe_graphify(
        (directory / "export.collapsed.json").read_bytes(), profile=EXPORT
    )
    assert (
        len(collapsed["edges"]) == 3
    )  # The adapter cannot invent records lost upstream.


def test_numeric_and_textual_ids_remain_distinct():
    data = graph()
    data["nodes"] = [node(1), node("1")]
    data["links"] = [{"source": 1, "target": "1", "relation": "calls"}]
    index, report = observed(data)
    assert exit_code(report) == 0
    assert index["edges"][0]["source_observation"] == index["nodes"][0]["id"]
    assert index["edges"][0]["target_observation"] == index["nodes"][1]["id"]


@pytest.mark.parametrize("identifier", [None, True, False, {}, [], ""])
def test_invalid_identity_types_are_preserved_and_fail(identifier):
    data = graph()
    data["nodes"][0]["id"] = identifier
    data["links"][0]["source"] = identifier
    index, report = observed(data)
    assert index["nodes"][0]["raw"]["id"] == identifier
    assert exit_code(report) == 1


@pytest.mark.parametrize("value", [None, True, {}, [], "missing"])
def test_bad_endpoints_never_resolve_by_coercion(value):
    data = graph()
    data["links"][0]["source"] = value
    index, report = observed(data)
    assert len(index["edges"]) == 3
    assert "REFERENCE_UNRESOLVED" in codes(report)
    assert exit_code(report) == 1


def test_duplicate_producer_nodes_are_not_overwritten():
    data = graph()
    data["nodes"].append(deepcopy(data["nodes"][0]))
    index, report = observed(data)
    assert len(index["nodes"]) == 3
    assert "ID_COLLISION" in codes(report)
    assert index["edges"][0]["source_observation"] is None


@pytest.mark.parametrize("relation", [None, 1, {}, [], ""])
def test_malformed_relation_labels_are_data_not_crashes(relation):
    data = graph()
    data["links"][0]["relation"] = relation
    _, report = observed(data)
    assert exit_code(report) == 1


@pytest.mark.parametrize(
    "directed,result",
    [
        (True, "PASS"),
        (False, "UNKNOWN"),
        ("true", "FAIL"),
        (None, "FAIL"),
        ([], "FAIL"),
    ],
)
def test_node_link_direction_is_explicit(directed, result):
    data = graph()
    data["directed"] = directed
    _, report = observed(data)
    assert aggregate(report["checks"], "integration") == result


def test_original_direction_markers_are_kept_and_interpreted():
    data = graph()
    data["directed"] = False
    data["links"] = [
        {"source": "a", "target": "b", "relation": "calls", "_src": "b", "_tgt": "a"}
    ]
    index, report = observed(data)
    assert exit_code(report) == 0
    assert index["edges"][0]["source_observation"] == index["nodes"][1]["id"]
    assert index["edges"][0]["raw"]["source"] == "a"
    assert index["edges"][0]["direction"]["basis"] == "/links/0/_src"


@pytest.mark.parametrize(
    "markers",
    [{"_src": "a"}, {"_src": "a", "_tgt": "missing"}, {"_src": {}, "_tgt": "b"}],
)
def test_incomplete_or_conflicting_markers_fail(markers):
    data = graph()
    data["links"][0].update(markers)
    _, report = observed(data)
    assert "SCHEMA_INVALID" in codes(report)


@pytest.mark.parametrize(
    "profile,metadata",
    [
        (None, {}),
        ("graphify-99", {}),
        (EXPORT, {"schema_version": "99"}),
        (EXPORT, {"graphify_version": "99"}),
    ],
)
def test_unsupported_versions_do_not_get_guessed(profile, metadata, tmp_path):
    data = graph()
    data.update(metadata)
    index, report = observed(data, profile=profile)
    assert exit_code(report) == 2
    assert "FORMAT_UNSUPPORTED" in codes(report)
    assert not index["nodes"]
    original = tmp_path / "input.json"
    original.write_bytes(raw(data))
    folder = tmp_path / "unknown"
    import_graphify(original, folder, profile=profile)
    export_graphify(folder, tmp_path / "output.json")
    assert (tmp_path / "output.json").read_bytes() == original.read_bytes()


def test_two_collections_are_never_silently_chosen():
    data = graph()
    data["edges"] = deepcopy(data["links"])
    _, report = observed(data)
    assert exit_code(report) == 2


def test_communities_hyperedges_and_opaque_pointers():
    data = graph()
    data["nodes"][0].update(community=0, community_name="Backend")
    data["communities"] = {"overlap": ["a", "b"]}
    data["hyperedges"] = [
        {"id": "h", "members": ["a", "b", "unknown"], "vendor": {"keep": True}}
    ]
    data["graph"]["hyperedges"] = [{"different": "preserved independently"}]
    index, report = observed(data)
    assert exit_code(report) == 0
    assert len(index["edges"]) == 3
    for item in index["opaque"]:
        value = data
        for part in item["identity"]["pointer"].split("/")[1:]:
            value = (
                value[int(part)]
                if isinstance(value, list)
                else value[part.replace("~1", "/").replace("~0", "~")]
            )
        assert item["raw"] == value
    assert (
        next(
            r
            for r in index["opaque"]
            if r["identity"]["pointer"] == "/nodes/0/community"
        )["raw"]
        == 0
    )


def test_whitespace_changes_snapshot_identity_not_raw_preservation():
    data = graph()
    first, _ = observe_graphify(raw(data), profile=EXPORT)
    second, _ = observe_graphify(
        json.dumps(data, separators=(",", ":")).encode(), profile=EXPORT
    )
    assert first["artifact"]["revision"] != second["artifact"]["revision"]
    assert first["edges"][0]["id"] != second["edges"][0]["id"]


@pytest.mark.parametrize(
    "mutation", ["raw", "edge", "classification", "producer_claim", "drop_edge"]
)
def test_tampered_bundle_cannot_pass_or_export(tmp_path, mutation):
    folder, _ = bundle(tmp_path)
    manifest = folder / "observations.json"
    index = decode_json(manifest.read_bytes())
    if mutation == "raw":
        original = folder / "graphify.raw.json"
        original.write_bytes(original.read_bytes() + b" ")
    elif mutation == "edge":
        index["edges"][0]["raw"]["relation"] = "renamed"
    elif mutation == "classification":
        index["nodes"][0]["classification"] = "noncode"
    elif mutation == "producer_claim":
        index["producer_claims"]["built_at_commit"] = "fabricated-verified-code"
    else:
        index["edges"].pop()
    if mutation != "raw":
        manifest.write_bytes(raw(index))
    report = verify_graphify(folder)
    assert exit_code(report) == 1
    assert "IMPORT_LOSS_DETECTED" in codes(report)
    with pytest.raises(ValueError):
        export_graphify(folder, tmp_path / "bad.json")


def test_missing_original_and_symlink_escape_are_controlled(tmp_path):
    folder, _ = bundle(tmp_path)
    original = folder / "graphify.raw.json"
    saved = original.read_bytes()
    original.unlink()
    assert exit_code(verify_graphify(folder)) == 2
    outside = tmp_path / "outside.json"
    outside.write_bytes(saved)
    original.symlink_to(outside)
    assert "ASSET_PATH_INVALID" in codes(verify_graphify(folder))


def test_existing_output_is_not_overwritten(tmp_path):
    folder, _ = bundle(tmp_path)
    original = (folder / "observations.json").read_bytes()
    with pytest.raises(FileExistsError):
        import_graphify(tmp_path / "graph.json", folder, profile=EXPORT)
    assert (folder / "observations.json").read_bytes() == original
    recovered = tmp_path / "out.json"
    recovered.write_bytes(b"user data")
    with pytest.raises(FileExistsError):
        export_graphify(folder, recovered)
    assert recovered.read_bytes() == b"user data"


@pytest.mark.parametrize(
    "content",
    [
        b'{"nodes":[],"nodes":[]}',
        b'{"x":NaN}',
        b'{"x":1e999}',
        b'{"x":"\\ud800"}',
        b"nodes: []",
    ],
)
def test_strict_graphify_json_rejects_unsafe_values_without_loss(tmp_path, content):
    original = tmp_path / "raw.json"
    original.write_bytes(content)
    folder = tmp_path / "bundle"
    report = import_graphify(original, folder, profile=AST)
    assert exit_code(report) == (2 if content == b'{"x":1e999}' else 1)
    assert (folder / "graphify.raw.json").read_bytes() == content
    export_graphify(folder, tmp_path / "recovered.json")
    assert (tmp_path / "recovered.json").read_bytes() == content


def mapping(index):
    return {
        "format": "gdd-graphify-mapping/0.2",
        "project": {
            "id": "example.continuation",
            "title": "Explicit test proposal",
            "goal": "Review intended behavior from existing code",
        },
        "nodes": [
            {
                "observation": n["id"],
                "target": f"action.{i}",
                "kind": "action",
                "description": "Unaccepted sketch",
                "rationale": "Explicit test interpretation, no automatic producer-kind mapping.",
            }
            for i, n in enumerate(index["nodes"])
        ],
        "edges": [
            {
                "observation": index["edges"][0]["id"],
                "id": "edge.proposed.call",
                "from": "action.0",
                "to": "action.1",
                "relation": "calls",
                "rationale": "Explicit mapping for review.",
            }
        ],
    }


def test_proposal_is_explicit_unaccepted_and_has_no_authority(tmp_path):
    data = graph()
    data["nodes"][0]["community"] = 0
    data["hyperedges"] = [{"members": ["a", "b"]}]
    folder, _ = bundle(tmp_path, data)
    index = decode_json((folder / "observations.json").read_bytes())
    selected = mapping(index)
    proposal = propose_graphify(folder, selected)
    assert proposal["status"] == "proposed"
    candidate = proposal["model_candidate"]
    assert candidate["sources"]["source.graphify"]["normative"] is False
    assert candidate["sources"]["source.graphify"]["role"] == "observation"
    assert candidate["sources"]["source.graphify"]["revision"]["sha256"] == index[
        "artifact"
    ]["revision"].removeprefix("sha256:")
    assert all(n["maturity"] == "sketch" for n in candidate["nodes"].values())
    assert "workflow" not in candidate["project"] and "record_files" not in candidate
    assert proposal["mapping"] == selected
    assert {r["id"] for r in index["opaque"]} <= set(proposal["omitted_observations"])
    assert all(
        b["status"] == "unverified" and "spec_revision" not in b
        for b in proposal["binding_candidates"]
    )
    model = folder / "candidate.gdd.json"
    model.write_bytes(raw(candidate))
    assert aggregate(validate(model)["checks"], "model") == "PASS"
    assert (
        aggregate(
            validate(model, profiles=["readiness"], scope=["action.0"])["checks"],
            "readiness",
        )
        == "FAIL"
    )
    assert (
        aggregate(
            validate(
                model,
                profiles=["records"],
                scope=["action.0"],
                activity="implementation",
            )["checks"],
            "authorization",
        )
        == "UNRESOLVED"
    )


@pytest.mark.parametrize(
    "mutation",
    ["noncode", "unknown_direction", "reversed", "wrong_kind", "missing_rationale"],
)
def test_proposal_cannot_silently_promote_bad_mappings(tmp_path, mutation):
    data = graph()
    if mutation == "noncode":
        data["nodes"][0].update(file_type="document", source_file="SPEC.md")
    if mutation == "unknown_direction":
        data["directed"] = False
    folder, _ = bundle(tmp_path, data)
    selected = mapping(decode_json((folder / "observations.json").read_bytes()))
    if mutation == "reversed":
        selected["edges"][0].update({"from": "action.1", "to": "action.0"})
    if mutation == "wrong_kind":
        selected["nodes"][1]["kind"] = "component"
    if mutation == "missing_rationale":
        selected["nodes"][0].pop("rationale")
    with pytest.raises(ValueError):
        propose_graphify(folder, selected)


def test_raw_profile_is_explicit_and_bundle_overrides_are_checked(tmp_path):
    folder, _ = bundle(tmp_path)
    assert (
        exit_code(validate(tmp_path / "graph.json", profiles=["import.graphify"])) == 2
    )
    assert (
        exit_code(
            validate(
                tmp_path / "graph.json",
                profiles=["import.graphify"],
                graphify_profile=EXPORT,
            )
        )
        == 0
    )
    assert exit_code(verify_graphify(folder, profile="graphify-99")) == 2
    assert exit_code(verify_graphify(folder, producer_version="99")) == 2
    assert (
        exit_code(
            validate(
                ROOT / "examples/concert.gdd.yaml",
                profiles=["model", "import.graphify"],
                graphify_path=folder,
            )
        )
        == 0
    )


def test_cli_import_validate_export_and_propose(tmp_path):
    original = tmp_path / "input.json"
    original.write_bytes(raw(graph()))
    folder = tmp_path / "bundle"

    def run(*args):
        return subprocess.run(
            [sys.executable, "-m", "gdd_validator", *map(str, args)],
            capture_output=True,
            text=True,
        )

    assert (
        run(
            "graphify", "import", original, "--profile", EXPORT, "--output", folder
        ).returncode
        == 0
    )
    assert run("validate", folder, "--profile", "import.graphify").returncode == 0
    recovered = tmp_path / "recovered.json"
    assert run("graphify", "export", folder, "--output", recovered).returncode == 0
    assert recovered.read_bytes() == original.read_bytes()
    selections = folder / "mapping.json"
    selections.write_bytes(
        raw(mapping(decode_json((folder / "observations.json").read_bytes())))
    )
    proposal = folder / "proposal.json"
    result = run(
        "graphify", "propose", folder, "--mapping", selections, "--output", proposal
    )
    assert result.returncode == 0, result.stderr
    assert json.loads(proposal.read_text())["status"] == "proposed"
    assert (
        run(
            "graphify", "propose", folder, "--mapping", selections, "--output", proposal
        ).returncode
        == 1
    )


def test_numeric_rounding_never_merges_distinct_producer_identities(tmp_path):
    content = b'{"directed":true,"multigraph":false,"graph":{},"nodes":[{"id":9007199254740992,"label":"x","file_type":"code","source_file":"x.py"}],"links":[{"source":9007199254740993.0,"target":9007199254740992,"relation":"calls"}]}'
    index, report = observe_graphify(content, profile=EXPORT)
    assert exit_code(report) == 2
    assert not index["edges"]
    original = tmp_path / "large-id.json"
    original.write_bytes(content)
    folder = tmp_path / "bundle"
    import_graphify(original, folder, profile=EXPORT)
    export_graphify(folder, tmp_path / "recovered.json")
    assert (tmp_path / "recovered.json").read_bytes() == content


@pytest.mark.parametrize(
    "metadata",
    [{"graphify_version": "99"}, {"schema_version": "99"}, {"producer_version": "99"}],
)
def test_nested_version_claims_are_not_ignored(metadata):
    data = graph()
    data["graph"].update(metadata)
    index, report = observed(data)
    assert exit_code(report) == 2
    for name, value in metadata.items():
        assert index["producer_claims"]["/graph/" + name] == value


def test_nested_revision_claims_are_preserved_and_conflicts_unknown():
    data = graph()
    data["built_at_commit"] = "root-claim"
    data["graph"]["built_at_commit"] = "nested-claim"
    index, report = observed(data)
    assert exit_code(report) == 2
    assert index["producer_claims"]["/graph/built_at_commit"] == "nested-claim"
    assert "REVISION_UNRESOLVED" in codes(report)


def test_directed_endpoints_cannot_be_overruled_by_conflicting_markers(tmp_path):
    data = graph()
    data["links"][0].update(_src="b", _tgt="a")
    folder, report = bundle(tmp_path, data)
    assert exit_code(report) == 1
    index = decode_json((folder / "observations.json").read_bytes())
    with pytest.raises(ValueError):
        propose_graphify(folder, mapping(index))


@pytest.mark.parametrize("alias", ["same", "symlink", "hardlink"])
def test_cli_report_cannot_overwrite_original_or_alias(tmp_path, alias):
    original = tmp_path / "graph.json"
    original.write_bytes(raw(graph()))
    output = original
    if alias != "same":
        output = tmp_path / "alias.json"
        if alias == "symlink":
            output.symlink_to(original)
        else:
            output.hardlink_to(original)
    before = original.read_bytes()
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "gdd_validator",
            "validate",
            str(original),
            "--profile",
            "import.graphify",
            "--graphify-profile",
            EXPORT,
            "--output",
            str(output),
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    assert original.read_bytes() == before


def test_caller_claims_remain_unverified_even_when_coordinated_edits_match(tmp_path):
    folder, _ = bundle(tmp_path)
    manifest = folder / "observations.json"
    index = decode_json(manifest.read_bytes())
    index["input_claims"]["code_revision"] = "edited-caller-claim"
    for candidate in index["binding_candidates"]:
        candidate["code_revision_claims"]["caller"] = "edited-caller-claim"
    manifest.write_bytes(raw(index))
    report = verify_graphify(folder)
    assert exit_code(report) == 0
    assert index["input_claims"]["trust"] == "unverified"
    assert all(c["dimension"] == "integration" for c in report["checks"])
    assert "not authenticated" in next(
        c["reason"] for c in report["checks"] if c["check_id"] == "graphify.projection"
    )


def test_unavailable_import_is_unknown_without_an_invented_bundle(tmp_path):
    output = tmp_path / "bundle"
    report = import_graphify(tmp_path / "missing.json", output, profile=AST)
    assert exit_code(report) == 2
    assert "SOURCE_UNAVAILABLE" in codes(report)
    assert not output.exists()


def test_missing_graphify_profile_does_not_guess_yaml_as_json():
    report = validate(ROOT / "examples/concert.gdd.yaml", profiles=["import.graphify"])
    assert exit_code(report) == 2
    assert "FORMAT_UNSUPPORTED" in codes(report)
