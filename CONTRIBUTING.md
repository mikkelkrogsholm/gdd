# Contributing

Contributions are welcome: documentation, reproducible bug reports, conformance
cases, viewer improvements, adapter profiles and proposals for the method or
standard. You do not need to build a whole application to help.

## Choose a contribution

Search [existing issues](https://github.com/mikkelkrogsholm/gdd/issues) first.
Use the bug-report form for unexpected behavior, or the proposal form to explain
a workflow that GDD should support. Questions and small documentation fixes are
welcome too. For a larger feature or standard change, open an issue before
implementing it so we can agree on scope and compatibility.

Start with [README.md](README.md) and the complete [tutorial](docs/tutorial.md).
Keep changes bounded to a concrete user journey, validator defect or documented
contract. A small bug report should identify the input, command, actual result
and expected behavior; preserve diagnostic codes and include a minimal fixture
when possible. Do not include credentials or private source material.

Install the source using [INSTALL.md](INSTALL.md). The repository prose is
English, including comments, examples, generated visuals and evidence summaries.
Proper names, identifiers and intentional Unicode test data keep their technical
meaning. Prefer Markdown, JSON, YAML, JSONL, CSV or HTML for new deliverables.

## Make a pull request

Fork the repository on GitHub, then clone your fork. Replace `YOUR-USERNAME`
below with your account name:

```sh
git clone https://github.com/YOUR-USERNAME/gdd.git
cd gdd
git remote add upstream https://github.com/mikkelkrogsholm/gdd.git
git switch -c contribution/my-change
```

Make one focused change, run the relevant local checks, and commit it with a
message describing the result. Push your branch to your fork and open a pull
request against this repository's `main` branch. Use the PR template to describe
the problem, behavior after the change, actual checks and any revision/evidence
impact. Link the issue when one exists.

Small fixes can go directly to a PR. Add a regression case for a behavior change;
documentation-only changes need link/command checks rather than new unit tests.
Include a meaningful negative or UNKNOWN case when changing a validator rule.
Do not add GitHub CI as part of your contribution: checks are intentionally local.

Keep discussion respectful and concrete. The maintainer reviews contributions;
review may request a smaller scope, clarification or more evidence. No CLA or
special commit-signing requirement is imposed. AI-assisted contributions are
welcome; contributors remain responsible for understanding and verifying them.

## Check a change

Run the local Rust checks from a source checkout with Cargo, rustfmt and Clippy
installed. Use quick mode while iterating and the full command to verify the
workspace and standalone skill together.

```sh
cargo run --locked -p gdd-conformance -- local-check --quick
cargo run --locked -p gdd-conformance -- local-check --output ../gdd-check
cargo run --locked -p gdd-conformance -- local-check --release --output ../gdd-native-release
```

Quick mode runs the scoped readiness and view CLI integration targets, the
headless native viewer frame tests, and both standalone skill asset checks
(`sync --check` and `verify`). Full mode checks formatting in both workspaces,
builds the workspace binaries, runs the workspace and standalone skill tests,
runs Clippy on all targets and features with warnings as errors, checks the
skill assets, and executes the nine named conformance suites. Each suite writes
its own JSON receipt.

Release mode runs every full check, then uses `cargo install --locked --path`
to install optimized builds of `gdd`, `gdd-viewer` and `gdd-conformance`
under the output directory's `install/` prefix. From outside the checkout it
runs the 34 installed CLI controls with the workspace-built `gdd` as their
reference, checks that the installed viewer loads and prints `--help` headlessly,
and runs the installed quote workflow demo through its failing baseline and
accepted repair. It then retains the root CLI and library source file
(`gdd-<version>.crate`), records its path and SHA-256 in `summary.json`, and
compiles and runs the locked root tests from a fresh extraction outside the
checkout, using a new build directory for the package and its dependencies.
Choose either `--quick` or `--release` for a non-default mode.

The command prints its new output directory and stops at the first failed
step. It saves separate stdout and stderr logs, plus a `summary.json` with
commands, working directory, host and toolchain, timings and exit status.
Omit `--output` to keep the results in a fresh system temporary directory.
Use `--checkout PATH` **before** `local-check` to build and test another
checkout. Relative output and `CARGO_TARGET_DIR` paths are resolved from the
directory where you start the runner. Builds default to the selected checkout's
`target/`. See [native distribution evidence](RELEASING.md#verify-the-native-distribution-locally)
for the source package, installed binaries, verifier receipt and workflow journal.

### Individual Rust checks

Run the Rust in-memory Python import observer tests with
`cargo test --locked -p gdd --lib python_imports`. Its [Rust module](src/python_imports/mod.rs)
documents the target grammars, runtime module input and supported source encodings.

Run native tests with `cargo test --locked --workspace`. To execute the
[58 standard cases](conformance/README.md#standard-validator) and save their
full Rust validation reports in a new file, run
`cargo run --offline --locked -p gdd-conformance -- standard --output standard-run.json`.

To run the 18 named
Graphify adapter cases and keep their full Rust import reports, use
`cargo run --locked -p gdd-conformance -- graphify --output graphify-run.json`.
The suite and its precision expectation are documented in
[Conformance cases](conformance/README.md#graphify-adapter).

Run the 16 [native visualization artifact cases](conformance/README.md#visualization)
with `cargo run --offline --locked -p gdd-conformance -- visualization --output native-view-run.json`.

Run the 28 [native code observation cases](conformance/README.md#code-observation)
with `cargo run --offline --locked -p gdd-conformance -- code --output native-code-run.json`.

Run the 13 [native project status cases](conformance/README.md#project-status)
with `cargo run --offline --locked -p gdd-conformance -- status --output native-status-run.json`.

Run the 17 [native impact cases](conformance/README.md#declared-model-impact)
with `cargo run --offline --locked -p gdd-conformance -- impact --output native-impact-run.json`.

Run the 16 [native model diff cases](conformance/README.md#stable-identity-model-diff)
with `cargo run --offline --locked -p gdd-conformance -- diff --output native-diff-run.json`.

Run the 32 [native design boundary and review cases](conformance/README.md#gdd-03-boundaries-and-design-principles)
with `cargo run --offline --locked -p gdd-conformance -- design --output native-design-run.json`.
Use a new file path for each receipt.

To exercise a separately installed native CLI, run the
[34 installed CLI controls](conformance/README.md#installed-native-cli).

### Python migration and release checks

The Python reference and distribution checks use the existing local runner.
It records interpreter versions, durations, commands and logs in a new
temporary directory and stops on the first unsuccessful step.

```sh
uv sync --locked --dev --python 3.12
uv run --no-sync python scripts/check.py --quick   # focused parser, diagnostics, tutorial, Graphify, viewer and skill checks
uv run --no-sync python scripts/check.py           # full pytest suite and skill synchronization check
uv run --no-sync python scripts/check.py --matrix  # full pytest on installed Python 3.11, 3.12, 3.13 and 3.14
uv run --no-sync python scripts/check.py --release # full suite, named conformance reports, wheel build and installed verification
```

The full suite can take several minutes. `--matrix` is explicit and sequential;
it uses isolated environments and refuses automatic Python downloads. A missing
interpreter is a reported failure, not a skipped success. `--quick --matrix`
runs the focused set on those interpreters. Install the desired interpreters
separately. The matrix list is maintained in `scripts/check.py`; it describes
the tested interpreter lines, not every future version admitted by packaging.
The host interpreter's exact patch is retained for its matching matrix line;
choose it with the outer `uv run --python VERSION_OR_PATH` when checking a
particular patch. Reports always identify the interpreter actually executed.

Use `--output NEW_DIRECTORY` to retain reports in a chosen location; existing
directories are refused. `summary.json` and per-step logs provide the local
verification record for a PR. Browser checks remain separate and opt-in.
Passing pytest scratch directories are discarded; failing cases retain their
temporary inputs for diagnosis. Full suites on several interpreters can use
substantial disk space, so matrix execution is sequential.
The release mode intentionally executes conformance cases separately to retain
named receipts; ordinary full checks do not repeat those runners afterward.

The individual commands remain available as a reference. For release
verification, use the full set below from the repository root with a fresh
output directory so new verification does not overwrite historical evidence:

```sh
uv sync --locked --dev --python 3.12
GDD_CHECK_DIR="$(mktemp -d)"
uv run --no-sync python scripts/sync_skill.py --check
uv run --no-sync pytest -q
uv run --no-sync python conformance/run.py --output "$GDD_CHECK_DIR/standard.json"
uv run --no-sync python conformance/run_graphify.py --output "$GDD_CHECK_DIR/graphify.json"
uv run --no-sync python conformance/run_visualization.py --output "$GDD_CHECK_DIR/visualization.json"
uv run --no-sync python conformance/run_workflow.py --output "$GDD_CHECK_DIR/workflow.json"
uv run --no-sync python conformance/run_design.py --output "$GDD_CHECK_DIR/design.json"
uv run --no-sync python conformance/run_code.py --output "$GDD_CHECK_DIR/code.json"
uv run --no-sync python conformance/run_status.py --output "$GDD_CHECK_DIR/status.json"
uv run --no-sync python conformance/run_impact.py --output "$GDD_CHECK_DIR/impact.json"
uv run --no-sync python conformance/run_diff.py --output "$GDD_CHECK_DIR/diff.json"
uv build --wheel --out-dir "$GDD_CHECK_DIR/dist"
uv run --no-sync python conformance/verify_readiness_wheel.py "$GDD_CHECK_DIR"/dist/*.whl --output "$GDD_CHECK_DIR/installed-wheel.json"
```

For viewer interactions or browser behavior, also run the optional browser
recipe in [INSTALL.md](INSTALL.md). Artifact-byte verification and browser
interaction verification establish different facts. See the historical
[interpreter coverage](docs/proposals/version-policy.md#python-verification-and-admission)
for exact patches and full versus focused checks. Identify the runtime and
environment of new executions explicitly.

Keep tests focused on observable behavior and meaningful failure controls.
Use FAIL for a demonstrated violation and UNKNOWN for missing or unsupported
evidence. A passing schema check does not establish implementation quality,
operational authority or acceptance of a change.

## Preserve revisions and evidence

Changing model or normative asset bytes changes their revisions. Update current
snapshots and record references, regenerate affected derived artifacts and run
the relevant conformance checks. Keep original execution evidence tied to its
original Git state. Mark translations and reconstructed captures explicitly;
do not present them as new executions. [evidence/README.md](evidence/README.md)
describes the provenance policy.

The standalone skill contains synchronized reference copies. After changing
authoritative guidance, regenerate the skill, review the changes and run its
standalone tests. Run these commands from the repository root so Cargo builds
in the checkout's `target/` directory.

```sh
cargo run --locked --target-dir target --manifest-path skills/gdd/scripts/Cargo.toml -- sync
cargo run --locked --target-dir target --manifest-path skills/gdd/scripts/Cargo.toml -- sync --check
cargo test --locked --target-dir target --manifest-path skills/gdd/scripts/Cargo.toml
```

Use `--root PATH` for a different checkout. The index retains its source URL
and Git base unless supplied with `--repository URL` or `--git-base-revision REV`.
The hashes describe the actual working-tree bytes. The existing Python freshness
check remains part of local release verification during the migration.

## Versions and releases

Explain the concrete problem, resulting behavior and validation in the pull
request. Include reproducible commands and material limits; a screenshot or
worker's PASS statement alone is insufficient evidence. Keep unrelated user
changes and historical fixtures intact.

[RELEASING.md](RELEASING.md) defines the active version, compatibility, tag,
local release-check and publication rules. Its linked policy proposal records
pending decisions without changing today's support. Describe user-visible
changes in [CHANGELOG.md](CHANGELOG.md)'s Unreleased section; the maintainer
assigns the release version.

The maintainer selected the [MIT License](LICENSE) for GDD's code and associated
documentation. Contributions are accepted under that license; preserve existing
third-party notices and provenance. Registry publication requires a separate
maintainer decision.
