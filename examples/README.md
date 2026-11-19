# Standard examples

Start with the small [graph-to-code tutorial](../docs/tutorial.md) for the
complete path through implementation, local tests and current evidence. The
examples below provide larger references and individual format demonstrations.

Run the [Rust quote workflow](agent-native/README.md) to see seven failing
rounding tests on the baseline and 19 passing tests after three repairs. Each
run produces ten workflow events, fresh Cargo receipts and a revision-bound
acceptance.

Start the [Rust concert application](concert-app/README.md) to exercise
reservations through a browser or its JSON API. Its tests cover the domain,
HTTP routes, browser interactions and compatibility with a phase-A database.
The [historical A/B experiment](concert-app/README.md#historical-ab-delivery-and-replay)
records the original requirement change, red/green checks and agent deliveries.
For a new project, use `gdd init` and the [GDD skill](../skills/gdd/SKILL.md).

## Open a native example

From the repository root, open a saved graph with the [installed native viewer](../INSTALL.md#install-the-rust-cli-and-viewer),
for example `gdd-viewer examples/concert-0.3.gddview`. Each file contains the full graph and opens offline.

| Model | Saved graph | What to explore |
| --- | --- | --- |
| [Concert 0.2](concert.gdd.yaml) | [concert.gddview](concert.gddview) | Reservation actions and their interfaces |
| [Concert 0.3](concert-0.3.gdd.yaml) | [concert-0.3.gddview](concert-0.3.gddview) | Public contracts and community boundaries |
| [Product](product/greenfield.gdd.json) | [greenfield.gddview](product/greenfield.gddview) | The same booking flow from the journey, component and data perspectives |
| [Local Sessions](concert-app/gdd.json) | [graph.gddview](concert-app/graph.gddview) | The reservation policy and component responsibilities in the runnable app |

## Model files

- [Product perspectives](../docs/PERSPECTIVES.md): `product/greenfield.gdd.json`,
  `product/refactoring.gdd.json` and `product/partial.gdd.json` connect human journeys,
  components and domain data with stable IDs and an exact local extension asset.
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

To try the [released Python viewer](../VIEWER.md#released-browser-viewer),
download the [v0.8.0 concert HTML file](https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/examples/concert.graph.html)
and open it offline in a browser.

## GDD 0.3 design-quality example

[concert-0.3.gdd.yaml](concert-0.3.gdd.yaml) intentionally migrates the reservation
model: authored component boundary rationale/responsibility/exclusions, public
provider contracts, action postconditions, four advisory principle rules and an
ownership-aware overlapping feature community. API's old duplicated exports are
removed; Reservations remains the provider. The frontend screen remains a sketch.
[Its snapshot](concert-0.3.snapshot.json) and planned records are regenerated for
these bytes; no reservation application execution is claimed.

[The planned design review](design-review.planned.json) declares executed=false and
unknown assessments. It is a format example, not review evidence.
[The derived public view](concert-0.3.boundaries.json) and the
[native saved graph](concert-0.3.gddview) preserve the model IDs and revision.
The [v0.8.0 concert 0.3 HTML file](https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/examples/concert-0.3.graph.html)
preserves the browser presentation from that release.

## Recover the v0.8.0 examples

To recover the Python examples, submitted worker files, phase-A sources,
replay tools and all four standalone HTML graphs byte-for-byte, run this
command in a checkout containing the v0.8.0 commit. It also restores the
matching validator and dependency lockfile for the release replay commands.

```sh
release_dir=$(mktemp -d)
git archive 0dbb10b8c193f55c9777405de3f4ae22ac2d0825 | tar -x -C "$release_dir"
cd "$release_dir"
```

Follow the recovered guides for the [quote workflow](https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/examples/agent-native/README.md)
or the [concert replay](https://github.com/mikkelkrogsholm/gdd/blob/0dbb10b8c193f55c9777405de3f4ae22ac2d0825/examples/concert-app/README.md).
The [indexed concert archive](../archive/experiments/README.md) separately
preserves the original 197-file experiment at commit `7e27757`.
