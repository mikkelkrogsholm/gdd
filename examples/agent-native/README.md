# Integer quote workflow

The [quote contract](REQUIREMENTS.md) requires floor rounding in integer minor
units. Discount applies to the subtotal, then tax applies to the net. From the
repository root, run the Rust CLI and its acceptance suite.

```sh
quote_manifest=examples/agent-native/rust/Cargo.toml
cargo run --quiet --locked --manifest-path "$quote_manifest" -- 101 5000 5000
# {"discount": 50, "net": 51, "subtotal": 101, "tax": 25, "total": 76}
cargo test --locked --manifest-path "$quote_manifest" --test acceptance
```

## Run the three repairs

The workflow starts with a half-up baseline. Seven of its 18 tests fail the
floor assertions. It dispatches the discount and tax packages, integrates
their repairs, then dispatches the quote package for a large-integer CLI
regression. All 19 final tests pass, including calls to the compiled binary.

```sh
cargo fetch --locked --manifest-path examples/agent-native/rust/Cargo.toml
quote_output=$(mktemp -d)/quote
cargo run --locked -p gdd-conformance -- workflow-demo --output "$quote_output"
cargo run --locked -p gdd -- workflow status "$quote_output/model.gdd.json" \
  --run "$quote_output/run" --host-context "$quote_output/host.acceptance.json"
```

The demo applies bundled Rust source changes in a private copy and runs Cargo
offline at each check. It writes three dispatches, three submissions, three
integrations and one acceptance to `run/events/`. Inspect `evidence/baseline.run.json`
and `evidence/final.run.json` in the output for named results, source snapshots
and process exits. The status reports `event_count: 10` and
`acceptance_current: true` against that run's host context. Its model declares
the Cargo checker and carries its own revision. See the [Rust package
guide](rust/README.md) for the CLI input grammar and individual repair checks.

## Original agent delivery

The original agents received the [three work packages](workflow.plan.json) and
worked in separate copies. They repaired two Python modules and added the CLI
regression. The lead integrated and accepted their work. The [original
results](evidence/summary.json), [journal](native-run/run.json), [records](model.records.json)
and [host context](host.acceptance.json) identify that execution. The
[baseline](code.snapshot.json) and [final](code.final.snapshot.json) code
snapshots hash a sorted map of Python source and test file digests. The
[model snapshot](model.snapshot.json) separately identifies the original graph
and requirements bytes.

The [v0.8.0 source tree](https://github.com/mikkelkrogsholm/gdd/tree/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/examples/agent-native)
includes the original baseline, Python CLI, tests, submitted worker files and
replay script. For an offline copy, follow the [release recovery
command](../README.md#recover-the-v080-examples). Run the historical replay
from the root of that recovered release.

See the [workflow contract](../../WORKFLOW.md) and the
[lead](../../agents/gdd-lead.md) and [worker](../../agents/gdd-worker.md) guides.
