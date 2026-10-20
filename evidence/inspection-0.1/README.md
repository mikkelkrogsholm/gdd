# Read-only project inspection

This review records issues #20 and #21 in the unreleased package 0.8.0.
`gdd status` composes existing assessments, explicit workflow state and grounded
next actions. `gdd impact` explains declared potential consequences with stable-ID
witnesses, reverse dependency direction and separate supporting context. Both
provide JSON and text, Python APIs and informative tool schemas. See the
[inspection guide](../../docs/inspection.md) for exact inputs and coverage.

## Actual local verification

The [verification record](verification.json) identifies the clean tested source
commit, changed-file hashes, unchanged protected assets and exact wheel digest.
Receipts retain their original temporary paths and actual commands.

- [Full release verification](release/summary.json): **1,692 tests passed** on
  Python **3.12.12**, followed by all eight conformance runners.
- [Focused matrix](quick-matrix/summary.json): **325 tests passed** separately on
  Python **3.11.11, 3.12.12, 3.13.13 and 3.14.7**. This is focused cross-version
  coverage, not four full-suite executions.
- [Status conformance](release/status.json): **13 named cases / 15 instances**.
- [Impact conformance](release/impact.json): **17 named cases / 28 instances**.
- [Fresh installed wheel](release/installed-wheel.json): **28 CLI checks**, exact
  packaged assets, source/module/console parity, standalone skill extraction,
  preserved legacy replay and existing code-observation controls outside checkout.

Controls cover starter/ready/unresolved/failed states, explicit missing or stale
runs, historical records, dependency direction, cycles, multiple roots, extension
semantics, embedded references, applicable plans/bindings, read-only operation
and exclusive output. The documented tutorial executes both new commands.

Independent review reproduced two defects before final verification: retired
check plans appeared as rerun candidates, and status omitted integration checks
from its overall exit policy. Targeted regressions verify the corrections. An
integration test also exposed package-export name shadowing in its monkeypatch
setup; that test now resolves the actual module explicitly.

The standard, existing core schemas, models, example views, archive and earlier
evidence remain unchanged. Impact does not claim complete runtime coverage or
grant write ownership. Unassessed status dimensions are explicit; no project
completion verdict is synthesized. Recorded acceptance projection uses synthetic
assessor responses; existing workflow tests retain responsibility for real
acceptance evidence. No new interactive browser run is claimed. Verification
remains local; no GitHub CI, registry publication or release tag was added.
