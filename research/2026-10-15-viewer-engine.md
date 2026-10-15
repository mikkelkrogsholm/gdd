# Viewer engine selection for GDD

Date: October 15, 2026.
Status: integrated in development package 0.8.0 / viewer 0.3.0.
The exploratory results below retain their original scope; see [the viewer guide](../VIEWER.md) for the delivered implementation.

## Decision

Use **Cytoscape.js** as the target rendering and interaction engine, with **fCoSE**
for the initial architecture layout. Keep the canonical GDD model, projections,
manifests, inspector text and validation outside the engine. Community memberships
remain explicit GDD relations.

This is a choice for GDD's architecture exploration and offline export needs,
not a claim that one library wins every graph-visualization benchmark.

| Candidate | Relevant capability | Assessment for GDD |
| --- | --- | --- |
| [Cytoscape.js](https://js.cytoscape.org/) | Graph model, styling, inspection, interaction and multiple layout extensions | Selected: strong fit for structured architecture exploration; MIT core with no external dependencies |
| [fCoSE](https://github.com/iVis-at-Bilkent/cytoscape.js-fcose) | Compound spring layout, label-aware dimensions and placement constraints | Selected layout extension; layout settings still need integration tests |
| [AntV G6](https://g6.antv.antgroup.com/en/manual/plugin/overview) | Built-in hulls, BubbleSets, layouts and interaction behaviors | Credible alternative with useful community visualization built in |
| [vis-network](https://visjs.github.io/vis-network/docs/network/) | Canvas network rendering, physics and clustering | Direct route to Graphify-style interaction; GDD community overlays still require integration |
| [Sigma.js](https://www.sigmajs.org/) | WebGL network exploration with Graphology | Worth considering for a demonstrated large-graph rendering bottleneck; no comparative scale benchmark was run here |

## Overlapping communities

Cytoscape compound nodes use a `parent` relationship. GDD communities may overlap,
so compound containment cannot represent every membership. Use independent visual
community layers, with shared nodes represented once and all memberships preserved.

The production viewer uses deterministic convex extent envelopes rather than this plugin.

The [Cytoscape BubbleSets extension](https://github.com/upsetjs/cytoscape.js-bubblesets)
is a candidate for these layers, **not an accepted production dependency yet**.
Its declared member collections were preserved in the spike, but an after-layout
check did not establish that every member center was inside its drawn outline.
Resolve the integration/configuration or choose another outline implementation
before claiming geometric correctness.

## Local feasibility checks

Temporary browser prototypes tested Cytoscape.js 3.34.4, fCoSE 2.2.0,
cytoscape-bubblesets 4.1.0 and G6 5.1.1 with Chromium 156.0.8078.4.
Both prototypes opened from single HTML files with networking disabled. Both
preserved the concert view's 33 node IDs, 74 edge IDs and three community member
collections. Node selection worked through actual browser clicks. Force layout
completed with finite node positions. This was not a performance benchmark.

Cytoscape core also preserved all seven node IDs and nine directed edge records
from the existing hostile multigraph fixture, including parallel edges, a
self-loop and the `constructor` node ID. The raw-id G6 fixture encountered
`getPorts is not a function`; an opaque internal ID mapping may address it, but
that mapping was not verified. This is a limitation of that attempted adapter,
not proof that G6 cannot support the model.

These exploratory checks do not replace the viewer conformance suite. Before
delivery of the migration, verify offline packaging and exact artifact checks,
all current browser interactions, readable community labels and overlap, stable
filtering, accessible inspection, and representative larger graphs. Publish scale
claims only for workloads actually measured.
