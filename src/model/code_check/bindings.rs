use super::{Inventory, diagnostic, glob};
use crate::model::{Model, get, string, strings};
use crate::validation::to_json;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io;

pub(super) struct Bindings {
    pub selected: BTreeMap<String, Value>,
    pub components: BTreeMap<String, String>,
    pub diagnostics: Vec<Value>,
    pub advisory: Vec<Value>,
}

pub(super) fn select(
    model: &Model<'_>,
    implementation: &str,
    revision: &str,
    inventory: &Inventory,
    hashes: &BTreeMap<String, String>,
) -> io::Result<Bindings> {
    let records: BTreeMap<_, _> = model.bundle.records().map(|r| (r.id, r.value)).collect();
    let invalid = model.invalid_records(&records, Some(revision));
    let mut result = Bindings {
        selected: BTreeMap::new(),
        components: BTreeMap::new(),
        diagnostics: vec![],
        advisory: vec![],
    };
    let mut claims: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for (id, record) in records {
        if string(record, "kind") != "binding"
            || string(record, "role") != "code"
            || string(record, "implementation") != implementation
        {
            continue;
        }
        if string(record, "spec_revision") != revision {
            result.advisory.push(json!({"code":"CODE_BINDING_HISTORICAL","binding":id,"message":"Historical binding was not rebound to the current model."}));
            continue;
        }
        let raw = to_json(record)?;
        let resource = string(record, "resource");
        let entry=result.selected.entry(id.to_owned()).or_insert_with(|| json!({"component":null,"resource":resource,"status":raw["status"],"files":[],
            "record_revision":crate::canonical_digest(&raw),"declared_revision":raw["revision"],"declared_code_revision":raw["code_revision"]}));
        let targets: BTreeSet<_> = strings(record, "targets")
            .map(|target| {
                if model
                    .nodes
                    .get(target)
                    .is_some_and(|n| string(n, "kind") == "component")
                {
                    Some(target)
                } else {
                    model.owner(target)
                }
            })
            .collect();
        let Some(component) = targets
            .first()
            .copied()
            .flatten()
            .filter(|_| targets.len() == 1 && !invalid.contains(id))
        else {
            result.diagnostics.push(diagnostic(
                "CODE_BINDING_AMBIGUOUS",
                "Binding does not resolve to one unambiguous component.",
                None,
                &[id],
            ));
            continue;
        };
        entry["component"] = component.into();
        if get(record, "selector").is_none_or(|s| string(s, "kind") != "whole-document") {
            result.diagnostics.push(diagnostic(
                "CODE_SELECTOR_UNSUPPORTED",
                "The Python profile requires whole-document bindings.",
                None,
                &[id, component],
            ));
            continue;
        }
        if let Err(e) = glob::validate(resource) {
            result.diagnostics.push(diagnostic(
                "CODE_BINDING_RESOURCE_INVALID",
                &e.to_string(),
                None,
                &[id, component],
            ));
            continue;
        }
        let exact = !resource.ends_with('/') && !resource.contains(['*', '?', '[']);
        if exact && !resource.ends_with(".py") {
            result.advisory.push(json!({"code":"CODE_RESOURCE_OUTSIDE_PROFILE","binding":id,"message":"Non-Python binding is outside this observer profile."}));
            continue;
        }
        let matches: BTreeMap<_, _> = hashes
            .iter()
            .filter(|(name, _)| glob::matches(name, resource))
            .collect();
        entry["files"] = json!(matches.keys().collect::<Vec<_>>());
        if matches.is_empty() || glob::overlaps(resource, &inventory.omitted) {
            result.diagnostics.push(diagnostic(
                "CODE_BINDING_SOURCE_MISSING",
                "Binding has missing, excluded or unavailable Python coverage.",
                None,
                &[id, component],
            ));
        }
        if !matches.is_empty() && matches!(string(record, "status"), "observed" | "checked") {
            let actual = if exact {
                format!("sha256:{}", hashes[resource])
            } else {
                crate::canonical_digest(&json!(matches))
            };
            if string(record, "revision") != actual {
                result.diagnostics.push(diagnostic("CODE_BINDING_STALE","Declared resource revision differs from current observed bytes; mapping remains explicit intent only.",None,&[id,component]));
            }
        }
        for name in matches.keys() {
            claims.entry(name).or_default().push((id, component));
        }
    }
    for (name, candidates) in claims {
        if let [(_, component)] = candidates.as_slice() {
            result.components.insert(name.into(), (*component).into());
        } else {
            result.diagnostics.push(diagnostic(
                "CODE_BINDING_AMBIGUOUS",
                "Multiple binding records claim the same Python source.",
                Some(name),
                &candidates.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            ));
        }
    }
    Ok(result)
}
