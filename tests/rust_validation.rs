use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gdd::report::CheckResult;
use gdd::validation::{Profile, build_schema, to_json, validate};
use gdd::{bundle, input};
use serde_json::{Value, json};

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

fn cli(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gdd"))
        .arg("validate")
        .arg(path)
        .args(args)
        .output()
        .unwrap()
}

fn read_report(output: &Output, exit_code: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(exit_code),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let schema = serde_json::from_str(include_str!("../schemas/report.schema.json")).unwrap();
    let value = serde_json::from_slice(&output.stdout).unwrap();
    build_schema(&schema).unwrap().validate(&value).unwrap();
    value
}

#[test]
fn validates_all_standalone_schema_kinds_and_loaded_bundles() {
    let directory = tempfile::tempdir().unwrap();
    let fragment_03 = directory.path().join("fragment.json");
    fs::write(
        &fragment_03,
        r#"{"format":"gdd-fragment/0.3","nodes":{},"edges":[]}"#,
    )
    .unwrap();

    for (path, revision_known) in [
        (example("concert.gdd.yaml"), true),
        (example("concert-0.3.gdd.yaml"), true),
        (example("fragmented.gdd.yaml"), true),
        (example("extension.gdd.yaml"), true),
        (example("note.fragment.yaml"), false),
        (fragment_03, false),
        (example("extensions/sensors.extension.json"), false),
        (example("concert.records.yaml"), false),
        (example("illustrative.report.json"), false),
        (example("concert.snapshot.json"), false),
    ] {
        let value = read_report(&cli(&path, &["--profile", "representation"]), 0);
        assert_eq!(!value["spec_revision"].is_null(), revision_known);
        assert_eq!(value["profiles"], json!(["representation"]));
    }
}

#[test]
fn validates_original_documents_without_filesystem_access() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("gdd.yaml");
    fs::copy(example("sketch.gdd.yaml"), &path).unwrap();
    let bundle = bundle::load(&path);
    fs::remove_file(path).unwrap();

    let report = validate(&bundle, Profile::Representation);
    assert_eq!(report.result("model"), CheckResult::Pass);
    assert_eq!(
        report.spec_revision.as_deref(),
        Some(bundle.snapshot().unwrap().revision.as_str())
    );
}

#[test]
fn unsupported_and_overriding_extensions_keep_their_document_location() {
    let path = example("extension.gdd.yaml");
    let unsupported = read_report(
        &cli(
            &path,
            &[
                "--profile",
                "representation",
                "--unsupported-extension",
                "example.sensors",
            ],
        ),
        2,
    );
    assert!(unsupported["spec_revision"].is_string());
    let diagnostic = &unsupported["checks"].as_array().unwrap().last().unwrap()["diagnostics"][0];
    assert_eq!(diagnostic["code"], "EXTENSION_UNSUPPORTED");
    assert_eq!(
        diagnostic["location"]["file"],
        "extensions/sensors.extension.json"
    );

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("extension.json");
    let mut artifact: Value = serde_json::from_str(include_str!(
        "../examples/extensions/sensors.extension.json"
    ))
    .unwrap();
    let relations = artifact["relations"].as_object_mut().unwrap();
    let relation = relations.remove("example.sensors.measures").unwrap();
    relations.insert("calls".into(), relation);
    fs::write(&path, serde_json::to_vec(&artifact).unwrap()).unwrap();
    let overriding = read_report(&cli(&path, &["--profile", "representation"]), 1);
    let diagnostic = overriding["checks"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| diagnostic["code"] == "EXTENSION_OVERRIDE")
        .unwrap();
    assert_eq!(
        diagnostic["location"],
        json!({"file": "extension.json", "pointer": "/relations/calls"})
    );
}

#[test]
fn checks_exact_numbers_and_blocks_external_schema_resolution() {
    let schema = build_schema(&json!({"type": "integer"})).unwrap();
    for (literal, is_integer) in [
        ("99999999999999999999999999999999999999999", true),
        ("9007199254740992.000000000000000001", false),
        ("1.00000000000000000000", true),
    ] {
        let original = input::parse(literal.as_bytes()).unwrap();
        assert_eq!(schema.is_valid(&to_json(&original).unwrap()), is_integer);
    }
    assert!(build_schema(&json!({"$ref": "https://example.test/remote.json"})).is_err());
    assert!(build_schema(&json!({"$ref": "file:///tmp/gdd-schema.json"})).is_err());
}

#[test]
fn distinguishes_schema_failure_and_unavailable_input() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.json");
    let schema_arguments = &["--profile", "representation"];
    let unavailable = read_report(&cli(&path, schema_arguments), 2);
    assert_eq!(
        unavailable["checks"][0]["diagnostics"][0]["code"],
        "SOURCE_UNAVAILABLE"
    );

    let mut artifact: Value =
        serde_json::from_str(include_str!("../examples/illustrative.report.json")).unwrap();
    artifact["tool"]["version"] = json!(false);
    fs::write(&path, serde_json::to_vec(&artifact).unwrap()).unwrap();
    let invalid = read_report(&cli(&path, schema_arguments), 1);
    assert_eq!(
        invalid["checks"][0]["diagnostics"][0]["location"],
        json!({"file": "report.json", "pointer": "/tool/version"})
    );
    assert_eq!(
        invalid["checks"][0]["diagnostics"][0]["code"],
        "SCHEMA_INVALID"
    );

    fs::write(&path, "format: gdd/0.3\nformat: gdd/0.2\n").unwrap();
    let duplicate = read_report(&cli(&path, schema_arguments), 1);
    assert_eq!(
        duplicate["checks"][0]["diagnostics"][0]["code"],
        "YAML_DUPLICATE_KEY"
    );
}

#[test]
fn checks_record_date_time_against_the_schema() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("records.json");
    let revision = format!("sha256:{}", "0".repeat(64));
    let mut artifact = json!({
        "format": "gdd-records/0.2",
        "project_id": "example.test",
        "records": {
            "decision.example": {
                "kind": "decision", "topic": "approval", "scope": ["action.test"],
                "spec_revision": revision, "policy_revision": revision,
                "actor": {"id": "actor.fixture", "kind": "human"},
                "outcome": "accepted", "activity": "implementation", "rationale": "For testing."
            }
        }
    });
    for (date, code) in [("2026-11-18T10:00:00Z", 0), ("2026-02-31T10:00:00Z", 1)] {
        artifact["records"]["decision.example"]["expires_at"] = date.into();
        fs::write(&path, serde_json::to_vec(&artifact).unwrap()).unwrap();
        read_report(&cli(&path, &["--profile", "representation"]), code);
    }
}

#[test]
fn reports_profiles_and_scope_and_writes_output_exclusively() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("report.json");
    let path = example("concert.records.yaml");

    let default = read_report(&cli(&path, &[]), 2);
    assert_eq!(default["profiles"], json!(["model"]));
    let args = [
        "--profile",
        "representation",
        "--profile",
        "model",
        "--profile",
        "representation",
        "--scope",
        "action.second",
        "--scope",
        "action.first",
        "--scope",
        "action.second",
    ];
    let stdout = cli(&path, &args);
    let report = read_report(&stdout, 2);
    assert_eq!(report["profiles"], json!(["representation", "model"]));
    assert_eq!(report["scope"], json!(["action.second", "action.first"]));

    let output_args = [
        args.as_slice(),
        &["--output", destination.to_str().unwrap()],
    ]
    .concat();
    let output = cli(&path, &output_args);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(&destination).unwrap(), stdout.stdout);
    let duplicate = cli(&path, &output_args);
    assert_eq!(duplicate.status.code(), Some(1));
    assert_eq!(fs::read(&destination).unwrap(), stdout.stdout);
}
