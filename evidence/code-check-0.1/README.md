# Static Python architecture observation

This review records the local implementation of issue #3 in the unreleased
package 0.8.0. `gdd check-code` and `gdd_validator.check_code` compare current
Python source dependencies with explicit GDD forbid-import constraints and
implementation-specific binding records. The [profile guide](../../docs/python-imports.md)
sets the exact scope and limitations.

## Observations and trust

The new `gdd-code-observation/0.1` is an informative tool envelope, separate from
frozen `gdd-report/0.2`. It retains actual source/model/binding identities,
imports, shortest positive-length witnesses, unresolved coverage and advisory
findings. It executes no project modules or model text, writes no trusted
records, and grants no authorization or implementation acceptance.

Planned records supply explicit mapping intent. Observed/checked resource claims
are compared with current bytes; their status and code revision are not accepted
as proof. Missing, historical, ambiguous, unsupported, excluded or changing input
cannot produce absence-based PASS. A definite forbidden in-profile path takes
FAIL precedence while uncertainty remains visible.

## Actual local verification

The [verification record](verification.json) identifies the clean source commit,
per-file hashes, baseline, preserved assets and exact wheel digest. The
[full release summary](release/summary.json) records the actual commands and logs:

- Entire suite: **1,635 tests passed** on Python **3.12.12**.
- [Focused matrix](quick-matrix/summary.json): **268 tests passed** separately on
  Python **3.11.11, 3.12.12, 3.13.13 and 3.14.7**, including all 114 observer tests.
  This is focused cross-version coverage, not four full-suite executions.
- Standard, Graphify, visualization, workflow, design and
  [code observation](release/code.json) named conformance runners passed.
- The [fresh installed-wheel receipt](release/installed-wheel.json) retains
  source/module/console parity, exact packaged assets, the existing legacy replay
  and actual clean/injected/repaired/missing observer controls outside the checkout.

The new code conformance covers 28 named cases. It copies real concert
application Python source into a temporary root, authors current mapping records,
then observes **PASS → FAIL → PASS → UNKNOWN**, preserving original input bytes.
Independent nonconcert fixtures demonstrate there is no built-in component map.
Direct/transitive/self paths, bindings/currentness, parsing/dynamic imports,
unbound code, exclusions, identity collisions, no-execution sentinels, output
schema and exclusive output have separate meaningful controls.

Existing standard/core schema, model, example HTML, archive and historical
execution bytes were preserved. No new interactive browser test is claimed;
renderer code and existing example artifacts were unchanged. Verification is
local to macOS; no GitHub Actions, registry publication or release tag was added.

## Review and diagnosed failures

Independent review reproduced two implementation defects: frozen-module names
could be mistaken for local source, and excluding a known local dependency could
turn UNKNOWN into PASS. The retained [red execution](gdd-code-review-red.txt)
shows five actual failing instances. The [green execution](gdd-code-review-green.txt)
shows the then-existing 109 observer tests passing after repair. The final matrix
adds legacy and strict digest controls. Seven targeted independent controls
confirmed both fixes, including local relative imports and unrelated exclusions.

The [first full-suite receipt](schema-anchor-failure/summary.json) remains FAIL:
1,630 passed and one existing global schema check caught permissive end anchors
in the new schema. The patterns were repaired and four functional controls now
reject trailing newlines in file, binding, model and code digest identities.
That earlier execution has not been relabeled as PASS. The final receipts above
identify the actual corrected source; previous evidence remains historical.
