# Stable-ID model comparison

This records the local issue #6 implementation in the unreleased package 0.8.0,
with preparation and limitations for the parallel #9/#10/#15 tracks.
[Usage](../../docs/model-diff.md) describes `gdd diff` and `diff_models`.

## Actual verification

The [verification record](verification.json) identifies the clean tested source,
changed-file hashes, baseline and exact wheel digest. Receipts retain original
commands and temporary execution paths.

- [Full release verification](release/summary.json): **1,722 tests passed** on
  **Python 3.12.12**, then all **nine conformance runners** passed.
- [Focused matrix](quick-matrix/summary.json): **355 tests passed** separately
  on **Python 3.11.11, 3.12.12, 3.13.13 and 3.14.7**. This is not four full-suite
  executions.
- [Diff conformance](release/diff.json): **16 named cases / 25 instances**.
- [Installed wheel](release/installed-wheel.json): **30 CLI checks**, including
  equal-model and removed-postcondition controls, source/module/console parity,
  exact assets, standalone skill extraction and preserved legacy replay.
- A fresh byte-verified restoration of the concert archive was compared through
  the actual CLI in JSON and text. [JSON](concert-comparison.json) and
  [text](concert-comparison.txt) expose `LimitReached` and the per-principal limit:
  five added/five changed nodes, 22 added edges and one changed source.

The replay compares preserved model declarations; it does not execute the app
or replace historical execution evidence. Input bytes remain unchanged.

Controls cover registry identity, fragment relocation, formatting, exact field
pointers, legacy/current formats, extension declarations, structural advisories,
missing inputs, collisions, read-only operation and exclusive outputs. Advisory
findings do not prove semantic weakening or historical ID reuse. Independent
review caught a real semantic-UNKNOWN exit-policy defect before final verification;
two regression instances retain UNKNOWN/FAIL precedence and suppress fabricated
absence while preserving known field changes. No further blockers were found
in that bounded review.

The standard, existing core schemas, models, archived bytes and historical
receipts are unchanged. The current concert README only gains a comparison link.
No new interactive browser run, GitHub CI, release, publication or push is claimed.

## Other tracks and pending decisions

- [External pilot proposal](../../docs/proposals/external-pilot.md): pinned Huey,
  pagination brief, neutral evaluator, budgets and equivalent arms. Inspection
  was performed; no qualification tests or comparative study were executed.
  Approve subject/task, pilot boundary, evaluator and budgets before execution.
- [Second-runtime access trial](../second-runtime-2026-10-20/README.md): against
  the earlier baseline wheel, Claude Code discovered the skill but both actual
  journeys failed HTTP 401 before model execution. The maintainer has no Claude
  subscription here. Park #10; no second-runtime success is claimed for this
  source state or the earlier wheel.
- [Version policy proposal](../../docs/proposals/version-policy.md): explicit
  recommendations for legacy support, contract versions, Python coverage and
  separate package/standard 1.0 criteria. Current release rules remain in force;
  #15 needs maintainer decisions, not an automatic breaking change.
