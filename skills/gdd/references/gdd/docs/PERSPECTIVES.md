# Journey, component and data perspectives

A product can be described from three connected perspectives of one GDD model:
what a human does and sees, which components are responsible, and what data and
rules those components use. These are projections with the same canonical node
and edge IDs, not three models to keep synchronized. They work for a greenfield
sketch or an existing system being refactored. Model only the useful slice;
unresolved links are advisory gaps and incomplete sketches are allowed.

## Author the model

Use the existing `gdd/0.2` or `gdd/0.3` manifest and core entity fields, rules and
`constrains`, actions with `reads`/`creates`/`changes`, and `owned_by`. Entity
fields express domain types and references independently of a database or
implementation language. Keep storage, framework and language decisions in
component descriptions or later implementation contracts when needed.

Copy the exact [product extension](../gdd_validator/extensions/product-0.1.json)
into the manifest's local asset directory. Declare its `namespace: gdd.product`,
`version: 0.1.0`, relative `path`, and SHA-256 of the exact file bytes in the
manifest's `extensions` registry. The extension is also bundled in the portable
GDD skill at `references/gdd/gdd_validator/extensions/product-0.1.json`.
Never reconstruct a similarly named definition and assume it has these semantics.
The normal loader checks declared asset hashes; perspectives additionally pins
interpretation to the exact shipped version, definition and bytehash.

| Kind or relation | Meaning |
| --- | --- |
| `gdd.product.actor` | A human user; optional `data.goal`. Model software services and automated agents as components. |
| `gdd.product.journey_step` | A human-facing step; optional nonempty `data.user_action` and `data.system_response`. |
| `gdd.product.participates` | Actor → step. Several actors may participate. |
| `gdd.product.next` | Step → possible next step; branches, loops and multiple edges are allowed. |
| `gdd.product.realized_by` | Step → responsible component; several components may collaborate. |
| `gdd.product.uses` | Component → entity; several components may use the same entity. |
| `gdd.product.relates_to` | Entity → entity; optional `data.from_cardinality` and `data.to_cardinality`, each `0..1`, `1`, `0..*` or `1..*`. |

Custom nodes have `description` and `data`; custom edges have `data: {}` when no
relation metadata is needed. Actor nodes describe human roles, rather than real
people's personal information. Human meaning is authored, not mechanically
proved by the string schema. Cardinalities describe the conceptual association
at each endpoint; they do not create database constraints. `next` is journey
ordering, never a code execution dependency. All product relations have
`dependency: false` and `effect: none`; use core action effects for behavior.

## Inspect and compare

From a current source checkout:

```sh
uv run gdd perspectives examples/product/greenfield.gdd.json
uv run gdd perspectives examples/product/partial.gdd.json --output /tmp/partial-perspectives.json
uv run gdd diff examples/product/greenfield.gdd.json examples/product/refactoring.gdd.json
```

Output files must be new paths. The compact [greenfield model](../examples/product/greenfield.gdd.json)
starts with a guest choosing a performance and reserving a seat. The
[refactoring model](../examples/product/refactoring.gdd.json) moves reservation
responsibility and usage to the Reservations component. Journey nodes, entity
nodes and the capacity rule retain their IDs and content; the changed asset
bytes produce a different `spec_revision`. The
[partial model](../examples/product/partial.gdd.json) captures one step and an
entity without requiring an actor, component or complete fields.

The JSON envelope is `gdd-perspectives/0.1`, described by the
[output schema](../schemas/perspectives.schema.json). Each view records its
`primary_node_ids`, full canonical node/edge records, direct contextual neighbors,
`cross_links` to other perspectives, and exact `shown`/`omitted` canonical IDs.
Records may appear in several views with the same identity. Context includes
only edges touching a primary node, not every edge between contextual neighbors.
Unknown extension nodes/relations may appear as canonical context but are not
interpreted as product concepts; `unsupported_extensions` identifies their
namespaces. An unsupported or altered product definition is an explicit error
(exit 2), never silently interpreted as this version. Invalid bundles, references
or extension hashes fail (exit 1); `gdd validate PATH` provides diagnostics.

`component_data_links` lists explicit product usage and core entity ownership or
storage, plus action effects derived through the action's `owned_by` component.
Every link retains canonical `edge_ids` as witnesses; no synthetic canonical
edge is created and duplicate endpoint pairs retain their distinct relations.

Python callers can use `perspectives(path)`. A viewer can call
`load_perspective_bundle(path)` once, then `perspective_data(bundle)` to project
the already validated, unchanged bundle without reading arbitrary model paths.
Do not mutate the bundle between validation and projection. Returned records
are copies and may be used by a consumer without mutating the model.

## Advisory limits and compatibility

Gaps flag steps without actors, user actions, system responses or responsible
components, entities without responsible/using components, and invariant rules
without explicit targets. An invariant without targets needs human review to
decide whether it is a data rule. A global invariant remains valid; a targeted
rule without targets is already invalid under core semantics. Fields that are
present but invalid do not become advisory omissions: normal validation fails.

A successful inspection or an empty gap list establishes no implementation PASS,
readiness, authorization, domain completeness or acceptance. This command does
not extract a model from code, migrate code, generate a database, or generate an
implementation. It does not alter the existing HTML viewer. Continue through
normal scoped contracts and revision-bound verification for actual development.

| Compatibility surface | Version | Boundary |
| --- | --- | --- |
| Product authoring extension / advisory projection | `gdd.product` 0.1.0 / `gdd-perspectives/0.1` | Optional exact hashed `gdd-extension/0.2` asset on unchanged `gdd/0.2` and `gdd/0.3` cores; independent tool output schema. |
