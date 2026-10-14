# Independent existing-code GDD journey

The requested Reservation lifecycle is implemented and accepted in this isolated project. The installed CLI and the standalone installed GDD skill were sufficient to complete import, explicit proposal, refinement, scoped readiness/context, graph generation, work planning, direct implementation, execution evidence and workflow acceptance. No parent checkout or tailored example tools were read. No worker was spawned. All authored artifacts are inside this project.

The current scope is `action.reserve` and `action.cancel`, implementation `implementation.reservation`. `reserve(value: Reservation) -> Reservation` retains its parameter name, parameter kind and annotations. A new Reservation starts in `draft`; reserve mutates draft to reserved and returns the supplied object. Repeated reserve in reserved is harmless. `cancel(value)` changes reserved to cancelled, returns the same object, and is harmless when repeated. Reserve of cancelled raises `InvalidTransition` without mutation. The bounded agent decisions to expose cancel as a function and reject draft cancellation/unsupported states are visible in [requirements.md](requirements.md).

## Observations and accepted intent

The supplied genuine Graphify 0.9.75 AST input, [graphify-input.json](graphify-input.json), remains byte-for-byte unchanged. The agent did not run a new Graphify extraction or authenticate producer version claims. Import explicitly selected `graphify-0.9.75-ast`, producing [observations](observations/README.md). All three nodes and five edges were independently indexed. The two parameter/return type references and constructor call remain separate observations.

[The mapping](mapping-refined.json) interprets the observed module as `component.reservation`, class as `entity.reservation`, and function as `action.reserve`. The selected parameter type observation supports a visibly proposed reads edge; it does not prove current runtime behavior. The observed constructor call conflicts with the human same-object requirement and is omitted from intended relationships. No observation was relabeled as accepted behavior or executed evidence.

The first mapping attempted to reverse observed containment into GDD `owned_by`, which the adapter rejected. The revised mapping preserves endpoint direction and leaves containment in the observation layer. Explicit ownership is authored in the refinement. Both mappings and the failed command remain available. The [proposal](proposal.json) remains `status: proposed`; its [candidate](candidate.json) is preserved separately.

The installed adapter emitted a `gdd/0.2` candidate. I explicitly upgraded the refined intended model to `gdd/0.3`, preserving stable node IDs and observation provenance while adding component boundaries, public interfaces, postconditions, data, lifecycle machine, a blocking behavior rule and six normal/negative/boundary scenarios. [model.json](model.json) is the accepted intended model; [model-change.json](evidence/model-change.json) and `decision.refinement` in [records.json](records.json) record the rationale and changed IDs. The original contract question is resolved through that accepted, trusted current decision. The observation source remains non-normative.

## Real checks and receipts

Substantive CLI operations and application check commands were invoked by [the command recorder](evidence/command.py), with argv, current directory, timestamps, exit code, full stdout and stderr saved in `evidence/*.command.json`. Structured CLI outputs and raw application check receipts are separate files. Inputs, pseudocode and method labels were never executed as commands.

| Assessment | Actual result and evidence |
| --- | --- |
| Supported AST import/validation | PASS: [import report](observations/import.report.json), [verification](evidence/import-verify.report.json) |
| Raw export | PASS: [recovered raw graph](evidence/recovered.graphify.json) is exactly the original 1,978 bytes |
| Unsupported importer profile | UNKNOWN/`FORMAT_UNSUPPORTED`, exit 2: [unsupported report](observations-unsupported/import.report.json); raw integrity remains PASS |
| Unsupported export | Exit 2 retains semantic UNKNOWN while [recovered bytes](evidence/recovered-unsupported.graphify.json) are identical |
| Candidate | Model PASS; readiness FAIL as expected for the unaccepted sketch with missing contracts |
| Refined model and readiness | PASS: [model report](evidence/model.report.json), [trusted scoped readiness](evidence/readiness-trusted.report.json) |
| Scoped context | PASS: [context](context.json), [verification](evidence/context.report.json); context is reading scope, not extra write ownership |
| Graph projection | Generated [offline graph](graph.html), revision verification PASS: [view report](evidence/view.report.json) |
| Work plan | [request](request.json), [plan](plan.json), plan verification PASS; one direct package owns only reservation.py and test_reservation.py |
| Original implementation | Actual FAIL: [red receipt](evidence/lifecycle.red.run.json), 10 tests, 2 failures and 7 errors; the signature assertion already passed |
| Implemented result | Actual PASS: [green receipt](evidence/lifecycle.green.run.json), 10 tests; fixed test bytes match the red run |
| Post-integration result | Actual PASS: [integrated receipt](evidence/lifecycle.integrated.run.json), 10 tests on the current combined code snapshot |
| Negative controls | Both actual mutants fail: [control results](evidence/negative-controls.json); constructor-return causes 2 identity failures, cancelled-reopen causes 1 missing-rejection failure |
| Four-principle advisory review | Performed against actual code and graph IDs: [review](evidence/design-review.json); applicability COMPLETE, separate from behavior acceptance |
| Evidence applicability | Implementation PASS with current verified code/test bindings: [implementation report](evidence/implementation.report.json) |
| Acceptance authorization | GRANTED under the separate trusted human mandate: [authorization report](evidence/acceptance-authorization.report.json) |
| Workflow acceptance | Dispatch, submit, integrate and accept all applied once; [status](evidence/accepted-status.json) is accepted with `acceptance_current: true` and four events |
| Final integrity audit | PASS: [final audit](evidence/final-audit.json) checks current source snapshots, unchanged raw bytes and observation identities, reports and acceptance |

The source before implementation is preserved as [reservation.before.py](evidence/reservation.before.py). The implementation is [reservation.py](reservation.py), regression assertions are [test_reservation.py](test_reservation.py), and the actual diff is [implementation.patch](evidence/implementation.patch). The current agent performed the dispatched package directly. Submit is a local delivery claim; integration records were registered only after inspecting the actual patch, declared file ownership, tests and advisory review. Acceptance uses a fresh post-integration receipt, not the earlier green result.

## Revisions

| Basis | Revision |
| --- | --- |
| Preserved Graphify artifact | `sha256:f4c7a0d19f6cf42579a30b5d474dd6fd1752f2197b2c4af6d16447136a7cffdb` |
| Unaccepted candidate model | `sha256:5d9ccb6dc70244a7ef1042c0909f915dee7668981f029c74f6ed8bd43d8df4ef` |
| Accepted GDD 0.3 model | `sha256:eb3b8706abbc89cd6f4ffab94086b43a79a13cb8982b3ea186258a0c71fec429` |
| Initial implementation selection | `sha256:f9f7e6003e9e46d3dd865759ffc2522688467e5e115f98e91f0b6b2c56f7d282` |
| Combined implementation/test result | `sha256:161310c70013fd962fd6ae736ce194881d933db181ae4442978d302c2e31f0c9` |

[Environment metadata](evidence/environment.json) identifies the installed `gdd-validator` 0.6.0, Python executable/version, platform, skill path/hash and consulted references. These are actual local execution facts. The workflow and host context are local audit attestations, not authenticated third-party evidence.

## Usability findings and interventions

1. `graphify propose` requires preserving observation endpoint identities and direction. The bundled mapping schema accepts independently selected endpoints and the route did not mention this restriction. A natural contains-to-owned_by reversal fails with “Edge mapping must preserve the selected endpoint identities and direction.” [Failed command](evidence/graphify-propose.command.json). The recovery was bounded: omit those observation edges, then author ownership explicitly in refinement. Add the restriction and this containment example to the installed guidance/schema documentation.
2. `graphify propose` produces a `gdd/0.2` model candidate even though the installed skill entry routes new intent toward GDD 0.3. This is supported legacy output rather than a runtime blocker, but the guide should name the deliberate upgrade step or the adapter should offer an explicit candidate format selection. The preserved proposal demonstrates the actual output.
3. The first authored rule used unsupported category `business`; schema rejected it. I consulted the installed schema and repaired it to `behavior`, preserving [the rejected model](evidence/model.invalid-rule.json) and [failed snapshot report](evidence/model.snapshot.json). This was an agent drafting error, not a validator defect.
4. The first direct `append_records` call passed a raw map instead of a fragment containing `records`; it failed before mutation. I repaired the envelope and retained [the failed invocation](evidence/work-preparation.command.json). The supplied HOST example passes the helper-returned fragment correctly; this was an authoring error. A small documented example for manually authored check/decision fragments would reduce discovery effort.
5. Without host trust for the current resolution decision, readiness correctly remained UNKNOWN. Supplying the separately attested current mandate/decision produced scoped PASS. No graph label or producer claim was used to grant authority.

There is no remaining implementation or acceptance blocker in the agreed scope. The generated HTML was revision-validated; browser interaction was not exercised. The lifecycle is sequential and in memory. Persistence, concurrent access guarantees and identity permissions remain explicitly outside scope. No historical or failed result was relabeled as a fresh successful execution, and no requirement or expected assertion was weakened to obtain PASS.
