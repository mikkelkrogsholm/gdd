# Integer quote demo in Rust

This standalone Cargo package implements the [accepted quote contract](../REQUIREMENTS.md).
From this directory, run the CLI with a subtotal and two basis-point rates.

```sh
cargo run --quiet --locked -- 101 5000 5000
# {"discount": 50, "net": 51, "subtotal": 101, "tax": 25, "total": 76}
cargo test --locked --test acceptance
```

With the locked dependencies cached, both commands also accept `--offline`.
The CLI accepts signed decimal integer text with optional surrounding whitespace,
Unicode decimal digits, and single underscores between digits. For example,
`cargo run --quiet --locked -- 101 10001 5000` supplies an out-of-range rate.
The CLI exits with status 2, leaves stdout empty and prints `InvalidInput` to stderr.

The library functions take references to signed, arbitrary-precision `BigInt`
values. Rust rejects boolean, float, string and optional arguments at compile
time. The functions return `Err(InvalidInput)` for negative amounts or rates
outside 0 through 10000. The quote validates all three inputs before calculating
discount on the subtotal and tax on the remaining amount.

## Replaying the repair

The 19 named Rust cases correspond to the [v0.8.0 Python acceptance suite](https://github.com/mikkelkrogsholm/gdd/tree/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/examples/agent-native/tests).
The [archived execution evidence](../evidence/) records the original Python
runs. The Rust suite starts its own compiled binary for CLI checks.

From the repository root, run the ten-event Rust workflow into a new directory.
The checker runs Cargo offline and needs the quote package's locked dependencies
in the local cache. `cargo fetch --locked --manifest-path examples/agent-native/rust/Cargo.toml`
populates that cache.

```sh
cargo run --locked -p gdd-conformance -- workflow-demo --output /tmp/quote-rust-run
cargo run --locked -p gdd -- workflow status /tmp/quote-rust-run/model.gdd.json \
  --run /tmp/quote-rust-run/run --host-context /tmp/quote-rust-run/host.acceptance.json
```

The replay applies the three bundled Rust changes in a private copy and starts
the compiled CLI through the acceptance tests. It binds four `cargo_test` rule
declarations to a new spec revision and preserves the normative requirements
bytes. The journal and its relative submission and evidence paths travel with
the output directory. Each receipt under `evidence/` includes the actual Cargo
argument list, exit code, stdout, stderr, toolchain, named outcomes and source
snapshot. Cargo exits 101 for the seven baseline failures. The summary reports
their test verdict as `baseline_exit: 1` and the process status as
`baseline_cargo_exit: 101`.

### Checking the three repairs by hand

To reconstruct the defective state, copy this package to a temporary directory,
then copy the contents of `baseline/` over the root of that copy. The overlay
reproduces the three archived source changes. Both leaf modules round half up
and the quote suite omits the large-integer regression. The acceptance run then
has 11 passing tests and seven failures.

Restore each of the following files from the final package and run its focused
tests in the temporary directory.

| File restored | Command | Expected passes |
| --- | --- | ---: |
| `src/discount.rs` | `cargo test --locked --test acceptance discount::` | 5 |
| `src/tax.rs` | `cargo test --locked --test acceptance tax::` | 5 |
| `tests/scenarios/quote.rs` | `cargo test --locked --test acceptance quote::` | 9 |

Run the full acceptance command again for all 19 cases.
