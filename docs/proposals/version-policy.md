# Proposed compatibility and 1.0 policy

**Status: PARTIALLY ADOPTED — GDD 0.2 lifecycle, independent contract versions
and Python verification policy accepted October 20, 2026; 1.0 criteria pending.**
This is the bounded design response to
[issue #15](https://github.com/mikkelkrogsholm/gdd/issues/15), originally based on
package 0.8.0 development at `b5bfcce`. [RELEASING.md](../../RELEASING.md)
records the three active decisions. Only the 1.0 readiness criteria remain
proposed. No version, supported format, interpreter requirement or normative
asset bytes change as a result of these decisions.

## Decisions recommended

| Decision | Recommendation | Practical consequence if accepted |
| --- | --- | --- |
| GDD 0.2 lifecycle — ADOPTED October 20, 2026 | Keep full explicit 0.2 validation and legacy replay throughout the package 0.x series. | Do not reduce support to loading or silently migrate inputs. Reconsider the 1.0 support set in a separate explicit decision. |
| Envelope versions — ADOPTED October 20, 2026 | Keep versions independent and maintain the single compatibility inventory in RELEASING. | Change only affected contracts; avoid a forced simultaneous protocol bump. |
| Python support — ADOPTED October 20, 2026 | Keep `>=3.11` installation eligibility and document actual test coverage per release. | A new minor line requires full local tests, conformance and installed-wheel verification before being declared supported; focused coverage alone is not admission evidence. |
| 1.0 readiness | Use the observable gates below; decide package and standard readiness separately. | A package 1.0 tag cannot imply a standard 1.0 or complete runtime coverage. |

These are four independent decisions. Accepting one does not adopt the others.

## Compatibility inventory

The adopted [single compatibility inventory](../../RELEASING.md#compatibility-inventory)
is maintained in RELEASING.md. It lists exact current contracts and their version
triggers; this proposal does not maintain a second copy. The adopted rules for
independent versions, public contracts and preserved historical identities are
recorded alongside that inventory.

## Legacy lifecycle

Keep the existing 0.2 schemas, version dispatch, explicit-input behavior and
installed-wheel legacy replay throughout the package 0.x series, as adopted October 20, 2026. Maintain
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

The adopted [Python compatibility policy](../../RELEASING.md#python-compatibility--adopted-october-20-2026)
keeps `>=3.11` metadata and requires actual release coverage and local admission
checks. The table above is historical focused/full evidence, not a declaration
that all four minor lines passed full admission. More recent
[tutorial verification](../../evidence/tutorial-0.1/README.md) records 1,723 full-suite
tests on Python 3.12.12 and 356 focused tests on each of the four exact
interpreters, plus the composed conformance and corrected wheel evidence.
Neither adopting the policy nor documenting focused coverage creates missing
full-suite or per-interpreter wheel executions.

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

The maintainer accepted the GDD 0.2 lifecycle on October 20, 2026: full
0.2 support throughout package 0.x; the 1.0 support decision remains deferred.
The maintainer also accepted independent contract versions and a single
compatibility overview on October 20, 2026. RELEASING.md contains that inventory.
The maintainer accepted the Python verification policy on October 20, 2026:
keep `>=3.11`, report actual coverage and require full local tests, conformance
and installed-wheel verification before declaring a new minor line supported.
No runtime implementation, version bump or metadata change is required for these
three adopted decisions.

After discussion, record accept/revise/defer for the remaining 1.0 criteria,
the approved wording and the deciding date in RELEASING.md. Update this
proposal's status and issue #15 accordingly. File concrete follow-up issues only
for adopted changes that require implementation; do not close #15 while its
required maintainer decisions are pending.
