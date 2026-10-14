# Independent installed-skill journey

The bounded Reservation implementation was integrated and accepted. The real final
application check ran nine tests successfully, all three deliberate defect controls
failed, and `status-accepted.json` reports `acceptance_current: true`. This is a scoped
result for `component.reservations`, not a claim that every historical model record
or every tool profile is globally PASS.

## Starting point and instruction sources

I started with only `reservation.py` and the supplied genuine Graphify 0.9.75 AST
artifact, `graphify-input.json`. I read the standalone installed skill, its existing-code
and change routes, and bundled Graphify, format, migration, workflow, validator, host,
lead-agent, schema and design-principle references. I used the isolated installation's
`venv/bin/gdd` and matching Python. No source checkout, tailored example tool, extractor,
additional agent or example solution was used. The bundled asset verification passed.

The actual CLI reports validator 0.7.0. Some bundled reference headings still identify
older tool versions, including VALIDATOR.md's 0.6.0 title. That metadata did not block
the supported operations; callable help and actual reports were used as current evidence.

## Observation preservation and explicit intent

The explicit `graphify-0.9.75-ast` import passed. Its three nodes and five edges retained
their artifact-plus-pointer identities. `mapping.json` selects three symbols and one
parameter-type edge with an explicit interpretation. `interpretation.json` records
omissions: observed module containment cannot be reversed into ownership, the return
type belongs in the contract, and the old constructor call conflicts with returning
the same object. The raw constructor call remains preserved as an observation.

`proposal.json` and `gdd-proposal.json` remain unaccepted GDD 0.2 sketches. I explicitly
adopted the human lifecycle requirements, migrated to 0.3, preserved mapped stable IDs,
and authored ownership, public interfaces, the component boundary, the state machine,
postconditions, errors, risk assessments, rules and scenarios. `source.graphify` remains
non-normative observation provenance. `INPUT.md` is a separately hashed contract source.
`model-change-final.json` records the base/result revisions and the adopted decision.

The implementation choice for unspecified draft cancellation is explicit: it raises
InvalidTransition without mutation. Cancel returns the same object. These choices
preserve every requested assertion and keep the bounded single-threaded local API clear.

`evidence/raw-integrity.json` verifies five byte-identical files: original input, both
imported raw copies, and supported/unsupported exports. All have 1,978 bytes and SHA-256
`f4c7a0d19f6cf42579a30b5d474dd6fd1752f2197b2c4af6d16447136a7cffdb`.
The unsupported `graphify-999.0.0-ast` profile correctly remained UNKNOWN with exit 2;
its byte integrity and successful recovery were not treated as semantic support.

## Refinement and real implementation

The first readiness attempt caught my incorrect cancellation boundary scenario link.
I changed that link to the actual draft-cancellation negative scenario without changing
the acceptance assertions. A resolved question also needed a trusted current decision
whose scope explicitly covered every question target. My initial owner-only scope did
not meet that condition. I used new immutable decision/check IDs rather than replacing
registered records. The final scoped readiness assessment passed.

Before implementation, the eight fixed tests ran against the original code: two
failures, five errors and one passing signature check. The red output is real and
preserved. The public reserve signature was already correct; I did not claim otherwise.
The workplan declared one package and two writable resources. Dispatch first remained
UNRESOLVED because I omitted the delegated execution actor from host authorities; the
corrected host policy recorded the human's existing authorization and dispatch applied.

The implementation is a 20-line domain module. Reservation defaults to draft. Reserve
sets reserved and returns its input; cancel sets cancelled and returns its input. Both
guard before mutation, allow harmless repeats and reject invalid transitions.
After the eight fixed checks passed, I added a ninth check for independent state in
separate objects within the same package. I reran all nine checks before integration.
`evidence/combined.diff` shows the actual final changes relative to the recorded base.
The pre-addition test file was reconstructed by removing that added test and verified
against its recorded baseline SHA-256; its name identifies the reconstruction explicitly.
The captured application baseline is preserved separately as `evidence/base-reservation.py`.

Dispatch, submit, integrate and accept events all have actual applied receipts. Integration
was performed and reviewed directly in the isolated project; no receipt was treated as
agent transport or patch application. After integration, the installed host helper ran
the final tests and stored the real command, stdout/stderr and before/after code hashes.
Isolated controls introduced replacement-return, resurrection and repeat-cancel-error;
each failed the unchanged assertions. Their mutated code and raw receipts are preserved
under `evidence/negative-controls/`. The original implementation was unaffected.

The host helper registered immutable current test binding/evidence records. A separate
code binding identifies the actual module bytes. Current rule evidence is PASS and
acceptance authority is GRANTED. The advisory four-principle review is COMPLETE for
the actual model/code revisions; that structural review status is separate from the
executed behavior evidence.

## Current reading artifacts and documented UNKNOWN results

The final intended model revision is
`sha256:fee6a315acc673e367d0a65c32ec8d2aa59e002ee76b33700e764162ca8fc62f`.
The final combined code revision is
`sha256:12579f6b89d1bf9eb00ebbdfec0daaf4c7b7f19c82cbe69e02ba1993ea95559f`.

I retained earlier records without rebinding or deleting them. Bundled VALIDATOR.md
explicitly says the current validator does not load historical snapshots and dependent
target checks remain UNKNOWN. Those documented mixed-history diagnostics remain in the
current reports. Global model assessments therefore remain UNKNOWN even though core
schema/semantics and current scoped readiness pass. This is not a demonstrated runtime
defect, and I did not remove history to turn an aggregate green.

In this mixed-history state, CLI snapshot/context output paths received reports rather
than reading artifacts. I authored `snapshot-final.json` from the documented exact-byte
profile and compared it for exact equality with the installed final workplan's generated
snapshot. `context-scoped.json` is a copy of the installed workplan's authoritative context.
`context-verified.json` reports package and readiness PASS while preserving historical
UNKNOWN diagnostics. The graph was generated as `graph-final.html`; view generation
reported PASS, but `visualization-final.json` correctly retains UNKNOWN because global
model assessment is unresolved. I claim generation, not globally verified visualization.

`accept-result.json` reports applied true, and `status-accepted.json` reports an accepted
run with `acceptance_current: true`. The latter command exits 2 because its report also
contains the preserved historical diagnostics. The acceptance event exits 0 on its actual
committed effect. Current scoped acceptance and global historical UNKNOWN are distinct.

## Guide and API friction

No required operation remained blocked. The most material cold-start friction was
assembling exact current decision scope and host actor authority. The generic question
diagnostic did not identify which prerequisite was absent, so I inspected the installed
readiness implementation to establish exact target coverage. The snapshot/context output
shape under documented historical UNKNOWN also needed careful inspection; I initially
expected a snapshot revision field but received a report spec_revision field. A short
standalone example for resolving imported questions and handling mixed-history reading
artifacts would make these steps easier. The original reports and interventions remain
visible, and no expected assertion was weakened.

Substantive CLI and verification commands are indexed in `evidence/commands.csv`, with
exact argv, working directory, timestamp, exit code and raw stdout/stderr receipts.
Host execution receipts additionally preserve the actual code snapshots and test output.
`plan-final.json`, `readiness-final.json`, `implementation-evidence.json`, the run journal,
and the accepted status are the principal structured result artifacts.
