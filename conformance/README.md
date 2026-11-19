# Conformance cases

`cases.yaml` describes 58 positive and negative expectations for the standard's profiles. `diagnostics.csv` contains stable diagnostic codes. Case IDs identify tests in the suite, not nodes in the example model.

The installed `gdd-conformance` runs named suites against a matching checkout.
From another directory, put `--checkout PATH` before the suite name:

```sh
gdd-conformance --checkout /path/to/gdd standard --output standard-run.json
```

Without the option, the runner looks for `conformance/cases.yaml` in the
current directory and its ancestors, up to the Git checkout root. A copied
fixture tree can also be used. Keep its suite catalogs in `conformance/` and
preserve the relative paths of their inputs, including example bundles,
schemas, tutorial models and archives used by the chosen suite. Relative
output paths resolve from the directory where you run the command.

## Installed native CLI

Supply the installed `gdd` and a separate reference `gdd` built for the same
checkout. Both paths may be relative to the directory where you start the runner.

```sh
gdd-conformance --checkout /path/to/gdd verify-install \
  --installed "/path/to/install/bin/gdd" --reference /path/to/reference/gdd \
  --output installed-cli.json
```

The runner stages checkout fixtures in a fresh temporary directory outside the
checkout and invokes both programs directly. Its 34 controls cover GDD 0.2 and
0.3 inspection, byte-for-byte regeneration of both concert views and the
concert application gallery view, product perspectives,
boundaries, planned review, acceptance, project creation, readiness, diff and
static import observation with an explicit CPython target. It compares parsed
read-only output and exit codes, validates the checked-in perspectives schema
and boundaries, and checks the bytes of files that must survive inspection or
refused initialization.

The `gdd-verify-install/0.1` JSON receipt records both executable SHA-256 digests
and the expected and observed exit codes of each attempted control. The first
failure ends the run with a diagnostic and a nonzero exit. Use a fresh path
for each receipt or omit `--output` to print it.

## Standard validator

Run the 58 named cases from this checkout with the locked Cargo dependencies
available locally.

```sh
cargo run --offline --locked -p gdd-conformance -- standard --output standard-run.json
```

Omit `--output` to print the full JSON receipt. With `--output`, the command
creates a new file and prints the matched and total counts. Each completed
case includes its ID, expected and actual verdicts, diagnostic codes in the
selected dimension, and the full native validator report. Case setup errors and
mismatches produce a nonzero exit after the remaining cases run.

[cases.yaml](cases.yaml) supplies the bases, scopes, profiles, [RFC 6902
patches](https://www.rfc-editor.org/rfc/rfc6902) and expectations. The runner
applies each patch and named condition to a private copy of the example inputs,
preserving authoring order when serializing modified fixtures for snapshot
revisions. Trusted mandates, evidence, parallel plans and import audits are
synthetic inputs generated for their named cases.

Replay the Python suite with `uv run python conformance/run.py --output python-standard-run.json`.
The `verification` field preserves the classification from the [standard
review](../evidence/standard-review.md). The [historical validator
receipt](../evidence/validator-conformance.json) identifies its own execution.

The harness checks the expected dimension/result and required diagnostic. It preserves all report checks, including when FAIL and UNKNOWN occur together. Parallel cases assess the separate plan operation; it does not claim action readiness. Import cases check demonstrable data loss and explicit UNKNOWN for unsupported profiles/versions. Therefore, 58 matching expectations are not certification of every possible standard requirement.

Synthetic receipts in case patches are test inputs to the representation schema. They are not registered approvals, observed code files or executed application evidence.

## Graphify adapter

Run the 18 named adapter cases with Cargo. Omit `--output` to print the
summary alone on a successful run.

```sh
cargo run --locked -p gdd-conformance -- graphify --output graphify-run.json
```

The runner reads [graphify-cases.yaml](graphify-cases.yaml), applies each case's
patches and conditions to its base graph, and imports the resulting bytes into
a temporary bundle. It compares the recovered bytes, declared node and edge
counts, verdict and required diagnostic. A case can select one check using
`check_id`. The receipt includes the full native import report even when the
case assesses a single check. A matching expected FAIL or UNKNOWN counts as a
success. Any mismatch or case error produces a nonzero exit after the remaining
cases run. `cargo test --locked --workspace` also executes the named suite.

The precision case uses the literal endpoint `9007199254740993.0` and node ID
`9007199254740992`. Rust distinguishes the two and expects FAIL with
`REFERENCE_UNRESOLVED`. The Python adapter reports UNKNOWN with
`FORMAT_UNSUPPORTED` at its numeric projection limit. The native receipt
records the Rust expectation. Replay the Python expectations with
`uv run python conformance/run_graphify.py`. Producer fixtures and their hashes
are documented in [examples/graphify](../examples/graphify/README.md).

## Visualization

Run the 16 named artifact cases against the passive `gdd-viewer-native/0.1`
profile from a checkout with the locked Cargo dependencies available locally.

```sh
cargo run --offline --locked -p gdd-conformance -- visualization --output native-view-run.json
```

The command prints the JSON receipt to stdout when `--output` is omitted.
With `--output`, it creates a new receipt file and prints a summary. Each row
records its model base, operation, scope, expected verdict, actual verdict
and full verifier report. A case setup error records the affected ID and error.
Any mismatch or setup error yields a nonzero exit after the remaining cases run.

[visualization-cases.yaml](visualization-cases.yaml) defines the inputs and
expectations for both the native and HTML suites. Each native case renders a
fresh `.gddview` from its example model or the [multigraph fixture](fixtures/viewer-multigraph.json).
The fixture preserves the bytes emitted by [viewer_fixture.py](viewer_fixture.py),
including its parallel edges, hostile labels and integer `9007199254740993`.
The scoped case renders the concert model around `action.reserve`.

For cases that change or withhold an artifact, the runner first verifies the
original saved bytes. The historical `changed_script` case appends unauthorized
trailing content to the passive native file, `blank_html` empties the file,
and `deep_payload` adds JSON nested beyond the native reader's depth limit.
The receipt identifies the renderer profile and UTC execution time.

Replay the historical `gdd-viewer-html/0.1` cases with
`uv run python conformance/run_visualization.py`. [run_browser.cjs](run_browser.cjs)
exercises offline browser interactions. See [VIEWER.md](../VIEWER.md) for the
artifact and interaction verification scopes.

## Code observation

Run the 28 named code observation cases (50 parameter instances) locally.

```sh
cargo run --offline --locked -p gdd-conformance -- code --output native-code-run.json
```

The Rust runner reads [code.cases.json](code.cases.json) and exercises each
behavior in process. It creates a private source tree, model and mapping records
for each filesystem control. The concert case copies the example's Python files
and checks the original tree after repairing and removing files in the copy.
The drift case changes a file between the observer's two inventory scans.
The schema case validates each verdict against the code observation schema.

The JSON receipt identifies the native producer in `tool` and `runner`. Its
`runtime_target` is a synthetic CPython 3.11 environment for these cases,
with the four frozen names used by the finder controls. Each case retains its
manifest ID and assertion and records its verdict and number of attempted
instances. Failed rows include parameter-specific errors. A failed expectation
or invalid manifest produces a nonzero exit. Omit `--output` for JSON on
standard output. With `--output`, the command creates a new file and prints
the matched case count.

To obtain a Python receipt with pytest exit status and captured output, run
`uv run python conformance/run_code.py --output python-code-run.json`.

## Project status

Run the 13 named project status cases (15 parameter instances) locally.

```sh
cargo run --offline --locked -p gdd-conformance -- status --output native-status-run.json
```

The runner reads [status.cases.json](status.cases.json) and exercises the
native status, readiness and workflow APIs in process. It initializes starter
projects, copies tutorial models, and prepares private workflow runs. The
integration case checks both an unavailable source and a rejected path. The
acceptance case replays the accepted journal in [agent-native](../examples/agent-native)
with its archived evidence, then tests a separate copy with the final evidence
record removed. Both runs retain the accepted state. The assessor reports
current acceptance only for the first copy.

Omit `--output` for the JSON receipt on standard output. With `--output`, the
runner creates a new file and prints the matched case count. Each row records
the manifest ID, assertion, result and attempted instance count. A failed
expectation or invalid catalog exits nonzero. The native receipt identifies
its producer in `tool` and `runner`. To run the corresponding Python tests
and collect pytest output, use
`uv run python conformance/run_status.py --output python-status-run.json`.

## Declared-model impact

Run the 17 named impact cases (28 parameter instances) locally.

```sh
cargo run --offline --locked -p gdd-conformance -- impact --output native-impact-run.json
```

The runner reads [impact.cases.json](impact.cases.json) and exercises the Rust
impact and context APIs in process. Private copies of the tutorial model provide
reverse dependency paths, refinements and cycles. Cases also load extension
definitions and record ledgers to check missing inputs, revision matching and
supersession. The read-only case compares the fixture files before and after
analysis and validates the report against [the impact schema](../schemas/impact.schema.json).

Rust accepts scope as a slice of strings. For the two Python scope inputs
`"action.add"` and `[42]`, the native case checks that decoding the JSON value
as a vector of strings fails. Empty scopes, malformed IDs, unknown nodes and
edge IDs reach the impact assessor. A single CLI argument `--scope action.add`
supplies a one-element scope.

Omit `--output` for JSON on standard output. With `--output`, the runner creates
a new receipt and prints the matched case count. The native receipt identifies
its producer in `tool` and `runner`. Each row preserves the catalog ID and
assertion and reports the verdict and number of attempted instances. Failed
expectations produce a nonzero exit. To collect the Python receipt with pytest
output, run `uv run python conformance/run_impact.py --output python-impact-run.json`.

## Stable-identity model diff

Run the 16 named diff cases (25 parameter instances) locally.

```sh
cargo run --offline --locked -p gdd-conformance -- diff --output native-diff-run.json
```

The runner reads [diff.cases.json](diff.cases.json) and exercises the Rust
model diff and text renderer in process. It compares private copies of the
tutorial model to check exact field changes, structural advisories, fragment
relocation, extension packaging and partial comparisons. It validates results
against [the diff schema](../schemas/model-diff.schema.json) and restores the
indexed concert experiment to compare the phase A and final requirements.

Omit `--output` to print the JSON receipt. With `--output`, the runner creates
a new file and prints the matched case count. Each row records the catalog ID,
assertion, verdict and number of attempted instances. Failed expectations
produce a nonzero exit. To collect the Python receipt with pytest output, run
`uv run python conformance/run_diff.py --output python-diff-run.json`.

## GDD 0.3 boundaries and design principles

Run the 32 cases and 52 behavioral instances in [design.cases.json](design.cases.json)
with the Rust runner.

```sh
cargo run --offline --locked -p gdd-conformance -- design --output native-design-run.json
```

The suite checks model and readiness diagnostics, component interfaces,
community calls, context closure, snapshot revisions, workflow packet integrity,
advisory review applicability and versioned schema definitions. Mutations use
private copies of the concert model. The artifact case renders and verifies a
standalone native `.gddview` file and reads its responsibility, rationale,
public contracts and calls. The Python case checks labels in rendered HTML.
Interactive viewer checks run separately.

The same case executes the native `gdd boundaries` and `gdd design-review`
commands and checks their JSON output and exit codes. Install a matching `gdd`
beside `gdd-conformance`, or set `GDD_NATIVE_CLI` to its absolute path. The
environment variable takes precedence. When neither is available, the runner
locates the executable with a locked, offline Cargo build in the selected
checkout.

Omit `--output` to print the JSON receipt. With `--output`, the runner creates
a new file and prints the matched case count. Each row identifies the assertion,
verdict and number of attempted instances. Failed expectations include their
parameter and error and cause a nonzero exit. To capture the Python receipt
with its pytest output, run
`uv run python conformance/run_design.py --output python-design-run.json`.
Synthetic reviews exercise envelope completeness and applicability. The
original 58 standard cases run under GDD 0.2.

## Workflow

Run the 20 named workflow cases (63 parameter instances) from a checkout.

```sh
cargo run --offline --locked -p gdd-conformance -- workflow --output native-workflow-run.json
```

The runner reads [workflow.cases.json](workflow.cases.json) in order. Private
copies of the concert bundle exercise packet integrity, read context,
write ownership and serialization. Synthetic host mandates, bindings and
evidence records exercise the dispatch, integration and acceptance gates. The
journal cases verify that malformed hosts, stale packets and declined events
cannot publish work. Two synchronized publishers compete to append to the same
head. Other cases cancel a run, replay a receipt, retry with a new attempt and
retain a late submission after the model becomes unavailable.

The final case reconstructs Rust counterparts of the historical Python agent
patches in a fresh project. Cargo runs the quote application's acceptance target,
including the compiled CLI. The seven expected floor-rounding failures on the
baseline become 19 passing tests after the repairs. Each of the three
integrations has a scoped Cargo receipt. The run reaches current acceptance
after ten journal events. The receipt identifies the Rust implementation and
the origin of its repair inputs in `demo_implementation` and `demo_inputs`.
To retain the new Rust project, journal and raw Cargo evidence, run the
replay directly with a new directory.

```sh
cargo run --offline --locked -p gdd-conformance -- workflow-demo --output native-workflow-demo
```

Omit `--output` from `workflow` to print the JSON receipt. With `--output`,
the runner creates a new file and prints the matched case count. Each row gives
the catalog ID, assertion, result and number of attempted instances. Failed
expectations include the parameter and error and yield a nonzero exit.
To run the original Python suite and capture its pytest output, use
`uv run python conformance/run_workflow.py --output python-workflow-run.json`.
