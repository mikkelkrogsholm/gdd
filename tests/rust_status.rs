use gdd::bundle;
use gdd::input;
use gdd::report::CheckResult;
use gdd::status::{
    ProjectStatus, StatusOptions, project_status, project_status_from_bundle, status_text,
};
use gdd::validation::{Activity, Profile, ValidationOptions, validate_with_options};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn cli(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gdd"))
        .arg("status")
        .arg(path)
        .args(args)
        .output()
        .unwrap()
}

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap()
}

fn schema_status(status: &ProjectStatus) -> Value {
    let report: Value =
        serde_json::from_str(include_str!("../schemas/report.schema.json")).unwrap();
    let id = report["$id"].as_str().unwrap().to_owned();
    let registry = jsonschema::Registry::new()
        .add(&id, report)
        .unwrap()
        .prepare()
        .unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/status.schema.json")).unwrap();
    let validator = jsonschema::draft202012::options()
        .offline()
        .with_registry(&registry)
        .build(&schema)
        .unwrap();
    let value = serde_json::to_value(status).unwrap();
    validator.validate(&value).unwrap();
    value
}

fn canonical(value: &Value) -> String {
    let mut value = value.clone();
    value.sort_all_objects();
    value.to_string()
}

#[test]
fn all_assessments_use_one_snapshot_even_after_the_ledger_and_contract_change() {
    let dir = tempfile::tempdir().unwrap();
    let example = Path::new("examples/agent-native");
    for name in ["model.gdd.json", "model.records.json", "REQUIREMENTS.md"] {
        fs::copy(example.join(name), dir.path().join(name)).unwrap();
    }
    let path = dir.path().join("model.gdd.json");
    let selected = [
        "action.tax".to_owned(),
        "action.quote".to_owned(),
        "action.tax".to_owned(),
    ];
    let host = input::parse(&fs::read(example.join("host.acceptance.json")).unwrap()).unwrap();
    let options = || StatusOptions {
        scope: &selected,
        host_context: Some(&host),
        activity: Activity::Acceptance,
        ..Default::default()
    };
    let captured = bundle::load_for_readiness(&path, &selected);
    let before = project_status(&path, options()).unwrap();
    assert_eq!(before.exit_code, 0);
    assert_eq!(
        before.assessments["authorization"].result,
        Some(CheckResult::Granted)
    );
    assert_eq!(
        before.assessments["implementation"].result,
        Some(CheckResult::Pass)
    );
    let json = schema_status(&before);
    assert_eq!(json["scope"], json!(["action.quote", "action.tax"]));
    assert_eq!(json["inputs"]["missing"], json!(["run"]));
    for (dimension, profile, activity) in [
        ("model", Profile::Model, None),
        ("readiness", Profile::Readiness, None),
        (
            "authorization",
            Profile::Records,
            Some(Activity::Acceptance),
        ),
        ("implementation", Profile::Records, None),
    ] {
        let scope = if profile == Profile::Model {
            selected.as_slice()
        } else {
            before.scope.as_slice()
        };
        let reference = validate_with_options(
            &captured,
            ValidationOptions {
                profiles: &[profile],
                scope,
                host_context: Some(&host),
                activity,
                ..Default::default()
            },
        )
        .unwrap();
        let mut expected = serde_json::to_value(reference).unwrap();
        expected["checks"]
            .as_array_mut()
            .unwrap()
            .sort_by_cached_key(canonical);
        assert_eq!(json["reports"][dimension], expected);
        for reference in &before.assessments[dimension].check_refs {
            assert_eq!(json.pointer(reference).unwrap()["dimension"], dimension);
        }
    }
    fs::write(dir.path().join("REQUIREMENTS.md"), "Changed after capture.").unwrap();
    write(
        &dir.path().join("model.records.json"),
        &json!({"format":"gdd-records/0.2","project_id":"quote-demo","records":{}}),
    );
    assert_eq!(
        json,
        serde_json::to_value(project_status_from_bundle(&captured, options()).unwrap()).unwrap()
    );
    assert_eq!(project_status(&path, options()).unwrap().exit_code, 1);
}

#[test]
fn unassessed_and_assessed_unknown_preserve_the_distinction_and_input_conflicts() {
    let path = Path::new("docs/tutorial/refined.gdd.json");
    let no_host = project_status(path, StatusOptions::default()).unwrap();
    assert_eq!(no_host.exit_code, 0);
    let alternate = cli(Path::new("./docs//tutorial/refined.gdd.json/"), &[]);
    assert_eq!(alternate.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<Value>(&alternate.stdout).unwrap(),
        serde_json::to_value(&no_host).unwrap()
    );
    let plain = schema_status(&no_host);
    assert_eq!(
        plain["assessments"]["authorization"],
        json!({"assessed":false,"result":null,"check_refs":[]})
    );
    assert!(plain["reports"].get("implementation").is_none());
    let empty = input::parse(b"{}").unwrap();
    let status = project_status(
        path,
        StatusOptions {
            host_context: Some(&empty),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(status.exit_code, 2);
    assert_eq!(
        status.assessments["authorization"].result,
        Some(CheckResult::Unresolved)
    );
    let host = input::parse(br#"{"implementation":"backend","code_revision":"v1"}"#).unwrap();
    let result = project_status(
        path,
        StatusOptions {
            host_context: Some(&host),
            code_revision: Some("v2"),
            ..Default::default()
        },
    );
    assert!(result.unwrap_err().to_string().contains("conflicts"));
    assert!(
        project_status(
            path,
            StatusOptions {
                host_context: Some(&input::Value::Boolean(false)),
                ..Default::default()
            }
        )
        .is_err()
    );
    let explicit = project_status(
        path,
        StatusOptions {
            code_revision: Some("v1"),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(explicit.exit_code, 2);
    assert!(!explicit.inputs.host_context_supplied);
    assert_eq!(
        explicit.assessments["implementation"].result,
        Some(CheckResult::Unknown)
    );
}

#[test]
fn starter_actions_and_questions_link_to_their_sources_and_text_quotes_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("team's draft");
    gdd::init_project(&root, "demo", "Draft").unwrap();
    let path = root.join("gdd.json");
    let mut model: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    model["questions"]["question.first"] = model["questions"]["question.scope"].clone();
    model["questions"]["question.first"]["statement"] = json!("Which version?");
    write(&path, &model);
    let scope = ["action.main".to_owned()];
    let status = project_status(
        &path,
        StatusOptions {
            scope: &scope,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(status.exit_code, 1);
    assert!(status.next_action_count > 12);
    assert_eq!(status.next_actions.len(), 12);
    let json = schema_status(&status);
    assert_eq!(json["open_questions"][0]["id"], "question.first");
    assert_eq!(json["open_questions"][1]["id"], "question.scope");
    assert_eq!(
        model
            .pointer(status.open_questions[0].location.pointer.as_str())
            .unwrap(),
        &status.open_questions[0].question
    );
    for action in &status.next_actions {
        let diagnostic = json.pointer(&action.source).unwrap();
        assert_eq!(diagnostic["message"], action.reason);
        assert_eq!(diagnostic["code"], action.diagnostic_code.unwrap());
        assert_eq!(
            diagnostic["location"],
            serde_json::to_value(&action.location).unwrap()
        );
    }
    let text = status_text(&status);
    assert!(text.contains("team'\"'\"'s draft/gdd.json'"));
    assert!(text.contains("Additional actions:"));
    let output = root.join("report.txt");
    let result = cli(
        &path,
        &[
            "--scope",
            "action.main",
            "--format",
            "text",
            "--output",
            output.to_str().unwrap(),
        ],
    );
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read_to_string(&output).unwrap(), text);
    let repeated = cli(
        &path,
        &["--format", "json", "--output", output.to_str().unwrap()],
    );
    assert_eq!(repeated.status.code(), Some(1));
    assert_eq!(fs::read_to_string(&output).unwrap(), text);
    let unsupported = cli(&path, &["--run", root.to_str().unwrap()]);
    assert_eq!(unsupported.status.code(), Some(2));
    assert!(
        String::from_utf8(unsupported.stderr)
            .unwrap()
            .contains("--run")
    );
}

#[test]
fn integration_outcomes_and_asset_locations_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("absent.gdd");
    let absent = project_status(&path, StatusOptions::default()).unwrap();
    assert_eq!(absent.exit_code, 2);
    let diagnostic = &absent.reports["model"]
        .checks
        .iter()
        .flat_map(|c| &c.diagnostics)
        .find(|d| d.code == "SOURCE_UNAVAILABLE")
        .unwrap();
    assert_eq!(diagnostic.location.file.as_deref(), Some("absent.gdd"));
    assert!(diagnostic.message.contains(path.to_str().unwrap()));
    let mut model: Value =
        serde_json::from_str(include_str!("../docs/tutorial/refined.gdd.json")).unwrap();
    let path = dir.path().join("model.json");
    let scope = ["action.add".to_owned()];
    for (uri, exit) in [("https://example.invalid/spec", 2), ("../outside", 1)] {
        model["sources"]["source.notes"] = json!({"role":"knowledge","title":"Notes","uri":uri,"format":"text/plain","normative":true,"revision":{"sha256":"0".repeat(64)}});
        write(&path, &model);
        let status = project_status(
            &path,
            StatusOptions {
                scope: &scope,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(status.spec_revision.is_none());
        assert_eq!(status.exit_code, exit);
        assert_eq!(
            status.assessments["readiness"].result,
            Some(CheckResult::Pass)
        );
    }
    model["sources"]["source.notes"]["normative"] = json!(false);
    model["sources"]["source.notes"]["uri"] = json!("absent.md");
    model["nodes"]["action.add"]["source_refs"] =
        json!([{"source":"source.notes","purpose":"contract"}]);
    write(&path, &model);
    let status = project_status(
        &path,
        StatusOptions {
            scope: &scope,
            ..Default::default()
        },
    )
    .unwrap();
    let source = status
        .next_actions
        .iter()
        .find(|a| a.diagnostic_code == Some("SOURCE_UNAVAILABLE"))
        .unwrap();
    assert!(source.reason.contains("absent.md"));
    assert!(source.reason.ends_with('.'));
    assert!(!source.reason.ends_with(".."));
}
