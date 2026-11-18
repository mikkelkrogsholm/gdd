//! Keep the ordered native workflow receipt aligned with the catalog's parameter matrix.

use serde_json::Value;
use std::process::Command;

#[test]
fn workflow_receipt_covers_every_named_case_and_parameter() {
    let output = Command::new(env!("CARGO_BIN_EXE_gdd-conformance"))
        .arg("workflow")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(receipt["format"], "gdd-workflow-conformance/0.1");
    assert_eq!(receipt["runner"], "rust");
    assert_eq!(
        receipt["demo_implementation"],
        "implementation.quote_demo.rust"
    );
    assert_eq!(
        receipt["demo_inputs"],
        "Rust reconstruction of the archived Python repairs"
    );
    assert_eq!(receipt["passed"], true);
    assert_eq!(receipt["case_count"], 20);
    assert_eq!(receipt["test_instances"], 63);
    let catalog: Vec<Value> =
        serde_json::from_str(include_str!("../../../conformance/workflow.cases.json")).unwrap();
    let rows = receipt["cases"].as_array().unwrap();
    assert_eq!(rows.len(), catalog.len());
    for (row, case) in rows.iter().zip(catalog) {
        let id = case["id"].as_str().unwrap();
        assert_eq!(row["id"], id);
        assert_eq!(row["assertion"], case["assertion"], "{id}");
        assert_eq!(row["result"], "PASS", "{id}");
        let count = match id {
            "workflow.ownership" | "workflow.publication" => 3,
            "workflow.host_shape" => 20,
            "workflow.unresolved_model" => 2,
            "workflow.historical_records" | "workflow.integration" => 6,
            "workflow.current_acceptance" => 10,
            _ => 1,
        };
        assert_eq!(row["test_instances"], count, "{id}");
    }
}
