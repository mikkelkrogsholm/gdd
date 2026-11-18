use super::{Model, Value, get, items, string, strings, text};
use std::collections::{BTreeMap, BTreeSet};

/// Return the current trusted accepted decisions that remain applicable after
/// compatible supersession. Only decision records can confer readiness.
pub(super) fn accepted<'a>(
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
    let invalid = model.invalid_plans(&records, "decision", revision);
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
            && compatible(decision, prior)
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

fn compatible(replacement: &Value, previous: &Value) -> bool {
    [
        "kind",
        "topic",
        "spec_revision",
        "policy_revision",
        "activity",
        "implementation",
        "code_revision",
    ]
    .into_iter()
    .all(|key| get(replacement, key) == get(previous, key))
        && (items(replacement, "scope").is_empty()
            || !items(previous, "scope").is_empty()
                && strings(previous, "scope")
                    .all(|s| strings(replacement, "scope").any(|member| member == s)))
}
