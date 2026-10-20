# Local Sessions: a runnable GDD reference

A local concert reservation app developed through an executed GDD 0.3 experiment.
The reference keeps the current model, application, meaningful tests, phase-A replay
fixtures and selected results. The [complete original experiment](../../archive/experiments/README.md)
is preserved in a byte-indexed archive from Git commit `7e27757`.

From the repository root:

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

## Model and checks

Read [requirements](REQUIREMENTS.md), [HTTP contracts](HTTP.md), [design](DESIGN.md),
and the [interactive graph](graph.html). [gdd.json](gdd.json) contains the current
component rationales and public action contracts.
The graph is regenerated with the current viewer; its layout is a current
presentation of the model, separate from the original experiment's captures.

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
