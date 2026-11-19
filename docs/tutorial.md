# From a brief to code and current evidence

Build one pure Rust function, inspect its GDD graph, and run the same five
comparisons against the implementation and an isolated subtraction mutant.
You will produce the model snapshot, native graph view, execution receipts and
evidence ledger on your own machine.

Start at the root of the current Rust checkout with Rust 1.95+, Cargo and the
[native CLI installed](../INSTALL.md#install-the-rust-cli-and-viewer). The
desktop viewer is optional. Use one fresh POSIX shell for all the command
blocks. If your Cargo cache is empty, run `cargo fetch --locked` first. The
exercise runs offline and requires no LLM account.

## Turn a bounded brief into a draft

> A caller supplies two mathematical integers and receives their exact sum.
> The operation is pure and local. Transport, text parsing and bounded
> machine-integer overflow handling are outside this exercise.

One component and one public action suffice. Initialize a new draft and check
its structure, then ask if its placeholder action is ready to implement.

```sh
set -eu
work=$(mktemp -d "${TMPDIR:-/tmp}/gdd-tutorial.XXXXXX")
draft="$work/draft/gdd.json"
expect_exit() (
    expected=$1
    shift
    actual=0
    "$@" || actual=$?
    test "$actual" -eq "$expected"
)
gdd init "$work/draft" --project-id project.integer-addition --title "Local integer addition"
gdd validate "$draft" --profile model
expect_exit 1 gdd validate "$draft" --profile readiness --scope action.main
```

Model validation exits **0**. Readiness exits **1**. Find
`READINESS_CONTRACT_MISSING`, `BOUNDARY_RATIONALE_MISSING`,
`SCENARIO_CATEGORY_MISSING` and `QUESTION_BLOCKS_SCOPE` in the diagnostics.
The draft leaves the component boundary, action contract, behavior and normal
scenario open. Its scope question blocks implementation.

## Refine the model and inspect the change

Read the [authored refinement](tutorial/refined.gdd.json). The arithmetic
component explains its boundary and publishes `action.add`. The action points
back to its owner. Its contract names both integer inputs, exact integer output
and postcondition. The explicit empty context, errors and effects, and the risk
rationales, follow from a calculation with no parsing, identity, shared state
or range threshold. A bounded integer API would need a different contract.

```sh
model="$work/refined.gdd.json"
cp docs/tutorial/refined.gdd.json "$model"
gdd diff "$draft" "$model" --format text
gdd validate "$model" --profile model
gdd validate "$model" --profile readiness --scope action.add
gdd status "$model" --scope action.add --format text
gdd impact "$model" --scope action.add --format text
```

The diff shows three added nodes, the removed placeholders and the removed
scope question. The refined model and scoped readiness both pass. Status still
shows unassessed implementation and authorization. Impact identifies the owner
and the planned scenario as supporting context. Readiness describes the model
at this point, before anyone has implemented or run the example.

## Hand off the revision and inspect a detached view

```sh
gdd snapshot "$model" --output "$work/snapshot.json"
gdd context "$model" --scope action.add --output "$work/context.json"
cat "$work/snapshot.json" "$work/context.json"
gdd view "$model" --output "$work/add.gddview"
gdd validate "$model" --profile visualization --view "$work/add.gddview"
```

Compare the snapshot's `revision` with `spec_revision` in the context packet.
The packet names the selected action, its owner `component.arithmetic`, the
planned `scenario.add` and the two connecting edges. Give your coding agent
the model and packet together so it can read the contracts at that revision.
Choose an implementation, files and checks under your actual working mandate.

**Optional desktop step.** Open the self-contained view with
`gdd-viewer "$work/add.gddview"`. Select `action.add` and expand **Canonical
node record** for the contract and behavior. The viewer initially hides
scenarios. Enable **Show scenarios (1)**, select `scenario.add` and expand
**Canonical node record** to read the given, when and then fields. For inputs
−7 and 7, the planned result is zero. In a narrow window, open **Nodes** and
expand **View filters** to reach the checkbox. The saved file can travel on
its own to another desktop with the viewer.

## Plan the evidence

The planning model contains no blocking verification rule. The
[evidence model](tutorial/evidence.gdd.json) adds `rule.sum`, its `constrains`
edge to the action, a `verifies` edge from the scenario and a separate ledger.
The [Rust preparation example](../examples/tutorial_prepare.rs) copies that
model with the verification method `rust_integer_addition_examples`, takes
its new revision and appends the immutable check definition `check.sum`.

```sh
plan="$work/sum"
cargo run --offline --locked -p gdd --example tutorial_prepare -- "$plan"
gdd diff "$model" "$plan/evidence.gdd.json" --format text
gdd validate "$plan/evidence.gdd.json" --profile model
gdd validate "$plan/evidence.gdd.json" --profile readiness --scope action.add
validate_sum() { gdd validate "$plan/evidence.gdd.json" --profile records --scope action.add "$@"; }
cat "$plan/records.json"
expect_exit 2 validate_sum
```

Read `check.sum`'s scope, `rule.sum` and `scenario.add`. It requires an
observed Rust compiler version and the `isolated-subtraction-mutant` control.
The method name describes a check. The host selects the actual command.
Implementation is **UNKNOWN** (2) while the ledger contains only the plan.

## Run and inspect both native checks

The [implementation](tutorial/rust/src/lib.rs) accepts two borrowed
`BigInt` values and returns `left + right`. The [checker](tutorial/rust/src/main.rs)
compares five sums against expected values, covering the planned cancellation,
two positives, zero, two negatives and an integer larger than a machine word.
Review the implementation for purity and the examples for coverage.
Packaging gives the host a standalone Cargo manifest and lockfile.

```sh
cargo package --offline --locked --allow-dirty --no-verify -p gdd-tutorial-add --target-dir "$work"
tar -xzf "$work/package/gdd-tutorial-add-0.1.0.crate" -C "$work"
source_dir="$work/gdd-tutorial-add-0.1.0"
cat "$source_dir/src/lib.rs" "$source_dir/src/main.rs"
tutorial() { cargo run --offline --locked -p gdd --example tutorial_evidence -- "$@"; }
tutorial run "$plan" "$source_dir"
current="$plan/runs/current"
mutant="$plan/runs/mutant"
for file in Cargo.toml Cargo.lock src/main.rs; do
    cmp "$current/$file" "$mutant/$file"
done
expect_exit 1 diff -u "$current/src/lib.rs" "$mutant/src/lib.rs"
cat "$plan/runs/negative.receipt.json" "$plan/runs/pass.receipt.json"
```

The [host example](../examples/tutorial_evidence/main.rs) runs the mutant
first. Only `src/lib.rs` changes, from `left + right` to `left - right`.
Each copy uses a fresh private target directory and this literal command.

```text
cargo run --quiet --offline --locked --manifest-path Cargo.toml --target-dir target --bin gdd-tutorial-add
```

Inspect each receipt's command, working directory, tool versions, timestamps,
stdout, stderr and before/after revisions. Both snapshots identify the four
selected files by path and hash. The mutant must return **FAIL** (1) with
`FAIL: -7 + 7: expected 0, got -14`. The current implementation must return
**PASS** (0) with `PASS: all five integer sums matched`. A build error yields
**UNKNOWN** with its diagnostic. The failing comparison shows that these
examples distinguish addition from subtraction. Their coverage is limited to
the five cases.

The local release check also runs the host example from an extracted `gdd`
crate with the separately packaged `gdd-tutorial-add` as the checker source.
See [repeating the packaged tests](../RELEASING.md#repeat-the-packaged-root-and-tutorial-tests).

## Record reviewed evidence and assess acceptance

After inspecting both implementations, the unchanged checker and both receipts,
attest that you reviewed their coverage and the performed control.

```sh
tutorial record "$plan" --attest-reviewed-checks
cat "$plan/records.json" "$plan/review/host-context.json"
expect_exit 2 validate_sum
validate_sum --host-context "$plan/review/host-context.json"
expect_exit 2 validate_sum --host-context "$plan/review/host-context.json" --activity acceptance
```

The host rechecks the receipts against the files, command, compiler and
expected outputs. It appends `binding.sum.v1`, which binds the checker file
to both targets, and `evidence.sum.v1` for `implementation.integer-addition.rust`.
The evidence identifies the model revision, check digest and exact code snapshot.

Without caller trust the implementation is **UNKNOWN** (2). With your local
host context it is **PASS** (0). Look at the separate authorization dimension
in the last report. Acceptance is **UNRESOLVED** (2) because the caller
supplied no acceptance policy or authority. For work with an actual mandate
and run acceptance, see the [workflow guide](../WORKFLOW.md).

## Observe what a later edit does to evidence

```sh
tutorial drift "$plan"
expect_exit 1 diff -u "$current/src/lib.rs" "$plan/drift/changed/src/lib.rs"
expect_exit 2 validate_sum --host-context "$plan/drift/host-context.json"
```

The drift stage appends a comment in a third copy, observes its new code
revision, and tries to author fresh binding and evidence IDs from the old PASS
receipt. Watch for `Old receipt rejected: The selected code snapshot is stale
or unresolved.` The subsequent validation reports stale evidence and
implementation **UNKNOWN** (2). Even this behavior-preserving edit changed the
bytes that the checker would run. The original three ledger records and both
receipts remain available for inspection. For a new implementation revision,
run the checks again, review the results and append new records. To repeat
this exercise from the beginning, start with a fresh directory.

## Reproduce the historical Python tutorial

The Python exercise and its assets are preserved in [the v0.8.0 tutorial](https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/docs/tutorial.md).
For exact reproduction from a clone, create a separate checkout with
`git worktree add --detach ../gdd-python-v0.8.0 0dbb10b8c193f55c9777405de3f4ae22ac2d0825`.
In that directory, run `uv sync --locked --dev --python 3.12`, activate
`. .venv/bin/activate`, and follow its `docs/tutorial.md`. The
[archived verification](../evidence/tutorial-0.1/README.md) describes the
historical Python runs.
