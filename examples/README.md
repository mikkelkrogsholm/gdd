# Standard examples

The standard files below illustrate the format and method. The separate [agent-native example](agent-native/README.md) demonstrates an actually executed, bounded code workflow with red/green tests and revision-bound acceptance.

- `sketch.gdd.yaml`: valid minimal sketch; the action lacks a contract and is not ready for implementation.
- `concert.gdd.yaml`: explicit migration of the existing reservation material. Original node, edge, rule, scenario, machine, and transition IDs are preserved. New scenarios make error/risk coverage structurally explicit.
- `concert.records.yaml`: planned bindings and check plans. No executed application checks or invented PASS evidence.
- `concert.snapshot.json`: reproducible model snapshot referenced by the planned records. It includes normative model assets; planned design sources are not an accepted design contract.
- `refinement.gdd.yaml`: heterogeneous children under an action and two different relations between the same endpoints. A sketch, with no claim of semantic refinement or readiness.
- `extension.gdd.yaml` and `extensions/sensors.extension.json`: small custom node type/relation with a pinned extension asset.
- `fragmented.gdd.yaml` and `note.fragment.yaml`: one logical model distributed across local files.
- `illustrative.report.json`: schema example with UNKNOWN, explicitly not an executed validator report.
- `sources/legacy-concert.gdd.yaml`: exact snapshot of the previous intention as a migration source.

The frontend in the concert model is deliberately a sketch. `source.design` points to a planned DESIGN.md that does not exist; no design basis is invented. A screen scope is therefore not implementation-ready. Backend contracts illustrate the standard's structure; their domain adequacy and actual readiness must be assessed by a later validator/reviewer.

No workflow policy is chosen in the standard examples below. Missing policy prevents implicit implementation authorization. A concrete project adds its own policy and a trusted mandate.

The standard examples are used for format and method semantics. The practical quote example is separate under `agent-native/`; it grants no implicit authorization to other projects.

[concert.graph.html](concert.graph.html) is a generated, interactive, offline [GDD view](../VIEWER.md) of the concert model. It is read-only, contains no application evidence, and is not a normative model asset.
