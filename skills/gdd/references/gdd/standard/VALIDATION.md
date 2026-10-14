# Validation contract

Normative chapter for GDD 0.3. The concrete implementation's support and limitations are documented in [VALIDATOR.md](../VALIDATOR.md); the requirements in this chapter are not themselves execution evidence.

## 1. Four independent assessments

| Dimension | Question | Results |
| --- | --- | --- |
| model | Can representation, references and known graph semantics be validated? | PASS / FAIL / UNKNOWN |
| readiness | Is a particular slice structurally specified for implementation? | PASS / FAIL / UNKNOWN |
| authorization | Does the current mandate allow this activity? | GRANTED / DENIED / UNRESOLVED |
| implementation | What does a concrete executed check of an implementation show? | PASS / FAIL / UNKNOWN |

Integration checks have PASS/FAIL/UNKNOWN and a declared scope. A valid sketch can be unready. Readiness does not grant authorization. Approval does not provide evidence. A schema PASS is solely a representation result.

PASS requires the check to be completed with sufficient input, known semantics and a satisfied criterion. FAIL requires a known violation. UNKNOWN means the required condition cannot be determined: an unsupported version, missing analysis input, an unavailable external contract or missing execution. If a dimension has both a known error and unresolved checks, the aggregate is FAIL and the unresolved results are preserved. Otherwise, a required UNKNOWN produces an UNKNOWN aggregate.

There is no implicit combined "the project is verified" status. A verification assertion MUST name scope, implementation, revisions and the required checks.

## 2. Profiles and loading order

1. Identify the format version; an unknown version produces FORMAT_UNSUPPORTED/UNKNOWN. Do not attempt to validate it as a known version.
2. Safe parsing, duplicate-key checks, JSON-compatible values, path boundaries and includes.
3. Validate each input file against the relevant local JSON Schema.
4. Assemble registries without overrides; check identity and all internal references.
5. Check endpoints, decomposition, machines, constraints and declared extensions.
6. Compute selected context closure and scoped readiness if requested.
7. Assess authorization against trusted policy/mandate if an activity is requested.
8. Read records and integration results separately; do not execute application checks as a hidden part of model validation.

Unknown extension semantics must not be dropped at steps 5/6. A consumer can pass representation while having model UNKNOWN. A model check MUST list supported core/extension rules. Unknown annotations are non-normative and can be ignored semantically.

## 3. Semantic model checks beyond schemas

MUST checks:

- Global ID uniqueness, including transitions and record collisions; all internal reference types and endpoint kinds.
- Source/question/design references and revision requirements for normative sources. An unavailable external source is integration UNKNOWN, not a dangling internal reference.
- At most one owned_by edge to a component per action/entity/screen/machine when ownership is specified. Two edges with different IDs and identical owner semantics are ownership ambiguity, not an additional owner.
- Refines without cycles, self-loops or multiple parents. Realization children are descendants of the declaring parent.
- Public component interfaces resolve to actions; specified action owners match their exporting component. A resolved cross-component call targets a declared public provider interface. Unknown provider/consumer ownership and unspecified interfaces remain incomplete sketches.
- A rule with principle uses category=design and blocking=false; this does not reinterpret other design rules, including precise UI requirements.
- Member_of to a community from a non-community; overlapping membership is allowed.
- A targeted rule has constrains targets; a global rule applies regardless of local graph distance. Architecture constraint endpoints must be components, and relation is imports in this core profile. Other constraint semantics requires an extension.
- Machine states are unique; initial/terminal/from/to exist; trigger is an action; `(from,trigger)` is unique; a terminal state has no outgoing mutation. State_of must be unambiguous when specified.
- Type references target an entity. Required object fields exist in properties, constraints match the type, and minimum must not exceed maximum.
- Source derivation is acyclic over source+revision identities; referenced sources exist. Older historical revisions must not be rebound to new ones. An unknown relation/kind without an extension declaration is an error.
- Custom kinds/relations belong to the declared namespace; the asset's namespace/version must exactly match the manifest, and the byte hash must match. There must be at most one version per namespace and no name collisions. Data follows the extension's schema. Core semantics must not be redefined. Extension schemas must not fetch remote refs or executable code.

Calls/navigation cycles and multiple distinct relations between the same endpoints are positive controls. A tool that discards them to make the graph acyclic or simple is not model-conformant.

## 4. Scoped readiness profile

Readiness scope is a list of implementation targets; read context is kept separate. Unrelated sketches do not block a slice. All specified parts must still be valid; known model defects elsewhere must be shown and prevent a full model PASS. Scoped readiness can be reported separately if the defect is demonstrably outside its closure.

Each selected action requires:

- Exactly one owned_by component and access to relevant interfaces/boundary contracts.
- Input, context, output, errors, preconditions, postconditions, external_effects, effects_complete=true, dependencies_complete=true and non-empty behavior. Empty maps/lists can be deliberate declarations.
- Type specifications and the relevant fields of referenced entities are available.
- All five risk assessments with rationale. A relevant risk requires scenarios; they must exercise the action. Authorization/idempotency/boundaries/atomicity require negative or boundary scenarios; concurrency requires a concurrency scenario. A scenario can cover multiple risks.
- Each declared error outcome has at least one error_cases reference to a negative/boundary scenario that exercises the action.
- At least one normal scenario that exercises the action. Domain coverage remains a review question.
- All applicable blocking rules have planned scenario coverage through verifies. Architecture rules can use architecture scenarios without calling an action.
- No open question in closure that blocks implementation. Resolved questions require a valid accepted decision receipt or must be treated as unresolved.
- Necessary contract/design sources are normative=true, locally materialized, resolved and pinned with a verified byte hash, and included in the model snapshot; unknown extension semantics must not affect closure.

A screen requires interaction.states, behavior, accessibility and responsive; relevant calls/navigation boundaries; a normal scenario and resolved design_refs. Assessment of relevant error/boundary flows is recorded as a readiness_review decision. Every component in the required closure needs nonblank boundary.responsibility, a short nonblank boundary.rationale, explicit boundary.excludes (possibly []), and specified interfaces. Public actions and call consumers/providers need unique matching owners. Entity/machine/component must have sufficient relevant fields/states/interfaces when the slice depends on them. A top-level component can be ready as implementation scope only when its realization obligations and necessary child contracts are specified and structurally covered. Children need not already be implemented. "Sketch" on a parent does not make a full parent scope ready.

`maturity`, a handwritten ready marker, planned bindings or textual "reviewed" is not readiness evidence. The first validator checks this structural profile. A semantic review can challenge declarations but must not be disguised as a deterministic check.

## 5. Authorization and acceptance

Activity, scope and revision are compared with the project's gates and trusted mandate. An appropriate accepted approval receipt gives GRANTED if the actor and policy are valid. A standing mandate can also give GRANTED. A missing or ambiguous mandate gives UNRESOLVED; an explicit rejection or scope violation gives DENIED.

All entries in rule.verification are required methods, not implicit alternatives. Each blocking rule requires an unambiguous active check plan for each declared method; the check plan must include the rule ID in scope and the exact method label. A missing plan or current executed evidence gives UNKNOWN, never vacuous PASS. Plan selection and the implementation must be explicit.

Implementation acceptance requires all these checks to be current and PASS unless the project's policy permits an explicit exception. Exceptions identify the responsible actor, scope, implementation/code_revision, checks, rationale and expiry. They do not change the check's FAIL/UNKNOWN. Model changes and exceptions are different actions.

## 6. Bindings and evidence

A planned binding does not prove file/symbol existence. An observed binding does not prove correctness. A check must declare the necessary environment and relevant scope. Concurrency in database requirements requires the persistence/transaction behavior actually selected; stubs cannot give this check PASS.

Evidence MUST refer to an existing immutable check plan and matching check_revision, spec_revision, implementation and code_revision, together with checker identity/version and raw output. Evidence scope must cover the check plan's required scope; extra scope is not automatically verified. A missing environment, missing revision-bound bindings, an unsupported method or unknown observation provenance gives UNKNOWN.

Older evidence retains its historical result. Under 0.3's conservative profile, its applicability is UNKNOWN when the model, code or normative asset revision changes. There is no automatic semantic cache invalidation. Negative controls are declared in the check plan and must be executed before the combined check can claim to demonstrate defect detection.

## 7. Report and diagnostics

The report follows [report.schema.json](../schemas/report.schema.json): format, tool(name/version), spec_revision, profiles, scope and checks. Each check has check_id, dimension, result, scope, reason and diagnostics. Implementation PASS/FAIL must also identify implementation, code_revision and evidence_refs; the report's spec_revision must be known. Authorization GRANTED requires activity, policy_revision and authority_basis, which can identify a receipt or a trusted standing mandate. Diagnostics have code, severity (`error|warning|info`), message, location and related_ids. Location is a project-relative file plus JSON Pointer; file can be null if the condition is not local.

No fabricated hash: spec_revision can be null until the snapshot can be computed. The check must then not claim revision-bound PASS. Recommended CLI exit codes are 0=all requested required checks PASS/GRANTED, 1=known FAIL/DENIED, 2=UNKNOWN/UNRESOLVED without a known error. An API can use the same results without a process exit.

The machine-readable [diagnostic catalog](../conformance/diagnostics.csv) lists stable codes. [Legacy cases](../conformance/cases.yaml) and [0.3 boundary/design cases](../conformance/design.cases.json) specify the expected dimension/result and whether the check is based on schema, semantics or readiness. An implementation must declare which cases/profiles it satisfies.

## 8. Advisory design review

[DESIGN-PRINCIPLES](DESIGN-PRINCIPLES.md) specifies concrete assessments and review evidence. Structural completeness or current applicability of a review MUST NOT be reported as deterministic proof of KISS, DRY, separation of concerns or modularity. Missing, planned, unsupported or stale review input is UNKNOWN when that review is requested. Findings from a broad principle remain advisory and MUST NOT silently enter blocking acceptance checks. Precise blocking constraints retain the ordinary evidence requirements.
