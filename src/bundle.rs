//! Load model documents and compute a snapshot from the bytes read.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::input::{self, InputError, Value};
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

#[derive(Debug)]
pub struct Asset {
    /// Resolved path relative to the bundle root, with POSIX separators.
    pub path: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
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

#[derive(Debug)]
pub struct Diagnostic {
    pub code: &'static str,
    pub outcome: Outcome,
    pub dimension: &'static str,
    pub message: Box<str>,
    pub location: Location,
    pub related_ids: Box<[String]>,
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
    normative_sources: BTreeMap<String, Asset>,
    extensions: BTreeMap<String, usize>,
    locations: BTreeMap<String, Location>,
    nodes: BTreeMap<String, usize>,
    edges: Vec<(String, usize, usize)>,
    records: BTreeMap<String, (usize, Location)>,
    diagnostics: Vec<Diagnostic>,
    snapshot: Option<Snapshot>,
}

impl Bundle {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root_document(&self) -> Option<&Document> {
        self.documents.first()
    }

    pub fn manifest(&self) -> Option<&Document> {
        self.root_document().filter(|doc| doc.kind.is_manifest())
    }

    pub fn documents(&self) -> &[Document] {
        &self.documents
    }

    pub fn normative_sources(&self) -> &BTreeMap<String, Asset> {
        &self.normative_sources
    }

    pub fn extensions(&self) -> impl Iterator<Item = (&str, &Document)> {
        self.extensions
            .iter()
            .map(|(namespace, index)| (namespace.as_str(), &self.documents[*index]))
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
        if self
            .diagnostics
            .iter()
            .any(|issue| issue.outcome == Outcome::Fail)
        {
            1
        } else if self.diagnostics.is_empty() {
            0
        } else {
            2
        }
    }
}

/// Load one root document and its model assets. Structural loading checks cover
/// the fields used to select, combine and hash local assets.
pub fn load(path: &Path) -> Bundle {
    let mut bundle = Bundle {
        path: path.to_owned(),
        ..Bundle::default()
    };
    let resolved = match fs::canonicalize(path) {
        Ok(path) => path,
        Err(error) => {
            bundle.diagnostics.push(path_io(path, error));
            return bundle;
        }
    };
    bundle.path = resolved;
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
    if kind.is_manifest() {
        load_model(&mut reader, &mut bundle);
    } else if kind == DocumentKind::Records {
        index_records(&mut bundle, 0, None);
    } else if kind == DocumentKind::Other {
        let document = &bundle.documents[0];
        let issue = match get_string(&document.value, "format") {
            Some(format) => unknown(
                "FORMAT_UNSUPPORTED",
                &document.asset.path,
                "/format",
                format!("Unsupported format {format:?}."),
            ),
            None => shape(&document.asset.path, "/format", "a format string"),
        };
        bundle.diagnostics.push(issue);
    }
    bundle
}

fn load_model(reader: &mut Reader, bundle: &mut Bundle) {
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
            bundle.diagnostics.push(invalid(
                "EXTENSION_OVERRIDE",
                file,
                &pointer,
                "Multiple declarations for one extension namespace.",
            ));
            continue;
        }
        let document = match reader.asset(path).and_then(|asset| {
            if asset.sha256 != hash {
                Err(invalid(
                    "EXTENSION_DATA_INVALID",
                    &asset.path,
                    "",
                    "Extension bytehash mismatch.",
                ))
            } else {
                parse_document(asset, Some(DocumentKind::Extension))
            }
        }) {
            Ok(document) => document,
            Err(issue) => {
                bundle.diagnostics.push(issue);
                continue;
            }
        };
        let identity_matches = get_string(&document.value, "namespace") == Some(namespace)
            && get_string(&document.value, "version") == Some(version);
        if identity_matches {
            // The manifest will be inserted at index zero.
            bundle
                .extensions
                .insert(namespace.into(), bundle.documents.len() + 1);
        } else {
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
    if bundle
        .diagnostics
        .iter()
        .all(|item| matches!(item.code, "ID_COLLISION" | "EXTENSION_IDENTITY_MISMATCH"))
    {
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
        .filter_map(
            |relative| match reader.document(relative, Some(DocumentKind::Records)) {
                Ok(document) => Some(document),
                Err(issue) => {
                    record_issues.push(issue);
                    None
                }
            },
        )
        .collect();
    bundle.documents.extend(record_documents);
    bundle.diagnostics.extend(record_issues);
    for index in 1..bundle.documents.len() {
        if bundle.documents[index].kind == DocumentKind::Records {
            index_records(bundle, index, project_id.as_deref());
        }
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
        Entry::Occupied(_) => {
            diagnostics.push(collision(id, file, &pointer));
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
            if insert_location(
                &mut bundle.locations,
                &mut bundle.diagnostics,
                id,
                file,
                pointer,
            ) && field == "nodes"
            {
                bundle.nodes.insert(id.clone(), index);
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
            pointer,
        ) {
            bundle.edges.push((id.into(), index, edge_index));
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

struct Reader {
    root: PathBuf,
    seen: BTreeSet<String>,
}

impl Reader {
    fn document(
        &mut self,
        path: &str,
        expected: Option<DocumentKind>,
    ) -> Result<Document, Diagnostic> {
        parse_document(self.asset(path)?, expected)
    }

    fn asset(&mut self, path: &str) -> Result<Asset, Diagnostic> {
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
        if !self.seen.insert(normalized.clone()) {
            return Err(invalid(
                "ASSET_PATH_INVALID",
                path,
                "",
                "Asset has already been loaded.",
            ));
        }
        let bytes = read_regular(&self.root, relative).map_err(|error| {
            unknown(
                "SOURCE_UNAVAILABLE",
                &normalized,
                "",
                format!("Cannot read asset: {error}."),
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
        return Err(invalid(
            "SCHEMA_INVALID",
            path,
            "/format",
            format!(
                "Expected {}, got {}.",
                expected.format(),
                get_string(&value, "format").unwrap_or("an invalid format")
            ),
        ));
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
fn read_regular(root: &Path, relative: &Path) -> io::Result<Vec<u8>> {
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
fn read_regular(root: &Path, relative: &Path) -> io::Result<Vec<u8>> {
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
