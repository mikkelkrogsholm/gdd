//! Offline validation of the original documents in a loaded GDD bundle.

use std::collections::BTreeSet;
use std::io;
use std::sync::OnceLock;

use clap::ValueEnum;
use jsonschema::{ValidationError, Validator};
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::bundle::{Bundle, Document, DocumentKind};
use crate::input::Value;
use crate::report::{Check, CheckResult, Diagnostic, Report};
use crate::valid_id;

pub(crate) const CORE_KINDS: &[&str] = &[
    "component",
    "entity",
    "action",
    "screen",
    "rule",
    "scenario",
    "machine",
    "community",
];
pub(crate) const CORE_RELATIONS: &[&str] = &[
    "owned_by",
    "stored_in",
    "imports",
    "calls",
    "reads",
    "creates",
    "changes",
    "constrains",
    "exercises",
    "verifies",
    "state_of",
    "navigates_to",
    "depends_on",
    "refines",
    "member_of",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Representation,
    Model,
    Readiness,
}

#[derive(Debug, Default)]
pub struct ValidationOptions<'a> {
    pub profiles: &'a [Profile],
    pub scope: &'a [String],
    pub host_context: Option<&'a Value>,
    pub unsupported_extensions: &'a [String],
}

/// Validate a loaded bundle without reading its files again.
pub fn validate(bundle: &Bundle, profile: Profile) -> Report {
    validate_with_options(
        bundle,
        ValidationOptions {
            profiles: &[profile],
            ..ValidationOptions::default()
        },
    )
    .expect("the default scope is valid")
}

pub fn validate_with_options(
    bundle: &Bundle,
    options: ValidationOptions<'_>,
) -> io::Result<Report> {
    let profiles = if options.profiles.is_empty() {
        vec![Profile::Model]
    } else {
        distinct(options.profiles)
    };
    if let Some(invalid) = options.scope.iter().find(|id| !valid_id(id)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Scope ID must be a GDD ID string: {invalid:?}."),
        ));
    }
    if let Some(host) = options.host_context {
        validate_host(host)?;
    }
    let mut report = Report::new(profiles, distinct(options.scope));
    let mut normative_valid = true;
    let mut manifest_valid = bundle.manifest().is_some();
    for document in bundle.documents() {
        if let Some(schema) = embedded_schema(document.kind) {
            let check = document_check(document, schema);
            if document.kind != DocumentKind::Records && check.result != CheckResult::Pass {
                normative_valid = false;
                if document.kind.is_manifest() {
                    manifest_valid = false;
                }
            }
            report.checks.push(check);
        }
    }
    let wants_readiness = report.profiles.contains(&Profile::Readiness);
    let wants_model = wants_readiness || report.profiles.contains(&Profile::Model);
    report.checks.extend(
        bundle
            .diagnostics()
            .iter()
            .filter(|issue| wants_model || !issue.semantic_only)
            .map(Check::from),
    );
    if normative_valid {
        report.spec_revision = bundle.snapshot().map(|snapshot| snapshot.revision.clone());
    }
    for (namespace, document) in bundle.extensions() {
        if options
            .unsupported_extensions
            .iter()
            .any(|name| name == namespace)
        {
            let reason = "Extension explicitly unsupported by this consumer.";
            let mut check = Check::new(
                "check.extension_unsupported",
                "model",
                CheckResult::Unknown,
                reason,
            );
            check.diagnostics.push(Diagnostic::new(
                "EXTENSION_UNSUPPORTED",
                check.result,
                reason,
                Some(&document.asset.path),
                "",
            ));
            report.checks.push(check);
        }
    }

    let representation_valid = report.result("model") == CheckResult::Pass;
    if wants_model {
        if let Some(manifest) = bundle.manifest().filter(|_| manifest_valid) {
            if representation_valid {
                report.checks.extend(crate::model::check(
                    bundle,
                    manifest,
                    options.unsupported_extensions,
                    wants_readiness.then_some((&report.scope, options.host_context)),
                ));
            }
        } else {
            let reason = "Semantic model validation requires a supported GDD 0.2 or 0.3 model manifest; schema validation alone is representation-only.";
            let mut check = Check::new("model.unavailable", "model", CheckResult::Unknown, reason);
            check.scope.clone_from(&report.scope);
            check.diagnostics.push(Diagnostic::new(
                "FORMAT_UNSUPPORTED",
                check.result,
                "No model semantics for this artifact without its model manifest.",
                None,
                "",
            ));
            report.checks.push(check);
        }
    }
    if wants_readiness && (!representation_valid || bundle.manifest().is_none()) {
        let mut check = Check::new(
            "readiness.unavailable",
            "readiness",
            CheckResult::Unknown,
            "A valid supported GDD model is required; no semantic PASS inferred.",
        );
        check.scope.clone_from(&report.scope);
        check.diagnostics.push(Diagnostic::new(
            "REVISION_UNRESOLVED",
            check.result,
            "Model unavailable or representation invalid.",
            None,
            "",
        ));
        report.checks.push(check);
    }
    Ok(report)
}

fn validate_host(host: &Value) -> io::Result<()> {
    static HOST: OnceLock<Validator> = OnceLock::new();
    let validator = HOST.get_or_init(|| {
        let schema: JsonValue =
            serde_json::from_str(include_str!("../schemas/validator-inputs.schema.json"))
                .expect("embedded tool schemas are JSON");
        build_schema(&schema["$defs"]["host"]).expect("embedded host schema compiles offline")
    });
    validator.validate(&to_json(host)?).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Invalid host input: {error}"),
        )
    })
}

fn distinct<T: Clone + Ord>(items: &[T]) -> Vec<T> {
    let mut seen = BTreeSet::new();
    items
        .iter()
        .filter(|item| seen.insert(*item))
        .cloned()
        .collect()
}

/// Compile a local Draft 2020-12 schema and enforce its format assertions.
pub fn build_schema(schema: &JsonValue) -> Result<Validator, Box<ValidationError<'static>>> {
    jsonschema::draft202012::options()
        .offline()
        .should_validate_formats(true)
        .build(schema)
        .map_err(Box::new)
}

/// Adapt the strict parser's values while preserving the precision of each number.
pub fn to_json(value: &Value) -> serde_json::Result<JsonValue> {
    match value {
        Value::Null => Ok(JsonValue::Null),
        Value::Boolean(value) => Ok(JsonValue::Bool(*value)),
        Value::Number(value) => value.parse().map(JsonValue::Number),
        Value::String(value) => Ok(JsonValue::String(value.clone())),
        Value::Array(values) => values
            .iter()
            .map(to_json)
            .collect::<Result<_, _>>()
            .map(JsonValue::Array),
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), to_json(value)?)))
            .collect::<Result<_, _>>()
            .map(JsonValue::Object),
    }
}

struct EmbeddedSchema {
    source: &'static str,
    validator: OnceLock<Validator>,
}

impl EmbeddedSchema {
    const fn new(source: &'static str) -> Self {
        Self {
            source,
            validator: OnceLock::new(),
        }
    }

    fn validator(&self) -> &Validator {
        self.validator.get_or_init(|| {
            let schema = serde_json::from_str(self.source).expect("embedded schema is JSON");
            build_schema(&schema).expect("embedded schema compiles offline")
        })
    }
}

fn embedded_schema(kind: DocumentKind) -> Option<&'static Validator> {
    static MANIFEST_02: EmbeddedSchema =
        EmbeddedSchema::new(include_str!("../schemas/gdd-0.2.schema.json"));
    static MANIFEST_03: EmbeddedSchema =
        EmbeddedSchema::new(include_str!("../schemas/gdd.schema.json"));
    static FRAGMENT_02: EmbeddedSchema =
        EmbeddedSchema::new(include_str!("../schemas/fragment-0.2.schema.json"));
    static FRAGMENT_03: EmbeddedSchema =
        EmbeddedSchema::new(include_str!("../schemas/fragment.schema.json"));
    static EXTENSION: EmbeddedSchema =
        EmbeddedSchema::new(include_str!("../schemas/extension.schema.json"));
    static RECORDS: EmbeddedSchema =
        EmbeddedSchema::new(include_str!("../schemas/records.schema.json"));
    static REPORT: EmbeddedSchema =
        EmbeddedSchema::new(include_str!("../schemas/report.schema.json"));
    static SNAPSHOT: EmbeddedSchema =
        EmbeddedSchema::new(include_str!("../schemas/snapshot.schema.json"));

    let schema = match kind {
        DocumentKind::Manifest02 => &MANIFEST_02,
        DocumentKind::Manifest03 => &MANIFEST_03,
        DocumentKind::Fragment02 => &FRAGMENT_02,
        DocumentKind::Fragment03 => &FRAGMENT_03,
        DocumentKind::Extension => &EXTENSION,
        DocumentKind::Records => &RECORDS,
        DocumentKind::Report => &REPORT,
        DocumentKind::Snapshot => &SNAPSHOT,
        DocumentKind::Other => return None,
    };
    Some(schema.validator())
}

fn document_check(document: &Document, validator: &Validator) -> Check {
    let mut check = Check::new(
        "representation.schema",
        "model",
        CheckResult::Pass,
        "Local JSON Schema validation.",
    );
    let file = Some(document.asset.path.as_str());
    let value = match to_json(&document.value) {
        Ok(value) => value,
        Err(error) => {
            check.result = CheckResult::Unknown;
            check.diagnostics.push(Diagnostic::new(
                "FORMAT_UNSUPPORTED",
                check.result,
                error.to_string(),
                file,
                "",
            ));
            return check;
        }
    };
    check
        .diagnostics
        .extend(validator.iter_errors(&value).map(|error| {
            Diagnostic::new(
                "SCHEMA_INVALID",
                CheckResult::Fail,
                error.to_string(),
                file,
                error.instance_path().to_string(),
            )
        }));
    if document.kind == DocumentKind::Extension {
        for registry in ["node_kinds", "relations"] {
            if let Some(entries) = value[registry].as_object() {
                for name in entries.keys() {
                    if CORE_KINDS.contains(&name.as_str())
                        || CORE_RELATIONS.contains(&name.as_str())
                    {
                        check.diagnostics.push(Diagnostic::new(
                            "EXTENSION_OVERRIDE",
                            CheckResult::Fail,
                            format!("Core name cannot be redefined: {name}."),
                            file,
                            format!("/{registry}/{name}"),
                        ));
                    }
                }
            }
        }
    }
    if !check.diagnostics.is_empty() {
        check.result = CheckResult::Fail;
        check
            .diagnostics
            .sort_by(|a, b| a.location.pointer.cmp(&b.location.pointer));
    }
    check
}
