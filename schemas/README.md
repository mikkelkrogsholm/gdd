# GDD 0.2 JSON Schemas

All schemas use JSON Schema Draft 2020-12 and can be used with an existing schema validator. They define representation, not a complete GDD validator.

| Schema | Document |
| --- | --- |
| gdd.schema.json | Model manifest and safely loaded YAML/JSON |
| fragment.schema.json | Local node/edge fragments |
| extension.schema.json | Declaration of custom kinds/relations and their data schemas |
| records.schema.json | Bindings, decisions, check plans and evidence |
| report.schema.json | Scoped check results and diagnostics |
| snapshot.schema.json | Byte assets and model snapshot identity |

[validator-inputs.schema.json](validator-inputs.schema.json) is an informative tool schema with `$defs` for host, package, parallel and import. It describes the [validator's](../VALIDATOR.md) API envelopes and is not a seventh normative GDD document format. It is derived from `gdd_validator.validator.input_schema`.

[observations.schema.json](observations.schema.json) and [graphify-mapping.schema.json](graphify-mapping.schema.json) describe the [Graphify adapter's](../GRAPHIFY.md) observation bundle and explicit mapping to a proposal. These are tool formats outside the six GDD core documents. The original is preserved alongside the index; schema PASS alone does not verify integrity/projection.

Duplicate YAML keys, aliases, fragment loading, global ID collisions, reference resolution, endpoint kinds, state semantics, extension semantics, readiness and authorization require additional checks under [VALIDATION](../standard/VALIDATION.md). A schema PASS must not be presented as model PASS.

Schemas have local identifier URIs under `https://gdd.example/`; this is not a published service. They have no remote `$ref` dependencies. Validation must be possible offline with the included files. `format: date-time` must be validated with format assertion enabled when checking exception expiry.

The selected Draft 2020-12 contract is described by [JSON Schema](https://json-schema.org/draft/2020-12). The concrete GDD fields and semantics are this draft's own proposals.

[view.schema.json](view.schema.json) and [view-state.schema.json](view-state.schema.json) describe the [viewer's](../VIEWER.md) embedded payload and interactive visibility manifest. They are informative tool contracts. Model fields, exact records/ID partitions and renderer bytes are checked separately; view state does not change snapshot identity.

[workflow.schema.json](workflow.schema.json) describes the agent workflow's request, workplan, packet and event envelopes. It is an informative tool format (`0.1`), not a change to GDD 0.2. Complete derived plans are verified against current model bytes and selected check plans; schema alone proves neither package integrity, authorization nor execution.
