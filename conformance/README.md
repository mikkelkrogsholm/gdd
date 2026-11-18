# Conformance cases

`cases.yaml` describes 58 positive and negative expectations for the standard's profiles. `diagnostics.csv` contains stable diagnostic codes. Case IDs identify tests in the suite, not nodes in the example model.

A case selects a base artifact and profile, specifies any scope and changes, and states the expected dimension/result. `patches` uses [RFC 6902 JSON Patch](https://www.rfc-editor.org/rfc/rfc6902). [run.py](run.py) creates concrete local fixtures for condition cases and executes the validator in temporary bundles. The original examples are not changed.

`verification` preserves the classification from the standard phase. The historical authoring check is documented in the [standard review](../evidence/standard-review.md). Current validator execution is recorded separately in [validator-conformance.json](../evidence/validator-conformance.json). `expected` is used only for the assertion after actual execution, never as a result inside the validator.

```sh
uv run python conformance/run.py --output evidence/validator-conformance.json
uv run pytest -q
```

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

[visualization-cases.yaml](visualization-cases.yaml) adds 16 artifact cases for `gdd-viewer-html/0.1`. Run `uv run python conformance/run_visualization.py`. [run_browser.cjs](run_browser.cjs) also tests actual offline browser actions, graph identity, hostile text, numeric precision, folding, downloads and responsiveness. See [VIEWER.md](../VIEWER.md) for runtime and verification scope.


## GDD 0.3 boundaries and design principles

[design.cases.json](design.cases.json) names the new boundary, contract, community,
workflow and advisory review assertions. [run_design.py](run_design.py) executes
their actual pytest nodes in an isolated process and retains counts and output.
Parameterized instances include positive, negative, unknown, legacy-compatibility
and revision-drift controls. The original 58 cases remain unchanged and run under
GDD 0.2. Run `uv run python conformance/run_design.py --output REPORT.json`.
The checker tests envelope completeness/applicability; synthetic reviews provide
no application-quality or acceptance evidence.
