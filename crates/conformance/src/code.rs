//! Execute the named code conformance cases in process, using the native observer.

mod imports;
mod mapping;
mod observation;
mod project;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(Deserialize)]
struct Case {
    id: String,
    assertion: String,
    test: String,
}

struct Behavior {
    id: &'static str,
    reference: &'static str,
    parameters: &'static [&'static str],
    execute: fn(&Path, &str) -> Result<()>,
}

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

const BEHAVIORS: &[Behavior] = &[
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
struct Row {
    id: String,
    assertion: String,
    result: &'static str,
    test_instances: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    errors: Vec<String>,
}

#[derive(Serialize)]
struct Run {
    format: &'static str,
    tool: &'static str,
    runner: &'static str,
    runtime_target: Value,
    case_count: usize,
    test_instances: usize,
    passed: bool,
    cases: Vec<Row>,
}

fn execute(directory: &Path, case: Case, seen: &mut BTreeSet<String>) -> Row {
    let mut errors = Vec::new();
    let mut count = 0;
    let behavior = BEHAVIORS.iter().find(|b| b.id == case.id);
    if !seen.insert(case.id.clone()) {
        errors.push(format!("Duplicate case ID {}", case.id));
    } else if case.assertion.trim().is_empty() {
        errors.push("Missing case assertion".to_owned());
    } else if let Some(behavior) = behavior {
        if case.test != behavior.reference {
            errors.push(format!("Unrecognized test reference {}", case.test));
        } else {
            for parameter in behavior.parameters {
                count += 1;
                if let Err(error) = (behavior.execute)(directory, parameter) {
                    errors.push(format!("{parameter}: {error:#}"));
                }
            }
        }
    } else {
        errors.push(format!("Unknown code case {}", case.id));
    }
    Row {
        id: case.id,
        assertion: case.assertion,
        result: if errors.is_empty() && count > 0 {
            "PASS"
        } else {
            "FAIL"
        },
        test_instances: count,
        errors,
    }
}

fn run(directory: &Path) -> Result<Run> {
    let bytes = fs::read(directory.join("code.cases.json")).context("read code case manifest")?;
    let definitions: Vec<Case> =
        serde_json::from_slice(&bytes).context("parse code case definitions")?;
    ensure!(
        definitions.len() == BEHAVIORS.len(),
        "expected {} named code cases, found {}",
        BEHAVIORS.len(),
        definitions.len()
    );
    let mut seen = BTreeSet::new();
    let cases: Vec<_> = definitions
        .into_iter()
        .map(|case| execute(directory, case, &mut seen))
        .collect();
    Ok(Run {
        format: "gdd-code-conformance/0.1",
        tool: concat!("gdd-conformance/", env!("CARGO_PKG_VERSION")),
        runner: "rust",
        runtime_target: project::runtime_descriptor(),
        case_count: cases.len(),
        test_instances: cases.iter().map(|r| r.test_instances).sum(),
        passed: cases.iter().all(|r| r.result == "PASS"),
        cases,
    })
}

pub(super) fn command(directory: &Path, output: Option<PathBuf>) -> Result<ExitCode> {
    let run = run(directory)?;
    let matched = run.cases.iter().filter(|row| row.result == "PASS").count();
    crate::receipt::publish(&run, output, "code", matched, run.case_count)
}

#[cfg(test)]
mod tests;
