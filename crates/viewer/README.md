# GDD native viewer preview

From the repository root, run:

```sh
cargo run --locked -p gdd-viewer -- examples/concert-0.3.gdd.yaml
```

The viewer requires Rust 1.95 or newer and a desktop with OpenGL. On Linux it
uses X11 or Wayland. The native window was tested on Linux with Xvfb and Mesa
software rendering. The root CLI remains the default Cargo package.

Use the graph or the node list to select a node. Search by label, ID, or kind
with Ctrl+F (Command+F on macOS). Use Tab and Enter to select a search result
or follow a connection. The inspector shows the exact node fields and each
incident edge, including parallel edges and self-loops.

The preview opens standalone GDD 0.2 and 0.3 JSON or YAML files with embedded
nodes and edges. It checks the document's syntax and the fields needed to
display the graph, including edge endpoints. Use the GDD validator for schema,
semantic and profile checks. Fix duplicate IDs or keys in the source file,
quote YAML strings that would otherwise resolve to booleans, numbers or null,
and expand any YAML anchors, aliases or merge keys into explicit values.

The preview leaves the model file untouched and supports navigation without
network access. Its basic layout can overlap edges. Select a standalone model
to preview an embedded graph. Fragment includes require the bundle-aware
validator. The preview displays the graph's embedded records without loading
extensions or perspectives.

The Rust parser keeps finite numbers as their original decimal text, including
values outside the Python validator's numeric conversion limits. It also
reserves bare `NaN`, `Infinity`, and `-Infinity` in YAML. Quote them to use them
as strings.
