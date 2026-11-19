# Local Sessions: a runnable GDD reference

`gdd-concert` is a Rust reservation server for a local concert program. Its
Cargo source archive includes the server, domain library, tests, two SQLite
fixtures, GDD 0.3 model and contracts, portable graph and MIT license.
The model records the phase-B policy from the [original A/B experiment](#historical-ab-delivery-and-replay).

## Run the native concert app

In a Git checkout, change to `examples/concert-app`. With the source archive,
run `tar -xzf gdd-concert-0.1.0.crate` and `cd gdd-concert-0.1.0`. In either
directory, run the Rust tests and start the server with a local database.

```sh
cargo test --locked
cargo run --locked -- --database local-sessions.sqlite --port 8766
```

You can also install the executable from that directory with
`cargo install --locked --path .` and run it as
`gdd-concert --database local-sessions.sqlite --port 8766`.

The startup line reports the listening address once the database is ready. Use
`--port 0` for a free port, or omit the option for port 8765. The server binds
to IPv4 loopback and accepts `127.0.0.1` and `localhost` with its actual port.
Open `http://127.0.0.1:8766/` to browse the program. Sign in with the fictional
account `alice` / `alice-demo` or `bob` / `bob-demo`. Reserve one seat at a time,
refresh availability, and use **Your reservations** to see or cancel your seats.
The desk shows the active allowance returned by the reservation service. The
current limit is two per person across all concerts, and cancellations free
allowance while preserving history.

The executable serves the HTML forms and bundled stylesheet. The browser works
with JavaScript disabled. A completed form submission redirects to a new page.
If the browser shows an error after you reserve, use Back and submit the
original form again, or reopen its original desk URL. If your request reached
the server and reserved a seat, that concert shows **Check reservation**, even
when your allowance is full or the concert has sold out. Checking returns the
current state of that reservation, including any cancellation.
After a successful redirect, the new program page offers fresh reservations.
If a shared cookie changes, the old desk asks for explicit sign-in before
showing private history or accepting another action.

The same server implements the seven JSON routes in the [HTTP contract](HTTP.md).
Open `http://127.0.0.1:8766/api/concerts` to read the public catalog. To sign in
and read your history from a terminal, use a temporary cookie jar.

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

For focused domain and HTTP checks, run these commands from the app directory.

```sh
cargo test --locked --lib --test reservations
cargo test --locked --test http
```

The domain suite checks the allowance, capacity, retries, concurrent writes
and phase-A database compatibility. The HTTP tests start the executable on
ephemeral ports using temporary databases and exercise its JSON API and native forms.
On Unix they also exercise SIGINT, SIGTERM, request draining and restart.

To run the native browser regressions, install a local Chromium or Chrome and a
matching ChromeDriver. Put `chromedriver` on `PATH` or set `GDD_CHROMEDRIVER`
to its executable path. From the app directory, run the ignored browser tests.

```sh
cargo test --locked --test browser -- --ignored
```

Each test opens a headless browser with JavaScript disabled and a temporary
profile. The phase-B test starts the Rust server on a loopback port with a copy
of `tests/fixtures/recorded-phase-a.db`. It checks the original reservation cards,
the two-seat allowance, cancellation and rebooking through the forms, then
compares the final history with the JSON API and the saved database. It also
uses tabs sharing a cookie to check that an outdated Alice desk cannot read
Bob's history, reserve on his behalf or sign him out. A final check opens the
public program at a 320-pixel viewport.

The response-loss test uses an empty database. A loopback proxy drops the
server's completed reservation redirects until Chromium reports a failed
navigation. The test reads the committed reference from the authenticated API,
uses Back to resubmit the original form, then cancels and replays the old form.
The desk and the API must still show one cancelled reservation with that
reference.

Another test takes both matinee seats and replaces the second redirect with an
HTTP 500. It reopens the original desk URL to check the committed reservation,
verifies that a fresh desk blocks new bookings, then cancels and checks the
same reference again.

The keyboard test tabs from the public program through the labelled username,
password and sign-in controls, then presses Enter and reads the signed-in
status. It checks the form and reservation desk at a 320 CSS-pixel viewport
and in a 640-pixel window with Chromium's per-host page zoom set to 200%.
Chromium reports the effective zoom, viewport and content width. Every visible
button and input must fit horizontally and measure at least 44 CSS pixels high.
The test also verifies that the reduced-motion rule is active and that the desk
has one status region and one alert region.

The ordinary Cargo suite skips these browser tests.

The native HTTP/1 parser rejects repeated Content-Length fields with the
JSON error `400 InvalidInput` before routing. The body limit is 16 KiB.
Repeated Host fields or an unexpected Host or Origin on a parsed request
return `403 Forbidden`. If the same request has both a repeated length and an
invalid local address or origin, the native parser returns 400. The historical
Python service checked the address and origin first and returned 403 in that case.

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
The [native graph](graph.gddview) contains the component rationales and public
action contracts from [gdd.json](gdd.json). The [snapshot](model.snapshot.json)
identifies the model and contract bytes. The [record store](gdd.records.json)
preserves the historical execution records. With the GDD CLI and desktop viewer
installed separately, you can check the reservation action, verify the graph
against its source model and open it from the app directory.

```sh
gdd validate gdd.json --profile readiness --scope action.reserve
gdd validate gdd.json --profile visualization --view graph.gddview
gdd-viewer graph.gddview
```

| Component | Responsibility |
| --- | --- |
| Frontend | Program and personal tasks; consumes the server policy value |
| API | HTTP translation and stale presented-identity rejection |
| Identity | Fictional credentials and opaque in-memory sessions |
| Reservations | Ordered business decisions, replay and active-reservation policy |
| Storage | SQL, fresh connections, protected transactions and history compatibility |

SQLite writes acquire `BEGIN IMMEDIATE` before business reads. Capacity, global
allowance, request-key binding and insertion share one writer transaction.

## Historical A/B delivery and replay

The original agents delivered a Python application in two phases. Phase A had
no personal reservation limit. Phase B introduced a two-seat limit across all
concerts. The v0.8.0 replay recreates both deliveries, tests B’s unchanged
requirements against A, activates B on the same database, and writes new
journals, checker receipts and host acceptance for each phase. The Rust suite
above exercises the shipped server and phase-A data compatibility. The
complete two-phase delivery and attestation replay belongs to v0.8.0.

From a Git checkout containing the v0.8.0 commit, extract that release into an
empty directory. This recovers the exact Python application, phase-A sources,
browser assets, replay tools, HTML graph and their matching validator.

```sh
release_dir=$(mktemp -d)
git archive 0dbb10b8c193f55c9777405de3f4ae22ac2d0825 | tar -x -C "$release_dir"
cd "$release_dir"
```

Follow the [v0.8.0 replay instructions] from that directory, including the
Node, Playwright and Chromium setup and a new output path. The [v0.8.0
verification] records a separate run of those curated tools. The [byte-indexed
archive] preserves the complete original 197-file experiment tree from commit
`7e27757`, including its original checkers, journals and captures.

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

For the original checker bindings and acceptance results, see the [experiment
evidence]. To compare the retained phase-A and phase-B models by stable ID,
run the native CLI from the concert app directory in the current Git checkout.

```sh
gdd diff history/phase-a/gdd.json gdd.json --format text
```

[v0.8.0 replay instructions]: https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/examples/concert-app/README.md#replay-in-a-fresh-directory
[v0.8.0 verification]: https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/examples/concert-app/evidence/current-replay.verification.json
[byte-indexed archive]: https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/archive/experiments/README.md
[experiment evidence]: https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/examples/concert-app/evidence/README.md
