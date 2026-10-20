# Complete graph-to-code tutorial

The [tutorial](../../docs/tutorial.md) now runs from a brief and GDD graph through
actual Python code, assertions, immutable records and current evidence assessment.
It uses the installed CLI and explicit host helpers, without an LLM service.
The old planning model remains byte-identical; a new model adds the explicit
blocking verification rule and a separate ledger.

## Actual verification and correction

The [verification record](verification.json) identifies source revisions and
changed-file hashes. **Verification is composed from the executions below**;
the original failed release attempt has not been relabeled as PASS.

- [Full-suite execution](release-attempt/summary.json): **1,723 tests passed** on
  **Python 3.12.12** at source `98c998e`; all **nine conformance runners** passed.
  Their [compact rollup](conformance-summary.json) retains original report hashes
  and locations; command logs are retained without duplicating the old raw
  conformance payloads again.
- That release attempt then **failed** the new installed tutorial location guard.
  macOS `mktemp -d` ignored the selected TMPDIR without an explicit template.
  The [original error](release-attempt/installed-wheel.stderr.txt) remains.
- Only `docs/tutorial.md` and its installed-wheel verifier changed afterward:
  use an explicit POSIX temp template and normalize its returned path. The
  runtime/package assets and built wheel stayed byte-identical. **38 affected
  tutorial/host/skill tests passed** after repair; the entire suite was not repeated.
- [Final focused matrix](quick-matrix/summary.json): **356 tests passed** separately
  on **Python 3.11.11, 3.12.12, 3.13.13 and 3.14.7**, including the corrected
  exact documented shell sequence. This is not four full-suite executions.
- [Corrected installed-wheel rerun](installed-wheel.json): **34 CLI checks**, exact
  assets, standalone skill, legacy replay and **all ten tutorial shell blocks**
  passed outside the checkout. Source/module/console validation parity is retained.

The installed receipt contains actual current PASS and isolated mutant FAIL
outputs, source hashes, timestamps and runner identity. Controls establish:

| Selected assessment | Actual result |
| --- | --- |
| Subtraction mutant, unchanged unittest assertions | FAIL / exit 1 |
| Original implementation and assertions | PASS / exit 0 |
| Records without host trust | UNKNOWN / exit 2 |
| Reviewed current evidence with explicit local host context | PASS / exit 0 |
| Acceptance without operational policy | UNRESOLVED / exit 2 |
| Actually changed code revision against old evidence | UNKNOWN / exit 2 |
| Authoring new records from the stale receipt | REFUSED |

The host script requires an explicit reviewed-checks attestation. Its fragment
supplies evidence trust, not policy or a mandate. The negative control executes
actual assertions and leaves original code untouched. Drift uses an isolated
copy, observes its changed revision and preserves original model/ledger/receipts.
This small example does not claim exhaustive correctness or broad project acceptance.

## Documentation and provenance

README leads to the complete tutorial; INSTALL distinguishes current development
assets from the earlier release. Tool guides link the tutorial and canonical
validation/host contracts instead of repeating the entire trust explanation.
**220 relative file targets** resolved across root/docs guides and the example
index. The declined external proposal is explicitly marked withdrawn.

Working notes now live under [docs/history](../../docs/history/README.md).
Root redirects preserve old citations. The [provenance map](../../docs/history/provenance.json)
retains original Git blobs/digests: WORKLOG moved byte-for-byte; the other two
notes changed only relative Markdown destinations, with prose unchanged. Earlier
execution identities were not rebound to these presentations.

No validator runtime, existing model, core schema or standard bytes changed.
No new interactive browser run, external study, runtime subscription, GitHub CI,
release, publication or push was performed.
