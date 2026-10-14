# File format and revision profile

Normative chapter for GDD 0.2. The machine-readable form of fields is specified in [schemas](../schemas/README.md). Semantic rules apply in addition to JSON Schema.

## 1. Documents and loading

A model manifest is JSON or YAML with `format: gdd/0.2`, `project`, `nodes` and `edges`. Other registries are optional. The recommended name is `gdd.yaml`; the name is not a semantic identity.

A YAML consumer MUST use YAML 1.2 JSON-compatible scalars and safe loaders. Only JSON values are allowed: objects with string keys, arrays, strings with valid Unicode scalar values, finite numbers, booleans and null. Duplicate mapping keys, custom tags, anchors, aliases and merge keys are forbidden. Constructors or code must not be executed. A consumer that supports only JSON MUST declare this.

Local `includes` refer to `gdd-fragment/0.2` files, which can contribute only nodes and edges. Fragments must not include files themselves. Registry IDs and edge IDs must not be overwritten. In 0.2, the bundle root is the manifest's directory. Includes, record_files, normative local sources and extension paths are relative to the manifest and MUST stay within this root after normalization and symlink resolution. Each asset is loaded once; cycles, duplicates and path escapes are errors. External sources use separate `sources`.

Node, source, question and extension IDs are object keys. Edges and transitions have `id`. ID syntax consists of lowercase ASCII letters, digits, underscores and periods/hyphens between segments; the first character is a letter. Unicode is allowed in labels and text.

## 2. Manifest

| Field | Meaning |
| --- | --- |
| format | Exact format version, `gdd/0.2` |
| project | id, title, goal; optional scope, assumptions, workflow |
| nodes | Map from stable ID to node |
| edges | List of explicit relation records |
| includes | Relative local fragment paths |
| sources | Map of knowledge, input, design and observation sources |
| questions | Map of scoped open or resolved questions |
| extensions | Map of declared extension assets |
| record_files | Relative paths to separate operational records |

Workflow has `mode: manual|autonomous|custom`, a textual `mandate` and gates for `model_changes`, `implementation` and `acceptance`. Each gate has `requirement: explicit|standing|none`, `authorities` containing actor IDs and `scope` containing node IDs; an empty scope means the entire project. `none` means no separate GDD gate, not the absence of platform/user boundaries. `standing` requires explicit bounded authorization in the mandate. Mode is a descriptive label; gates and the trusted mandate determine the action. Additional permissions or approval requirements must not be inferred from the label.

A missing workflow is allowed in a sketch. Dependent authorization decisions are UNRESOLVED until a trusted mandate is available. A file in untrusted input cannot grant itself authority.

## 3. Nodes and contract fields

All nodes: `kind`, `description`; optional `label`, `maturity: sketch|specified`, `source_refs`, `annotations` and `realization`. Unknown structural fields are rejected; non-normative extensions belong in namespaced annotations.

- Component: `interfaces` is a list of public action IDs. A readiness assessment requires relevant boundary contracts to be available.
- Entity: `fields` maps field names to abstract type specifications.
- Action: `contract` and `behavior`. Contract has input/context field maps, an output type, an errors map from error name to description, preconditions, external_effects, effects_complete, dependencies_complete, risks and error_cases.
- Screen: `interaction` describes states, accessibility, responsive and behavior; `design_refs` refers to sources with `purpose: contract`. `calls` and `navigates_to` are described as edges.
- Rule: `rule` has category, statement, scope (`global|targeted`), blocking and verification (a list of method IDs). An architecture rule can have a constraint with relation/from/to/reach (`direct|transitive`)/policy (`forbid`). Method IDs are labels for checkers, not code that is executed.
- Scenario: `scenario` has category (`normal|negative|boundary|concurrency|architecture`), given, when and then.
- Machine: `machine` has states, initial, terminal and transitions. Each transition has id/from/to/trigger/guard/effects. All transitions are mutating in this profile.
- Community: the common fields suffice; membership is described only in edges.

In the readiness profile, a contract MUST list all five risk categories: authorization, idempotency, boundaries, concurrency and atomicity. Each assessment has relevant (boolean), rationale and scenarios (an ID list). When relevant is true, at least one appropriate scenario is required. An exclusion has a non-empty rationale. `error_cases` maps each declared error name to at least one scenario. Presence and references can be validated; relevance and coverage require review.

Abstract type specifications are objects with `type`. Types: string, integer, number, boolean, object, array, reference, enum and any. String can have min_length; numbers can have minimum/maximum. Reference requires target (an entity ID) and denotes a logical entity identity, not an automatically embedded entity object; the physical ID type is mapped during implementation. Enum requires a non-empty list of unique values; array requires items. Object can have properties and required (names must exist in properties); missing required means no mandatory properties. `origin: authenticated_context` can mark trusted runtime identity. This is an implementation requirement, not an authentication mechanism. Type specifications are declarative; constraints must be consistent and apply to the selected type.

Entries in contract.input, contract.context and entity.fields are mandatory unless they have `optional: true`. Nullable concerns the value null, not field absence: `nullable: true` allows null in addition to the type. The defaults are optional=false and nullable=false. `any` means any non-null JSON value, with null allowed when nullable=true. Optional can be used only in the three named field maps; in object.properties, the parent's required list controls field absence, and optional must not be specified there. Optional must not appear on a standalone output type or array.items either. Output is always one returned value unless a declared error occurs.

## 3a. Extension assets

The manifest's extension record has namespace, version, path and an exact sha256. The asset is `gdd-extension/0.2` with the same namespace/version, plus node_kinds and relations. There must be at most one version per namespace in a model. All names must start with the namespace followed by a period; core terms must not be taken over.

A node kind describes description and data_schema. A relation describes description, from/to lists of exact kind names, context, dependency, effect and data_schema. Endpoint names must be known core kinds or declared extension kinds.

Context specifies contributions to closure: outgoing follows the relation from an included source to the target; incoming follows from an included target to the source; both does both; none adds no closure. Dependency=true gives the source a prerequisite on the target for full realization. Effect identifies the source as an actor reading/creating/changing the target (`read|create|change`), or none. This makes these contributions visible in context and parallel planning; it executes nothing and does not make a custom kind substitutable for a core kind.

Data_schema is a JSON Schema Draft 2020-12 object for the data field. References must be local within this schema object; remote refs, executable hooks and mutable schema lookups are not allowed. A consumer that does not support these declarative semantics must report UNKNOWN for dependent checks.

`realization` is a list with obligation (text) and children (node IDs). The children MUST be descendants of the node via refines. The field must not be used as evidence that the parent contract has actually been realized.

## 4. Sources and questions

Source has role (`input|knowledge|design|observation|code`), title, uri, format and normative (boolean). Revision can contain sha256 and producer_revision. Producer can have name/version. `derived_from` contains records with a source ID and the revision actually used (sha256 and/or producer_revision). An unknown revision is omitted and produces UNKNOWN for lineage checks; the consumer must not substitute the source's later revision. Identity in a derivation path is source+revision. An older revision of the same source can be legitimate; cycles among identical revision identities are rejected. Missing historical inputs produce UNKNOWN for dependent lineage checks. An external profile contract can be pinned in specification with uri/revision/sha256.

Source-ref has a source ID, purpose (`provenance|contract`) and an optional selector. Selectors are `whole-document`, `json-pointer`, `markdown-heading`, `line-range` or `opaque`. Value syntax: JSON Pointer follows [RFC 6901](https://www.rfc-editor.org/rfc/rfc6901); Markdown-heading is the exact heading path separated by ` / `, with an optional 1-based ordinal; line-range is `Lstart-Lend` with end ≥ start and an exact source hash; opaque is preserved uninterpreted text. Known but unsupported selectors produce integration UNKNOWN; a non-conformant selector kind is schema FAIL. YAML tokens are referenced through JSON Pointer after safe YAML parsing; no new unstandardized yaml-path language is defined.

A declared source ID with an unreadable external target is not a dangling internal reference. The affected source check is UNKNOWN. Before readiness, every purpose=contract reference actually used, including design_refs, MUST use normative=true, a locally materialized source and a verified revision.sha256 so that the asset is included in the model snapshot. Planned non-normative contract refs are allowed in sketches. Provenance sources can be retained as unavailable with a visible diagnostic.

Question has statement, targets, blocks (`modeling|implementation|acceptance`), status (`open|resolved`) and, when resolved, a resolution with a decision-record ID. A resolution is insufficient if the decision is missing or does not apply to the current change.

## 5. Operational records

`gdd-records/0.2` has project_id and a records map. It is separate from the authoritative model snapshot. Record kinds are:

- binding: spec_revision, targets, implementation, role (`code|test|design`), resource, selector and status (`planned|observed|checked`). Planned can lack a resource revision. Observed/checked requires an exact resource revision and, for code/test, also code_revision for the specific implementation. Overlap between bindings can be legitimate; ambiguous mapping and competing write ownership are different errors.
- decision: topic (`model_change|approval|readiness_review|exception|source_promotion`), scope, spec_revision, policy_revision, actor (id/kind), outcome (`proposed|accepted|rejected`) and rationale. Approval requires activity (`model_changes|implementation|acceptance`). Acceptance receipts and exceptions also require implementation and code_revision. Exceptions require expires_at and references to affected checks. Receipt identity is a recorded claim; the host must check the actor's authority.
- check: spec_revision, scope, method, required_environment and negative_controls. This is a plan; method must be supported by the executing checker.
- evidence: check ID and check_revision, implementation, spec_revision, code_revision, checker(name/version), scope, result, executed, reason, environment and evidence_uri. PASS requires executed=true and sufficient input. The schema does not prove that a reported execution took place.

Records are immutable after registration. A change requires a new record ID; `supersedes` can refer to the previous record of the same kind and must be acyclic. The check plan's check_revision is `sha256:` plus the hash of the record value serialized as UTF-8 JSON with sorted object keys, ensure_ascii=false, separators=(',', ':') and no newline. Evidence MUST match this hash. Multiple active plans for the same rule/method require explicit selection; they must not be silently merged.

Check and binding targets MUST refer to model IDs in their spec_revision snapshot. Historical records must not be resolved against the latest model when nodes have been deleted. The binding revision identifies the resource/code basis. Decision checks and evidence-check MUST refer to check records in the same project. Evidence and the check plan must have the same spec_revision. Model/code revisions, the check hash and raw evidence must be checked before results are used for acceptance.

## 6. Revisions without self-reference

The portable profile `gdd-snapshot/0.2` contains assets: a list of `{path, sha256}`. Paths are normalized relative POSIX paths, sorted in ascending Unicode code point order. The list includes the manifest, all fragments, extension assets used and sources with normative=true. All are local snapshot assets; necessary external normative sources must first be materialized or produce UNKNOWN. Duplicate paths are not allowed.

Manifest identity is `sha256:` followed by SHA-256 over the UTF-8 bytes of a JSON list of `[path, sha256]` pairs with `ensure_ascii=false`, no whitespace (`separators=(',', ':')`) and no newline. Asset hashes cover the exact file bytes. The report file, operational records and the snapshot manifest itself are not in the asset list. Paths and changes to model bytes produce a new revision; graph IDs are preserved. This is a reproducible byte profile, not semantic equivalence or a digital signature.

A label or Git commit MAY be an additional locator but must not replace identification of relevant local changes. A code snapshot uses a separate implementation ID and revision; multiple implementations must not implicitly share evidence.

## 7. Versioning

0.2 is explicitly incompatible with 0.1. No silent upgrade. A consumer MUST declare supported formats and extension versions. Future changes with new interpretation requirements must have a new format version. The schema IDs' `https://gdd.example/` is a local identifier namespace, not a published service; schemas must be resolved from this package without network access.
