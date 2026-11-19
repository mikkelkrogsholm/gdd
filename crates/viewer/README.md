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

Wide windows show the node list, graph and inspector side by side. Below 960
logical pixels wide, the viewer opens on **Nodes**. Choose **Nodes**, **Graph**
or **Details** to give that pane the full width. Use Tab and Enter to choose a
pane with the keyboard. On **Nodes**, expand **View filters** for perspective,
filters and folding. Scroll the list to reach the remaining controls and
results in short windows. **Graph** contains the camera controls.

Selecting a result or a graph item opens **Details**. Ctrl+F (Command+F on
macOS) opens **Nodes** and focuses search from any pane. Alt+Home opens the
overview in **Details**. Switching panes or resizing the window preserves
scope, filters, folding, camera settings and the selected record. Hover over a shortened
header title or open the overview to read the full project title.

In **Model overview**, use Tab to reach the **Theme** choices and Enter to select
**System**, **Light** or **Dark** for this window. **System** follows the color
preference reported by the desktop. When the window backend supplies no
preference, as on X11, the viewer uses light colors.

The inspector opens on **Model overview** with the project title, ID and
goal. Expand **Project fields** or a question to read its complete declaration.
Select a declared source to inspect its origin and metadata. **Model snapshot**
lists the asset paths and SHA-256 digests that identify the revision. A saved
view also identifies its generator and the number of external observation
inputs it contains.

**Current view** shows the active perspective, filters, zoom and folding.
Expand a count to select and copy the exact node and edge IDs that are visible,
filtered within scope, or outside the saved scope. Choose **Model overview**
or press Alt+Home to clear the selection and keep the filters, folding and
camera position. Each newly opened record or overview starts at the top,
including when you return to one you have already inspected.

Use the graph or the node list to select a node. Press Ctrl+F (Command+F on
macOS) to **Search graph**. Search matches node IDs and the full text of node
and edge records, including labels, descriptions, contracts, pseudocode and
exact numbers. Matching is case-insensitive. An edge match includes both
endpoints when they pass the active filters and scope. Press Enter in the
search field to inspect an exact node ID or the first result. Use Tab and
Enter to select another result or follow a connection. Try searching for
`entity.note` and following its `edge.note.owner` connection to
`component.notes`. The inspector shows the
node label, ID, kind, description and each incident edge. Selecting a node
highlights its incoming and outgoing links. Selecting a search result or
following a connection scrolls the graph to that node when it belongs to the
current perspective and saved scope.

The node and edge inspectors show the declaring **Model file** and **JSON
Pointer**. **Source references** and **Design references** show each declared
source ID, purpose and exact selector, including large ordinal values. Expand
**Inspect source** to see the declaration's location, URI and SHA-256 digest.
**Full source declaration** includes fields such as the producer and revision.
The inspector displays URIs as text and reads the metadata captured in the
model or saved view.

Choose **All graph**, **User journey**, **Components**, or **Data model** under
**Perspective**. Each lens includes its primary concepts and direct context.
The node list includes every visible canonical node. An edge appears when the perspective
includes that edge and both endpoints remain visible. Use **Kind** for a
specific node kind and **Community** for a community and its direct
`member_of` members. Shared members belong to each declared community.
**Relation** limits the edges between matching nodes and keeps isolated
matches in the list and canvas. These filters combine with search in the
current perspective. Counts show visible nodes and edges against the complete
model.

The graph initially hides scenarios and their incident edges unless a saved
scope seed is a scenario. **Show scenarios** gives the total number of
scenarios in the model. Check it to include scenarios that pass the current
filters. Selecting the `scenario` kind or entering a matching search also
reveals scenarios while preserving the checkbox setting. **Reset filters**
clears search and all three selectors, enables **Show scenarios**, and restores
All graph within the saved scope with folding off.

Expand **Community areas** in the **Nodes** pane to see each community with
a visible member or a visible community node. The numbered outlines enclose
those figures and may overlap. Each legend entry counts distinct visible
members declared by `member_of` edges. Select an entry with Tab and Enter to
inspect the community, including when a filter hides its own node. A shared
member contributes to each of its communities and appears once on the canvas.
When folded, every outline follows the figure representing that member.

**Show community areas** toggles the outlines and their headings. Search,
filters, node and edge counts, and the layout are preserved. **Fit graph**
includes the visible outlines. Headings retain their text size and appear
where they fit around the visible figures.

Enable **Fold communities** to represent direct members inside a visible
community. A shared member appears in the first visible community by canonical
ID. Select a folded member in the list to see its **Direct communities**
and the community marked **Representative**. Its original membership edges
appear under **Connections**. Search, perspective changes and navigation keep
folding enabled.

A folded figure shows how many canonical nodes it represents. Select the
figure to inspect its **Represented nodes**, **Internal edges** and **Directed
bundles**. Each arrow groups all visible edges with that ordered pair of
representatives. A badge such as **×3** marks three edges. Click the arrow,
badge or inspector bundle link to inspect each edge ID and its original
endpoints. A self-loop on the representative remains an arrow. A self-loop
on a folded member appears among the figure’s internal edges. The drawing
counts appear beneath the canonical node and edge counts.

Select a journey step to read its user action and system response, or an actor
to read its goal. A **Related perspectives** link clears the filters and reveals
the selected node in that perspective within the saved scope. **Advisory gaps**
in the node panel identify missing details in a sketch. Selecting a gap opens
its node in All graph when the node is in scope. An outside target opens in
the inspector and keeps the current drawing and filters.

When sharing a saved view, send the `.gddview` file to a recipient with the
native viewer installed. Its search and drawing respect the saved scope.
Following a connection to an omitted node opens its inspector and leaves the
drawing in scope. Expand **Canonical node record** or **Canonical edge record**
to inspect the full saved record. See the [artifact contract](../../VIEWER.md#share-a-native-view).

The viewer computes the unfolded left-to-right layered layout once using the
pure Rust graphlay library. It keeps related figures together using the first
in-scope direct community by canonical ID, or a chain of unique owners leading
to a community or component. Scenarios and rules can follow the nodes they
exercise, apply to or change. These placement choices preserve every declared
membership, including shared ones. The viewer packs groups into rows and routes
every arrow around the resulting node positions. Folding lays out the visible
figures and directed bundles. With at least 1,000 edges, graphlay computes
the layers for the whole drawing, and the viewer separates groups vertically
to preserve those layers. It also skips a pass that straightens edge chains.
Each folded bundle counts as
one edge. If the filters change during layout, the viewer draws the latest
projection. You can search the node list, inspect nodes and follow connections
while graphlay runs. When the drawing is ready, the canvas scrolls to your
selected node or its representative.

Arrows follow the routed paths into their declared target nodes, including
links that run back across a cycle.
In the unfolded drawing, parallel links have separate routes and repeated
self-loops nest outside the node box. Dense graphs can still have crossings.
**Fit graph** centers all visible nodes and routed edges, or the folded figures
and bundles, with a margin. The percentage updates as the window or the
filtered drawing changes. Below 40%, the overview shows shapes and routes.
Choose a fixed percentage under **Graph zoom** (50% to 200%) to read labels
and bundle counts. The list and inspector retain their text size.

Drag a node or folded figure with the primary mouse button to move it.
Its incident arrows and community outlines follow the pointer. Releasing the
button reroutes edges around the current node positions in the background.
A click opens the inspector. Dragging preserves the current selection.

Positions belong to this window. Unfolded positions survive filter changes
and folding. A new folded projection starts with its own layout. **Fit graph**
and **Reset camera** keep the positions you set. In Fit mode the drawing
fits the viewport again after you release the figure. Reopening the model or
`.gddview` restores the automatic layout. Dragging leaves both files untouched.

When the drawing extends beyond the canvas, drag the background with the
primary mouse button or scroll to pan. **Reset camera** restores 100% zoom
and returns to the top left of the current drawing. Smaller drawings center
in the available space. Both camera controls preserve the current scope,
filters, folding and selection.

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
