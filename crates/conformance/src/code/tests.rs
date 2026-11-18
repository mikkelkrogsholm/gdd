use super::*;

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

#[test]
fn unknown_duplicate_and_misnamed_cases_cannot_pass() {
    let directory = Path::new(".");
    let mut seen = BTreeSet::new();
    for (id, reference, error) in [
        ("unknown", "unused", "Unknown code case"),
        (
            "code.output-schema",
            "unused",
            "Unrecognized test reference",
        ),
        ("code.output-schema", "unused", "Duplicate case ID"),
    ] {
        let case = Case {
            id: id.into(),
            assertion: "Verify controls.".into(),
            test: reference.into(),
        };
        let row = execute(directory, case, &mut seen);
        assert_eq!(row.result, "FAIL");
        assert_eq!(row.test_instances, 0);
        assert!(row.errors[0].starts_with(error));
    }
}

#[test]
fn failed_instance_records_a_failure_for_each_parameter_attempted() -> Result<()> {
    // The schema is intentionally absent from this checkout-shaped directory.
    let root = tempfile::tempdir()?;
    let directory = root.path().join("conformance");
    fs::create_dir(&directory)?;
    let behavior = BEHAVIORS
        .iter()
        .find(|b| b.id == "code.output-schema")
        .unwrap();
    let case = Case {
        id: behavior.id.into(),
        assertion: "Check observation schema.".into(),
        test: behavior.reference.into(),
    };
    let row = execute(&directory, case, &mut BTreeSet::new());
    assert_eq!(row.result, "FAIL");
    assert_eq!(row.test_instances, 4);
    assert_eq!(row.errors.len(), 4);
    Ok(())
}
