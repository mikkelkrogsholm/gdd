# GDD native viewer preview

From the repository root, run:

```sh
cargo run --locked -p gdd-viewer -- examples/product/greenfield.gdd.json
```

The viewer requires Rust 1.95 or newer and a desktop with OpenGL. On Linux it
uses X11 or Wayland. The native window was tested on Linux with Xvfb and Mesa
software rendering. The root CLI remains the default Cargo package.

Use the graph or the node list to select a node. Search by label, ID, or kind
with Ctrl+F (Command+F on macOS). Use Tab and Enter to select a search result
or follow a connection. The inspector shows the exact node fields and each
incident edge, including parallel edges and self-loops.

The preview reads a local GDD 0.2 or 0.3 JSON model with embedded nodes and edges.
It checks the fields it needs to display the graph, including edge endpoints.
Use the GDD validator for schema and semantic checks. The preview leaves the
model file untouched and supports navigation without network access. Its basic
layout can overlap edges, and it does not load fragment includes, extensions,
or perspectives.
