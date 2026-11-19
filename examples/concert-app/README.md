# Local Sessions: a runnable GDD reference

A local concert reservation app developed through an executed GDD 0.3 experiment.
The reference keeps the current model, application, meaningful tests, phase-A replay
fixtures and selected results. The [complete original experiment](../../archive/experiments/README.md)
is preserved in a byte-indexed archive from Git commit `7e27757`.

## Original browser application

Run the Python browser application from the repository root.

```sh
uv sync --group dev
uv run python examples/concert-app/run.py --database /tmp/local-sessions.sqlite --port 8765
```

Open `http://127.0.0.1:8765`. Fictional accounts are `alice` / `alice-demo` and
`bob` / `bob-demo`. Sign in, browse concerts, reserve, inspect personal history,
cancel and sign out. Phase B permits at most two active reservations per person
across all concerts. Cancellation frees allowance and preserves history and the
successful request key. Reservations persist in SQLite; sessions require a new
sign-in after restart. The application uses only Python's standard library.

This bounded loopback demo excludes payments, ticket issuance, expiry, external
authentication and public deployment. A database already containing more than two
active reservations for a person is refused without modifying history.

## Native Rust JSON API

The `gdd-concert` executable serves the seven JSON routes in the [HTTP contract](HTTP.md).
Use a separate database to try the Rust server alongside the Python browser application.
Run these commands from the repository root.

```sh
cargo run --locked -p gdd-concert -- --database /tmp/local-sessions-rust.sqlite --port 8766
```

The startup line reports the listening address once the database is ready. Use
`--port 0` for a free port, or omit the option for port 8765. The server binds
to IPv4 loopback and accepts `127.0.0.1` and `localhost` with its actual port.
Open `http://127.0.0.1:8766/api/concerts` to view the public catalog. To sign in
and read your history from another terminal, use a temporary cookie jar.

```sh
jar=$(mktemp)
curl --silent --show-error --cookie-jar "$jar" --json '{"username":"alice","password":"alice-demo"}' \
  http://127.0.0.1:8766/api/login
curl --silent --show-error --cookie "$jar" http://127.0.0.1:8766/api/reservations
rm "$jar"
```

Ctrl+C or Unix SIGTERM drains admitted requests and checkpoints the database
before exit. Wait for a successful exit before copying the main file. Restart
with the same path to recover reservations and sign in again for a fresh session.

Run the domain and HTTP checks locally with `cargo test --locked -p gdd-concert`.
The HTTP suite starts the executable on ephemeral ports using temporary databases.
On Unix it also exercises SIGINT, SIGTERM, request draining and restart.

The native HTTP/1 parser rejects repeated Content-Length fields with the
JSON error `400 InvalidInput` before routing. The body limit is 16 KiB.
Repeated Host fields or an unexpected Host or Origin on a parsed request
return `403 Forbidden`. If the same request has both a repeated length and an
invalid local address or origin, the native parser returns 400. The Python
service checks the address and origin first and returns 403 in that case.

## Native Rust reservation library

At startup, await `Reservations::open(path)` to initialize the catalog and
activate phase B. Clone the resulting handle for concurrent requests. Call
`catalog()` for public availability. For personal operations, pass the
session store's resolved ID to `Principal::from_session(Option<&str>)`, then
await `list_own(principal)`, `reserve(principal, concert_id, request_key)` or
`cancel(principal, reservation_id)`. Success values serialize with the
documented HTTP fields. A `DomainError` serializes as its code and provides
the public text through `message()`. Controlled requirement-change replays
can explicitly select `Reservations::open_phase_a_for_replay(path)`.

Turso runs in process and opens a fresh connection for each operation on the
shared database handle. Database paths must be representable as UTF-8.
Before transferring the main database file, stop admitting requests, drain them,
await a successful `checkpoint()`, and drop all handles. A busy or incomplete
checkpoint returns `PersistenceUnavailable` and can be retried. After an abrupt
exit, reopen the database in place with its WAL to recover committed changes. A
live Rust handle holds a file lock even between operations. A stock
`sqlite3 -readonly` query on Linux reported “database is locked” while Rust
held the file and succeeded after the last handle closed.

Two SQLite files in `tests/fixtures` exercise compatibility with prior data.
`recorded-phase-a.db` was reconstructed using stock SQLite from the archived
`evidence/phase-a/database.sql` at Git commit `7e27757`. Its source SQL has
SHA256 `953d0127838ad73c45349037ca8d6c2e79c6a949d014f44df58ad2605bbf936b`.
`over-limit.db` was created using the Python storage implementation with three
active Alice reservations and one cancelled Bob reservation. Rust tests use
temporary copies to check replay and byte-preserving refusal.

## Model and checks

Read the [requirements](REQUIREMENTS.md), [HTTP contracts](HTTP.md) and [design](DESIGN.md).
From the repository root, run `gdd-viewer examples/concert-app/graph.gddview`
to explore the [native graph](graph.gddview). It contains the component rationales
and public action contracts from [gdd.json](gdd.json). The [released HTML view](graph.html)
also presents this model. The original execution captures are in the experiment archive.

| Component | Responsibility |
| --- | --- |
| Frontend | Program and personal tasks; consumes the server policy value |
| API | HTTP translation and stale presented-identity rejection |
| Identity | Fictional credentials and opaque in-memory sessions |
| Reservations | Ordered business decisions, replay and active-reservation policy |
| Storage | SQL, fresh connections, protected transactions and history compatibility |

SQLite writes acquire `BEGIN IMMEDIATE` before business reads. Capacity, global
allowance, request-key binding and insertion share one writer transaction.

```sh
(cd examples/concert-app && uv run --project ../.. python -m pytest tests tools/test_import_check.py -q)
uv run gdd validate examples/concert-app/gdd.json --profile readiness --scope action.reserve
uv run python examples/concert-app/tools/import_check.py --help
```

The bounded Python AST checker detects the precise forbidden direct API-to-Storage
import. It reports UNKNOWN for missing, dynamic or ambiguous inputs. Broad code-quality
judgments remain advisory; [historical review status](evidence/phase-b/design.review.status.json)
does not grant acceptance.

## Replay in a fresh directory

Replay copies the delivered A and B patches, creates new journals and evidence,
runs real SQLite/HTTP/browser checks, obtains red results for the unchanged B
requirements against A, and verifies B against the same database. It calls no
LLM agents. Both source/test snapshots must match the original deliveries.

The optional browser checks need Node and Playwright. One explicit setup, following
[Playwright's library installation guidance](https://playwright.dev/docs/library):

```sh
npm install --prefix "$PWD/.gdd-browser" --no-audit --no-fund --save-exact playwright@1.64.1
node "$PWD/.gdd-browser/node_modules/playwright/cli.js" install chromium
export GDD_PLAYWRIGHT="$PWD/.gdd-browser/node_modules/playwright"
uv run python examples/concert-app/tools/replay.py --output /tmp/local-sessions-replay
```

The replay output must not already exist. `GDD_PLAYWRIGHT` selects an installed
module; otherwise Node resolves `playwright` normally. Playwright launches its
installed Chromium by default. Set `GDD_CHROME` to an existing browser executable
when using another installation. `GDD_NODE` or `--node` selects the Node executable.
The current scripts contain no personal runtime defaults. The earlier recorded
trial used an explicitly configured existing Playwright 1.62.1 and Chrome
installation. A separate readiness trial installed Playwright 1.64.1 and its
Chromium build in a fresh directory and completed both A and B replays; see
[repository readiness evidence](../../evidence/repository-readiness.md).

Each replay creates `replay.provenance.json` with the current tool hashes and
runtime configuration, and binds new checker evidence to the actual script bytes.
[The current replay verification](evidence/current-replay.verification.json) records
its actual checks separately from the [historical results](evidence/summary.json).

## Original execution and recovery

| Original stage or control | Observed result |
| --- | --- |
| A: unrestricted personal allowance | 98 tests, 11 subtests, five browser cases, acceptance |
| Fixed B requirements against A | Eight failures and two passes |
| B: global limit of two | 112 tests, 11 subtests, eight browser cases, acceptance |
| Direct API-to-Storage import | PASS, injected FAIL, repaired PASS |
| Missing Storage source | UNKNOWN |
| Capacity guard bypass | Fixed capacity assertions FAIL |
| SQLite faults and uncertain committed responses | Rollback or same-key retry preserves history and binding |
| Shared-cookie identity change and uncertain logout retry | Stale actions rejected; the other account preserved |

Later review found identity-presentation and unknown-500 retry defects in A and an
uncertain-logout retry defect during B repair. B includes fixes and real controls;
historical A acceptance covers its executed checks and retains these limitations.

Application code, tests, models, normative assets, snapshots and retained record
bytes are unchanged by curation. `history/phase-a/`, work requests and host policy
fixtures are replay inputs; old temporary workspace names in those fixtures are
replaced by each replay. The historical B record store remains a presentation of
its original execution. Its browser binding identifies the original archived
checker, whose bytes differ from the portable current tool. Use newly generated
replay records for current checks. Do not reuse old receipts to authorize new work.

[Evidence guidance](evidence/README.md) distinguishes original execution, fresh
verification and reconstructed presentations. [Archive recovery instructions](../../archive/experiments/README.md)
cover all original packets, plans, journals, tools, receipts and captures, including
files removed from this concise reference.

## Review the model requirement change

The current `gdd diff` command can compare the archived phase-A and phase-B
models by stable ID. It surfaces the per-principal reservation limit and the
changed contract/scenario declarations without executing the app. Follow the
[restoration and comparison commands](../../docs/model-diff.md#replay-the-archived-requirement-change).
This is a current comparison of historical model bytes, not new evidence of
the application's original execution.
