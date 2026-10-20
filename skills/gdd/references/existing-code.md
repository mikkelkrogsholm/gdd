# From existing code to intended behavior

Read [GRAPHIFY](gdd/GRAPHIFY.md) and the existing project instructions. Inspect
actual code/local changes and retain its observed code revision separately from
the graph/artifact revision. Do not reset the checkout. Existing accepted intent
continues to govern until a visible authorized model change replaces it.

If Graphify data already exists, preserve that file. If it must be produced, use
an available Graphify tool with known provenance; this skill installs no extractor
and assumes no implicit extraction route. Choose the profile for the actual
received layout, not a guessed upstream version:

```sh
gdd graphify import graph.json --profile graphify-0.9.75-node-link --output observations
gdd validate observations --profile import.graphify
gdd graphify export observations --output recovered.graph.json
```

Use `graphify-0.9.75-ast` for its supported nodes/edges layout. Compare recovered
bytes to the preserved original; record the hashes and actual result. An unknown
version/layout/direction remains UNKNOWN even when raw-byte integrity passes.
Producer revision/path/type claims are unverified observations, not source hashes,
GDD stable IDs, authority, or implemented contracts. The importer cannot recover
information lost before export.

Inspect `observations.json`. Select explicit observation IDs, proposed stable GDD
IDs/types, descriptions, and rationale in
[a mapping](gdd/schemas/graphify-mapping.schema.json). Select each proposed edge's
observation, stable edge ID, endpoints, relation, and rationale. A function is not
automatically an action; an observed call is not automatically an intended call.
Record omissions and material disagreements with the existing model.

A mapped edge must retain its observed endpoints and direction. For example,
module-to-function containment cannot be reversed into function-to-module
`owned_by`. Omit that observed edge with a reason and author the intended
ownership separately during refinement; retain the raw observation in the bundle.

```sh
gdd graphify propose observations --mapping mapping.json --output proposal.json
```

The adapter defaults to a `gdd/0.3` candidate. Select `--format gdd/0.2` (or
API `format="gdd/0.2"`) to continue a legacy model explicitly. The proposal and
mapping envelope versions stay `0.2`; `model_candidate.format` identifies the
selected standard. Preserve stable IDs and observations while refining the
candidate. Component boundaries/interfaces and action contracts/postconditions
must come from actual requirements; the adapter invents none. Model validity
alone does not establish readiness or authorize implementation.

The proposal is an unaccepted sketch. Review its interpretation under the current
mandate; imports and proposals do not change accepted requirements. Materialize
`model_candidate` relative to the proposal's original source URI, or explicitly
adjust that locator. Retain the observation source as non-normative provenance.
Verify source existence/code bytes independently before registering bindings.

Validate and show the proposed graph, resolve its scoped contract questions using
actual requirements and code, and refine a bounded implementation target through
[the new-project route](new-project.md). Report observation integrity separately
from model validity/readiness. Continue implementation only when its own mandate,
readiness, and prerequisites are established.
