//! Load model documents and compute a snapshot from the bytes read.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::input::{self, InputError, Value};
use crate::report::{Check, CheckResult};
use crate::{sha256, snapshot_revision};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentKind {
    Manifest02,
    Manifest03,
    Fragment02,
    Fragment03,
    Extension,
    Records,
    Report,
    Snapshot,
    Other,
}

impl DocumentKind {
    fn from_value(value: &Value) -> Self {
        match get_string(value, "format") {
            Some("gdd/0.2") => Self::Manifest02,
            Some("gdd/0.3") => Self::Manifest03,
            Some("gdd-fragment/0.2") => Self::Fragment02,
            Some("gdd-fragment/0.3") => Self::Fragment03,
            Some("gdd-extension/0.2") => Self::Extension,
            Some("gdd-records/0.2") => Self::Records,
            Some("gdd-report/0.2") => Self::Report,
            Some("gdd-snapshot/0.2") => Self::Snapshot,
            _ => Self::Other,
        }
    }

    pub fn format(self) -> &'static str {
        match self {
            Self::Manifest02 => "gdd/0.2",
            Self::Manifest03 => "gdd/0.3",
            Self::Fragment02 => "gdd-fragment/0.2",
            Self::Fragment03 => "gdd-fragment/0.3",
            Self::Extension => "gdd-extension/0.2",
            Self::Records => "gdd-records/0.2",
            Self::Report => "gdd-report/0.2",
            Self::Snapshot => "gdd-snapshot/0.2",
            Self::Other => "unknown",
        }
    }

    pub fn is_manifest(self) -> bool {
        matches!(self, Self::Manifest02 | Self::Manifest03)
    }
}

#[derive(Clone, Debug)]
pub struct Asset {
    /// Resolved path relative to the bundle root, with POSIX separators.
    pub path: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Document {
    pub kind: DocumentKind,
    pub asset: Asset,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Outcome {
    Fail,
    Unknown,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Location {
    pub file: String,
    pub pointer: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum FormatIssue {
    Missing,
    Expected(DocumentKind),
}

#[derive(Debug)]
pub struct Diagnostic {
    pub code: &'static str,
    pub outcome: Outcome,
    pub dimension: &'static str,
    pub message: Box<str>,
    pub location: Location,
    pub related_ids: Box<[String]>,
    pub(crate) semantic_only: bool,
    pub(crate) format_issue: Option<FormatIssue>,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}{}: {}: {}",
            self.location.file, self.location.pointer, self.code, self.message
        )
    }
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub format: &'static str,
    pub revision: String,
    pub assets: Vec<SnapshotAsset>,
}

#[derive(Debug, Serialize)]
pub struct SnapshotAsset {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug)]
pub struct LocatedRecord<'a> {
    pub id: &'a str,
    pub value: &'a Value,
    pub location: &'a Location,
}

#[derive(Debug, Default)]
pub struct Bundle {
    path: PathBuf,
    documents: Vec<Document>,
    secondary_start: Option<usize>,
    declared_record_paths: BTreeSet<PathBuf>,
    // A paired capture validates each contributing root before it can supply
    // a project identity or record declarations. Reuse these checks in reports.
    representation_checks: BTreeMap<usize, Check>,
    normative_sources: BTreeMap<String, Asset>,
    fragment_declarations: BTreeMap<String, String>,
    advisory_paths: BTreeMap<String, Result<String, String>>,
    advisory_assets: BTreeMap<String, Asset>,
    extensions: BTreeMap<String, usize>,
    // Keep the identity of each unreadable declaration alongside the load
    // diagnostic. Impact reports distinguish missing semantics from pin errors.
    pub(crate) unread_extensions: BTreeSet<String>,
    // A failed pin excludes these bytes from the model registry and snapshot.
    // A partial comparison may still inspect a schema-accepted observation.
    pub(crate) unpinned_extensions: BTreeMap<String, Result<Document, Diagnostic>>,
    locations: BTreeMap<String, Location>,
    nodes: BTreeMap<String, usize>,
    edges: Vec<(String, usize, usize)>,
    colliding_edges: Vec<(String, usize, usize, Location)>,
    records: BTreeMap<String, (usize, Location)>,
    diagnostics: Vec<Diagnostic>,
    snapshot: Option<Snapshot>,
}

impl Bundle {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root_document(&self) -> Option<&Document> {
        self.primary_documents().first()
    }

    pub fn manifest(&self) -> Option<&Document> {
        self.root_document().filter(|doc| doc.kind.is_manifest())
    }

    /// Documents read from the primary input, its fragments and declared assets.
    pub fn primary_documents(&self) -> &[Document] {
        &self.documents[..self.secondary_start.unwrap_or(self.documents.len())]
    }

    /// All captured documents, including an explicitly supplied secondary input.
    pub fn documents(&self) -> &[Document] {
        &self.documents
    }

    pub(crate) fn representation_check(&self, index: usize) -> Option<Check> {
        self.representation_checks
            .get(&index)
            .cloned()
            .or_else(|| crate::validation::representation_check(&self.documents[index]))
    }

    fn validate_document(&mut self, index: usize) -> bool {
        let Some(check) = crate::validation::representation_check(&self.documents[index]) else {
            return false;
        };
        let valid = check.result == CheckResult::Pass;
        self.representation_checks.insert(index, check);
        valid
    }

    fn accepted_root(&self) -> Option<&Document> {
        self.representation_checks
            .get(&0)
            .filter(|c| c.result == CheckResult::Pass)
            .and_then(|_| self.root_document())
    }

    // Reindex the accepted graph when a partial capture contains rejected
    // fragments or extension definitions. This uses only captured documents.
    pub(crate) fn accepted_graph(&self, rejected: &BTreeSet<String>) -> Option<Self> {
        let graph_document = |d: &Document| {
            matches!(
                d.kind,
                DocumentKind::Manifest02
                    | DocumentKind::Manifest03
                    | DocumentKind::Extension
                    | DocumentKind::Fragment02
                    | DocumentKind::Fragment03
            )
        };
        if self.unpinned_extensions.is_empty()
            && !self
                .primary_documents()
                .iter()
                .any(|d| graph_document(d) && rejected.contains(&d.asset.path))
        {
            return None;
        }
        let mut graph = Self {
            path: self.path.clone(),
            fragment_declarations: self.fragment_declarations.clone(),
            ..Self::default()
        };
        for (index, doc) in self.primary_documents().iter().enumerate() {
            if graph_document(doc) && !rejected.contains(&doc.asset.path) {
                let new_index = graph.documents.len();
                graph.documents.push(doc.clone());
                index_model_document(&mut graph, new_index);
                for (name, _) in self.extensions.iter().filter(|(_, i)| **i == index) {
                    graph.extensions.insert(name.clone(), new_index);
                }
            }
        }
        for (namespace, observed) in &self.unpinned_extensions {
            if let Ok(doc) = observed
                && !rejected.contains(&doc.asset.path)
            {
                graph
                    .extensions
                    .insert(namespace.clone(), graph.documents.len());
                graph.documents.push(doc.clone());
            }
        }
        Some(graph)
    }

    pub fn normative_sources(&self) -> &BTreeMap<String, Asset> {
        &self.normative_sources
    }

    pub(crate) fn contract_asset(&self, id: &str) -> Result<&Asset, &str> {
        if let Some(asset) = self.normative_sources.get(id) {
            return Ok(asset);
        }
        let path = self
            .advisory_paths
            .get(id)
            .ok_or("The source could not be loaded")?
            .as_deref()
            .map_err(String::as_str)?;
        self.asset_at(path).ok_or("The source could not be loaded")
    }

    fn asset_at(&self, path: &str) -> Option<&Asset> {
        self.advisory_assets
            .get(path)
            .or_else(|| self.normative_sources.values().find(|a| a.path == path))
            .or_else(|| {
                self.primary_documents()
                    .iter()
                    .map(|d| &d.asset)
                    .find(|a| a.path == path)
            })
    }

    pub fn extensions(&self) -> impl Iterator<Item = (&str, &Document)> {
        self.extensions
            .iter()
            .map(|(namespace, index)| (namespace.as_str(), &self.documents[*index]))
    }

    // Workplan locations identify the declared include, while snapshot assets
    // identify its resolved path. Preserve both through symlink replacement.
    pub(crate) fn declared_fragment_path<'a>(&'a self, path: &'a str) -> &'a str {
        self.fragment_declarations
            .get(path)
            .map_or(path, String::as_str)
    }

    pub fn location(&self, id: &str) -> Option<&Location> {
        self.locations.get(id)
    }

    pub fn nodes(&self) -> impl ExactSizeIterator<Item = LocatedRecord<'_>> {
        self.nodes.iter().map(|(id, index)| LocatedRecord {
            id,
            value: &registry(&self.documents[*index].value, "nodes").expect("indexed nodes")[id],
            location: &self.locations[id],
        })
    }

    pub fn edges(&self) -> impl ExactSizeIterator<Item = LocatedRecord<'_>> {
        self.edges
            .iter()
            .map(|(id, document, index)| LocatedRecord {
                id,
                value: &array(get(&self.documents[*document].value, "edges"))
                    .expect("indexed edges")[*index],
                location: &self.locations[id],
            })
    }

    // Invalid global identities still have graph semantics. Retain their
    // occurrences for checks on other records and for ambiguity detection.
    pub(crate) fn model_edges(&self) -> impl Iterator<Item = LocatedRecord<'_>> {
        self.edges().chain(
            self.colliding_edges
                .iter()
                .map(|(id, doc, index, location)| LocatedRecord {
                    id,
                    value: &array(get(&self.documents[*doc].value, "edges"))
                        .expect("indexed edges")[*index],
                    location,
                }),
        )
    }

    pub fn records(&self) -> impl ExactSizeIterator<Item = LocatedRecord<'_>> {
        self.records
            .iter()
            .map(|(id, (index, location))| LocatedRecord {
                id,
                value: &registry(&self.documents[*index].value, "records")
                    .expect("indexed records")[id],
                location,
            })
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }

    pub fn exit_code(&self) -> u8 {
        let schema_results = self
            .representation_checks
            .values()
            .map(|check| check.result);
        if self
            .diagnostics
            .iter()
            .any(|issue| issue.outcome == Outcome::Fail)
            || schema_results
                .clone()
                .any(|result| result == CheckResult::Fail)
        {
            1
        } else if self.diagnostics.is_empty()
            && schema_results
                .into_iter()
                .all(|result| result == CheckResult::Pass)
        {
            0
        } else {
            2
        }
    }
}

/// Load the model, records, and every referenced contract source so that any
/// subsequent validation scope can be assessed without reopening files.
pub fn load(path: &Path) -> Bundle {
    capture(path, None, Some(&[]))
}

/// Capture normative model inputs and records without opening advisory contracts.
/// Use `load_for_readiness` when advisory contract diagnostics are needed.
pub fn load_model_only(path: &Path) -> Bundle {
    capture(path, None, None)
}

// Impact admits the manifest before following its declarations and admits
// ledgers before indexing their records, just like paired-input validation.
pub(crate) fn load_for_inspection(path: &Path) -> Bundle {
    load_primary(path, None, true)
}

/// Capture the model and advisory contracts in the selected read context. An
/// empty scope captures all contract inputs for subsequent readiness queries.
pub fn load_for_readiness(path: &Path, scope: &[String]) -> Bundle {
    capture(path, None, Some(scope))
}

/// Capture a primary model, its contract sources, and an explicit secondary
/// ledger. Reuse this bundle with any scope without reopening the input files.
pub fn load_with_records(model: &Path, secondary: &Path) -> Bundle {
    capture(model, Some(secondary), Some(&[]))
}

pub(crate) fn capture(model: &Path, secondary: Option<&Path>, scope: Option<&[String]>) -> Bundle {
    let mut bundle = load_primary(model, scope, secondary.is_some());
    if let Some(path) = secondary {
        match resolve_local(path) {
            Ok(resolved) if !bundle.declared_record_paths.contains(&resolved) => {
                let other = load_resolved(resolved, None, true, Some(&bundle));
                merge_records(&mut bundle, other);
            }
            Ok(_) => {}
            Err(mut issue) => {
                issue.message = format!(
                    "Secondary record input cannot be resolved: {}",
                    issue.message
                )
                .into();
                bundle.diagnostics.push(issue);
            }
        }
    }
    bundle
}

fn merge_records(primary: &mut Bundle, mut secondary: Bundle) {
    let first_secondary = primary.documents.len();
    primary.diagnostics.extend(
        secondary
            .diagnostics
            .drain(..)
            .filter(|issue| !issue.semantic_only),
    );
    if let Some(document) = secondary.accepted_root() {
        if document.kind != DocumentKind::Records {
            primary.diagnostics.push(composition_issue(
                "SCHEMA_INVALID",
                "Secondary input must be gdd-records/0.2.",
            ));
        } else if let Some(project_id) = primary
            .accepted_root()
            .filter(|m| m.kind.is_manifest())
            .and_then(|m| get(&m.value, "project"))
            .and_then(|p| get_string(p, "id"))
            && Some(project_id) != get_string(&document.value, "project_id")
        {
            primary.diagnostics.push(composition_issue(
                "RECORD_INVALID",
                "Secondary records belong to another project.",
            ));
        }
    }
    for (id, (index, location)) in secondary.records {
        match primary.records.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert((first_secondary + index, location));
            }
            Entry::Occupied(entry) => primary.diagnostics.push(composition_issue(
                "ID_COLLISION",
                format!("Record loaded twice: {}.", entry.key()),
            )),
        }
    }
    primary.secondary_start = Some(first_secondary);
    primary.documents.extend(secondary.documents);
    primary.representation_checks.extend(
        secondary
            .representation_checks
            .into_iter()
            .map(|(index, check)| (first_secondary + index, check)),
    );
}

fn load_primary(path: &Path, scope: Option<&[String]>, paired: bool) -> Bundle {
    match resolve_local(path) {
        Ok(path) => load_resolved(path, scope, paired, None),
        Err(issue) => Bundle {
            path: path.to_owned(),
            diagnostics: vec![issue],
            ..Bundle::default()
        },
    }
}

pub(crate) fn resolve_local(path: &Path) -> Result<PathBuf, Diagnostic> {
    let absolute = std::path::absolute(path).map_err(|error| path_io(path, error))?;
    resolve_asset_path(&absolute, &path.display().to_string())
}

fn load_resolved(
    path: PathBuf,
    scope: Option<&[String]>,
    paired: bool,
    primary: Option<&Bundle>,
) -> Bundle {
    let mut bundle = Bundle {
        path,
        ..Bundle::default()
    };
    let (Some(root), Some(name)) = (bundle.path.parent(), bundle.path.file_name()) else {
        bundle.diagnostics.push(invalid(
            "ASSET_PATH_INVALID",
            &bundle.path.display().to_string(),
            "",
            "Expected a manifest file.",
        ));
        return bundle;
    };
    let Some(name) = name.to_str() else {
        bundle.diagnostics.push(invalid(
            "ASSET_PATH_INVALID",
            &bundle.path.display().to_string(),
            "",
            "Asset paths must be valid Unicode.",
        ));
        return bundle;
    };
    let mut reader = Reader {
        primary,
        root: root.to_owned(),
        seen: BTreeSet::new(),
    };
    let document = match reader.document(name, None) {
        Ok(doc) => doc,
        Err(issue) => {
            bundle.diagnostics.push(issue);
            return bundle;
        }
    };
    let kind = document.kind;
    bundle.documents.push(document);
    // An unrecognised or invalid input contributes its own representation
    // result. Its fields cannot identify a project or declare other files.
    if paired && kind != DocumentKind::Other && !bundle.validate_document(0) {
        return bundle;
    }
    if kind.is_manifest() {
        load_model(&mut reader, &mut bundle, paired);
        if let Some(scope) = scope {
            capture_advisory_contracts(&reader, &mut bundle, scope);
        }
    } else if kind == DocumentKind::Records {
        index_records(&mut bundle, 0, None);
    } else if kind == DocumentKind::Other {
        let document = &bundle.documents[0];
        let issue = match get_string(&document.value, "format") {
            Some(format) => unknown(
                "FORMAT_UNSUPPORTED",
                &document.asset.path,
                "/format",
                format,
            ),
            None => Diagnostic {
                format_issue: Some(FormatIssue::Missing),
                ..shape(&document.asset.path, "/format", "a format string")
            },
        };
        bundle.diagnostics.push(issue);
    }
    bundle
}

fn load_model(reader: &mut Reader, bundle: &mut Bundle, paired: bool) {
    // Keep the original manifest intact while the other documents are loaded.
    let manifest = bundle.documents.pop().expect("a parsed manifest");
    let value = &manifest.value;
    let file = manifest.asset.path.as_str();
    let project_id = get(value, "project").and_then(|project| get_string(project, "id"));
    if project_id.is_none() {
        bundle
            .diagnostics
            .push(shape(file, "/project/id", "a project ID string"));
    }

    let fragment_kind = if manifest.kind == DocumentKind::Manifest02 {
        DocumentKind::Fragment02
    } else {
        DocumentKind::Fragment03
    };
    for relative in paths(value, "includes", file, &mut bundle.diagnostics) {
        match reader.document(relative, Some(fragment_kind)) {
            Ok(document) => {
                if let Value::Object(fields) = &document.value
                    && let Some(field) = fields
                        .keys()
                        .find(|field| !matches!(field.as_str(), "format" | "nodes" | "edges"))
                {
                    bundle.diagnostics.push(invalid(
                        "SCHEMA_INVALID",
                        &document.asset.path,
                        &format!("/{}", escape_segment(field)),
                        "A fragment can contain only format, nodes and edges.",
                    ));
                }
                if relative != document.asset.path {
                    bundle
                        .fragment_declarations
                        .insert(document.asset.path.clone(), relative.to_owned());
                }
                bundle.documents.push(document);
            }
            Err(issue) => bundle.diagnostics.push(issue),
        }
    }

    let mut namespaces = BTreeSet::new();
    for (id, declaration) in optional_registry(value, "extensions", file, &mut bundle.diagnostics) {
        let pointer = member_pointer("extensions", id);
        let (Some(path), Some(namespace), Some(version), Some(hash)) = (
            get_string(declaration, "path"),
            get_string(declaration, "namespace"),
            get_string(declaration, "version"),
            get_string(declaration, "sha256").filter(|hash| valid_hash(hash)),
        ) else {
            bundle.diagnostics.push(shape(
                file,
                &pointer,
                "an extension path, namespace, version and lowercase SHA-256 digest",
            ));
            continue;
        };
        if !namespaces.insert(namespace) {
            let mut issue = invalid(
                "EXTENSION_OVERRIDE",
                file,
                &pointer,
                "Multiple declarations for one extension namespace.",
            );
            issue.semantic_only = true;
            bundle.diagnostics.push(issue);
        }
        let document = match reader.asset(path).and_then(|asset| {
            if asset.sha256 != hash {
                let issue = invalid(
                    "EXTENSION_DATA_INVALID",
                    &asset.path,
                    "",
                    "Extension bytehash mismatch.",
                );
                bundle.unpinned_extensions.insert(
                    namespace.to_owned(),
                    parse_document(asset, Some(DocumentKind::Extension)),
                );
                Err(issue)
            } else {
                parse_document(asset, Some(DocumentKind::Extension))
            }
        }) {
            Ok(document) => document,
            Err(issue) => {
                if issue.code != "EXTENSION_DATA_INVALID" {
                    bundle.unread_extensions.insert(id.clone());
                }
                bundle.diagnostics.push(issue);
                continue;
            }
        };
        let identity_matches = get_string(&document.value, "namespace") == Some(namespace)
            && get_string(&document.value, "version") == Some(version);
        // Workplans retain pinned extension bytes even when the declaration
        // has an identity error. The loader check still fails the model report.
        // The manifest will be inserted at index zero.
        bundle
            .extensions
            .insert(namespace.into(), bundle.documents.len() + 1);
        if !identity_matches {
            bundle.diagnostics.push(invalid(
                "EXTENSION_IDENTITY_MISMATCH",
                &document.asset.path,
                "",
                "Extension namespace/version mismatch.",
            ));
        }
        bundle.documents.push(document);
    }

    for (id, source) in optional_registry(value, "sources", file, &mut bundle.diagnostics) {
        let pointer = member_pointer("sources", id);
        match get(source, "normative") {
            Some(Value::Boolean(false)) => continue,
            Some(Value::Boolean(true)) => {}
            _ => {
                bundle
                    .diagnostics
                    .push(shape(file, &pointer, "a source with a normative boolean"));
                continue;
            }
        }
        let Some(uri) = get_string(source, "uri") else {
            bundle
                .diagnostics
                .push(shape(file, &pointer, "a normative source URI string"));
            continue;
        };
        if is_external_uri(uri) {
            let mut issue = unknown(
                "SOURCE_UNAVAILABLE",
                file,
                &pointer,
                "Materialize the normative source within the bundle.",
            );
            issue.dimension = "integration";
            bundle.diagnostics.push(issue);
            continue;
        }
        let hash = get(source, "revision").and_then(|revision| get_string(revision, "sha256"));
        let Some(hash) = hash.filter(|hash| valid_hash(hash)) else {
            bundle.diagnostics.push(invalid(
                "CONTRACT_SOURCE_UNPINNED",
                file,
                &pointer,
                "Normative source requires a lowercase SHA-256 digest.",
            ));
            continue;
        };
        match reader.asset(uri) {
            Ok(asset) if asset.sha256 == hash => {
                bundle.normative_sources.insert(id.to_owned(), asset);
            }
            Ok(_) => bundle.diagnostics.push(invalid(
                "CONTRACT_SOURCE_UNPINNED",
                file,
                &pointer,
                "Normative source hash mismatch.",
            )),
            Err(mut issue) => {
                issue.location = Location {
                    file: file.into(),
                    pointer,
                };
                issue.dimension = "integration";
                bundle.diagnostics.push(issue);
            }
        }
    }

    // Record files have diagnostics of their own and do not affect this revision.
    bundle.documents.insert(0, manifest);
    for index in 0..bundle.documents.len() {
        index_model_document(bundle, index);
    }
    // These diagnostics leave all normative reads complete and pinned.
    if bundle.diagnostics.iter().all(|item| {
        item.semantic_only || matches!(item.code, "ID_COLLISION" | "EXTENSION_IDENTITY_MISMATCH")
    }) {
        build_snapshot(bundle);
    }

    let manifest = &bundle.documents[0];
    let mut record_issues = Vec::new();
    let record_paths = paths(
        &manifest.value,
        "record_files",
        &manifest.asset.path,
        &mut record_issues,
    );
    let project_id = get(&manifest.value, "project")
        .and_then(|project| get_string(project, "id"))
        .map(str::to_owned);
    let record_documents: Vec<_> = record_paths
        .into_iter()
        .filter_map(|relative| {
            let record = reader.relative_path(relative).and_then(|normalized| {
                if paired {
                    // Remember even an unreadable or malformed declaration so an
                    // explicit alias never retries the same operational input.
                    bundle
                        .declared_record_paths
                        .insert(reader.root.join(&normalized));
                }
                let asset = reader.resolved_asset(normalized, relative)?;
                parse_document(asset, Some(DocumentKind::Records))
            });
            match record {
                Ok(document) => Some(document),
                Err(issue) => {
                    record_issues.push(issue);
                    None
                }
            }
        })
        .collect();
    bundle.documents.extend(record_documents);
    bundle.diagnostics.extend(record_issues);
    for index in 1..bundle.documents.len() {
        if bundle.documents[index].kind == DocumentKind::Records
            && (!paired || bundle.validate_document(index))
        {
            index_records(bundle, index, project_id.as_deref());
        }
    }
}

// Capture advisory bytes only when a contract explicitly refers to them. They
// remain outside the snapshot and cannot establish readiness on their own.
fn capture_advisory_contracts(reader: &Reader, bundle: &mut Bundle, scope: &[String]) {
    let declarations = registry(&bundle.documents[0].value, "sources");
    if declarations.is_none_or(|sources| {
        !sources
            .values()
            .any(|s| get(s, "normative") == Some(&Value::Boolean(false)))
    }) {
        return;
    }
    let graph = (!scope.is_empty()).then(|| crate::context::ContextGraph::new(bundle));
    let context = graph
        .as_ref()
        .map(|g| g.scope(scope.iter().map(String::as_str)));
    let mut references = BTreeSet::new();
    let nodes = bundle
        .nodes()
        .filter(|item| context.as_ref().is_none_or(|c| c.contains(item.id)));
    let edges = bundle.model_edges().filter(|item| {
        context.as_ref().is_none_or(|c| {
            ["from", "to"]
                .iter()
                .all(|key| get_string(item.value, key).is_some_and(|id| c.contains(id)))
        })
    });
    for item in nodes.chain(edges) {
        for field in ["source_refs", "design_refs"] {
            for reference in array(get(item.value, field)).into_iter().flatten() {
                if (field == "design_refs" || get_string(reference, "purpose") == Some("contract"))
                    && let Some(id) = get_string(reference, "source")
                {
                    references.insert(id);
                }
            }
        }
    }
    let advisory: Vec<_> = references
        .into_iter()
        .filter_map(|id| {
            let source = declarations?.get(id)?;
            if get(source, "normative") != Some(&Value::Boolean(false)) {
                return None;
            }
            Some((id.to_owned(), get_string(source, "uri")?.to_owned()))
        })
        .collect();
    for (id, uri) in advisory {
        let result = reader.relative_path(&uri).and_then(|path| {
            if bundle.asset_at(&path).is_none() {
                let asset = reader.read_asset(path.clone())?;
                bundle.advisory_assets.insert(path.clone(), asset);
            }
            Ok(path)
        });
        bundle
            .advisory_paths
            .insert(id, result.map_err(|issue| issue.message.into_string()));
    }
}

fn build_snapshot(bundle: &mut Bundle) {
    let mut assets: Vec<_> = bundle
        .documents
        .iter()
        .map(|doc| &doc.asset)
        .chain(bundle.normative_sources.values())
        .collect();
    assets.sort_unstable_by_key(|asset| asset.path.as_str());
    let pairs: Vec<_> = assets
        .iter()
        .map(|asset| (asset.path.as_str(), asset.sha256.as_str()))
        .collect();
    match snapshot_revision(&pairs) {
        Ok(revision) => {
            bundle.snapshot = Some(Snapshot {
                format: "gdd-snapshot/0.2",
                revision,
                assets: assets
                    .into_iter()
                    .map(|asset| SnapshotAsset {
                        path: asset.path.clone(),
                        sha256: asset.sha256.clone(),
                    })
                    .collect(),
            });
        }
        Err(error) => bundle.diagnostics.push(invalid(
            "ASSET_PATH_INVALID",
            &bundle.documents[0].asset.path,
            "",
            error.to_string(),
        )),
    }
}

fn insert_location(
    locations: &mut BTreeMap<String, Location>,
    diagnostics: &mut Vec<Diagnostic>,
    id: &str,
    file: &str,
    pointer: String,
) -> bool {
    match locations.entry(id.into()) {
        Entry::Vacant(entry) => {
            entry.insert(Location {
                file: file.into(),
                pointer,
            });
            true
        }
        Entry::Occupied(entry) => {
            let mut issue = collision(id, file, &pointer);
            // Including a fragment must not override a node. Other duplicate
            // model identifiers are graph identity errors.
            issue.semantic_only =
                !(pointer.starts_with("/nodes/") && entry.get().pointer.starts_with("/nodes/"));
            diagnostics.push(issue);
            false
        }
    }
}

fn collision(id: &str, file: &str, pointer: &str) -> Diagnostic {
    let mut diagnostic = invalid(
        "ID_COLLISION",
        file,
        pointer,
        format!("ID occurs more than once: {id}."),
    );
    diagnostic.related_ids = Box::new([id.into()]);
    diagnostic
}

fn index_model_document(bundle: &mut Bundle, index: usize) {
    let doc = &bundle.documents[index];
    let manifest = doc.kind.is_manifest();
    if !manifest
        && !matches!(
            doc.kind,
            DocumentKind::Fragment02 | DocumentKind::Fragment03
        )
    {
        return;
    }
    let file = doc.asset.path.as_str();
    let fields = if manifest {
        &["nodes", "sources", "questions", "extensions"][..]
    } else {
        &["nodes"][..]
    };
    for &field in fields {
        let Some(entries) = registry(&doc.value, field) else {
            // The loader has already checked the two asset-declaring registries.
            if field == "nodes" || field == "questions" && get(&doc.value, field).is_some() {
                bundle
                    .diagnostics
                    .push(shape(file, &format!("/{field}"), "an object"));
            }
            continue;
        };
        for (id, value) in entries {
            let pointer = member_pointer(field, id);
            if field == "nodes" && !matches!(value, Value::Object(_)) {
                bundle
                    .diagnostics
                    .push(shape(file, &pointer, "a node object"));
                continue;
            }
            let inserted = insert_location(
                &mut bundle.locations,
                &mut bundle.diagnostics,
                id,
                file,
                pointer.clone(),
            );
            if field == "nodes" && !bundle.nodes.contains_key(id) {
                bundle.nodes.insert(id.clone(), index);
                if !inserted {
                    bundle.locations.insert(
                        id.clone(),
                        Location {
                            file: file.into(),
                            pointer,
                        },
                    );
                }
            }
        }
    }
    let Some(edges) = array(get(&doc.value, "edges")) else {
        bundle.diagnostics.push(shape(file, "/edges", "an array"));
        return;
    };
    for (edge_index, edge) in edges.iter().enumerate() {
        let pointer = format!("/edges/{edge_index}");
        let Some(id) = get_string(edge, "id") else {
            bundle
                .diagnostics
                .push(shape(file, &format!("{pointer}/id"), "an edge ID string"));
            continue;
        };
        if insert_location(
            &mut bundle.locations,
            &mut bundle.diagnostics,
            id,
            file,
            pointer.clone(),
        ) {
            bundle.edges.push((id.into(), index, edge_index));
        } else {
            bundle.colliding_edges.push((
                id.into(),
                index,
                edge_index,
                Location {
                    file: file.into(),
                    pointer,
                },
            ));
        }
    }
}

fn index_records(bundle: &mut Bundle, index: usize, project_id: Option<&str>) {
    let doc = &bundle.documents[index];
    let file = doc.asset.path.as_str();
    let actual = get_string(&doc.value, "project_id");
    if actual.is_none() {
        bundle
            .diagnostics
            .push(shape(file, "/project_id", "a project ID string"));
    } else if project_id.is_some() && actual != project_id {
        bundle.diagnostics.push(invalid(
            "RECORD_INVALID",
            file,
            "/project_id",
            "Record project mismatch.",
        ));
    }
    let Some(records) = registry(&doc.value, "records") else {
        bundle
            .diagnostics
            .push(shape(file, "/records", "an object"));
        return;
    };
    for (id, value) in records {
        let pointer = member_pointer("records", id);
        if !matches!(value, Value::Object(_)) {
            bundle
                .diagnostics
                .push(shape(file, &pointer, "a record object"));
            continue;
        }
        match bundle.records.entry(id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert((
                    index,
                    Location {
                        file: file.into(),
                        pointer,
                    },
                ));
            }
            Entry::Occupied(_) => bundle.diagnostics.push(collision(id, file, &pointer)),
        }
    }
}

fn paths<'a>(
    value: &'a Value,
    field: &str,
    file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<&'a str> {
    let Some(value) = get(value, field) else {
        return Vec::new();
    };
    let Some(items) = array(Some(value)) else {
        diagnostics.push(shape(
            file,
            &format!("/{field}"),
            "an array of local file paths",
        ));
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item {
            Value::String(path) => Some(path.as_str()),
            _ => {
                diagnostics.push(shape(
                    file,
                    &format!("/{field}/{index}"),
                    "a local file path string",
                ));
                None
            }
        })
        .collect()
}

fn optional_registry<'a>(
    value: &'a Value,
    field: &str,
    file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> impl Iterator<Item = (&'a String, &'a Value)> + use<'a> {
    let entries = registry(value, field);
    if entries.is_none() && get(value, field).is_some() {
        diagnostics.push(shape(file, &format!("/{field}"), "an object"));
    }
    entries.into_iter().flatten()
}

fn get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(values) => values.get(key),
        _ => None,
    }
}

fn get_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    match get(value, key)? {
        Value::String(value) => Some(value),
        _ => None,
    }
}

fn array(value: Option<&Value>) -> Option<&[Value]> {
    match value? {
        Value::Array(values) => Some(values),
        _ => None,
    }
}

fn registry<'a>(value: &'a Value, key: &str) -> Option<&'a BTreeMap<String, Value>> {
    match get(value, key)? {
        Value::Object(values) => Some(values),
        _ => None,
    }
}

fn is_external_uri(uri: &str) -> bool {
    // A single-letter prefix followed by a colon can denote a drive path.
    uri.split_once(':').is_some_and(|(scheme, _)| {
        scheme.len() > 1
            && scheme.as_bytes()[0].is_ascii_alphabetic()
            && scheme
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    })
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn member_pointer(registry: &str, id: &str) -> String {
    format!("/{registry}/{}", escape_segment(id))
}

fn escape_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn composition_issue(code: &'static str, message: impl Into<String>) -> Diagnostic {
    invalid(code, "", "", message)
}

fn shape(file: &str, pointer: &str, expected: &str) -> Diagnostic {
    invalid(
        "SCHEMA_INVALID",
        file,
        pointer,
        format!("Expected {expected}."),
    )
}

fn invalid(
    code: &'static str,
    file: &str,
    pointer: &str,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        code,
        outcome: Outcome::Fail,
        dimension: "model",
        message: message.into().into_boxed_str(),
        location: Location {
            file: file.into(),
            pointer: pointer.into(),
        },
        related_ids: Box::new([]),
        semantic_only: false,
        format_issue: None,
    }
}

fn unknown(
    code: &'static str,
    file: &str,
    pointer: &str,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        outcome: Outcome::Unknown,
        ..invalid(code, file, pointer, message)
    }
}

struct Reader<'a> {
    primary: Option<&'a Bundle>,
    root: PathBuf,
    seen: BTreeSet<String>,
}

impl Reader<'_> {
    fn document(
        &mut self,
        path: &str,
        expected: Option<DocumentKind>,
    ) -> Result<Document, Diagnostic> {
        parse_document(self.asset(path)?, expected)
    }

    fn asset(&mut self, path: &str) -> Result<Asset, Diagnostic> {
        let normalized = self.relative_path(path)?;
        self.resolved_asset(normalized, path)
    }

    fn resolved_asset(&mut self, normalized: String, path: &str) -> Result<Asset, Diagnostic> {
        if !self.seen.insert(normalized.clone()) {
            return Err(invalid(
                "ASSET_PATH_INVALID",
                path,
                "",
                "Asset has already been loaded.",
            ));
        }
        self.read_asset(normalized)
    }

    fn relative_path(&self, path: &str) -> Result<String, Diagnostic> {
        if path.is_empty() || path.starts_with('/') || path.contains(['\\', ':', '\0']) {
            return Err(invalid(
                "ASSET_PATH_INVALID",
                path,
                "",
                "Expected a relative POSIX path.",
            ));
        }
        let candidate = self.root.join(path);
        let local = resolve_asset_path(&candidate, path)?;
        let relative = local
            .strip_prefix(&self.root)
            .map_err(|_| invalid("ASSET_PATH_INVALID", path, "", "Asset escapes bundle root."))?;
        let normalized = portable_path(relative).ok_or_else(|| {
            invalid(
                "ASSET_PATH_INVALID",
                path,
                "",
                "Asset path must name a valid local file.",
            )
        })?;
        Ok(normalized)
    }

    fn read_asset(&self, normalized: String) -> Result<Asset, Diagnostic> {
        // A separate ledger may already be a pinned source. The model revision
        // and the parsed records must refer to the same captured bytes.
        if let Some(primary) = self.primary
            && let Some(root) = primary.path().parent()
            && let Ok(relative) = self.root.join(&normalized).strip_prefix(root)
            && let Some(path) = portable_path(relative)
            && let Some(asset) = primary.asset_at(&path)
        {
            return Ok(Asset {
                path: normalized,
                sha256: asset.sha256.clone(),
                bytes: asset.bytes.clone(),
            });
        }
        let bytes = read_regular(&self.root, Path::new(&normalized)).map_err(|error| {
            unknown(
                "SOURCE_UNAVAILABLE",
                &normalized,
                "",
                format!(
                    "Cannot read asset {}: {}.",
                    self.root.join(&normalized).display(),
                    error.to_string().trim_end_matches('.')
                ),
            )
        })?;
        Ok(Asset {
            path: normalized,
            sha256: sha256(&bytes),
            bytes,
        })
    }
}

fn parse_document(asset: Asset, expected: Option<DocumentKind>) -> Result<Document, Diagnostic> {
    let path = &asset.path;
    let value = input::parse(&asset.bytes).map_err(|error| match error {
        InputError::UnsupportedDepth => unknown("FORMAT_UNSUPPORTED", path, "", error.to_string()),
        InputError::DuplicateKey(_) => invalid("YAML_DUPLICATE_KEY", path, "", error.to_string()),
        InputError::Invalid(_) => invalid("YAML_FEATURE_FORBIDDEN", path, "", error.to_string()),
    })?;
    let kind = DocumentKind::from_value(&value);
    if let Some(expected) = expected
        && kind != expected
    {
        if kind == DocumentKind::Other
            && let Some(format) = get_string(&value, "format")
        {
            return Err(unknown("FORMAT_UNSUPPORTED", path, "/format", format));
        }
        let actual = get_string(&value, "format");
        let format_issue = if actual.is_some() {
            FormatIssue::Expected(expected)
        } else {
            FormatIssue::Missing
        };
        return Err(Diagnostic {
            format_issue: Some(format_issue),
            ..invalid(
                "SCHEMA_INVALID",
                path,
                "/format",
                format!(
                    "Expected {}, got {}.",
                    expected.format(),
                    actual.unwrap_or("an invalid format")
                ),
            )
        });
    }
    Ok(Document { kind, asset, value })
}

fn portable_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return None;
        };
        let part = part.to_str()?;
        if part.contains(['\\', ':', '\0']) {
            return None;
        }
        parts.push(part);
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

// Resolve dangling links and dot segments before checking containment. Existing
// paths go through canonicalize for platform-specific spelling and link rules.
fn resolve_asset_path(path: &Path, display: &str) -> Result<PathBuf, Diagnostic> {
    match fs::canonicalize(path) {
        Ok(resolved) => return Ok(resolved),
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            return Err(link_or_io(display, error));
        }
        Err(_) => {}
    }

    let mut pending: Vec<OsString> = path.iter().rev().map(OsString::from).collect();
    let mut resolved = PathBuf::new();
    let mut links = 0;
    while let Some(part) = pending.pop() {
        match Path::new(&part).components().next() {
            Some(Component::ParentDir) => {
                resolved.pop();
            }
            Some(Component::Normal(_)) => {
                resolved.push(part);
                match fs::symlink_metadata(&resolved) {
                    Ok(metadata) if metadata.is_symlink() => {
                        links += 1;
                        if links > 40 {
                            return Err(invalid(
                                "ASSET_PATH_INVALID",
                                display,
                                "",
                                "Cannot resolve a symlink loop or chain longer than 40 links.",
                            ));
                        }
                        let destination =
                            fs::read_link(&resolved).map_err(|error| link_or_io(display, error))?;
                        resolved.pop();
                        if destination.is_absolute() {
                            resolved.clear();
                        }
                        pending.extend(destination.iter().rev().map(OsString::from));
                    }
                    Ok(_) => {
                        // Normalize existing prefixes, including their spelling on Windows.
                        resolved = fs::canonicalize(&resolved)
                            .map_err(|error| link_or_io(display, error))?;
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(link_or_io(display, error)),
                }
            }
            Some(Component::Prefix(_) | Component::RootDir) => resolved.push(part),
            _ => {}
        }
    }
    Ok(resolved)
}

fn path_io(path: &Path, error: io::Error) -> Diagnostic {
    link_or_io(&path.display().to_string(), error)
}

fn link_or_io(path: &str, error: io::Error) -> Diagnostic {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error()) {
        return invalid(
            "ASSET_PATH_INVALID",
            path,
            "",
            format!("Cannot resolve symlink: {error}."),
        );
    }
    #[cfg(windows)]
    if error.raw_os_error() == Some(1921) {
        return invalid(
            "ASSET_PATH_INVALID",
            path,
            "",
            format!("Cannot resolve symlink: {error}."),
        );
    }
    unknown(
        "SOURCE_UNAVAILABLE",
        path,
        "",
        format!("Cannot resolve asset: {error}."),
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn read_regular(root: &Path, relative: &Path) -> io::Result<Vec<u8>> {
    use rustix::fs::{Mode, OFlags, openat};

    // Open the canonical root and its descendants without following links that
    // may have been substituted after path resolution.
    let mut parent = File::from(rustix::fs::open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )?);
    let mut parts = relative.iter().peekable();
    while let Some(part) = parts.next() {
        let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
        if parts.peek().is_none() {
            let flags = flags | OFlags::NONBLOCK | OFlags::NOCTTY;
            let file = openat(&parent, part, flags, Mode::empty())?;
            return read_file(File::from(file));
        }
        parent = File::from(openat(
            &parent,
            part,
            flags | OFlags::DIRECTORY,
            Mode::empty(),
        )?);
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "Expected a file path",
    ))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn read_regular(root: &Path, relative: &Path) -> io::Result<Vec<u8>> {
    read_file(File::open(root.join(relative))?)
}

fn read_file(mut file: File) -> io::Result<Vec<u8>> {
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Expected a regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod secondary_capture_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn secondary_reads_previously_captured_source_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("observations.json");
        let manifest = directory.path().join("model.json");
        let original = br#"{"format":"gdd-records/0.2","project_id":"demo","records":{}}"#;
        fs::write(&source, original).unwrap();
        let model = json!({"format":"gdd/0.2","project":{"id":"demo","title":"Demo","goal":"Review."},"nodes":{},"edges":[],
            "sources":{"source.observations":{"role":"input","title":"Recorded observations","uri":"observations.json",
                "format":"json","normative":true,"revision":{"sha256":sha256(original)}}}});
        fs::write(&manifest, serde_json::to_vec(&model).unwrap()).unwrap();
        let mut primary = load_primary(&manifest, None, true);
        assert!(primary.snapshot().is_some());
        fs::write(&source, b"{}").unwrap();
        let secondary = load_resolved(source, None, true, Some(&primary));
        assert_eq!(secondary.root_document().unwrap().asset.bytes, original);
        merge_records(&mut primary, secondary);
        assert_eq!(
            crate::validation::validate(&primary, crate::validation::Profile::Model).exit_code(),
            0
        );
    }
}
