//! Exercise the standard runner as a checkout command without external executables.

use anyhow::{Context, Result};
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path, process::Command};

#[test]
fn standard_runs_all_declared_cases_offline_and_protects_its_receipt() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_gdd-conformance"));
    command
        .current_dir(temp.path())
        .env("PATH", "")
        .arg("standard");
    let run = command.output()?;
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let receipt: Value = serde_json::from_slice(&run.stdout)?;
    assert_eq!(receipt["format"], "gdd-conformance-run/0.2");
    assert_eq!(
        receipt["tool"],
        concat!("gdd-conformance/", env!("CARGO_PKG_VERSION"))
    );
    assert!(receipt["executed_at"].as_str().unwrap().ends_with('Z'));
    assert_eq!(receipt["synthetic_inputs"], true);

    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = fs::read(checkout.join("conformance/cases.yaml"))?;
    let manifest = gdd::validation::to_json(&gdd::input::parse(&manifest)?)?;
    let definitions = manifest["cases"].as_array().unwrap();
    let rows = receipt["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 58);
    assert_eq!(definitions.len(), rows.len());
    assert_eq!(receipt["matched"], rows.len());
    assert_eq!(receipt["total"], rows.len());
    for (case, row) in definitions.iter().zip(rows) {
        let id = &case["id"];
        assert_eq!(row["id"], *id);
        assert_eq!(row["matched"], true, "{id}: {row}");
        assert_eq!(row["expected"], case["expected"], "{id}");
        assert_eq!(row["actual"], case["expected"]["result"], "{id}");
        assert_eq!(row["report"]["format"], "gdd-report/0.2", "{id}");
        let checks = row["report"]["checks"].as_array().unwrap();
        let dimension = &case["expected"]["dimension"];
        let assessed: Vec<_> = checks
            .iter()
            .filter(|check| check["dimension"] == *dimension)
            .collect();
        assert!(!assessed.is_empty(), "{id}");
        let codes: BTreeSet<_> = assessed
            .iter()
            .flat_map(|c| c["diagnostics"].as_array().unwrap())
            .map(|diagnostic| diagnostic["code"].as_str().unwrap())
            .collect();
        assert_eq!(row["diagnostics"], serde_json::to_value(&codes)?, "{id}");
        if let Some(required) = case["expected"]["diagnostic"].as_str() {
            assert!(codes.contains(required), "{id}");
        }
    }
    // The unmodified model uses the same byte-based revision as direct validation.
    let report = gdd::validation::validate_path(
        &checkout.join("examples/concert.gdd.yaml"),
        gdd::validation::ValidationOptions {
            profiles: &[gdd::validation::Profile::Representation],
            ..Default::default()
        },
        None,
    )?;
    let concert = rows
        .iter()
        .find(|r| r["id"] == "case.schema.concert")
        .context("missing concert case")?;
    assert_eq!(concert["report"], serde_json::to_value(report)?);

    let path = temp.path().join("new.json");
    command.args(["--output", "new.json"]);
    let saved = command.output()?;
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    assert_eq!(
        String::from_utf8(saved.stdout)?.trim(),
        "58/58 native standard expectations matched"
    );
    let original = fs::read(&path)?;
    assert_eq!(
        serde_json::from_slice::<Value>(&original)?["matched"],
        rows.len()
    );
    let collision = command.output()?;
    assert!(!collision.status.success());
    assert!(String::from_utf8_lossy(&collision.stderr).contains("create receipt new.json"));
    assert_eq!(fs::read(path)?, original);
    Ok(())
}
