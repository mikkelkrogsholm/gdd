use serde_json::Value;
use std::{fs, path::Path, process::Command};

#[test]
fn status_command_reports_each_manifest_case_and_preserves_existing_receipts() {
    let run = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gdd-conformance"));
        command.arg("status");
        command
    };
    let stdout = run().output().unwrap();
    assert!(
        stdout.status.success(),
        "{}",
        String::from_utf8_lossy(&stdout.stderr)
    );
    let receipt: Value = serde_json::from_slice(&stdout.stdout).unwrap();
    assert_eq!(receipt["format"], "gdd-status-conformance/0.1");
    assert_eq!(
        receipt["tool"],
        concat!("gdd-conformance/", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(receipt["runner"], "rust");
    assert_eq!(receipt["case_count"], 13);
    assert_eq!(receipt["test_instances"], 15);
    assert_eq!(receipt["passed"], true);
    for field in ["runtime_target", "pytest_exit", "stdout", "stderr"] {
        assert!(receipt.get(field).is_none());
    }
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance");
    let catalog: Vec<Value> =
        serde_json::from_slice(&fs::read(directory.join("status.cases.json")).unwrap()).unwrap();
    let rows = receipt["cases"].as_array().unwrap();
    assert_eq!(rows.len(), catalog.len());
    for (case, row) in catalog.iter().zip(rows) {
        assert_eq!(row["id"], case["id"]);
        assert_eq!(row["assertion"], case["assertion"]);
        assert_eq!(row["result"], "PASS");
        let instances = match case["id"].as_str().unwrap() {
            "status.integration_exit_policy"
            | "status.recorded_acceptance_is_only_current_when_existing_assessor_says_so" => 2,
            _ => 1,
        };
        assert_eq!(row["test_instances"], instances);
    }

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("receipt.json");
    let publish = || run().arg("--output").arg(&path).output().unwrap();
    assert!(publish().status.success());
    assert_eq!(fs::read(&path).unwrap(), stdout.stdout);
    fs::write(&path, b"existing receipt").unwrap();
    assert!(!publish().status.success());
    assert_eq!(fs::read(&path).unwrap(), b"existing receipt");
}
