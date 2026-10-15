/* Browser controls operate only on local view state. */
(() => {
  const { data, model, $, state, el, label } = GDD;
  $("project-title").textContent = model.project.title;
  $("model-mode").textContent = `Read-only view · GDD ${model.format.split("/")[1]}`;
  $("renderer-meta").textContent = `${data.generator.name} ${data.generator.version} · Offline`;
  document.title = `${model.project.title} · GDD`;
  $("total-count").textContent = `${Object.keys(model.nodes).length} nodes`;
  $("revision").textContent = `Model ${data.spec_revision}`;
  $("revision").title = data.spec_revision;
  function option(select, value, title) {
    const item = el("option", title);
    item.value = value;
    $(select).append(item);
  }
  [...new Set(Object.values(model.nodes).map((n) => n.kind))]
    .sort()
    .forEach((kind) => option("kind", kind, kind));
  [...new Set(model.edges.map((e) => e.relation))]
    .sort()
    .forEach((relation) => option("relation", relation, relation));
  GDD.communities.forEach((id) => option("community", id, label(id)));
  $("scenario-count").textContent =
    `(${Object.values(model.nodes).filter((n) => n.kind === "scenario").length})`;
  state.scenarios = data.scope.some(
    (id) => model.nodes[id].kind === "scenario",
  );
  $("scenarios").checked = state.scenarios;
  function list() {
    const box = $("object-list");
    box.replaceChildren();
    const objects =
      state.tab === "nodes"
        ? GDD.projection.nodes
        : GDD.projection.edges.map((e) => e.id);
    for (const id of objects) {
      const node = state.tab === "nodes" ? model.nodes[id] : null;
      const edge = node ? null : model.edges.find((e) => e.id === id);
      const item = el("button", undefined, "object-row");
      item.setAttribute("aria-controls", "detail");
      item.setAttribute(
        "aria-pressed",
        state.selected?.type === (node ? "node" : "edge") &&
          state.selected.id === id,
      );
      item.setAttribute(node ? "data-list-node" : "data-list-edge", id);
      const title = el("span", undefined, "row-title"),
        dot = el(
          "span",
          undefined,
          `type-dot type-${node && GDD.kinds.includes(node.kind) ? node.kind : "component"}`,
        );
      title.append(dot, document.createTextNode(node ? label(id) : id));
      const hidden = node && GDD.projection.representatives.get(id) !== id;
      item.append(
        title,
        el(
          "span",
          node
            ? `${node.kind}${hidden ? " · folded member" : ""}`
            : `${edge.from} → ${edge.to} · ${edge.relation}`,
          "row-meta",
        ),
      );
      item.onclick = () => GDD.select(node ? "node" : "edge", id);
      box.append(item);
    }
    if (!objects.length) box.append(el("p", "No matches.", "hint"));
    $("tab-nodes").setAttribute("aria-pressed", state.tab === "nodes");
    $("tab-edges").setAttribute("aria-pressed", state.tab === "edges");
  }
  function refresh(reflow = true) {
    GDD.project();
    GDD.layout(reflow);
    GDD.draw();
    list();
    const p = GDD.projection,
      totalNodes = Object.keys(model.nodes).length,
      totalEdges = model.edges.length;
    $("view-count").textContent =
      `${p.nodes.length}/${totalNodes} nodes · ${p.edges.length}/${totalEdges} relations`;
    $("projection-status").textContent = state.fold
      ? `${p.glyphs.length} glyphs · ${p.internal.length} folded internal relations`
      : `${totalNodes - p.nodes.length} nodes and ${totalEdges - p.edges.length} relations omitted`;
    $("fold-hint").textContent = state.fold
      ? "Overlap is represented in the first visible community by ID. All memberships can be inspected."
      : "Membership is grouping, not ownership.";
    $("empty").hidden = Boolean(p.nodes.length);
    GDD.showOverview();
    GDD.fit();
  }
  for (const key of ["search", "kind", "relation", "community"])
    $(key).addEventListener(key === "search" ? "input" : "change", () => {
      state[key] = $(key).value;
      refresh(false);
    });
  $("fold").onchange = () => {
    state.fold = $("fold").checked;
    refresh();
  };
  $("scenarios").onchange = () => {
    state.scenarios = $("scenarios").checked;
    refresh(false);
  };
  $("tab-nodes").onclick = () => {
    state.tab = "nodes";
    list();
  };
  $("tab-edges").onclick = () => {
    state.tab = "edges";
    list();
  };
  $("reset").onclick = () => {
    for (const key of ["search", "kind", "relation", "community"]) {
      state[key] = "";
      $(key).value = "";
    }
    state.fold = false;
    $("fold").checked = false;
    state.scenarios = true;
    $("scenarios").checked = true;
    state.tab = "nodes";
    refresh();
  };
  $("overview").onclick = GDD.showOverview;
  $("manifest-download").onclick = () => {
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(GDD.manifest(), null, 2) + "\n"], {
        type: "application/json",
      }),
    );
    const link = el("a");
    link.href = url;
    link.download = "gdd-view-state.json";
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };
  window.addEventListener("resize", GDD.fit);
  refresh();
})();
