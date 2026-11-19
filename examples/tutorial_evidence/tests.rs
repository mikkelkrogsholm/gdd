//! Exercise the full workflow with the separately packaged tutorial sources.

use super::*;
use gdd::report::{CheckResult, Report};
use gdd::validation::{Activity, Profile, ValidationOptions, validate_with_options};
use std::process::Command;

fn assess(plan: &Path, context: Option<&Value>, activity: Option<Activity>) -> Report {
    let host = context.map(gdd::input::Value::from);
    validate_with_options(
        &gdd::bundle::load(&plan.join("evidence.gdd.json")),
        ValidationOptions {
            profiles: &[Profile::Records],
            scope: &["action.add".to_owned()],
            host_context: host.as_ref(),
            activity,
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
#[ignore = "Set GDD_TUTORIAL_SOURCE to the extracted gdd-tutorial-add package and run with --ignored"]
fn attest_actual_checks_and_observe_drift() {
    let source = PathBuf::from(std::env::var_os("GDD_TUTORIAL_SOURCE").unwrap());
    let temp = tempfile::tempdir().unwrap();
    let plan = temp.path().join("sum");
    assert!(
        Command::new(env!("CARGO"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args([
                "run",
                "--offline",
                "--locked",
                "--example",
                "tutorial_prepare",
                "--"
            ])
            .arg(&plan)
            .status()
            .unwrap()
            .success()
    );
    checks::run(&prepared(&plan).unwrap(), &source).unwrap();
    let ledger = plan.join("records.json");
    let original = fs::read(&ledger).unwrap();
    assert!(record(&plan, false).is_err());
    assert_eq!(fs::read(&ledger).unwrap(), original);

    // Tamper with one field of each genuine receipt and verify that neither can be used.
    for (name, key, value) in [
        (
            "negative.receipt.json",
            "stderr",
            json!("compilation failed"),
        ),
        ("pass.receipt.json", "command", json!(["cargo", "check"])),
    ] {
        let path = plan.join("runs").join(name);
        let genuine = fs::read(&path).unwrap();
        let mut receipt = read(&path).unwrap();
        receipt[key] = value;
        write_json(File::create(&path).unwrap(), &receipt).unwrap();
        assert!(record(&plan, true).is_err());
        assert_eq!(fs::read(&ledger).unwrap(), original);
        fs::write(path, genuine).unwrap();
    }
    record(&plan, true).unwrap();
    let reviewed = fs::read(&ledger).unwrap();
    assert!(record(&plan, true).is_err());
    let current = read(&plan.join("review/host-context.json")).unwrap();
    let untrusted = assess(&plan, None, None);
    assert_eq!(untrusted.result("implementation"), CheckResult::Unknown);
    assert_eq!(untrusted.exit_code(), 2);
    let trusted = assess(&plan, Some(&current), None);
    assert_eq!(trusted.result("implementation"), CheckResult::Pass);
    assert_eq!(trusted.exit_code(), 0);
    let acceptance = assess(&plan, Some(&current), Some(Activity::Acceptance));
    assert_eq!(acceptance.result("authorization"), CheckResult::Unresolved);
    assert_eq!(acceptance.exit_code(), 2);
    drift(&plan).unwrap();
    let changed = read(&plan.join("drift/host-context.json")).unwrap();
    let stale = assess(&plan, Some(&changed), None);
    assert_eq!(stale.result("implementation"), CheckResult::Unknown);
    assert_eq!(stale.exit_code(), 2);
    assert_eq!(fs::read(&ledger).unwrap(), reviewed);
}
