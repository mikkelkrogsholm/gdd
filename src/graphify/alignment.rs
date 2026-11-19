//! Compare reviewed architectural intent with one or two Graphify captures.
use super::{decode_json, load_observations, records};
use crate::projection::load_projection_bundle;
use crate::report::CheckResult::{Fail, Pass};
use crate::validation::to_json;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::{fs, io, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mapping {
    format: String,
    project: String,
    source_id: String,
    bindings: Vec<Binding>,
    #[serde(default)]
    relations: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    id: String,
    targets: Vec<String>,
    selectors: Vec<Selector>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Selector {
    ProducerId(Value),
    Symbol { file: String, label: String },
}

fn invalid(reason: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason.into())
}

type Edges = BTreeMap<(String, String, String), Vec<String>>;

struct Intent {
    project: String,
    revision: String,
    nodes: BTreeSet<String>,
    edges: Edges,
}

impl Intent {
    fn load(model: &Path) -> io::Result<Self> {
        let bundle = load_projection_bundle(model)?;
        let manifest = to_json(&bundle.manifest().unwrap().value)?;
        let mut edges = BTreeMap::new();
        for edge in bundle.edges() {
            let record = to_json(edge.value)?;
            let value = |field: &str| record[field].as_str().unwrap().to_owned();
            edges
                .entry((value("from"), value("relation"), value("to")))
                .or_insert_with(Vec::new)
                .push(edge.id.to_owned());
        }
        Ok(Self {
            project: manifest["project"]["id"].as_str().unwrap().to_owned(),
            revision: bundle.snapshot().unwrap().revision.clone(),
            nodes: bundle.nodes().map(|n| n.id.to_owned()).collect(),
            edges,
        })
    }
}

impl Mapping {
    fn load(path: &Path, intent: &Intent) -> io::Result<(Self, String)> {
        let raw = fs::read(path)?;
        let mapping: Self = serde_json::from_value(decode_json(&raw)?)?;
        if mapping.format != "gdd-code-alignment/0.1" || mapping.project != intent.project {
            return Err(invalid(
                "Expected a gdd-code-alignment/0.1 file for the selected model project.",
            ));
        }
        if !crate::valid_id(&mapping.source_id) {
            return Err(invalid("Expected a valid observation source ID."));
        }
        let mut ids = BTreeSet::new();
        for binding in &mapping.bindings {
            if !crate::valid_id(&binding.id) || !ids.insert(&binding.id) {
                return Err(invalid(format!(
                    "Invalid or repeated binding ID: {}",
                    binding.id
                )));
            }
            if binding.targets.is_empty() || binding.selectors.is_empty() {
                return Err(invalid(format!(
                    "Binding {} needs targets and selectors.",
                    binding.id
                )));
            }
            let mut targets = BTreeSet::new();
            for target in &binding.targets {
                if !intent.nodes.contains(target) || !targets.insert(target) {
                    return Err(invalid(format!(
                        "Unknown or repeated target {target} in {}.",
                        binding.id
                    )));
                }
            }
            for selector in &binding.selectors {
                let valid = match selector {
                    Selector::ProducerId(id) => records::identity(id).is_some(),
                    Selector::Symbol { file, label } => !file.is_empty() && !label.is_empty(),
                };
                if !valid {
                    return Err(invalid(format!("Invalid code selector in {}.", binding.id)));
                }
            }
        }
        if mapping
            .relations
            .iter()
            .any(|(k, v)| k.trim().is_empty() || v.trim().is_empty())
        {
            return Err(invalid("Relation names must be nonempty."));
        }
        Ok((mapping, format!("sha256:{}", crate::sha256(&raw))))
    }
}

fn verified_index(path: &Path, source: &str) -> io::Result<Value> {
    let loaded = load_observations(path)?;
    let checks = &loaded.report.checks;
    for id in [
        "graphify.integrity",
        "graphify.projection",
        "graphify.layout",
    ] {
        if !checks.iter().any(|c| c.check_id == id && c.result == Pass) {
            return Err(invalid(format!(
                "Alignment requires {id} to pass for {}.",
                path.display()
            )));
        }
    }
    if let Some(check) = checks.iter().find(|c| c.result == Fail) {
        return Err(invalid(format!("{}: {}", check.check_id, check.reason)));
    }
    let index = loaded
        .index
        .ok_or_else(|| invalid("Missing observation index."))?;
    if index["source_id"] != source {
        return Err(invalid(format!("Observation source must match {source}.")));
    }
    Ok(index)
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().expect("verified index array")
}

fn location(node: &Value) -> Value {
    json!({
        "producer_id": node["producer_id"],
        "file": node["raw"]["source_file"],
        "label": node["raw"]["label"]
    })
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
struct Dependency {
    from_bindings: BTreeSet<String>,
    relation: String,
    to_bindings: BTreeSet<String>,
}

struct Side {
    revision: Value,
    report: Value,
    // Cross-snapshot comparison uses reviewed binding IDs and code locators.
    // Snapshot-specific observation IDs and pointer order change on extraction.
    locations: BTreeMap<String, Vec<Value>>,
    dependencies: BTreeMap<Dependency, usize>,
}

impl Side {
    fn compare(index: Value, intent: &Intent, mapping: &Mapping) -> Self {
        let nodes = array(&index, "nodes");
        let mut by_id = BTreeMap::new();
        let mut by_symbol = BTreeMap::new();
        for (i, n) in nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n["classification"] == "code_candidate")
        {
            if let Some(id) = records::identity(&n["producer_id"]) {
                by_id.insert(id, i);
            }
            if let (Some(file), Some(label)) =
                (n["raw"]["source_file"].as_str(), n["raw"]["label"].as_str())
            {
                by_symbol
                    .entry((file, label))
                    .or_insert_with(Vec::new)
                    .push(i);
            }
        }

        let mut targets_by_node: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut bindings_by_node: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut ideal_bindings: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut bindings = BTreeMap::new();
        let mut locations = BTreeMap::new();
        for binding in &mapping.bindings {
            let mut matches = BTreeSet::new();
            for selector in &binding.selectors {
                match selector {
                    Selector::ProducerId(id) => {
                        matches.extend(by_id.get(&records::identity(id).unwrap()).copied())
                    }
                    Selector::Symbol { file, label } => {
                        if let Some(indices) = by_symbol.get(&(file.as_str(), label.as_str())) {
                            matches.extend(indices.iter().copied());
                        }
                    }
                }
            }
            let status = match matches.len() {
                0 => "unobserved",
                1 => {
                    let id = nodes[*matches.first().unwrap()]["id"].as_str().unwrap();
                    targets_by_node
                        .entry(id)
                        .or_default()
                        .extend(binding.targets.iter().cloned());
                    bindings_by_node
                        .entry(id)
                        .or_default()
                        .insert(binding.id.clone());
                    "matched"
                }
                _ => "ambiguous",
            };
            for target in &binding.targets {
                ideal_bindings.entry(target).or_default().push(&binding.id);
            }
            let mut locs = Vec::new();
            let mut candidates = Vec::new();
            for i in matches {
                let n = &nodes[i];
                let loc = location(n);
                candidates
                    .push(json!({"observation":n["id"], "identity":n["identity"], "location":loc}));
                locs.push(loc);
            }
            locs.sort_by_cached_key(Value::to_string);
            locations.insert(binding.id.clone(), locs);
            bindings.insert(
                binding.id.clone(),
                json!({"state":status, "targets":binding.targets, "candidates":candidates}),
            );
        }

        let mut unmapped = Vec::new();
        for n in nodes {
            let id = n["id"].as_str().unwrap();
            if !targets_by_node.contains_key(id) {
                unmapped.push(json!({"observation":id, "classification":n["classification"], "location":location(n)}));
            }
        }
        let mut observed_edges = Vec::new();
        let mut ideal_edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for ((_, relation, _), ids) in &intent.edges {
            if mapping.relations.values().any(|name| name == relation) {
                ideal_edges.extend(ids.iter().map(|id| (id.as_str(), Vec::new())));
            }
        }
        let mut dependencies = BTreeMap::new();
        for edge in array(&index, "edges") {
            let id = edge["id"].as_str().unwrap();
            let from = edge["source_observation"].as_str();
            let to = edge["target_observation"].as_str();
            let left = from.and_then(|id| targets_by_node.get(id));
            let right = to.and_then(|id| targets_by_node.get(id));
            let relation = edge["raw"]["relation"].as_str();
            let translated = relation.and_then(|r| mapping.relations.get(r));
            let directed = edge["direction"]["value"] == "source_to_target";
            if let (true, Some(l), Some(r), Some(rel)) = (
                directed,
                from.and_then(|id| bindings_by_node.get(id)),
                to.and_then(|id| bindings_by_node.get(id)),
                relation,
            ) {
                let key = Dependency {
                    from_bindings: l.clone(),
                    to_bindings: r.clone(),
                    relation: rel.to_owned(),
                };
                *dependencies.entry(key).or_default() += 1;
            }
            let mut ideal_ids = None;
            let state = match (directed, left, right, translated) {
                (false, ..) => "unknown_direction",
                (_, None, ..) | (_, _, None, _) => "unmapped_endpoint",
                (_, _, _, None) => "unmapped_relation",
                (_, Some(l), Some(r), Some(rel)) if l.len() == 1 && r.len() == 1 => {
                    let (a, b) = (l.first().unwrap(), r.first().unwrap());
                    ideal_ids = intent.edges.get(&(a.clone(), rel.clone(), b.clone()));
                    if let Some(ids) = ideal_ids {
                        if let [eid] = ids.as_slice() {
                            ideal_edges.get_mut(eid.as_str()).unwrap().push(id);
                            "observed"
                        } else {
                            "ambiguous_ideal_edges"
                        }
                    } else if a == b {
                        "internal"
                    } else {
                        "observed_only"
                    }
                }
                _ => "ambiguous_targets",
            };
            observed_edges.push(json!({
                "observation":id, "from_observation":from, "to_observation":to,
                "relation":relation, "from_targets":left, "to_targets":right,
                "producer_confidence":edge["raw"]["confidence"], "direction":edge["direction"],
                "ideal_edges":ideal_ids, "state":state
            }));
        }
        let revision = index["artifact"]["revision"].clone();
        let ideal_nodes: BTreeMap<_, _> = intent
            .nodes
            .iter()
            .map(|id| {
                let links = ideal_bindings
                    .get(id.as_str())
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let matched = links
                    .iter()
                    .filter(|&&link| bindings[link]["state"] == "matched")
                    .count();
                let state = if links.is_empty() {
                    "unmapped"
                } else if links
                    .iter()
                    .any(|&link| bindings[link]["state"] == "ambiguous")
                {
                    "ambiguous"
                } else if matched == links.len() {
                    "observed"
                } else if matched > 0 {
                    "partially_observed"
                } else {
                    "unobserved"
                };
                (id, json!({"state":state, "bindings":links}))
            })
            .collect();
        let report = json!({
            "observation_revision":revision, "producer_code_revision_claim":index["input_claims"]["code_revision"],
            "bindings":bindings, "ideal_nodes":ideal_nodes, "unmapped_observations":unmapped,
            "observed_edges":observed_edges, "comparable_ideal_edges":ideal_edges
        });
        Self {
            revision,
            report,
            locations,
            dependencies,
        }
    }

    fn changes_since(&self, previous: &Side) -> Value {
        let bindings: BTreeMap<_, _> = self
            .locations
            .iter()
            .filter_map(|(id, after)| {
                let before = &previous.locations[id];
                (before != after).then(|| (id, json!({"before":before, "after":after})))
            })
            .collect();
        let keys: BTreeSet<_> = self
            .dependencies
            .keys()
            .chain(previous.dependencies.keys())
            .collect();
        let dependencies: Vec<_> = keys
            .into_iter()
            .filter_map(|dependency| {
                let before = previous.dependencies.get(dependency).copied().unwrap_or(0);
                let after = self.dependencies.get(dependency).copied().unwrap_or(0);
                (before != after)
                    .then(|| json!({"dependency":dependency, "before":before, "after":after}))
            })
            .collect();
        json!({"previous_observation_revision":previous.revision, "bindings":bindings, "dependencies":dependencies})
    }
}

/// Evaluate a mapping against the supplied model and verified observation bundle.
pub fn align(model: &Path, observations: &Path, mapping: &Path) -> io::Result<Value> {
    align_with_previous(model, observations, mapping, None)
}

/// Also describe locator and directed dependency changes between two extracts.
pub fn align_with_previous(
    model: &Path,
    observations: &Path,
    mapping: &Path,
    previous_observations: Option<&Path>,
) -> io::Result<Value> {
    let intent = Intent::load(model)?;
    let (mapping, mapping_revision) = Mapping::load(mapping, &intent)?;
    let current = Side::compare(
        verified_index(observations, &mapping.source_id)?,
        &intent,
        &mapping,
    );
    let changes = if let Some(path) = previous_observations {
        let previous = Side::compare(verified_index(path, &mapping.source_id)?, &intent, &mapping);
        Some(current.changes_since(&previous))
    } else {
        None
    };
    Ok(
        json!({"format":"gdd-alignment-report/0.1", "project":intent.project, "source_id":mapping.source_id,
        "spec_revision":intent.revision, "mapping_revision":mapping_revision, "current":current.report, "changes":changes}),
    )
}
