# Local Sessions: an executed GDD 0.3 experiment

A working local concert reservation app built from a frozen graph through actual
native-agent packages. It includes credential sign-in, a concert catalog, personal
reservation history, cancellation, file SQLite persistence and a responsive browser
interface. All application text is English.

Start from the repository root:

```sh
uv sync --group dev
uv run python examples/concert-app/run.py --database /tmp/local-sessions.sqlite --port 8765
```

Open `http://127.0.0.1:8765`. Fictional accounts are `alice` / `alice-demo` and
`bob` / `bob-demo`. The current app implements phase B: at most two active reservations
per person across all concerts. Cancelling a reservation frees allowance and preserves
its history and successful request key. Sessions are in memory; restarting the server
requires signing in again while reservations remain in SQLite.

This is a bounded loopback demo with fictional credentials. Payments, ticket issuance,
expiry, external authentication and public deployment are outside its accepted scope.
An incompatible database containing more than two active reservations for a person is
refused without deleting or silently changing history; reconcile it explicitly using
the previous policy before activation.

## What was actually tested

| Stage | Observed result |
| --- | --- |
| Phase A, unrestricted personal allowance | 98 tests and 11 subtests; five browser cases; GDD acceptance |
| Revised graph and ten fixed requirement assertions against A | Eight failures, two passes |
| Phase B, global limit of two | 112 tests and 11 subtests; eight browser cases; GDD acceptance |
| Forbidden direct API → Storage import | Clean PASS → injected FAIL → repaired PASS |
| Missing Storage source | UNKNOWN |
| Capacity guard deliberately bypassed | Unchanged capacity assertions FAIL |
| Real SQLite faults after insert/cancel and before commit | Rollback preserves rows and key binding |
| Committed reservation with lost response or unknown 500 | Same-key retry returns one reservation, including at the limit |
| Shared-cookie identity changes and uncertain logout retry | Stale actions rejected; other account's history/session preserved |

The actual A browser database was reused when starting B. Its full logical SQL dump
was identical before B browser mutations. The ten phase-B tests stayed byte-identical
from red to green. Independent review found two additional defects after the original
A acceptance, and a logout-retry defect during repair. B includes their fixes and real
controls; A remains a historical baseline with those known limitations.

See [the execution evidence](evidence/README.md), [summary](evidence/summary.json),
[requirements](REQUIREMENTS.md), [HTTP contracts](HTTP.md), [design system](DESIGN.md),
and [interactive graph](graph.html). The short component rationales and public action
contracts live in [gdd.json](gdd.json).

## Boundaries and agent delivery

| Component | Responsibility and reason for its boundary |
| --- | --- |
| Frontend | Present the program and personal tasks; it consumes the server's policy value |
| API | Translate HTTP and reject stale presented identities against the resolved session |
| Identity | Verify fictional credentials and own opaque session creation/resolution/revocation |
| Reservations | Own ordered business decisions, replay and the active-reservation policy |
| Storage | Own SQL, fresh connections, protected transactions and historical compatibility inspection |

Native workers developed backend and frontend in isolated directories from actual GDD
dispatches. A host-checker worker implemented the static import check. The lead wrote
transport/session code, reviewed and integrated every delivery, ran actual combined
checks and supplied trusted host receipts. B backend policy work ran in parallel with
the lead's API/frontend repairs. Shared-checkout workers were read-only; no worker made
Git changes or spawned another agent layer.

The [A](runs/phase-a/events/) and [B](runs/phase-b/events/) journals retain
actual dispatch, submission, integration and acceptance events. Work packets include
frozen graph revisions, public contracts, boundaries, design questions and owned files.
Absolute temporary workspace names in these historical artifacts identify the original
execution; they are not required by the running app.

Broad KISS, DRY, separation-of-concerns and modularity judgments remain advisory.
Their revision-bound lead review is [complete](evidence/phase-b/design.review.status.json);
that applicability result does not automatically certify code quality or grant acceptance.
The precise API import prohibition is checked against real source bytes. Dynamic,
ambiguous, missing or overlapping inputs remain UNKNOWN under the bounded Python
AST profile; the checker does not prove arbitrary runtime reflection safe.

SQLite writes use `BEGIN IMMEDIATE` before business reads so the global count,
concert capacity, key binding and insert share one writer transaction. See the
[SQLite transaction documentation](https://www.sqlite.org/lang_transaction.html) and
[Python sqlite3 transaction documentation](https://docs.python.org/3.12/library/sqlite3.html#transaction-control).

## Run checks and replay

```sh
(cd examples/concert-app && uv run --project ../.. python -m pytest tests tools/test_import_check.py -q)
uv run gdd validate examples/concert-app/gdd.json --profile readiness --scope action.reserve
uv run python examples/concert-app/tools/import_check.py --help
uv run python examples/concert-app/tools/replay.py --output /tmp/local-sessions-replay
```

The replay destination must not exist. Replay copies the archived delivered patches,
creates new GDD journals and host evidence, runs A in a real browser, obtains red results
for the fixed B requirement cases, applies B, then verifies B against the same database.
It performs the actual import/capacity fault controls again. It calls no LLM agents and
makes no claim of fresh code generation. Source/test snapshots must match the saved
delivery at both phases before checks and acceptance.

The browser runner uses Node, Playwright and Chrome. On this machine it can use the
bundled Codex Playwright module and installed Chrome. Elsewhere set `GDD_PLAYWRIGHT`
to an absolute installed Playwright module path and `GDD_CHROME` to the Chrome executable;
`--node` selects another Node executable. The app itself uses only Python's standard
library. GDD and verification dependencies are provided by the repository environment.

## Files and historical identity

`history/phase-a/` preserves the exact accepted A model, normative assets, records,
application and tests. Its browser checker was recovered after later test additions
and verified against the original recorded SHA256; it is not a new A execution.
Current normative sources and snapshots describe B. Record files and checker scripts
are separate from normative model assets. App code revisions hash all Python application
and test files, local HTML/CSS/JS, and `run.py`; host tools carry their own source hashes.
Do not reuse A evidence to authorize B or rewrite historical execution digests.
