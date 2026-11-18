//! Offline validation of the original documents in a loaded GDD bundle.

mod graphify;
pub use graphify::{
    GraphifyValidationOptions, validate_captured, validate_captured_with_view,
    validate_paths_with_graphify, validate_paths_with_integrations,
};

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
    Records,
    #[serde(rename = "import.graphify")]
    #[value(name = "import.graphify")]
    ImportGraphify,
    Visualization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum Activity {
    ModelChanges,
    Implementation,
    Acceptance,
}

impl Activity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModelChanges => "model_changes",
            Self::Implementation => "implementation",
            Self::Acceptance => "acceptance",
        }
    }
}

#[derive(Debug, Default)]
pub struct ValidationOptions<'a> {
    pub profiles: &'a [Profile],
    pub scope: &'a [String],
    pub activity: Option<Activity>,
    pub host_context: Option<&'a Value>,
    pub package: Option<&'a Value>,
    pub parallel_plan: Option<&'a Value>,
    pub unsupported_extensions: &'a [String],
}

impl<'a> ValidationOptions<'a> {
    /// Validate the input envelopes and resolve their scope in declared order.
    pub fn effective_scope(&self) -> io::Result<Vec<&'a str>> {
        self.scope_with_audit(None)
    }

    fn scope_with_audit(&self, audit: Option<&JsonValue>) -> io::Result<Vec<&'a str>> {
        for (kind, input) in [
            ("host", self.host_context),
            ("package", self.package),
            ("parallel", self.parallel_plan),
        ] {
            if let Some(value) = present(input) {
                validate_input(kind, value)?;
            }
        }
        if let Some(audit) = audit {
            validate_json_input("import", audit)?;
        }
        let package = present(self.package);
        let parallel = present(self.parallel_plan);
        let mut seen = BTreeSet::new();
        let requested = self.scope.iter().map(String::as_str).collect::<Vec<_>>();
        let scope: Vec<_> = match (package, parallel) {
            (Some(_), Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Context package and parallel plan cannot be supplied together.",
                ));
            }
            (Some(package), None) => crate::model::strings(package, "scope").collect(),
            (None, Some(plan)) => crate::model::items(plan, "packages")
                .iter()
                .flat_map(|package| crate::model::strings(package, "scope"))
                .filter(|id| seen.insert(*id))
                .collect(),
            (None, None) => requested
                .iter()
                .copied()
                .filter(|id| seen.insert(*id))
                .collect(),
        };
        if (package.is_some() || parallel.is_some())
            && !requested.is_empty()
            && requested.into_iter().collect::<BTreeSet<_>>() != scope.iter().copied().collect()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Requested scope conflicts with the supplied envelope scope.",
            ));
        }
        if let Some(invalid) = scope.iter().find(|id| !valid_id(id)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Scope ID must be a GDD ID string: {invalid:?}."),
            ));
        }
        Ok(scope)
    }
}

pub(crate) fn present(value: Option<&Value>) -> Option<&Value> {
    value.filter(|v| !matches!(v, Value::Null))
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
    validate_captured(Some(bundle), None, options, None, None)
}

/// Validate once and write the scope paths for readiness diagnostics to the
/// supplied writer. The returned report has the ordinary validation schema.
pub fn validate_explained(
    bundle: &Bundle,
    options: ValidationOptions<'_>,
    output: &mut impl io::Write,
) -> io::Result<Report> {
    validate_captured(Some(bundle), None, options, None, Some(output))
}

/// Validate tool inputs before capturing the model and the advisory sources
/// needed by this assessment. Supply a writer for readiness explanations.
pub fn validate_path(
    path: &std::path::Path,
    options: ValidationOptions<'_>,
    explanation: Option<&mut dyn io::Write>,
) -> io::Result<Report> {
    validate_paths(path, None, options, explanation)
}

/// Validate a positional input, optionally alongside its primary model. A
/// distinct secondary ledger contributes records and representation checks.
/// A model-bound report uses the primary model's scope and snapshot. The
/// import.graphify profile alone can also assess positional observations.
pub fn validate_paths(
    input: &std::path::Path,
    model: Option<&std::path::Path>,
    options: ValidationOptions<'_>,
    explanation: Option<&mut dyn io::Write>,
) -> io::Result<Report> {
    validate_paths_with_graphify(input, model, options, Default::default(), explanation)
}

/// Assess a capture after the caller validated the input envelopes and resolved the scope.
pub(crate) fn assess<'a>(
    bundle: &'a Bundle,
    options: ValidationOptions<'a>,
    scope: &[&'a str],
    explanation: Option<&mut dyn io::Write>,
) -> io::Result<Report> {
    if explanation.is_some() && !options.profiles.contains(&Profile::Readiness) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--explain requires --profile readiness.",
        ));
    }
    let mut profiles = if options.profiles.is_empty() {
        vec![Profile::Model]
    } else {
        distinct(options.profiles)
    };
    if options.activity.is_some() && !profiles.contains(&Profile::Records) {
        profiles.push(Profile::Records);
    }
    let mut report = Report::new(profiles, scope.iter().copied().map(str::to_owned).collect());
    let mut normative_valid = true;
    let mut manifest_valid = bundle.manifest().is_some();
    for (index, document) in bundle.documents().iter().enumerate() {
        if bundle.declared_extension(index) {
            continue;
        }
        if let Some(check) = bundle.representation_check(index) {
            if index < bundle.primary_documents().len()
                && document.kind != DocumentKind::Records
                && check.result != CheckResult::Pass
            {
                normative_valid = false;
                if document.kind.is_manifest() {
                    manifest_valid = false;
                }
            }
            report.checks.push(check);
        }
    }
    report
        .checks
        .extend(bundle.extension_checks(options.unsupported_extensions));
    let wants_readiness = report.profiles.contains(&Profile::Readiness);
    let wants_records = report.profiles.contains(&Profile::Records);
    let wants_model = report.profiles != [Profile::Representation];
    report.checks.extend(
        bundle
            .diagnostics()
            .iter()
            .filter(|issue| !issue.semantic_only && !issue.extension_capture)
            .map(Check::from),
    );
    if normative_valid && bundle.primary_extensions_accepted() {
        report.spec_revision = bundle.snapshot().map(|snapshot| snapshot.revision.clone());
    }
    let representation_valid = report.result("model") == CheckResult::Pass;
    let mut context = None;
    if wants_model {
        if let Some(manifest) = bundle.manifest().filter(|_| manifest_valid) {
            if representation_valid {
                let (checks, assessed_scope) =
                    crate::model::check(bundle, manifest, &options, scope);
                report.checks.extend(checks);
                context = assessed_scope;
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
    if !representation_valid || bundle.manifest().is_none() {
        for (requested, id, dimension) in [
            (wants_readiness, "readiness.unavailable", "readiness"),
            (wants_records, "records.unavailable", "implementation"),
        ] {
            if requested {
                report
                    .checks
                    .push(unavailable_check(id, dimension, &report.scope));
            }
        }
        if let Some(activity) = options.activity {
            let mut check = Check::new(
                "authorization.unavailable",
                "authorization",
                CheckResult::Unresolved,
                "Valid model and revision unavailable.",
            );
            check.scope.clone_from(&report.scope);
            check.details = Some(crate::report::CheckDetails::Authorization {
                activity,
                policy_revision: None,
                authority_basis: None,
            });
            check.diagnostics.push(Diagnostic::new(
                "AUTHORIZATION_UNRESOLVED",
                check.result,
                "Authorization input incomplete.",
                None,
                "",
            ));
            report.checks.push(check);
        }
    }
    if let Some(output) = explanation {
        crate::model::explain::readiness(bundle, &report, context.as_ref(), output)?;
    }
    Ok(report)
}

pub(crate) fn unavailable_check(id: &str, dimension: &'static str, scope: &[String]) -> Check {
    let mut check = Check::new(
        id,
        dimension,
        CheckResult::Unknown,
        "A valid supported GDD model is required; no semantic PASS inferred.",
    );
    check.scope = scope.to_vec();
    check.diagnostics.push(Diagnostic::new(
        "REVISION_UNRESOLVED",
        check.result,
        "Model unavailable or representation invalid.",
        None,
        "",
    ));
    check
}

pub(crate) fn validate_input(kind: &str, input: &Value) -> io::Result<()> {
    validate_json_input(kind, &to_json(input)?)
}

fn validate_json_input(kind: &str, input: &JsonValue) -> io::Result<()> {
    static HOST: OnceLock<Validator> = OnceLock::new();
    static PACKAGE: OnceLock<Validator> = OnceLock::new();
    static PARALLEL: OnceLock<Validator> = OnceLock::new();
    static IMPORT: OnceLock<Validator> = OnceLock::new();
    let slot = match kind {
        "host" => &HOST,
        "package" => &PACKAGE,
        "parallel" => &PARALLEL,
        "import" => &IMPORT,
        _ => unreachable!(),
    };
    let validator = slot.get_or_init(|| {
        let schema: JsonValue =
            serde_json::from_str(include_str!("../schemas/validator-inputs.schema.json"))
                .expect("embedded tool schemas are JSON");
        build_schema(&schema["$defs"][kind]).expect("embedded tool schema compiles offline")
    });
    validator.validate(input).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Invalid {kind} input: {error}"),
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

pub(crate) struct EmbeddedSchema {
    source: &'static str,
    validator: OnceLock<Validator>,
}

impl EmbeddedSchema {
    pub(crate) const fn new(source: &'static str) -> Self {
        Self {
            source,
            validator: OnceLock::new(),
        }
    }

    pub(crate) fn validator(&self) -> &Validator {
        self.validator.get_or_init(|| {
            let schema = serde_json::from_str(self.source).expect("embedded schema is JSON");
            build_schema(&schema).expect("embedded schema compiles offline")
        })
    }
}

pub(crate) fn embedded_schema(kind: DocumentKind) -> Option<&'static Validator> {
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

pub(crate) fn representation_check(document: &Document) -> Option<Check> {
    embedded_schema(document.kind).map(|schema| document_check(document, schema))
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
