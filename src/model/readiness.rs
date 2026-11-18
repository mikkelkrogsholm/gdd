//! Structural readiness of the selected implementation targets and their read context.

use super::{Model, blank_text, entries, escape, flag, get, items, string, strings, text};
use crate::bundle::DocumentKind;
use crate::context::{ContextGraph, ContextScope};
use crate::input::Value;
use crate::report::{Check, CheckResult, Diagnostic};
use crate::validation::CORE_KINDS;
use std::collections::{BTreeMap, BTreeSet};

pub(super) mod decisions;
mod sources;

const MISSING: &str = "READINESS_CONTRACT_MISSING";
const SCENARIO: &str = "SCENARIO_CATEGORY_MISSING";

pub(super) fn check<'a>(
    model: &Model<'a>,
    graph: &ContextGraph<'a>,
    scope: &[&'a str],
    host: Option<&Value>,
) -> (Vec<Check>, ContextScope<'a>) {
    let seeds: Vec<_> = if scope.is_empty() {
        model
            .nodes
            .iter()
            .filter_map(|(&id, node)| (!support(string(node, "kind"))).then_some(id))
            .collect()
    } else {
        scope.to_vec()
    };
    let context = graph.scope(seeds.iter().copied());
    let mut readiness = Readiness {
        model,
        checks: Vec::new(),
        scenarios: BTreeMap::new(),
        decisions: decisions::accepted(model, host),
    };
    for edge in model.bundle.model_edges() {
        let relation = string(edge.value, "relation");
        if matches!(relation, "exercises" | "verifies") {
            readiness
                .scenarios
                .entry((string(edge.value, "to"), relation))
                .or_default()
                .insert(string(edge.value, "from"));
        }
    }
    let mut missing_targets = BTreeSet::new();
    for id in seeds {
        if model.kind(id).is_none() && missing_targets.insert(id) {
            readiness.fail(id, "REFERENCE_UNRESOLVED", "Unknown readiness target.", &[]);
        }
    }
    for &id in &context.implementation_targets {
        if model.kind(id).is_some_and(support) {
            readiness.fail(id, MISSING,
                "Implementation scope must be component/entity/action/screen/machine or a declared extension kind.", &[]);
        }
    }
    readiness.model_findings(&context);
    for id in context.nodes() {
        if let Some(&node) = model.nodes.get(id) {
            readiness.node(id, node, context.implementation_targets.contains(id));
        }
    }
    for edge in model.bundle.model_edges() {
        let from = string(edge.value, "from");
        let to = string(edge.value, "to");
        if context.contains(from) && context.contains(to) {
            if model.manifest.kind == DocumentKind::Manifest03
                && string(edge.value, "relation") == "calls"
            {
                let call =
                    super::boundaries::resolve_call(edge.id, edge.value, &model.nodes, |id| {
                        model.owner(id)
                    });
                if call.status == super::boundaries::CallStatus::Unresolved {
                    readiness.fail(to, "PUBLIC_CONTRACT_UNRESOLVED",
                        "Call needs unique consumer/provider owners and a specified public interface.", &[]);
                }
            }
            readiness.references(edge.id, edge.value);
        }
    }
    for (id, question) in entries(&model.manifest.value, "questions") {
        if !strings(question, "targets").any(|target| context.contains(target))
            || !strings(question, "blocks").any(|block| block == "implementation")
        {
            continue;
        }
        if string(question, "status") == "open" {
            readiness.fail(
                id,
                "QUESTION_BLOCKS_SCOPE",
                "Open question blocks implementation context.",
                &[],
            );
        } else if !decisions::resolves(question, &readiness.decisions) {
            readiness.unknown(
                id,
                "QUESTION_BLOCKS_SCOPE",
                "Resolved question has no applicable trusted current decision.",
            );
        }
    }
    if readiness.checks.is_empty() {
        let mut pass = Check::new(
            "readiness.structural",
            "readiness",
            CheckResult::Pass,
            "Selected targets and required boundary contracts are structurally specified; no behavioral proof claimed.",
        );
        pass.scope = context
            .implementation_targets
            .iter()
            .copied()
            .map(str::to_owned)
            .collect();
        readiness.checks.push(pass);
    }
    (readiness.checks, context)
}

struct Readiness<'m, 'a> {
    model: &'m Model<'a>,
    scenarios: BTreeMap<(&'a str, &'a str), BTreeSet<&'a str>>,
    decisions: BTreeMap<&'a str, &'a Value>,
    checks: Vec<Check>,
}

fn support(kind: &str) -> bool {
    matches!(kind, "rule" | "scenario" | "community")
}

fn blank(value: &Value, field: &str) -> bool {
    get(value, field).and_then(text).is_none_or(blank_text)
}

impl Readiness<'_, '_> {
    fn issue(
        &mut self,
        id: &str,
        code: &'static str,
        result: CheckResult,
        message: impl Into<String>,
        fields: &[&str],
    ) {
        let location = self.model.bundle.location(id);
        let mut pointer = location.map_or_else(String::new, |loc| loc.pointer.clone());
        let mut value = self.model.nodes.get(id).copied();
        for &field in fields {
            value = value.and_then(|node| get(node, field));
            if value.is_none() {
                break;
            }
            pointer.push('/');
            pointer.push_str(&escape(field));
        }
        let file = location.map_or(&self.model.manifest.asset.path, |loc| &loc.file);
        let mut check = Check::new(
            format!("check.{}", code.to_ascii_lowercase()),
            "readiness",
            result,
            message,
        );
        check.diagnostics.push(
            Diagnostic::new(code, result, &check.reason, Some(file), pointer)
                .with_related_ids([id]),
        );
        self.checks.push(check);
    }
    fn fail(&mut self, id: &str, code: &'static str, message: impl Into<String>, fields: &[&str]) {
        self.issue(id, code, CheckResult::Fail, message, fields);
    }
    fn unknown(&mut self, id: &str, code: &'static str, message: impl Into<String>) {
        self.issue(id, code, CheckResult::Unknown, message, &[]);
    }

    fn model_findings(&mut self, context: &ContextScope<'_>) {
        let record_only = self.model.record_only_ids();
        for check in self
            .model
            .checks
            .iter()
            .filter(|c| c.dimension == "model" && c.result != CheckResult::Pass)
        {
            let related: BTreeSet<_> = check
                .diagnostics
                .iter()
                .flat_map(|d| d.related_ids.iter().map(String::as_str))
                .collect();
            if !related.is_empty() && related.is_subset(&record_only) {
                continue;
            }
            if related.is_empty()
                || related
                    .iter()
                    .any(|id| context.contains(id) || !self.model.nodes.contains_key(id))
            {
                let code = if check.result == CheckResult::Unknown {
                    "EXTENSION_UNSUPPORTED"
                } else {
                    MISSING
                };
                self.issue(
                    context
                        .implementation_targets
                        .first()
                        .copied()
                        .unwrap_or("project"),
                    code,
                    check.result,
                    format!(
                        "Model defect or unknown semantics intersects readiness context: {}",
                        check.reason
                    ),
                    &[],
                );
            }
        }
    }

    fn scenario(&self, id: &str) -> Option<&Value> {
        self.model
            .nodes
            .get(id)
            .and_then(|node| get(node, "scenario"))
    }

    fn exercising(&self, id: &str, scenario: &str, categories: &[&str]) -> bool {
        self.scenarios
            .get(&(id, "exercises"))
            .is_some_and(|set| set.contains(scenario))
            && self
                .scenario(scenario)
                .is_some_and(|s| categories.contains(&string(s, "category")))
    }

    fn has_normal(&self, id: &str) -> bool {
        self.scenarios.get(&(id, "exercises")).is_some_and(|set| {
            set.iter().any(|s| {
                self.scenario(s)
                    .is_some_and(|node| string(node, "category") == "normal")
            })
        })
    }

    fn node(&mut self, id: &str, node: &Value, selected: bool) {
        match string(node, "kind") {
            "action" => self.action(id, node),
            "entity" if get(node, "fields").is_none() => self.fail(
                id,
                MISSING,
                "Entity fields are missing; specify the field/type map.",
                &["fields"],
            ),
            "machine" if get(node, "machine").is_none() => self.fail(
                id,
                MISSING,
                "Machine definition is missing; specify states and transitions.",
                &["machine"],
            ),
            "component" => self.component(id, node),
            "screen" if selected => {
                if get(node, "interaction").is_none() {
                    self.fail(
                        id,
                        MISSING,
                        "Screen interaction contract is incomplete.",
                        &[],
                    );
                }
                if items(node, "design_refs").is_empty() {
                    self.fail(id, MISSING, "Screen requires resolved design_refs.", &[]);
                }
                if !self.has_normal(id) {
                    self.fail(id, SCENARIO, "Screen needs normal scenario.", &[]);
                }
                if !self.decisions.values().any(|d| {
                    string(d, "topic") == "readiness_review"
                        && (items(d, "scope").is_empty()
                            || strings(d, "scope").any(|target| target == id))
                }) {
                    self.unknown(
                        id,
                        "RECORD_INVALID",
                        "Screen requires current trusted readiness_review decision.",
                    );
                }
            }
            "rule" => match get(node, "rule") {
                None => self.fail(id, MISSING, "Applicable rule is unspecified.", &[]),
                Some(rule)
                    if flag(rule, "blocking")
                        && !self
                            .scenarios
                            .get(&(id, "verifies"))
                            .is_some_and(|set| set.iter().any(|s| self.scenario(s).is_some())) =>
                {
                    self.fail(
                        id,
                        SCENARIO,
                        "Blocking rule lacks planned verifying scenario.",
                        &[],
                    )
                }
                _ => {}
            },
            kind if !CORE_KINDS.contains(&kind) => self.unknown(
                id,
                "EXTENSION_UNSUPPORTED",
                "Custom kinds have no core implementation-readiness contract.",
            ),
            _ => {}
        }
        self.references(id, node);
    }

    fn component(&mut self, id: &str, node: &Value) {
        if get(node, "interfaces").is_none() {
            self.fail(
                id,
                MISSING,
                "Component interfaces are missing; list public action IDs or explicitly use [].",
                &["interfaces"],
            );
        }
        if self.model.manifest.kind != DocumentKind::Manifest03 {
            return;
        }
        let boundary = get(node, "boundary").unwrap_or(&Value::Null);
        let missing: Vec<_> = ["responsibility", "rationale", "excludes"]
            .into_iter()
            .filter(|&field| {
                get(boundary, field).is_none() || field != "excludes" && blank(boundary, field)
            })
            .collect();
        if let Some(&first) = missing.first() {
            self.fail(id, "BOUNDARY_RATIONALE_MISSING", format!("Component boundary needs fields: {}; responsibility and rationale must be nonblank; excludes may be [].", missing.join(", ")), &["boundary", first]);
        }
        let mut invalid: Vec<_> = strings(node, "interfaces")
            .filter(|action| self.model.owner(action) != Some(id))
            .collect();
        if !invalid.is_empty() {
            invalid.sort_unstable();
            self.fail(id, MISSING, format!("Public interface actions need exactly one owned_by edge to this component: {}.", invalid.join(", ")), &["interfaces"]);
        }
    }

    fn action(&mut self, id: &str, node: &Value) {
        let contract = get(node, "contract").unwrap_or(&Value::Null);
        let missing: Vec<_> = [
            "context",
            "dependencies_complete",
            "effects_complete",
            "error_cases",
            "errors",
            "external_effects",
            "input",
            "output",
            "postconditions",
            "preconditions",
            "risks",
        ]
        .into_iter()
        .filter(|&field| {
            (field != "postconditions" || self.model.manifest.kind == DocumentKind::Manifest03)
                && get(contract, field).is_none()
        })
        .collect();
        if !missing.is_empty() {
            self.fail(
                id,
                MISSING,
                format!("Action contract is missing fields: {}.", missing.join(", ")),
                &["contract"],
            );
        }
        if blank(node, "behavior") {
            self.fail(
                id,
                MISSING,
                "Action behavior is missing or blank; specify its ordered behavior.",
                &["behavior"],
            );
        }
        let incomplete: Vec<_> = ["effects_complete", "dependencies_complete"]
            .into_iter()
            .filter(|&field| !flag(contract, field))
            .collect();
        if let Some(&first) = incomplete.first() {
            self.fail(
                id,
                "EFFECTS_INCOMPLETE",
                format!(
                    "Action contract must declare true: {}.",
                    incomplete.join(", ")
                ),
                &["contract", first],
            );
        }
        if self.model.owner(id).is_none() {
            self.fail(id, MISSING, "Action requires exactly one owner.", &[]);
        }
        if !self.has_normal(id) {
            self.fail(id, SCENARIO, "Action needs a normal scenario.", &[]);
        }
        for name in [
            "authorization",
            "idempotency",
            "boundaries",
            "concurrency",
            "atomicity",
        ] {
            let risk = get(contract, "risks")
                .and_then(|risks| get(risks, name))
                .unwrap_or(&Value::Null);
            if blank(risk, "rationale") {
                self.fail(
                    id,
                    MISSING,
                    format!("Risk assessment {name} needs a nonblank rationale."),
                    &["contract", "risks", name, "rationale"],
                );
            } else if flag(risk, "relevant") {
                let categories: &[&str] = if name == "concurrency" {
                    &["concurrency"]
                } else {
                    &["boundary", "negative"]
                };
                if items(risk, "scenarios").is_empty()
                    || strings(risk, "scenarios").any(|s| !self.exercising(id, s, categories))
                {
                    self.fail(
                        id,
                        SCENARIO,
                        format!(
                            "Risk {name} needs exercising scenarios of category {{'{}'}}.",
                            categories.join("', '")
                        ),
                        &[],
                    );
                }
            }
        }
        let cases = get(contract, "error_cases").unwrap_or(&Value::Null);
        for (name, _) in entries(contract, "errors") {
            if items(cases, name).is_empty()
                || strings(cases, name).any(|s| !self.exercising(id, s, &["negative", "boundary"]))
            {
                self.fail(
                    id,
                    SCENARIO,
                    format!("Error {name} needs negative/boundary scenario."),
                    &[],
                );
            }
        }
    }

    fn references(&mut self, id: &str, value: &Value) {
        for field in ["source_refs", "design_refs"] {
            for reference in items(value, field) {
                if string(reference, "purpose") == "contract" {
                    self.source(id, reference);
                } else if field == "design_refs" {
                    self.fail(
                        id,
                        MISSING,
                        "Design references must have purpose=contract.",
                        &[],
                    );
                    self.source(id, reference);
                }
            }
        }
    }
}
