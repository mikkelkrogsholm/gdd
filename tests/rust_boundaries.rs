use gdd::boundaries::{PRINCIPLES, boundary_view, design_checklist};
use gdd::bundle::{self, Bundle};
use gdd::design_review::{DesignReviewStatus, ReviewStatus, review_design};
use gdd::input;
use gdd::projection::load_projection_bundle;
use gdd::validation::{self, Profile};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn cli(command: &str, path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gdd"))
        .arg(command)
        .arg(path)
        .args(args)
        .output()
        .unwrap()
}
fn concert() -> Value {
    let bytes = include_bytes!("../examples/concert-0.3.gdd.yaml");
    let mut model = validation::to_json(&input::parse(bytes).unwrap()).unwrap();
    model.as_object_mut().unwrap().remove("record_files");
    model
}
fn complete(bundle: &Bundle) -> Value {
    json!({"format":"gdd-design-review/0.3","spec_revision":bundle.snapshot().unwrap().revision,"implementation":"impl","code_revision":"rev1",
        "scope":["action.reserve"],"reviewer":{"name":"reviewer","version":"1"},"executed":true,"assessments":PRINCIPLES.iter().map(|(p,_)|json!({"principle":p,"status":"reviewed","rationale":"Reviewed the contract.","findings":[]})).collect::<Vec<_>>()})
}
fn review(bundle: &Bundle, value: &Value, scope: Option<&[String]>) -> DesignReviewStatus {
    let parsed = input::parse(&serde_json::to_vec(value).unwrap()).unwrap();
    review_design(bundle, Some(&parsed), Some("impl"), Some("rev1"), scope).unwrap()
}

#[test]
fn community_membership_and_call_omissions_remain_bound_to_the_capture() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let mut model = concert();
    write(&path, &model);
    let captured = load_projection_bundle(&path).unwrap();
    let all = boundary_view(&captured, None).unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/boundaries.schema.json")).unwrap();
    validation::build_schema(&schema)
        .unwrap()
        .validate(&all)
        .unwrap();
    let community = &all["communities"]["community.backend"];
    assert!(
        community["calls"]["inbound"]
            .as_array()
            .unwrap()
            .contains(&json!("edge.screen.reserve.calls"))
    );
    let feature = &all["communities"]["community.reservation_feature"];
    assert_eq!(
        feature["membership_basis"]["component.reservations"],
        json!(["action.reserve"])
    );
    assert!(
        feature["calls"]["internal"]
            .as_array()
            .unwrap()
            .contains(&json!("edge.screen.reserve.calls"))
    );
    let selected = vec![
        "action.reserve".to_string(),
        "community.backend".to_string(),
    ];
    let scoped = boundary_view(&captured, Some(&selected)).unwrap();
    assert!(scoped["components"].get("component.frontend").is_none());
    assert!(
        scoped["communities"]["community.backend"]["omitted_calls"]
            .as_array()
            .unwrap()
            .contains(&json!("edge.screen.reserve.calls"))
    );
    assert_eq!(
        boundary_view(&captured, Some(&[])).unwrap()["scope"],
        json!([])
    );
    assert!(boundary_view(&captured, Some(&["absent".into()])).is_err());
    let checklist = design_checklist(
        &captured,
        &["rule.design.dry".into(), "action.reserve".into()],
    )
    .unwrap();
    assert_eq!(
        checklist
            .iter()
            .map(|c| c["principle"].as_str().unwrap())
            .collect::<Vec<_>>(),
        PRINCIPLES.map(|p| p.0)
    );
    assert_eq!(checklist[1]["rules"], json!(["rule.design.dry"]));
    assert!(checklist.iter().all(|c| c["mode"] == "advisory"));
    model["edges"]
        .as_array_mut()
        .unwrap()
        .retain(|e| !(e["relation"] == "owned_by" && e["from"] == "action.reserve"));
    write(&path, &model);
    let changed = boundary_view(&load_projection_bundle(&path).unwrap(), None).unwrap();
    let call = changed["calls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["edge"] == "edge.screen.reserve.calls")
        .unwrap();
    assert_eq!(call["status"], "unresolved");
    assert!(call["provider"].is_null());
    assert!(
        changed["communities"]["community.reservation_feature"]["unresolved_members"]
            .as_array()
            .unwrap()
            .contains(&json!("action.reserve"))
    );
    fs::remove_file(&path).unwrap();
    assert_eq!(boundary_view(&captured, None).unwrap(), all);
}

#[test]
fn review_scope_principles_and_findings_are_advisory_and_revision_bound() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    write(&path, &concert());
    let bundle = load_projection_bundle(&path).unwrap();
    let mut input = complete(&bundle);
    input["assessments"][0]["findings"] = json!([{"targets":["action.reserve"],"consequence":"One extra dependency.","recommendation":"Review the dependency.","code_locations":[{"resource":"app.rs","selector":"reserve"}]}]);
    let result = review(&bundle, &input, None);
    assert_eq!(result.status, ReviewStatus::Complete);
    assert_eq!(result.exit_code(), 0);
    assert_eq!(result.findings[0]["principle"], "kiss");
    assert_eq!(
        result.review_identity.unwrap()["reviewer"],
        input["reviewer"]
    );
    assert_eq!(
        review(&bundle, &input, Some(&[])).status,
        ReviewStatus::Invalid
    );
    assert_eq!(
        review(&bundle, &input, Some(&["action.cancel".into()])).status,
        ReviewStatus::Unknown
    );
    let mut invalid = input.clone();
    invalid["assessments"][0]["findings"][0]["targets"] = json!(["screen.reserve"]);
    assert_eq!(
        review(&bundle, &invalid, None).reasons,
        ["Finding targets must belong to the review's model context."]
    );
    invalid = input.clone();
    invalid["assessments"][1]["principle"] = json!("kiss");
    assert_eq!(
        review(&bundle, &invalid, None).status,
        ReviewStatus::Invalid
    );
    let mut stale = input.clone();
    stale["assessments"].as_array_mut().unwrap().pop();
    stale["assessments"][0]["status"] = json!("unknown");
    stale["executed"] = json!(false);
    stale["code_revision"] = json!("old");
    assert_eq!(
        review(&bundle, &stale, None).reasons,
        [
            "Review of kiss is unresolved.",
            "All four design principles require an explicit assessment.",
            "The supplied document declares a planned review.",
            "Review belongs to another implementation or code revision."
        ]
    );
    let mut broken = concert();
    broken["nodes"]["component.reservations"]["interfaces"] = json!([]);
    write(&path, &broken);
    let untrusted = bundle::load_model_only(&path);
    let rejected = review(&untrusted, &input, None);
    assert_eq!(rejected.status, ReviewStatus::Unknown);
    assert_eq!(
        rejected.reasons,
        ["Model semantics are unresolved or invalid."]
    );
    assert_eq!(review(&bundle, &input, None).status, ReviewStatus::Complete);
}

#[test]
fn projections_check_loading_and_core_semantics_but_admit_historical_operational_records() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let ledger = dir.path().join("ledger.json");
    let mut model = concert();
    model["record_files"] = json!(["ledger.json"]);
    write(&path, &model);
    let project = model["project"]["id"].clone();
    write(
        &ledger,
        &json!({"format":"gdd-records/0.2","project_id":project,"records":{"check.old":{"kind":"check","spec_revision":format!("sha256:{}","0".repeat(64)),"scope":["old"],"method":"review","required_environment":[],"negative_controls":[]}}}),
    );
    let admitted = load_projection_bundle(&path).unwrap();
    assert_eq!(
        validation::validate(&admitted, Profile::Model).exit_code(),
        2
    );
    let output = dir.path().join("out.json");
    assert_eq!(
        cli("boundaries", &path, &["--output", output.to_str().unwrap()])
            .status
            .code(),
        Some(0)
    );
    let saved = fs::read(&output).unwrap();
    assert_eq!(
        cli("boundaries", &path, &["--output", output.to_str().unwrap()])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(fs::read(&output).unwrap(), saved);
    let review_path = dir.path().join("review.json");
    let args = [
        "--review",
        review_path.to_str().unwrap(),
        "--implementation",
        "impl",
        "--code-revision",
        "rev1",
    ];
    write(&review_path, &complete(&admitted));
    assert_eq!(cli("design-review", &path, &args).status.code(), Some(0));
    write(&review_path, &json!({}));
    let invalid = cli("design-review", &path, &args);
    assert_eq!(invalid.status.code(), Some(1));
    assert_eq!(
        serde_json::from_slice::<Value>(&invalid.stdout).unwrap()["status"],
        "INVALID"
    );
    write(&review_path, &json!({"format":"gdd-design-review/9"}));
    assert_eq!(cli("design-review", &path, &args).status.code(), Some(2));
    // Model admission runs before opening a supplied review or output file.
    fs::remove_file(&review_path).unwrap();
    fs::remove_file(&ledger).unwrap();
    let unavailable = cli("design-review", &path, &args);
    assert_eq!(unavailable.status.code(), Some(2));
    assert!(unavailable.stdout.is_empty());
    assert!(
        String::from_utf8(unavailable.stderr)
            .unwrap()
            .contains("SOURCE_UNAVAILABLE")
    );
    model.as_object_mut().unwrap().remove("record_files");
    model["sources"]["source.external"] = json!({"role":"input","title":"Unavailable normative source","uri":"https://example.invalid/x","format":"text","normative":true,"revision":{"sha256":"0".repeat(64)}});
    write(&path, &model);
    assert_eq!(load_projection_bundle(&path).unwrap_err().exit_code, 2);
    model["sources"]["source.external"]["uri"] = json!("../escape");
    write(&path, &model);
    assert_eq!(load_projection_bundle(&path).unwrap_err().exit_code, 1);
    let legacy = Path::new("examples/concert.gdd.yaml");
    assert_eq!(cli("boundaries", legacy, &[]).status.code(), Some(1));
    let old = cli(
        "design-review",
        legacy,
        &["--implementation", "impl", "--code-revision", "v1"],
    );
    assert_eq!(old.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&old.stdout).unwrap()["status"],
        "UNKNOWN"
    );
}
