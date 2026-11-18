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
    let model_ids = model.model_id_counts();
    let mut invalid = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for (&id, record) in &records {
        if string(record, "kind") != "decision" {
            continue;
        }
        if model_ids.contains_key(id)
            || get(record, "spec_revision").and_then(text) == Some(revision)
                && strings(record, "scope").any(|s| model_ids.get(s) != Some(&1))
            || strings(record, "checks").any(|s| {
                model_ids.contains_key(s)
                    || records.get(s).is_none_or(|r| string(r, "kind") != "check")
            })
            || get(record, "supersedes").and_then(text).is_some_and(|s| {
                model_ids.contains_key(s)
                    || records
                        .get(s)
                        .is_none_or(|r| string(r, "kind") != "decision")
            })
        {
            invalid.insert(id);
        }
        let mut chain = Vec::new();
        let mut positions = BTreeMap::new();
        let mut current = id;
        while let Some(record) = records.get(current) {
            if visited.contains(current) {
                break;
            }
            if let Some(pos) = positions.insert(current, chain.len()) {
                invalid.extend(chain[pos..].iter().copied());
                break;
            }
            chain.push(current);
            let Some(previous) = get(record, "supersedes").and_then(text) else {
                break;
            };
            current = previous;
        }
        visited.extend(chain);
    }
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
