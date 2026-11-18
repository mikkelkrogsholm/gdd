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
Unicode decimal digits, and single underscores between digits. Invalid input
exits with status 2 and prints only `InvalidInput` to stderr.

The library functions take references to signed, arbitrary-precision `BigInt`
values. Rust rejects boolean, float, string and optional arguments at compile
time. The functions return `Err(InvalidInput)` for negative amounts or rates
outside 0 through 10000. The quote validates all three inputs before calculating
discount on the subtotal and tax on the remaining amount.

## Replaying the repair

The 19 named cases correspond to the [Python acceptance suite](../tests/).
The [archived execution evidence](../evidence/) records the original Python
runs. The Rust suite starts its own compiled binary for CLI checks.

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
