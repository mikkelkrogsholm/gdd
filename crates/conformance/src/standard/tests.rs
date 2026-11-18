use super::*;
use std::fs;

#[test]
fn rewritten_fixtures_preserve_nested_field_order_and_scalar_types() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("fixture.yaml");
    fs::write(
        &path,
        "z: [{c: '4', b: true, a: 7}]\nremove: 1\nm: null\na: last\n",
    )?;
    let mut data = fixture::read(&path)?;
    let patches = serde_json::from_value(json!([
        {"op":"remove","path":"/remove"},
        {"op":"replace","path":"/z/0/c","value":"01"},
        {"op":"add","path":"/z/-","value":{"last":1,"first":2}}
    ]))?;
    fixture::patch(&mut data, &patches)?;
    fixture::write(&path, &data)?;
    let raw = fs::read(&path)?;
    assert!(raw.ends_with(b"\n"));
    assert_eq!(
        serde_json::to_string(&fixture::read(&path)?)?,
        r#"{"z":[{"c":"01","b":true,"a":7},{"last":1,"first":2}],"m":null,"a":"last"}"#
    );
    Ok(())
}

#[test]
fn errors_and_wrong_expectations_preserve_other_results_and_exit_with_failure() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let directory = temp.path().join("conformance");
    let examples = temp.path().join("examples");
    fs::create_dir(&directory)?;
    fs::create_dir(&examples)?;
    let model = examples.join("sketch.gdd.yaml");
    let base = include_bytes!("../../../../examples/sketch.gdd.yaml");
    fs::write(&model, base)?;
    let control = json!({"id":"control", "profile":"model", "scope":[], "base":"../examples/sketch.gdd.yaml",
        "expected":{"dimension":"model", "result":"PASS","diagnostic":null}});
    let mut broken = control.clone();
    broken["id"] = "bad-patch".into();
    broken["patches"] = json!([{"op":"remove","path":"/absent"}]);
    let mut wrong = control.clone();
    wrong["id"] = "wrong".into();
    wrong["expected"]["result"] = "FAIL".into();
    let mut diagnostic = control.clone();
    diagnostic["id"] = "missing-diagnostic".into();
    diagnostic["expected"]["diagnostic"] = "SCHEMA_INVALID".into();
    let mut unknown = control.clone();
    unknown["id"] = "unknown-condition".into();
    unknown["condition"] = "An unimplemented fixture operation.".into();
    let manifest = json!({"format":"gdd-conformance-cases/0.2", "cases":[broken, wrong, diagnostic, unknown, control]});
    fixture::write(&directory.join("cases.yaml"), &manifest)?;
    let output = temp.path().join("run.json");
    assert_eq!(
        command(&directory, Some(output.clone()))?,
        ExitCode::FAILURE
    );
    let receipt: Value = serde_json::from_slice(&fs::read(output)?)?;
    assert_eq!(receipt["total"], 5);
    assert_eq!(receipt["matched"], 1);
    let rows = &receipt["cases"];
    assert_eq!(rows[0]["id"], "bad-patch");
    assert!(rows[0]["error"].as_str().unwrap().contains("/absent"));
    assert!(rows[0].get("report").is_none());
    assert_eq!(rows[1]["actual"], "PASS");
    assert_eq!(rows[1]["matched"], false);
    assert_eq!(rows[2]["diagnostics"], json!([]));
    assert_eq!(rows[2]["matched"], false);
    assert!(
        rows[3]["error"]
            .as_str()
            .unwrap()
            .contains("unimplemented standard condition")
    );
    assert_eq!(rows[4]["matched"], true);
    assert_eq!(rows[1]["report"], rows[4]["report"]);
    assert_eq!(fs::read(model)?, base);
    Ok(())
}
