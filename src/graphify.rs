//! Pure observations of one supplied Graphify artifact.

mod decode;
mod records;

use crate::input::MAX_CONTAINER_DEPTH;
use crate::report::CheckResult::{Fail, Pass, Unknown};
use crate::report::{Check, CheckResult, Diagnostic, checks_exit_code};
use jsonschema::Validator;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{io, sync::OnceLock};

pub const AST_PROFILE: &str = "graphify-0.9.75-ast";
pub const NODE_LINK_PROFILE: &str = "graphify-0.9.75-node-link";
const RAW: &str = "graphify.raw.json";
const IMPORTER: &str = "gdd-graphify";
const VERSION: &str = "0.1.2";

/// Caller-supplied layout, locator and provenance claims for one byte snapshot.
#[derive(Clone, Copy, Debug)]
pub struct GraphifyOptions<'a> {
    pub profile: Option<&'a str>,
    pub producer_version: Option<&'a str>,
    pub code_revision: Option<&'a str>,
    pub source_id: &'a str,
    pub locator: &'a str,
}

impl Default for GraphifyOptions<'_> {
    fn default() -> Self {
        Self {
            profile: None,
            producer_version: None,
            code_revision: None,
            source_id: "source.graphify",
            locator: "",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ImportReport {
    format: &'static str,
    tool: Value,
    spec_revision: Option<String>,
    profiles: [&'static str; 1],
    scope: Vec<String>,
    pub checks: Vec<Check>,
}

impl ImportReport {
    pub fn exit_code(&self) -> u8 {
        checks_exit_code(self.checks.iter())
    }

    fn result(
        &mut self,
        name: &str,
        status: CheckResult,
        reason: impl Into<String>,
        code: Option<&'static str>,
    ) {
        let reason = reason.into();
        let diagnostics = code
            .into_iter()
            .map(|c| issue(c, status, &reason, ""))
            .collect();
        self.check(name, status, reason, diagnostics);
    }

    fn check(
        &mut self,
        name: &str,
        status: CheckResult,
        reason: impl Into<String>,
        diagnostics: Vec<Diagnostic>,
    ) {
        let mut check = Check::new(format!("graphify.{name}"), "integration", status, reason);
        check.diagnostics = diagnostics;
        self.checks.push(check);
    }
}

/// Wrap integration checks in the Graphify adapter report envelope.
pub fn import_report(checks: Vec<Check>) -> ImportReport {
    ImportReport {
        format: "gdd-report/0.2",
        tool: json!({"name": IMPORTER, "version": VERSION}),
        spec_revision: None,
        profiles: ["import.graphify"],
        scope: Vec::new(),
        checks,
    }
}

fn issue(
    code: &'static str,
    result: CheckResult,
    reason: impl Into<String>,
    path: &str,
) -> Diagnostic {
    Diagnostic::new(code, result, reason, Some(RAW), path)
}

/// Keep the identity and caller claims of an observation index, discarding producer projections.
pub fn compact_observations(index: &Value) -> Value {
    let mut compact = Map::new();
    for key in [
        "format",
        "source_id",
        "artifact",
        "importer",
        "input_claims",
    ] {
        compact.insert(key.into(), index[key].clone());
    }
    compact.insert("producer_claims".into(), Value::Object(Map::new()));
    for key in ["nodes", "edges", "opaque", "binding_candidates"] {
        compact.insert(key.into(), Value::Array(Vec::new()));
    }
    Value::Object(compact)
}

fn observations_schema() -> &'static Validator {
    static SCHEMA: OnceLock<Validator> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        let source = serde_json::from_str(include_str!("../schemas/observations.schema.json"))
            .expect("embedded observations schema is JSON");
        crate::validation::build_schema(&source)
            .expect("embedded observations schema compiles offline")
    })
}

/// Hash and interpret the original bytes. Invalid caller options return an
/// error. Artifact failures and unresolved checks are included in the report.
pub fn observe_graphify(
    raw: &[u8],
    options: GraphifyOptions<'_>,
) -> io::Result<(Value, ImportReport)> {
    if !crate::valid_id(options.source_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source_id must be a GDD identifier.",
        ));
    }
    let revision = format!("sha256:{}", crate::sha256(raw));
    let mut index = json!({
        "format": "gdd-observations/0.2", "source_id": options.source_id,
        "artifact": {"path": RAW, "revision": revision, "byte_count": raw.len()},
        "importer": {"name": IMPORTER, "version": VERSION, "profile": options.profile},
        "input_claims": {"locator": options.locator, "producer_version": options.producer_version,
                         "code_revision": options.code_revision, "trust": "unverified"},
        "producer_claims": {}, "nodes": [], "edges": [], "opaque": [], "binding_candidates": []
    });
    let mut report = import_report(Vec::new());
    report.result("raw", Pass, format!("Supplied artifact {revision}: original bytes hashed; producer completeness and code correctness are not inferred."), None);
    if let Some(profile) = options.profile {
        match decode::decode(raw, MAX_CONTAINER_DEPTH) {
            Err(error) => report.result("json", Fail, error, Some("SCHEMA_INVALID")),
            Ok((data, _)) if !data.is_object() => report.result("layout", Fail,
                "Graphify artifact must be a JSON object.", Some("SCHEMA_INVALID")),
            Ok((_, true)) => report.result("depth", Unknown,
                format!("Raw JSON container depth exceeds {MAX_CONTAINER_DEPTH}; no producer projection is asserted. Original bytes retained."),
                Some("FORMAT_UNSUPPORTED")),
            Ok((data, false)) => project(&data, &revision, profile, options, &mut index, &mut report),
        }
    } else {
        report.result("layout", Unknown, "No Graphify interpretation profile supplied; input bytes retained without guessing their format.", Some("FORMAT_UNSUPPORTED"));
    }
    observations_schema()
        .validate(&index)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    Ok((index, report))
}

fn project(
    data: &Value,
    revision: &str,
    profile: &str,
    options: GraphifyOptions<'_>,
    index: &mut Value,
    report: &mut ImportReport,
) {
    let mut revisions = Map::new();
    let mut unsupported_version = options.producer_version.is_some_and(|v| v != "0.9.75");
    for (container, prefix) in [(data, ""), (&data["graph"], "/graph")] {
        for field in [
            "graphify_version",
            "producer_version",
            "schema_version",
            "format",
            "built_at_commit",
            "code_revision",
        ] {
            if let Some(value) = container.get(field) {
                let pointer = format!("{prefix}/{field}");
                match field {
                    "graphify_version" | "producer_version" => {
                        unsupported_version |= value != "0.9.75"
                    }
                    "schema_version" | "format" => unsupported_version = true,
                    _ => {
                        revisions.insert(pointer.clone(), value.clone());
                    }
                }
                index["producer_claims"][pointer] = value.clone();
            }
        }
    }
    let edge_key = match profile {
        AST_PROFILE => "edges",
        NODE_LINK_PROFILE => "links",
        _ => {
            report.result(
                "layout",
                Unknown,
                "Select an explicitly supported Graphify layout profile.",
                Some("FORMAT_UNSUPPORTED"),
            );
            return;
        }
    };
    if unsupported_version {
        report.result(
            "layout",
            Unknown,
            "Declared producer/schema version or format is outside the inspected profiles.",
            Some("FORMAT_UNSUPPORTED"),
        );
        return;
    }
    if data.get("edges").is_some() && data.get("links").is_some() {
        report.result(
            "layout",
            Unknown,
            "Both edges and links are present; no collection is silently discarded.",
            Some("FORMAT_UNSUPPORTED"),
        );
        return;
    }
    let mut errors = Vec::new();
    if !data["nodes"].is_array() || !data[edge_key].is_array() {
        errors.push(issue(
            "SCHEMA_INVALID",
            Fail,
            format!("Profile requires nodes and {edge_key} arrays."),
            "",
        ));
    }
    if profile == NODE_LINK_PROFILE {
        if !data["directed"].is_boolean()
            || !data["multigraph"].is_boolean()
            || !data["graph"].is_object()
        {
            errors.push(issue(
                "SCHEMA_INVALID",
                Fail,
                "Node-link profile requires boolean directed/multigraph and object graph.",
                "",
            ));
        }
    } else if data.get("directed").is_some_and(|d| !d.is_boolean()) {
        errors.push(issue(
            "SCHEMA_INVALID",
            Fail,
            "directed must be boolean when supplied.",
            "/directed",
        ));
    }
    if !errors.is_empty() {
        report.check("layout", Fail, "Invalid selected Graphify layout.", errors);
        return;
    }
    report.result("layout", Pass, format!("Explicit profile {profile} matches the supplied collection layout; unknown fields preserved in original bytes."), None);
    records::project(data, revision, edge_key, options, &revisions, index, report);
    let mut known = options.code_revision;
    let mut conflict = known == Some("");
    for claim in revisions.values().filter(|claim| !claim.is_null()) {
        let next = claim.as_str();
        conflict |= next.is_none_or(str::is_empty) || known.is_some() && known != next;
        known = next;
    }
    if conflict {
        report.result("revision", Unknown, "Code revision claims conflict or are malformed; artifact revision is not code revision.", Some("REVISION_UNRESOLVED"));
    }
}
