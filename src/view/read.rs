//! Offline structural admission of a saved projection. All locators are passive strings.

use super::{FORMAT, GENERATOR, PROFILE};
use crate::bundle::{DocumentKind, read_regular};
use crate::input::parse_json;
use crate::validation::{EmbeddedSchema, embedded_schema, to_json};
use serde_json::Value as Json;
use std::{collections::BTreeSet, io, path::Path};

/// Capture a local regular file once. The returned bytes can be opened or
/// verified repeatedly without resolving any paths inside the artifact.
pub fn read_view_bytes(path: &Path) -> io::Result<Vec<u8>> {
    let resolved = path.canonicalize()?;
    let (Some(parent), Some(name)) = (resolved.parent(), resolved.file_name()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected a view file.",
        ));
    };
    read_regular(parent, Path::new(name))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn ids<'a>(value: &'a Json, field: &str) -> io::Result<BTreeSet<&'a str>> {
    let array = value[field]
        .as_array()
        .ok_or_else(|| invalid(&format!("Expected an ID list in {field}.")))?;
    let mut ids = BTreeSet::new();
    for item in array {
        let id = item
            .as_str()
            .ok_or_else(|| invalid("Expected a string ID."))?;
        if !ids.insert(id) {
            return Err(invalid(&format!("Duplicate ID in {field}.")));
        }
    }
    Ok(ids)
}

fn partition<'a>(
    manifest: &'a Json,
    all: &BTreeSet<&str>,
    shown: &str,
    omitted: &str,
) -> io::Result<BTreeSet<&'a str>> {
    let included = ids(manifest, shown)?;
    let excluded = ids(manifest, omitted)?;
    if !included.is_disjoint(&excluded)
        || !included.union(&excluded).copied().eq(all.iter().copied())
    {
        return Err(invalid(&format!("Invalid {shown}/{omitted} partition.")));
    }
    Ok(included)
}

/// Read one UTF-8 JSON artifact from memory. Validate its embedded data and
/// graph identities without resolving any included path or opening another file.
pub fn read_view(bytes: &[u8]) -> io::Result<Json> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("Expected UTF-8 JSON."))?;
    let input = parse_json(text).map_err(io::Error::other)?;
    let data = to_json(&input)?;
    drop(input);
    let generator = &data["generator"];
    if data["format"] != FORMAT
        || generator["profile"] != PROFILE
        || generator["name"] != GENERATOR
        || generator["version"].as_str().is_none_or(str::is_empty)
    {
        return Err(invalid("Unsupported native view format or generator."));
    }
    if !data["observation_inputs"]
        .as_array()
        .is_some_and(Vec::is_empty)
    {
        return Err(invalid("Expected empty observation_inputs."));
    }
    let model = &data["model"];
    let kind = match model["format"].as_str() {
        Some("gdd/0.2") => DocumentKind::Manifest02,
        Some("gdd/0.3") => DocumentKind::Manifest03,
        _ => return Err(invalid("Unsupported embedded model format.")),
    };
    static PERSPECTIVES: EmbeddedSchema =
        EmbeddedSchema::new(include_str!("../../schemas/perspectives.schema.json"));
    static BOUNDARIES: EmbeddedSchema =
        EmbeddedSchema::new(include_str!("../../schemas/boundaries.schema.json"));
    let snapshot = &data["snapshot"];
    for (section, validator) in [
        ("model", embedded_schema(kind).unwrap()),
        ("snapshot", embedded_schema(DocumentKind::Snapshot).unwrap()),
        ("perspectives", PERSPECTIVES.validator()),
    ] {
        validator
            .validate(&data[section])
            .map_err(|e| invalid(&format!("Invalid {section}: {e}")))?;
    }
    let revision = &snapshot["revision"];
    let assets: Vec<_> = snapshot["assets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["path"].as_str().unwrap(), a["sha256"].as_str().unwrap()))
        .collect();
    if *revision != crate::snapshot_revision(&assets)?
        || *revision != data["spec_revision"]
        || *revision != data["perspectives"]["spec_revision"]
    {
        return Err(invalid("Inconsistent native view snapshot revision."));
    }
    if kind == DocumentKind::Manifest03 {
        BOUNDARIES
            .validator()
            .validate(&data["boundaries"])
            .map_err(|e| invalid(&format!("Invalid boundaries: {e}")))?;
        if data["boundaries"]["spec_revision"] != *revision {
            return Err(invalid("Inconsistent boundary revision."));
        }
    } else if data.get("boundaries").is_some() {
        return Err(invalid("Unexpected boundaries for GDD 0.2."));
    }

    let nodes: BTreeSet<_> = model["nodes"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let manifest = &data["manifest"];
    let shown = partition(manifest, &nodes, "shown_nodes", "omitted_nodes")?;
    let scope = ids(&data, "scope")?;
    if !scope.is_subset(&shown) || (scope.is_empty() && shown != nodes) {
        return Err(invalid("Scope is inconsistent with shown_nodes."));
    }
    let mut records = nodes.clone();
    for name in ["sources", "questions", "extensions"] {
        if let Some(entries) = model[name].as_object() {
            for id in entries.keys() {
                if !records.insert(id) {
                    return Err(invalid("Duplicate model ID."));
                }
            }
        }
    }
    let mut edges = BTreeSet::new();
    let mut eligible = BTreeSet::new();
    for edge in model["edges"].as_array().unwrap() {
        let id = edge["id"].as_str().unwrap();
        let from = edge["from"].as_str().unwrap();
        let to = edge["to"].as_str().unwrap();
        if !records.insert(id) || !nodes.contains(from) || !nodes.contains(to) {
            return Err(invalid(&format!(
                "Duplicate edge ID or unknown endpoint in {id}."
            )));
        }
        edges.insert(id);
        if shown.contains(from) && shown.contains(to) {
            eligible.insert(id);
        }
    }
    if partition(manifest, &edges, "shown_edges", "omitted_edges")? != eligible {
        return Err(invalid("Shown edges do not match shown endpoints."));
    }
    if kind == DocumentKind::Manifest03 && ids(&data["boundaries"], "scope")? != shown {
        return Err(invalid("Boundary scope differs from shown_nodes."));
    }
    let locations = data["locations"]
        .as_object()
        .ok_or_else(|| invalid("Expected record locations."))?;
    if locations.len() != records.len() || !records.iter().all(|id| {
        matches!(locations.get(*id).and_then(Json::as_array).map(Vec::as_slice), Some([file, pointer]) if file.as_str().is_some_and(|s| !s.is_empty()) && pointer.as_str().is_some_and(|s| s.starts_with('/')))
    }) {
        return Err(invalid("Expected a source file and JSON pointer for every model record."));
    }
    Ok(data)
}
