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
multiple scopes. Validation uses the captured bytes. Each extension declaration
reports its read and schema result before checking the digest and identity.
An unreadable or rejected definition also reports `EXTENSION_UNSUPPORTED`.

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

## Boundaries and advisory design review

Run `gdd boundaries examples/concert-0.3.gdd.yaml` for the components,
public contracts and community calls in a GDD 0.3 snapshot. A missing owner
or interface appears as unresolved. Scoped library views list omitted calls
and components so a consumer can distinguish a partial view from the whole model.

Use `gdd design-review MODEL --review REVIEW --implementation ID
--code-revision REV --scope NODE` to assess a supplied review. Repeat
`--scope` for additional nodes. The result records findings, review identity
and reasons for COMPLETE, INVALID or UNKNOWN, with exit codes 0, 1 and 2.
A complete advisory review can contain findings. Execution evidence and
acceptance remain separate assessments. Both commands accept `--output`
with an unused destination.

In Rust, `gdd::projection::load_projection_bundle(&path)` checks the input
representation, model semantics and revision. The projection can use a model
with historical ledger records. Call `gdd::boundaries::boundary_view(&bundle,
scope)` with `None` for the whole model or `Some(&[])` for an empty view.
`gdd::boundaries::design_checklist(&bundle, &scope)` returns the four advisory
questions and matching rules. Use `gdd::design_review::review_design(&bundle,
review, implementation, code_revision, expected_scope)` to assess the same
capture. An explicitly empty expected review scope is invalid.

## Inspect journey, component and data perspectives

```sh
cargo run --bin gdd -- perspectives examples/product/greenfield.gdd.json
cargo run --bin gdd -- perspectives examples/product/partial.gdd.json --output partial-perspectives.json
```

The native command emits the [advisory perspectives envelope](PERSPECTIVES.md)
for GDD 0.2 and 0.3. Each view contains canonical records and their direct
neighbors. Derived component–data links identify the edges that witness them.
The product vocabulary requires the exact shipped extension bytes. A valid
sketch can have gaps. The CLI returns 1 for invalid input and 2 for unresolved
input, with diagnostics on stderr. Supplying `--output` publishes the complete
JSON document to a new file.

Rust callers can project a path with `gdd::perspectives::perspectives(path)`,
or admit one capture and reuse its snapshot:

```rust
use std::path::Path;
use gdd::perspectives::{load_perspective_bundle, perspective_data};
use gdd::projection::ProjectionError;

fn inspect(path: &Path) -> Result<(), ProjectionError> {
    let captured = load_perspective_bundle(path)?;
    let data = perspective_data(&captured)?;
    println!("{}", data["views"]["journey"]["primary_node_ids"]);
    Ok(())
}
```

To admit an existing `gdd::bundle::Bundle`, use
`gdd::perspectives::PerspectiveBundle::try_from(bundle)`. Validation uses the
captured model and extension bytes. The wrapper exposes the shared model via
`bundle()`. Each call to `perspective_data` returns an owned `serde_json::Value`
that consumers can edit independently, even after the source files are removed.

## Workplans and packet verification

From the repository root, prepare a plan and verify the saved artifact against
the current model and selected checks. Both commands accept an optional
`--host-context` file.

```sh
cargo run --locked --bin gdd -- workflow plan examples/agent-native/model.gdd.json \
  --request examples/agent-native/workflow.request.json --output plan.json
cargo run --locked --bin gdd -- workflow verify examples/agent-native/model.gdd.json --plan plan.json
```

The first command prints the artifact and its assessment. Its optional output
file contains the workplan alone. Verification re-derives the complete plan,
including package context, source locations, selected check declarations and
GDD 0.3 design boundaries. It reports `workflow.packet_integrity` alongside
structural readiness and declared ownership. Both commands require a new
destination when `--output` is supplied. Verification writes its report there.

Rust callers use `gdd::workflow::{prepare_workflow, verify_workflow}` with a
model path, a parsed request or plan, and optional host context. The companion
`prepare_workflow_from_bundle` and `verify_workflow_from_bundle` reuse captured
model, ledger and contract bytes. Use `gdd::input::Value::from(&result.artifact)`
to pass a prepared artifact directly to verification.

## Read a workflow run

Replay a run journal and assess its current acceptance against the captured
model, operational ledger, contracts and supplied host context.

```sh
cargo run --locked --bin gdd -- workflow status examples/agent-native/model.gdd.json \
  --run examples/agent-native/native-run --host-context examples/agent-native/host.acceptance.json
```

`state` describes the published event history. `acceptance_current` also
requires current scoped readiness, the frozen check selection, matching code
identity, evidence, authorization and resolution of any acceptance blockers.
The command reads the manifest and contiguous numbered receipts and verifies
their hashes, idempotency keys, transitions and changed-file ownership.

Rust callers use `gdd::workflow_run::run_status(&model_path, &run_dir, host)`.
Call `load_journal(&run_dir)` and `run_status_from_bundle(&bundle, &journal, host)`
to reuse both captures.

## Publish a workflow run and its events

Create a run at a new directory under an existing parent. For the archived
agent-native example, the following command writes the initial status to a
new JSON file and creates the journal in the same temporary directory.

```sh
GDD_QUEUE_DIR="$(mktemp -d)"
cargo run --locked --bin gdd -- workflow create examples/agent-native/model.gdd.json \
  --plan examples/agent-native/workflow.plan.json --run "$GDD_QUEUE_DIR/run" \
  --output "$GDD_QUEUE_DIR/created.json"
```

The host supplies each event with an `idempotency_key` and the
`expected_revision` returned by create or status. To publish a dispatch
from a host-prepared `dispatch.json`, use the following command.

```sh
cargo run --locked --bin gdd -- workflow event examples/agent-native/model.gdd.json \
  --run "$GDD_QUEUE_DIR/run" --event dispatch.json \
  --host-context examples/agent-native/host.implementation.json
```

Dispatch and resume check current readiness, implementation authority and the
current code identity. Integration also requires the host to attest the
reviewed new code revision. Acceptance checks the selected evidence, current
authority and blockers. A successful dispatch returns the package for the host
to deliver. Retrying the identical event returns its recorded receipt and the
current status. A changed event requires a fresh key and the current head.

The CLI returns the gate report for a blocked event and appends a receipt
only for an applied transition. Applied events and idempotent replays exit
with code 0. Create can publish a current, unready draft and return its
nonzero assessment exit code. It rejects a stale or altered plan. Create and
event write their result to stdout by default. With `--output`, an existing
destination is refused before processing the run or event. Competing writers
publish to each journal name using an exclusive same-filesystem rename.

The parser accepts up to 256 nested containers. Plan generation can write a
derived workplan beyond that limit. Verifying the saved file then returns
exit code 2. If the plan fits but its run manifest exceeds the limit, create
publishes the run directory and then returns 2. Reading its status also
returns 2.

Rust callers use `gdd::workflow_run::create_run(&model_path, &plan, &run_dir,
host)` and `advance_run(&model_path, &run_dir, &event, host)`. Inspect
the returned `applied`, `replayed`, `receipt` and `artifact` fields, or use
`exit_code()` for CLI-compatible event status.

## Project status

Run `gdd status project/gdd.json --scope action.main --format text` for a
scoped overview of the model, readiness and open questions. JSON is the
default format. It includes the underlying reports and JSON Pointers from
each assessment and suggested action to its source. The first 12 actions are
shown. The exit code includes failures and unresolved integration checks.

Authorization and implementation appear as UNASSESSED until the caller
supplies `--host-context`, `--implementation` or `--code-revision`.
An explicit empty host context requests both assessments. Select the
authorization activity with `--activity`. Add `--run DIR` to include the
journal status in JSON or text. An unusable journal adds a `workflow_error`
and contributes to the exit code alongside the independent model assessments.

Rust callers use `gdd::status::project_status(&path, options)`, or
`project_status_from_bundle(&bundle, options)` to reuse captured model,
ledger and contract bytes. Read `status.assessments` for the individual
verdicts and `status.exit_code` for the combined outcome. The
`status_text(&status)` function renders the existing result. For an explicit
run, use `project_status_with_run(&path, &directory, options)` or
`project_status_with_run_from_bundle(&bundle, &directory, options)`. The
path-based operation captures contracts for the project and journal scopes
together.

## Declared change impact

Run `gdd impact docs/tutorial/refined.gdd.json --scope action.add --format text`
to see the declared consumers of a changed node and their supporting read
context. Repeat `--scope` to inspect several seeds. Each item includes its
directional witness. JSON output includes the model revision, scoped GDD 0.3
boundaries and current check plans or bindings to inspect. A model may leave
runtime callers and unlisted dependencies unassessed even when this declared
coverage is complete.

Use `--implementation ID` to filter applicable records. The report names
omitted historical, invalid, retired or implementation-specific records.
Explicitly incomplete dependency declarations produce partial coverage and
exit code 2. Invalid scope or model data produces exit code 1. A mixed scope
retains the known roots. Parsing and model diagnostics use native wording and
locations, including the array index of a conflicting interface. Both output
formats support `--output NEW_FILE`.

In Rust, call `gdd::impact::analyze_impact(&path, &scope, implementation)`
or `analyze_impact_from_bundle(&bundle, &scope, implementation)` to reuse
one capture. Each returns an `ImpactReport` with an `exit_code`. Use
`gdd::impact::impact_text(&report)` to render the same witnesses in text.

## Compare model snapshots

Run `gdd diff BASE HEAD` to compare nodes, sources, questions and edges by
stable ID across GDD 0.2 and 0.3 models. The result includes exact field changes
with JSON pointers, separate project and extension changes, and relocations
between manifest and fragment files. Reordering edges or moving a declaration
can change the byte revision while preserving the declared model.

Use `--format text` to render the same result, or `--output NEW_FILE` for
exclusive publication. Exit 0 means both sides were completely assessed and
may have changes. Exit 1 includes known failures, and exit 2 means unresolved
coverage. Partial comparisons report differences between known common IDs and
suppress additions, removals and metadata comparisons.

Rust callers use `gdd::model_diff::{diff_models, diff_models_from_bundles,
diff_text}`. The path API captures aliases of one resolved manifest once.
The bundle API compares the two supplied captures, even if both were taken
from the same path at different times. Numeric comparison preserves integer
versus decimal declarations and compares equivalent finite decimal spellings
without rounding through a machine float.

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

## Observe Graphify JSON in memory

The Rust library projects the original bytes of a Graphify 0.9.75 AST or
node-link export. The index follows `gdd-observations/0.2` and the report
follows `gdd-report/0.2`. Caller-supplied source locators, producer versions
and code revisions are recorded as unverified claims.

```rust
use gdd::graphify::{AST_PROFILE, GraphifyOptions, observe_graphify};

fn inspect(raw: &[u8]) -> std::io::Result<()> {
    let options = GraphifyOptions {
        profile: Some(AST_PROFILE),
        locator: "graphify-out/graph.json",
        ..Default::default()
    };
    let (index, report) = observe_graphify(raw, options)?;
    println!("Artifact {}", index["artifact"]["revision"]);
    for check in &report.checks {
        println!("{} {}", check.check_id, check.result);
    }
    Ok(())
}
```

Use `NODE_LINK_PROFILE` for node-link exports. With no profile the report
records the byte identity and returns UNKNOWN. With an explicit profile the
adapter validates strict UTF-8 JSON, including duplicate object keys, and
checks the layout, references, edge directions and code classifications.
`report.exit_code()` returns 0 for PASS, 1 for FAIL and 2 for UNKNOWN.
The API returns an argument error for an invalid source ID.

A raw object can contain up to 256 nested containers, including the root.
Valid deeper objects yield a compact index and UNKNOWN. The adapter still
checks JSON syntax and duplicate keys at greater depths. For example, a
missing closing brace after 1,200 nested containers produces FAIL in Rust.
Python can return UNKNOWN when its parser exhausts the runtime stack before
reaching the malformed suffix.

Numeric producer identities compare by exact decimal value, so `1` and `1.0` collide,
while `"1"` is a distinct ID. Values such as `1e-4000` retain their identity
in Rust. Python reports UNKNOWN when its numeric projection would round a
value. Observation IDs bind to the original byte digest and the JSON Pointer,
including for numerically equivalent IDs in different records.

Use `gdd::graphify::decode_json` to read a producer JSON value up to the
projection depth limit while preserving field order and exact numbers. The
[Graphify conformance suite](../conformance/README.md#graphify-adapter)
executes the named fixtures and records the native precision verdict.

## Import, verify and recover Graphify files

```sh
cargo run --bin gdd -- graphify import examples/graphify/raw.ast.json \
  --profile graphify-0.9.75-ast --output new-observations
cargo run --bin gdd -- graphify validate new-observations
cargo run --bin gdd -- graphify export new-observations --output recovered.json
```

Import creates the bundle only after verifying the staged original and index.
Import and export require unused destinations. Each command writes its report
to stdout and uses the report's exit code. Export can create a byte-identical
file with exit 1 or 2 when the producer claims fail or remain unknown. A
failed integrity, manifest or projection check prevents recovery, as does an
unavailable original.

Use `gdd::graphify::import_graphify(input, output, options)` with
`GraphifyOptions`. File import derives the locator from the resolved input
path. The CLI requires an explicit `--profile` and accepts `--source-id`,
`--producer-version` and `--code-revision` as unverified claims.
`verify_graphify(path, profile, producer_version)` accepts a bundle directory,
an observations index, or raw JSON. A raw file named `observations.json`
requires an explicit profile. `load_observations(path)` returns the index,
original bytes and report together. `export_graphify(path, output)` performs
verification and writes the captured bytes.

Verification recognizes importer versions 0.1.0 through 0.1.2. It recomputes
the projection using the recorded claims and checks the original's length and
SHA-256. A historical compact index for a number such as `1e-4000` can be
recovered with an UNKNOWN projection. Fresh native imports project that number
exactly. A historical full index can embed producer data beyond the 256-container
raw projection limit. The native index reader supports up to 1,024 containers
including the index root. An index exceeding its reading limit cannot establish
the original's identity for export.

### Combine observations with model validation

```sh
gdd validate examples/graphify/observations --profile import.graphify
gdd validate examples/concert.gdd.yaml --profile model --profile import.graphify \
  --graphify examples/graphify/raw.ast.json --graphify-profile graphify-0.9.75-ast \
  --graphify-producer-version 0.9.75
```

With the import profile alone, the positional observations produce the adapter's
report. A separate `--graphify` path, an explicit `--model`, an additional
profile or an activity selects model-bound validation. The validator assesses
the positional model or secondary records as usual and appends integration
checks to its report. Supply `--graphify` to identify the observations in this
mode. An omitted observations input produces an UNKNOWN integration check.

An optional `--import-audit` file contains `format: gdd-import-audit/0.2`
and the JSON-compatible values `original` and `reconstructed`. The validator
checks the envelope for every profile. The import profile reports
`integration.loss` when reconstruction changes a value, an array's order,
a numeric type or the sign of decimal zero. Object keys may be reordered.
Finite decimals compare exactly across equivalent spellings.

Rust callers can assess files with `validation::validate_paths_with_graphify`
and `validation::GraphifyValidationOptions`. Pass a parsed audit through
`import_audit`. To reuse captured inputs, pass the model bundle and a report
from `verify_graphify`, `load_observations` or `observe_graphify` to
`validate_captured`. The following function uses the same evidence for each
call and returns the combined report without reopening either input.

```rust
use gdd::{bundle::Bundle, graphify::ImportReport, report::Report};
use gdd::validation::{Profile, ValidationOptions, validate_captured};

fn assess(bundle: &Bundle, observations: &ImportReport) -> std::io::Result<Report> {
    validate_captured(
        Some(bundle),
        Some(observations),
        ValidationOptions {
            profiles: &[Profile::Model, Profile::ImportGraphify],
            ..Default::default()
        },
        None,
        None,
    )
}
```

For standalone validation of captured observations, pass `None` for the
bundle and select only `Profile::ImportGraphify`.

## Propose a model from Graphify observations

```sh
cargo run --bin gdd -- graphify propose examples/graphify/observations \
  --mapping examples/graphify/mapping.json --output reviewed-proposal.json
```

The mapping file accepts the same JSON or restricted YAML as other GDD inputs.
Identify each selected code observation and supply its proposed target ID, kind,
interpretation and rationale. Each selected edge needs a known direction and
matching selected endpoints. A bundle can contain unresolved observations.
The proposal lists omissions and copies only the selected nodes' unverified
binding candidates. A failed import check or an unverified original or
projection prevents proposal generation.

The CLI creates a new file and emits the proposal on stdout. The candidate is
a `gdd/0.3` sketch with a non-normative source and an open question blocking
implementation and acceptance. Use `--format gdd/0.2` for a legacy candidate.
Both formats pass representation and model checks before publication.

Rust callers use `gdd::graphify::propose_graphify(path, &mapping, source_uri, format)`
with a `serde_json::Value` mapping and receive the proposal as JSON. Pass
`"graphify.raw.json"` when the candidate will reside beside the original.
`propose_graphify_to_file(path, &mapping, output, format)` computes the locator
relative to the output directory and publishes the proposal exclusively.

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
| `validate`, `exit_code` | `gdd::validation::{validate, validate_with_options}` and `Report::exit_code()`. Rust supports representation, model, readiness and records, including context packages, parallel plans and activity authorization. Separate records inputs are supported via `--model` and `validate_paths`. Use `Profile::ImportGraphify` for standalone observations and model-bound Graphify checks. Visualization validation still requires Python. |
| `import_graphify`, `verify_graphify`, `export_graphify`, `propose_graphify` | Native file import, verification, byte recovery, in-memory observations and explicit proposals for both model formats via `gdd::graphify`. |
| `perspectives`, `perspective_data`, `load_perspective_bundle` | Native path-based and captured model projections via `gdd::perspectives`. |
| `render_view`, `verify_view` | Python-only HTML APIs. The [native viewer](../crates/viewer/README.md) is a separate desktop preview. |
| `prepare_workflow`, `verify_workflow` | Native path-based and bundle-based workplan preparation and verification. |
| `run_status` | Native journal replay and current acceptance with path-based or captured inputs. |
| `create_run`, `advance_run` | Native exclusive run creation, gated event publication and idempotent replay. |
| `boundary_view`, `design_checklist`, `review_design` | Native GDD 0.3 boundary views, scoped review questions and advisory review applicability. |
| `project_status` | Native project assessments with an optional workflow journal, JSON and text output, and captured-bundle entry points. |
| `analyze_impact`, `impact_text` | Native declared consumers, supporting context, applicable records, JSON and text output, with path-based and captured-bundle entry points. |
| `diff_models`, `diff_text` | Native comparison of paths or captured models with scoped diagnostics, field changes, relocations and structural advisories. |
| `check_code` | Python-only code observation API. |
| `gdd_validator.host` helpers `snapshot_code`, `execute_check`, `verification_records`, `append_records` | Python-only trusted host operations. A code snapshot identifies selected implementation files, while a model snapshot identifies normative GDD assets. See the [host guide](../HOST.md). |
