use super::*;
use std::fs;

#[test]
fn all_named_cases_run_and_existing_receipts_are_preserved() -> Result<()> {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir()?;
    // Run from the suite's data files alone, with no live Python project or checker.
    for file in [
        "conformance/code.cases.json",
        "schemas/code-observation.schema.json",
        "archive/experiments/concert-7e27757.index.json",
        "archive/experiments/concert-7e27757.tar.gz",
    ] {
        let target = temp.path().join(file);
        fs::create_dir_all(target.parent().unwrap())?;
        fs::copy(checkout.join(file), target)?;
    }
    let directory = temp.path().join("conformance");
    let output = temp.path().join("run.json");
    assert_eq!(
        command(&directory, Some(output.clone()))?,
        ExitCode::SUCCESS
    );
    let original = fs::read(&output)?;
    let run: Value = serde_json::from_slice(&original)?;
    assert_eq!(run["case_count"], 28);
    assert_eq!(run["test_instances"], 50);
    assert_eq!(run["passed"], true);
    assert_eq!(run["runner"], "rust");
    assert_eq!(run["format"], "gdd-code-conformance/0.1");
    assert!(command(&directory, Some(output.clone())).is_err());
    assert_eq!(fs::read(output)?, original);
    Ok(())
}
