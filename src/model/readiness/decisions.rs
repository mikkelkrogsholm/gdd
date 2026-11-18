use super::{Model, Value, get, string, strings, text};
use std::collections::{BTreeMap, BTreeSet};

/// Return the current trusted accepted decisions that remain applicable after
/// compatible supersession. Only decision records can confer readiness.
pub(in crate::model) fn accepted<'a>(
    model: &Model<'a>,
    host: Option<&Value>,
) -> BTreeMap<&'a str, &'a Value> {
    let Some(revision) = model.bundle.snapshot().map(|s| s.revision.as_str()) else {
        return BTreeMap::new();
    };
    let trusted: BTreeSet<_> = host
        .into_iter()
        .flat_map(|h| strings(h, "trusted_decisions"))
        .collect();
    if trusted.is_empty() {
        return BTreeMap::new();
    }
    let records: BTreeMap<_, _> = model.bundle.records().map(|r| (r.id, r.value)).collect();
    let invalid = model.invalid_records(&records, Some(revision));
    let mut retired = BTreeSet::new();
    for (&id, &decision) in &records {
        if !trusted.contains(id)
            || invalid.contains(id)
            || string(decision, "kind") != "decision"
            || !matches!(string(decision, "outcome"), "accepted" | "rejected")
        {
            continue;
        }
        if let Some(previous) = get(decision, "supersedes").and_then(text)
            && !invalid.contains(previous)
            && let Some(prior) = records.get(previous)
            && crate::model::records::compatible_decision(decision, prior)
        {
            retired.insert(previous);
        }
    }
    records
        .into_iter()
        .filter(|(id, record)| {
            trusted.contains(id)
                && !invalid.contains(id)
                && !retired.contains(id)
                && string(record, "kind") == "decision"
                && string(record, "outcome") == "accepted"
                && get(record, "spec_revision").and_then(text) == Some(revision)
        })
        .collect()
}

pub(in crate::model) fn resolves(question: &Value, accepted: &BTreeMap<&str, &Value>) -> bool {
    get(question, "resolution")
        .and_then(|r| get(r, "decision"))
        .and_then(text)
        .and_then(|id| accepted.get(id))
        .is_some_and(|decision| {
            super::items(decision, "scope").is_empty()
                || strings(question, "targets")
                    .all(|target| strings(decision, "scope").any(|member| member == target))
        })
}
