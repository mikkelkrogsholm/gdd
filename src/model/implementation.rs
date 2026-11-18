use super::context::ContextGraph;
use super::records::{check_digest, covers};
use super::{Model, Value, flag, get, string, strings, text};
use crate::report::{Check, CheckDetails, CheckResult, Diagnostic};
use std::collections::{BTreeMap, BTreeSet};

type Records<'a> = BTreeMap<&'a str, &'a Value>;

fn retired<'a>(records: &Records<'a>, invalid: &BTreeSet<&str>, host: &Value) -> BTreeSet<&'a str> {
    records
        .iter()
        .filter_map(|(id, r)| {
            let previous = get(r, "supersedes").and_then(text)?;
            let prior = records.get(previous)?;
            let kind = string(r, "kind");
            if invalid.contains(id) || invalid.contains(previous) || kind != string(prior, "kind") {
                return None;
            }
            let (fields, targets): (&[&str], &str) = match kind {
                "check" => (&["spec_revision", "method"], "scope"),
                "binding" if strings(host, "verified_bindings").any(|s| s == *id) => (
                    &["implementation", "spec_revision", "code_revision"],
                    "targets",
                ),
                "evidence" if strings(host, "trusted_evidence").any(|s| s == *id) => (
                    &["check", "implementation", "spec_revision", "code_revision"],
                    "scope",
                ),
                _ => return None,
            };
            (fields
                .iter()
                .all(|field| get(r, field) == get(prior, field))
                && covers(r, prior, targets))
            .then_some(previous)
        })
        .collect()
}

fn unresolved(id: &str, scope: &[&str], reason: &str, code: &'static str, message: &str) -> Check {
    let mut check = Check::new(id, "implementation", CheckResult::Unknown, reason);
    check.scope = scope.iter().copied().map(str::to_owned).collect();
    check
        .diagnostics
        .push(Diagnostic::new(code, check.result, message, None, ""));
    check
}

pub(super) fn check<'a>(
    model: &Model<'a>,
    graph: &ContextGraph<'a>,
    scope: &[&str],
    host: &Value,
) -> Vec<Check> {
    if scope.iter().any(|id| !model.nodes.contains_key(id)) {
        return vec![unresolved(
            "implementation.scope",
            scope,
            "Requested scope does not resolve.",
            "REFERENCE_UNRESOLVED",
            "Unknown requested scope.",
        )];
    }

    let ids = model.model_id_counts();
    let ambiguous: BTreeSet<_> = ids
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|(&id, _)| id)
        .collect();
    let ambiguous_request: Vec<_> = ambiguous
        .iter()
        .copied()
        .filter(|id| scope.is_empty() || scope.contains(id))
        .collect();
    let mut checks = Vec::new();
    if !ambiguous_request.is_empty() {
        let mut check = unresolved(
            "implementation.scope",
            scope,
            "Requested scope contains ambiguous model identities.",
            "REFERENCE_UNRESOLVED",
            "Requested scope does not resolve unambiguously.",
        );
        check.diagnostics[0].related_ids =
            ambiguous_request.into_iter().map(str::to_owned).collect();
        checks.push(check);
    }
    let closure = (!scope.is_empty()).then(|| graph.scope(scope.iter().copied()));
    let requirements: Vec<_> = model
        .nodes
        .iter()
        .filter(|(id, _)| closure.as_ref().is_none_or(|c| c.contains(id)))
        .filter_map(|(&id, node)| {
            get(node, "rule")
                .filter(|r| flag(r, "blocking"))
                .map(|r| (id, r))
        })
        .flat_map(|(id, r)| strings(r, "verification").map(move |method| (id, method)))
        .collect();
    if requirements.is_empty() {
        checks.push(unresolved(
            "implementation.requirements",
            scope,
            "No explicit required check set; no vacuous implementation PASS.",
            "CHECK_PLAN_MISSING",
            "Required check set is absent.",
        ));
        return checks;
    }
    let revision = model.bundle.snapshot().map(|s| s.revision.as_str());
    let mut active: Records<'_> = model.bundle.records().map(|r| (r.id, r.value)).collect();
    let invalid = model.invalid_records(&active, revision);
    let retired = retired(&active, &invalid, host);
    active.retain(|id, _| !retired.contains(id));
    let mapped: BTreeSet<_> = active
        .iter()
        .filter(|(id, r)| {
            string(r, "kind") == "binding"
                && !invalid.contains(*id)
                && strings(host, "verified_bindings").any(|s| s == **id)
                && matches!(string(r, "status"), "observed" | "checked")
                && get(r, "spec_revision").and_then(text) == revision
                && ["implementation", "code_revision"]
                    .iter()
                    .all(|f| get(r, f).and_then(text) == get(host, f).and_then(text))
                && matches!(string(r, "role"), "code" | "test")
        })
        .flat_map(|(_, r)| strings(r, "targets"))
        .collect();
    let assessment = Assessment {
        model,
        graph,
        host,
        active,
        invalid,
        ids,
        ambiguous,
        mapped,
        revision,
    };
    for (index, (rule, method)) in requirements.into_iter().enumerate() {
        checks.push(assessment.required(index, rule, method));
    }
    checks
}

struct Assessment<'m, 'a> {
    model: &'m Model<'a>,
    graph: &'m ContextGraph<'a>,
    host: &'m Value,
    active: Records<'a>,
    invalid: BTreeSet<&'a str>,
    ids: BTreeMap<&'a str, usize>,
    ambiguous: BTreeSet<&'a str>,
    mapped: BTreeSet<&'a str>,
    revision: Option<&'a str>,
}

impl Assessment<'_, '_> {
    fn required(&self, index: usize, rule: &str, method: &str) -> Check {
        let mut check = Check::new(
            format!("implementation.required.{index}"),
            "implementation",
            CheckResult::Unknown,
            format!(
                "Applicability of recorded {rule}/{method}; application check not executed by validator."
            ),
        );
        check.scope.push(rule.into());
        let mut unknown = |code, message: String| {
            check.diagnostics.push(
                Diagnostic::new(code, CheckResult::Unknown, message, None, "")
                    .with_related_ids([rule]),
            )
        };
        let choice = ["check_selection", rule, method]
            .into_iter()
            .try_fold(self.host, get)
            .and_then(text);
        let mut plans = self.active.iter().filter(|(id, r)| {
            choice.is_none_or(|s| s == **id)
                && string(r, "kind") == "check"
                && get(r, "spec_revision").and_then(text) == self.revision
                && string(r, "method") == method
                && strings(r, "scope").any(|s| s == rule)
        });
        let (Some((&check_id, &plan)), None) = (plans.next(), plans.next()) else {
            unknown(
                "CHECK_PLAN_MISSING",
                format!("No unambiguous active plan for {rule}/{method}."),
            );
            return check;
        };
        if self.invalid.contains(check_id) {
            unknown(
                "RECORD_INVALID",
                "Selected check plan has invalid record integrity.".into(),
            );
        }
        let implementation = get(self.host, "implementation").and_then(text);
        let code_revision = get(self.host, "code_revision").and_then(text);
        let choice = ["evidence_selection", check_id]
            .into_iter()
            .try_fold(self.host, get)
            .and_then(text);
        let mut observations = self.active.iter().filter(|(id, r)| {
            choice.is_none_or(|s| s == **id)
                && string(r, "kind") == "evidence"
                && string(r, "check") == check_id
                && implementation.is_none_or(|s| s == string(r, "implementation"))
        });
        let (Some((&evidence_id, &evidence)), None) = (observations.next(), observations.next())
        else {
            unknown(
                "REVISION_UNRESOLVED",
                format!("No unambiguous evidence for {check_id}."),
            );
            return check;
        };
        if self.invalid.contains(evidence_id) {
            unknown(
                "RECORD_INVALID",
                "Selected evidence has invalid record integrity.".into(),
            );
        }
        let current = self.revision.is_some()
            && self.revision == get(evidence, "spec_revision").and_then(text);
        if current && strings(evidence, "scope").any(|s| !self.ids.contains_key(s)) {
            unknown(
                "RECORD_INVALID",
                "Evidence scope contains unknown current model IDs.".into(),
            );
        }
        if string(evidence, "check_revision") != check_digest(plan) {
            unknown(
                "CHECK_REVISION_MISMATCH",
                "Evidence does not match immutable check digest.".into(),
            );
        }
        if !current
            || implementation.is_none()
            || implementation != get(evidence, "implementation").and_then(text)
            || code_revision.is_none()
            || code_revision != get(evidence, "code_revision").and_then(text)
        {
            unknown(
                "EVIDENCE_STALE",
                "Evidence does not match requested spec, implementation and code revision.".into(),
            );
        }
        if !strings(self.host, "supported_methods").any(|s| s == method)
            || !strings(self.host, "trusted_evidence").any(|s| s == evidence_id)
            || !flag(evidence, "executed")
        {
            unknown(
                "REVISION_UNRESOLVED",
                "Checker method/execution provenance is not host-verified.".into(),
            );
        }
        for environment in strings(plan, "required_environment") {
            let actual = get(evidence, "environment").and_then(|e| get(e, environment));
            if actual.is_none()
                || actual != get(self.host, "environment").and_then(|e| get(e, environment))
            {
                unknown(
                    "REQUIRED_ENVIRONMENT_MISSING",
                    format!("Required actual environment not verified: {environment}."),
                );
            }
        }
        if !covers(self.host, plan, "negative_controls") {
            unknown(
                "REVISION_UNRESOLVED",
                "Declared negative controls lack host-verified execution.".into(),
            );
        }
        if !covers(evidence, plan, "scope") {
            unknown(
                "REVISION_UNRESOLVED",
                "Evidence does not cover plan scope.".into(),
            );
        }
        let constraint = get(self.model.nodes[rule], "rule").and_then(|r| get(r, "constraint"));
        let relevant: BTreeSet<_> = self
            .model
            .targets(rule, "constrains")
            .iter()
            .copied()
            .chain(
                ["from", "to"]
                    .into_iter()
                    .filter_map(|k| constraint.and_then(|c| get(c, k)).and_then(text)),
            )
            .collect();
        if !self.mapped.contains(rule) && (relevant.is_empty() || !relevant.is_subset(&self.mapped))
        {
            unknown(
                "BINDING_REVISION_MISSING",
                "No host-verified current implementation binding.".into(),
            );
        }
        if !check.diagnostics.is_empty() {
            return check;
        }
        check.result = match string(evidence, "result") {
            "PASS"
                if !self.ambiguous.is_empty()
                    && self
                        .graph
                        .scope([rule])
                        .nodes()
                        .any(|s| self.ambiguous.contains(s)) =>
            {
                check.diagnostics.push(
                    Diagnostic::new(
                        "REFERENCE_UNRESOLVED",
                        CheckResult::Unknown,
                        "Required rule context contains ambiguous model node identities.",
                        None,
                        "",
                    )
                    .with_related_ids([rule]),
                );
                CheckResult::Unknown
            }
            "PASS" => CheckResult::Pass,
            "FAIL" => CheckResult::Fail,
            _ => CheckResult::Unknown,
        };
        if matches!(check.result, CheckResult::Pass | CheckResult::Fail) {
            check.details = Some(CheckDetails::Implementation {
                implementation: implementation.expect("checked implementation").into(),
                code_revision: code_revision.expect("checked revision").into(),
                evidence_refs: vec![evidence_id.into()],
            });
        }
        check
    }
}
