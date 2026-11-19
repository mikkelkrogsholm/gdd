//! Project graph nodes, workflow progress and evidence for one captured revision.

use crate::{
    bundle::Bundle,
    input::Value,
    issues::IssueLink,
    model::{get, items, string, strings, text},
    report::{Check, CheckDetails, CheckResult},
    validation::{self, Profile, ValidationOptions},
    workflow_run::{self, PackagePhase, RunPhase},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    io,
    path::Path,
};

pub const FORMAT: &str = "gdd-work-overview/0.1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkOverview {
    pub format: String,
    pub project_id: String,
    pub spec_revision: Option<String>,
    pub plan_spec_revision: String,
    pub plan_current: bool,
    pub model_nodes: Vec<String>,
    pub run_revision: String,
    pub run_phase: String,
    pub code_revision: String,
    pub evidence_revision: Option<String>,
    pub acceptance_current: bool,
    pub packages: Vec<WorkPackage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkPackage {
    pub id: String,
    /// Declared work roots, before expanding refinements.
    pub scope: Vec<String>,
    /// Work roots, their implementation targets, and their owning components.
    pub nodes: Vec<String>,
    pub phase: String,
    pub attempt: Option<String>,
    pub available: bool,
    pub blockers: Vec<String>,
    pub verification: Option<CheckResult>,
    pub evidence: Vec<WorkCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<IssueLink>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkCheck {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    pub result: CheckResult,
    pub reason: String,
    pub code_revision: Option<String>,
    pub evidence_refs: Vec<String>,
}

impl WorkOverview {
    pub fn packages_for(&self, scope: &[String]) -> Vec<&WorkPackage> {
        self.packages.iter().filter(|p| p.matches(scope)).collect()
    }

    /// Filter to selected nodes. A known node can have zero associated packages.
    pub fn retain_scope(&mut self, scope: &[String]) -> io::Result<()> {
        for id in scope {
            if !self.model_nodes.contains(id) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unknown graph node '{id}'."),
                ));
            }
        }
        self.packages.retain(|p| p.matches(scope));
        Ok(())
    }
}

impl WorkPackage {
    fn matches(&self, scope: &[String]) -> bool {
        scope.is_empty() || scope.iter().any(|id| self.nodes.contains(id))
    }
}

fn describe(check: &Check) -> String {
    std::iter::once(check.reason.as_str())
        .chain(
            check
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str()),
        )
        .collect::<Vec<_>>()
        .join(" ")
}

fn package_nodes(packet: &Value) -> Vec<String> {
    let targets: BTreeSet<_> = strings(packet, "scope")
        .chain(strings(packet, "implementation_targets"))
        .collect();
    let mut nodes = targets.clone();
    if let Some(slice) = get(packet, "model_slice") {
        for edge in items(slice, "edges") {
            if get(edge, "relation").and_then(text) == Some("owned_by")
                && get(edge, "from")
                    .and_then(text)
                    .is_some_and(|id| targets.contains(id))
                && let Some(owner) = get(edge, "to").and_then(text)
            {
                nodes.insert(owner);
            }
        }
    }
    nodes.into_iter().map(str::to_owned).collect()
}

/// Assess once using the original implementation assessor, retaining its rule IDs.
fn assess_work_checks(
    bundle: &Bundle,
    scope: &[String],
    host: &Value,
) -> io::Result<Vec<WorkCheck>> {
    let record_checks: BTreeMap<_, _> = bundle
        .records()
        .filter_map(|record| {
            get(record.value, "check")
                .and_then(text)
                .map(|id| (record.id, id))
        })
        .collect();
    let report = validation::validate_with_options(
        bundle,
        ValidationOptions {
            profiles: &[Profile::Records],
            scope,
            host_context: Some(host),
            ..Default::default()
        },
    )?;
    let checks = report
        .checks
        .iter()
        .filter(|check| check.dimension == "implementation")
        .map(|check| {
            let (code_revision, evidence_refs) = match &check.details {
                Some(CheckDetails::Implementation {
                    code_revision,
                    evidence_refs,
                    ..
                }) => (Some(code_revision.clone()), evidence_refs.clone()),
                _ => (None, Vec::new()),
            };
            let id = evidence_refs
                .first()
                .and_then(|id| record_checks.get(id.as_str()))
                .copied()
                .unwrap_or(&check.check_id);
            let rule = check
                .check_id
                .starts_with("implementation.required.")
                .then(|| check.scope.first().cloned())
                .flatten();
            WorkCheck {
                id: id.to_owned(),
                rule,
                result: check.result,
                reason: describe(check),
                code_revision,
                evidence_refs,
            }
        })
        .collect();
    Ok(checks)
}

/// A package's normative read context can include a caller's and a sibling's
/// obligations. Show checks for its own targets and their components separately.
fn package_checks(
    packet: &Value,
    nodes: &[String],
    checks: &[WorkCheck],
) -> (Option<CheckResult>, Vec<WorkCheck>) {
    let mut rules: BTreeSet<&str> = nodes.iter().map(String::as_str).collect();
    if let Some(slice) = get(packet, "model_slice") {
        if let Some(Value::Object(declarations)) = get(slice, "nodes") {
            for (id, node) in declarations {
                if let Some(rule) = get(node, "rule") {
                    let constraint = get(rule, "constraint");
                    let applies = get(rule, "scope").and_then(text) == Some("global")
                        || ["from", "to"].iter().any(|key| {
                            constraint
                                .and_then(|c| get(c, key))
                                .and_then(text)
                                .is_some_and(|target| nodes.iter().any(|n| n == target))
                        });
                    if applies {
                        rules.insert(id);
                    }
                }
            }
        }
        for edge in items(slice, "edges") {
            if get(edge, "relation").and_then(text) == Some("constrains")
                && get(edge, "to")
                    .and_then(text)
                    .is_some_and(|id| nodes.iter().any(|n| n == id))
                && let Some(rule) = get(edge, "from").and_then(text)
            {
                rules.insert(rule);
            }
        }
    }
    let evidence: Vec<_> = checks
        .iter()
        .filter(|check| check.rule.as_deref().is_none_or(|id| rules.contains(id)))
        .cloned()
        .collect();
    match evidence.iter().map(|check| check.result).max() {
        Some(result) => (Some(result), evidence),
        None => unavailable_verification(
            "No verified blocking rule applies directly to this package's targets.",
        ),
    }
}

fn unavailable_verification(reason: &str) -> (Option<CheckResult>, Vec<WorkCheck>) {
    (
        Some(CheckResult::Unknown),
        vec![WorkCheck {
            id: "workflow.evidence_context".into(),
            rule: None,
            result: CheckResult::Unknown,
            reason: reason.into(),
            code_revision: None,
            evidence_refs: Vec::new(),
        }],
    )
}

/// Read one consistent journal head and capture the workplan's model inputs once.
/// Availability is structural. The dispatch event applies the host's authorization gates.
pub fn work_overview(model: &Path, run: &Path, host: Option<&Value>) -> io::Result<WorkOverview> {
    let journal = workflow_run::load_journal(run)?;
    let scope = journal
        .request_scope()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let bundle = crate::workflow::capture_plan_inputs(model, &scope, journal.plan());
    let status = workflow_run::run_status_from_bundle(&bundle, &journal, host)?;
    let plan = journal.plan();
    let project_id = string(plan, "project_id");
    let model_project = bundle
        .manifest()
        .and_then(|m| get(&m.value, "project"))
        .and_then(|p| get(p, "id"))
        .and_then(text);
    if model_project != Some(project_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The workplan and model must identify the same project.",
        ));
    }
    let spec_revision = bundle.snapshot().map(|snapshot| snapshot.revision.clone());
    let plan_spec_revision = string(plan, "spec_revision");
    let plan_current =
        spec_revision.as_deref() == Some(plan_spec_revision)
            && status.report.checks.iter().any(|c| {
                c.check_id == "workflow.packet_integrity" && c.result == CheckResult::Pass
            });
    let host = validation::present(host);
    let evidence_context = host.map(|host| workflow_run::evidence_context(plan, host));
    let bound_host =
        host.is_some_and(|h| workflow_run::bound_host(h, plan, &status.state.code_revision));
    let request = get(plan, "request").expect("validated workplan");
    let mut waiting = BTreeMap::<&str, Vec<String>>::new();
    for pair in items(request, "serialized") {
        if let Value::Array(ids) = pair
            && let [Value::String(before), Value::String(after)] = ids.as_slice()
            && !status
                .state
                .packages
                .get(before)
                .is_some_and(|p| p.status == PackagePhase::Integrated)
        {
            waiting
                .entry(after)
                .or_default()
                .push(format!("Waiting for {before} to be integrated."));
        }
    }
    let gates: Vec<_> = status
        .report
        .checks
        .iter()
        .filter(|c| c.dimension == "readiness" && c.result != CheckResult::Pass)
        .map(|c| format!("{}: {}", c.check_id, describe(c)))
        .collect();
    let checks = match &evidence_context {
        Some((context, false)) if plan_current && bound_host => {
            assess_work_checks(&bundle, &scope, context)?
        }
        _ => Vec::new(),
    };
    let mut packages = Vec::new();
    for packet in items(plan, "packages") {
        let id = string(packet, "id");
        let state = &status.state.packages[id];
        let available = status.available_packages.iter().any(|p| p == id);
        let phase = match state.status {
            PackagePhase::Pending => "pending",
            PackagePhase::Dispatched => "dispatched",
            PackagePhase::Submitted => "submitted",
            PackagePhase::Integrated => "integrated",
        };
        let mut blockers = Vec::new();
        if state.status == PackagePhase::Pending && !available {
            if status.state.status == RunPhase::Cancelled {
                blockers.push("The run is cancelled.".into());
            }
            blockers.extend(waiting.remove(id).unwrap_or_default());
            blockers.extend(gates.iter().cloned());
        }
        let scope: Vec<_> = strings(packet, "scope").map(str::to_owned).collect();
        let nodes = package_nodes(packet);
        let (verification, evidence) = match &evidence_context {
            None => (None, Vec::new()),
            Some(_) if !plan_current => unavailable_verification(
                "Rebuild the workplan for the current model and check declarations.",
            ),
            Some(_) if !bound_host => unavailable_verification(
                "Host evidence must identify this run's implementation and current integrated code revision.",
            ),
            Some((_, true)) => {
                unavailable_verification("Host check selection differs from the workplan.")
            }
            Some((_, false)) => package_checks(packet, &nodes, &checks),
        };
        packages.push(WorkPackage {
            id: id.into(),
            scope,
            nodes,
            phase: phase.into(),
            attempt: state.attempt.clone(),
            available,
            blockers,
            verification,
            evidence,
            issue: None,
        });
    }
    packages.sort_by(|a, b| a.id.cmp(&b.id));
    let run_phase = match status.state.status {
        RunPhase::Active => "active",
        RunPhase::Accepted => "accepted",
        RunPhase::Cancelled => "cancelled",
    };
    Ok(WorkOverview {
        format: FORMAT.into(),
        project_id: project_id.into(),
        spec_revision,
        plan_spec_revision: plan_spec_revision.into(),
        plan_current,
        model_nodes: bundle
            .nodes()
            .map(|n| n.id.to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        run_revision: status.revision,
        run_phase: run_phase.into(),
        evidence_revision: evidence_context
            .as_ref()
            .filter(|(_, conflict)| plan_current && bound_host && !*conflict)
            .map(|_| status.state.code_revision.clone()),
        code_revision: status.state.code_revision,
        acceptance_current: status.acceptance_current,
        packages,
    })
}

/// Format a captured report without reopening model or workflow files.
pub fn work_overview_text(overview: &WorkOverview, scope: &[String]) -> String {
    let mut output = format!(
        "{} — {}\nCode revision: {}\n",
        overview.project_id, overview.run_phase, overview.code_revision
    );
    if overview.run_phase == "accepted" {
        output.push_str(if overview.acceptance_current {
            "Acceptance verified for this capture.\n"
        } else {
            "Recorded acceptance is unverified for this capture.\n"
        });
    }
    if !overview.plan_current {
        output.push_str("The recorded workplan is out of date or unresolved.\n");
    }
    let packages = overview.packages_for(scope);
    if packages.is_empty() {
        output.push_str("No work packages for the selected nodes.\n");
    }
    for p in packages {
        let state = if p.available { "available" } else { &p.phase };
        let verified = p
            .verification
            .map(|result| result.to_string())
            .unwrap_or_else(|| "unassessed".into());
        writeln!(
            output,
            "{}  {}  [{}]  verification {}",
            p.id,
            state,
            p.scope.join(", "),
            verified
        )
        .unwrap();
        for reason in &p.blockers {
            writeln!(output, "  {reason}").unwrap();
        }
        for check in &p.evidence {
            writeln!(output, "  {} {}  {}", check.id, check.result, check.reason).unwrap();
            if let Some(revision) = &check.code_revision {
                writeln!(
                    output,
                    "    {} at {}",
                    check.evidence_refs.join(", "),
                    revision
                )
                .unwrap();
            }
        }
        if let Some(issue) = &p.issue {
            writeln!(output, "  Issue: {}", issue.url).unwrap();
            if let Some(git) = &issue.github {
                writeln!(output, "    {} ({})", git.title, git.state).unwrap();
                if !git.assignees.is_empty() {
                    writeln!(output, "    Assigned to {}", git.assignees.join(", ")).unwrap();
                }
            }
        }
    }
    output
}
