//! Exclusive publication of prepared runs and host-reported transitions.

use super::{
    MAX_RECEIPTS, RunState, RunStatus, accepted, assess_acceptance, bound_host, capture_model,
    current_report, gate, load_journal, reconcile, status_with_report, validate_host,
};
use crate::bundle::Bundle;
use crate::input::{Value, check_output_depth};
use crate::model::{get, items, string, strings};
use crate::report::{CheckResult, Report};
use crate::validation::{self, Activity, Profile, ValidationOptions, to_json};
use crate::workflow::{Source, canonical, invalid, require, verify_valid_plan};
use serde::Serialize;
use serde_json::{Value as Json, json};
use std::{
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::Path,
};

const RESULT_FORMAT: &str = "gdd-workflow-result/0.1";

/// Result of a host-reported transition. A replay returns the original receipt
/// and current status. Only a newly applied dispatch includes a worker packet.
#[derive(Debug, Serialize)]
pub struct AdvanceResult {
    pub format: &'static str,
    pub applied: bool,
    pub replayed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<Report>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<RunState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<RunStatus>,
}

impl AdvanceResult {
    pub fn exit_code(&self) -> u8 {
        if self.applied || self.replayed {
            0
        } else {
            self.report
                .as_ref()
                .expect("a blocked event has a gate report")
                .exit_code()
        }
    }
}

fn write_json(file: &mut File, value: &Json) -> io::Result<()> {
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Publish the verified plan in a new run directory. An unready current plan
/// is still a valid draft and retains its readiness diagnostics.
pub fn create_run(
    path: &Path,
    plan: &Value,
    directory: &Path,
    host_context: Option<&Value>,
) -> io::Result<RunStatus> {
    require(plan, "plan", "workplan")?;
    validate_host(host_context)?;
    let bundle = capture_model(path, plan);
    let report = verify_valid_plan(Source::Bundle(&bundle), plan, host_context)?;
    if report
        .checks
        .iter()
        .any(|c| c.check_id == "workflow.packet_integrity" && c.result != CheckResult::Pass)
    {
        return Err(invalid(
            "Cannot publish a run from a stale or altered workplan.",
        ));
    }

    let mut manifest = json!({"format":"gdd-run/0.1","plan":to_json(plan)?});
    let revision = canonical::digest(&manifest);
    manifest["revision"] = json!(revision);
    let status = status_with_report(plan, RunState::initial(plan), revision, 0, report)?;

    let parent = directory
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = tempfile::Builder::new()
        .prefix(".gdd-run-")
        .tempdir_in(parent)?;
    let staging = temporary.path().join("run");
    fs::create_dir(&staging)?;
    fs::create_dir(staging.join("events"))?;
    write_json(&mut File::create_new(staging.join("run.json"))?, &manifest)?;
    crate::rename_exclusive(&staging, directory)?;
    // Creation reports the readability of the published manifest, like run status.
    check_output_depth(&manifest).map_err(io::Error::other)?;
    Ok(status)
}

fn publish_event(directory: &Path, number: usize, receipt: &Json) -> io::Result<()> {
    if number > MAX_RECEIPTS {
        return Err(invalid(
            "Workflow log has reached its six-digit event capacity.",
        ));
    }
    let events = directory.join("events");
    let mut pending = tempfile::Builder::new()
        .prefix(".pending-")
        .tempfile_in(&events)?;
    write_json(pending.as_file_mut(), receipt)?;
    // Close the file before rename on every platform. Keep the cleanup guard
    // armed on failure and disarm it after exclusive publication succeeds.
    let mut temporary = pending.into_temp_path();
    crate::rename_exclusive(&temporary, &events.join(format!("{number:06}.json")))?;
    temporary.disable_cleanup(true);
    Ok(())
}

fn declared_package<'a>(plan: &'a Value, event: &Value) -> &'a Value {
    let id = string(event, "package");
    items(plan, "packages")
        .iter()
        .find(|p| string(p, "id") == id)
        .expect("known workflow package")
}

fn assess_transition(
    bundle: &Bundle,
    plan: &Value,
    state: &RunState,
    event: &Value,
    host: Option<&Value>,
    report: &mut Report,
) -> io::Result<bool> {
    let kind = string(event, "type");
    let ready = report.result("readiness") == CheckResult::Pass;
    if kind == "accept" {
        if ready {
            assess_acceptance(bundle, plan, state, host, report)?;
        }
        return Ok(accepted(report));
    }
    if !matches!(kind, "dispatch" | "resume" | "integrate") {
        return Ok(true);
    }
    report.profiles = vec![Profile::Model, Profile::Readiness, Profile::Records];
    let declaration = if kind == "resume" {
        plan
    } else {
        declared_package(plan, event)
    };
    let scope = strings(declaration, "scope")
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let authorization = validation::validate_with_options(
        bundle,
        ValidationOptions {
            profiles: &[Profile::Records],
            scope: &scope,
            activity: Some(Activity::Implementation),
            host_context: host,
            ..Default::default()
        },
    )?;
    let granted = authorization.result("authorization") == CheckResult::Granted;
    report.checks.extend(
        authorization
            .checks
            .into_iter()
            .filter(|c| c.dimension == "authorization"),
    );
    let (code, reason) = if kind == "integrate" {
        (
            string(event, "code_revision"),
            "Host must independently attest the reviewed integrated code revision.",
        )
    } else {
        (
            state.code_revision.as_str(),
            "Host must attest this run implementation and current code base.",
        )
    };
    let bound = bound_host(host.unwrap_or(&Value::Null), plan, code);
    report.checks.push(gate(
        "workflow.code_binding",
        if bound {
            CheckResult::Pass
        } else {
            CheckResult::Unknown
        },
        reason,
        &scope,
    ));
    Ok(ready && granted && bound)
}

/// Compare an event with the published head, assess its current gates and
/// atomically append its receipt. The host supplies the result of external work.
pub fn advance_run(
    path: &Path,
    directory: &Path,
    event: &Value,
    host_context: Option<&Value>,
) -> io::Result<AdvanceResult> {
    require(event, "event", "workflow event")?;
    validate_host(host_context)?;
    let mut journal = load_journal(directory)?;
    let plan = &journal.plan;
    let key = string(event, "idempotency_key");
    for receipt in &journal.receipts {
        let previous = get(receipt, "event").expect("validated receipt");
        if string(previous, "idempotency_key") == key {
            // Every event field is a string, string array or object of strings.
            // Structural equality on parsed objects is canonical equality here.
            if previous != event {
                return Err(invalid(
                    "Idempotency key already refers to a different request.",
                ));
            }
            let bundle = capture_model(path, plan);
            return Ok(AdvanceResult {
                format: RESULT_FORMAT,
                applied: false,
                replayed: true,
                receipt: Some(to_json(receipt)?),
                status: Some(reconcile(&bundle, &journal, host_context)?),
                report: None,
                state: None,
                artifact: None,
            });
        }
    }
    if string(event, "expected_revision") != journal.revision {
        return Err(invalid(
            "Workflow head changed; read status before publishing a new event.",
        ));
    }
    // Structural replay only changes the candidate in memory. A declined gate
    // or a competing publication leaves the existing journal intact.
    journal
        .state
        .apply(plan, event, &mut journal.used_attempts)?;
    let bundle = capture_model(path, plan);
    let mut report = current_report(&bundle, plan, host_context);
    let allowed = assess_transition(
        &bundle,
        plan,
        &journal.state,
        event,
        host_context,
        &mut report,
    )?;
    if !allowed {
        return Ok(AdvanceResult {
            format: RESULT_FORMAT,
            applied: false,
            replayed: false,
            report: Some(report),
            receipt: None,
            artifact: None,
            state: None,
            status: None,
        });
    }
    let artifact = if string(event, "type") == "dispatch" {
        let request = get(plan, "request").expect("validated plan");
        Some(json!({
            "format": "gdd-dispatch/0.1",
            "plan_revision": string(plan, "revision"),
            "spec_revision": string(plan, "spec_revision"),
            "implementation": string(request, "implementation"),
            "attempt": string(event, "attempt"),
            "base_code_revision": journal.state.code_revision,
            "package": to_json(declared_package(plan, event))?,
        }))
    } else {
        None
    };
    let mut receipt = json!({"format":"gdd-run-event/0.1","previous_revision":journal.revision,"event":to_json(event)?});
    receipt["revision"] = canonical::digest(&receipt).into();
    publish_event(directory, journal.receipts.len() + 1, &receipt)?;
    Ok(AdvanceResult {
        format: RESULT_FORMAT,
        applied: true,
        replayed: false,
        receipt: Some(receipt),
        report: Some(report),
        state: Some(journal.state),
        artifact,
        status: None,
    })
}
