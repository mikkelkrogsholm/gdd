use std::{fs, path::Path, process::Command};

use gdd::bundle::{self, Bundle};
use gdd::input;
use gdd::report::{CheckResult, Report};
use gdd::validation::{self, Activity, Profile, ValidationOptions};
use serde_json::{Value, json};

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn tiny_model() -> Value {
    json!({"format":"gdd/0.3","project":{"id":"demo","title":"Review","goal":"Inspect the ledger."},"nodes":{},"edges":[]})
}

fn ledger(project: &str) -> Value {
    json!({"format":"gdd-records/0.2","project_id":project,"records":{}})
}

fn assess(model: &Path, secondary: &Path) -> Report {
    validation::validate_paths(
        secondary,
        Some(model),
        ValidationOptions {
            profiles: &[Profile::Readiness, Profile::Records],
            activity: Some(Activity::Implementation),
            ..Default::default()
        },
        None,
    )
    .unwrap()
}

fn codes(report: &Report) -> Vec<&str> {
    report
        .checks
        .iter()
        .flat_map(|c| &c.diagnostics)
        .map(|d| d.code)
        .collect()
}

#[test]
fn external_evidence_combines_with_the_declared_plan_on_one_capture() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/agent-native");
    for name in ["model.gdd.json", "REQUIREMENTS.md"] {
        fs::copy(example.join(name), root.path().join(name)).unwrap();
    }
    let model = root.path().join("model.gdd.json");
    let declared = root.path().join("model.records.json");
    let secondary = other.path().join("observations.json");
    let mut plans: Value =
        serde_json::from_slice(&fs::read(example.join("model.records.json")).unwrap()).unwrap();
    let mut observations = ledger("quote-demo");
    observations["records"]["evidence.demo.final"] = plans["records"]
        .as_object_mut()
        .unwrap()
        .remove("evidence.demo.final")
        .unwrap();
    write(&declared, &plans);
    write(&secondary, &observations);
    let host_path = example.join("host.acceptance.json");
    let host = input::parse(&fs::read(&host_path).unwrap()).unwrap();
    let scope = ["action.quote".to_owned()];
    let records = |b: &Bundle| {
        validation::validate_with_options(
            b,
            ValidationOptions {
                profiles: &[Profile::Records],
                host_context: Some(&host),
                scope: &scope,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let captured = bundle::load_with_records(&model, &secondary);
    let result = records(&captured);
    assert_eq!(result.exit_code(), 0);
    assert_eq!(result.result("implementation"), CheckResult::Pass);
    assert_eq!(
        result.spec_revision,
        records(&bundle::load_model_only(&model)).spec_revision
    );
    assert!(
        captured
            .primary_documents()
            .iter()
            .all(|d| d.asset.path != "observations.json")
    );
    assert_eq!(
        captured.documents().last().unwrap().asset.path,
        "observations.json"
    );

    let package_path = root.path().join("package.json");
    write(
        &package_path,
        &serde_json::to_value(gdd::context::context_manifest(&captured, &scope).unwrap()).unwrap(),
    );
    let cli = Command::new(env!("CARGO_BIN_EXE_gdd"))
        .arg("validate")
        .arg(&secondary)
        .arg("--model")
        .arg(&model)
        .args(["--profile", "records", "--package"])
        .arg(package_path)
        .arg("--host-context")
        .arg(host_path)
        .output()
        .unwrap();
    assert_eq!(cli.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<Value>(&cli.stdout).unwrap(),
        serde_json::to_value(&result).unwrap()
    );
    fs::remove_file(&secondary).unwrap();
    fs::remove_file(&declared).unwrap();
    assert_eq!(
        serde_json::to_value(records(&captured)).unwrap(),
        serde_json::to_value(&result).unwrap()
    );
    assert_eq!(
        records(&bundle::load_with_records(&model, &secondary)).exit_code(),
        2
    );
}

#[test]
fn competing_ledger_ids_and_project_ids_are_reported_without_changing_the_revision() {
    let root = tempfile::tempdir().unwrap();
    let model = root.path().join("model.json");
    let declared = root.path().join("declared.json");
    let secondary = root.path().join("extra.json");
    let mut primary = tiny_model();
    primary["record_files"] = json!(["declared.json"]);
    write(&model, &primary);
    write(&declared, &ledger("demo"));
    let revision = bundle::load_model_only(&model)
        .snapshot()
        .unwrap()
        .revision
        .clone();
    let check = json!({"kind":"check","method":"review","scope":["entity.x"],"spec_revision":revision,"required_environment":[],"negative_controls":[]});
    let mut one = ledger("demo");
    one["records"]["check.x"] = check.clone();
    write(&declared, &one);
    let mut two = ledger("other");
    two["records"]["check.x"] = check;
    write(&secondary, &two);
    let report = assess(&model, &secondary);
    assert_eq!(report.spec_revision.as_deref(), Some(revision.as_str()));
    assert_eq!(report.result("model"), CheckResult::Fail);
    assert_eq!(report.result("authorization"), CheckResult::Unresolved);
    assert!(codes(&report).contains(&"ID_COLLISION"));
    assert!(codes(&report).contains(&"RECORD_INVALID"));
    assert!(
        report
            .checks
            .iter()
            .any(|c| c.reason == "Secondary records belong to another project.")
    );
}

#[test]
fn unavailable_inputs_are_checked_independently_and_never_supply_the_other_root() {
    let root = tempfile::tempdir().unwrap();
    let model = root.path().join("primary.json");
    let secondary = root.path().join("secondary.json");
    let unsupported = json!({"format":"gdd/99.0","project":{"id":"untrusted"}});
    write(&secondary, &ledger("other"));
    let missing = assess(&model, &secondary);
    assert_eq!(missing.exit_code(), 2);
    assert!(missing.spec_revision.is_none());
    assert!(!codes(&missing).contains(&"RECORD_INVALID"));
    write(&model, &unsupported);
    let unknown = assess(&model, &secondary);
    assert_eq!(unknown.exit_code(), 2);
    assert!(!codes(&unknown).contains(&"RECORD_INVALID"));
    let check = unknown
        .checks
        .iter()
        .find(|c| c.check_id == "representation.format")
        .unwrap();
    assert_eq!(check.reason, "Format is not supported.");
    assert_eq!(check.diagnostics[0].message, "gdd/99.0");
    assert_eq!(
        check.diagnostics[0].location.file.as_deref(),
        Some("primary.json")
    );
    write(&secondary, &tiny_model());
    let wrong = assess(&model, &secondary);
    assert_eq!(wrong.exit_code(), 1);
    assert!(wrong.spec_revision.is_none());
    assert!(
        wrong
            .checks
            .iter()
            .any(|c| c.check_id == "model.unavailable")
    );
    assert!(codes(&wrong).contains(&"SCHEMA_INVALID"));

    let mut primary = tiny_model();
    primary["includes"] = json!(["absent-fragment.json"]);
    write(&model, &primary);
    write(&secondary, &ledger("other"));
    let mismatch = assess(&model, &secondary);
    assert_eq!(mismatch.exit_code(), 1);
    assert!(codes(&mismatch).contains(&"RECORD_INVALID"));
    primary.as_object_mut().unwrap().remove("nodes");
    write(&model, &primary);
    let invalid = assess(&model, &secondary);
    assert_eq!(invalid.exit_code(), 1);
    assert!(!codes(&invalid).contains(&"RECORD_INVALID"));
    assert!(!codes(&invalid).contains(&"SOURCE_UNAVAILABLE"));

    write(&model, &tiny_model());
    write(&secondary, &unsupported);
    let unknown = assess(&model, &secondary);
    let check = unknown
        .checks
        .iter()
        .find(|c| c.check_id == "representation.format")
        .unwrap();
    assert_eq!(
        check.diagnostics[0].location.file.as_deref(),
        Some("secondary.json")
    );
    assert_eq!(unknown.exit_code(), 2);
    assert!(unknown.spec_revision.is_some());
    fs::write(
        &secondary,
        format!("{}0{}", "[".repeat(300), "]".repeat(300)),
    )
    .unwrap();
    let deep = assess(&model, &secondary);
    assert_eq!(deep.exit_code(), 2);
    assert!(
        deep.checks
            .iter()
            .any(|c| c.check_id == "check.format_unsupported")
    );
    assert!(
        !deep
            .checks
            .iter()
            .any(|c| c.check_id == "representation.format")
    );
    write(
        &secondary,
        &json!({"format":"gdd-records/0.2","project_id":"other","records":[]}),
    );
    let capture = bundle::load_with_records(&model, &secondary);
    assert_eq!(capture.exit_code(), 1);
    let invalid = assess(&model, &secondary);
    assert_eq!(invalid.exit_code(), 1);
    assert!(invalid.spec_revision.is_some());
    assert!(codes(&invalid).contains(&"SCHEMA_INVALID"));
    assert!(!codes(&invalid).contains(&"RECORD_INVALID"));
}

#[test]
#[cfg(unix)]
fn declared_alias_deduplicates_valid_invalid_and_missing_ledgers() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let model = root.path().join("primary.json");
    let declared = root.path().join("ledger.json");
    let secondary = root.path().join("alias.json");
    symlink("ledger.json", &secondary).unwrap();
    let mut primary = tiny_model();
    primary["record_files"] = json!(["ledger.json"]);
    write(&model, &primary);
    write(&declared, &ledger("demo"));
    let captured = bundle::load_with_records(&model, &secondary);
    assert_eq!(captured.documents().len(), 2);
    assert_eq!(captured.primary_documents().len(), 2);
    fs::write(&declared, r#"{"records":1,"records":2}"#).unwrap();
    let invalid = assess(&model, &secondary);
    assert_eq!(invalid.exit_code(), 1);
    assert_eq!(
        codes(&invalid)
            .iter()
            .filter(|c| **c == "YAML_DUPLICATE_KEY")
            .count(),
        1
    );
    fs::remove_file(&declared).unwrap();
    let absent = assess(&model, &secondary);
    assert_eq!(absent.exit_code(), 2);
    assert_eq!(
        codes(&absent)
            .iter()
            .filter(|c| **c == "SOURCE_UNAVAILABLE")
            .count(),
        1
    );
    fs::remove_file(&secondary).unwrap();
    symlink("alias.json", &secondary).unwrap();
    let looped = assess(&model, &secondary);
    assert_eq!(looped.exit_code(), 1);
    assert!(looped.checks.iter().any(|c| {
        c.reason
            .contains("Secondary record input cannot be resolved")
    }));
}
