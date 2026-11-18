# Migrate a Python integration to Rust

The Rust `gdd` crate will replace the public `gdd_validator` Python API after
the port is complete. The released Python v0.8.0 package remains available for
existing integrations and historical replay. For an integration that needs an
operation still listed as Python-only below, keep that operation on the pinned
Python release until its Rust equivalent is available.

## Validate a scope

From a checkout containing the root [Cargo manifest](../Cargo.toml), create a
small consumer project. Cargo will record the local path to the unpublished crate.

```sh
GDD_CHECKOUT="$(pwd)"
GDD_DEMO="$(mktemp -d)/readiness-demo"
cargo new --bin "$GDD_DEMO"
cd "$GDD_DEMO"
cargo add --path "$GDD_CHECKOUT"
```

Replace the consumer's `src/main.rs` with the following program.

```rust
use std::{env, io, path::PathBuf, process::ExitCode};

use gdd::bundle;
use gdd::context::ContextGraph;
use gdd::report::CheckResult;
use gdd::validation::{validate_with_options, Profile, ValidationOptions};

fn main() -> io::Result<ExitCode> {
    let path = PathBuf::from(env::args_os().nth(1).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "Supply a model path")
    })?);
    let scope = vec![String::from("action.reserve")];
    let bundle = bundle::load_for_readiness(&path, &scope);
    let report = validate_with_options(&bundle, ValidationOptions {
        profiles: &[Profile::Readiness],
        scope: &scope,
        ..Default::default()
    })?;

    println!("Readiness: {:?}", report.result("readiness"));
    for check in &report.checks {
        for issue in &check.diagnostics {
            eprintln!("{} {}#{} {}", issue.code,
                issue.location.file.as_deref().unwrap_or("<unknown>"),
                issue.location.pointer, issue.message);
        }
    }

    if report.result("model") == CheckResult::Pass {
        let graph = ContextGraph::new(&bundle);
        let context = graph.scope(scope.iter().map(String::as_str));
        let target = "entity.concert";
        if context.contains(target) {
            if let Some(path) = context.path_to(target) {
                for step in path {
                    println!("{} -> {} ({})", step.from.unwrap_or("<global>"),
                        step.to, step.edge.unwrap_or(step.derivation));
                }
            }
            if let Some(location) = bundle.location(target) {
                println!("Defined at {}#{}", location.file, location.pointer);
            }
        }
    }
    Ok(ExitCode::from(report.exit_code()))
}
```

```sh
cargo run -- "$GDD_CHECKOUT/examples/concert-0.3.gdd.yaml"
```

For this example, the reservation action passes readiness and the witness shows
why the concert entity belongs to its read context. The location identifies
the file and JSON Pointer where that entity is defined. A scoped capture
includes advisory contracts reachable from the selected scope. Use the same
scope for validation, or capture once with `bundle::load(&path)` to assess
multiple scopes. Validation uses the captured bytes.

An invalid option can return an I/O error. A completed assessment returns a
report, including when the inputs are missing or the checks fail. Read
`report.result("readiness")` for the structural verdict and `report.exit_code()`
for the combined outcome across checks. Exit 0 means PASS, 1 means a known
failure, and 2 means unresolved with no known failure. Readiness alone
establishes neither authorization nor implementation evidence.

To see why a readiness diagnostic falls within the requested scope, run the
native CLI with `--explain`. This example exits 1 because the starter is a
draft. The JSON report goes to stdout. Stderr identifies the owning file and
JSON Pointer and follows the declared ownership edge from `action.main` to
`component.main`.

```sh
cargo run --manifest-path "$GDD_CHECKOUT/Cargo.toml" --bin gdd -- \
  validate "$GDD_CHECKOUT/templates/project/gdd.json" --profile readiness \
  --scope action.main --explain > readiness.json
```

### Context packages and parallel plans

Use `--package context.json` with `--profile readiness` to check a candidate
package against the model's required nodes, edges, sources, questions and
extensions. The same invocation assesses structural readiness for the package
scope. Use `--parallel-plan plan.json` to assess declared write ownership,
workspace independence, locked contracts and integration checks across packages.
Each command takes one envelope. An explicit `--scope` must agree with it.

Rust callers pass a parsed `gdd::input::Value` through `ValidationOptions::package`
or `ValidationOptions::parallel_plan`. The `validation::validate_path` function
captures the required inputs and returns the report. A full bundle also supports
either assessment through `validate_with_options`. Parallel-plan evaluation
resolves workspace paths relative to the model directory and checks the current
model and record declarations.

### Implementation evidence and authorization

Use `--profile records` to assess the recorded checks required by the scoped
blocking rules. Supply host assertions with `--host-context`. For example, the
archived quote demo includes a prepared context for its recorded implementation.

```sh
cargo run --manifest-path "$GDD_CHECKOUT/Cargo.toml" --bin gdd -- \
  validate "$GDD_CHECKOUT/examples/agent-native/model.gdd.json" --profile records \
  --scope action.quote --host-context "$GDD_CHECKOUT/examples/agent-native/host.acceptance.json"
```

The validator reads the captured ledger and compares check digests, spec and
code revisions, bindings, execution provenance and required environment. The
report records the evidence supporting each applicable PASS or FAIL. An empty
required check set produces UNKNOWN.

Use `--activity model_changes`, `--activity implementation` or
`--activity acceptance` to evaluate the host's authorization for that scope.
The activity adds the records profile and returns GRANTED, DENIED or UNRESOLVED
in the authorization dimension. In Rust, set `ValidationOptions::activity` to
the corresponding `gdd::validation::Activity`. Both assessments read the model
and its declared ledgers from the same bundle capture.

To assess a separate ledger, run `gdd validate observations.json --model
project/gdd.json --profile records`. Supply the usual scope and host context.
The ledger can live outside the primary project. When the primary already
declares the same resolved file, the validator reads it once. The primary
model supplies the project identity and snapshot revision. A project mismatch
or duplicate record ID appears in the report.

Rust callers can capture both inputs with
`bundle::load_with_records(&model, &ledger)` and pass that bundle to
`validate_with_options`. For a single scoped assessment, use
`validation::validate_paths(&ledger, Some(&model), options, None)`.

## Project status

Run `gdd status project/gdd.json --scope action.main --format text` for a
scoped overview of the model, readiness and open questions. JSON is the
default format. It includes the underlying reports and JSON Pointers from
each assessment and suggested action to its source. The first 12 actions are
shown. The exit code includes failures and unresolved integration checks.

Authorization and implementation appear as UNASSESSED until the caller
supplies `--host-context`, `--implementation` or `--code-revision`.
An explicit empty host context requests both assessments. Select the
authorization activity with `--activity`. Status with a workflow journal
(`--run`) is available in the Python CLI.

Rust callers use `gdd::status::project_status(&path, options)`, or
`project_status_from_bundle(&bundle, options)` to reuse captured model,
ledger and contract bytes. Read `status.assessments` for the individual
verdicts and `status.exit_code` for the combined outcome. The
`status_text(&status)` function renders the existing result.

## Snapshots, context and project creation

For a model-only assessment, use `bundle::load_model_only(&path)` and
`gdd::validation::validate(&bundle, Profile::Model)`. After a passing model
assessment, `bundle.snapshot()` gives the optional `gdd-snapshot/0.2` with
its normative asset paths, SHA-256 hashes and deterministic spec revision.
`gdd::snapshot_revision` computes the same revision from caller-supplied
normalized path and hash pairs. To export the revision-bound read context,
call `gdd::context::context_manifest(&bundle, &scope)` on a validated model.

To create a draft, call `gdd::init_project(&new_dir, project_id, title)`.
Choose an unused name under an existing parent. The returned `InitReceipt`
identifies the manifest and spec revision. The starter contains an open scope
question, so its model passes and its structural readiness fails.

## Numeric validation differences

The Rust parser preserves finite numeric literals such as `1e-4000` and
compares model bounds using exact arithmetic. A number field with minimum
`1e-4000` and maximum `2e-4000` passes this check. With maximum `0`, the
same minimum produces `TYPE_CONSTRAINT_INVALID`. The Python validator reports
`FORMAT_UNSUPPORTED` and UNKNOWN for these inputs because conversion to its
numeric representation would lose the value. Those Python capacity tests
intentionally have different native results. Rust hashes the original
normative file bytes to compute the spec revision.

Unquoted non-finite literals such as `NaN` and `Infinity` are invalid input.
The native model interval comparison has a 10,000-digit budget, including
zeros introduced by the exponent. A comparison that exceeds its capacity
returns `FORMAT_UNSUPPORTED` and UNKNOWN. The parser also limits JSON and
YAML nesting to 256 containers.

## Python API coverage

| Python entry points | Current Rust support |
| --- | --- |
| `validate`, `exit_code` | `gdd::validation::{validate, validate_with_options}` and `Report::exit_code()`. Rust supports representation, model, readiness and records, including context packages, parallel plans and activity authorization. Separate records inputs are supported via `--model` and `validate_paths`. Visualization and Graphify validation still require Python. |
| `import_graphify`, `verify_graphify`, `export_graphify`, `propose_graphify` | Python-only Graphify operations. |
| `render_view`, `verify_view`, `perspectives`, `perspective_data`, `load_perspective_bundle` | Python-only HTML and perspective APIs. The [native viewer](../crates/viewer/README.md) is a separate desktop preview. |
| `prepare_workflow`, `verify_workflow`, `create_run`, `run_status`, `advance_run` | Python-only work planning and run journals. |
| `project_status` | `gdd::status::{project_status, project_status_from_bundle, status_text}` for assessments without a supplied workflow run. |
| `check_code`, `analyze_impact`, `diff_models` | Python-only code observation and project inspection APIs. |
| `gdd_validator.host` helpers `snapshot_code`, `execute_check`, `verification_records`, `append_records` | Python-only trusted host operations. A code snapshot identifies selected implementation files, while a model snapshot identifies normative GDD assets. See the [host guide](../HOST.md). |
