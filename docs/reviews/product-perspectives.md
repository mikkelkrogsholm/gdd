# Product perspectives review

## Round 1: input status and projection capacity

Scope: manifest/asset loading, supported formats and product semantics, mixed
validation results, projection/output capacity, and caller path hygiene.
The viewer's shared model input boundary is included because it embeds the
product projection. Advisory gap semantics and historical evidence are unchanged.

Reproduced against base `1e16fa1`:

- `uv run --no-sync gdd perspectives /tmp/gdd-review-definitely-missing-model.json`
  returned 1 with a misleading supported-manifest message. Validation returned 2
  with `SOURCE_UNAVAILABLE`. After correction, perspectives returns 2 and retains
  the diagnostic code, file location and underlying reason.
- Missing includes, extensions and normative assets, unsupported formats, and
  extension validation capacity were collapsed into generic invalid-bundle
  failures. Known invalid input also lost its detailed diagnostics.
- Simulated perspective copy/output recursion escaped as a traceback. Capacity
  stops now return an unresolved error before any output file is opened.
- The viewer input boundary had the same diagnostic/status collapse. Its legacy
  extension and incomplete-product controls still render successfully.

Corrections preserve every failing/unresolved bundle diagnostic in rejection
messages and prioritize a known FAIL over UNKNOWN checks. A shared validated
projection loader supports core 0.2/0.3 without imposing product semantics on
unrelated extensions. Product interpretation remains pinned at the product
projection boundary. Invalid or unresolved inputs emit no projection and neither
create nor modify the requested output file. Deep valid annotation data at 250
nested arrays remains accepted; data beyond the existing parser depth limit is
unresolved. Pure bundle projection still performs no model-path reads.

Local execution evidence (temporary machine-local logs, not historical execution
records or regenerated evidence captures):

- `/tmp/gdd-review-r1-before-after.json`: missing-manifest exit codes 1 before,
  validation 2, perspectives 2 after. Matching `.stdout`/`.stderr` files retain
  the original and corrected messages.
- `/tmp/gdd-review-r1-regressions-before.stdout`: the initial 16 input regressions
  failed on the base implementation, including diagnostic retention and capacity.
- `/tmp/gdd-review-r1-viewer-before.stdout`: 17 viewer input regressions failed
  against the base viewer loader; two valid controls passed.
- `/tmp/gdd-review-r1-focused-final.log`: final focused verification of the
  delivered source and regressions: **148 passed in 19.56s**. Command:

  ```sh
  uv run --no-sync python -m pytest tests/test_perspectives.py tests/test_perspectives_inputs.py tests/test_perspectives_viewer.py tests/test_review_viewer.py tests/test_review_viewer_loop4.py tests/test_review_viewer_loop9.py tests/test_inspection_cli.py tests/test_review_cli.py tests/test_review_cli_loop2.py tests/test_review_cli_loop3.py tests/test_skill_assets.py -q
  ```

- `uv run --no-sync python scripts/sync_skill.py --check` and `git diff --check`
  passed. The changed perspective error contract was synchronized into the
  portable skill. No full-suite, wheel, browser, or remote CI run was requested
  for this bounded round.

## Round 2: advisory semantics and offline interaction

Scope: existing advisory gaps, global versus targeted core rules, and bounded
adversarial browser interactions. No new rule categories, completeness obligations,
renderer assets or versions were introduced.

Reproduced against base `0f5ca06`: a valid global invariant without `constrains`
produced `MISSING_RULE_TARGET` on both core 0.2 and 0.3. This contradicted the
existing global applicability semantics in `standard/VALIDATION.md`. The new
regression controls failed twice for that advisory; targeted rules without
`constrains` already failed correctly. The misleading rule-gap branch is removed.
Global rules retain their original canonical content in the data perspective and
remain in context closure from either an entity or its component. Targeted rules
without targets still fail model validation and projection (exit 1), with the
`CONSTRAINT_INVALID` diagnostic. Incomplete journeys remain valid and retain all
five existing actor/action/response/component/entity-link advisory codes on both
core formats.

Real offline Chromium coverage now includes exact actor goal and action/response
inspection with closing-script text, HTML event handlers, an external image URL,
Unicode and line/paragraph separators. Text remains inert, with no injected DOM,
network asset request or console/page error. Lens/kind/search/community/folding
combinations preserve exact canonical node and edge intersections and partitions.
Folded internal edges and opposite-direction parallel bundles retain their IDs.
Gap and related-perspective links clear appropriate filters, keep folding state,
allow canonical inspection and never widen artifact scope; links to records
outside scope leave the complete manifest unchanged. Applying filters resets the
inspector to overview, so the navigation test selects its matching record after
applying filters. No browser product defect was reproduced in these cases.

Local execution evidence (temporary machine-local logs, not historical evidence):

- `/tmp/gdd-review-r2-before.log`: `uv run --no-sync python -m pytest
  tests/test_perspectives.py -k plain_core_rule -q` against base implementation:
  **2 failed, 2 passed**. Failures are the two false global advisories.
- `/tmp/gdd-review-r2-projection.log`: initial corrected projection suite:
  **21 passed** (before adding the second partial-sketch format control).
- `/tmp/gdd-review-r2-browser.log`: **11 checks passed**, Chromium
  **151.0.7922.34**, offline. Command:

  ```sh
  GDD_PLAYWRIGHT_MODULE=/Users/mikkelfreltoftkrogsholm/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright node conformance/run_perspectives_browser.cjs
  ```

- `/tmp/gdd-review-r2-focused.log`: **151 passed in 21.38s**, focused
  product/input/viewer/skill verification before adding the fourth demo control:

  ```sh
  uv run --no-sync python -m pytest tests/test_perspectives.py tests/test_perspectives_inputs.py tests/test_perspectives_viewer.py tests/test_review_viewer.py tests/test_review_viewer_loop4.py tests/test_review_viewer_loop9.py tests/test_inspection_cli.py tests/test_review_cli.py tests/test_review_cli_loop2.py tests/test_review_cli_loop3.py tests/test_skill_assets.py -q
  ```

- `/tmp/gdd-review-r2-demos.log`: the current concert 0.2, concert 0.3 and product
  greenfield artifacts verify **PASS**, so their derived payloads need no
  regeneration. An expanded inventory found the preexisting
  `examples/concert-app/graph.html` at renderer 0.3.0/profile 0.1; its current
  verifier result is **UNKNOWN** (unsupported renderer/profile/version). The
  artifact was byte-identical to the base. The manager confirmed this is a
  documented current presentation, separate from historical captures, and
  authorized regeneration from its unchanged `gdd.json`.
- `/tmp/gdd-review-r2-demo-regeneration.log`: regenerated only
  `examples/concert-app/graph.html` through `render_view` into a temporary new
  output, then replaced the current presentation. Its verifier now reports
  **PASS**. No application/model/evidence/snapshot or historical capture changed.
- `/tmp/gdd-review-r2-current-artifacts.log`: **26 passed in 3.19s**, including
  the new fourth-demo verifier control and the repository's historical concert
  archive integrity checks. Command:

  ```sh
  uv run --no-sync python -m pytest tests/test_perspectives_viewer.py tests/test_concert_archive.py -q
  ```

- `/tmp/gdd-review-r2-skill.log` and `/tmp/gdd-review-r2-diff-check.log`:
  `uv run --no-sync python scripts/sync_skill.py --check` and `git diff --check`
  passed.

The authoritative perspective guide and its portable skill copy were synchronized.
No full-suite, wheel or remote CI execution was performed for this bounded round.

## Round 3: independent canonical and adversarial confirmation

Scope: the shipped product projection, CLI rejection/output boundaries, and the
unchanged offline viewer. Review included the actual implementation and rounds
1–2 changes. No further runtime defect was reproduced within this bounded round.

Fifteen additional deterministic regression cases exercise included fragments
containing product and unrelated custom nodes, legal `constructor` and `prototype`
IDs, Unicode text and integers beyond JavaScript's exact-number range. They
compare canonical fields, directions, parallel edges, exact shown/omitted
partitions and cross-view witnesses; ordering edges between contextual neighbors
remain omitted from Components. Seven independently authored routes between the
same component/entity pair retain their separate explicit usage, ownership,
storage, read, create and change witnesses. Six input-order permutations change
the byte-bound snapshot revision while preserving the complete sorted semantic
projection, including gaps and witnesses. Returned custom records do not alias
the validated bundle, and inspector text retains the large integers exactly.

CLI controls refuse output to the input manifest, its symlink, and a normative
extension asset without changing any source bytes. Semantically identical but
reformatted product definitions, with their correctly updated declared hashes,
remain explicitly unsupported (exit 2). Existing valid partial/global-rule,
invalid targeted-rule/dangling-reference, capacity and mixed-status controls also
passed. No model, normative extension, version or historical evidence was changed.

Local execution evidence (temporary machine-local receipts):

- `/tmp/gdd-review-r3-focused.log`: **92 passed in 14.48s**, including all 15
  new adversarial cases. Command:

  ```sh
  uv run --no-sync python -m pytest tests/test_perspectives_adversarial.py tests/test_perspectives.py tests/test_perspectives_inputs.py tests/test_perspectives_viewer.py -q
  ```

- `/tmp/gdd-review-r3-browser-product.log`: **11 passed**, Chromium
  **151.0.7922.34**, offline. The existing adversarial browser controls confirm
  exact inert hostile text, canonical lens/filter/folding intersections, scoped
  navigation and no network assets or browser errors. Command:

  ```sh
  GDD_PLAYWRIGHT_MODULE=/Users/mikkelfreltoftkrogsholm/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright node conformance/run_perspectives_browser.cjs
  ```

- `/tmp/gdd-review-final-legacy-browser/browser-conformance.json`: the manager's
  original browser suite reports **32 passed**, Chromium **151.0.7922.34**,
  offline, with empty error and request arrays. Command:

  ```sh
  GDD_PLAYWRIGHT_MODULE=/Users/mikkelfreltoftkrogsholm/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright GDD_BROWSER_OUTPUT=/tmp/gdd-review-final-legacy-browser node conformance/run_browser.cjs
  ```

- `git diff --check` passed. All worker-associated tool/test jobs were terminal;
  an overlapping duplicate original-browser run was stopped and is not counted
  as execution evidence.

This confirmation is bounded to the documented scenarios and finite deterministic
permutations, not a guarantee that all bugs are absent. Full-suite, wheel and
integrated release verification remain the manager's final local checks; this
round did not run remote CI or change historical execution records.

### Final integrated acceptance

The manager completed the previously pending final local verification against
source commit `1900e58240f48a29368cb796c727bfafbd2d31bd`. The pending-check note
above describes the earlier handoff state and is now complete. Command run from
`/Users/mikkelfreltoftkrogsholm/Projekter/gdd`:

```sh
uv run --no-sync python scripts/check.py --release --output /tmp/gdd-review-final-release-20261102
```

The actual receipt `/private/tmp/gdd-review-final-release-20261102/summary.json`
reports **PASS** for every release step on host Python **3.12.2**, with no Python
version matrix requested. Its `pytest-current.stdout.txt` records **1,815 passed
in 375.22s**. All nine separate conformance suites passed: standard, graphify,
visualization, workflow, design, code, status, impact and diff. Skill synchronization
and wheel construction also passed. The actual `installed-wheel.stdout.txt` and
`installed-wheel.json` record **37 installed CLI checks**, legacy replay and
**125 exact assets** passing outside the checkout, with `PYTHONPATH` removed and
the standalone skill extracted and verified. The verified wheel is
`gdd_validator-0.8.0-py3-none-any.whl`, SHA-256
`3c870655f3b563484917dde2fd8cbe2ac20eb9c8e130924ce7583fe248bd96c8`.
Together with the **43 offline browser checks (32 + 11)** recorded above, these
complete the agreed integrated local acceptance; no GitHub CI or push occurred.

This final documentation-only update records those completed executions on
`1900e58`; it does not claim tests ran on a later runtime revision. Runtime and
wheel bytes remain unchanged. Only `git diff --check` was run for this final
prose update. All associated verification jobs were terminal at handoff.
