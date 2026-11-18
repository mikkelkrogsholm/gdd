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
exact node fields and each incident edge, including parallel edges and self-loops.

Open a GDD 0.2 or 0.3 JSON or YAML manifest. The preview uses the Rust CLI's
bundle loader to combine the manifest and its declared local fragments.
A load problem, such as a missing asset or duplicate ID, stops the preview
with a source diagnostic. The preview leaves the bundle untouched, works
offline, and displays the graph. Its basic layout can overlap edges.
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
