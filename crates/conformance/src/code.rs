//! Execute the named code conformance cases in process, using the native observer.

mod imports;
mod mapping;
mod observation;
mod project;

use crate::named::{self, Behavior};
use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

// Most manifest IDs and Python test references share the native function name.
macro_rules! package_case {
    ($module:ident::$name:ident, $parameters:expr) => {
        Behavior {
            id: concat!("case.code.", stringify!($name)),
            reference: concat!("tests/test_code_check.py::test_", stringify!($name)),
            parameters: $parameters,
            execute: $module::$name,
        }
    };
    ($module:ident::$name:ident) => {
        package_case!($module::$name, &["default"])
    };
}

pub(super) const BEHAVIORS: &[Behavior] = &[
    package_case!(observation::independent_package_clean_injected_repaired),
    package_case!(observation::copied_concert_clean_injected_repaired_missing),
    package_case!(
        observation::direct_and_shortest_transitive_paths,
        &["direct", "transitive"]
    ),
    package_case!(observation::self_import_is_a_real_cycle),
    package_case!(
        mapping::unresolved_bindings_prevent_pass,
        &[
            "missing",
            "ambiguous",
            "stale",
            "other-implementation",
            "selector"
        ]
    ),
    package_case!(mapping::unrelated_historical_binding_does_not_block_current_mapping),
    package_case!(
        mapping::supported_resource_expansion,
        &["pkg/", "pkg/*.py", "pkg/**/front.py"]
    ),
    package_case!(
        mapping::unsafe_binding_resource_is_unknown,
        &[
            "../pkg/front.py",
            "/tmp/front.py",
            "pkg\\front.py",
            "pkg/./front.py"
        ]
    ),
    package_case!(observation::excluded_endpoint_is_explicit_and_unresolved),
    package_case!(observation::source_symlink_is_not_followed),
    package_case!(
        observation::source_uncertainty_is_unknown,
        &["syntax", "dynamic", "absent"]
    ),
    package_case!(observation::unbound_import_into_bound_component_is_unknown),
    package_case!(observation::witnessed_violation_wins_over_unrelated_parse_uncertainty),
    package_case!(observation::observation_never_executes_project_code),
    package_case!(
        mapping::unsupported_selection_has_no_false_pass,
        &["profile", "scope", "constraint"]
    ),
    package_case!(mapping::checked_resource_revision_must_match_current_bytes),
    package_case!(observation::scoped_transitive_rule_retains_intermediate_inventory),
    package_case!(
        mapping::noncomponent_binding_requires_unique_explicit_owner,
        &["unique", "ambiguous"]
    ),
    package_case!(
        mapping::additional_records_cannot_override_declared_mapping,
        &["wrong-project", "duplicate-record"]
    ),
    package_case!(observation::successful_observation_is_deterministic_and_preserves_inputs),
    package_case!(observation::transitive_witness_chooses_shortest_available_path),
    package_case!(mapping::no_selected_constraints_cannot_produce_vacuous_pass),
    package_case!(observation::fixed_infrastructure_is_excluded_from_source_inventory),
    package_case!(mapping::already_declared_records_are_not_loaded_twice),
    Behavior {
        id: "code.frozen-precedence",
        reference: "tests/test_python_imports.py::test_frozen_finder_prevents_false_source_root_collision_edge",
        parameters: &project::FROZEN,
        execute: imports::frozen_precedence,
    },
    Behavior {
        id: "code.excluded-import",
        reference: "tests/test_code_check_cli.py::test_excluding_a_known_local_import_cannot_turn_unknown_into_pass",
        parameters: &["default"],
        execute: imports::excluded_import,
    },
    Behavior {
        id: "code.inventory-drift",
        reference: "tests/test_code_check_cli.py::test_source_inventory_drift_cannot_establish_a_witness",
        parameters: &["default"],
        execute: imports::inventory_drift,
    },
    Behavior {
        id: "code.output-schema",
        reference: "tests/test_code_check_cli.py::test_observations_follow_their_own_schema",
        parameters: &["clean", "violation", "relative", "syntax"],
        execute: imports::output_schema,
    },
];

#[derive(Serialize)]
struct CodeProfile {
    runtime_target: Value,
}

pub(super) fn command(directory: &Path, output: Option<PathBuf>) -> Result<ExitCode> {
    named::command(
        directory,
        output,
        "code",
        "gdd-code-conformance/0.1",
        BEHAVIORS,
        CodeProfile {
            runtime_target: project::runtime_descriptor(),
        },
    )
}

#[cfg(test)]
mod tests;
