# Review of issues 1–19

Reviewed October 20, 2026 against `73494ed9933f5df7e663a0dec6a89aa29b10886a`.
All 19 issues were open when retrieved. Their complete descriptions and the
one existing comment, on issue 4, were read. This is an advisory review, not an
approved implementation roadmap. No GitHub issue was edited or commented on.

## Assessment

The review identifies real portability failures and useful adoption work. Its
strongest recommendations preserve GDD's existing boundaries: observations do
not become intent automatically, and model validity, readiness, authorization
and implementation evidence remain separate assessments.

The backlog needs prioritization before execution. It combines correctness
repairs, usability changes, product experiments, publication decisions and
optional cleanup. Completing every proposed command and format before an
external trial would expand the tool before demonstrating that users need the
additional surface.

## Independently checked behavior

The targeted tests ran in isolated environments without replacing the project's
existing virtual environment. No full-suite matrix was rerun for this review.

| Runtime | Selected tests | Observed result |
| --- | --- | --- |
| CPython 3.12.12 | Parser loop 8, two Graphify depth tests, protocol loop 9 | 25 failed, 111 passed |
| CPython 3.13.13 | Four symlink-loop tests listed in issue 2 | All four failed |
| CPython 3.12.2 | Three parser-capacity cases and the four symlink-loop cases | All seven passed |

The first result confirms that issue 1 also occurs on an installed patch older
than the issue's 3.12.15 example. The four symlink failures reproduce the stated
missing exception and UNKNOWN-versus-FAIL classification changes. These tests
do not establish a filesystem escape or an ordinary-model validation failure.

The starter was also initialized in a temporary directory. Its scoped readiness
report contained 14 checks and 11 diagnostics. The generic missing-contract,
missing-interface and missing-boundary messages described in issues 4 and 5
are present. The current Graphify proposal's hard-coded 0.2 candidate and the
stale 0.4.1 package reference in GRAPHIFY.md were confirmed in source.

Reproduction commands from the repository root:

```sh
uv run --isolated --locked --group dev --python /opt/homebrew/bin/python3.12 \
  pytest -q tests/test_review_parser_loop8.py \
  tests/test_review_graphify.py::test_strict_json_deep_nesting_has_a_controlled_error \
  tests/test_review_graphify_loop2.py::test_proposal_api_depth_exhaustion_is_a_controlled_error \
  tests/test_review_protocol_loop9.py --tb=short

uv run --isolated --locked --group dev \
  --python /Users/mikkelfreltoftkrogsholm/.local/share/uv/python/cpython-3.13-macos-aarch64-none/bin/python3.13 \
  pytest -q \
  tests/test_review_graphify.py::test_looping_bundle_root_is_a_controlled_invalid_asset \
  tests/test_review_graphify.py::test_looping_model_manifest_has_a_structured_public_report \
  tests/test_review_graphify.py::test_looping_raw_symlink_is_a_controlled_invalid_asset \
  tests/test_review_protocol.py::test_secondary_record_loop_returns_controlled_diagnostic --tb=short

.venv/bin/python -m pytest -q \
  tests/test_review_parser_loop8.py::test_actual_parser_recursion_is_format_unsupported \
  tests/test_review_graphify.py::test_looping_bundle_root_is_a_controlled_invalid_asset \
  tests/test_review_graphify.py::test_looping_model_manifest_has_a_structured_public_report \
  tests/test_review_graphify.py::test_looping_raw_symlink_is_a_controlled_invalid_asset \
  tests/test_review_protocol.py::test_secondary_record_loop_returns_controlled_diagnostic --tb=line
```

Interpreter paths identify the actual review environment; use corresponding
installed interpreters when reproducing elsewhere. Issue descriptions' full
3.11–3.14 suite counts remain their authors' reported executions.

## Disposition of every issue

| Issue | Recommendation | Scope or qualification |
| --- | --- | --- |
| [1: Parser depth](https://github.com/mikkelkrogsholm/gdd/issues/1) | Fix first | Confirmed portability and test defect. Define an explicit consumer capacity, preserve UNKNOWN beyond it, and test its boundary. Do not make Python's stack size the format rule. |
| [2: Symlink loops](https://github.com/mikkelkrogsholm/gdd/issues/2) | Fix first | Confirmed on 3.13.13. Normalize loop detection while preserving distinct missing-file and containment behavior; changing every resolution to strict mode indiscriminately would change those contracts. |
| [3: Code checker](https://github.com/mikkelkrogsholm/gdd/issues/3) | High product value; narrow and specify first | Generalize explicit Python import constraints and current bindings. Clarify the observation report, code identity and host evidence integration before implementation. This is not a universal graph/code equivalence checker. |
| [4: Missing-field diagnostics](https://github.com/mikkelkrogsholm/gdd/issues/4) | Accept as a small contribution | Confirmed usability issue. Start with precise messages and pointers, retaining codes and assessment results. Machine-readable details require a separate report-contract decision. A contributor has already expressed interest. |
| [5: Readiness checklist](https://github.com/mikkelkrogsholm/gdd/issues/5) | Combine discovery with 4; defer a new envelope | First improve the existing report. A text/explain projection may be sufficient. Avoid duplicating readiness rules, inventing a second readiness score or assuming every graph has a total dependency order. |
| [6: Model diff](https://github.com/mikkelkrogsholm/gdd/issues/6) | Accept a smaller first version | ID-based added/removed/changed summaries are useful. Separate semantic changes from byte revisions; treat weakening hints as review triggers, and remove the promise to infer historical ID reuse from two snapshots. |
| [7: Graphify to 0.3](https://github.com/mikkelkrogsholm/gdd/issues/7) | Prioritize | The documented 0.2 candidate is an existing limitation, not an accidental invalid result. Add explicit target-format selection, preserve 0.2 behavior and produce honest unready 0.3 sketches without invented contracts. |
| [8: Authoring language](https://github.com/mikkelkrogsholm/gdd/issues/8) | Defer pending observed need | A compiler creates a second language, identity mapping and maintenance obligation. Measure real authoring friction first. Never manufacture risk rationales merely to fill required fields. |
| [9: External trial](https://github.com/mikkelkrogsholm/gdd/issues/9) | Start early | Highest-value method validation. Begin with a bounded pilot and common independent acceptance checks. A generic code checker is useful but should not block the trial; a frozen shared checker can serve both arms. |
| [10: Second runtime](https://github.com/mikkelkrogsholm/gdd/issues/10) | Prioritize adoption validation | Verify actual skill loading and both journeys in another runtime. Keep design independence distinct from demonstrated runtime coverage. Record interventions and outcomes honestly. |
| [11: Tutorial and root docs](https://github.com/mikkelkrogsholm/gdd/issues/11) | Split | Prioritize the small executable tutorial. Treat moving historical notes as optional cleanup. Five minutes and approximately 40 edited lines should not be acceptance gates for a full two-component implementation/evidence journey. |
| [12: Version prose](https://github.com/mikkelkrogsholm/gdd/issues/12) | Fix alongside correctness work | Confirmed stale reference. Removing an unnecessary package version may be simplest. Check intentional current-version statements without coupling tests to every historical number or incidental sentence. |
| [13: Evidence curation](https://github.com/mikkelkrogsholm/gdd/issues/13) | Useful, lower urgency | Preserve recoverability and distinguish a smaller working tree/source archive from a smaller full Git clone. There are indirect test dependencies on evidence citation targets. Start by moving browser output defaults outside tracked evidence. |
| [14: Test organization](https://github.com/mikkelkrogsholm/gdd/issues/14) | Gradual maintenance | The cross-test imports and review-round naming are real. Reorganize bounded areas as they are touched; preserve assertions, parametrizations, coverage and historical mapping. Equal collection counts alone do not prove equal coverage. |
| [15: Version policy](https://github.com/mikkelkrogsholm/gdd/issues/15) | Extend existing policy | RELEASING.md already covers several decisions. Add tested Python coverage, legacy lifecycle and 1.0 criteria there. Preserve explicit 0.2 support until an actual deprecation decision; do not synchronize unrelated envelope versions solely for visual consistency. |
| [16: Local check entry](https://github.com/mikkelkrogsholm/gdd/issues/16) | Accept a lightweight version | Fits the no-GitHub-CI decision. Use fresh output and isolated interpreters. Separate routine checks from a full release/matrix run rather than labelling repeated full-suite and conformance execution as fast. |
| [17: PyPI](https://github.com/mikkelkrogsholm/gdd/issues/17) | Prepare after repairs; publication remains a decision | Keep the established distribution name unless a concrete naming problem justifies changing it. Verify release artifacts, including the sdist installation path, and recheck name availability at publication time. |
| [18: CLI handlers](https://github.com/mikkelkrogsholm/gdd/issues/18) | Optional enabling refactor | Small functions in the existing module can help when adding a command. Preserve command-specific output and exclusive-create semantics. A 40-line main or one-line registration target is not a product acceptance criterion. |
| [19: Roadmap](https://github.com/mikkelkrogsholm/gdd/issues/19) | Retain as an index; revise sequencing | Separate bugs, validated adoption work, hypotheses and cleanup. Do not turn all 18 underlying proposals into prerequisites for a useful release or external trial. |

## Corrections needed in the larger proposals

### Code observation is not yet a specified report contract

Issue 3 says that no format change is required, but the existing
[report schema](../schemas/report.schema.json) permits only the current profile
enumeration. A new observer profile needs an explicit representation there or
a separate observation result contract. An implementation-dimension PASS/FAIL
also requires `implementation`, `code_revision` and `evidence_refs`.

Define where static observation results live and how the trusted host turns an
actual execution into revision-bound evidence. The existing
[host API](../gdd_validator/host.py) already separates execution from the host's
coverage and negative-control attestations; keep that distinction. Clarify
selection of current bindings, historical/superseded records, unsupported
features and the analyzed source boundary. Schema changes must be versioned
and propagated according to their actual compatibility impact.

Keep the first observer limited to explicit static Python import constraints.
An absence of violations within that supported analysis is not proof of all
calls, runtime behavior, KISS, DRY or modularity.

### A two-snapshot diff cannot establish every claimed weakening

Issue 6 should distinguish three kinds of output:

1. Exact structural changes by stable ID.
2. Candidate risk indicators requiring human or agent review.
3. Missing or unsupported input that prevents a reliable comparison.

Two endpoint snapshots cannot reveal that an ID was deleted and reused between
them. Changed prose cannot mechanically establish whether its meaning became
weaker. Endpoint changes and removed preconditions are not universally weaker
requirements either. Describe them as review-sensitive changes.

The loader's `canonical()` sorts object keys but preserves array order; a
semantic comparison must additionally respect the standard's order-independent
edge list. Reformatting and fragment relocation can change snapshot identity
while leaving the logical graph unchanged. Show those facts separately.

For invalid or partially loaded inputs, preserve known FAIL diagnostics and
mark comparison coverage incomplete. Do not infer deletions merely because a
fragment could not be loaded.

### Curation must preserve actual consumers and realistic size claims

Issue 13's statement that no test reads evidence is too broad.
[The skill test fixture](../tests/test_skill_assets.py) copies every optional
repository citation target, including evidence paths. The current referenced
targets are summaries and one conformance JSON, so this need not block curation;
it does require an inventory and verified navigation after moves.

Removing files from the latest tree does not remove their blobs from existing
Git history. Bundles or external release assets can improve the working tree,
source archives and future growth without shrinking an ordinary full-history
clone. Keep historical claims bound to their original executions.

### Make validation cheaper before adding more authoring machinery

Issues 4 and 5 describe the same immediate refinement friction. Improve that
existing path before introducing a new stable checklist format. For issue 16,
the workflow and design conformance runners already invoke pytest selections;
running all of those after a full suite repeats work. A thorough release mode
can be appropriate, but routine contributor checks should remain proportionate.

The existing public 0.7.0 release and all older verification remain historical
facts. The newly reproduced interpreter failures limit the tested portability
claim; they do not erase the successful earlier runs on their recorded runtimes.

## Recommended next boundary

1. Repair 1 and 2, correct 12, and add the smallest useful local check entry from
   16. Verify the admitted Python runtimes without adding GitHub Actions.
2. Complete 4 as a bounded contribution and improve the existing-code entry
   through 7. Add a concise executable tutorial from 11.
3. Specify and build the narrow observer from 3 while beginning the external
   pilot in 9 and the second-runtime journey in 10. Do not wait for all proposed
   commands or cleanup to finish before learning from these trials.
4. Use those observations to choose the scope of 5, 6 and 8. Schedule 13, 14 and
   18 when their demonstrated maintenance benefit justifies the change. Record
   decisions from 15 and obtain the maintainer's publication decision for 17.

The immediate product goal remains an agent-readable software planning method,
CLI and skill with verifiable contracts, rather than a larger command catalog.
