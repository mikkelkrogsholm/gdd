# GDD native viewer preview

From the repository root, open a model directly or save a view to share.

```sh
cargo run --locked -p gdd-viewer -- examples/fragmented.gdd.yaml
cargo run --locked -p gdd -- view examples/fragmented.gdd.yaml --scope component.notes --output notes.gddview
cargo run --locked -p gdd-viewer -- notes.gddview
```

Building the viewer requires Rust 1.95 or newer. Running it requires a desktop with OpenGL. On Linux it
uses X11 or Wayland. The native window was tested on Linux with Xvfb and Mesa
software rendering. The root CLI remains the default Cargo package.

Use the graph or the node list to select a node. Search by label, ID, or kind
with Ctrl+F (Command+F on macOS). Use Tab and Enter to select a search result
or follow a connection. Try searching for `entity.note` and following its
`edge.note.owner` connection to `component.notes`. The inspector shows the
node label, ID, kind, description and each incident edge. Selecting a node
highlights its incoming and outgoing links. Selecting a search result or
following a connection scrolls the graph to that node when it belongs to the
current perspective and saved scope.

Choose **All graph**, **User journey**, **Components**, or **Data model** under
**Perspective**. Each lens includes its primary concepts and direct context.
The list and canvas show the same nodes. An edge appears when the perspective
includes that edge and both endpoints remain visible. Search narrows the
current lens. Counts show visible nodes and edges against the complete model.

Select a journey step to read its user action and system response, or an actor
to read its goal. A **Related perspectives** link clears search and reveals
the selected node in that perspective within the saved scope. **Advisory gaps**
in the node panel identify missing details in a sketch. Selecting a gap opens
its node in All graph when the node is in scope. An outside target opens in
the inspector and keeps the current drawing and filters.

When sharing a saved view, send the `.gddview` file to a recipient with the
native viewer installed. Its search and drawing respect the saved scope.
Following a connection to an omitted node opens its inspector and leaves the
drawing in scope. A saved node's inspector also shows its source location and
full record. See the [artifact contract](../../VIEWER.md#share-a-native-view).

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
The viewer derives perspectives from that same captured bundle. If a directly
opened model can be drawn but fails perspective admission, All graph remains
available. Expand **Product perspectives unavailable** to read the diagnostic.
Use the CLI for validation reports and operational record status.

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
