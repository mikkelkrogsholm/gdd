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

## Python API coverage

| Python entry points | Current Rust support |
| --- | --- |
| `validate`, `exit_code` | `gdd::validation::{validate, validate_with_options}` and `Report::exit_code()`. Rust supports representation, model and readiness. Readiness includes context packages and parallel plans. The records, visualization and Graphify profiles and activity authorization still require Python. |
| `import_graphify`, `verify_graphify`, `export_graphify`, `propose_graphify` | Python-only Graphify operations. |
| `render_view`, `verify_view`, `perspectives`, `perspective_data`, `load_perspective_bundle` | Python-only HTML and perspective APIs. The [native viewer](../crates/viewer/README.md) is a separate desktop preview. |
| `prepare_workflow`, `verify_workflow`, `create_run`, `run_status`, `advance_run` | Python-only work planning and run journals. |
| `check_code`, `project_status`, `analyze_impact`, `diff_models` | Python-only code observation and project inspection APIs. |
| `gdd_validator.host` helpers `snapshot_code`, `execute_check`, `verification_records`, `append_records` | Python-only trusted host operations. A code snapshot identifies selected implementation files, while a model snapshot identifies normative GDD assets. See the [host guide](../HOST.md). |
