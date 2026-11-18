//! Passive, self-contained native viewer projections of a validated model snapshot.

mod read;
pub use read::read_view;

use crate::boundaries::boundary_view;
use crate::bundle::DocumentKind;
use crate::context::ContextGraph;
use crate::input::check_output_depth;
use crate::perspectives::{PerspectiveBundle, load_perspective_bundle, perspective_data};
use crate::report::{Check, CheckResult};
use crate::validation::to_json;
use serde_json::{Map, Value as Json, json};
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;

pub const FORMAT: &str = "gdd-native-view/0.1";
pub const PROFILE: &str = "gdd-viewer-native/0.1";
const GENERATOR: &str = "gdd-viewer";
pub const VERSION: &str = "0.1.0";

/// Project one admitted capture. Scope limits the drawing while retaining all records.
pub fn view_data(capture: &PerspectiveBundle, scope: &[String]) -> io::Result<Json> {
    let bundle = capture.bundle();
    let manifest = bundle.manifest().expect("admitted manifest");
    let snapshot = bundle.snapshot().expect("admitted snapshot");
    let scope: BTreeSet<_> = scope.iter().map(String::as_str).collect();
    let nodes: BTreeSet<_> = bundle.nodes().map(|n| n.id).collect();
    if !scope.is_subset(&nodes) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unknown view scope.",
        ));
    }
    let shown = if scope.is_empty() {
        nodes.clone()
    } else {
        ContextGraph::new(bundle)
            .scope(scope.iter().copied())
            .nodes()
            .collect()
    };
    let mut model = to_json(&manifest.value)?;
    let mut locations = Map::new();
    let mut locate = |id: &str| {
        let origin = bundle.location(id).expect("indexed model record");
        locations.insert(
            id.into(),
            json!([bundle.declared_fragment_path(&origin.file), origin.pointer]),
        );
    };
    for registry in ["sources", "questions", "extensions"] {
        if let Some(entries) = model[registry].as_object() {
            entries.keys().for_each(|id| locate(id));
        }
    }
    let mut records = Map::new();
    for node in bundle.nodes() {
        locate(node.id);
        records.insert(node.id.to_owned(), to_json(node.value)?);
    }
    model["nodes"] = Json::Object(records);
    let mut shown_edges = BTreeSet::new();
    let mut omitted_edges = BTreeSet::new();
    let mut edges = Vec::new();
    for edge in bundle.edges() {
        locate(edge.id);
        let record = to_json(edge.value)?;
        if ["from", "to"]
            .iter()
            .all(|field| shown.contains(record[field].as_str().expect("admitted edge endpoint")))
        {
            shown_edges.insert(edge.id);
        } else {
            omitted_edges.insert(edge.id);
        }
        edges.push(record);
    }
    model["edges"] = Json::Array(edges);
    let mut data = json!({
        "format": FORMAT,
        "generator": {"name": GENERATOR, "version": VERSION, "profile": PROFILE},
        "spec_revision": snapshot.revision,
        "snapshot": snapshot,
        "scope": scope,
        "manifest": {
            "shown_nodes": shown, "omitted_nodes": nodes.difference(&shown).collect::<Vec<_>>(),
            "shown_edges": shown_edges, "omitted_edges": omitted_edges
        },
        "observation_inputs": [],
    });
    data["locations"] = Json::Object(locations);
    data["model"] = model;
    data["perspectives"] = perspective_data(capture)?;
    if manifest.kind == DocumentKind::Manifest03 {
        let selected: Vec<_> = shown.into_iter().map(str::to_owned).collect();
        data["boundaries"] = boundary_view(bundle, Some(&selected))?;
    }
    check_output_depth(&data).map_err(io::Error::other)?;
    Ok(data)
}

/// Publish a new artifact atomically and return its scoped generation report.
pub fn render_view(model: &Path, output: &Path, scope: &[String]) -> io::Result<Json> {
    let capture = load_perspective_bundle(model)?;
    let payload = view_data(&capture, scope)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut builder = tempfile::Builder::new();
    builder.prefix(".gdd-view-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o666));
    }
    let mut staging = builder.tempfile_in(parent)?;
    {
        let mut writer = io::BufWriter::new(staging.as_file_mut());
        serde_json::to_writer(&mut writer, &payload)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    staging.persist_noclobber(output)?;
    let mut check = Check::new(
        "integration.visualization",
        "integration",
        CheckResult::Pass,
        format!("Generated a read-only {PROFILE} projection."),
    );
    check.scope = serde_json::from_value(payload["scope"].clone())?;
    Ok(json!({
        "format": "gdd-report/0.2",
        "tool": {"name": GENERATOR, "version": VERSION},
        "spec_revision": payload["spec_revision"], "scope": check.scope,
        "profiles": ["visualization"], "checks": [check]
    }))
}
