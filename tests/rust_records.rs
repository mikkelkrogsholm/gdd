use gdd::bundle::{Bundle, load_model_only};
use gdd::input;
use gdd::report::{CheckResult, Report};
use gdd::validation::{Activity, Profile, ValidationOptions, validate_with_options};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn write(path: &Path, json: &Value) {
    fs::write(path, serde_json::to_vec(json).unwrap()).unwrap();
}

fn assess(
    bundle: &Bundle,
    profiles: &[Profile],
    scope: &[&str],
    host: &Value,
    activity: Option<Activity>,
) -> Report {
    let host = input::parse_json(&host.to_string()).unwrap();
    let scope = scope.iter().copied().map(str::to_owned).collect::<Vec<_>>();
    validate_with_options(
        bundle,
        ValidationOptions {
            profiles,
            scope: &scope,
            host_context: Some(&host),
            activity,
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn archived_evidence_uses_the_captured_ledger_and_requested_code_revision() {
    let dir = tempfile::tempdir().unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/agent-native");
    for filename in ["model.gdd.json", "model.records.json", "REQUIREMENTS.md"] {
        fs::copy(example.join(filename), dir.path().join(filename)).unwrap();
    }
    let manifest = dir.path().join("model.gdd.json");
    let ledger = dir.path().join("model.records.json");
    let host: Value =
        serde_json::from_slice(&fs::read(example.join("host.acceptance.json")).unwrap()).unwrap();
    let implementation = |bundle: &Bundle, host: &Value| {
        assess(bundle, &[Profile::Records], &["action.quote"], host, None)
    };
    let captured = load_model_only(&manifest);
    let report = implementation(&captured, &host);
    assert_eq!(report.exit_code(), 0);
    assert_eq!(report.result("implementation"), CheckResult::Pass);
    assert_eq!(report.result("authorization"), CheckResult::Unresolved);
    let original = serde_json::to_value(&report).unwrap();
    assert!(
        original["checks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["dimension"] == "implementation")
            .all(|c| c["evidence_refs"] == json!(["evidence.demo.final"])
                && c["code_revision"] == host["code_revision"])
    );
    let mut stale = host.clone();
    stale["code_revision"] = json!("code.changed");
    let stale_report = implementation(&captured, &stale);
    assert_eq!(stale_report.exit_code(), 2);
    assert!(
        stale_report
            .checks
            .iter()
            .any(|c| c.diagnostics.iter().any(|d| d.code == "EVIDENCE_STALE"))
    );
    assert!(stale_report.checks.iter().all(|c| c.details.is_none()));

    let mut records: Value = serde_json::from_slice(&fs::read(&ledger).unwrap()).unwrap();
    records["records"]["evidence.demo.final"]["result"] = json!("FAIL");
    write(&ledger, &records);
    assert_eq!(
        serde_json::to_value(implementation(&captured, &host)).unwrap(),
        original
    );
    let reloaded = implementation(&load_model_only(&manifest), &host);
    assert_eq!(reloaded.spec_revision, report.spec_revision);
    assert_eq!(reloaded.result("implementation"), CheckResult::Fail);
    assert_eq!(reloaded.exit_code(), 1);

    // A replacement cannot erase a failure solely by naming its predecessor.
    let mut successor = records["records"]["evidence.demo.final"].clone();
    successor["result"] = json!("PASS");
    successor["supersedes"] = json!("evidence.demo.final");
    records["records"]["evidence.new"] = successor;
    write(&ledger, &records);
    let with_successor = load_model_only(&manifest);
    assert_eq!(
        implementation(&with_successor, &host).result("implementation"),
        CheckResult::Unknown
    );
    let mut updated = host;
    updated["trusted_evidence"] = json!(["evidence.demo.final", "evidence.new"]);
    assert_eq!(
        implementation(&with_successor, &updated).result("implementation"),
        CheckResult::Pass
    );
    records["records"]["evidence.new"]["supersedes"] = json!("evidence.new");
    write(&ledger, &records);
    updated["evidence_selection"] = json!({"check.demo.all":"evidence.demo.final"});
    assert_eq!(
        implementation(&load_model_only(&manifest), &updated).result("implementation"),
        CheckResult::Fail
    );
}

#[test]
fn trusted_rejection_precedes_a_valid_approval_and_a_standing_mandate() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("model.json");
    let ledger = dir.path().join("ledger.json");
    write(
        &manifest,
        &json!({"format":"gdd/0.2","project":{"id":"demo","title":"Demo","goal":"Review a change."},
        "nodes":{"entity.x":{"kind":"entity","description":"Value","fields":{}}},"edges":[],"record_files":["ledger.json"]}),
    );
    let mut records = json!({"format":"gdd-records/0.2","project_id":"demo","records":{}});
    write(&ledger, &records);
    let revision = load_model_only(&manifest)
        .snapshot()
        .unwrap()
        .revision
        .clone();
    let receipt = json!({"kind":"decision","topic":"approval","activity":"model_changes","actor":{"id":"actor.a","kind":"human"},
        "outcome":"accepted","rationale":"Reviewed.","spec_revision":revision,"policy_revision":"policy.a","scope":["entity.x"]});
    records["records"]["decision.yes"] = receipt.clone();
    records["records"]["decision.no"] = receipt;
    records["records"]["decision.no"]["outcome"] = json!("rejected");
    write(&ledger, &records);
    let bundle = load_model_only(&manifest);
    let mut host = json!({"policy":{"revision":"policy.a","activity":"model_changes","actor":"actor.a","spec_revision":revision,
        "scope":[],"requirement":"explicit","authorities":["actor.a"],"basis":"Mandate"},"trusted_decisions":["decision.yes"]});
    let authorize = |host: &Value| {
        assess(
            &bundle,
            &[Profile::Representation],
            &["entity.x"],
            host,
            Some(Activity::ModelChanges),
        )
    };
    let granted = authorize(&host);
    assert_eq!(
        granted.profiles,
        [Profile::Representation, Profile::Records]
    );
    assert_eq!(granted.result("authorization"), CheckResult::Granted);
    assert_eq!(granted.exit_code(), 0);
    let check = granted
        .checks
        .iter()
        .find(|c| c.dimension == "authorization")
        .unwrap();
    assert_eq!(
        serde_json::to_value(check).unwrap()["authority_basis"],
        "decision.yes"
    );
    host["trusted_decisions"] = json!(["decision.yes", "decision.no"]);
    for requirement in ["explicit", "standing"] {
        host["policy"]["requirement"] = json!(requirement);
        let denied = authorize(&host);
        assert_eq!(denied.result("authorization"), CheckResult::Denied);
        assert_eq!(denied.exit_code(), 1);
        let check = denied
            .checks
            .iter()
            .find(|c| c.dimension == "authorization")
            .unwrap();
        assert_eq!(check.reason, "Applicable explicit rejection.");
        assert_eq!(check.diagnostics[0].severity, "error");
        assert!(
            serde_json::to_value(check)
                .unwrap()
                .get("authority_basis")
                .is_none()
        );
    }
}

#[test]
fn empty_requirements_standalone_records_and_unavailable_models_are_unresolved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    write(
        &path,
        &json!({"format":"gdd/0.2","project":{"id":"demo","title":"Demo","goal":"Review."},"nodes":{},"edges":[]}),
    );
    let bundle = load_model_only(&path);
    let empty = assess(&bundle, &[Profile::Records], &[], &Value::Null, None);
    assert_eq!(empty.exit_code(), 2);
    assert!(
        empty
            .checks
            .iter()
            .any(|c| c.check_id == "records.integrity" && c.result == CheckResult::Pass)
    );
    assert!(
        empty.checks.iter().any(
            |c| c.check_id == "implementation.requirements" && c.result == CheckResult::Unknown
        )
    );
    let missing = assess(
        &bundle,
        &[Profile::Records],
        &["entity.missing"],
        &Value::Null,
        None,
    );
    assert!(
        missing
            .checks
            .iter()
            .any(|c| c.check_id == "implementation.scope")
    );
    let missing = assess(
        &bundle,
        &[],
        &["entity.missing"],
        &Value::Null,
        Some(Activity::Implementation),
    );
    assert!(
        missing
            .checks
            .iter()
            .any(|c| c.check_id == "authorization.scope")
    );
    let global = assess(&bundle, &[], &[], &Value::Null, Some(Activity::Acceptance));
    assert_eq!(global.result("authorization"), CheckResult::Unresolved);
    assert_eq!(global.profiles, [Profile::Model, Profile::Records]);

    write(
        &path,
        &json!({"format":"gdd-records/0.2","project_id":"demo","records":{}}),
    );
    let standalone = assess(
        &load_model_only(&path),
        &[Profile::Readiness],
        &[],
        &Value::Null,
        Some(Activity::Acceptance),
    );
    assert_eq!(standalone.exit_code(), 2);
    for id in [
        "model.unavailable",
        "readiness.unavailable",
        "records.unavailable",
        "authorization.unavailable",
    ] {
        assert!(standalone.checks.iter().any(|c| c.check_id == id));
    }
    write(
        &path,
        &json!({"format":"gdd/0.3","project":{},"nodes":{},"edges":[]}),
    );
    let invalid = assess(
        &load_model_only(&path),
        &[Profile::Records],
        &[],
        &Value::Null,
        Some(Activity::Acceptance),
    );
    assert_eq!(invalid.exit_code(), 1);
    assert_eq!(invalid.result("implementation"), CheckResult::Unknown);
    assert_eq!(invalid.result("authorization"), CheckResult::Unresolved);
    assert!(invalid.spec_revision.is_none());
}
