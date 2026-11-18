use super::*;
use std::fs;

#[test]
fn all_named_cases_run_and_existing_receipts_are_preserved() -> Result<()> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance");
    let temp = tempfile::tempdir()?;
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
