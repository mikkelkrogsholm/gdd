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
