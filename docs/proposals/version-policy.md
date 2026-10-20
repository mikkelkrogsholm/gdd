# Proposed compatibility and 1.0 policy

**Status: PROPOSED — maintainer decisions pending.** This is the bounded design
response to [issue #15](https://github.com/mikkelkrogsholm/gdd/issues/15), based on
package 0.8.0 development at `b5bfcce`. It records recommendations, not adopted
support promises. [RELEASING.md](../../RELEASING.md) remains the release policy.
No version, supported format, interpreter requirement or normative asset changes
as a result of this proposal.

## Decisions recommended

| Decision | Recommendation | Practical consequence if accepted |
| --- | --- | --- |
| GDD 0.2 lifecycle | Keep full explicit 0.2 validation and legacy replay through the pre-1.0 series. | Do not reduce support to loading or silently migrate inputs. Reconsider the 1.0 support set in a separate explicit decision. |
| Envelope versions | Keep versions independent and maintain the compatibility inventory below. | Change only affected contracts; avoid a forced simultaneous protocol bump. |
| Python support | Declare tested minor lines 3.11–3.14, with exact patches and coverage attached to each release. | Admission of a new line requires local focused, full-suite, conformance and installed-wheel verification before calling it tested. |
| 1.0 readiness | Use the observable gates below; decide package and standard readiness separately. | A package 1.0 tag cannot imply a standard 1.0 or complete runtime coverage. |

These are four independent decisions. Accepting one does not adopt the others.

## Compatibility inventory

This compact inventory describes current contracts, not an exhaustive list of
all serialized artifacts. Exact `format` or profile identifiers remain the
compatibility selectors; a package version is not a substitute.

| Surface | Current contract | Version trigger |
| --- | --- | --- |
| Package / CLI / public Python API | 0.8.0, unreleased | Distributed behavior and API compatibility; package release policy in RELEASING |
| Model and fragment | `gdd/0.3`, `gdd-fragment/0.3`; explicit 0.2 support | Changed interpretation or required structure needs a new format version |
| Records, report, snapshot, context, parallel plan | Respective `gdd-…/0.2` formats | Only the affected serialized contract changes version |
| Workflow request, plan, run and run status | Respective `gdd-…/0.1` formats | Only the affected workflow contract changes version |
| Status / impact | `gdd-status/0.1`, `gdd-impact/0.1` | Projection/output contract changes independently of core graph semantics |
| Code observation / Python observer | `gdd-code-observation/0.1`, `python-imports/0.1` | Observation structure and analysis profile are separate compatibility surfaces |
| Boundaries / design review | `gdd-boundaries/0.3`, `gdd-design-review/0.3` | Affected output or design interpretation contract changes |
| Graphify adapter | Named `graphify-0.9.75-ast` and `graphify-0.9.75-node-link` profiles | A new source dialect needs an explicit profile; unsupported input stays visible |
| Viewer | Renderer 0.3.0; `gdd-viewer-html/0.1`; `gdd-view/0.2` | Renderer implementation, verification profile and serialized projection version independently |

New tool formats should document their own scope and compatibility instead of
inheriting the standard version by convention. A defect fix that restores the
existing documented contract does not itself require a new format identifier.
Normative asset byte changes still affect revision identity even when a version
identifier remains appropriate. Preserve historical reports and snapshots;
never relabel their digests or claim they were executed against a new release.

## Legacy lifecycle

Keep the existing 0.2 schemas, version dispatch, explicit-input behavior and
installed-wheel legacy replay while the recommendation above applies. Maintain
fixes necessary to honor the already supported interpretation. New 0.3-only
features need not be backported into the 0.2 format.

Before any later reduction or removal, record the affected commands and formats,
a migration recipe with preserved source provenance, a pinned replay route for
historical evidence, the first release announcing deprecation and the proposed
removal release. The maintainer must approve that lifecycle before it is enacted.
Until then, neither a future package tag nor a newer standard draft removes 0.2.
Do not open a removal or consolidation implementation issue unless that change
is actually selected; retaining today's support needs no removal work.

## Python verification and admission

The latest completed [inspection verification](../../evidence/inspection-0.1/README.md)
establishes the following **historical baseline**, not an execution of this
proposal or a promise about every future patch or platform:

| Interpreter | Recorded coverage |
| --- | --- |
| Python 3.12.12 | Full suite: 1,692 tests; eight conformance runners; installed wheel: 28 CLI checks |
| Python 3.11.11, 3.12.12, 3.13.13, 3.14.7 | Focused matrix: 325 tests on each exact interpreter |

The package metadata `requires-python = ">=3.11"` describes installation
eligibility. It does not verify all future Python versions. The recommendation
is to keep that metadata and describe verification separately, rather than add
an untested upper bound. Existing resolved Python defects are not current
failures; the receipts above supersede the original issue's old failure counts.

For each release, identify the host platform, exact interpreter patches, full
versus focused coverage and installed-wheel results. Run the current local
release checks and focused matrix. A new minor line may be added to the declared
tested range after its full suite, conformance runners and fresh installed-wheel
checks also pass locally; retain those receipts and update the matrix list.
Focused checks on an admitted line are continuing regression coverage, not a
claim of a fresh full execution. Missing interpreters or failed checks remain
visible. No GitHub CI is required or proposed.

## Observable 1.0 criteria

Before proposing **standard 1.0**:

1. Complete a bounded external codebase trial against a stated baseline. Publish
   the task, initial graph, requirement change, deliberate architecture failure,
   outputs, outcomes, costs and limits. A negative result needs an explicit
   maintainer disposition rather than being omitted.
2. Complete a reproducible standalone-skill workflow in a second agent runtime,
   covering planning, validation, change and honest reporting of missing evidence.
3. Resolve known planned breaking decisions about core identity, relations,
   contracts, extension semantics and evidence interpretation. List any remaining
   experimental profiles with their explicit boundaries.
4. Freeze and document the intended core conformance contract and the initial
   code-check profile (`python-imports/0.1` or an explicit successor). Verify
   meaningful PASS, FAIL and UNKNOWN controls and migration examples locally.
5. Decide and publish the legacy support set, deprecation rules and compatibility
   commitments. Draft status cannot be removed solely because tests pass.

Before proposing **package 1.0**, additionally verify that the documented CLI,
public APIs, exact wheels and standalone skill agree, complete the local release
checks, and publish the supported-runtime coverage and migration guide. Package
1.0 can intentionally ship a draft standard only if that distinction is explicit
and the promised package interfaces are stable. Standard 1.0 does not require
all optional adapters or all programming languages to be supported.

The external trial (#9) and second-runtime work (#10) are evidence inputs to
these decisions. They are not completed by this document. PyPI publication
(#17), release tagging and any support removal remain separate maintainer
choices; this proposal authorizes none of them.

## Decision record to complete

After discussion, record accept/revise/defer for each of the four decisions
above, the approved wording and the deciding date in RELEASING.md. Update this
proposal's status and issue #15 accordingly. File concrete follow-up issues only
for adopted changes that require implementation; do not close #15 while its
required maintainer decisions are pending.
