# Graphify → GDD

Adapter 0.1.2 in `gdd-validator` imports Graphify as a separate observation layer. It preserves the received artifact's bytes and indexes every node and relation without a Graph/DiGraph builder. The agent can then choose an explicit interpretation and prepare an unaccepted GDD proposal. Known 0.1.0/0.1.1 bundles are revalidated under the current rules; the importer's version field bypasses no checks.

```sh
uv run gdd graphify import graphify-out/graph.json \
  --profile graphify-0.9.75-node-link --output observations
uv run gdd validate observations --profile import.graphify
uv run gdd graphify export observations --output recovered.graph.json
```

`recovered.graph.json` is byte-identical to the imported input, including whitespace, unknown fields, and order. Output files and bundles are created exclusively: existing files are not overwritten. Choose a new output name when rerunning.

## Bundle and identity

| File | Content |
| --- | --- |
| graphify.raw.json | Exact copy of the received input; no normalization or sorting |
| observations.json | Agent-readable index, raw payload projections, revision-bound identities, producer claims, and binding candidates |
| import.report.json | Actual checks in `gdd-report/0.2`; model revision is null because import does not create an authoritative model |
| README.md | Meaning and trust boundary of the observation layer |

An observation record's canonical identity is `(artifact_revision, JSON Pointer)`, for example `sha256:…` and `/links/4`. The short `observation.edge.h…` ID is derived deterministically from this identity. The producer's own ID and its JSON type are preserved separately. New bytes or a new array order produce new snapshot identities; they are not stable symbol IDs across re-extraction.

Two identical relations each receive their own record. A `multigraph:false` flag is never used as a reason to deduplicate a raw array. Communities, `community=0`, top-level/nested hyperedges, and unknown metadata are preserved. Hyperedges are not expanded into a clique.

## Explicitly supported profiles

| Profile | Input contract |
| --- | --- |
| graphify-0.9.75-ast | `nodes` and `edges` from the examined extractor. Nodes have id, label, file_type, and source_file; edges have source, target, relation, confidence, and source_file. A confidence score is optional. Raw source→target order has the direction declared by the AST profile when no directed flag contradicts it. |
| graphify-0.9.75-node-link | `nodes`, `links`, boolean directed/multigraph, and object graph from the JSON export. Hyperedges, communities, norm_label, extra fields, and commit claims are preserved. |

The profiles are based on the actually installed **Graphify 0.9.75** and its source hashes in [producer-evidence.json](examples/graphify/producer-evidence.json). They do not promise support for all Graphify versions or all upstream routes. [The examined exporter](https://github.com/Graphify-Labs/graphify/blob/48d7c0e832cd2d67d86850e716ddea16df6238ea/graphify/export.py) and [builder](https://github.com/Graphify-Labs/graphify/blob/48d7c0e832cd2d67d86850e716ddea16df6238ea/graphify/build.py) explain `original_links` and simple graph storage.

The examined exporter does not itself write a Graphify/schema version. The layout profile must therefore be selected explicitly; it invents no verified producer version. Optional `--producer-version 0.9.75` and `--code-revision ...` are stored as **unverified caller claims**. Known producer claims are read both at the top level and under graph with their actual JSON Pointers. Unknown/conflicting version or revision claims yield UNKNOWN. Inputs containing both edges and links are not interpreted by silently choosing one array.

Strings, integers, and safely preserved finite numeric identities are supported without string coercion; `1` and `"1"` are different. Bool/null/object identities are invalid. Numbers that the JSON projection would round to a different decimal value or cannot convert yield UNKNOWN before reference resolution; original bytes are preserved and can still be exported. The JSON constants NaN/Infinity are invalid and yield FAIL.

The raw projection supports JSON container depth up to 256: the root object/array counts as 1, each nested object/array adds 1, and scalars and keys do not count. Greater depth or exhausted runtime capacity yields UNKNOWN/FORMAT_UNSUPPORTED with a compact byte-identity index without producer records. The original is preserved for hash-checked export; compact depth indexes provide no verified producer projection or usable model proposals. Readable historical full deep indexes are assessed as UNKNOWN in the projection check. An index that the runtime cannot load at all cannot provide a verified original hash for export.

## What PASS means

The report keeps raw integrity, layout, nodes/references, direction, code candidates, and projection separate. Intact original bytes can have PASS while direction is UNKNOWN. FAIL takes precedence in the aggregate, but unresolved checks are retained.

With `directed:false` in a node-link export, stored source/target order is preserved. Without intact `_src`/`_tgt`, this profile cannot determine the original direction and reports UNKNOWN. Markers can recover direction in undirected storage; contradictory markers in an explicitly directed graph are rejected. No synthetic reverse edges are created.

Exit 0 means the requested import checks passed. Exit 2 means the input is preserved but a required property cannot be determined, such as direction or layout. Exit 1 means a known failure, such as ambiguous endpoints, corrupt bytes, or a modified index. Export can reconstruct intact bytes from input with an unknown/invalid layout; the accompanying report retains the original semantic result. A corrupt index/original hash cannot be exported through this operation.

Code candidates require the producer's file_type=code and a supported code-file extension. Documents/papers/images/knowledge are separate observations. This is candidate classification, not verified source existence or implementation evidence. `source_file/source_location` are unverified locator claims; an artifact hash is neither a source-file hash, Git revision, nor GDD model revision.

Raw-derived index fields are recomputed during validation. Changes to records, classifications, Pointers, or producer claims are detected against the original. Caller claims, however, are current inputs to the projection and are not authenticated; even coordinated changes to them provide no verified code or authority. `input_claims.trust` and binding-candidate status are always unverified. Preserve/pin the artifact revision through the project's trusted process when its identity must be compared with a previously accepted snapshot.

The adapter can preserve everything in the received input. It cannot reconstruct information that Graphify already removed before export. The actual fixture shows 5 raw AST edges and 5 in export with original_links, but only 3 through the simple builder/export path. This is documented in [the example](examples/graphify/README.md).

## Continue through agents

The agent reads observations.json and the original, clarifies desired behavior, and chooses the interpretation in a separate mapping document:

```sh
uv run gdd graphify propose examples/graphify/observations \
  --mapping examples/graphify/mapping.json --output reviewed-proposal.json
```

The mapping lists the project's id/title/goal, selected observations, desired model IDs/core types, and rationale. Relations require an explicit relation, ID, and endpoints. Types such as function/class do not automatically become action/entity. An observed calls edge does not automatically become a normative GDD edge.

Proposals default to a `gdd/0.3` candidate. Use CLI `--format gdd/0.2` or API `format="gdd/0.2"` when continuing a legacy model; `--format gdd/0.3` selects the current format explicitly. Unsupported targets are rejected. The mapping contract remains `gdd-graphify-mapping/0.2`.

The result is `gdd-graphify-proposal/0.2` with `status: proposed`, `model_candidate`, the explicit mapping, omitted observation IDs, and unverified binding candidates. The envelope version describes the proposal structure, while `model_candidate.format` identifies the selected GDD standard. The candidate is a valid GDD sketch with a provenance reference to raw input, role=observation/normative=false, and blocking contract questions. It receives no workflow state, accepted receipt, check evidence, or invented spec_revision.

Components receive no invented boundary rationale or interfaces, and actions receive no invented contracts or postconditions. An open question blocks implementation and acceptance; model validity is not readiness or evidence that the implementation satisfies intent. Resolve these omissions from trusted requirements during refinement.

The agent can then apply the proposal as an ordinary authorized model change under the project's existing mandate. Source promotion/model change must be visible; existing accepted requirements are not changed by import/propose. The source URI is relative to the proposal file's location; materialize model_candidate there or adjust the locator explicitly. A binding can only be registered as observed/checked once targets, implementation, model snapshot, and actual source/code revisions are known.

```sh
uv run gdd validate examples/concert.gdd.yaml --profile model \
  --profile import.graphify --graphify examples/graphify/observations
```

The last command combines separate model and import checks. It does not prove that the concert example is implemented by the imported fixture or that imported code claims satisfy GDD rules. Such mappings and checks require revision-bound bindings and actual checkers.

## API and verification

```python
from gdd_validator import import_graphify, verify_graphify, export_graphify, propose_graphify

report = import_graphify("graph.json", "observations",
                        profile="graphify-0.9.75-node-link")
report = verify_graphify("observations")
export_graphify("observations", "recovered.json")
proposal = propose_graphify("observations", explicit_mapping)
```

The observation and mapping schemas are [observations.schema.json](schemas/observations.schema.json) and [graphify-mapping.schema.json](schemas/graphify-mapping.schema.json). They are tool contracts outside the GDD core. `source_uri` in the proposal API is an explicit locator; its default is graphify.raw.json for a model materialized in the observations directory. The mapping and locator must be materializable as JSON with valid UTF-8. Proposals require available original bytes and passed integrity/projection checks.

```sh
uv run pytest -q
uv run python conformance/run.py
uv run python conformance/run_graphify.py
uv build
```

The original 58 expectations are preserved. The two original import conditions now have concrete Graphify JSON fixtures. A further [18 adapter cases](conformance/graphify-cases.yaml) and pytest check positive/negative imports, raw round-trip, metadata, duplicates, precision, direction, tampering, CLI, and explicit continuation. [The Graphify review](evidence/graphify-review.md) contains actual results and remaining limitations.
