//! Verify replay diagnostics using real Cargo and an intentionally incomplete concert package.

use anyhow::Result;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

const RUNNER: &str = env!("CARGO_BIN_EXE_gdd-conformance");

fn fixture(checkout: &Path) -> Result<()> {
    fs::create_dir_all(checkout.join("conformance"))?;
    let app = checkout.join("examples/concert-app");
    fs::create_dir_all(app.join("history/phase-a"))?;
    fs::create_dir_all(app.join("src"))?;
    fs::write(app.join("src/lib.rs"), "")?;
    fs::create_dir(app.join("tests"))?;
    fs::write(
        app.join("tests/requirement_change.rs"),
        "#[test] fn unrelated_failure() { assert_eq!(1, 2); }",
    )?;
    fs::write(
        checkout.join("Cargo.toml"),
        "[workspace]\nresolver='3'\nmembers=['examples/concert-app']\n",
    )?;
    fs::write(
        app.join("Cargo.toml"),
        "[package]\nname='gdd-concert'\nversion='0.0.0'\nedition='2024'\n",
    )?;
    fs::write(
        checkout.join("Cargo.lock"),
        "version = 4\n[[package]]\nname = 'gdd-concert'\nversion = '0.0.0'\n",
    )?;
    let originals = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/concert-app");
    for phase in ["", "history/phase-a"] {
        for name in [
            "gdd.json",
            "gdd.records.json",
            "DESIGN.md",
            "HTTP.md",
            "REQUIREMENTS.md",
        ] {
            fs::copy(originals.join(phase).join(name), app.join(phase).join(name))?;
        }
    }
    Ok(())
}

fn replay(cwd: &Path, destination: &str) -> Result<std::process::Output> {
    Ok(Command::new(RUNNER)
        .current_dir(cwd)
        .env("CARGO_NET_OFFLINE", "true")
        .args([
            "--checkout",
            "selected checkout",
            "concert-policy-replay",
            "--output",
            destination,
        ])
        .output()?)
}

#[test]
fn unrelated_red_test_is_rejected_and_its_logs_archive_and_model_diff_survive_retry() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let cwd = directory.path().canonicalize()?;
    let checkout = cwd.join("selected checkout");
    fixture(&checkout)?;
    let output = cwd.join("run one");
    let result = replay(&cwd, "run one")?;
    assert_eq!(result.status.code(), Some(1));
    let bytes = fs::read(output.join("summary.json"))?;
    let receipt: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(receipt["passed"], false);
    assert_eq!(receipt["checkout"], checkout.to_str().unwrap());
    let archive = output.join("gdd-concert-0.0.0.crate");
    assert_eq!(receipt["source_package"]["path"], archive.to_str().unwrap());
    assert_eq!(
        receipt["source_package"]["sha256"],
        gdd::sha256(&fs::read(&archive)?)
    );

    let model_diff: Value = serde_json::from_slice(&fs::read(output.join("model-diff.json"))?)?;
    assert_eq!(model_diff["comparison"]["coverage"], "complete");
    let policy = &receipt["policy_change"];
    assert_eq!(
        policy["phase_a"]["spec_revision"],
        model_diff["base"]["spec_revision"]
    );
    assert_eq!(
        policy["phase_b"]["spec_revision"],
        model_diff["head"]["spec_revision"]
    );
    assert_ne!(
        policy["phase_a"]["spec_revision"],
        policy["phase_b"]["spec_revision"]
    );
    assert_eq!(policy["added_rule"]["id"], "rule.active_limit");
    for phase in ["phase_a", "phase_b"] {
        let captured = output.join(policy[phase]["capture"].as_str().unwrap());
        let bundle = gdd::bundle::load_model_only(&captured);
        assert_eq!(
            bundle.snapshot().unwrap().revision,
            policy[phase]["spec_revision"]
        );
    }
    let steps = receipt["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    let red = &steps[1];
    assert_eq!(red["name"], "concert-requirements-a");
    assert_eq!(
        red["policy_spec_revision"],
        model_diff["base"]["spec_revision"]
    );
    assert_eq!(
        red["requirements_spec_revision"],
        model_diff["head"]["spec_revision"]
    );
    assert_eq!(red["exit_code"], 101);
    assert_eq!(red["passed"], false);
    assert!(red["outcomes"].is_null());
    assert!(
        fs::read_to_string(output.join(red["stdout"].as_str().unwrap()))?
            .contains("unrelated_failure ... FAILED")
    );
    assert!(
        fs::read_to_string(output.join(red["stderr"].as_str().unwrap()))?
            .contains("expected exactly ten concert test results")
    );
    assert!(replay(&cwd, "run one")?.status.code() == Some(1));
    assert_eq!(fs::read(output.join("summary.json"))?, bytes);

    // A rule with the same ID can state an entirely different allowance.
    let model = checkout.join("examples/concert-app/gdd.json");
    let original = fs::read(&model)?;
    let mut changed: Value = serde_json::from_slice(&original)?;
    let rule = &mut changed["nodes"]["rule.active_limit"];
    let three = "Each principal has at most three reserved reservations across all concerts.";
    rule["rule"]["statement"] = three.into();
    rule["description"] = three.into();
    fs::write(&model, serde_json::to_vec_pretty(&changed)?)?;
    let rejected = replay(&cwd, "changed allowance")?;
    assert_eq!(rejected.status.code(), Some(1));
    let changed_dir = cwd.join("changed allowance");
    let rejected: Value = serde_json::from_slice(&fs::read(changed_dir.join("summary.json"))?)?;
    assert_eq!(rejected["passed"], false);
    assert_eq!(rejected["steps"].as_array().unwrap().len(), 1);
    assert!(
        rejected["error"]
            .as_str()
            .unwrap()
            .contains("unsupported concert policy revision")
    );
    let diff: Value = serde_json::from_slice(&fs::read(changed_dir.join("model-diff.json"))?)?;
    assert_ne!(
        diff["head"]["spec_revision"],
        model_diff["head"]["spec_revision"]
    );
    assert!(
        diff["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change["id"] == "rule.active_limit"
                && change["change"] == "added"
                && change["after"]["rule"]["statement"] == three)
    );
    fs::write(model, original)?;

    fs::write(
        checkout.join("examples/concert-app/history/phase-a/HTTP.md"),
        "different contract",
    )?;
    assert_eq!(replay(&cwd, "run two")?.status.code(), Some(1));
    let rejected: Value = serde_json::from_slice(&fs::read(cwd.join("run two/summary.json"))?)?;
    assert_eq!(rejected["steps"].as_array().unwrap().len(), 1);
    assert!(
        rejected["error"]
            .as_str()
            .unwrap()
            .contains("expected a complete semantic concert model diff")
    );
    assert!(cwd.join("run two/model-diff.json").is_file());
    Ok(())
}
