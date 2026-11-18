use gdd::bundle;
use gdd::input::{self, Value};
use gdd::report::{CheckResult, Report};
use gdd::status::{
    StatusOptions, project_status_with_run, project_status_with_run_from_bundle, status_text,
};
use gdd::workflow_run::{PackagePhase, RunPhase, load_journal, run_status, run_status_from_bundle};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn fixture() -> &'static Path {
    Path::new("examples/agent-native")
}
fn json_file(path: &Path) -> Json {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn write(path: &Path, value: &Json) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn check(report: &Report, id: &str) -> CheckResult {
    report
        .checks
        .iter()
        .find(|c| c.check_id == id)
        .unwrap()
        .result
}
fn copy_journal(root: &Path, count: usize) -> PathBuf {
    let directory = root.join("run");
    fs::create_dir_all(directory.join("events")).unwrap();
    let src = fixture().join("native-run");
    fs::copy(src.join("run.json"), directory.join("run.json")).unwrap();
    for n in 1..=count {
        let name = format!("events/{n:06}.json");
        fs::copy(src.join(&name), directory.join(name)).unwrap();
    }
    directory
}

#[test]
fn archived_acceptance_uses_the_same_captured_model_ledger_and_event_head() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["model.gdd.json", "model.records.json", "REQUIREMENTS.md"] {
        fs::copy(fixture().join(name), dir.path().join(name)).unwrap();
    }
    let path = dir.path().join("model.gdd.json");
    let directory = copy_journal(dir.path(), 10);
    let journal = load_journal(&directory).unwrap();
    let model = bundle::load(&path);
    let host = input::parse(&fs::read(fixture().join("host.acceptance.json")).unwrap()).unwrap();
    let status = run_status_from_bundle(&model, &journal, Some(&host)).unwrap();
    assert!(status.acceptance_current);
    assert_eq!(status.event_count, 10);
    assert_eq!(status.state.status, RunPhase::Accepted);
    assert_eq!(
        status.state.attempts,
        ["native.discount.1", "native.tax.1", "native.quote.1"]
    );
    assert_eq!(
        status.state.packages["package.tax"].status,
        PackagePhase::Integrated
    );
    assert_eq!(status.report.exit_code(), 0);
    assert_eq!(
        status.revision,
        json_file(&directory.join("events/000010.json"))["revision"]
    );
    assert!(status.available_packages.is_empty());

    let mut conflict = json_file(&fixture().join("host.acceptance.json"));
    conflict["check_selection"] = json!({"rule.discount_floor":{"stdlib_unittest":"check.other"}});
    let rejected = run_status_from_bundle(&model, &journal, Some(&Value::from(&conflict))).unwrap();
    assert_eq!(
        check(&rejected.report, "workflow.check_selection"),
        CheckResult::Fail
    );
    assert_eq!(rejected.report.result("implementation"), CheckResult::Pass);
    assert!(!rejected.acceptance_current);

    let ledger = dir.path().join("model.records.json");
    let mut records = json_file(&ledger);
    records["records"]
        .as_object_mut()
        .unwrap()
        .remove("evidence.demo.final");
    write(&ledger, &records);
    let current = run_status(&path, &directory, Some(&host)).unwrap();
    assert_eq!(current.state.status, RunPhase::Accepted);
    assert_eq!(current.revision, status.revision);
    assert_eq!(current.report.spec_revision, status.report.spec_revision);
    assert!(!current.acceptance_current);

    let options = || StatusOptions {
        scope: &[],
        host_context: Some(&host),
        ..Default::default()
    };
    let project = project_status_with_run_from_bundle(&model, &directory, options()).unwrap();
    assert_eq!(
        project.workflow.as_ref().unwrap()["acceptance_current"],
        true
    );
    assert!(status_text(&project).contains("Workflow: accepted; acceptance_current=true"));
    assert!(status_text(&project).contains("package.quote: integrated; attempt=native.quote.1"));
    fs::remove_dir_all(&directory).unwrap();
    fs::remove_file(&path).unwrap();
    fs::remove_file(dir.path().join("REQUIREMENTS.md")).unwrap();
    let replay = run_status_from_bundle(&model, &journal, Some(&host)).unwrap();
    assert_eq!(
        serde_json::to_value(replay).unwrap(),
        serde_json::to_value(status).unwrap()
    );
}

// Test receipts contain only strings and arrays of strings. Equal sorted
// compact UTF-8 bytes give the independent recorded hash for each transition.
fn receipt(directory: &Path, n: usize, previous: &str, mut event: Json) -> String {
    event["expected_revision"] = json!(previous);
    event["idempotency_key"] = json!(format!("event.{n}"));
    let mut entry =
        json!({"format":"gdd-run-event/0.1","previous_revision":previous,"event":event});
    let head = seal(&mut entry);
    write(&directory.join(format!("events/{n:06}.json")), &entry);
    head
}

fn seal(entry: &mut Json) -> String {
    entry.sort_all_objects();
    let head = format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(entry).unwrap())
    );
    entry["revision"] = json!(head);
    head
}

#[test]
fn replay_preserves_late_results_retry_history_and_requires_file_ownership() {
    let temp = tempfile::tempdir().unwrap();
    let directory = copy_journal(temp.path(), 1);
    let path = fixture().join("model.gdd.json");
    let first = json_file(&directory.join("events/000001.json"));
    let initial = run_status(&path, &directory, None).unwrap();
    assert_eq!(initial.available_packages, ["package.tax"]);
    fs::write(directory.join("events/.pending-write"), "{").unwrap();
    let cancelled = receipt(
        &directory,
        2,
        first["revision"].as_str().unwrap(),
        json!({"type":"cancel","reason":"Pause integration."}),
    );
    let submit = json_file(&fixture().join("native-run/events/000003.json"))["event"].clone();
    let submitted = receipt(&directory, 3, &cancelled, submit.clone());
    let late = run_status(&path, &directory, None).unwrap();
    assert_eq!(late.state.status, RunPhase::Cancelled);
    assert_eq!(
        late.state.packages["package.discount"].status,
        PackagePhase::Submitted
    );
    assert!(late.available_packages.is_empty());
    let resumed = receipt(
        &directory,
        4,
        &submitted,
        json!({"type":"resume","reason":"Resume work."}),
    );
    let retry = receipt(
        &directory,
        5,
        &resumed,
        json!({"type":"retry","reason":"Repeat review.","package":"package.discount"}),
    );
    let retried = run_status(&path, &directory, None).unwrap();
    assert_eq!(
        retried.available_packages,
        ["package.discount", "package.tax"]
    );
    assert!(retried.state.packages["package.discount"].result.is_none());
    receipt(&directory, 6, &retry, first["event"].clone());
    assert!(
        load_journal(&directory)
            .unwrap_err()
            .to_string()
            .contains("Attempt tokens cannot be reused")
    );
    let mut next = first["event"].clone();
    next["attempt"] = json!("generation.2");
    let dispatched = receipt(&directory, 6, &retry, next);
    let mut changed = submit;
    changed["attempt"] = json!("generation.2");
    changed["changed_resources"] = json!(["tests/"]);
    receipt(&directory, 7, &dispatched, changed);
    assert!(
        load_journal(&directory)
            .unwrap_err()
            .to_string()
            .contains("write ownership")
    );
    fs::remove_file(directory.join("events/000006.json")).unwrap();
    assert!(
        load_journal(&directory)
            .unwrap_err()
            .to_string()
            .contains("gap")
    );
}

#[test]
fn journal_errors_preserve_independent_assessments_and_exit_precedence() {
    let temp = tempfile::tempdir().unwrap();
    let directory = copy_journal(temp.path(), 2);
    let path = fixture().join("model.gdd.json");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_gdd"))
            .args(["workflow", "status"])
            .arg(&path)
            .arg("--run")
            .arg(&directory)
            .output()
            .unwrap()
    };
    let p = directory.join("events/000002.json");
    let original = fs::read(&p).unwrap();
    let mut tampered = json_file(&p);
    tampered["event"]["attempt"] = json!("tampered");
    write(&p, &tampered);
    for (bytes, code, category, combined_exit, direct_exit) in [
        (fs::read(&p).unwrap(), "RECORD_INVALID", "FAIL", 1, 1),
        (
            [vec![b'['; 300], b"0".to_vec(), vec![b']'; 300]].concat(),
            "FORMAT_UNSUPPORTED",
            "UNKNOWN",
            2,
            2,
        ),
        (b"{".to_vec(), "YAML_FEATURE_FORBIDDEN", "FAIL", 1, 1),
        (
            b"{\"x\":1,\"x\":2}".to_vec(),
            "YAML_DUPLICATE_KEY",
            "FAIL",
            1,
            1,
        ),
    ] {
        fs::write(&p, bytes).unwrap();
        let status = project_status_with_run(&path, &directory, StatusOptions::default()).unwrap();
        assert_eq!(status.assessments["model"].result, Some(CheckResult::Pass));
        assert_eq!(
            status.assessments["readiness"].result,
            Some(CheckResult::Pass)
        );
        assert!(status.workflow.is_none());
        let error = status.workflow_error.as_ref().unwrap();
        assert_eq!(error["check"]["result"], category);
        assert_eq!(error["check"]["diagnostics"][0]["code"], code);
        assert_eq!(status.exit_code, combined_exit);
        let action = status
            .next_actions
            .iter()
            .find(|a| a.diagnostic_code == Some(code))
            .unwrap();
        assert_eq!(action.source, "/workflow_error/check/diagnostics/0");
        assert_eq!(action.argv[1..3], ["workflow", "status"]);
        let output = run();
        assert!(output.stdout.is_empty());
        assert_eq!(output.status.code(), Some(direct_exit));
    }
    fs::write(&p, &original).unwrap();
    let destination = temp.path().join("status.json");
    let cli = || {
        Command::new(env!("CARGO_BIN_EXE_gdd"))
            .args(["workflow", "status"])
            .arg(&path)
            .arg("--run")
            .arg(&directory)
            .arg("--output")
            .arg(&destination)
            .output()
            .unwrap()
    };
    assert!(cli().status.success());
    let stored = fs::read(&destination).unwrap();
    assert_eq!(
        serde_json::from_slice::<Json>(&stored).unwrap()["event_count"],
        2
    );
    assert!(!cli().status.success());
    assert_eq!(fs::read(destination).unwrap(), stored);
    assert_eq!(fs::read(p).unwrap(), original);
}

#[test]
fn acceptance_questions_require_a_trusted_applicable_resolution() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("model.json");
    write(
        &path,
        &json!({
            "format":"gdd/0.2","project":{"id":"project.x","title":"Item","goal":"Implement an item."},
            "nodes":{"entity.x":{"kind":"entity","description":"Item.","fields":{}}}, "edges":[],
            "record_files":["records.json"],
            "questions":{"question.x":{"statement":"Accept the item?","targets":["entity.x"],"blocks":["acceptance"],
                "status":"resolved","resolution":{"decision":"decision.review"}}}
        }),
    );
    let ledger = temp.path().join("records.json");
    let mut records = json!({"format":"gdd-records/0.2","project_id":"project.x","records":{}});
    write(&ledger, &records);
    let captured = bundle::load_model_only(&path);
    let decision = json!({"kind":"decision","topic":"model_change","scope":["entity.x"],"spec_revision":captured.snapshot().unwrap().revision,
        "policy_revision":"policy.1","actor":{"id":"actor.reviewer","kind":"human"},"outcome":"accepted","rationale":"Reviewed."});
    records["records"]["decision.review"] = decision.clone();
    let mut retired = decision;
    retired["supersedes"] = json!("decision.review");
    retired["outcome"] = json!("rejected");
    records["records"]["decision.replacement"] = retired;
    write(&ledger, &records);
    let model = bundle::load(&path);
    let request = Value::from(&json!({
        "format":"gdd-work-request/0.1","id":"request.x","objective":"Implement the item.",
        "implementation":"sample","base_code_revision":"base","serialized":[],"integration":{"owner":"owner","checks":[]},
        "packages":[{"id":"package.one","scope":["entity.x"],"workspace":"work","write_resources":["item.txt"],
            "read_resources":[],"deliverables":["Item"],"integration_plan":"Inspect the item."}]
    }));
    let plan = gdd::workflow::prepare_workflow_from_bundle(&model, &request, None)
        .unwrap()
        .artifact;
    let dir = temp.path().join("journal");
    fs::create_dir_all(dir.join("events")).unwrap();
    let mut manifest = json!({"format":"gdd-run/0.1","plan":plan});
    let mut head = seal(&mut manifest);
    write(&dir.join("run.json"), &manifest);
    let events = [
        json!({"type":"dispatch","package":"package.one","attempt":"a"}),
        json!({"type":"submit","package":"package.one","attempt":"a","changed_resources":["item.txt"],"summary":"Item.","artifact_uri":"item.txt"}),
        json!({"type":"integrate","package":"package.one","attempt":"a","verified_resources":["item.txt"],"base_code_revision":"base","code_revision":"integrated",
            "review":{"reviewer":"reviewer","verdict":"PASS","evidence_uri":"review.txt"}}),
        json!({"type":"accept"}),
    ];
    for (index, event) in events.into_iter().enumerate() {
        head = receipt(&dir, index + 1, &head, event);
    }
    let journal = load_journal(&dir).unwrap();
    for (trusted, expected) in [
        (json!([]), CheckResult::Unknown),
        (json!(["decision.review"]), CheckResult::Pass),
        (
            json!(["decision.review", "decision.replacement"]),
            CheckResult::Unknown,
        ),
    ] {
        let host = Value::from(&json!({"trusted_decisions":trusted}));
        let status = run_status_from_bundle(&model, &journal, Some(&host)).unwrap();
        assert_eq!(
            check(&status.report, "workflow.question.question.x"),
            expected
        );
    }
}
