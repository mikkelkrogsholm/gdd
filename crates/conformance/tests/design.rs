use serde_json::Value;
use std::{fs, process::Command};

const RUNNER: &str = env!("CARGO_BIN_EXE_gdd-conformance");

#[test]
fn design_receipt_covers_the_ordered_catalog_and_parameterized_expectations() {
    let run = Command::new(RUNNER).arg("design").output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let receipt: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(receipt["format"], "gdd-design-conformance/0.3");
    assert_eq!(receipt["runner"], "rust");
    assert_eq!(receipt["passed"], true);
    assert_eq!(receipt["case_count"], 32);
    assert_eq!(receipt["test_instances"], 52);
    let catalog: Vec<Value> =
        serde_json::from_str(include_str!("../../../conformance/design.cases.json")).unwrap();
    let rows = receipt["cases"].as_array().unwrap();
    assert_eq!(rows.len(), catalog.len());
    for (row, case) in rows.iter().zip(&catalog) {
        assert_eq!(row["id"], case["id"]);
        assert_eq!(row["assertion"], case["assertion"]);
        assert_eq!(row["result"], "PASS");
        let name = case["id"]
            .as_str()
            .unwrap()
            .strip_prefix("case.design.")
            .unwrap();
        let count = match name {
            "partial_boundary_sketch_and_readiness" | "unresolved_call_valid_sketch_unready" => 3,
            "blank_boundary_is_invalid"
            | "refinement_preserves_parent_boundary_context"
            | "packet_omission_rejected"
            | "broad_principle_is_advisory" => 2,
            "review_unknown_is_explicit" => 8,
            "invalid_review_cannot_be_complete" => 6,
            _ => 1,
        };
        assert_eq!(row["test_instances"], count, "{name}");
    }
}

#[test]
fn failing_native_cli_is_recorded_and_the_suite_continues() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("receipt.json");
    // The conformance binary rejects the gdd subcommand with a nonzero exit.
    let run = Command::new(RUNNER)
        .env("GDD_NATIVE_CLI", RUNNER)
        .arg("design")
        .arg("--output")
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(1));
    let receipt: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(receipt["passed"], false);
    assert_eq!(receipt["test_instances"], 52);
    let failed: Vec<_> = receipt["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["result"] != "PASS")
        .collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["id"], "case.design.new_viewer_and_cli_operations");
    assert!(
        failed[0]["errors"][0]
            .as_str()
            .unwrap()
            .contains("gdd boundaries exited with")
    );
}
