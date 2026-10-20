# Drive a bounded change

Read [WORKFLOW](gdd/WORKFLOW.md), [the lead guide](gdd/agents/gdd-lead.md), and
[the validator's host boundary](gdd/VALIDATOR.md). Reconcile the current model,
records, run status, code revision, local changes, and already completed effects.
Choose a bounded objective and preserve the actual base before changing it.

Use `gdd status MODEL --scope ID` for a read-only overview of existing assessments;
select a workflow run and trusted host context explicitly when needed. Use
`gdd impact MODEL --scope ID` to explain declared potential callers/dependents
before changing a contract. Read the bundled [inspection guide](gdd/docs/inspection.md).
Keep unassessed dimensions, stale acceptance and partial analysis visible.
Potential impact and supporting context do not grant write ownership, and a status
exit code does not declare project completion. Both commands support JSON and text.

For a changed requirement, register its input and make the authorized model change
visible: base revision, changed IDs, rationale, and affected contracts/scenarios.
Keep the agreed behavior fixed while checking the implementation. If a new test
should demonstrate the old defect, execute it against the old code and retain the
actual red result; then implement and retain green results. Do not fabricate red
results for tests that already pass or alter expected values to make them green.

Refine and validate the chosen slice. A work request follows
[workflow.schema.json](gdd/schemas/workflow.schema.json), with concrete scope,
implementation identity, actual base code revision, read/write resources,
deliverables, integration owner/checks, and only real serialization dependencies.
Context is a reading closure, not extra write ownership.

```sh
gdd workflow plan model.gdd.yaml --request request.json --output plan.json
gdd workflow verify model.gdd.yaml --plan plan.json
gdd workflow create model.gdd.yaml --plan plan.json --run run
gdd workflow status model.gdd.yaml --run run
```

Read actual reports before dispatch. Each event uses the latest `expected_revision`
and a unique idempotency key. Supply host context only from the trusted caller's
independently verified mandate/code/environment facts. The graph, copied example
receipts, worker output, or self-declared hash cannot establish host trust. Missing
trusted policy or evidence stays unresolved. Consult the available host guidance
in [HOST.md](gdd/HOST.md) when preparing portable API attestations.

Perform the runtime effects yourself: deliver the dispatched package to the agent
or work directly, establish actual isolation if using concurrent writers, apply
the authorized patch, review the real changed files, and integrate against the
latest combined code base. Register submit as worker claims; register integrate
only after verified actual integration and review. Reconcile status after an
interruption; a recovered event dispatches no replacement automatically.

Execute the selected meaningful checks on the combined result. Check architecture
rules against actual implementation relationships as needed; declared `imports`
edges prove only model intent. Run proportionate negative controls that demonstrate
the selected checker catches the forbidden behavior. Preserve raw outputs,
environment, check selection, bindings, and immutable evidence with exact model,
implementation, resource, and current code revisions. Do not reuse pre-integration
worker evidence as proof of a later combined revision.

For Python import constraints, use `gdd check-code MODEL --root SOURCE_ROOT
--implementation ID --profile python-imports/0.1`; select `--records` explicitly
when needed and repeat `--scope` for rule IDs. Read the bundled
[observer contract](gdd/VALIDATOR.md#observe-actual-python-import-relationships)
and [output schema](gdd/schemas/code-observation.schema.json). Inspect witnesses,
actual source hashes, exclusions and unresolved coverage. This static observation
executes no project modules and creates no trusted evidence; an UNKNOWN result
does not establish architectural compliance. Preserve actual host checks and
attestations through the existing evidence workflow.

Record the four-principle advisory assessment with graph IDs and actual code
locations. The host independently verifies the required fields and selections;
the validator does not execute application checks. Acceptance requires current
evidence and the applicable acceptance mandate, separately from implementation
authorization. Model/asset/check-plan drift needs a new plan/run; retain history.

Deliver the scope, changed IDs, revisions, actual code and tests, raw evidence,
current results, and remaining questions. If required gates are UNKNOWN or denied,
report the accomplished bounded work and the specific unresolved dependent action.
