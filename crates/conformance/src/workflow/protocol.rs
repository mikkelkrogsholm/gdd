//! Real journal operations exercised with independently controlled synthetic host inputs.

use super::probe::Probe;
use super::*;
use gdd::{
    workflow::{prepare_workflow, verify_workflow},
    workflow_run::{PackagePhase, RunPhase, advance_run, create_run, run_status},
};
use std::{io::ErrorKind, sync::Barrier, thread};

pub(super) fn host_shape(_: &Path, parameter: &str) -> Result<()> {
    let (operation, value) = parameter
        .split_once(' ')
        .context("missing malformed host")?;
    let bad = Input::from(&serde_json::from_str::<Value>(value)?);
    let case = Probe::new(false, false)?;
    let request = Input::from(&case.request);
    let plan = Input::from(&case.plan()?);
    if matches!(operation, "status" | "event") {
        case.start()?;
    }
    let result = match operation {
        "prepare" => prepare_workflow(&case.path, &request, Some(&bad)).map(drop),
        "verify" => verify_workflow(&case.path, &plan, Some(&bad)).map(drop),
        "create" => create_run(&case.path, &plan, &case.run, Some(&bad)).map(drop),
        "status" => run_status(&case.path, &case.run, Some(&bad))
            .map(drop)
            .map_err(std::io::Error::from),
        "event" => {
            let event = case.event(json!({"type":"cancel","reason":"Stop"}))?;
            advance_run(&case.path, &case.run, &Input::from(&event), Some(&bad)).map(drop)
        }
        _ => anyhow::bail!("unknown host API {operation}"),
    };
    let error = result.err().context("API accepted a malformed host")?;
    ensure!(
        error.to_string().contains("Invalid host input"),
        "expected invalid-host error, got {error}"
    );
    if operation == "create" {
        ensure!(!case.run.exists());
    }
    if operation == "event" {
        ensure!(case.status()?.event_count == 0);
    }
    Ok(())
}

pub(super) fn publication(_: &Path, defect: &str) -> Result<()> {
    let case = Probe::new(false, false)?;
    let mut plan = case.plan()?;
    match defect {
        "missing-packet-fields" => plan["packages"] = json!([{}]),
        "altered-slice" => {
            plan["packages"][0]["model_slice"]["nodes"]["entity.x"]["description"] =
                json!("Altered intention")
        }
        "stale-model" => {
            let mut model = read_document(&case.path)?;
            model["nodes"]["entity.x"]["description"] = json!("New model revision");
            write_json(&case.path, &model)?;
        }
        _ => anyhow::bail!("unknown publication defect {defect}"),
    }
    plan.as_object_mut()
        .context("missing plan")?
        .remove("revision")
        .context("missing plan revision")?;
    plan["revision"] = json!(gdd::canonical_digest(&plan));
    let result = create_run(
        &case.path,
        &Input::from(&plan),
        &case.run,
        Some(&Input::from(&case.host)),
    );
    ensure!(result.is_err_and(|e| e.kind() == ErrorKind::InvalidInput));
    ensure!(!case.run.exists(), "invalid packet published a run");
    Ok(())
}

pub(super) fn unresolved_model(_: &Path, defect: &str) -> Result<()> {
    let case = Probe::new(false, false)?;
    let mut model = read_document(&case.path)?;
    match defect {
        "edge" => model["edges"]
            .as_array_mut()
            .context("missing edges")?
            .push(json!({
            "id":"edge.bad","from":"rule.x","relation":"constrains","to":"entity.missing"})),
        "type" => {
            model["nodes"]["entity.x"]["fields"] =
                json!({"v":{"type":"reference","target":"entity.missing"}})
        }
        _ => anyhow::bail!("unknown dangling reference {defect}"),
    }
    write_json(&case.path, &model)?;
    let report = validation::validate_path(
        &case.path,
        ValidationOptions {
            profiles: &[Profile::Model],
            ..Default::default()
        },
        None,
    )?;
    ensure!(report.result("model") == CheckResult::Fail);
    let attempted = prepare_workflow(
        &case.path,
        &Input::from(&case.request),
        Some(&Input::from(&case.host)),
    );
    ensure!(attempted.is_err_and(|e| e.kind() == ErrorKind::InvalidInput));
    Ok(())
}

pub(super) fn historical_records(_: &Path, parameter: &str) -> Result<()> {
    let (defect, count) = parameter.split_once(' ').context("missing package count")?;
    let multi = match count {
        "multi" => true,
        "single" => false,
        _ => anyhow::bail!("unknown package count {count}"),
    };
    let mut case = Probe::new(multi, false)?;
    match defect {
        "historical" | "unused-current-invalid" => {
            let revision = if defect == "historical" {
                json!(format!("sha256:{}", "a".repeat(64)))
            } else {
                case.host["policy"]["spec_revision"].clone()
            };
            case.records["decision.unrelated"] = json!({"kind":"decision","topic":"model_change",
                "scope":["entity.deleted"],"spec_revision":revision,"policy_revision":"policy.old",
                "actor":{"id":"actor.probe","kind":"human"},"outcome":"accepted","rationale":"Unrelated synthetic record"});
            case.write_records()?;
        }
        "none" => {}
        _ => anyhow::bail!("unknown record defect {defect}"),
    }
    let status = case.start()?;
    ensure!(status.report.result("readiness") == CheckResult::Pass);
    let expected: &[&str] = if multi {
        &["package.x", "package.y"]
    } else {
        &["package.x"]
    };
    ensure!(status.available_packages == expected);
    ensure!(case.dispatch("attempt.1")?.applied);
    Ok(())
}

pub(super) fn draft(_: &Path, _: &str) -> Result<()> {
    let mut case = Probe::new(false, false)?;
    let mut model = read_document(&case.path)?;
    model["nodes"]["entity.x"]
        .as_object_mut()
        .context("missing entity")?
        .remove("fields")
        .context("missing fields")?;
    write_json(&case.path, &model)?;
    let spec = revision(&case.path)?;
    case.host["policy"]["spec_revision"] = json!(spec);
    case.records["check.x"]["spec_revision"] = json!(spec);
    case.write_records()?;
    let status = case.start()?;
    ensure!(case.run.is_dir() && status.available_packages.is_empty());
    ensure!(!case.dispatch("attempt.1")?.applied);
    let after = case.status()?;
    ensure!(after.revision == status.revision && after.event_count == 0);
    Ok(())
}

pub(super) fn current_acceptance(_: &Path, variant: &str) -> Result<()> {
    let mut case = Probe::new(false, variant == "acceptance-question")?;
    case.submitted()?;
    ensure!(case.step(Probe::integration())?.applied);
    if variant == "no-evidence" {
        case.host["policy"]["activity"] = json!("acceptance");
    } else {
        case.attach_evidence()?;
    }
    match variant {
        "untrusted" => case.host["trusted_evidence"] = json!([]),
        "wrong-code" => case.host["code_revision"] = json!("code.other"),
        "wrong-implementation" => case.host["implementation"] = json!("implementation.other"),
        "wrong-activity" => case.host["policy"]["activity"] = json!("implementation"),
        "denied" => case.host["policy"]["denied"] = json!(true),
        "check-selection" => {
            case.host["check_selection"] = json!({"rule.x":{"probe":"check.other"}})
        }
        "hash-mismatch" => {
            case.records["evidence.x"]["check_revision"] =
                json!(format!("sha256:{}", "a".repeat(64)));
            case.write_records()?;
        }
        "valid" | "no-evidence" | "acceptance-question" => {}
        _ => anyhow::bail!("unknown acceptance variant {variant}"),
    }
    let before = case.status()?;
    let accepted = case.step(json!({"type":"accept"}))?;
    ensure!(accepted.applied == (variant == "valid"));
    let status = case.status()?;
    if variant == "valid" {
        ensure!(status.state.status == RunPhase::Accepted && status.acceptance_current);
        case.host["code_revision"] = json!("code.changed");
        let stale = case.status()?;
        ensure!(
            stale.state.status == RunPhase::Accepted
                && !stale.acceptance_current
                && stale.revision == status.revision
        );
    } else {
        ensure!(before.revision == status.revision && !status.acceptance_current);
    }
    Ok(())
}

pub(super) fn integration(_: &Path, variant: &str) -> Result<()> {
    let mut case = Probe::new(false, false)?;
    case.submitted()?;
    let mut fields = Probe::integration();
    match variant {
        "valid" => {}
        "denied" => case.host["policy"]["denied"] = json!(true),
        "wrong-code" => case.host["code_revision"] = json!("code.other"),
        "wrong-implementation" => case.host["implementation"] = json!("implementation.other"),
        "wrong-base" => fields["base_code_revision"] = json!("code.wrong"),
        "out-of-scope" => fields["verified_resources"] = json!(["outside.py"]),
        _ => anyhow::bail!("unknown integration variant {variant}"),
    };
    let before = case.status()?;
    let result = case.advance(&case.event(fields)?);
    if matches!(variant, "wrong-base" | "out-of-scope") {
        let error = result.err().context("invalid integration was accepted")?;
        ensure!(
            error
                .get_ref()
                .and_then(|e| e.downcast_ref::<gdd::workflow_run::JournalError>())
                .is_some_and(|error| error.code == "RECORD_INVALID"),
            "unexpected error {error}"
        );
    } else {
        ensure!(result?.applied == (variant == "valid"));
    }
    let status = case.status()?;
    if variant == "valid" {
        ensure!(
            status.state.packages.get("package.x").map(|p| p.status)
                == Some(PackagePhase::Integrated)
                && status.state.code_revision == "code.result"
        );
    } else {
        ensure!(status.revision == before.revision);
    }
    Ok(())
}

pub(super) fn resume(_: &Path, _: &str) -> Result<()> {
    let case = Probe::new(false, false)?;
    let initial = case.start()?;
    let dispatch = json!({"type":"dispatch","package":"package.x","attempt":"attempt.1",
        "idempotency_key":"stable.dispatch","expected_revision":initial.revision});
    let first = case.advance(&dispatch)?;
    ensure!(first.applied && first.receipt.is_some() && first.artifact.is_some());
    ensure!(
        case.step(json!({"type":"cancel","reason":"User cancellation"}))?
            .applied
    );
    let replay = case.advance(&dispatch)?;
    ensure!(
        replay.replayed
            && !replay.applied
            && replay.artifact.is_none()
            && replay.receipt == first.receipt
    );
    ensure!(replay.status.context("missing replay status")?.state.status == RunPhase::Cancelled);
    let late = case.submit()?;
    let state = late.state.context("missing submission state")?;
    ensure!(
        late.applied
            && state.status == RunPhase::Cancelled
            && state.packages.get("package.x").map(|p| p.status) == Some(PackagePhase::Submitted)
    );
    ensure!(
        case.step(json!({"type":"resume","reason":"Explicit resume"}))?
            .applied
    );
    ensure!(
        case.step(json!({"type":"retry","package":"package.x","reason":"Fresh attempt"}))?
            .applied
    );
    let before = case.status()?;
    ensure!(case.dispatch("attempt.1").is_err() && case.status()?.revision == before.revision);
    ensure!(case.dispatch("attempt.2")?.applied);
    let before = case.status()?;
    ensure!(case.submit().is_err() && case.status()?.revision == before.revision);
    Ok(())
}

pub(super) fn concurrency(_: &Path, _: &str) -> Result<()> {
    let case = Probe::new(false, false)?;
    let initial = case.start()?;
    let events = [1, 2].map(|i| {
        json!({"type":"cancel","reason":"Race",
        "idempotency_key":format!("cancel.{i}"),"expected_revision":initial.revision})
    });
    let barrier = Barrier::new(2);
    let results = thread::scope(|scope| {
        let publishers = events.each_ref().map(|event| {
            scope.spawn(|| {
                barrier.wait();
                case.advance(event)
            })
        });
        publishers.map(|publisher| publisher.join())
    });
    let mut winners = Vec::new();
    for (i, result) in results.into_iter().enumerate() {
        match result.map_err(|_| anyhow::anyhow!("journal publisher panicked"))? {
            Ok(result) => {
                ensure!(result.applied);
                winners.push(i);
            }
            Err(error) => ensure!(
                error.kind() == ErrorKind::AlreadyExists
                    || error.kind() == ErrorKind::InvalidInput
                        && error.to_string().contains("head changed"),
                "unexpected publication error {error}"
            ),
        }
    }
    ensure!(winners.len() == 1, "expected one concurrent winner");
    let status = case.status()?;
    ensure!(status.event_count == 1 && status.state.status == RunPhase::Cancelled);
    ensure!(
        fs::read_dir(case.run.join("events"))?
            .collect::<std::io::Result<Vec<_>>>()?
            .len()
            == 1
    );
    ensure!(case.advance(&events[winners[0]])?.replayed);
    Ok(())
}

pub(super) fn cancel_unavailable(_: &Path, _: &str) -> Result<()> {
    let case = Probe::new(false, false)?;
    case.start()?;
    let dispatched = case.dispatch("attempt.1")?;
    ensure!(dispatched.applied);
    let receipt = dispatched.receipt.context("missing dispatch receipt")?;
    fs::remove_file(&case.path)?;
    let cancel = case.advance(
        &json!({"type":"cancel","reason":"User cancellation","idempotency_key":"cancel",
        "expected_revision":receipt["revision"]}),
    )?;
    ensure!(
        cancel.applied
            && cancel.state.context("missing cancel state")?.status == RunPhase::Cancelled
    );
    ensure!(case.status()?.event_count == 2);
    let late = case.submit()?;
    let state = late.state.context("missing late submission state")?;
    ensure!(
        late.applied
            && state.status == RunPhase::Cancelled
            && state.packages.get("package.x").map(|p| p.status) == Some(PackagePhase::Submitted)
    );
    Ok(())
}
