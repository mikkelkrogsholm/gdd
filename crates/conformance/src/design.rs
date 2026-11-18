//! Execute the design boundary, workflow packet and advisory review cases.

mod boundaries;
mod entrypoints;
mod packets;
mod review;

use crate::fixture::write_json;
use crate::named::{self, Behavior};
use anyhow::{Context, Result, ensure};
use gdd::bundle::{self, Bundle};
use gdd::input;
use gdd::report::{CheckResult, Report};
use gdd::validation::{self, Profile, ValidationOptions};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

macro_rules! design_case {
    ($module:ident::$name:ident, $parameters:expr) => {
        Behavior {
            id: concat!("case.design.", stringify!($name)),
            reference: concat!("tests/test_design_boundaries.py::test_", stringify!($name)),
            parameters: $parameters,
            execute: |directory, parameter| $module::$name(&mut Model::new(directory)?, parameter),
        }
    };
    ($module:ident::$name:ident) => {
        design_case!($module::$name, &["default"])
    };
}

const BEHAVIORS: &[Behavior] = &[
    design_case!(boundaries::complete_boundary_readiness),
    design_case!(
        boundaries::partial_boundary_sketch_and_readiness,
        &["responsibility", "rationale", "excludes"]
    ),
    design_case!(
        boundaries::blank_boundary_is_invalid,
        &["responsibility", "rationale"]
    ),
    design_case!(boundaries::explicit_empty_exclusions),
    design_case!(boundaries::unrelated_incomplete_component),
    Behavior {
        id: "case.design.legacy_semantics_preserved",
        reference: "tests/test_design_boundaries.py::test_legacy_semantics_preserved",
        parameters: &["default"],
        execute: boundaries::legacy_semantics_preserved,
    },
    design_case!(boundaries::new_fields_cannot_silently_enter_legacy_format),
    design_case!(boundaries::fragment_version_must_match),
    design_case!(boundaries::public_interface_requires_matching_owner),
    design_case!(boundaries::cross_component_private_call),
    design_case!(boundaries::internal_private_call_allowed),
    design_case!(
        boundaries::unresolved_call_valid_sketch_unready,
        &["provider_owner", "consumer_owner", "interfaces"]
    ),
    design_case!(boundaries::postconditions_required_in_read_context),
    design_case!(boundaries::unrelated_private_call_does_not_block_readiness),
    design_case!(boundaries::community_overlap_and_owner_derived_membership),
    design_case!(boundaries::community_membership_does_not_grant_access),
    design_case!(boundaries::scoped_boundary_view_declares_omitted_calls),
    design_case!(boundaries::unrelated_unresolved_call_not_in_every_community),
    design_case!(boundaries::boundary_and_contract_changes_change_revision),
    design_case!(
        boundaries::refinement_preserves_parent_boundary_context,
        &["present", "absent"]
    ),
    design_case!(packets::packet_contains_canonical_boundaries_and_review_topics),
    design_case!(
        packets::packet_omission_rejected,
        &["boundaries", "design_review"]
    ),
    design_case!(packets::boundary_change_invalidates_plan),
    design_case!(
        review::broad_principle_is_advisory,
        &["design", "architecture"]
    ),
    design_case!(review::precise_design_rule_can_still_block),
    design_case!(review::advisory_review_complete_even_with_findings),
    design_case!(
        review::review_unknown_is_explicit,
        &[
            "absent",
            "planned",
            "missing_principle",
            "unknown",
            "stale_model",
            "stale_code",
            "unsupported",
            "narrow_scope"
        ]
    ),
    design_case!(
        review::invalid_review_cannot_be_complete,
        &[
            "duplicate",
            "unknown_scope",
            "unknown_target",
            "outside_context",
            "missing_location",
            "not_applicable_findings"
        ]
    ),
    design_case!(review::invalid_model_cannot_supply_complete_review),
    design_case!(review::complete_review_does_not_replace_blocking_evidence),
    design_case!(entrypoints::new_viewer_and_cli_operations),
    Behavior {
        id: "case.design.versioned_schema_definitions_align",
        reference: "tests/test_design_boundaries.py::test_versioned_schema_definitions_align",
        parameters: &["default"],
        execute: versioned_schema_definitions_align,
    },
];

pub(super) fn command(directory: &Path, output: Option<PathBuf>) -> Result<ExitCode> {
    named::command(
        directory,
        output,
        "design",
        "gdd-design-conformance/0.3",
        BEHAVIORS,
        (),
    )
}

const RESERVE: &str = "action.reserve";
const PROVIDER: &str = "component.reservations";

struct Model<'a> {
    directory: &'a Path,
    temp: tempfile::TempDir,
    path: PathBuf,
    data: Value,
}

impl<'a> Model<'a> {
    fn new(directory: &'a Path) -> Result<Self> {
        let path = directory.join("../examples/concert-0.3.gdd.yaml");
        let mut data = read_document(&path)?;
        remove(&mut data, "record_files")?;
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("model.gdd.json");
        write_json(&path, &data)?;
        Ok(Self {
            directory,
            temp,
            path,
            data,
        })
    }

    fn save(&self) -> Result<()> {
        write_json(&self.path, &self.data)
    }

    fn bundle(&self) -> Bundle {
        bundle::load_model_only(&self.path)
    }

    fn revision(&self) -> Result<String> {
        Ok(self
            .bundle()
            .snapshot()
            .context("unresolved fixture snapshot")?
            .revision
            .clone())
    }

    fn model(&self, expected: CheckResult) -> Result<Report> {
        assess(&self.path, Profile::Model, &[], "model", expected)
    }

    fn readiness(&self, scope: &str, expected: CheckResult) -> Result<Report> {
        assess(
            &self.path,
            Profile::Readiness,
            &[scope.into()],
            "readiness",
            expected,
        )
    }

    fn edges(&mut self) -> Result<&mut Vec<Value>> {
        self.data["edges"]
            .as_array_mut()
            .context("expected concert edges")
    }
}

fn assess(
    path: &Path,
    profile: Profile,
    scope: &[String],
    dimension: &str,
    expected: CheckResult,
) -> Result<Report> {
    let report = validation::validate_path(
        path,
        ValidationOptions {
            profiles: &[profile],
            scope,
            ..Default::default()
        },
        None,
    )?;
    let actual = report.result(dimension);
    ensure!(
        actual == expected,
        "{dimension}: expected {expected}, got {actual}"
    );
    Ok(report)
}

fn diagnostic(report: &Report, code: &str) -> Result<()> {
    ensure!(
        report
            .checks
            .iter()
            .flat_map(|c| &c.diagnostics)
            .any(|d| d.code == code),
        "missing diagnostic {code}"
    );
    Ok(())
}

fn read_document(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(validation::to_json(&input::parse(&bytes)?)?)
}

fn remove(value: &mut Value, key: &str) -> Result<()> {
    value
        .as_object_mut()
        .and_then(|v| v.remove(key))
        .with_context(|| format!("missing fixture field {key}"))?;
    Ok(())
}

fn read_schema(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse JSON schema {}", path.display()))
}

fn validate_boundaries(directory: &Path, view: &Value) -> Result<()> {
    let schema = read_schema(&directory.join("../schemas/boundaries.schema.json"))?;
    validation::build_schema(&schema)?
        .validate(view)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn versioned_schema_definitions_align(directory: &Path, _: &str) -> Result<()> {
    let schemas = directory.join("../schemas");
    for suffix in ["", "-0.2"] {
        let model = read_schema(&schemas.join(format!("gdd{suffix}.schema.json")))?;
        let fragment = read_schema(&schemas.join(format!("fragment{suffix}.schema.json")))?;
        let definitions = model["$defs"]
            .as_object()
            .context("missing model definitions")?;
        ensure!(
            !definitions.is_empty() && fragment["$defs"].as_object() == Some(definitions),
            "model/fragment definitions differ for {suffix:?}"
        );
    }
    ensure!(
        fs::read(schemas.join("gdd-0.2.schema.json"))?
            == fs::read(directory.join("../archive/gdd-0.2/schemas/gdd.schema.json"))?,
        "legacy schema bytes differ"
    );
    Ok(())
}
