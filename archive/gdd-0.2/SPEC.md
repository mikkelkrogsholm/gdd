# Graph-Driven Development 0.2

**Status: draft standard. Date: 13 October 2026. Format: `gdd/0.2`.**

GDD is a method and an agent-independent representation for developing software from a pseudo-graph with pseudocode. The accepted model describes intent; code, scans and test results describe the implementation and evidence.

This chapter and [FORMAT](standard/FORMAT.md), [AGENT-PROTOCOL](standard/AGENT-PROTOCOL.md), [VALIDATION](standard/VALIDATION.md) and [INTEGRATIONS](standard/INTEGRATIONS.md) are normative. Examples, migration, research and the scratchpad are informative. MUST, MUST NOT, SHOULD and MAY mean a requirement, prohibition, recommendation with a documentable exception, and permission. A discrepancy between schema and text is a standard defect; a tool must not silently choose the most permissive interpretation.

## 1. Method and mandate

Input from humans and agents is organized in a maintained knowledge layer, such as an LLM wiki in OKF. The model is refined through iterations. It covers structure, behavior, rules, scenarios and user experience. Only the selected development slice needs to be fully specified.

New projects can start from input; existing projects can start from Graphify observations. Observations MUST remain descriptive until selected as intent through the project's authorized process. They MUST NOT automatically rewrite accepted requirements.

The project chooses its working mode and approval rules. There is no universal manual or autonomous default mode. The project's mandate and the platform's access boundaries apply. The agent changes the model; the human sees derived views and gives feedback.

## 2. One logical model

The model is a directed, typed multigraph. Nodes have stable IDs, and each edge has its own stable ID. Multiple edges with the same endpoints MUST be preserved, including those with the same relation type. Edge order has no semantic meaning.

Local fragments MUST be assembled into one model without implicit overrides. Architecture, behavior, traceability and communities are views of this model.

| Core kind | Meaning |
| --- | --- |
| component | Responsibility, system boundary and public contracts |
| entity | Domain concept and abstract data fields |
| action | Action with a contract and pseudocode |
| screen | User interface, interactions, states and design references |
| rule | Requirement, invariant or architecture constraint |
| scenario | Concrete planned normal, error or boundary flow |
| machine | An entity's states and mutating transitions |
| community | Grouping for navigation and context |

Sources, questions, extensions and operational records are referable attachments. A traceability view MAY show them as vertices when their role is clear.

A node MUST have `kind` and `description`; its ID is the registry key. A sketch MAY omit contracts, but specified fields and references MUST be valid. `maturity: specified` is an author's declaration, not computed readiness.

## 3. Identity and revision

IDs MUST be unique across the model's nodes, edges, transitions, sources, questions and extensions. Record IDs MUST be unique in the project's record log and must not collide with model IDs. Global identity is `(project.id, local ID)`.

IDs MUST be preserved across renaming, moves and new code bindings. Deleted IDs MUST NOT be reused for other concepts. Changes MUST identify their base, changed IDs and rationale.

Internal references MUST resolve unambiguously. An external URI is only a locator. A model snapshot revision MUST include the model manifest, fragments, extensions and normative design assets. [FORMAT](standard/FORMAT.md) defines a SHA-256 profile. Code and operational records have separate revisions. Receipts and evidence are outside the snapshot they assess so that hashing is not self-referential.

## 4. Relations

`edges` is authoritative for the following relations. Parallel authoritative owner/members/obeys fields are not allowed. Type targets, transition triggers and source refs are embedded references with separately defined meaning; connections derived from them MUST be marked as derived.

| Relation | From → to | Meaning |
| --- | --- | --- |
| owned_by | action/entity/screen/machine → component | Responsible component |
| stored_in | entity → component | Logical persistence boundary |
| imports | component → component | Intended permitted direct import |
| calls | component/action/screen → action | Intended activation |
| reads | action → entity | Reading domain data |
| creates | action → entity | Creating an instance |
| changes | action → entity | Mutating an existing instance |
| constrains | rule → component/entity/action/screen/machine | Rule applies to target |
| exercises | scenario → action/screen/machine | Scenario flow |
| verifies | scenario → rule | Planned check, not executed evidence |
| state_of | machine → entity | Entity mutation lifecycle |
| navigates_to | screen → screen | Intended navigation |
| depends_on | component/entity/action/screen/machine → same kinds | Prerequisite for full realization |
| refines | all except community → component/entity/action/screen | Child → decomposition parent |
| member_of | all except community → community | Grouping |

The absence of an edge means not specified. `imports` is not a complete allowlist. Prohibitions MUST be expressed in a rule constraint with relation, from, to, reach and policy. Policy is `forbid` in 0.2; complete allowlists are not a core capability. A direct prohibition does not prohibit a legal indirect path.

`contract.effects_complete: true` declares that reads/creates/changes edges and external effects are listed. `dependencies_complete: true` declares that known prerequisites and necessary boundary contracts are listed. These are reviewable declarations, not evidence of completeness. Missing declarations MUST NOT be interpreted as independence.

## 5. Subgraphs and communities

`refines` is decomposition, not proven logical refinement. Children MAY have different kinds from their parent. A child MAY have at most one direct parent. The refines graph MUST be acyclic and have no self-loops.

The subgraph MUST preserve the parent's visible boundary contract. When developing a child, the parent's relevant obligations MUST be included. `realization` describes how named obligations are allocated to children; a structural mapping does not prove semantic equivalence. Parent requirements MUST NOT be weakened without a visible authorized requirements change.

Communities MAY overlap. They do not prove ownership, order or independence. The core has flat communities; member_of between communities is not allowed.

Calls, navigation and non-terminal state flows MAY be cyclic. The entire model need not be a DAG. Cyclic development dependencies require an explicit plan for boundary contracts or combined integration; they do not automatically invalidate the model.

## 6. Contracts and language

Actions use fixed fields for input, context, output, errors, preconditions, external effects, completeness declarations and risk assessments. Empty lists/objects MAY be explicit exclusions. Omitted fields mean unknown or not yet specified.

Behavior, guards, effects, statements and scenario text are free-form pseudocode/prose. They MUST NOT be executed with eval or treated as machine-checked proof. The agent interprets them within the contract and mandate. Structural validation checks form and references, not domain truth.

Domain types MUST be abstract. Concrete paths, framework types and language-specific symbols belong in implementation bindings. Multiple implementations MAY realize the same model; evidence MUST be tied to each implementation separately.

## 7. Rules, scenarios and machines

A rule has category, statement, scope and planned verification methods. Global scope applies to the entire project; targeted scope requires at least one constrains edge. A blocking rule requires relevant passing evidence or a visible policy-permitted exception at implementation acceptance. It does not prevent a valid sketch.

A slice ready for implementation MUST have at least one normal flow and relevant negative/boundary flows. Scenarios have given/when/then and a category. Risk assessments and error_cases connect declared risks and error outcomes to scenarios. Exclusions MUST have a visible rationale. Scenarios describe planned checks.

A machine has unique local states, an initial state, terminal states and transitions with stable IDs. Start/end MUST exist; trigger MUST be an action. The profile allows one mutating transition per `(from, trigger)`; textual guards do not disambiguate duplicates. Unlisted mutations and outgoing mutations from a terminal state are forbidden. Idempotent responses and authorized reads can still be described in the action contract.

`absent` MAY denote pre-existence and is not automatically a database value. Hierarchical machines, parallel regions and multiple guarded alternatives are outside the core.

## 8. Extensions

Extensions MUST have a namespace, exact version, local schema asset and declarations of node kinds and relations. Custom node fields belong in `data`. Custom relations declare endpoints, context traversal and any meaning for development dependencies and effects.

Extensions MUST NOT redefine core semantics, authorization or PASS. There is no implicit substitutability between custom and core kinds. Non-normative annotations have namespaced keys.

Unknown extensions MUST be preserved losslessly. Dependent checks produce UNKNOWN; full validation or readiness must not be claimed. An unknown non-normative annotation can be ignored semantically. Validation MUST NOT automatically execute extension code or fetch schemas over the network.

## 9. Conformance

Tools MUST declare profiles: representation, model, readiness, records, import.graphify and/or visualization. Representation covers schemas and safe loading; model adds graph semantics; readiness requires model and scoped readiness. No profile is a universal behavior verifier.

Unknown formats and missing analysis input produce scoped UNKNOWN with a diagnostic. Known violations produce FAIL. An incomplete analysis with an empty error list MUST NOT produce PASS. [VALIDATION](standard/VALIDATION.md) defines results and reports.

Conformance requires satisfying normative rules and relevant [cases](conformance/cases.yaml). There is not yet a certification body or a complete reference validator. The draft builds on existing methods; [research](research/2026-10-13-existing-work.md) documents overlap.
