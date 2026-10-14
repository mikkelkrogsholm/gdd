# Migration from gdd/0.1

This chapter is informative. 0.2 is an explicit format revision, not a transparent change to the old contract.

## Preserved foundation

The previous [SPEC](../archive/gdd-0.1/SPEC.md) and [model](../archive/gdd-0.1/concert.gdd.yaml) are preserved as historical 0.1 material, originally copied byte-for-byte and now presented in English. The root `concert.gdd.yaml` remains 0.1. The new example is in [examples/concert.gdd.yaml](../examples/concert.gdd.yaml).

Stable IDs, the meaning of the original behavior text, global security principles and existing scenarios are preserved. Old linter results are not reused as 0.2 results.

## Structural mapping

| 0.1 | 0.2 |
| --- | --- |
| nodes with component/entity/action | nodes with the same ID and kind |
| rules registry | rule nodes with the same ID, payload in rule |
| scenarios registry | scenario nodes with the same ID, payload in scenario |
| machines registry | machine nodes with the same ID; transitions retain their IDs |
| owner field | explicit owned_by edge with a new edge ID |
| stored_in field | explicit stored_in edge |
| rule.targets | constrains edges |
| scenario.actions/verifies | exercises/verifies edges |
| machine.entity | state_of edge |
| action.specification | behavior; the meaning of the original text is preserved |
| action input/context/returns/errors | abstracted contract fields; output is a concrete data contract |
| bindings | separate planned binding records |
| verification_policy | the standard's results, check plans and the project's configurable workflow policy |

0.1 imports/calls edges retain their IDs and direction. New core relations replace the previously embedded ownership/coverage references; the old and new representations are not maintained in parallel.

## Explicit additions in the example

Abstract types make the principal's trusted origin and output fields visible. Actions gain declared effects, risk assessments, error_cases and new relevant scenarios. These are specification refinements, not evidence of behavior. Cancel gains a separate normal flow and a concurrency scenario. The previous idempotent and atomic behavior is preserved.

Frontend/community nodes are new sketches. A planned DESIGN.md reference field does not provide an existing design asset. Communities do not provide evidence of parallelism.

The previous strict acceptance profile can be configured in a concrete project with required checks and gates. 0.2 does not impose a universal human approval mode on every project. Blocking rules can be accepted only with PASS or a visible policy-permitted exception; the results are not changed.

The `check_spec.py` included in the zip file is a 0.1 linter and must not be called a 0.2 validator. A future migration must validate both formats with their own contracts and report loss, ambiguities and changed requirements.


## Migration from gdd/0.2 to gdd/0.3

The new format is explicit. The [0.2 contract](../archive/gdd-0.2/README.md) and
existing examples/native executions remain preserved; the tool supports their
original schemas and semantics. The new example is
[concert-0.3.gdd.yaml](../examples/concert-0.3.gdd.yaml).

1. Change the model and each included fragment to their respective 0.3 formats.
2. Author responsibility, rationale and exclusions for every component boundary
   needed by the selected slice. A tool must not invent rationales from clustering.
3. Make each public action's owner match its exporting component and ensure resolved
   cross-component calls use the provider's public interface. The migrated concert
   example removes API's duplicate reservation exports; Reservations remains their
   authoritative provider. Forwarding operations require separate action contracts.
4. Add successful-return postconditions to relevant action contracts. Preserve errors,
   effects, risk assessments, scenarios and parent obligations; new prose is intent,
   not evidence of execution or semantic equivalence.
5. Optionally label broad nonblocking design rules with a principle. Keep precise
   architecture/design requirements and their blocking status intact.
6. Regenerate current snapshots, planned record references, packets and views. Run
   relevant conformance and regression checks. Old records and evidence retain their
   original identities and revisions; do not rebind historical execution claims.

Records, extensions, snapshots, reports, context and parallel-plan envelopes remain
at 0.2; workflow request/plan/event envelopes remain at 0.1. The containing snapshot
identifies the 0.3 model. New boundary and advisory review projections use explicit
0.3 identifiers. Neither an old envelope number nor a review COMPLETE status supplies
implementation acceptance. Unsupported model versions remain UNKNOWN.
