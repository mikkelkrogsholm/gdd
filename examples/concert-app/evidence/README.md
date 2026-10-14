# Concert app execution evidence

These artifacts describe actual local execution on October 14, 2026. The latest
[summary](summary.json) identifies the accepted A and B model/code revisions. Code and
model byte snapshots can be recomputed; GDD checks remain host-attested observations,
not execution performed by the standard or by pseudocode.

- `phase-a/`: actual worker/lead tests, browser controls and captures, positive import
  scan, distinct negative controls, acceptance/status, advisory review and SQL history.
- `phase-b/`: fixed requirement cases red against A, actual revised deliveries and
  combined tests/browser controls, preserved database activation, current import controls,
  acceptance/status, advisory review, graph rendering and final SQL history.
- `checker.worker.run.json` and `checker.host.run.json`: independent worker and lead
  execution of the bounded AST checker tests.
- `design.lint.run.json`: actual pinned design-md 0.4.0 lint. Zero errors; four orphan-token
  warnings concern canvas/muted/border/focus roles described in prose rather than assigned
  to structured component fields. The lead reviewed their actual CSS use.
- `phase-a/late-review-retrieved-output.json`: exact tool stdout retrieved from the
  independent agent's actual Chrome/temp-SQLite probes. The original temporary scripts
  and databases were not persisted; this is a retrieved presentation, not an original
  saved execution receipt. B's new controls execute the repaired behavior directly.
- `phase-a/metadata.correction.json`: the original A checker label mistakenly names
  pytest8.3.5. Actual execution used installed pytest9.1.1. The original records remain
  preserved; this explicit correction changes no source, assertion, result or digest.

The original A acceptance covered its declared executed tests. Later independent review
identified shared-cookie identity confusion and loss of the retry key after an unknown
postcommit500. During B repair, review also found that uncertain logout retry could
discard its original identity expectation. All three are fixed and covered by actual B
controls. Historical A acceptance is not presented as a claim that those defects were absent.

`import.controls.json` contains four separate source revisions: clean, deliberately
injected, repaired and missing. The FAIL belongs to the altered code and the UNKNOWN
to missing input. Neither is rebound as current positive evidence. The single selected
current checker receipt attests the clean code and these executed controls.

Browser screenshots are actual integrated application captures, not generated mockups.
The original post-zoom mobile screenshot has Chrome compositing artifacts. It remains
preserved; `phase-b/mobile.clean.png` and `desktop.clean.png` are later actual
fresh-context presentation captures, with their execution described in
`phase-b/clean-captures.json`. The original browser assertions and receipts are unchanged.
The B unknown500 browser control commits through the real server, then substitutes its
ambiguous response; a separate real HTTP test induces the server's actual serialization
failure after commit. The lost-response controls similarly drop a real committed reply.

The original [native journals](../runs/) and [dispatches](../dispatches/) are preserved.
Any later replay creates new run identities and evidence and explicitly reports that it
reuses archived patches rather than executing new agents.

`replay/` preserves byte-identical selected receipts, journals and snapshots from the
actual verified fresh replay. Its original execution directory is recorded in
`replay/provenance.json`; copied paths are presentations of that execution, not new runs.

The current record store contains only B's records. Exact original A records are in
the preserved A bundle. A diagnostic scratch copy and the correction are in
`phase-b/records.before-reconciliation.json` and `records.reconciliation.json`.
Implementation and authorization were validated separately before combining the final
report. `workflow-active-gates.retrieved.json` records an independent agent's15 actual
active-gate mutation probes; each revoked current acceptance while preserving history.
No validator-core modification was needed.
