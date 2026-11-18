//! Execute the revision-bound packet and local workflow protocol catalog.

mod concert;
mod demo;
mod model;
mod probe;
mod protocol;

use crate::fixture::write_json;
use crate::named::{self, Behavior};
use anyhow::{Context, Result, ensure};
use gdd::{
    bundle,
    input::{self, Value as Input},
    report::CheckResult,
    validation::{self, Profile, ValidationOptions},
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

macro_rules! case {
    ($file:ident::$test:ident, $module:ident::$id:ident $(, $parameters:expr)?) => {
        Behavior {
            id: concat!("workflow.", stringify!($id)),
            reference: concat!("tests/test_workflow_", stringify!($file), ".py::test_", stringify!($test)),
            parameters: case!(@parameters $($parameters)?),
            execute: $module::$id,
        }
    };
    (@parameters) => { &["default"] };
    (@parameters $parameters:expr) => { $parameters };
}

const BEHAVIORS: &[Behavior] = &[
    case!(
        model::scope_is_not_closure_and_complete_contract_payloads_are_preserved,
        model::scope
    ),
    case!(
        model::preparation_is_deterministic_and_never_rewrites_inputs,
        model::determinism
    ),
    case!(
        model::raw_model_byte_change_invalidates_the_old_plan,
        model::model_drift
    ),
    case!(
        model::changed_pinned_contract_asset_invalidates_old_plan,
        model::asset_drift
    ),
    case!(
        model::tampered_packet_contract_cannot_verify_against_unchanged_model,
        model::packet_tamper
    ),
    case!(
        model::parallel_pass_cannot_hide_missing_structural_readiness,
        model::parallel_readiness
    ),
    case!(
        model::parallel_ownership_conflicts_remain_known_failures,
        model::ownership,
        &["same_file", "directory", "same_workspace"]
    ),
    case!(
        model::explicit_serialization_can_resolve_declared_write_overlap,
        model::serialization
    ),
    case!(
        model::calls_and_depends_on_do_not_invent_code_execution_order,
        model::graph_dependencies
    ),
    case!(
        adversary::falsy_nonobject_host_is_rejected,
        protocol::host_shape,
        &[
            "prepare []",
            "prepare false",
            "prepare \"\"",
            "prepare 0",
            "verify []",
            "verify false",
            "verify \"\"",
            "verify 0",
            "create []",
            "create false",
            "create \"\"",
            "create 0",
            "status []",
            "status false",
            "status \"\"",
            "status 0",
            "event []",
            "event false",
            "event \"\"",
            "event 0"
        ]
    ),
    case!(
        protocol::create_rejects_invalid_packet_before_publication,
        protocol::publication,
        &["missing-packet-fields", "altered-slice", "stale-model"]
    ),
    case!(
        protocol::semantically_dangling_model_is_controlled_before_slice_derivation,
        protocol::unresolved_model,
        &["edge", "type"]
    ),
    case!(
        protocol::unrelated_records_preserve_scoped_dispatch,
        protocol::historical_records,
        &[
            "historical single",
            "historical multi",
            "unused-current-invalid single",
            "unused-current-invalid multi",
            "none single",
            "none multi"
        ]
    ),
    case!(
        protocol::valid_unready_draft_can_be_created_without_dispatch,
        protocol::draft
    ),
    case!(
        protocol::current_acceptance_requires_separate_bound_gates,
        protocol::current_acceptance,
        &[
            "valid",
            "no-evidence",
            "untrusted",
            "wrong-code",
            "wrong-implementation",
            "wrong-activity",
            "denied",
            "check-selection",
            "hash-mismatch",
            "acceptance-question"
        ]
    ),
    case!(
        protocol::integration_has_fresh_authority_code_and_scope_guards,
        protocol::integration,
        &[
            "valid",
            "denied",
            "wrong-code",
            "wrong-implementation",
            "wrong-base",
            "out-of-scope"
        ]
    ),
    case!(
        protocol::idempotency_cancellation_and_attempt_generation,
        protocol::resume
    ),
    case!(
        protocol::compare_and_publish_has_one_concurrent_winner,
        protocol::concurrency
    ),
    case!(
        protocol::cancel_remains_available_when_model_is_unavailable,
        protocol::cancel_unavailable
    ),
    case!(
        demo::archived_native_results_replay_red_green_and_current_acceptance,
        demo::native_demo
    ),
];

pub(super) fn command(directory: &Path, output: Option<PathBuf>) -> Result<ExitCode> {
    named::command(
        directory,
        output,
        "workflow",
        "gdd-workflow-conformance/0.1",
        BEHAVIORS,
        json!({
            "demo_implementation": "implementation.quote_demo.rust",
            "demo_inputs": "Rust reconstruction of the archived Python repairs"
        }),
    )
}

fn read_document(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(validation::to_json(&input::parse(&bytes)?)?)
}

fn revision(path: &Path) -> Result<String> {
    Ok(bundle::load_model_only(path)
        .snapshot()
        .context("missing model snapshot")?
        .revision
        .clone())
}
