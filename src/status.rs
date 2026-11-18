//! A read-only project overview from one captured model and its operational records.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io;
use std::path::Path;

use serde::{Serialize, Serializer, ser::SerializeMap};
use serde_json::Value as JsonValue;

use crate::bundle::{self, Bundle};
use crate::input::Value;
use crate::report::{Check, CheckResult, Diagnostic, DiagnosticLocation, Report, checks_exit_code};
use crate::validation::{self, Activity, Profile, ValidationOptions};
use crate::workflow_run::{self, Journal, JournalError};

const MAX_ACTIONS: usize = 12;
const DIMENSIONS: [&str; 4] = ["model", "readiness", "authorization", "implementation"];

pub struct StatusOptions<'a> {
    pub scope: &'a [String],
    pub host_context: Option<&'a Value>,
    pub implementation: Option<&'a str>,
    pub code_revision: Option<&'a str>,
    pub activity: Activity,
}

impl Default for StatusOptions<'_> {
    fn default() -> Self {
        Self {
            scope: &[],
            host_context: None,
            implementation: None,
            code_revision: None,
            activity: Activity::Implementation,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StatusTool {
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Serialize)]
pub struct StatusInputs {
    pub model: String,
    pub run: Option<String>,
    pub host_context_supplied: bool,
    pub implementation: Option<String>,
    pub code_revision: Option<String>,
    pub activity: Activity,
    pub missing: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct Assessment {
    pub assessed: bool,
    pub result: Option<CheckResult>,
    pub check_refs: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct OpenQuestion {
    pub id: String,
    pub question: JsonValue,
    pub location: DiagnosticLocation,
}

#[derive(Debug, Serialize)]
pub struct NextAction {
    pub reason: String,
    pub source: String,
    pub diagnostic_code: Option<&'static str>,
    pub location: Option<DiagnosticLocation>,
    pub argv: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectStatus {
    pub format: &'static str,
    pub tool: StatusTool,
    pub spec_revision: Option<String>,
    pub scope: Vec<String>,
    pub inputs: StatusInputs,
    #[serde(serialize_with = "serialize_dimensions")]
    pub assessments: BTreeMap<&'static str, Assessment>,
    #[serde(serialize_with = "serialize_dimensions")]
    pub reports: BTreeMap<&'static str, Report>,
    pub workflow: Option<JsonValue>,
    pub workflow_error: Option<JsonValue>,
    pub open_questions: Vec<OpenQuestion>,
    pub next_actions: Vec<NextAction>,
    pub next_action_count: usize,
    pub exit_code: u8,
}

// Reports and assessments follow the same public dimension order as the text renderer.
fn serialize_dimensions<S: Serializer, V: Serialize>(
    entries: &BTreeMap<&str, V>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(entries.len()))?;
    for dimension in DIMENSIONS {
        if let Some(value) = entries.get(dimension) {
            map.serialize_entry(dimension, value)?;
        }
    }
    map.end()
}

struct Request {
    scope: Vec<String>,
    host: Value,
    inputs: StatusInputs,
}

impl Request {
    fn new(path: &Path, options: StatusOptions<'_>) -> io::Result<Self> {
        let supplied = validation::present(options.host_context);
        let mut host = match supplied {
            Some(Value::Object(fields)) => fields.clone(),
            None => BTreeMap::new(),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid host input: expected an object.",
                ));
            }
        };
        for (field, explicit) in [
            ("implementation", options.implementation),
            ("code_revision", options.code_revision),
        ] {
            if let Some(explicit) = explicit {
                let value = Value::String(explicit.to_owned());
                if host.get(field).is_some_and(|v| *v != value) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Explicit {field} conflicts with host context."),
                    ));
                }
                host.insert(field.into(), value);
            }
        }
        let source_text = |field: &str| match host.get(field) {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        };
        let inputs = StatusInputs {
            model: path.display().to_string(),
            run: None,
            host_context_supplied: supplied.is_some(),
            implementation: source_text("implementation"),
            code_revision: source_text("code_revision"),
            activity: options.activity,
            missing: Vec::new(),
        };
        let host = Value::Object(host);
        let scope = ValidationOptions {
            scope: options.scope,
            host_context: Some(&host),
            ..Default::default()
        }
        .effective_scope()?
        .into_iter()
        .map(str::to_owned)
        .collect();
        Ok(Self {
            scope,
            host,
            inputs,
        })
    }
}

/// Capture the scoped advisory sources, model assets, and declared records once.
pub fn project_status(path: &Path, options: StatusOptions<'_>) -> io::Result<ProjectStatus> {
    let request = Request::new(path, options)?;
    let bundle = bundle::load_for_readiness(path, &request.scope);
    compose(&bundle, request, None)
}

/// Assess an existing capture. The resulting commands use the capture's model
/// path. Supply a full bundle or capture advisory contracts for the same scope.
pub fn project_status_from_bundle(
    bundle: &Bundle,
    options: StatusOptions<'_>,
) -> io::Result<ProjectStatus> {
    compose(bundle, Request::new(bundle.path(), options)?, None)
}

/// Reconcile an explicitly supplied workflow run with the independent project
/// assessments. Capture the journal request scope and project scope together.
pub fn project_status_with_run(
    path: &Path,
    directory: &Path,
    options: StatusOptions<'_>,
) -> io::Result<ProjectStatus> {
    let mut request = Request::new(path, options)?;
    request.inputs.run = Some(directory.display().to_string());
    let journal = workflow_run::load_journal(directory);
    let mut capture_scope = request.scope.clone();
    if !capture_scope.is_empty()
        && let Ok(journal) = &journal
    {
        capture_scope.extend(journal.request_scope().map(str::to_owned));
    }
    let bundle = bundle::load_for_readiness(path, &capture_scope);
    compose(&bundle, request, Some(journal))
}

/// Reuse a captured model while reading the supplied journal once. The model
/// capture should cover both the project scope and all workplan packages.
pub fn project_status_with_run_from_bundle(
    bundle: &Bundle,
    directory: &Path,
    options: StatusOptions<'_>,
) -> io::Result<ProjectStatus> {
    let mut request = Request::new(bundle.path(), options)?;
    request.inputs.run = Some(directory.display().to_string());
    compose(bundle, request, Some(workflow_run::load_journal(directory)))
}

fn canonical(value: &impl Serialize) -> Vec<u8> {
    let mut json = serde_json::to_value(value).expect("report data is JSON");
    json.sort_all_objects();
    serde_json::to_vec(&json).expect("JSON serializes to bytes")
}

fn compose(
    bundle: &Bundle,
    request: Request,
    journal: Option<Result<Journal, JournalError>>,
) -> io::Result<ProjectStatus> {
    let Request {
        scope: original,
        host,
        mut inputs,
    } = request;
    let mut scope = original.clone();
    scope.sort();
    let assess = |profile, scope: &[String], activity| {
        let ids: Vec<_> = scope.iter().map(String::as_str).collect();
        validation::assess(
            bundle,
            ValidationOptions {
                profiles: &[profile],
                scope,
                host_context: Some(&host),
                activity,
                ..Default::default()
            },
            &ids,
            None,
        )
    };
    let readiness = assess(Profile::Readiness, &scope, None)?;
    // Structural readiness starts with the complete model report. Preserve the
    // original requested scope in the model report, including unavailable checks.
    let model = if original == scope {
        let mut model = Report::new(vec![Profile::Model], original);
        model.spec_revision.clone_from(&readiness.spec_revision);
        model.checks.extend(
            readiness
                .checks
                .iter()
                .filter(|c| c.dimension != "readiness")
                .cloned(),
        );
        model
    } else {
        assess(Profile::Model, &original, None)?
    };
    let open_questions = questions(bundle, &model)?;
    let spec_revision = model.spec_revision.clone();
    let mut reports = BTreeMap::from([("model", model), ("readiness", readiness)]);
    if inputs.host_context_supplied
        || inputs.implementation.is_some()
        || inputs.code_revision.is_some()
    {
        reports.insert(
            "authorization",
            assess(Profile::Records, &scope, Some(inputs.activity))?,
        );
        reports.insert("implementation", assess(Profile::Records, &scope, None)?);
    }
    for report in reports.values_mut() {
        report.checks.sort_by_cached_key(canonical);
    }
    let assessments = DIMENSIONS
        .into_iter()
        .map(|dimension| {
            let report = reports.get(dimension);
            let check_refs = report
                .into_iter()
                .flat_map(|r| r.checks.iter().enumerate())
                .filter(|(_, c)| c.dimension == dimension)
                .map(|(index, _)| format!("/reports/{dimension}/checks/{index}"))
                .collect();
            (
                dimension,
                Assessment {
                    assessed: report.is_some(),
                    result: report.map(|r| r.result(dimension)),
                    check_refs,
                },
            )
        })
        .collect();
    let reconciled =
        journal.map(|j| j.and_then(|j| workflow_run::reconcile(bundle, &j, Some(&host))));
    let (workflow, workflow_error) = match reconciled {
        Some(Ok(mut workflow)) => {
            workflow.report.checks.sort_by_cached_key(canonical);
            (Some(workflow), None)
        }
        Some(Err(error)) => {
            let reason = error.to_string();
            let mut check = Check::new(
                "status.workflow_input",
                "integration",
                error.result,
                &reason,
            );
            check.diagnostics.push(Diagnostic::new(
                error.code,
                error.result,
                reason,
                inputs.run.as_deref(),
                "",
            ));
            (None, Some(check))
        }
        None => (None, None),
    };
    let workflow_report = workflow.as_ref().map(|s| &s.report);
    let (mut actions, mut next_action_count) =
        diagnostic_actions(&reports, workflow_report, &inputs.model, &scope);
    if let Some(check) = &workflow_error {
        next_action_count += 1;
        if actions.len() < MAX_ACTIONS {
            let diagnostic = &check.diagnostics[0];
            actions.push(NextAction {
                reason: check.reason.clone(),
                source: "/workflow_error/check/diagnostics/0".into(),
                diagnostic_code: Some(diagnostic.code),
                location: Some(diagnostic.location.clone()),
                argv: [
                    "gdd",
                    "workflow",
                    "status",
                    &inputs.model,
                    "--run",
                    inputs.run.as_deref().expect("supplied run"),
                ]
                .map(str::to_owned)
                .into(),
            });
        }
    }
    for (name, missing) in [
        ("run", inputs.run.is_none()),
        ("host_context", !inputs.host_context_supplied),
        ("implementation", inputs.implementation.is_none()),
        ("code_revision", inputs.code_revision.is_none()),
    ] {
        if missing {
            let index = inputs.missing.len();
            inputs.missing.push(name);
            next_action_count += 1;
            if actions.len() == MAX_ACTIONS {
                continue;
            }
            actions.push(NextAction {
                reason: format!(
                    "{name} was not supplied; no corresponding state or trust is inferred."
                ),
                source: format!("/inputs/missing/{index}"),
                diagnostic_code: None,
                location: None,
                argv: ["gdd", "status", "--help"].map(str::to_owned).into(),
            });
        }
    }
    // All assessed reports contribute integration/load checks to the process verdict.
    let exit_code = checks_exit_code(
        reports
            .values()
            .chain(workflow_report)
            .flat_map(|r| &r.checks)
            .chain(workflow_error.iter()),
    );
    let workflow = workflow.map(serde_json::to_value).transpose()?;
    let workflow_error =
        workflow_error.map(|check| serde_json::json!({"input":inputs.run,"check":check}));
    Ok(ProjectStatus {
        format: "gdd-status/0.1",
        tool: StatusTool {
            name: "gdd-validator",
            version: env!("CARGO_PKG_VERSION"),
        },
        spec_revision,
        scope,
        inputs,
        assessments,
        reports,
        workflow,
        workflow_error,
        open_questions,
        next_actions: actions,
        next_action_count,
        exit_code,
    })
}

fn questions(bundle: &Bundle, model: &Report) -> io::Result<Vec<OpenQuestion>> {
    let Some(manifest) = bundle.manifest() else {
        return Ok(Vec::new());
    };
    // The root representation check precedes all included documents and model
    // semantics. Unavailable fragments do not hide valid manifest questions.
    if model
        .checks
        .first()
        .is_none_or(|c| c.check_id != "representation.schema" || c.result != CheckResult::Pass)
    {
        return Ok(Vec::new());
    }
    let Value::Object(root) = &manifest.value else {
        return Ok(Vec::new());
    };
    let Some(Value::Object(questions)) = root.get("questions") else {
        return Ok(Vec::new());
    };
    let mut open = Vec::new();
    for (id, question) in questions {
        let Value::Object(fields) = question else {
            continue;
        };
        if matches!(fields.get("status"), Some(Value::String(s)) if s=="open") {
            open.push(OpenQuestion {
                id: id.clone(),
                question: validation::to_json(question)?,
                location: DiagnosticLocation {
                    file: Some(manifest.asset.path.clone()),
                    pointer: format!("/questions/{id}"),
                },
            });
        }
    }
    Ok(open)
}

fn diagnostic_actions(
    reports: &BTreeMap<&str, Report>,
    workflow: Option<&Report>,
    path: &str,
    scope: &[String],
) -> (Vec<NextAction>, usize) {
    let mut seen = BTreeSet::new();
    let mut first = BTreeMap::new();
    let sources = DIMENSIONS
        .into_iter()
        .filter_map(|dimension| {
            reports
                .get(dimension)
                .map(|r| (format!("/reports/{dimension}"), r))
        })
        .chain(workflow.map(|r| ("/workflow/report".to_owned(), r)));
    for (prefix, report) in sources {
        for (index, check) in report.checks.iter().enumerate() {
            if matches!(check.result, CheckResult::Pass | CheckResult::Granted) {
                continue;
            }
            let profile = match check.dimension {
                "implementation" | "authorization" => "records",
                "readiness" => "readiness",
                _ => "model",
            };
            for (number, diag) in check.diagnostics.iter().enumerate() {
                if !seen.insert(canonical(diag)) {
                    continue;
                }
                let source = format!("{prefix}/checks/{index}/diagnostics/{number}");
                first.insert((diag.code, diag.message.as_str(), source), (diag, profile));
                if first.len() > MAX_ACTIONS {
                    first.pop_last();
                }
            }
        }
    }
    let actions = first
        .into_iter()
        .map(|((_, _, source), (diag, profile))| {
            let argv = ["gdd", "validate", path, "--profile", profile]
                .into_iter()
                .chain(scope.iter().flat_map(|s| ["--scope", s.as_str()]))
                .map(str::to_owned)
                .collect();
            NextAction {
                reason: diag.message.clone(),
                source,
                diagnostic_code: Some(diag.code),
                location: Some(diag.location.clone()),
                argv,
            }
        })
        .collect();
    (actions, seen.len())
}

fn shell_quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&b))
    {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\"'\"'"))
    }
}

/// Render the already-assessed status, including shell-quoted follow-up commands.
pub fn status_text(status: &ProjectStatus) -> String {
    let mut text = String::from("GDD status\n");
    writeln!(
        text,
        "Revision: {}",
        status.spec_revision.as_deref().unwrap_or("unresolved")
    )
    .unwrap();
    writeln!(
        text,
        "Scope: {}",
        if status.scope.is_empty() {
            "default model scope".into()
        } else {
            status.scope.join(", ")
        }
    )
    .unwrap();
    for dimension in DIMENSIONS {
        let result = status.assessments[dimension].result;
        writeln!(
            text,
            "{dimension}: {}",
            result
                .map(|r| r.to_string())
                .unwrap_or_else(|| "UNASSESSED".into())
        )
        .unwrap();
    }
    if let Some(workflow) = &status.workflow {
        let state = &workflow["state"];
        writeln!(
            text,
            "Workflow: {}; acceptance_current={}",
            state["status"].as_str().unwrap_or_default(),
            workflow["acceptance_current"].as_bool().unwrap_or(false)
        )
        .unwrap();
        if let Some(packages) = state["packages"].as_object() {
            let ordered: BTreeMap<_, _> = packages.iter().collect();
            for (id, p) in ordered {
                writeln!(
                    text,
                    "  {id}: {}; attempt={}",
                    p["status"].as_str().unwrap_or_default(),
                    p["attempt"].as_str().unwrap_or("None")
                )
                .unwrap();
            }
        }
    } else if let Some(error) = &status.workflow_error {
        writeln!(
            text,
            "Workflow: {}; {}",
            error["check"]["result"].as_str().unwrap_or_default(),
            error["check"]["reason"].as_str().unwrap_or_default()
        )
        .unwrap();
    } else {
        text.push_str("Workflow: UNASSESSED (no run supplied)\n");
    }
    for q in &status.open_questions {
        writeln!(
            text,
            "Open question {}: {}",
            q.id,
            q.question["statement"].as_str().unwrap_or_default()
        )
        .unwrap();
    }
    for action in &status.next_actions {
        let location = action
            .location
            .as_ref()
            .map(|l| format!("{}:{}", l.file.as_deref().unwrap_or("<input>"), l.pointer))
            .unwrap_or_else(|| action.source.clone());
        writeln!(
            text,
            "Next [{}] {location}: {}",
            action.source, action.reason
        )
        .unwrap();
        writeln!(
            text,
            "  {}",
            action
                .argv
                .iter()
                .map(|s| shell_quote(s))
                .collect::<Vec<_>>()
                .join(" ")
        )
        .unwrap();
    }
    let additional = status.next_action_count - status.next_actions.len();
    if additional > 0 {
        writeln!(
            text,
            "Additional actions: {additional}; consult underlying reports."
        )
        .unwrap();
    }
    writeln!(text, "Exit code: {}", status.exit_code).unwrap();
    text
}
