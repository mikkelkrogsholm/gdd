# GDD 0.3 JSON Schemas and preserved 0.2 schemas

All schemas use JSON Schema Draft 2020-12 and can be used with an existing schema validator. They define representation, not a complete GDD validator.

[status.schema.json](status.schema.json) and [impact.schema.json](impact.schema.json)
describe read-only project overviews and explained potential change consequences.
They are local tool contracts outside the core GDD formats. See
[inspection coverage](../docs/inspection.md); status is not a project-complete
verdict and potential impact is not write ownership or verified runtime behavior.

[code-observation.schema.json](code-observation.schema.json) describes the
informative `gdd-code-observation/0.1` static Python observer output. It is
outside the GDD core and does not extend `gdd-report/0.2` or establish trusted
implementation evidence. See [the Python profile](../docs/python-imports.md).

| Schema | Document |
| --- | --- |
| gdd.schema.json | GDD 0.3 model manifest and safely loaded YAML/JSON |
| gdd-0.2.schema.json | Preserved GDD 0.2 model contract |
| fragment.schema.json | GDD 0.3 local node/edge fragments |
| fragment-0.2.schema.json | Preserved GDD 0.2 fragment contract |
| extension.schema.json | Declaration of custom kinds/relations and their data schemas |
| records.schema.json | Bindings, decisions, check plans and evidence |
| report.schema.json | Scoped check results and diagnostics |
| snapshot.schema.json | Byte assets and model snapshot identity |

[validator-inputs.schema.json](validator-inputs.schema.json) is an informative tool schema with `$defs` for host, package, parallel and import. It describes the [validator's](../VALIDATOR.md) API envelopes and is not a seventh normative GDD document format. It is derived from `gdd_validator.validator.input_schema`.

[observations.schema.json](observations.schema.json) and [graphify-mapping.schema.json](graphify-mapping.schema.json) describe the [Graphify adapter's](../GRAPHIFY.md) observation bundle and explicit mapping to a proposal. These are tool formats outside the six GDD core documents. The original is preserved alongside the index; schema PASS alone does not verify integrity/projection.

Duplicate YAML keys, aliases, fragment loading, global ID collisions, reference resolution, endpoint kinds, state semantics, extension semantics, readiness and authorization require additional checks under [VALIDATION](../standard/VALIDATION.md). A schema PASS must not be presented as model PASS.

Schemas have local identifier URIs under `https://gdd.example/`; this is not a published service. Identifier references, including the two model versions in the view schema, resolve to included package assets through an offline registry; network retrieval is not required or authorized. Validation must be possible offline with the included files. `format: date-time` must be validated with format assertion enabled when checking exception expiry.

The selected Draft 2020-12 contract is described by [JSON Schema](https://json-schema.org/draft/2020-12). The concrete GDD fields and semantics are this draft's own proposals.

[view.schema.json](view.schema.json) and [view-state.schema.json](view-state.schema.json) describe the [viewer's](../VIEWER.md) embedded payload and interactive visibility manifest. They are informative tool contracts. Model fields, exact records/ID partitions and renderer bytes are checked separately; view state does not change snapshot identity.

[workflow.schema.json](workflow.schema.json) describes the agent workflow's request, workplan, packet and event envelopes. It is an informative tool format (`0.1`), supports GDD 0.2/0.3. Complete derived plans are verified against current model bytes and selected check plans; schema alone proves neither package integrity, authorization nor execution.


Model/fragment 0.3 adds boundary responsibility/rationale/exclusions, action
postconditions and broad advisory principle labels. Other core envelopes remain 0.2
because their representation is unchanged. Schema alone does not establish interface
ownership, cross-component public calls, readiness or review execution.

[boundaries.schema.json](boundaries.schema.json) describes a derived, scoped public
view. [design-review.schema.json](design-review.schema.json) describes raw advisory
review claims. These are informative tool envelopes, not additional authoritative
registries. Workflow 0.1 packets derived from 0.3 include boundary projections and four
review topics; their bytes are checked against the authoritative model by workflow
verification. Legacy 0.2 packets retain their previous representation.
