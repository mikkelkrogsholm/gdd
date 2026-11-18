use gdd::input::{self, Value};
use gdd::report::CheckResult;
use gdd::workflow::prepare_workflow;
use gdd::workflow_run::{RunPhase, advance_run, create_run, load_journal, run_status};
use serde_json::{Value as Json, json};
use std::{fs, path::Path, process::Command, sync::Barrier, thread};

fn fixture() -> &'static Path {
    Path::new("examples/agent-native")
}
fn read(path: &Path) -> Json {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn parsed(path: &Path) -> Value {
    input::parse(&fs::read(path).unwrap()).unwrap()
}
fn write(path: &Path, value: &Json) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn package_request(target: &str) -> Value {
    Value::from(
        &json!({"format":"gdd-work-request/0.1","id":"work.sample","objective":"Implement the scope.",
        "implementation":"sample","base_code_revision":"base", "packages":[{
            "id":"package.one","scope":[target],"workspace":"work","read_resources":[],"write_resources":["item"],
            "deliverables":["Item"],"integration_plan":"Review the item."}],
        "serialized":[],"integration":{"owner":"owner","checks":[]}}),
    )
}

#[test]
fn publishes_archived_receipts_checks_current_host_and_never_redispatches_on_retry() {
    let dir = tempfile::tempdir().unwrap();
    let run = dir.path().join("queue");
    let model = fixture().join("model.gdd.json");
    let plan = parsed(&fixture().join("workflow.plan.json"));
    let archived = fixture().join("native-run");
    let initial = create_run(&model, &plan, &run, None).unwrap();
    assert_eq!(
        initial.available_packages,
        ["package.discount", "package.tax"]
    );
    assert_eq!(
        read(&run.join("run.json")),
        read(&archived.join("run.json"))
    );
    let mut host = read(&fixture().join("host.implementation.json"));
    let acceptance = read(&fixture().join("host.acceptance.json"));
    let mut first_event = None;
    for index in 1..=10 {
        let name = format!("events/{index:06}.json");
        let original = read(&archived.join(&name));
        let event = &original["event"];
        let kind = event["type"].as_str().unwrap();
        if kind == "integrate" {
            // The old code revision is insufficient to attest the candidate integration.
            if host["code_revision"] != event["code_revision"] {
                let denied =
                    advance_run(&model, &run, &Value::from(event), Some(&Value::from(&host)))
                        .unwrap();
                assert!(!denied.applied && !denied.replayed);
                assert_ne!(denied.exit_code(), 0);
                assert!(!run.join(&name).exists());
            }
            host["code_revision"] = event["code_revision"].clone();
        }
        let context = Value::from(if kind == "accept" { &acceptance } else { &host });
        if index == 1 {
            first_event = Some(Value::from(event));
            let blocked = advance_run(&model, &run, &Value::from(event), None).unwrap();
            assert!(!blocked.applied && !blocked.replayed);
            assert_eq!(
                blocked.report.as_ref().unwrap().result("authorization"),
                CheckResult::Unresolved
            );
            assert_eq!(run.join("events").read_dir().unwrap().count(), 0);
        }
        let result = advance_run(&model, &run, &Value::from(event), Some(&context)).unwrap();
        assert!(result.applied && !result.replayed);
        assert_eq!(result.exit_code(), 0);
        assert_eq!(result.receipt.as_ref().unwrap(), &original);
        assert_eq!(read(&run.join(name)), original);
        assert_eq!(result.artifact.is_some(), kind == "dispatch");
        if let Some(packet) = result.artifact {
            assert_eq!(packet["format"], "gdd-dispatch/0.1");
            assert_eq!(packet["attempt"], event["attempt"]);
            assert_eq!(packet["package"]["id"], event["package"]);
        }
    }
    let status = run_status(&model, &run, Some(&Value::from(&acceptance))).unwrap();
    assert!(status.acceptance_current);
    let replay = advance_run(
        &model,
        &run,
        first_event.as_ref().unwrap(),
        Some(&Value::from(&acceptance)),
    )
    .unwrap();
    assert!(replay.replayed && !replay.applied);
    assert!(replay.artifact.is_none());
    assert_eq!(
        replay.status.as_ref().unwrap().state.status,
        RunPhase::Accepted
    );
    assert_eq!(
        replay.receipt.unwrap(),
        read(&archived.join("events/000001.json"))
    );
    assert_eq!(load_journal(&run).unwrap().receipts().len(), 10);
}

#[test]
fn competing_creators_and_events_leave_one_complete_publication() {
    let dir = tempfile::tempdir().unwrap();
    let run = dir.path().join("queue");
    let model = fixture().join("model.gdd.json");
    let plan = parsed(&fixture().join("workflow.plan.json"));
    let gate = Barrier::new(4);
    let creators = thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    gate.wait();
                    create_run(&model, &plan, &run, None)
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(creators.iter().filter(|r| r.is_ok()).count(), 1);
    let revision = &creators
        .iter()
        .find_map(|r| r.as_ref().ok())
        .unwrap()
        .revision;
    let event = |index| {
        Value::from(&json!({"type":"cancel","expected_revision":revision,
        "idempotency_key":format!("cancel.{index}"),"reason":"Stop after review."}))
    };
    let attempts = thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|i| {
                let e = event(i);
                let inputs = (&gate, &model, &run);
                scope.spawn(move || {
                    inputs.0.wait();
                    advance_run(inputs.1, inputs.2, &e, None)
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(attempts.iter().filter(|r| r.is_ok()).count(), 1);
    let winner = attempts.iter().position(|r| r.is_ok()).unwrap();
    let replay = advance_run(&model, &run, &event(winner), None).unwrap();
    assert!(replay.replayed);
    assert_eq!(run.join("events").read_dir().unwrap().count(), 1);
    assert_eq!(dir.path().read_dir().unwrap().count(), 1);

    // An existing empty directory also belongs to its caller.
    let existing = dir.path().join("existing");
    fs::create_dir(&existing).unwrap();
    assert!(create_run(&model, &plan, &existing, None).is_err());
    assert_eq!(existing.read_dir().unwrap().count(), 0);
    assert_eq!(dir.path().read_dir().unwrap().count(), 2);
}

#[test]
fn drafts_publish_with_failing_readiness_and_stale_packets_roll_back() {
    let dir = tempfile::tempdir().unwrap();
    let model = Path::new("templates/project/gdd.json");
    let prepared = prepare_workflow(model, &package_request("action.main"), None).unwrap();
    let mut altered = prepared.artifact.clone();
    altered["notice"] = json!("Different notice.");
    let run = dir.path().join("queue");
    assert!(
        create_run(model, &Value::from(&altered), &run, None)
            .unwrap_err()
            .to_string()
            .contains("stale or altered")
    );
    assert_eq!(dir.path().read_dir().unwrap().count(), 0);
    let current = create_run(model, &Value::from(&prepared.artifact), &run, None).unwrap();
    assert_eq!(current.report.result("readiness"), CheckResult::Fail);
    assert!(current.available_packages.is_empty());
    let cancel = json!({"type":"cancel","idempotency_key":"stop","expected_revision":current.revision,"reason":"Stop."});
    let absent = dir.path().join("missing-model.json");
    let result = advance_run(&absent, &run, &Value::from(&cancel), None).unwrap();
    assert!(result.applied);
    assert_eq!(result.report.as_ref().unwrap().exit_code(), 2);
    assert_eq!(result.exit_code(), 0);
    let mut reused = cancel.clone();
    reused["reason"] = json!("Changed event under the same key.");
    assert!(
        advance_run(&absent, &run, &Value::from(&reused), None)
            .unwrap_err()
            .to_string()
            .contains("different request")
    );
    let mut stale = cancel.clone();
    stale["idempotency_key"] = json!("another.stop");
    assert!(
        advance_run(&absent, &run, &Value::from(&stale), None)
            .unwrap_err()
            .to_string()
            .contains("head changed")
    );
    assert_eq!(load_journal(&run).unwrap().receipts().len(), 1);
}

#[test]
fn cli_exclusive_output_precedes_publication_and_carries_only_the_result() {
    let dir = tempfile::tempdir().unwrap();
    let run = dir.path().join("queue");
    let output = dir.path().join("result.json");
    let model = fixture().join("model.gdd.json");
    let plan = fixture().join("workflow.plan.json");
    let create = |plan: &Path| {
        Command::new(env!("CARGO_BIN_EXE_gdd"))
            .args(["workflow", "create"])
            .arg(&model)
            .arg("--output")
            .arg(&output)
            .arg("--run")
            .arg(&run)
            .arg("--plan")
            .arg(plan)
            .output()
            .unwrap()
    };
    fs::write(&output, "owned by caller").unwrap();
    let refusal = create(&dir.path().join("absent-plan"));
    assert_eq!(refusal.status.code(), Some(1));
    assert!(
        String::from_utf8(refusal.stderr)
            .unwrap()
            .contains("Output already exists")
    );
    assert!(!run.exists());
    assert_eq!(fs::read_to_string(&output).unwrap(), "owned by caller");
    fs::remove_file(&output).unwrap();
    let first = create(&plan);
    assert!(first.status.success());
    assert!(first.stdout.is_empty());
    assert_eq!(read(&output)["format"], "gdd-run-status/0.1");
    let e = dir.path().join("event.json");
    write(
        &e,
        &read(&fixture().join("native-run/events/000001.json"))["event"],
    );
    let send = || {
        Command::new(env!("CARGO_BIN_EXE_gdd"))
            .args(["workflow", "event"])
            .arg(&model)
            .arg("--event")
            .arg(&e)
            .arg("--run")
            .arg(&run)
            .arg("--output")
            .arg(&output)
            .arg("--host-context")
            .arg(fixture().join("host.implementation.json"))
            .output()
            .unwrap()
    };
    let refused = send();
    assert!(!refused.status.success());
    assert_eq!(load_journal(&run).unwrap().receipts().len(), 0);
    fs::remove_file(&output).unwrap();
    let applied = send();
    assert!(applied.status.success());
    assert!(applied.stdout.is_empty());
    assert_eq!(read(&output)["applied"], true);
    fs::remove_file(&output).unwrap();
    assert!(send().status.success());
    let replay = read(&output);
    assert_eq!(replay["replayed"], true);
    assert!(replay.get("artifact").is_none());
}

#[test]
fn workflow_publication_preserves_historical_depth_outcomes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let mut manifest = read(Path::new("docs/tutorial/refined.gdd.json"));
    manifest["nodes"]["action.add"]["annotations"] = json!({"deep.test":"DEPTH_SENTINEL"});
    let raw = serde_json::to_string(&manifest).unwrap();
    let req = dir.path().join("request.json");
    write(
        &req,
        &gdd::validation::to_json(&package_request("component.arithmetic")).unwrap(),
    );
    let workflow = |command: &str, options: &[(&str, &Path)]| {
        let mut process = Command::new(env!("CARGO_BIN_EXE_gdd"));
        process.args(["workflow", command]).arg(&path);
        for (name, value) in options {
            process.arg(name).arg(value);
        }
        process.output().unwrap()
    };
    for depth in 249..=252 {
        let arrays = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        fs::write(&path, raw.replace("\"DEPTH_SENTINEL\"", &arrays)).unwrap();
        let plan = dir.path().join(format!("plan-{depth}.json"));
        let generated = workflow("plan", &[("--request", &req), ("--output", &plan)]);
        assert!(generated.status.success(), "{:?}", generated.stderr);
        assert!(plan.is_file());
        let verified = workflow("verify", &[("--plan", &plan)]);
        if depth == 249 {
            assert!(verified.status.success());
            let run = dir.path().join("queue");
            let created = workflow("create", &[("--plan", &plan), ("--run", &run)]);
            assert_eq!(created.status.code(), Some(2));
            assert!(run.join("run.json").is_file());
            assert!(run.join("events").is_dir());
            assert_eq!(
                workflow("status", &[("--run", &run)]).status.code(),
                Some(2)
            );
        } else {
            assert_eq!(verified.status.code(), Some(2));
        }
    }
}
