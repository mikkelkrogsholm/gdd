use super::*;
use crate::code::BEHAVIORS;

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
        let row = execute(directory, "code", BEHAVIORS, case, &mut seen);
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
    let row = execute(&directory, "code", BEHAVIORS, case, &mut BTreeSet::new());
    assert_eq!(row.result, "FAIL");
    assert_eq!(row.test_instances, 4);
    assert_eq!(row.errors.len(), 4);
    Ok(())
}
