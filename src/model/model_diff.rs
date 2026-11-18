//! Compare declared model identities, expanded metadata and packaging locations.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value as Json, json};

use super::{entries, escape, string};
use crate::bundle::{self, Bundle};
use crate::input::Value;
use crate::report::{Check, CheckResult, Diagnostic};
use crate::validation::to_json;

const REGISTRIES: [&str; 4] = ["nodes", "sources", "questions", "edges"];

#[derive(Debug, Clone, Serialize)]
pub struct DiffSide {
    pub path: String,
    pub spec_revision: Option<String>,
    pub format: Option<&'static str>,
    pub coverage: &'static str,
    pub checks: Vec<Check>,
}
#[derive(Debug, Serialize)]
pub struct FieldChange {
    pub pointer: String,
    pub change: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Json>,
}
#[derive(Debug, Serialize)]
pub struct Change {
    pub registry: &'static str,
    pub id: String,
    pub change: &'static str,
    pub fields: Vec<FieldChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Json>,
}
#[derive(Debug, Serialize)]
pub struct Relocation {
    pub registry: &'static str,
    pub id: String,
    pub base: Option<[String; 2]>,
    pub head: Option<[String; 2]>,
}
#[derive(Debug, Serialize)]
pub struct Advisory {
    pub code: &'static str,
    pub id: String,
    pub pointer: String,
    pub message: &'static str,
    pub before: Json,
    pub after: Json,
}
#[derive(Debug, Serialize)]
pub struct Comparison {
    pub coverage: &'static str,
    pub semantic_changes: bool,
    pub counts: Json,
}
#[derive(Debug, Serialize)]
pub struct SideDiagnostic {
    pub side: &'static str,
    pub check_id: String,
    pub result: CheckResult,
    pub reason: String,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Debug, Serialize)]
pub struct ModelDiff {
    pub format: &'static str,
    pub base: DiffSide,
    pub head: DiffSide,
    pub comparison: Comparison,
    pub changes: Vec<Change>,
    pub metadata_changes: Vec<FieldChange>,
    pub relocated: Vec<Relocation>,
    pub advisories: Vec<Advisory>,
    pub diagnostics: Vec<SideDiagnostic>,
    pub exit_code: u8,
}

#[derive(Default)]
struct Registry {
    values: BTreeMap<String, Json>,
    locations: BTreeMap<String, [String; 2]>,
}

struct Side {
    report: DiffSide,
    manifest: Option<String>,
    registries: BTreeMap<&'static str, Registry>,
    metadata: Json,
}

fn normalized(value: &Value) -> io::Result<Json> {
    fn normalize(value: &mut Json) {
        match value {
            Json::Object(map) => map.values_mut().for_each(normalize),
            Json::Array(list) => list.iter_mut().for_each(normalize),
            Json::Number(n) => {
                let canonical = super::canonical::comparison_number(n.as_str());
                if canonical != n.as_str() {
                    *n = serde_json::Number::from_string_unchecked(canonical);
                }
            }
            _ => {}
        }
    }
    let mut json = to_json(value)?;
    normalize(&mut json);
    Ok(json)
}

impl Side {
    fn capture(bundle: &Bundle) -> io::Result<Self> {
        let inspected = super::inspection::inspect(bundle);
        let root = inspected.root;
        let format = root.map(|doc| doc.kind.format());
        let mut side = Self {
            report: DiffSide {
                path: bundle.path().display().to_string(),
                spec_revision: inspected.revision,
                format,
                coverage: "unavailable",
                checks: inspected.checks,
            },
            manifest: None,
            registries: REGISTRIES
                .into_iter()
                .map(|name| (name, Registry::default()))
                .collect(),
            metadata: Json::Null,
        };
        let Some(root) = root.filter(|doc| doc.kind.is_manifest()) else {
            side.report.checks.push(Check::new(
                "diff.manifest",
                "model",
                CheckResult::Unknown,
                "A supported GDD model manifest is unavailable.",
            ));
            return Ok(side);
        };
        let checks = &mut side.report.checks;
        let accepted = bundle.accepted_graph(&inspected.rejected);
        let graph = accepted.as_ref().unwrap_or(bundle);
        if accepted.is_some() {
            // A rejected fragment can share an ID with a later, usable node.
            // Only collisions among admitted fragments make that ID ambiguous.
            checks.retain(|check| {
                !check
                    .diagnostics
                    .iter()
                    .any(|d| d.code == "ID_COLLISION" && d.location.pointer.starts_with("/nodes/"))
            });
            checks.extend(
                graph
                    .diagnostics()
                    .iter()
                    .filter(|d| !d.semantic_only && d.code == "ID_COLLISION")
                    .map(Check::from),
            );
        }
        let definitions: BTreeMap<_, _> = graph.extensions().collect();
        let mut extensions = json!({});
        let mut unsupported = Vec::new();
        for (id, decl) in entries(&root.value, "extensions") {
            let namespace = string(decl, "namespace");
            let definition = definitions.get(namespace);
            extensions[id] = json!({"namespace":namespace,"version":string(decl,"version"),
                "definition":definition.map(|doc|normalized(&doc.value)).transpose()?});
            if definition.is_none() {
                unsupported.push(namespace.to_owned());
            }
        }
        let rep_complete = checks.iter().all(|c| c.result == CheckResult::Pass);
        if accepted.is_some() {
            // Extension declaration conflicts belong to the captured manifest.
            checks.extend(
                bundle
                    .diagnostics()
                    .iter()
                    .filter(|d| d.semantic_only && d.code != "ID_COLLISION")
                    .map(Check::from),
            );
        }
        let model = super::semantics(graph, root, &unsupported);
        checks.extend(model.checks);
        for check in checks
            .iter_mut()
            .filter(|c| c.check_id == "model.semantics" && c.result == CheckResult::Pass)
        {
            check.reason = concat!(
                "Core references, endpoints, identity, refinement, machines, types ",
                "and declared extension data checked."
            )
            .into();
        }
        let collisions: BTreeSet<_> = checks
            .iter()
            .flat_map(|c| &c.diagnostics)
            .filter(|d| d.code == "ID_COLLISION")
            .flat_map(|d| d.related_ids.iter().cloned())
            .collect();
        let complete = rep_complete
            && !checks.iter().any(|c| {
                c.result == CheckResult::Unknown
                    || c.diagnostics.iter().any(|d| d.code == "ID_COLLISION")
            });
        side.report.coverage = if complete { "complete" } else { "partial" };
        side.manifest = Some(root.asset.path.clone());
        for name in REGISTRIES {
            let registry = side.registries.get_mut(name).unwrap();
            let mut insert = |id: &str, value: &Value| -> io::Result<()> {
                if !collisions.contains(id) {
                    registry.values.insert(id.into(), normalized(value)?);
                    if let Some(location) = graph.location(id) {
                        registry.locations.insert(
                            id.into(),
                            [
                                graph.declared_fragment_path(&location.file).into(),
                                location.pointer.clone(),
                            ],
                        );
                    }
                }
                Ok(())
            };
            match name {
                "nodes" => {
                    for node in graph.nodes() {
                        insert(node.id, node.value)?;
                    }
                }
                "edges" => {
                    for edge in graph.edges() {
                        insert(edge.id, edge.value)?;
                    }
                }
                _ => {
                    for (id, value) in entries(&root.value, name) {
                        insert(id, value)?;
                    }
                }
            }
        }
        if complete {
            let mut metadata = json!({});
            if let Value::Object(fields) = &root.value {
                for (key, value) in fields {
                    if !matches!(
                        key.as_str(),
                        "nodes"
                            | "edges"
                            | "sources"
                            | "questions"
                            | "includes"
                            | "record_files"
                            | "extensions"
                    ) {
                        metadata[key] = normalized(value)?;
                    }
                }
            }
            metadata["extensions"] = extensions;
            side.metadata = metadata;
        }
        Ok(side)
    }
}

fn fields(before: &Json, after: &Json, path: &str, changes: &mut Vec<FieldChange>) {
    if let (Some(left), Some(right)) = (before.as_object(), after.as_object()) {
        for key in left.keys().chain(right.keys()).collect::<BTreeSet<_>>() {
            let pointer = format!("{path}/{}", escape(key));
            match (left.get(key), right.get(key)) {
                (Some(a), Some(b)) => fields(a, b, &pointer, changes),
                (a, b) => changes.push(FieldChange {
                    pointer,
                    change: if a.is_none() { "added" } else { "removed" },
                    before: a.cloned(),
                    after: b.cloned(),
                }),
            }
        }
    } else if before != after {
        changes.push(FieldChange {
            pointer: path.into(),
            change: "changed",
            before: Some(before.clone()),
            after: Some(after.clone()),
        });
    }
}

/// Compare two model paths. Aliases of the same resolved manifest use one capture.
pub fn diff_models(base: &Path, head: &Path) -> io::Result<ModelDiff> {
    let old = bundle::load_for_inspection(base);
    let old_side = Side::capture(&old)?;
    let same = base == head || std::fs::canonicalize(head).is_ok_and(|p| p == old.path());
    if same {
        Ok(compare(&old_side, &old_side, base, head))
    } else {
        let new = Side::capture(&bundle::load_for_inspection(head))?;
        Ok(compare(&old_side, &new, base, head))
    }
}

/// Compare independent immutable captures, using the stored model paths as labels.
pub fn diff_models_from_bundles(base: &Bundle, head: &Bundle) -> io::Result<ModelDiff> {
    let old = Side::capture(base)?;
    if std::ptr::eq(base, head) {
        return Ok(compare(&old, &old, base.path(), head.path()));
    }
    Ok(compare(
        &old,
        &Side::capture(head)?,
        base.path(),
        head.path(),
    ))
}

fn compare(base: &Side, head: &Side, base_path: &Path, head_path: &Path) -> ModelDiff {
    let complete = base.report.coverage == "complete" && head.report.coverage == "complete";
    let coverage = if complete {
        "complete"
    } else if base.report.coverage == "unavailable" || head.report.coverage == "unavailable" {
        "unavailable"
    } else {
        "partial"
    };
    let mut counts = json!({});
    let mut changes = Vec::new();
    let mut relocated = Vec::new();
    for registry in REGISTRIES {
        let old = &base.registries[registry];
        let new = &head.registries[registry];
        let (mut added, mut removed, mut changed) = (0, 0, 0);
        for id in old
            .values
            .keys()
            .chain(new.values.keys())
            .collect::<BTreeSet<_>>()
        {
            let (before, after) = (old.values.get(id), new.values.get(id));
            let change = match (before, after) {
                (Some(_), Some(_)) => "changed",
                _ if !complete => continue,
                (None, _) => "added",
                _ => "removed",
            };
            let mut delta = Vec::new();
            fields(
                before.unwrap_or(&Json::Null),
                after.unwrap_or(&Json::Null),
                "",
                &mut delta,
            );
            if !delta.is_empty() {
                match change {
                    "added" => added += 1,
                    "removed" => removed += 1,
                    _ => changed += 1,
                };
                changes.push(Change {
                    registry,
                    id: id.clone(),
                    change,
                    fields: delta,
                    before: before.cloned(),
                    after: after.cloned(),
                });
            }
            if before.is_some() && after.is_some() {
                fn container<'a>(
                    side: &Side,
                    location: Option<&'a [String; 2]>,
                ) -> Option<&'a str> {
                    location.map(|[file, _]| {
                        if Some(file) == side.manifest.as_ref() {
                            "manifest"
                        } else {
                            file
                        }
                    })
                }
                let a = old.locations.get(id);
                let b = new.locations.get(id);
                if container(base, a) != container(head, b) {
                    relocated.push(Relocation {
                        registry,
                        id: id.clone(),
                        base: a.cloned(),
                        head: b.cloned(),
                    });
                }
            }
        }
        counts[registry] = json!({"added":added,"removed":removed,"changed":changed});
    }
    let mut metadata_changes = Vec::new();
    if complete {
        fields(&base.metadata, &head.metadata, "", &mut metadata_changes);
    }
    let mut advisories = Vec::new();
    for change in &changes {
        advisories_for(change, &mut advisories);
    }
    if complete {
        let verified = |side: &Side| -> BTreeSet<String> {
            side.registries["edges"]
                .values
                .values()
                .filter(|e| e["relation"] == "verifies")
                .filter_map(|e| e["to"].as_str().map(str::to_owned))
                .collect()
        };
        for id in verified(base).difference(&verified(head)) {
            advisories.push(Advisory {
                code: "LAST_VERIFIER_REMOVED",
                id: id.clone(),
                pointer: "/edges".into(),
                message: "The rule lost its last declared verifying scenario edge.",
                before: json!(true),
                after: json!(false),
            });
        }
    }
    advisories.sort_by(|a, b| (&a.id, a.code, &a.pointer).cmp(&(&b.id, b.code, &b.pointer)));
    let diagnostics: Vec<_> = [("base", base), ("head", head)]
        .into_iter()
        .flat_map(|(name, side)| {
            side.report
                .checks
                .iter()
                .filter(|c| c.result != CheckResult::Pass)
                .map(move |c| SideDiagnostic {
                    side: name,
                    check_id: c.check_id.clone(),
                    result: c.result,
                    reason: c.reason.clone(),
                    diagnostics: c.diagnostics.clone(),
                })
        })
        .collect();
    let exit_code = if diagnostics.iter().any(|d| d.result == CheckResult::Fail) {
        1
    } else if complete {
        0
    } else {
        2
    };
    let mut base = base.report.clone();
    let mut head = head.report.clone();
    base.path = base_path.display().to_string();
    head.path = head_path.display().to_string();
    ModelDiff {
        format: "gdd-model-diff/0.1",
        base,
        head,
        comparison: Comparison {
            coverage,
            semantic_changes: !changes.is_empty() || !metadata_changes.is_empty(),
            counts,
        },
        changes,
        metadata_changes,
        relocated,
        advisories,
        diagnostics,
        exit_code,
    }
}

fn advisories_for(change: &Change, out: &mut Vec<Advisory>) {
    let old = change.before.as_ref().unwrap_or(&Json::Null);
    let new = change.after.as_ref().unwrap_or(&Json::Null);
    let mut add = |code, pointer: &str, message, before: &Json, after: &Json| {
        out.push(Advisory {
            code,
            id: change.id.clone(),
            pointer: pointer.into(),
            message,
            before: before.clone(),
            after: after.clone(),
        });
    };
    if change.registry == "sources" && change.change == "changed" && old["role"] != new["role"] {
        add(
            "IDENTITY_REVIEW",
            "/role",
            "A source identity's role changed. Two snapshots cannot establish historical ID reuse.",
            &old["role"],
            &new["role"],
        );
    }
    if change.registry != "nodes" || change.change == "added" {
        return;
    }
    let previous = &old["rule"];
    let updated = &new["rule"];
    if previous["blocking"] == true && updated["blocking"] != true {
        add(
            "BLOCKING_RULE_RELAXED",
            "/rule/blocking",
            "A blocking rule was removed or is no longer explicitly blocking.",
            &previous["blocking"],
            &updated["blocking"],
        );
    }
    let a = &previous["constraint"];
    let b = &updated["constraint"];
    if a["policy"] == "forbid" {
        if b["policy"] != "forbid" {
            add(
                "FORBID_POLICY_RELAXED",
                "/rule/constraint/policy",
                "A forbid constraint was removed or replaced by another policy.",
                &a["policy"],
                &b["policy"],
            );
        }
        if b.as_object().is_some_and(|v| !v.is_empty()) {
            if a["reach"] == "transitive" && b["reach"] == "direct" {
                add(
                    "FORBID_REACH_NARROWED",
                    "/rule/constraint/reach",
                    "A forbid constraint now checks direct paths only.",
                    &a["reach"],
                    &b["reach"],
                );
            }
            for key in ["from", "to"] {
                if a[key] != b[key] {
                    add(
                        "FORBID_ENDPOINT_CHANGED",
                        &format!("/rule/constraint/{key}"),
                        "A forbid endpoint changed; review whether the restriction narrowed.",
                        &a[key],
                        &b[key],
                    );
                }
            }
        }
    }
    if change.change != "changed" {
        return;
    }
    for key in ["kind", "role"] {
        if old[key] != new[key] {
            add(
                "IDENTITY_REVIEW",
                &format!("/{key}"),
                "An identity's kind or role changed. Two snapshots cannot establish historical ID reuse.",
                &old[key],
                &new[key],
            );
        }
    }
    for key in ["errors", "preconditions", "postconditions"] {
        let empty = if key == "errors" {
            json!({})
        } else {
            json!([])
        };
        let previous = old["contract"].get(key).unwrap_or(&empty);
        let current = new["contract"].get(key).unwrap_or(&empty);
        let removed: Vec<Json> =
            if let (Some(a), Some(b)) = (previous.as_object(), current.as_object()) {
                a.keys()
                    .filter(|key| !b.contains_key(*key))
                    .map(|s| json!(s))
                    .collect()
            } else if let (Some(a), Some(b)) = (previous.as_array(), current.as_array()) {
                a.iter().filter(|v| !b.contains(v)).cloned().collect()
            } else {
                Vec::new()
            };
        if !removed.is_empty() {
            add(
                "CONTRACT_REQUIREMENT_REMOVED",
                &format!("/contract/{key}"),
                "Contract entries disappeared or were replaced; review exact text without assuming semantic weakening.",
                &Json::Array(removed),
                current,
            );
        }
    }
    let empty = json!([]);
    let previous = old.get("interfaces").unwrap_or(&empty);
    let current = new.get("interfaces").unwrap_or(&empty);
    let mut removed: Vec<_> = previous
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Json::as_str)
        .filter(|id| {
            !current
                .as_array()
                .is_some_and(|ids| ids.iter().any(|v| v.as_str() == Some(id)))
        })
        .collect();
    removed.sort_unstable();
    if !removed.is_empty() {
        add(
            "INTERFACE_SHRUNK",
            "/interfaces",
            "Previously declared interface actions are absent.",
            &json!(removed),
            current,
        );
    }
}

// The text projection uses JSON's spaced compact form with sorted object keys.
fn display_json(value: &Json) -> String {
    match value {
        Json::Object(map) => {
            let fields: BTreeMap<_, _> = map.iter().collect();
            format!(
                "{{{}}}",
                fields
                    .into_iter()
                    .map(|(key, v)| format!("{}: {}", json!(key), display_json(v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Json::Array(list) => format!(
            "[{}]",
            list.iter().map(display_json).collect::<Vec<_>>().join(", ")
        ),
        _ => value.to_string(),
    }
}

/// Render an existing comparison without accessing either model.
pub fn diff_text(result: &ModelDiff) -> String {
    let mut out = format!(
        "GDD model diff — {}\nBase {} ({}) → Head {} ({})\n",
        result.comparison.coverage,
        result.base.spec_revision.as_deref().unwrap_or("None"),
        result.base.format.unwrap_or("None"),
        result.head.spec_revision.as_deref().unwrap_or("None"),
        result.head.format.unwrap_or("None")
    );
    if result.comparison.coverage != "complete" {
        out.push_str(
            "UNKNOWN comparison coverage: absence claims and metadata comparison are suppressed.\n",
        );
    }
    for name in REGISTRIES {
        let count = &result.comparison.counts[name];
        writeln!(
            out,
            "{name}: +{} added -{} removed ~{} changed",
            count["added"], count["removed"], count["changed"]
        )
        .unwrap();
    }
    for item in &result.changes {
        writeln!(out, "{} {} {}", item.change, item.registry, item.id).unwrap();
        for field in &item.fields {
            writeln!(
                out,
                "  {} {}: {} → {}",
                if field.pointer.is_empty() {
                    "/"
                } else {
                    &field.pointer
                },
                field.change,
                display_json(field.before.as_ref().unwrap_or(&Json::Null)),
                display_json(field.after.as_ref().unwrap_or(&Json::Null))
            )
            .unwrap();
        }
    }
    for field in &result.metadata_changes {
        writeln!(out, "metadata {}", display_json(&json!(field))).unwrap();
    }
    for item in &result.relocated {
        let location = |v: &Option<[String; 2]>| {
            v.as_ref()
                .map(|parts| super::planning::list(parts.iter().map(String::as_str)))
                .unwrap_or_else(|| "None".into())
        };
        writeln!(
            out,
            "relocated {} {}: {} → {}",
            item.registry,
            item.id,
            location(&item.base),
            location(&item.head)
        )
        .unwrap();
    }
    for a in &result.advisories {
        writeln!(
            out,
            "Advisory {} {}{}: {}",
            a.code, a.id, a.pointer, a.message
        )
        .unwrap();
    }
    for d in &result.diagnostics {
        writeln!(out, "{} {} {}: {}", d.side, d.result, d.check_id, d.reason).unwrap();
    }
    out.push_str("Structural advisories do not prove prose weakening or historical ID reuse.\n");
    out
}
