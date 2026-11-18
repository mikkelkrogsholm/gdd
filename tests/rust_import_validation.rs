use std::{fs, path::Path, process::Command};

use gdd::{
    bundle,
    graphify::{self, AST_PROFILE},
    input,
    report::CheckResult,
    validation::{
        GraphifyValidationOptions, Profile, ValidationOptions, validate_captured,
        validate_paths_with_graphify,
    },
};
use serde_json::Value;

const IMPORT: Profile = Profile::ImportGraphify;
const RAW: &[u8] =
    br#"{"nodes":[{"id":"n","label":"Node","file_type":"code","source_file":"a.rs"}],"edges":[]}"#;

#[test]
fn captured_and_file_reports_share_order_scope_and_revision() {
    let dir = tempfile::tempdir().unwrap();
    let graph = dir.path().join("raw.json");
    let model = dir.path().join("gdd.json");
    fs::write(&graph, RAW).unwrap();
    fs::write(&model, br#"{"format":"gdd/0.3","project":{"id":"demo","title":"Demo","goal":"Assess observations."},"nodes":{},"edges":[]}"#).unwrap();
    let audit = input::parse_json(
        r#"{"format":"gdd-import-audit/0.2","original":[1,2],"reconstructed":[2,1]}"#,
    )
    .unwrap();
    let scope = [
        "action.z".to_owned(),
        "action.a".to_owned(),
        "action.z".to_owned(),
    ];
    let profiles = [IMPORT, Profile::Representation, IMPORT];
    let options = || ValidationOptions {
        profiles: &profiles,
        scope: &scope,
        ..Default::default()
    };
    let files = validate_paths_with_graphify(
        &model,
        None,
        options(),
        GraphifyValidationOptions {
            path: Some(&graph),
            profile: Some(AST_PROFILE),
            import_audit: Some(&audit),
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let captured = bundle::load_model_only(&model);
    let observations = graphify::verify_graphify(&graph, Some(AST_PROFILE), None).unwrap();
    dir.close().unwrap();
    let report = validate_captured(
        Some(&captured),
        Some(&observations),
        options(),
        Some(&audit),
        None,
    )
    .unwrap();
    assert_eq!(report.profiles, [IMPORT, Profile::Representation]);
    assert_eq!(report.scope, ["action.z", "action.a"]);
    assert_eq!(
        report.spec_revision.as_deref(),
        Some(captured.snapshot().unwrap().revision.as_str())
    );
    assert_eq!(
        serde_json::to_value(&files).unwrap(),
        serde_json::to_value(&report).unwrap()
    );
    assert_eq!(report.result("model"), CheckResult::Pass);
    let loss = report
        .checks
        .iter()
        .position(|c| c.check_id == "integration.loss")
        .unwrap();
    assert_eq!(report.checks[loss].scope, report.scope);
    assert_eq!(report.checks[loss + 1].check_id, "graphify.raw");
    assert!(report.checks[loss + 1].scope.is_empty());
    assert_eq!(report.exit_code(), 1);

    let standalone = validate_captured(
        None,
        Some(&observations),
        ValidationOptions {
            profiles: &[IMPORT, IMPORT],
            scope: &scope,
            ..Default::default()
        },
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(standalone).unwrap(),
        serde_json::to_value(observations).unwrap()
    );
}

#[test]
fn audit_preserves_exact_numeric_types_and_json_structure() {
    let (_, observation) = graphify::observe_graphify(
        RAW,
        graphify::GraphifyOptions {
            profile: Some(AST_PROFILE),
            ..Default::default()
        },
    )
    .unwrap();
    for (left, right, same) in [
        (
            r#"{"a":0,"b":[true,null]}"#,
            r#"{"b":[true,null],"a":-0}"#,
            true,
        ),
        ("1", "1.0", false),
        ("false", "0", false),
        ("0.00", "-0e-99", false),
        ("-0.0", "-0e9", true),
        ("[1,2]", "[2,1]", false),
        ("1e-4000", "10.000e-4001", true),
        ("1e999999999999999999999", "10e999999999999999999998", true),
        (
            "0.1234567890123456789001",
            "0.1234567890123456789002",
            false,
        ),
    ] {
        let audit = input::parse_json(&format!(
            r#"{{"format":"gdd-import-audit/0.2","original":{left},"reconstructed":{right}}}"#
        ))
        .unwrap();
        let report = validate_captured(
            None,
            Some(&observation),
            ValidationOptions {
                profiles: &[IMPORT],
                ..Default::default()
            },
            Some(&audit),
            None,
        )
        .unwrap();
        assert_eq!(
            report.exit_code(),
            if same { 0 } else { 1 },
            "{left} and {right}"
        );
        if !same {
            let loss = report.checks.last().unwrap();
            assert_eq!(loss.check_id, "integration.loss");
            assert_eq!(loss.diagnostics[0].code, "IMPORT_LOSS_DETECTED");
            assert!(loss.scope.is_empty());
        }
    }
}

#[test]
fn envelopes_and_scope_are_checked_before_standalone_dispatch() {
    let path = Path::new("examples/graphify/observations");
    let invalid = input::parse_json(r#"{"original":1}"#).unwrap();
    for profiles in [&[IMPORT][..], &[Profile::Model]] {
        let err = validate_paths_with_graphify(
            path,
            None,
            ValidationOptions {
                profiles,
                ..Default::default()
            },
            GraphifyValidationOptions {
                import_audit: Some(&invalid),
                ..Default::default()
            },
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("Invalid import input"));
    }
    let scope = ["Bad ID".to_owned()];
    assert!(
        validate_paths_with_graphify(
            path,
            None,
            ValidationOptions {
                profiles: &[IMPORT],
                scope: &scope,
                ..Default::default()
            },
            Default::default(),
            None
        )
        .is_err()
    );
}

#[test]
fn model_bound_cli_reports_missing_observations_and_semantic_checks() {
    let cli = Command::new(env!("CARGO_BIN_EXE_gdd"))
        .args([
            "validate",
            "templates/project/records.json",
            "--model",
            "templates/project/gdd.json",
            "--profile",
            "import.graphify",
            "--scope",
            "action.main",
        ])
        .output()
        .unwrap();
    assert_eq!(cli.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(report["tool"]["name"], "gdd");
    assert!(report["spec_revision"].is_string());
    let checks = report["checks"].as_array().unwrap();
    assert!(
        checks
            .iter()
            .any(|c| c["check_id"].as_str().unwrap().starts_with("model."))
    );
    let missing = checks.last().unwrap();
    assert_eq!(missing["check_id"], "integration.graphify");
    assert_eq!(missing["result"], "UNKNOWN");
    assert_eq!(missing["scope"][0], "action.main");
}
