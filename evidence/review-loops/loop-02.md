# Loop 2 — fixes and fresh verification

Status: verified. Four parallel read-only reviews and the lead's cross-check found 11 new finding groups, R020–R030 in [loops.json](loops.json). All were fixed with regressions; no confirmed findings were deferred.

Authorization now computes compatible, trusted supersession before using the request's scope to select covering approvals. A narrow approval can therefore retire a narrow rejection under a broader standing mandate without itself authorizing broader work. The same compatibility rule protects readiness reviews and question decisions against unrelated replacements. Older/untrusted, incorrectly scoped, or merely proposed receipts cannot retire current decisions.

Directory ownership with terminal `.`/`..` is normalized with the subtree; versions use ASCII digits across schema engines. Native atomic rename rejects NUL before the platform call. The CLI proposal's source URI is calculated from the actual locations of the manifest and output directory, including through symlinks. The proposal API requires finite JSON values and Unicode scalars that can actually be materialized in UTF-8.

Design references require `purpose: contract` in both model schemas and are always resolved as contracts for readiness. Active local `$ref`/`$dynamicRef` can no longer conceal a different schema dialect, remote refs, hooks, or invalid subschemas in otherwise inert fields. Unused const/annotation data remain data. Depth that the schema engine used cannot determine yields UNKNOWN; supported positive/negative schemas retain their normal result. Very large decimal values in line-range are compared without a bounded integer conversion.

Negative reproductions: [the lead's first probes](loop-02-lead-before.json), [CLI locator](loop-02-cli-locator-before.txt), [Graphify/schema before fixes](loop-02-integration-before.txt), and [model/readiness before fixes](loop-02-model-before.txt). These are deliberate failure outputs, not the final status. The first lead test log also contained an incorrect proposal key in the locator test; the corrected negative locator test is saved separately. During combined browser integration, the existing design-ref fixture was also corrected to contract purpose, as was the pytest fixture.

Integrated result: **456 pytest tests pass**, **58/58** standard cases, **18/18** Graphify cases, **16/16** renderer cases, and **20/20** offline browser checks match. The browser run has no script/console errors or external requests. See [tests](loop-02-tests.txt) and the other concrete outputs in the ledger. This is framework validation with synthetic receipts and evidence, not attestation of actual application behavior.

A fresh loop 3 reviews the entire framework after the fixes. The model and protocol reviewers switch areas to give the changes a different perspective. The lead writes all checkout changes; workers have only prepared bounded patches and probes in temporary copies.
