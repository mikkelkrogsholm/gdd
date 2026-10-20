# Inspect status and potential change impact

The development CLI provides two read-only starting points. `gdd status` answers
what the existing assessors establish now. `gdd impact` explains declared
potential consequences before a model change. Neither command edits the graph,
source files, workflow journals or evidence, starts agents, or executes project
code. JSON is the default; `--format text` presents the same analysis concisely.

## Status: what is known now?

```sh
gdd status model.gdd.yaml --scope action.reserve
gdd status model.gdd.yaml --scope action.reserve --format text
gdd status model.gdd.yaml --scope action.reserve --run runs/feature \
  --host-context trusted-host.json --activity acceptance
```

A model alone is sufficient. Scope follows existing readiness semantics; repeat
`--scope` for multiple selections. The overview keeps four assessments separate:
model validity, structural readiness, authorization and implementation evidence.
Their coverage and underlying reports remain visible. Without actual trusted
inputs, authorization/evidence are explicitly unassessed or unresolved; readiness
PASS does not imply either. Record-integrity diagnostics do not overwrite a
separately established scoped readiness result.

`--run` selects one existing workflow run explicitly. Its recorded state,
packages/attempts and current acceptance come from the existing `run_status`
assessor. Historical accepted state is distinct from `acceptance_current`.
Unavailable or malformed run input is diagnosed, not silently skipped. No run or
host-context file is discovered automatically.

`--host-context` remains the trusted caller's attestation boundary described in
[HOST](../HOST.md). Optional `--implementation` and `--code-revision` supply explicit
assessment inputs; they cannot turn a hash or a model claim into trusted evidence.
`--activity` selects implementation, model_changes or acceptance authorization.
The command never refreshes evidence or infers current source bytes.

Open questions and a bounded deterministic list of next actions identify existing
diagnostics or explicit absent inputs. Suggested commands are argv examples to
review, not dispatched work or product priorities. The full underlying reports
remain available when the compact list omits additional diagnostics.

Status exit codes cover the **assessed** checks: known FAIL/DENIED takes priority
(exit 1), then assessed UNKNOWN/UNRESOLVED (exit 2), otherwise assessed checks pass
(exit 0). Loading and integration diagnostics also affect this exit policy;
an unresolved snapshot cannot silently pass because scoped readiness passes.
Deliberately unassessed dimensions remain visible; exit 0 is not a
project-complete or acceptance verdict. A fresh sketch therefore exits 1 for its
readiness blockers, while a ready model-only scope can exit 0 with authorization
and evidence explicitly unassessed.

The Python API is `gdd_validator.project_status(...)`. Its separate local tool
contract is `gdd-status/0.1`, described by
[status.schema.json](../schemas/status.schema.json). `status_text(result)` in
`gdd_validator.project_status` formats an already computed result.

## Impact: what might need review?

```sh
gdd impact model.gdd.yaml --scope action.reserve
gdd impact model.gdd.yaml --scope action.reserve --format text
gdd impact model.gdd.yaml --scope action.reserve --scope entity.reservation \
  --implementation implementation.main --output new-impact.json
```

Scope is required and identifies current model nodes. Unknown IDs and unsupported
identity kinds receive diagnostics. Supported roots are analyzed independently,
with deterministic deduplication and one witness per root/target. Cycles terminate.
This first delivery does not derive roots from a Git diff, compare two model
snapshots or infer deleted-node consequences.

Each result distinguishes the selected node, direct/transitive potential impact,
and supporting read context. A changed provider may require its declared callers
or dependents to be reviewed. Witnesses identify edge IDs, direction and relation;
embedded references and existing context derivations name their semantic rule
instead of inventing an edge.

| Existing source → target relation | Potential consequence when target changes |
| --- | --- |
| calls | Review the declared caller |
| depends_on | Review the declared dependent |
| imports | Review the declared importing component |
| reads, creates, changes | Review the action using the entity contract |
| navigates_to | Review the screen navigating to that destination |
| state_of | Review the machine describing that entity's lifecycle |
| Embedded abstract type reference | Review the node using that referenced entity |
| Machine transition trigger | Review the machine using that trigger action |
| Extension relation with dependency=true | Review the declared source dependent |

These relations propagate potential consequences in reverse dependency direction.
A changed writer is not automatically propagated forward into its entity and all
readers. Ownership, refinement, interfaces, communities, rules, scenarios and
extension context directions contribute supporting context under existing closure
semantics; they do not independently declare mandatory edits. The graph's
`imports` edges are intended relationships, distinct from the observed imports in
[check-code](python-imports.md).

Applicable current check plans are pointers to review/rerun, planned scenarios
are review context, and binding resources are locations to inspect. They are not
new execution results or proof that a resource is current. Implementation filters
are explicit. Compatible superseded check plans are retired using existing check
semantics; binding supersession needs verified trust and is not inferred here.
Historical or invalid records are not rebound to the current model;
their omission remains visible. Model-only analysis does not require code bindings
to establish its declared graph consequences, but missing implementation mapping
limits are reported.

Coverage states whether declared semantics are complete, partial or unavailable.
It never claims to enumerate every runtime caller. Unknown extensions, incomplete
assets or unresolved selected inputs stay visible; partial output is not an
empty-safe consequence set. A valid complete in-profile analysis exits 0 even
when it finds potential impact; known invalid input exits 1 and unresolved/partial
information exits 2 when no known invalidity takes precedence.

The API is `gdd_validator.analyze_impact(path, scope=[...], implementation=None)`.
Its separate local tool contract is `gdd-impact/0.1`, described by
[impact.schema.json](../schemas/impact.schema.json). `impact_text(result)` in
`gdd_validator.impact` formats that same result. These are tool contracts, not a
GDD standard revision or a grant of write ownership.

## Worked planning example

After refining the [integer-addition tutorial](tutorial.md), inspect it directly:

```sh
gdd status docs/tutorial/refined.gdd.json --scope action.add --format text
gdd impact docs/tutorial/refined.gdd.json --scope action.add --format text
```

Status establishes the ready planning scope while leaving authorization and
implementation evidence unassessed. Impact shows the selected action and its
owner/planned scenario as review context; it does not claim an implementation was
changed or tested. Both commands leave the model bytes unchanged. For declared
caller and transitive-dependent controls, run the local conformance suites:

```sh
uv run --no-sync python conformance/run_status.py --output /tmp/new-status-check.json
uv run --no-sync python conformance/run_impact.py --output /tmp/new-impact-check.json
```

Both commands accept `--output NEW_FILE` and refuse existing files or destination
symlinks. No GitHub CI, hooks or automatic execution are needed.
