# GDD native viewer preview

From the repository root, open the fragmented example:

```sh
cargo run --locked -p gdd-viewer -- examples/fragmented.gdd.yaml
```

The viewer requires Rust 1.95 or newer and a desktop with OpenGL. On Linux it
uses X11 or Wayland. The native window was tested on Linux with Xvfb and Mesa
software rendering. The root CLI remains the default Cargo package.

Use the graph or the node list to select a node. Search by label, ID, or kind
with Ctrl+F (Command+F on macOS). Use Tab and Enter to select a search result
or follow a connection. Try searching for `entity.note` and following its
`edge.note.owner` connection to `component.notes`. The inspector shows the
node label, ID, kind, description and each incident edge. Selecting a node
highlights its incoming and outgoing links. Selecting a search result or
following a connection scrolls the graph to that node.

The viewer computes a left-to-right layered layout once when it opens a model,
using the pure Rust graphlay library. While the layout runs in the background,
you can search the node list, inspect nodes, and follow connections. When the
drawing is ready, the canvas scrolls to your current selection.

Arrows follow the routed paths into
their declared target nodes, including links that run back across a cycle.
Parallel links have separate routes, and repeated self-loops nest outside the
node box. Dense graphs can still have crossings. Scroll in both directions
to explore them, or use **Graph zoom** to change the canvas scale from 50% to
200%. The list and inspector retain their text size.

Open a GDD 0.2 or 0.3 JSON or YAML manifest. The preview uses the Rust CLI's
bundle loader to combine the manifest and its declared local fragments.
A load problem, such as a missing asset or duplicate ID, stops the preview
with a source diagnostic. The preview runs offline and leaves the bundle
untouched. A layout error leaves the node list and inspector available.
Validation results, operational record status, and perspectives are outside
the preview.

Use the Rust CLI to check schema and model semantics or get the bundle's
revision and asset digests:

```sh
cargo run --locked -- validate examples/fragmented.gdd.yaml --profile model
cargo run --locked -- snapshot examples/fragmented.gdd.yaml
```

The validation report uses exit code 0 for pass, 1 for fail, and 2 for unknown.
The snapshot command returns a snapshot for a valid bundle and a report if
the checks fail or remain unknown.

Quote YAML strings that would otherwise resolve to booleans, numbers, or null,
and expand YAML anchors, aliases, or merge keys into explicit values. The Rust
parser keeps finite numbers as their original decimal text, including values
outside the Python validator's numeric conversion limits. It reserves bare
`NaN`, `Infinity`, and `-Infinity` in YAML. Quote them to use them as strings.
