use anyhow::Result;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

#[test]
fn diff_reports_each_catalog_instance_without_an_external_runtime() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = Command::new(env!("CARGO_BIN_EXE_gdd-conformance"));
    command
        .current_dir(temp.path())
        .env("PATH", "")
        .arg("--checkout")
        .arg(&checkout)
        .arg("diff");
    #[cfg(unix)]
    {
        let real = temp.path().join("scratch");
        let linked = temp.path().join("linked-scratch");
        fs::create_dir(&real)?;
        std::os::unix::fs::symlink(&real, &linked)?;
        command.env("TMPDIR", linked);
    }
    let run = command.output()?;
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let receipt: Value = serde_json::from_slice(&run.stdout)?;
    assert_eq!(receipt["format"], "gdd-model-diff-conformance/0.1");
    assert_eq!(
        receipt["tool"],
        concat!("gdd-conformance/", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(receipt["runner"], "rust");
    assert_eq!(receipt["case_count"], 16);
    assert_eq!(receipt["test_instances"], 25);
    assert_eq!(receipt["passed"], true);
    let catalog = checkout.join("conformance/diff.cases.json");
    let catalog: Vec<Value> = serde_json::from_slice(&fs::read(catalog)?)?;
    let rows = receipt["cases"].as_array().unwrap();
    assert_eq!(rows.len(), catalog.len());
    for (row, case) in rows.iter().zip(catalog) {
        assert_eq!(row["id"], case["id"]);
        assert_eq!(row["assertion"], case["assertion"]);
        assert_eq!(row["result"], "PASS");
        let count = match case["id"].as_str().unwrap() {
            "diff.semantic_unknown" => 2,
            "diff.weakening" => 7,
            "diff.contracts" => 3,
            _ => 1,
        };
        assert_eq!(row["test_instances"], count);
    }

    let output = temp.path().join("diff.json");
    command.arg("--output").arg(&output);
    assert!(command.output()?.status.success());
    assert_eq!(fs::read(&output)?, run.stdout);
    fs::write(&output, b"existing")?;
    assert!(!command.output()?.status.success());
    assert_eq!(fs::read(output)?, b"existing");
    Ok(())
}
