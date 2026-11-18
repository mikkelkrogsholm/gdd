//! Exercise portable evidence and exclusive publication through the demo CLI.

use gdd::{host::snapshot_code, input, workflow_run::run_status};
use serde_json::Value;
use std::{fs, path::Path, process::Command};

fn read(root: &Path, name: &str) -> Value {
    serde_json::from_slice(&fs::read(root.join(name)).unwrap()).unwrap()
}

#[test]
fn published_run_survives_relocation_and_refuses_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("demo");
    let invoke = || {
        Command::new(env!("CARGO_BIN_EXE_gdd-conformance"))
            .current_dir(temp.path())
            .args(["workflow-demo", "--output"])
            .arg(&destination)
            .output()
            .unwrap()
    };
    let result = invoke();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let summary: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(summary, read(&destination, "summary.json"));
    fs::write(destination.join("marker"), b"preserve me").unwrap();
    assert!(!invoke().status.success());
    assert_eq!(
        fs::read(destination.join("marker")).unwrap(),
        b"preserve me"
    );

    let root = temp.path().join("relocated");
    fs::rename(destination, &root).unwrap();
    let host = input::parse(&fs::read(root.join("host.acceptance.json")).unwrap()).unwrap();
    let status = run_status(&root.join("model.gdd.json"), &root.join("run"), Some(&host)).unwrap();
    assert!(status.acceptance_current);
    assert_eq!(status.event_count, 10);
    let sources = read(&root, "code.snapshot.json");
    let names: Vec<_> = sources["files"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(names.len(), 11);
    let snapshot = snapshot_code(&root, &names).unwrap();
    assert_eq!(summary["code_revision"], snapshot.revision);
    assert_eq!(status.state.code_revision, snapshot.revision);
    assert!(!root.join("target").exists());
    assert_eq!(summary["baseline_exit"], 1);
    assert_eq!(summary["baseline_cargo_exit"], 101);
    for (stage, count, failed, exit) in [
        ("baseline", 18, 7, 101),
        ("discount", 5, 0, 0),
        ("tax", 5, 0, 0),
        ("quote", 9, 0, 0),
        ("final", 19, 0, 0),
    ] {
        let receipt = read(&root, &format!("evidence/{stage}.run.json"));
        let tests = receipt["tests"].as_object().unwrap();
        assert_eq!(tests.len(), count);
        assert_eq!(
            tests
                .values()
                .filter(|outcome| *outcome == "FAILED")
                .count(),
            failed
        );
        assert_eq!(receipt["exit_code"], exit);
        assert_eq!(receipt["test_exit"], i32::from(failed != 0));
        assert_eq!(receipt["code_revision"], receipt["code_revision_after"]);
        assert_eq!(receipt["code_revision"], receipt["snapshot"]["revision"]);
        assert!(
            receipt["runner"]["cargo"]
                .as_str()
                .unwrap()
                .starts_with("cargo ")
        );
        assert!(
            receipt["runner"]["rustc"]
                .as_str()
                .unwrap()
                .starts_with("rustc ")
        );
    }
}
