//! Construct a model sketch from explicit selections in one verified capture.

use super::load_observations;
use crate::bundle::Bundle;
use crate::report::CheckResult::{Fail, Pass};
use crate::validation::{EmbeddedSchema, Profile, validate};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;

static MAPPING: EmbeddedSchema =
    EmbeddedSchema::new(include_str!("../../schemas/graphify-mapping.schema.json"));

fn invalid(reason: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, reason.into())
}

// Both the explicit mapping and captured observation index have passed their schemas.
fn records<'a>(document: &'a Value, name: &str) -> &'a [Value] {
    document[name].as_array().expect("validated record array")
}

fn string<'a>(record: &'a Value, name: &str) -> &'a str {
    record[name].as_str().expect("validated string field")
}

fn by_id(records: &[Value]) -> HashMap<&str, &Value> {
    records
        .iter()
        .map(|record| (string(record, "id"), record))
        .collect()
}

fn provenance(source: &str, observation: &Value) -> Value {
    json!([{"source": source, "purpose": "provenance", "selector": {
        "kind": "json-pointer", "value": observation["identity"]["pointer"]
    }}])
}

/// Propose a `gdd/0.2` or `gdd/0.3` model, using the supplied locator for the
/// original bytes. The returned envelope contains an unaccepted model sketch.
/// Use `graphify.raw.json` when materializing the model beside the original.
pub fn propose_graphify(
    path: &Path,
    mapping: &Value,
    source_uri: &str,
    format: &str,
) -> io::Result<Value> {
    if !matches!(format, "gdd/0.3" | "gdd/0.2") {
        return Err(invalid("Proposal format must be gdd/0.3 or gdd/0.2."));
    }
    crate::input::check_output_depth(mapping)
        .map_err(|e| invalid(format!("Invalid explicit mapping: {e}")))?;
    MAPPING
        .validator()
        .validate(mapping)
        .map_err(|error| invalid(format!("Invalid explicit mapping: {error}")))?;
    let loaded = load_observations(path)?;
    let checks = &loaded.report.checks;
    let verified = ["graphify.integrity", "graphify.projection"]
        .iter()
        .all(|id| checks.iter().any(|c| c.check_id == *id && c.result == Pass));
    let Some(index) = loaded
        .index
        .filter(|_| loaded.raw.is_some() && verified && !checks.iter().any(|c| c.result == Fail))
    else {
        return Err(invalid(
            "Proposal requires an intact observations bundle without known import violations.",
        ));
    };
    let observed_nodes = by_id(records(&index, "nodes"));
    let observed_edges = by_id(records(&index, "edges"));
    let source_id = string(&index, "source_id");
    let mut selected = HashMap::new();
    let mut nodes = Map::new();
    for selection in records(mapping, "nodes") {
        let id = string(selection, "observation");
        let target = string(selection, "target");
        let Some(observation) = observed_nodes
            .get(id)
            .filter(|n| n["classification"] == "code_candidate")
        else {
            return Err(invalid(
                "Select a producer-claimed code observation explicitly; noncode/unknown inputs cannot stand for code.",
            ));
        };
        if nodes.contains_key(target) || selected.insert(id, target).is_some() {
            return Err(invalid(
                "Node mapping must be one-to-one within this proposal.",
            ));
        }
        let mut node = json!({
            "kind": selection["kind"],
            "description": selection["description"],
            "maturity": "sketch",
            "source_refs": provenance(source_id, observation)
        });
        if let Some(label) = observation["raw"]["label"]
            .as_str()
            .filter(|label| !label.is_empty())
        {
            node["label"] = label.into();
        }
        nodes.insert(target.into(), node);
    }
    let mut edges = Vec::new();
    let mut selected_edges = HashSet::new();
    for selection in records(mapping, "edges") {
        let id = string(selection, "observation");
        let Some(observation) = observed_edges
            .get(id)
            .filter(|e| e["direction"]["value"] == "source_to_target")
        else {
            return Err(invalid(
                "A proposed relation requires selected observation direction; ambiguous exported order cannot become intended direction.",
            ));
        };
        let from = string(selection, "from");
        let to = string(selection, "to");
        if observation["source_observation"]
            .as_str()
            .and_then(|id| selected.get(id))
            != Some(&from)
            || observation["target_observation"]
                .as_str()
                .and_then(|id| selected.get(id))
                != Some(&to)
        {
            return Err(invalid(
                "Edge mapping must preserve the selected endpoint identities and direction.",
            ));
        }
        edges.push(json!({
            "id": selection["id"], "from": from, "to": to, "relation": selection["relation"],
            "source_refs": provenance(source_id, observation)
        }));
        selected_edges.insert(id);
    }
    let revision = string(&index["artifact"], "revision");
    let targets: Vec<_> = nodes.keys().cloned().collect();
    let model = json!({
        "format": format, "project": mapping["project"], "nodes": nodes, "edges": edges,
        "sources": {(source_id): {
            "role": "observation", "title": "Original Graphify observation snapshot; producer claims",
            "uri": source_uri, "format": index["importer"]["profile"], "normative": false,
            "revision": {"sha256": revision.trim_start_matches("sha256:")}
        }},
        "questions": {"question.graphify_contracts": {
            "statement": "Clarify the intended behavior, contracts, rules and scenarios; code observations describe the existing implementation and do not automatically define intent.",
            "targets": targets, "blocks": ["implementation", "acceptance"], "status": "open"
        }}
    });
    let report = validate(&Bundle::proposal_candidate(&model)?, Profile::Model);
    if let Some(check) = report.checks.iter().find(|c| c.result != Pass) {
        let reason = if check.check_id.starts_with("representation.") {
            "Explicit mapping does not form a valid GDD representation"
        } else {
            "Explicit mapping violates GDD model semantics"
        };
        return Err(invalid(format!(
            "{reason}: {}",
            serde_json::to_string(check)?
        )));
    }
    let bindings = records(&index, "binding_candidates")
        .iter()
        .filter(|b| selected.contains_key(string(b, "observation")))
        .cloned()
        .collect();
    let omitted: Vec<_> = ["nodes", "edges", "opaque"]
        .into_iter()
        .flat_map(|name| records(&index, name))
        .map(|r| string(r, "id"))
        .filter(|id| !selected.contains_key(id) && !selected_edges.contains(id))
        .collect();
    let mut proposal = json!({
        "format": "gdd-graphify-proposal/0.2", "status": "proposed", "artifact_revision": revision,
        "mapping": mapping, "model_candidate": model, "binding_candidates": [], "omitted_observations": omitted,
        "required_actions": [
            "Review the explicit mapping and intended behavior under the project's trusted mandate.",
            "Apply through the ordinary authorized source_promotion/model_change process; no approval receipt is fabricated.",
            "Keep original observations and verify source/code revisions before registering observed or checked bindings."
        ]
    });
    // Producer selector claims may contain any JSON object, including names
    // reserved by serde_json's arbitrary-precision number serializer.
    proposal["binding_candidates"] = Value::Array(bindings);
    Ok(proposal)
}
