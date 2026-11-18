use std::fs;
use std::path::Path;

use gdd::bundle;
use gdd::report::{CheckResult, Diagnostic, Report};
use gdd::validation::{Profile, validate};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn manifest(format: &str) -> Value {
    json!({
        "format": format,
        "project": {"id": "example.model", "title": "Model", "goal": "Check model semantics."},
        "nodes": {},
        "edges": [],
    })
}

fn check(data: &Value) -> Report {
    check_raw(&data.to_string())
}

fn check_raw(data: &str) -> Report {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("gdd.json");
    fs::write(&path, data).unwrap();
    validate(&bundle::load(&path), Profile::Model)
}

fn diagnostic<'a>(report: &'a Report, code: &str) -> &'a Diagnostic {
    report
        .checks
        .iter()
        .flat_map(|check| &check.diagnostics)
        .find(|item| item.code == code)
        .unwrap_or_else(|| panic!("expected {code} in {report:?}"))
}

#[test]
fn enforces_boundaries_only_in_0_3_and_preserves_sketches() {
    let mut model = manifest("gdd/0.2");
    model["nodes"] = json!({
        "component.caller": {"kind": "component", "description": "Caller."},
        "component.provider": {"kind": "component", "description": "Provider.", "interfaces": []},
        "action.internal": {"kind": "action", "description": "Internal action."},
    });
    model["edges"] = json!([
        {"id": "edge.owner", "relation": "owned_by", "from": "action.internal", "to": "component.provider"},
        {"id": "edge.call", "relation": "calls", "from": "component.caller", "to": "action.internal"},
    ]);
    assert_eq!(check(&model).result("model"), CheckResult::Pass);

    model["format"] = json!("gdd/0.3");
    let issue = check(&model);
    let private = diagnostic(&issue, "PRIVATE_ACTION_CALL");
    assert_eq!(private.location.pointer, "/edges/1");
    assert_eq!(
        private.related_ids,
        ["component.caller", "action.internal", "component.provider"]
    );

    model["nodes"]["component.provider"]
        .as_object_mut()
        .unwrap()
        .remove("interfaces");
    assert_eq!(check(&model).result("model"), CheckResult::Pass);
    model["nodes"]["component.caller"]["interfaces"] = json!(["action.internal"]);
    let issue = check(&model);
    assert_eq!(
        diagnostic(&issue, "INTERFACE_OWNER_MISMATCH")
            .location
            .pointer,
        "/nodes/component.caller/interfaces/0"
    );

    model["nodes"]["rule.design"] = json!({
        "kind": "rule",
        "description": "Advisory principle.",
        "rule": {
            "category": "design", "principle": "kiss", "blocking": true,
            "statement": "Check the design.", "scope": "global", "verification": ["review"],
        },
    });
    let issue = check(&model);
    assert_eq!(
        diagnostic(&issue, "DESIGN_PRINCIPLE_INVALID")
            .location
            .pointer,
        "/nodes/rule.design/rule/principle"
    );
}

#[test]
fn compares_numeric_intervals_and_integer_inhabitance_exactly() {
    for (kind, min, max, nullable, result) in [
        (
            "integer",
            "9007199254740992.1",
            "9007199254740992.9",
            false,
            CheckResult::Fail,
        ),
        (
            "integer",
            "-9007199254740992.9",
            "-9007199254740992.1",
            false,
            CheckResult::Fail,
        ),
        ("integer", "-0.5", "-0e0", false, CheckResult::Pass),
        ("integer", "1e-17", "9.999e-1", true, CheckResult::Pass),
        (
            "number",
            "9007199254740992.000000000000000002",
            "9007199254740992.000000000000000001",
            false,
            CheckResult::Fail,
        ),
        ("number", "1e10001", "2e10001", false, CheckResult::Unknown),
    ] {
        let mut model = manifest("gdd/0.3");
        model["nodes"]["entity.values"] = json!({
            "kind": "entity", "description": "Exact numeric field.",
            "fields": {"value": {"type": kind, "minimum": "MIN", "maximum": "MAX", "nullable": nullable}},
        });
        let raw = model
            .to_string()
            .replace("\"MIN\"", min)
            .replace("\"MAX\"", max);
        let report = check_raw(&raw);
        assert_eq!(report.result("model"), result, "{raw}: {report:?}");
        let code = match result {
            CheckResult::Pass => continue,
            CheckResult::Fail => "TYPE_CONSTRAINT_INVALID",
            CheckResult::Unknown => "FORMAT_UNSUPPORTED",
        };
        assert_eq!(
            diagnostic(&report, code).location.pointer,
            "/nodes/entity.values/fields/value"
        );
    }
}

#[test]
fn preserves_semantic_diagnostic_locations_in_fragments() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("gdd.json");
    let mut model = manifest("gdd/0.3");
    model["includes"] = json!(["graph.fragment.json"]);
    fs::write(&path, model.to_string()).unwrap();
    let fragment = json!({
        "format": "gdd-fragment/0.3",
        "nodes": {"action.known": {"kind": "action", "description": "Action."}},
        "edges": [
            {"id": "edge.calls", "relation": "calls", "from": "action.known", "to": "action.missing"},
        ],
    });
    fs::write(
        directory.path().join("graph.fragment.json"),
        fragment.to_string(),
    )
    .unwrap();
    let bundle = bundle::load(&path);
    assert_eq!(
        validate(&bundle, Profile::Representation).result("model"),
        CheckResult::Pass
    );
    let report = validate(&bundle, Profile::Model);
    assert_eq!(report.result("model"), CheckResult::Fail);
    let issue = diagnostic(&report, "REFERENCE_UNRESOLVED");
    assert_eq!(issue.location.file.as_deref(), Some("graph.fragment.json"));
    assert_eq!(issue.location.pointer, "/edges/0/to");
}

#[test]
fn transitive_import_rules_detect_actual_edges_and_cycles() {
    let mut model = manifest("gdd/0.3");
    model["nodes"] = json!({
        "component.a": {"kind": "component", "description": "A."},
        "component.b": {"kind": "component", "description": "B."},
        "rule.import": {
            "kind": "rule", "description": "Import policy.",
            "rule": {
                "category": "architecture", "statement": "Forbid a cycle.", "scope": "global",
                "blocking": true, "verification": ["analysis"],
                "constraint": {
                    "relation": "imports", "from": "component.a", "to": "component.a",
                    "reach": "transitive", "policy": "forbid",
                },
            },
        },
    });
    model["edges"] = json!([
        {"id": "edge.import.forward", "relation": "imports", "from": "component.a", "to": "component.b"},
    ]);
    assert_eq!(check(&model).result("model"), CheckResult::Pass);
    model["edges"].as_array_mut().unwrap().push(json!({
        "id": "edge.import.back", "relation": "imports", "from": "component.b", "to": "component.a",
    }));
    let report = check(&model);
    assert_eq!(
        diagnostic(&report, "CONSTRAINT_INVALID").location.pointer,
        "/nodes/rule.import/rule/constraint"
    );
}

#[test]
fn lineage_checks_source_and_revision_and_handles_valueless_selectors() {
    let mut model = manifest("gdd/0.3");
    model["nodes"]["entity.selected"] = json!({
        "kind": "entity", "description": "Reference a whole source.",
        "source_refs": [{"purpose": "provenance", "source": "source.a", "selector": {"kind": "whole-document"}}],
    });
    model["sources"] = json!({
        "source.a": {
            "role": "knowledge", "title": "A", "uri": "a.md", "format": "text/markdown", "normative": false,
            "revision": {"producer_revision": "a.1"},
            "derived_from": [{"source": "source.b", "revision": {"producer_revision": "b.1"}}],
        },
        "source.b": {
            "role": "knowledge", "title": "B", "uri": "b.md", "format": "text/markdown", "normative": false,
            "revision": {"producer_revision": "b.1"},
        },
    });
    assert_eq!(check(&model).result("model"), CheckResult::Pass);
    model["nodes"]["entity.selected"]["source_refs"][0]["selector"] =
        json!({"kind": "json-pointer", "value": "/bad~3"});
    let report = check(&model);
    assert_eq!(
        diagnostic(&report, "CONSTRAINT_INVALID").location.pointer,
        "/nodes/entity.selected/source_refs/0"
    );

    model["nodes"]["entity.selected"]["source_refs"][0]["selector"] =
        json!({"kind": "opaque", "value": "selector"});
    model["sources"]["source.b"]["derived_from"] = json!([{
        "source": "source.a", "revision": {"producer_revision": "older.a"},
    }]);
    let historical = check(&model);
    assert_eq!(historical.result("model"), CheckResult::Unknown);
    assert_eq!(
        diagnostic(&historical, "REVISION_UNRESOLVED")
            .location
            .pointer,
        "/sources/source.b/derived_from/0"
    );
    model["sources"]["source.b"]["derived_from"][0]["revision"]["producer_revision"] = json!("a.1");
    assert_eq!(check(&model).result("model"), CheckResult::Fail);
    diagnostic(&check(&model), "SOURCE_DERIVATION_CYCLE");
}

#[test]
fn validates_existing_extension_and_record_examples() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let extension = validate(
        &bundle::load(&examples.join("product/greenfield.gdd.json")),
        Profile::Model,
    );
    assert_eq!(
        extension.result("model"),
        CheckResult::Pass,
        "{extension:?}"
    );
    let records = validate(
        &bundle::load(&examples.join("concert-0.3.gdd.yaml")),
        Profile::Model,
    );
    assert_eq!(records.result("model"), CheckResult::Pass, "{records:?}");
    assert!(
        records
            .checks
            .iter()
            .any(|item| item.check_id == "records.integrity")
    );
}

fn extension_report(schema: Value, data: Value) -> Report {
    let directory = tempfile::tempdir().unwrap();
    let extension = json!({
        "format": "gdd-extension/0.2", "namespace": "example.test", "version": "1.0.0",
        "node_kinds": {"example.test.widget": {"description": "Widget.", "data_schema": schema}},
        "relations": {},
    })
    .to_string();
    fs::write(directory.path().join("extension.json"), &extension).unwrap();
    let mut model = manifest("gdd/0.3");
    model["extensions"] = json!({"extension.test": {
        "namespace": "example.test", "version": "1.0.0", "path": "extension.json",
        "sha256": format!("{:x}", Sha256::digest(extension.as_bytes())),
    }});
    model["nodes"] = json!({"node.widget": {"kind": "example.test.widget", "description": "Widget.", "data": data}});
    let path = directory.path().join("gdd.json");
    fs::write(&path, model.to_string()).unwrap();
    validate(&bundle::load(&path), Profile::Model)
}

#[test]
fn guards_every_local_reference_target_but_preserves_inert_data() {
    for reference in ["$ref", "$dynamicRef"] {
        for hidden in [
            json!({"$schema": "http://json-schema.org/draft-07/schema#", "type": "object"}),
            json!({"$ref": "https://example.test/remote"}),
            json!({"$ref": "file:///tmp/remote.json"}),
            json!({"$id": "https://example.test/mutable", "type": "object"}),
            json!({"x-executable": "Executable hook", "type": "object"}),
            json!({"type": 23}),
        ] {
            let report = extension_report(
                json!({"hidden": hidden, (reference): "#/hidden"}),
                json!({}),
            );
            assert_eq!(report.result("model"), CheckResult::Fail, "{report:?}");
            assert_eq!(
                diagnostic(&report, "EXTENSION_DATA_INVALID")
                    .location
                    .file
                    .as_deref(),
                Some("extension.json")
            );
        }
    }
    let inert = json!({"$ref":"https://example.test/inert", "$schema":"other", "$id":"inert", "x-executable":"an inert string"});
    for (schema, data) in [
        (json!({"const": inert}), inert),
        (json!({"hidden": true, "$ref": "#/hidden"}), json!({})),
        (
            json!({"$defs": {"x": {"$anchor": "value", "type": "object"}}, "$ref": "#value"}),
            json!({}),
        ),
        (
            json!({"$defs": {"x": {"$dynamicAnchor": "value", "type": "object"}}, "$dynamicRef": "#value"}),
            json!({}),
        ),
        (
            json!({"properties": {"date": {"format": "date-time"}}}),
            json!({"date":"invalid date"}),
        ),
        (
            json!({"properties": {"child": {"$ref": "#"}}}),
            json!({"child": {"child": {}}}),
        ),
        (
            json!({"$defs": {"unused": {"$ref": "#/$defs/unused"}}}),
            json!({}),
        ),
    ] {
        let report = extension_report(schema, data);
        assert_eq!(report.result("model"), CheckResult::Pass, "{report:?}");
    }

    for schema in [json!({"$ref": "#"}), json!({"allOf": [{"$ref": "#"}]})] {
        let report = extension_report(schema, json!({}));
        assert_eq!(report.result("model"), CheckResult::Unknown, "{report:?}");
        diagnostic(&report, "EXTENSION_UNSUPPORTED");
    }
    let report = extension_report(json!({"$ref":"#/absent"}), json!({}));
    diagnostic(&report, "EXTENSION_DATA_INVALID");
    let report = extension_report(
        json!({"hidden": {"properties": {"data": {"const": 9}}}, "$ref":"#/hidden"}),
        json!({"data": 10}),
    );
    assert_eq!(
        diagnostic(&report, "EXTENSION_DATA_INVALID")
            .location
            .pointer,
        "/nodes/node.widget/data/data"
    );
}

#[test]
fn representation_defers_model_collisions_but_rejects_fragment_overrides() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("gdd.json");
    let mut model = manifest("gdd/0.3");
    model["nodes"] = json!({"node.one": {"kind":"component", "description":"One."}});
    model["edges"] = json!([
        {"id":"node.one", "relation":"imports", "from":"node.one", "to":"node.one"},
    ]);
    fs::write(&path, model.to_string()).unwrap();
    let bundle = bundle::load(&path);
    assert_eq!(
        validate(&bundle, Profile::Representation).result("model"),
        CheckResult::Pass
    );
    diagnostic(&validate(&bundle, Profile::Model), "ID_COLLISION");

    let fragment = json!({"format":"gdd-fragment/0.3", "nodes": model["nodes"], "edges":[]});
    fs::write(directory.path().join("fragment.json"), fragment.to_string()).unwrap();
    model["includes"] = json!(["fragment.json"]);
    fs::write(&path, model.to_string()).unwrap();
    let representation = validate(&bundle::load(&path), Profile::Representation);
    assert_eq!(
        diagnostic(&representation, "ID_COLLISION")
            .location
            .file
            .as_deref(),
        Some("fragment.json")
    );
}

#[test]
fn checks_record_revisions_hashes_supersession_and_question_resolution() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("gdd.json");
    let ledger = directory.path().join("records.json");
    let mut model = manifest("gdd/0.3");
    model["record_files"] = json!(["records.json"]);
    model["nodes"] = json!({
        "action.a": {"kind":"action", "description":"Action A."},
        "action.b": {"kind":"action", "description":"Action B."},
    });
    model["questions"] = json!({"question.test": {
        "statement":"Is the check planned?", "status":"resolved", "blocks":["implementation"],
        "targets":["action.a"], "resolution":{"decision":"decision.test"},
    }});
    fs::write(&path, model.to_string()).unwrap();
    let revision = bundle::load(&path).snapshot().unwrap().revision.clone();
    let canonical = format!(
        r#"{{"description":"Crème 🦀","kind":"check","method":"test","negative_controls":[],"required_environment":[],"scope":["action.a","action.b"],"spec_revision":"{revision}"}}"#
    );
    let digest = format!("sha256:{:x}", Sha256::digest(canonical.as_bytes()));
    let mut records = json!({
        "format":"gdd-records/0.2", "project_id":"example.model",
        "records": {
            "check.test": {
                "kind":"check", "spec_revision":revision, "scope":["action.a","action.b"],
                "method":"test", "description":"Crème 🦀", "required_environment":[], "negative_controls":[],
            },
            "evidence.test": {
                "kind":"evidence", "check":"check.test", "check_revision":digest,
                "implementation":"implementation.test", "spec_revision":revision, "code_revision":"current",
                "checker":{"name":"fixture","version":"1"}, "scope":["action.a","action.b"],
                "result":"PASS", "executed":true, "reason":"Synthetic evidence.", "environment":{},
                "evidence_uri":"fixture://evidence",
            },
            "decision.test": {
                "kind":"decision", "topic":"model_change", "scope":["action.a"], "spec_revision":revision,
                "policy_revision":revision, "actor":{"id":"human.test","kind":"human"},
                "outcome":"accepted", "rationale":"Synthetic decision.", "checks":["check.test"],
            },
        },
    });
    let check_records = |records: &Value| {
        fs::write(&ledger, records.to_string()).unwrap();
        validate(&bundle::load(&path), Profile::Model)
    };
    assert_eq!(check_records(&records).result("model"), CheckResult::Pass);

    records["records"]["check.test"]["method"] = json!("changed");
    let mismatch = check_records(&records);
    let issue = diagnostic(&mismatch, "CHECK_REVISION_MISMATCH");
    assert_eq!(issue.location.file.as_deref(), Some("records.json"));
    assert_eq!(
        issue.location.pointer,
        "/records/evidence.test/check_revision"
    );
    records["records"]["check.test"]["method"] = json!("test");

    records["records"]["decision.test"]["scope"] = json!(["action.absent"]);
    let invalid_target = check_records(&records);
    assert_eq!(invalid_target.result("model"), CheckResult::Fail);
    assert_eq!(
        diagnostic(&invalid_target, "RECORD_INVALID")
            .location
            .pointer,
        "/records/decision.test/scope/0"
    );
    records["records"]["decision.test"]["spec_revision"] =
        json!(format!("sha256:{}", "0".repeat(64)));
    let historical = check_records(&records);
    assert_eq!(historical.result("model"), CheckResult::Unknown);
    assert_eq!(
        diagnostic(&historical, "REVISION_UNRESOLVED")
            .location
            .pointer,
        "/records/decision.test/spec_revision"
    );

    records["records"]["decision.test"]["supersedes"] = json!("decision.test");
    let cycle = check_records(&records);
    assert_eq!(cycle.result("model"), CheckResult::Fail);
    assert!(
        diagnostic(&cycle, "RECORD_INVALID")
            .message
            .contains("supersession")
    );
    records["records"]
        .as_object_mut()
        .unwrap()
        .remove("decision.test");
    let missing = check_records(&records);
    assert_eq!(
        diagnostic(&missing, "RECORD_INVALID").location.pointer,
        "/questions/question.test/resolution/decision"
    );
}
