use std::fs;
use std::path::Path;
use std::process::Command;

use gdd::bundle;
use gdd::impact::{
    ImpactItem, ImpactReport, analyze_impact, analyze_impact_from_bundle, impact_text,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn tutorial() -> Value {
    serde_json::from_str(include_str!("../docs/tutorial/refined.gdd.json")).unwrap()
}

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn edge(id: &str, from: &str, relation: &str, to: &str) -> Value {
    json!({"id": id, "from": from, "to": to, "relation": relation})
}

fn assess(path: &Path, scope: &[&str], implementation: Option<&str>) -> ImpactReport {
    let scope: Vec<String> = scope.iter().map(|s| s.to_string()).collect();
    analyze_impact(path, &scope, implementation).unwrap()
}

fn item<'a>(report: &'a ImpactReport, root: &str, id: &str) -> &'a ImpactItem {
    report
        .roots
        .iter()
        .find(|r| r.id == root)
        .unwrap()
        .items
        .iter()
        .find(|i| i.id == id)
        .unwrap()
}

fn cli(path: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gdd"))
        .arg("impact")
        .arg(path)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn keeps_consumer_and_support_witnesses_deterministic_across_cycles_and_ties() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let mut doc = tutorial();
    for name in ["caller", "outer", "child", "outside"] {
        doc["nodes"][format!("action.{name}")] = json!({"kind":"action","description":name});
    }
    doc["edges"].as_array_mut().unwrap().extend([
        edge(
            "edge.provider",
            "action.add",
            "depends_on",
            "component.arithmetic",
        ),
        edge("edge.z", "action.caller", "calls", "action.add"),
        edge("edge.a", "action.caller", "depends_on", "action.add"),
        edge("edge.outer", "action.outer", "calls", "action.caller"),
        edge("edge.cycle", "action.caller", "calls", "action.outer"),
        edge("edge.child", "action.child", "refines", "action.caller"),
        edge(
            "edge.sibling",
            "action.outside",
            "owned_by",
            "component.arithmetic",
        ),
    ]);
    write(&path, &doc);
    let root = "component.arithmetic";
    let report = assess(&path, &[root, "action.add", root], None);
    assert_eq!(report.exit_code, 0, "{:?}", report.diagnostics);
    assert_eq!(report.scope, ["action.add", "component.arithmetic"]);
    assert_eq!(item(&report, root, root).category, "selected");
    assert_eq!(item(&report, root, "action.add").category, "direct");
    assert_eq!(item(&report, root, "action.outer").category, "transitive");
    assert_eq!(
        item(&report, root, "action.outer")
            .witness
            .iter()
            .map(|s| s.edge.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["edge.provider", "edge.a", "edge.outer"]
    );
    let child = item(&report, root, "action.child");
    assert_eq!(child.category, "supporting");
    assert_eq!(
        child.witness.last().unwrap().derivation,
        "initial-refinement"
    );
    // The public interface and the reverse dependency give equally long routes
    // to this scenario. The reference projection prefers the named edge.
    let scenario = item(&report, root, "scenario.add");
    assert_eq!(scenario.category, "supporting");
    assert_eq!(scenario.witness[0].edge.as_deref(), Some("edge.provider"));
    assert_eq!(scenario.witness[1].relation.as_deref(), Some("exercises"));
    assert!(
        report
            .roots
            .iter()
            .flat_map(|r| &r.items)
            .all(|i| i.id != "action.outside")
    );
    assert!(report.boundaries.is_some());
    let before = serde_json::to_value(report).unwrap();
    doc["edges"].as_array_mut().unwrap().reverse();
    write(&path, &doc);
    let after = serde_json::to_value(assess(&path, &["action.add", root], None)).unwrap();
    assert_eq!(before["roots"], after["roots"]);
    assert_ne!(before["spec_revision"], after["spec_revision"]);
}

#[test]
fn propagates_declared_data_navigation_machine_and_type_dependencies_in_reverse() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let mut doc = tutorial();
    doc["nodes"]["entity.input"] = json!({"kind":"entity","description":"Input."});
    doc["nodes"]["action.add"]["contract"]["input"]["left"] =
        json!({"type":"array","items":{"type":"reference","target":"entity.input"}});
    doc["nodes"]["machine.work"] = json!({"kind":"machine","description":"Lifecycle.","machine":{
    "states":["open","done"],"initial":"open","terminal":["done"],"transitions":[
        {"id":"transition.done","from":"open","to":"done","trigger":"action.add","guard":"Ready.","effects":"Finish."}
    ]}});
    for role in ["reads", "creates", "changes"] {
        doc["nodes"][format!("action.{role}")] = json!({"kind":"action","description":role});
        doc["edges"].as_array_mut().unwrap().push(edge(
            &format!("edge.{role}"),
            &format!("action.{role}"),
            role,
            "entity.input",
        ));
    }
    for id in ["screen.entry", "screen.destination"] {
        doc["nodes"][id] = json!({"kind":"screen","description":id});
    }
    doc["edges"].as_array_mut().unwrap().extend([
        edge(
            "edge.navigation",
            "screen.entry",
            "navigates_to",
            "screen.destination",
        ),
        edge("edge.state", "machine.work", "state_of", "entity.input"),
    ]);
    write(&path, &doc);
    let report = assess(
        &path,
        &[
            "entity.input",
            "screen.destination",
            "action.creates",
            "action.add",
        ],
        None,
    );
    assert_eq!(report.exit_code, 0, "{:?}", report.diagnostics);
    for id in [
        "action.reads",
        "action.creates",
        "action.changes",
        "machine.work",
    ] {
        assert_eq!(item(&report, "entity.input", id).category, "direct");
    }
    assert_eq!(
        item(&report, "entity.input", "action.add").witness[0].derivation,
        "type-reference-consumer"
    );
    assert_eq!(
        item(&report, "action.add", "machine.work").witness[0].derivation,
        "machine-trigger-consumer"
    );
    assert_eq!(
        item(&report, "screen.destination", "screen.entry").category,
        "direct"
    );
    let writer = &report
        .roots
        .iter()
        .find(|r| r.id == "action.creates")
        .unwrap()
        .items;
    assert_eq!(
        item(&report, "action.creates", "entity.input").category,
        "supporting"
    );
    assert!(!writer.iter().any(|i| i.id == "action.reads"));
    assert!(
        report
            .coverage
            .unspecified_declarations
            .contains(&"action.creates".into())
    );
}

#[test]
fn only_extension_dependencies_propagate_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let ext_path = dir.path().join("extension.json");
    let mut doc = tutorial();
    let mut extension = json!({"format":"gdd-extension/0.2","namespace":"custom.links","version":"1.0.0","node_kinds":{},"relations":{}});
    for (label, direction, effect, dependency) in [
        ("read", "both", "read", false),
        ("consume", "none", "none", true),
    ] {
        let name = format!("custom.links.{label}");
        extension["relations"][&name] = json!({"description":name,"from":["action"],"to":["action"],"context":direction,
            "dependency":dependency,"effect":effect,"data_schema":{"type":"object"}});
        let consumer = format!("action.{label}");
        doc["nodes"][&consumer] = json!({"kind":"action","description":label});
        let mut relation = edge(&format!("edge.{label}"), &consumer, &name, "action.add");
        relation["data"] = json!({});
        doc["edges"].as_array_mut().unwrap().push(relation);
    }
    let bytes = serde_json::to_vec(&extension).unwrap();
    doc["extensions"] = json!({"extension.custom":{"namespace":"custom.links","version":"1.0.0","path":"extension.json",
        "sha256":format!("{:x}",Sha256::digest(&bytes))}});
    fs::write(&ext_path, bytes).unwrap();
    write(&path, &doc);
    let report = assess(&path, &["action.add"], None);
    assert_eq!(report.exit_code, 0, "{:?}", report.diagnostics);
    assert_eq!(
        item(&report, "action.add", "action.read").category,
        "supporting"
    );
    assert_eq!(
        item(&report, "action.add", "action.consume").category,
        "direct"
    );
    fs::remove_file(ext_path).unwrap();
    let unavailable = assess(&path, &["action.add"], None);
    assert_eq!(unavailable.exit_code, 2);
    assert_eq!(unavailable.coverage.result, "unavailable");
    assert_eq!(
        unavailable
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>(),
        ["SOURCE_UNAVAILABLE", "EXTENSION_UNSUPPORTED"]
    );
}

#[test]
fn recorded_candidates_require_current_revision_and_compatible_check_retirement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let ledger = dir.path().join("records.json");
    let mut doc = tutorial();
    doc["record_files"] = json!(["records.json"]);
    write(&path, &doc);
    let mut records =
        json!({"format":"gdd-records/0.2","project_id":doc["project"]["id"],"records":{}});
    write(&ledger, &records);
    let revision = bundle::load_model_only(&path)
        .snapshot()
        .unwrap()
        .revision
        .clone();
    let old = json!({"kind":"check","scope":["action.add","scenario.add"],"spec_revision":revision,
        "method":"unit_test","required_environment":[],"negative_controls":[]});
    let binding = json!({"kind":"binding","targets":["action.add"],"spec_revision":revision,
        "implementation":"impl.demo","role":"code","resource":"café.py","selector":{"kind":"whole-document"},"status":"planned"});
    records["records"] = json!({"check.old":old,"check.new":old,"binding.first":binding,"binding.new":binding,"binding.past":binding,"binding.foreign":binding});
    records["records"]["check.new"]["supersedes"] = json!("check.old");
    records["records"]["binding.new"]["supersedes"] = json!("binding.first");
    records["records"]["binding.foreign"]["implementation"] = json!("impl.foreign");
    records["records"]["binding.past"]["spec_revision"] =
        json!(format!("sha256:{}", "0".repeat(64)));
    for case in ["compatible", "method", "narrower", "invalid"] {
        let mut current = records.clone();
        match case {
            "method" => current["records"]["check.new"]["method"] = json!("lint"),
            "narrower" => current["records"]["check.new"]["scope"] = json!(["scenario.add"]),
            "invalid" => {
                current["records"]["check.new"]["scope"] =
                    json!(["action.add", "scenario.add", "missing"])
            }
            _ => {}
        }
        write(&ledger, &current);
        let report = assess(&path, &["action.add", "scenario.add"], Some("impl.demo"));
        assert_eq!(report.exit_code, 0, "{:?}", report.diagnostics);
        let ids: Vec<_> = report.records.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids.contains(&"check.new"), case != "invalid");
        assert_eq!(ids.contains(&"check.old"), case != "compatible");
        assert!(ids.contains(&"binding.first") && ids.contains(&"binding.new"));
        let found = &report.records[0];
        assert_eq!(found.id, "binding.first");
        assert_eq!(found.declaration, binding);
        let mut canonical = binding.clone();
        canonical.sort_all_objects();
        assert_eq!(
            found.record_revision,
            format!(
                "sha256:{:x}",
                Sha256::digest(serde_json::to_vec(&canonical).unwrap())
            )
        );
        assert_eq!(
            found.roots.iter().map(String::as_str).collect::<Vec<_>>(),
            ["action.add", "scenario.add"]
        );
        assert_eq!(report.record_coverage.code_bindings, "present");
        let reason = |id| {
            report
                .records_omitted
                .iter()
                .find(|r| r.id == id)
                .unwrap()
                .reason
        };
        assert_eq!(reason("binding.foreign"), "Different implementation.");
        assert!(reason("binding.past").contains("Historical revision"));
        if case == "compatible" {
            assert!(reason("check.old").contains("supersession"));
        }
    }
    let captured = bundle::load_model_only(&path);
    let before = assess(&path, &["action.add"], None);
    fs::remove_file(path).unwrap();
    fs::remove_file(ledger).unwrap();
    let after = analyze_impact_from_bundle(&captured, &["action.add".into()], None).unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
}

#[test]
fn invalid_scope_partial_models_and_unavailable_documents_keep_distinct_verdicts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    for (scope, implementation, code) in [
        (vec![], None, "IMPACT_SCOPE_INVALID"),
        (vec!["action.add\n"], None, "IMPACT_SCOPE_INVALID"),
        (
            vec!["action.add"],
            Some("bad id"),
            "IMPACT_IMPLEMENTATION_INVALID",
        ),
    ] {
        let report = assess(&path, &scope, implementation);
        assert_eq!(report.exit_code, 1);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, code);
        assert!(report.spec_revision.is_none());
    }
    let mut doc = tutorial();
    doc["nodes"]["action.add"]["contract"]["dependencies_complete"] = json!(false);
    write(&path, &doc);
    let report = assess(&path, &["action.add"], None);
    assert_eq!(report.exit_code, 2);
    assert_eq!(report.coverage.result, "partial");
    assert_eq!(report.diagnostics[0].code, "IMPACT_DEPENDENCIES_INCOMPLETE");
    let mixed = assess(&path, &["edge.add.owner", "action.add"], None);
    assert_eq!(mixed.exit_code, 1);
    assert_eq!(mixed.roots.len(), 1);
    assert_eq!(mixed.roots[0].id, "action.add");
    doc["edges"][0]["to"] = json!("unknown");
    write(&path, &doc);
    let failed = assess(&path, &["unknown"], None);
    assert_eq!(failed.coverage.result, "unavailable");
    assert!(failed.roots.is_empty() && failed.boundaries.is_none());
    assert_eq!(failed.record_coverage.code_bindings, "unassessed");
    assert!(
        failed
            .diagnostics
            .iter()
            .any(|d| d.code == "IMPACT_SCOPE_UNKNOWN")
    );
    assert!(
        failed
            .diagnostics
            .iter()
            .any(|d| d.code == "REFERENCE_UNRESOLVED")
    );
    let mut doc = tutorial();
    doc["record_files"] = json!(["bad.json"]);
    write(&path, &doc);
    write(
        &dir.path().join("bad.json"),
        &json!({"format":"gdd-records/0.2","project_id":"wrong","records":[]}),
    );
    let captured = bundle::load(&path);
    let from_capture = analyze_impact_from_bundle(&captured, &["action.add".into()], None).unwrap();
    let rejected = assess(&path, &["action.add"], None);
    assert_eq!(rejected.coverage.result, "unavailable");
    assert!(
        rejected
            .diagnostics
            .iter()
            .all(|d| d.code == "SCHEMA_INVALID")
    );
    assert_eq!(
        serde_json::to_value(from_capture).unwrap(),
        serde_json::to_value(rejected).unwrap()
    );
}

#[test]
fn cli_text_json_schema_and_exclusive_output_share_the_same_report() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/fragmented.gdd.yaml");
    let report = assess(&path, &["entity.note"], None);
    assert!(report.boundaries.is_none());
    assert_eq!(report.exit_code, 0);
    let value = serde_json::to_value(&report).unwrap();
    let schema = serde_json::from_str(include_str!("../schemas/impact.schema.json")).unwrap();
    gdd::validation::build_schema(&schema)
        .unwrap()
        .validate(&value)
        .unwrap();
    let json = cli(&path, &["--scope", "entity.note", "--scope", "entity.note"]);
    assert!(json.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&json.stdout).unwrap(),
        value
    );
    let expected = "Impact: complete (exit 0; declared model; runtime callers unassessed)\nScope: entity.note\nSeed entity.note:\n  supporting: component.notes — Established supporting read context; no change propagation inferred. [entity.note -> component.notes]\n    forward: edge.note.owner (owned_by)\n  selected: entity.note — Selected change seed. [entity.note]\nCode bindings: absent (declared records only)\n";
    assert_eq!(impact_text(&report), expected);
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("impact.txt");
    let args = [
        "--scope",
        "entity.note",
        "--format",
        "text",
        "--output",
        output.to_str().unwrap(),
    ];
    let first = cli(&path, &args);
    assert!(first.status.success() && first.stdout.is_empty());
    assert_eq!(fs::read_to_string(&output).unwrap(), expected);
    assert!(!cli(&path, &args).status.success());
    assert_eq!(fs::read_to_string(output).unwrap(), expected);
    assert_eq!(cli(&path, &[]).status.code(), Some(2));
}

#[test]
fn selector_ordinal_digests_preserve_integer_and_decimal_identity() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let ledger = dir.path().join("ledger.json");
    let mut doc = tutorial();
    doc["record_files"] = json!(["ledger.json"]);
    write(&path, &doc);
    let mut records =
        json!({"format":"gdd-records/0.2","project_id":doc["project"]["id"],"records":{}});
    write(&ledger, &records);
    let revision = bundle::load_model_only(&path)
        .snapshot()
        .unwrap()
        .revision
        .clone();
    records["records"]["binding.candidate"] = json!({"kind":"binding","spec_revision":revision,"targets":["action.add"],
        "implementation":"impl.demo","role":"code","resource":"module.py","status":"planned",
        "selector":{"kind":"markdown-heading","value":"Heading","ordinal":"NUMBER"}});
    let raw = records.to_string();
    let hash = |number: &str| {
        fs::write(&ledger, raw.replace("\"NUMBER\"", number)).unwrap();
        let result = assess(&path, &["action.add"], None);
        assert_eq!(result.exit_code, 0, "{:?}", result.diagnostics);
        result.records[0].record_revision.clone()
    };
    assert_ne!(hash("1"), hash("1.0"));
    for spelling in ["1.00", "1e0", "10e-1"] {
        assert_eq!(hash(spelling), hash("1.0"));
    }
}
