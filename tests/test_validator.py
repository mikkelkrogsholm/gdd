from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path
import runpy
import shutil
import subprocess
import sys

import pytest

from gdd_validator import exit_code, validate
from gdd_validator.loading import Bundle, InputError, digest, load_bundle, parse
from gdd_validator.model import has_cycle, validate_model
from gdd_validator.readiness import context_closure
from gdd_validator.records import authorize, evaluate_implementation
from gdd_validator.report import aggregate

ROOT = Path(__file__).resolve().parents[1]
HARNESS = runpy.run_path(str(ROOT / "conformance/run.py"))


def write(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")


@pytest.fixture
def examples(tmp_path):
    directory = tmp_path / "examples"
    shutil.copytree(ROOT / "examples", directory)
    return directory


def modified(examples, operation):
    path = examples / "concert.gdd.yaml"
    data = parse(path.read_bytes())
    data.pop("record_files", None)
    operation(data)
    write(path, data)
    return path


def codes(report):
    return {d["code"] for c in report["checks"] for d in c["diagnostics"]}


@pytest.mark.parametrize(
    "raw,expected",
    [
        ("a: yes\nb: on\nc: 2026-10-13", {"a": "yes", "b": "on", "c": "2026-10-13"}),
        ("a: true\nb: false\nc: null", {"a": True, "b": False, "c": None}),
        ('a: "true"\nb: 1e3\nc: -2.5', {"a": "true", "b": 1000.0, "c": -2.5}),
    ],
)
def test_yaml_json_scalars(raw, expected):
    assert parse(raw) == expected


@pytest.mark.parametrize(
    "raw",
    [
        "a: .nan",
        "a: 1e999",
        "1: value",
        "a: !!python/object:os.system {}",
        "a: &x {}",
        "a: *x",
        "<<: {}",
        'a: "\\uD800"',
        "[a, b]: value",
    ],
)
def test_unsafe_yaml_rejected(raw):
    with pytest.raises(InputError):
        parse(raw)


def test_json_duplicate_keys_rejected():
    with pytest.raises(InputError, match="Duplicate"):
        parse('{"format":"gdd/0.2","format":"gdd/99"}')


def test_json_compatible_builtin_yaml_tags():
    assert parse('a: !!str true\nb: !!int "3"\nc: !!bool false\nd: !!float 2') == {
        "a": "true",
        "b": 3,
        "c": False,
        "d": 2.0,
    }


@pytest.mark.parametrize("raw", ['a: !!int "no"', "a: !!bool yes", "a: !!str []"])
def test_invalid_builtin_yaml_tags(raw):
    with pytest.raises(InputError):
        parse(raw)


def test_backend_closure_is_exact_and_readiness_passes():
    path = ROOT / "examples/concert.gdd.yaml"
    bundle = load_bundle(path)
    closure = context_closure(bundle, ["action.reserve"])
    expected = set(bundle.model["nodes"]) - {
        "component.frontend",
        "screen.reserve",
        "community.backend",
        "community.frontend",
    }
    assert closure == expected
    assert len(closure) == 34
    assert (
        sum(e["from"] in closure and e["to"] in closure for e in bundle.model["edges"])
        == 73
    )
    report = validate(path, profiles=["readiness"], scope=["action.reserve"])
    assert aggregate(report["checks"], "readiness") == "PASS"
    assert exit_code(report) == 0
    assert (
        bundle.revision
        == json.loads((ROOT / "examples/concert.snapshot.json").read_text())["revision"]
    )


@pytest.mark.parametrize(
    "change,code",
    [
        (
            lambda d: d["nodes"]["action.reserve"].update(behavior="  "),
            "READINESS_CONTRACT_MISSING",
        ),
        (
            lambda d: d["nodes"]["action.reserve"]["contract"]["risks"].pop(
                "atomicity"
            ),
            "READINESS_CONTRACT_MISSING",
        ),
        (
            lambda d: d["nodes"]["action.reserve"]["contract"]["error_cases"].pop(
                "SoldOut"
            ),
            "SCENARIO_CATEGORY_MISSING",
        ),
        (
            lambda d: d["nodes"]["action.reserve"]["contract"]["risks"][
                "authorization"
            ].update(scenarios=["scenario.reserve"]),
            "SCENARIO_CATEGORY_MISSING",
        ),
        (
            lambda d: d["nodes"]["action.reserve"]["contract"]["output"].update(
                optional=False
            ),
            "TYPE_CONSTRAINT_INVALID",
        ),
        (
            lambda d: d["nodes"]["entity.concert"]["fields"]["capacity"].update(
                minimum=3, maximum=2
            ),
            "TYPE_CONSTRAINT_INVALID",
        ),
        (
            lambda d: d["nodes"]["action.reserve"]["contract"]["output"].update(
                required=["missing"]
            ),
            "TYPE_CONSTRAINT_INVALID",
        ),
        (
            lambda d: d["nodes"]["action.reserve"]["contract"]["output"]["properties"][
                "id"
            ].update(optional=False),
            "TYPE_CONSTRAINT_INVALID",
        ),
        (
            lambda d: d["nodes"]["rule.api_boundary"]["rule"]["constraint"].update(
                relation="magic"
            ),
            "CONSTRAINT_INVALID",
        ),
        (
            lambda d: d["edges"].append(
                {
                    "id": "edge.owner.duplicate",
                    "from": "action.reserve",
                    "relation": "owned_by",
                    "to": "component.reservations",
                }
            ),
            "OWNERSHIP_AMBIGUOUS",
        ),
    ],
)
def test_negative_model_and_readiness_mutations(examples, change, code):
    path = modified(examples, change)
    report = validate(path, profiles=["readiness"], scope=["action.reserve"])
    assert code in codes(report)
    assert exit_code(report) == 1


def test_deep_type_reference_checked(examples):
    def mutate(data):
        data["nodes"]["action.reserve"]["contract"]["output"] = {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "nested": {"type": "reference", "target": "action.cancel"}
                },
            },
        }

    report = validate(modified(examples, mutate))
    assert "ENDPOINT_KIND_INVALID" in codes(report)


@pytest.mark.parametrize(
    "selector",
    [
        {"kind": "json-pointer", "value": "invalid"},
        {"kind": "line-range", "value": "L2-L1"},
    ],
)
def test_edge_source_selectors_checked(examples, selector):
    path = modified(
        examples,
        lambda d: d["edges"][0].update(
            source_refs=[
                {
                    "source": "source.legacy",
                    "purpose": "provenance",
                    "selector": selector,
                }
            ]
        ),
    )
    assert "CONSTRAINT_INVALID" in codes(validate(path))


def test_long_graph_does_not_recurse():
    graph = {str(i): [str(i + 1)] for i in range(3000)}
    assert not has_cycle(graph)
    graph["3000"] = ["0"]
    assert has_cycle(graph)


@pytest.mark.parametrize(
    "data_schema,result",
    [
        ({"properties": []}, "FAIL"),
        ({"$defs": {"x": {"$ref": "https://invalid.example/schema"}}}, "FAIL"),
        ({"$ref": "#/$defs/missing"}, "FAIL"),
        ({"const": {"$ref": "https://example.org/inert"}}, "PASS"),
        ({"type": "object", "properties": {"code": {"type": "string"}}}, "PASS"),
    ],
)
def test_extension_schema_boundaries(examples, data_schema, result):
    path = examples / "extension.gdd.yaml"
    manifest = parse(path.read_bytes())
    extpath = examples / "extensions/sensors.extension.json"
    extension = parse(extpath.read_bytes())
    extension["node_kinds"]["example.sensors.sensor"]["data_schema"] = data_schema
    if "const" in data_schema:
        manifest["nodes"]["sensor.room"]["data"] = data_schema["const"]
    write(extpath, extension)
    manifest["extensions"]["extension.sensors"]["sha256"] = sha256(
        extpath.read_bytes()
    ).hexdigest()
    write(path, manifest)
    assert aggregate(validate(path)["checks"], "model") == result


def test_symlink_escape_rejected(examples, tmp_path):
    outside = tmp_path / "outside.yaml"
    outside.write_text("format: gdd-fragment/0.2\nnodes: {}\nedges: []\n")
    (examples / "escape.yaml").symlink_to(outside)
    path = examples / "fragmented.gdd.yaml"
    manifest = parse(path.read_bytes())
    manifest["includes"] = ["escape.yaml"]
    write(path, manifest)
    assert "ASSET_PATH_INVALID" in codes(validate(path))


def test_records_failure_keeps_model_revision(examples):
    path = examples / "concert.gdd.yaml"
    expected = load_bundle(path).revision
    (examples / "concert.records.yaml").unlink()
    report = validate(path)
    assert report["spec_revision"] == expected
    assert exit_code(report) == 2


def test_parent_context_contains_children():
    bundle = load_bundle(ROOT / "examples/refinement.gdd.yaml")
    assert context_closure(bundle, ["action.checkout"]) == set(bundle.model["nodes"])
    assert "entity.receipt" not in context_closure(bundle, ["action.record"])


def test_complete_context_cannot_bypass_sketch_readiness():
    path = ROOT / "examples/sketch.gdd.yaml"
    bundle = load_bundle(path)
    candidate = HARNESS["package"](bundle, ["action.hello"])
    report = validate(path, profiles=["readiness"], package=candidate)
    assert aggregate(report["checks"], "readiness") == "FAIL"


def test_context_requires_canonical_edges():
    path = ROOT / "examples/concert.gdd.yaml"
    candidate = HARNESS["package"](load_bundle(path), ["action.reserve"])
    candidate["edges"].pop()
    report = validate(path, profiles=["readiness"], package=candidate)
    assert (
        next(c for c in report["checks"] if c["check_id"] == "context.package")[
            "result"
        ]
        == "FAIL"
    )


def test_community_is_not_implementation_scope():
    report = validate(
        ROOT / "examples/concert.gdd.yaml",
        profiles=["readiness"],
        scope=["community.backend"],
    )
    assert aggregate(report["checks"], "readiness") == "FAIL"


def auth_fixture(requirement="explicit"):
    bundle = load_bundle(ROOT / "examples/concert.gdd.yaml")
    host = {
        "policy": {
            "revision": "synthetic-policy",
            "actor": "actor.fixture",
            "activity": "implementation",
            "spec_revision": bundle.revision,
            "scope": [],
            "requirement": requirement,
            "authorities": ["actor.fixture"],
            "basis": "Synthetic test-only mandate",
        },
        "trusted_decisions": ["decision.reject", "decision.accept", "decision.other"],
    }
    rejection = {
        "kind": "decision",
        "topic": "approval",
        "activity": "implementation",
        "spec_revision": bundle.revision,
        "policy_revision": "synthetic-policy",
        "scope": ["action.reserve"],
        "actor": {"id": "actor.fixture", "kind": "human"},
        "outcome": "rejected",
        "rationale": "Synthetic",
    }
    bundle.records.update(
        {
            "decision.reject": rejection,
            "decision.accept": {**rejection, "outcome": "accepted"},
        }
    )
    return bundle, host


@pytest.mark.parametrize("requirement", ["explicit", "standing", "none"])
@pytest.mark.parametrize("trust", [True, False])
def test_rejection_not_hidden_by_unrelated_supersession(requirement, trust):
    bundle, host = auth_fixture(requirement)
    bundle.records["decision.other"] = {
        **bundle.records["decision.accept"],
        "scope": ["action.cancel"],
        "supersedes": "decision.reject",
    }
    if not trust:
        host["trusted_decisions"].remove("decision.other")
    authorize(bundle, "implementation", ["action.reserve"], host)
    assert aggregate(bundle.checks, "authorization") == "DENIED"


def test_compatible_trusted_supersession_replaces_rejection():
    bundle, host = auth_fixture()
    bundle.records.pop("decision.accept")
    bundle.records["decision.other"] = {
        **bundle.records["decision.reject"],
        "outcome": "accepted",
        "supersedes": "decision.reject",
    }
    authorize(bundle, "implementation", ["action.reserve"], host)
    assert aggregate(bundle.checks, "authorization") == "GRANTED"


def test_unknown_scope_never_authorized():
    bundle, host = auth_fixture("standing")
    authorize(bundle, "implementation", ["action.missing"], host)
    assert aggregate(bundle.checks, "authorization") == "UNRESOLVED"


@pytest.mark.parametrize("scope", [["bad/id"], ["Bad"]])
def test_invalid_scope_is_controlled_input_error(scope):
    with pytest.raises(ValueError, match="GDD IDs"):
        validate(ROOT / "examples/concert.gdd.yaml", scope=scope)


@pytest.mark.parametrize(
    "change,expected",
    [
        (lambda b, h: None, "PASS"),
        (lambda b, h: h.update(trusted_evidence=[]), "UNKNOWN"),
        (lambda b, h: h.update(verified_bindings=[]), "UNKNOWN"),
        (
            lambda b, h: b.records["binding.fixture"].update(
                targets=["component.frontend"]
            ),
            "UNKNOWN",
        ),
        (lambda b, h: h.update(negative_controls=[]), "UNKNOWN"),
        (lambda b, h: b.records["evidence.fixture"].update(result="FAIL"), "FAIL"),
    ],
)
def test_evidence_requires_actual_host_verified_inputs(examples, change, expected):
    _, host = HARNESS["evidence_fixture"](examples)
    bundle = load_bundle(examples / "concert.gdd.yaml")
    change(bundle, host)
    evaluate_implementation(bundle, ["action.reserve"], host)
    capacity = next(
        c
        for c in bundle.checks
        if c["dimension"] == "implementation" and c["scope"] == ["rule.capacity"]
    )
    assert capacity["result"] == expected


@pytest.mark.parametrize("trust", [True, False])
def test_other_check_supersession_cannot_hide_failed_evidence(examples, trust):
    _, host = HARNESS["evidence_fixture"](examples)
    bundle = load_bundle(examples / "concert.gdd.yaml")
    bundle.records["evidence.failure"] = {
        **bundle.records["evidence.fixture"],
        "result": "FAIL",
    }
    host["trusted_evidence"].append("evidence.failure")
    bundle.records["evidence.other"] = {
        **bundle.records["evidence.fixture"],
        "check": "check.rule.idempotency",
        "supersedes": "evidence.failure",
        "result": "UNKNOWN",
        "executed": False,
    }
    if trust:
        host["trusted_evidence"].append("evidence.other")
    evaluate_implementation(bundle, ["action.reserve"], host)
    capacity = next(
        c
        for c in bundle.checks
        if c["dimension"] == "implementation" and c["scope"] == ["rule.capacity"]
    )
    assert capacity["result"] == "UNKNOWN"


@pytest.mark.parametrize(
    "format_name,result", [("text/plain", "PASS"), ("unknown-vendor/99", "UNKNOWN")]
)
def test_normative_contract_format_is_known(examples, format_name, result):
    source = examples / "contract.txt"
    source.write_text("Synthetic contract")

    def change(data):
        data["sources"]["source.contract"] = {
            "role": "knowledge",
            "title": "Test",
            "uri": "contract.txt",
            "format": format_name,
            "normative": True,
            "revision": {"sha256": sha256(source.read_bytes()).hexdigest()},
        }
        data["nodes"]["action.reserve"]["source_refs"].append(
            {
                "source": "source.contract",
                "purpose": "contract",
                "selector": {"kind": "whole-document"},
            }
        )

    report = validate(
        modified(examples, change), profiles=["readiness"], scope=["action.reserve"]
    )
    assert aggregate(report["checks"], "readiness") == result


def test_design_frontmatter_pointer_resolves(examples):
    source = examples / "DESIGN.md"
    source.write_text(
        "---\ncomponents:\n  button-primary:\n    color: blue\n---\n# Components\nPreserved prose\n"
    )

    def change(data):
        data["sources"]["source.design"].update(
            normative=True, revision={"sha256": sha256(source.read_bytes()).hexdigest()}
        )
        data["nodes"]["action.reserve"]["source_refs"].append(
            {
                "source": "source.design",
                "purpose": "contract",
                "selector": {
                    "kind": "json-pointer",
                    "value": "/components/button-primary",
                },
            }
        )

    report = validate(
        modified(examples, change), profiles=["readiness"], scope=["action.reserve"]
    )
    assert aggregate(report["checks"], "readiness") == "PASS"


def parallel_fixture(bundle):
    return {
        "format": "gdd-parallel-plan/0.2",
        "packages": [
            {
                "id": f"package.{i}",
                "scope": [s],
                "write_resources": [f"src/{i}.py"],
                "read_resources": ["src/contracts.py"],
                "locked_contracts": ["action.reserve", "action.cancel"],
                "workspace": f"workspace.{i}",
                "base_revision": bundle.revision,
                "integration_plan": "Integrate against declared checks",
            }
            for i, s in enumerate(["action.reserve", "action.cancel"])
        ],
        "serialized": [],
        "integration": {
            "owner": "actor.integration",
            "checks": [i for i, r in bundle.records.items() if r["kind"] == "check"],
        },
    }


def test_parallel_complete_plan_passes_and_normalized_overlap_fails():
    path = ROOT / "examples/concert.gdd.yaml"
    plan = parallel_fixture(load_bundle(path))
    assert (
        aggregate(
            validate(path, profiles=["readiness"], parallel_plan=plan)["checks"],
            "readiness",
        )
        == "PASS"
    )
    plan["packages"][0]["write_resources"] = ["src/a/../contracts.py"]
    plan["packages"][1]["write_resources"] = ["src/contracts.py"]
    assert "WRITE_OWNERSHIP_CONFLICT" in codes(
        validate(path, profiles=["readiness"], parallel_plan=plan)
    )
    plan["serialized"] = [["package.0", "package.1"]]
    assert (
        aggregate(
            validate(path, profiles=["readiness"], parallel_plan=plan)["checks"],
            "readiness",
        )
        == "PASS"
    )


@pytest.mark.parametrize(
    "profile,scope,expected",
    [("model", [], 0), ("readiness", ["screen.reserve"], 1), ("records", [], 2)],
)
def test_cli_exit_codes_and_report_schema(profile, scope, expected):
    command = [
        sys.executable,
        "-m",
        "gdd_validator",
        "validate",
        str(ROOT / "examples/concert.gdd.yaml"),
        "--profile",
        profile,
    ]
    for item in scope:
        command += ["--scope", item]
    result = subprocess.run(command, capture_output=True, text=True)
    assert result.returncode == expected, result.stderr
    assert json.loads(result.stdout)["format"] == "gdd-report/0.2"


def test_explicit_records_file_already_declared_is_loaded_once():
    report = validate(
        ROOT / "examples/concert.records.yaml",
        model_path=ROOT / "examples/concert.gdd.yaml",
        profiles=["records"],
    )
    assert "ID_COLLISION" not in codes(report)
    assert aggregate(report["checks"], "implementation") == "UNKNOWN"


def test_schema_pass_is_not_model_pass_for_standalone_records():
    path = ROOT / "examples/concert.records.yaml"
    assert (
        aggregate(validate(path, profiles=["representation"])["checks"], "model")
        == "PASS"
    )
    assert aggregate(validate(path)["checks"], "model") == "UNKNOWN"


def test_invalid_host_shape_is_not_silently_ignored():
    with pytest.raises(ValueError, match="Invalid host"):
        validate(ROOT / "examples/concert.gdd.yaml", host_context=[])


def test_missing_format_is_malformed_not_an_unknown_version(tmp_path):
    path = tmp_path / "gdd.yaml"
    path.write_text("nodes: {}\nedges: []\n")
    report = validate(path, profiles=["representation"])
    assert aggregate(report["checks"], "model") == "FAIL"
    assert "SCHEMA_INVALID" in codes(report)


def test_generated_context_includes_source_lineage(examples):
    def change(data):
        data["sources"]["source.upstream"] = {
            "role": "input",
            "title": "Upstream",
            "uri": "upstream.txt",
            "format": "text/plain",
            "normative": False,
            "revision": {"sha256": "0" * 64},
        }
        data["sources"]["source.legacy"]["derived_from"] = [
            {"source": "source.upstream", "revision": {"sha256": "0" * 64}}
        ]

    path = modified(examples, change)
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "gdd_validator",
            "context",
            str(path),
            "--scope",
            "action.reserve",
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    candidate = json.loads(result.stdout)
    assert "source.upstream" in candidate["sources"]
    assert (
        aggregate(
            validate(path, profiles=["readiness"], package=candidate)["checks"],
            "readiness",
        )
        == "PASS"
    )
