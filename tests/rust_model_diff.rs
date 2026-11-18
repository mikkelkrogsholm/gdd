use gdd::bundle::load_model_only;
use gdd::model_diff::{ModelDiff, diff_models, diff_models_from_bundles, diff_text};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::Command;

fn tutorial() -> Value {
    serde_json::from_str(include_str!("../docs/tutorial/refined.gdd.json")).unwrap()
}

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn check(report: &ModelDiff) -> Value {
    let json = serde_json::to_value(report).unwrap();
    let schema = serde_json::from_str(include_str!("../schemas/model-diff.schema.json")).unwrap();
    gdd::validation::build_schema(&schema)
        .unwrap()
        .validate(&json)
        .unwrap();
    json
}

#[test]
fn coalesces_decimal_spellings_and_distinguishes_types_in_objects_and_arrays() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.json");
    let head = dir.path().join("head.json");
    let mut model = tutorial();
    model["nodes"]["action.add"]["annotations"] = json!({"test.values":{"quote~/key":["NUMBER"]}});
    let text = serde_json::to_string(&model).unwrap();
    for (a, b, equal) in [
        ("1.00", "10e-1", true),
        ("1", "1.0", false),
        ("true", "1", false),
        ("-0.0", "0.0", false),
        ("-0", "0", true),
        ("1e-4000", "1000e-4003", true),
        ("1.00000000000000000001", "1.00000000000000000002", false),
        ("1E+09223372036854775809", "10.0e9223372036854775808", true),
    ] {
        fs::write(&base, text.replace("\"NUMBER\"", a)).unwrap();
        fs::write(&head, text.replace("\"NUMBER\"", b)).unwrap();
        let report = diff_models(&base, &head).unwrap();
        assert_eq!(report.exit_code, 0, "{a}: {:?}", report.diagnostics);
        assert_eq!(!report.comparison.semantic_changes, equal, "{a} {b}");
        if !equal {
            assert_eq!(report.changes.len(), 1);
            let field = &report.changes[0].fields[0];
            assert_eq!(field.pointer, "/annotations/test.values/quote~0~1key");
            assert_eq!(field.change, "changed");
            assert!(field.before.as_ref().unwrap().is_array());
        }
    }
}

#[test]
fn relocations_and_extension_packaging_are_independent_of_metadata_and_fields() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.json");
    let head = dir.path().join("head.json");
    let ext = json!({"format":"gdd-extension/0.2","namespace":"example.diff","version":"1.0.0",
        "node_kinds":{},"relations":{}});
    let mut left = tutorial();
    let mut right = left.clone();
    for (doc, name, raw) in [
        (&mut left, "left.json", ext.to_string()),
        (
            &mut right,
            "right.json",
            serde_json::to_string_pretty(&ext).unwrap(),
        ),
    ] {
        doc["extensions"] = json!({"extension.diff":{"namespace":"example.diff","version":"1.0.0","path":name,
            "sha256":format!("{:x}",Sha256::digest(raw.as_bytes()))}});
        fs::write(dir.path().join(name), raw).unwrap();
    }
    let node = right["nodes"]
        .as_object_mut()
        .unwrap()
        .remove("action.add")
        .unwrap();
    let edges = right["edges"]
        .as_array_mut()
        .unwrap()
        .drain(..)
        .rev()
        .collect::<Vec<_>>();
    let fragment = json!({"format":"gdd-fragment/0.3","nodes":{"action.add":node},"edges":edges});
    write(&dir.path().join("part.json"), &fragment);
    right["includes"] = json!(["./part.json"]);
    write(&base, &left);
    write(&head, &right);
    let original = diff_models(&base, &head).unwrap();
    assert_eq!(original.exit_code, 0);
    assert!(!original.comparison.semantic_changes);
    assert_ne!(original.base.spec_revision, original.head.spec_revision);
    assert_eq!(original.relocated.len(), 3);
    assert_eq!(
        original.relocated[0].head.as_ref().unwrap()[0],
        "./part.json"
    );
    assert!(diff_text(&original).contains("['./part.json', '/nodes/action.add']"));
    let before = load_model_only(&head);
    right["project"]["goal"] = json!("A changed goal.");
    write(&head, &right);
    let after = load_model_only(&head);
    fs::remove_file(head).unwrap();
    let from_captures = diff_models_from_bundles(&before, &after).unwrap();
    assert!(from_captures.changes.is_empty() && from_captures.relocated.is_empty());
    assert_eq!(from_captures.metadata_changes[0].pointer, "/project/goal");
    assert_eq!(from_captures.base.path, from_captures.head.path);
}

#[test]
fn partial_graph_uses_accepted_siblings_and_suppresses_absence_claims() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.json");
    let head = dir.path().join("head.json");
    let left = tutorial();
    write(&base, &left);
    let mut right = left.clone();
    right["includes"] = json!(["invalid.json", "valid.json"]);
    right["project"]["title"] = json!("Partial.");
    right["nodes"]
        .as_object_mut()
        .unwrap()
        .remove("scenario.add");
    let mut moved = right["nodes"]
        .as_object_mut()
        .unwrap()
        .remove("action.add")
        .unwrap();
    moved["behavior"] = json!("Changed in the usable fragment.");
    let bad = json!({"format":"gdd-fragment/0.3","nodes":{"action.add":{}},"edges":[]});
    write(&dir.path().join("invalid.json"), &bad);
    write(
        &dir.path().join("valid.json"),
        &json!({"format":"gdd-fragment/0.3","nodes":{"action.add":moved},"edges":[]}),
    );
    write(&head, &right);
    let partial = diff_models(&base, &head).unwrap();
    assert_eq!(partial.comparison.coverage, "partial");
    assert_eq!(partial.exit_code, 1);
    assert!(partial.metadata_changes.is_empty());
    assert!(partial.diagnostics.iter().any(|d| {
        d.diagnostics
            .iter()
            .any(|i| i.code == "REFERENCE_UNRESOLVED")
    }));
    assert_eq!(partial.changes.len(), 1);
    assert_eq!(partial.changes[0].fields[0].pointer, "/behavior");
    assert!(partial.head.spec_revision.is_none());
    check(&partial);
    fs::remove_file(head).unwrap();
    let unavailable = diff_models(&base, &dir.path().join("absent")).unwrap();
    assert_eq!(unavailable.exit_code, 2);
    assert_eq!(unavailable.comparison.coverage, "unavailable");
    assert!(unavailable.changes.is_empty());
}

#[test]
fn emits_ordered_structural_advisories_but_allows_complete_invalid_comparisons() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.json");
    let head = dir.path().join("head.json");
    let mut left = tutorial();
    left["nodes"]["rule.policy"] = json!({"kind":"rule","description":"Review imports.","rule":{
        "category":"architecture","statement":"Forbid.","scope":"global","blocking":true,"verification":["analysis"],
        "constraint":{"relation":"imports","from":"component.arithmetic","to":"component.arithmetic","reach":"transitive","policy":"forbid"}}});
    left["edges"].as_array_mut().unwrap().push(
        json!({"id":"edge.verify","relation":"verifies","from":"scenario.add","to":"rule.policy"}),
    );
    let mut right = left.clone();
    right["edges"].as_array_mut().unwrap().pop();
    right["edges"][0]["to"] = json!("missing");
    right["nodes"]["component.arithmetic"]["interfaces"] = json!([]);
    right["nodes"]["action.add"]["contract"]["postconditions"] = json!([]);
    right["nodes"]["rule.policy"]["rule"]["blocking"] = json!(false);
    right["nodes"]["rule.policy"]["rule"]["constraint"]["reach"] = json!("direct");
    write(&base, &left);
    write(&head, &right);
    let result = diff_models(&base, &head).unwrap();
    assert_eq!(result.comparison.coverage, "complete");
    assert_eq!(result.exit_code, 1);
    assert_eq!(
        result.advisories.iter().map(|a| a.code).collect::<Vec<_>>(),
        [
            "CONTRACT_REQUIREMENT_REMOVED",
            "INTERFACE_SHRUNK",
            "BLOCKING_RULE_RELAXED",
            "FORBID_REACH_NARROWED",
            "LAST_VERIFIER_REMOVED"
        ]
    );
    assert_eq!(
        result.comparison.counts["edges"],
        json!({"removed":1,"added":0,"changed":1})
    );
    check(&result);
}

#[test]
fn cli_preserves_input_labels_and_refuses_existing_destinations() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("model.json");
    write(&manifest, &tutorial());
    let alias = dir.path().join("another.json");
    fs::hard_link(&manifest, &alias).unwrap();
    let hard = diff_models(&manifest, &alias).unwrap();
    assert_ne!(hard.base.spec_revision, hard.head.spec_revision);
    assert!(!hard.comparison.semantic_changes);
    #[cfg(unix)]
    {
        let link = dir.path().join("alias.json");
        std::os::unix::fs::symlink(&manifest, &link).unwrap();
        let same = diff_models(&manifest, &link).unwrap();
        assert_eq!(same.base.spec_revision, same.head.spec_revision);
        assert_ne!(same.base.path, same.head.path);
    }
    let output = dir.path().join("diff.txt");
    let invoke = || {
        Command::new(env!("CARGO_BIN_EXE_gdd"))
            .arg("diff")
            .arg(&manifest)
            .arg(&alias)
            .args(["--format", "text", "--output"])
            .arg(&output)
            .output()
            .unwrap()
    };
    let first = invoke();
    assert!(first.status.success() && first.stdout.is_empty());
    assert_eq!(fs::read_to_string(&output).unwrap(), diff_text(&hard));
    assert!(!invoke().status.success());
    assert_eq!(fs::read_to_string(output).unwrap(), diff_text(&hard));
}

#[test]
fn child_format_checks_distinguish_missing_known_and_unsupported_formats() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.json");
    let head = dir.path().join("head.json");
    let fragment = dir.path().join("fragment.json");
    let mut model = tutorial();
    write(&base, &model);
    model["includes"] = json!(["fragment.json"]);
    write(&head, &model);
    for (child, expected_id, expected_code) in [
        (json!({}), "representation.format", "SCHEMA_INVALID"),
        (
            json!({"format":42}),
            "representation.format",
            "SCHEMA_INVALID",
        ),
        (
            json!({"format":"gdd-fragment/0.2"}),
            "representation.schema",
            "SCHEMA_INVALID",
        ),
        (
            json!({"format":"future/1.0"}),
            "representation.format",
            "FORMAT_UNSUPPORTED",
        ),
    ] {
        write(&fragment, &child);
        let result = diff_models(&base, &head).unwrap();
        assert_eq!(result.comparison.coverage, "partial");
        let check = result
            .head
            .checks
            .iter()
            .find(|c| {
                c.diagnostics.iter().any(|d| {
                    d.location.file.as_deref() == Some("fragment.json")
                        && d.location.pointer == "/format"
                })
            })
            .unwrap();
        assert_eq!(check.check_id, expected_id);
        assert_eq!(check.diagnostics[0].code, expected_code);
    }
}
