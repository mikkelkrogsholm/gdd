use super::*;

const FIXTURE: &[u8] = include_bytes!("../../../../conformance/fixtures/viewer-multigraph.json");

#[test]
fn each_tamper_changes_the_requested_artifact_content() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let model = temp.path().join("model.json");
    fs::write(&model, FIXTURE)?;
    let path = temp.path().join("original.gddview");
    render_view(&model, &path, &[])?;
    let original = fs::read(&path)?;
    let data = gdd::view::read_view(&original)?;
    let nodes = &data["model"]["nodes"];
    assert!(
        nodes["constructor"]["description"]
            .as_str()
            .unwrap()
            .contains("</script>")
    );
    assert_eq!(
        nodes["entity.precision"]["fields"]["huge"]["minimum"],
        9_007_199_254_740_993u64
    );
    let edges = data["model"]["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 9);
    let edge = |id| edges.iter().position(|e| e["id"] == id).unwrap();
    let first = edge("edge.call_one");
    let second = edge("edge.call_two");
    assert_eq!(edges[first]["relation"], edges[second]["relation"]);
    assert_eq!(edges[first]["from"], edges[second]["from"]);
    assert_eq!(edges[first]["to"], edges[second]["to"]);

    for op in ["renderer", "revision", "manifest", "direction", "omit_edge"] {
        fs::write(&path, &original)?;
        mutate_artifact(op, &path)?;
        let modified: Value = serde_json::from_slice(&fs::read(&path)?)?;
        let mut expected = data.clone();
        match op {
            "renderer" => expected["generator"]["version"] = "unsupported".into(),
            "revision" => expected["spec_revision"] = format!("sha256:{}", "0".repeat(64)).into(),
            "manifest" => expected["manifest"]["shown_edges"] = json!([]),
            "direction" => {
                expected["model"]["edges"][first]["from"] = edges[first]["to"].clone();
                expected["model"]["edges"][first]["to"] = edges[first]["from"].clone();
            }
            "omit_edge" => {
                expected["model"]["edges"]
                    .as_array_mut()
                    .unwrap()
                    .remove(second);
            }
            _ => unreachable!(),
        }
        assert_eq!(modified, expected, "{op}");
    }

    for op in ["script", "blank", "crlf", "deep", "unavailable"] {
        fs::write(&path, &original)?;
        mutate_artifact(op, &path)?;
        if op == "unavailable" {
            assert!(!path.exists());
            continue;
        }
        let modified = fs::read(&path)?;
        match op {
            "script" => assert!(
                !modified
                    .strip_prefix(original.as_slice())
                    .unwrap()
                    .is_empty()
            ),
            "blank" => assert!(modified.is_empty()),
            "crlf" => {
                let text = std::str::from_utf8(&modified)?;
                assert!(text.ends_with("\r\n"));
                assert_eq!(text.replace("\r\n", "\n").as_bytes(), original);
            }
            "deep" => assert_eq!(
                input::parse_json(std::str::from_utf8(&modified)?),
                Err(input::InputError::UnsupportedDepth)
            ),
            _ => unreachable!(),
        }
    }
    Ok(())
}

#[test]
fn case_errors_and_wrong_expectations_preserve_other_results_and_fail_the_command() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let fixtures = temp.path().join("fixtures");
    fs::create_dir(&fixtures)?;
    fs::write(fixtures.join("viewer-multigraph.json"), FIXTURE)?;
    let manifest = json!({"cases": [
        {"id":"case.bad_operation","base":"fixture","operation":"broken","expected":"UNKNOWN"},
        {"id":"case.malformed","base":"fixture","operation":null,"expected":"PASS"},
        {"id":"case.wrong_expected","base":"fixture","operation":"direction","expected":"PASS"},
        {"id":"case.control","base":"fixture","operation":"none","expected":"PASS"}
    ]});
    fs::write(
        temp.path().join("visualization-cases.yaml"),
        serde_json::to_vec(&manifest)?,
    )?;
    let output = temp.path().join("run.json");
    assert_eq!(
        command(temp.path(), Some(output.clone()))?,
        ExitCode::FAILURE
    );
    let receipt: Value = serde_json::from_slice(&fs::read(output)?)?;
    assert_eq!(receipt["matched"], 1);
    assert_eq!(receipt["total"], 4);
    let rows = &receipt["results"];
    assert_eq!(rows[0]["id"], "case.bad_operation");
    assert!(rows[0]["error"].as_str().unwrap().contains("broken"));
    assert_eq!(rows[1]["id"], "case.malformed");
    assert!(
        rows[1]["error"]
            .as_str()
            .unwrap()
            .contains("read visualization case definition")
    );
    for i in 0..2 {
        assert_eq!(rows[i]["matched"], false);
        assert!(rows[i].get("actual").is_none());
        assert!(rows[i].get("report").is_none());
    }
    assert_eq!(rows[2]["actual"], "FAIL");
    assert_eq!(rows[2]["matched"], false);
    assert_eq!(rows[2]["report"]["checks"][0]["result"], "FAIL");
    assert_eq!(rows[3]["matched"], true);
    Ok(())
}
