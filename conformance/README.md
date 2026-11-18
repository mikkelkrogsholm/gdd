# Conformance cases

`cases.yaml` describes 58 positive and negative expectations for the standard's profiles. `diagnostics.csv` contains stable diagnostic codes. Case IDs identify tests in the suite, not nodes in the example model.

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

## GDD 0.3 boundaries and design principles

[design.cases.json](design.cases.json) names the new boundary, contract, community,
workflow and advisory review assertions. [run_design.py](run_design.py) executes
their actual pytest nodes in an isolated process and retains counts and output.
Parameterized instances include positive, negative, unknown, legacy-compatibility
and revision-drift controls. The original 58 cases remain unchanged and run under
GDD 0.2. Run `uv run python conformance/run_design.py --output REPORT.json`.
The checker tests envelope completeness/applicability; synthetic reviews provide
no application-quality or acceptance evidence.
