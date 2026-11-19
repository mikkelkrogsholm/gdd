//! Read an append-only workflow journal and reconcile it against one model capture.

use super::{Source, canonical, require, verify_valid_plan};
use crate::bundle::Bundle;
use crate::input::{self, InputError, Value};
use crate::model::{
    context::ContextGraph, entries, get, items, readiness::decisions, semantics, string, strings,
    text,
};
use crate::report::{Check, CheckResult, Report};
use crate::validation::{self, Activity, Profile, ValidationOptions, to_json};
use serde::Serialize;
use serde_json::Value as Json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs, io,
    path::Path,
};

mod publication;
pub use publication::{AdvanceResult, advance_run, create_run};
mod replay;
const MAX_RECEIPTS: usize = 999_999;
pub use replay::{PackagePhase, PackageState, RunPhase, RunState};

#[derive(Debug)]
pub struct JournalError {
    pub code: &'static str,
    pub result: CheckResult,
    message: String,
}
impl JournalError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "RECORD_INVALID",
            result: CheckResult::Fail,
            message: message.into(),
        }
    }
    fn at(path: &Path, error: io::Error) -> Self {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let is_loop = error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error());
        #[cfg(windows)]
        let is_loop = error.raw_os_error() == Some(1921);
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        let is_loop = false;
        Self {
            code: if is_loop {
                "ASSET_PATH_INVALID"
            } else {
                "SOURCE_UNAVAILABLE"
            },
            result: if is_loop {
                CheckResult::Fail
            } else {
                CheckResult::Unknown
            },
            message: format!("{}: {error}", path.display()),
        }
    }
    pub fn exit_code(&self) -> u8 {
        if self.code == "FORMAT_UNSUPPORTED" {
            2
        } else {
            1
        }
    }
}
impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for JournalError {}
impl From<JournalError> for io::Error {
    fn from(error: JournalError) -> Self {
        io::Error::other(error)
    }
}

fn read(path: &Path) -> Result<Value, JournalError> {
    let bytes = fs::read(path).map_err(|e| JournalError::at(path, e))?;
    input::parse(&bytes).map_err(|e| {
        let (code, result) = match e {
            InputError::UnsupportedDepth => ("FORMAT_UNSUPPORTED", CheckResult::Unknown),
            InputError::DuplicateKey(_) => ("YAML_DUPLICATE_KEY", CheckResult::Fail),
            InputError::Invalid(_) => ("YAML_FEATURE_FORBIDDEN", CheckResult::Fail),
        };
        JournalError {
            code,
            result,
            message: format!("{}: {e}", path.display()),
        }
    })
}

fn has_fields(value: &Value, fields: &[&str]) -> bool {
    matches!(value,Value::Object(object) if object.len()==fields.len() && fields.iter().all(|key|object.contains_key(*key)))
}

fn payload_digest(value: &Value) -> String {
    let Json::Object(mut doc) = to_json(value).expect("parsed document") else {
        unreachable!("validated object")
    };
    doc.remove("revision");
    canonical::digest(&Json::Object(doc))
}

/// A validated manifest and its contiguous sequence of published receipts.
#[derive(Debug)]
pub struct Journal {
    plan: Value,
    revision: String,
    state: RunState,
    receipts: Vec<Value>,
    used_attempts: BTreeSet<String>,
}
impl Journal {
    pub fn plan(&self) -> &Value {
        &self.plan
    }
    pub fn receipts(&self) -> &[Value] {
        &self.receipts
    }
    pub fn revision(&self) -> &str {
        &self.revision
    }
    pub fn state(&self) -> &RunState {
        &self.state
    }
    /// Declared package targets, suitable as seeds for a scoped model capture.
    pub fn request_scope(&self) -> impl Iterator<Item = &str> {
        request_scope(&self.plan)
    }
}

fn request_scope(plan: &Value) -> impl Iterator<Item = &str> {
    let request = get(plan, "request").expect("validated plan");
    items(request, "packages")
        .iter()
        .flat_map(|p| strings(p, "scope"))
}

fn capture_model(path: &Path, plan: &Value) -> Bundle {
    let scope = request_scope(plan).map(str::to_owned).collect::<Vec<_>>();
    super::capture_plan_inputs(path, &scope, plan)
}

/// Read and structurally replay the journal once, including its hash chain.
pub fn load_journal(directory: &Path) -> Result<Journal, JournalError> {
    let manifest = read(&directory.join("run.json"))?;
    if !has_fields(&manifest, &["format", "plan", "revision"])
        || get(&manifest, "format").and_then(text) != Some("gdd-run/0.1")
    {
        return Err(JournalError::invalid("Invalid run manifest."));
    }
    let plan = get(&manifest, "plan").expect("required field");
    require(plan, "plan", "stored workplan").map_err(|e| JournalError::invalid(e.to_string()))?;
    let mut head = payload_digest(&manifest);
    if get(&manifest, "revision").and_then(text) != Some(&head) {
        return Err(JournalError::invalid("Run manifest hash differs."));
    }
    if string(plan, "revision") != payload_digest(plan) {
        return Err(JournalError::invalid("Stored workplan hash differs."));
    }
    let mut state = RunState::initial(plan);
    let mut seen_keys = BTreeSet::new();
    let mut used_attempts = BTreeSet::new();
    let mut receipts = Vec::new();
    let events = directory.join("events");
    let mut paths = fs::read_dir(&events)
        .and_then(|dir| {
            dir.map(|entry| entry.map(|e| e.path()))
                .collect::<io::Result<Vec<_>>>()
        })
        .map_err(|e| JournalError::at(&events, e))?;
    paths.sort();
    for path in paths {
        let name = path.file_name().expect("directory entry");
        if name.as_encoded_bytes().starts_with(b".") {
            continue;
        }
        let number = receipts.len() + 1;
        if number > MAX_RECEIPTS || name.to_str() != Some(format!("{number:06}.json").as_str()) {
            return Err(JournalError::invalid(
                "Workflow log has a gap or unsupported filename.",
            ));
        }
        let receipt = read(&path)?;
        if !has_fields(
            &receipt,
            &["format", "previous_revision", "event", "revision"],
        ) || get(&receipt, "format").and_then(text) != Some("gdd-run-event/0.1")
        {
            return Err(JournalError::invalid("Invalid workflow event receipt."));
        }
        let event = get(&receipt, "event").expect("required field");
        require(event, "event", "stored workflow event")
            .map_err(|e| JournalError::invalid(e.to_string()))?;
        let revision = payload_digest(&receipt);
        if get(&receipt, "previous_revision").and_then(text) != Some(head.as_str())
            || string(event, "expected_revision") != head
            || get(&receipt, "revision").and_then(text) != Some(&revision)
        {
            return Err(JournalError::invalid("Workflow log hash chain differs."));
        }
        if !seen_keys.insert(string(event, "idempotency_key").to_owned()) {
            return Err(JournalError::invalid("Duplicate workflow event key."));
        }
        state.apply(plan, event, &mut used_attempts)?;
        head = revision;
        receipts.push(receipt);
    }
    let Value::Object(mut fields) = manifest else {
        unreachable!("validated manifest")
    };
    Ok(Journal {
        plan: fields.remove("plan").expect("validated plan"),
        revision: head,
        state,
        receipts,
        used_attempts,
    })
}

#[derive(Debug, Serialize)]
pub struct RunStatus {
    pub format: &'static str,
    pub revision: String,
    pub state: RunState,
    pub available_packages: Vec<String>,
    pub event_count: usize,
    pub acceptance_current: bool,
    pub report: Report,
}

fn validate_host(host: Option<&Value>) -> Result<(), JournalError> {
    if let Some(host) = validation::present(host) {
        validation::validate_input("host", host)
            .map_err(|e| JournalError::invalid(e.to_string()))?;
    }
    Ok(())
}

/// Reconcile a journal with a single capture of its requested model scope.
pub fn run_status(
    path: &Path,
    directory: &Path,
    host_context: Option<&Value>,
) -> Result<RunStatus, JournalError> {
    validate_host(host_context)?;
    let journal = load_journal(directory)?;
    let bundle = capture_model(path, &journal.plan);
    reconcile(&bundle, &journal, host_context)
}

/// Reconcile a previously captured journal and model without reopening their
/// files. Use a full bundle or capture the scopes of the plan request.
pub fn run_status_from_bundle(
    bundle: &Bundle,
    journal: &Journal,
    host_context: Option<&Value>,
) -> Result<RunStatus, JournalError> {
    validate_host(host_context)?;
    reconcile(bundle, journal, host_context)
}

fn current_report(bundle: &Bundle, plan: &Value, host: Option<&Value>) -> Report {
    verify_valid_plan(Source::Bundle(bundle), plan, host).unwrap_or_else(|error| {
        let scope = strings(plan, "scope")
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut report = Report::new(vec![Profile::Readiness], scope);
        let mut check = Check::new(
            "workflow.model_unavailable",
            "readiness",
            CheckResult::Unknown,
            error.to_string(),
        );
        check.scope.clone_from(&report.scope);
        report.checks.push(check);
        report
    })
}
pub(crate) fn reconcile(
    bundle: &Bundle,
    journal: &Journal,
    host: Option<&Value>,
) -> Result<RunStatus, JournalError> {
    let plan = &journal.plan;
    let mut report = current_report(bundle, plan, host);
    let ready = report.result("readiness") == CheckResult::Pass;
    let accepted = journal.state.status == RunPhase::Accepted;
    if ready && accepted {
        assess_acceptance(bundle, plan, &journal.state, host, &mut report)
            .map_err(|e| JournalError::invalid(e.to_string()))?;
    }
    status_with_report(
        plan,
        journal.state.clone(),
        journal.revision.clone(),
        journal.receipts.len(),
        report,
    )
}

fn accepted(report: &Report) -> bool {
    report.result("readiness") == CheckResult::Pass
        && report.result("implementation") == CheckResult::Pass
        && report.result("authorization") == CheckResult::Granted
        && report.result("integration") == CheckResult::Pass
}

fn status_with_report(
    plan: &Value,
    state: RunState,
    revision: String,
    event_count: usize,
    report: Report,
) -> Result<RunStatus, JournalError> {
    let available_packages =
        if state.status == RunPhase::Active && report.result("readiness") == CheckResult::Pass {
            state.available(plan)?
        } else {
            Vec::new()
        };
    let acceptance_current = state.status == RunPhase::Accepted && accepted(&report);
    Ok(RunStatus {
        format: "gdd-run-status/0.1",
        revision,
        state,
        available_packages,
        event_count,
        acceptance_current,
        report,
    })
}

pub(crate) fn bound_host(host: &Value, plan: &Value, code_revision: &str) -> bool {
    let request = get(plan, "request").expect("validated plan");
    get(host, "implementation") == get(request, "implementation")
        && get(host, "code_revision").and_then(text) == Some(code_revision)
}

fn gate(id: &str, result: CheckResult, reason: &str, scope: &[String]) -> Check {
    let mut check = Check::new(id, "integration", result, reason);
    check.scope = scope.to_owned();
    check
}

/// Bind evidence evaluation to the check choices frozen in the workplan.
pub(crate) fn evidence_context(plan: &Value, host: &Value) -> (Value, bool) {
    let mut selection: BTreeMap<String, Value> = BTreeMap::new();
    let mut conflict = false;
    let supplied = get(host, "check_selection");
    for required in items(plan, "packages")
        .iter()
        .flat_map(|p| items(p, "required_checks"))
    {
        if let [id] = items(required, "checks") {
            let rule = string(required, "rule");
            let method = string(required, "method");
            let methods = selection
                .entry(rule.into())
                .or_insert_with(|| Value::Object(BTreeMap::new()));
            let Value::Object(methods) = methods else {
                unreachable!()
            };
            methods.insert(method.into(), id.clone());
            conflict |= supplied
                .and_then(|s| get(s, rule))
                .and_then(|s| get(s, method))
                .is_some_and(|other| other != id);
        }
    }
    let mut evidence_host = if let Value::Object(fields) = host {
        fields.clone()
    } else {
        BTreeMap::new()
    };
    evidence_host.insert("check_selection".into(), Value::Object(selection));
    (Value::Object(evidence_host), conflict)
}

fn assess_acceptance(
    bundle: &Bundle,
    plan: &Value,
    state: &RunState,
    host: Option<&Value>,
    report: &mut Report,
) -> io::Result<()> {
    report.profiles = vec![Profile::Model, Profile::Readiness, Profile::Records];
    let host = validation::present(host).unwrap_or(&Value::Null);
    let bound = bound_host(host, plan, &state.code_revision);
    report.checks.push(gate(
        "workflow.code_binding",
        if bound {
            CheckResult::Pass
        } else {
            CheckResult::Unknown
        },
        "Host implementation and code revision must equal this run's integrated result.",
        &report.scope,
    ));
    let (evidence_host, conflict) = evidence_context(plan, host);
    report.checks.push(gate(
        "workflow.check_selection",
        if conflict {
            CheckResult::Fail
        } else {
            CheckResult::Pass
        },
        "Verification uses the exact check plans frozen in this workplan.",
        &report.scope,
    ));
    for (dimension, activity, context) in [
        ("implementation", None, &evidence_host),
        ("authorization", Some(Activity::Acceptance), host),
    ] {
        let assessed = validation::validate_with_options(
            bundle,
            ValidationOptions {
                profiles: &[Profile::Records],
                scope: &report.scope,
                host_context: Some(context),
                activity,
                ..Default::default()
            },
        )?;
        report.checks.extend(
            assessed
                .checks
                .into_iter()
                .filter(|c| c.dimension == dimension),
        );
    }
    let manifest = bundle.manifest().expect("verified workplan");
    let graph = ContextGraph::new(bundle);
    let context = graph.scope(report.scope.iter().map(String::as_str));
    let mut questions = entries(&manifest.value, "questions")
        .filter(|(_, q)| {
            strings(q, "blocks").any(|s| s == "acceptance")
                && strings(q, "targets").any(|target| context.contains(target))
        })
        .peekable();
    if questions.peek().is_some() {
        let model = semantics(bundle, manifest, &[]);
        let accepted = decisions::accepted(&model, Some(host));
        for (id, q) in questions {
            let resolved = string(q, "status") != "open" && decisions::resolves(q, &accepted);
            report.checks.push(gate(
                &format!("workflow.question.{id}"),
                if resolved {
                    CheckResult::Pass
                } else {
                    CheckResult::Unknown
                },
                "Acceptance blocker needs a trusted current resolution.",
                std::slice::from_ref(id),
            ));
        }
    }
    Ok(())
}
