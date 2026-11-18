use super::{Model, Value, flag, get, items, records, string, strings, text};
use crate::report::{Check, CheckDetails, CheckResult, Diagnostic};
use crate::validation::Activity;
use std::collections::{BTreeMap, BTreeSet};

struct Decision<'a> {
    result: CheckResult,
    reason: &'static str,
    basis: Option<&'a str>,
}

impl<'a> Decision<'a> {
    fn unresolved() -> Self {
        Self {
            result: CheckResult::Unresolved,
            reason: "No applicable host-trusted policy and actor authority.",
            basis: None,
        }
    }

    fn deny(reason: &'static str) -> Self {
        Self {
            result: CheckResult::Denied,
            reason,
            basis: None,
        }
    }

    fn grant(reason: &'static str, basis: &'a str) -> Self {
        Self {
            result: CheckResult::Granted,
            reason,
            basis: Some(basis),
        }
    }
}

fn allows(record: &Value, scope: &[&str]) -> bool {
    items(record, "scope").is_empty()
        || (!scope.is_empty()
            && scope
                .iter()
                .all(|id| strings(record, "scope").any(|s| s == *id)))
}

pub(super) fn check(model: &Model<'_>, activity: Activity, scope: &[&str], host: &Value) -> Check {
    let policy = get(host, "policy");
    let mut decision = Decision::unresolved();
    let unknown_scope = scope.iter().any(|id| !model.nodes.contains_key(id));
    if unknown_scope {
        decision.reason = "Unknown scope cannot be authorized.";
    } else if let Some(policy) = policy {
        decision = resolve(model, activity, scope, host, policy);
        if decision.result == CheckResult::Granted
            && model
                .model_id_counts()
                .into_iter()
                .any(|(id, n)| n > 1 && (scope.is_empty() || scope.contains(&id)))
        {
            decision = Decision {
                result: CheckResult::Unresolved,
                reason: "Requested scope contains an ambiguous model identity.",
                basis: None,
            };
        }
    }
    let mut check = Check::new(
        if unknown_scope {
            "authorization.scope"
        } else {
            "authorization.activity"
        },
        "authorization",
        decision.result,
        decision.reason,
    );
    check.scope = scope.iter().copied().map(str::to_owned).collect();
    check.details = Some(CheckDetails::Authorization {
        activity,
        policy_revision: policy
            .filter(|_| !unknown_scope)
            .map(|p| string(p, "revision").into()),
        authority_basis: decision.basis.map(str::to_owned),
    });
    if decision.result != CheckResult::Granted {
        let code = if decision.result == CheckResult::Denied {
            "AUTHORIZATION_DENIED"
        } else {
            "AUTHORIZATION_UNRESOLVED"
        };
        check.diagnostics.push(Diagnostic::new(
            code,
            decision.result,
            if unknown_scope {
                "Requested node does not exist."
            } else {
                decision.reason
            },
            None,
            "",
        ));
    }
    check
}

fn resolve<'a>(
    model: &Model<'a>,
    activity: Activity,
    scope: &[&str],
    host: &'a Value,
    policy: &'a Value,
) -> Decision<'a> {
    let revision = model.bundle.snapshot().map(|s| s.revision.as_str());
    let matches = activity.as_str() == string(policy, "activity")
        && revision == get(policy, "spec_revision").and_then(text)
        && revision.is_some();
    if flag(policy, "denied") || (matches && !allows(policy, scope)) {
        return Decision::deny("Host policy denies activity or requested scope.");
    }
    if !matches {
        return Decision::unresolved();
    }
    let gate = ["project", "workflow", "gates", activity.as_str()]
        .into_iter()
        .try_fold(&model.manifest.value, get);
    if gate.is_some_and(|g| !allows(g, scope)) {
        return Decision::deny("Requested scope exceeds model gate.");
    }
    let authorized = |actor: &str| {
        strings(policy, "authorities").any(|id| id == actor)
            && gate.is_none_or(|g| {
                items(g, "authorities").is_empty()
                    || strings(g, "authorities").any(|id| id == actor)
            })
    };
    if !authorized(string(policy, "actor"))
        || (activity == Activity::Acceptance
            && ["implementation", "code_revision"]
                .iter()
                .any(|field| get(host, field).and_then(text).is_none_or(str::is_empty)))
    {
        return Decision::unresolved();
    }
    let records: BTreeMap<_, _> = model.bundle.records().map(|r| (r.id, r.value)).collect();
    let invalid = model.invalid_records(&records, revision);
    let trusted: BTreeSet<_> = strings(host, "trusted_decisions").collect();
    let applicable: BTreeMap<_, _> = records
        .into_iter()
        .filter(|(id, r)| {
            trusted.contains(id)
                && string(r, "kind") == "decision"
                && string(r, "topic") == "approval"
                && get(r, "activity").and_then(text) == Some(activity.as_str())
                && matches!(string(r, "outcome"), "accepted" | "rejected")
                && get(r, "spec_revision").and_then(text) == revision
                && get(r, "policy_revision") == get(policy, "revision")
                && get(r, "actor").is_some_and(|a| authorized(string(a, "id")))
                && (activity != Activity::Acceptance
                    || ["implementation", "code_revision"]
                        .iter()
                        .all(|field| get(r, field) == get(host, field)))
        })
        .collect();
    let retired: BTreeSet<_> = applicable
        .iter()
        .filter_map(|(id, r)| {
            let previous = get(r, "supersedes").and_then(text)?;
            let prior = applicable.get(previous)?;
            (!invalid.contains(id)
                && !invalid.contains(previous)
                && records::compatible_decision(r, prior))
            .then_some(previous)
        })
        .collect();
    let mut approvals = Vec::new();
    for (id, r) in applicable {
        if retired.contains(id) {
            continue;
        }
        if string(r, "outcome") == "rejected" {
            if items(r, "scope").is_empty()
                || scope.is_empty()
                || strings(r, "scope").any(|s| scope.contains(&s))
            {
                return Decision::deny("Applicable explicit rejection.");
            }
        } else if !invalid.contains(id) && allows(r, scope) {
            approvals.push(id);
        }
    }
    if string(policy, "requirement") != "explicit"
        && gate.is_none_or(|g| string(g, "requirement") != "explicit")
    {
        Decision::grant(
            "Applicable host-trusted standing mandate.",
            string(policy, "basis"),
        )
    } else if let [approval] = approvals.as_slice() {
        Decision::grant("Applicable host-authenticated approval receipt.", approval)
    } else {
        Decision::unresolved()
    }
}
