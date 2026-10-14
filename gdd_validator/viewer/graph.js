/* GDD viewer: projection and deterministic SVG layout. No model mutations. */
"use strict";
const GDD = (() => {
  const data = JSON.parse(document.getElementById("gdd-data").textContent);
  const model = data.model;
  const $ = (id) => document.getElementById(id);
  const state = {
    search: "",
    kind: "",
    relation: "",
    community: "",
    scenarios: false,
    fold: false,
    tab: "nodes",
    selected: null,
  };
  const positions = new Map();
  let projection = {
    nodes: [],
    edges: [],
    glyphs: [],
    bundles: [],
    internal: [],
    representatives: new Map(),
  };
  let camera = { x: 0, y: 0, k: 1 };
  const communities = Object.keys(model.nodes)
    .filter((id) => model.nodes[id].kind === "community")
    .sort();
  const memberships = new Map(Object.keys(model.nodes).map((id) => [id, []]));
  for (const edge of model.edges) {
    if (edge.relation === "member_of") memberships.get(edge.from).push(edge.to);
  }
  for (const [id, list] of memberships)
    memberships.set(id, [...new Set(list)].sort());
  const kinds = [
    "component",
    "action",
    "entity",
    "rule",
    "scenario",
    "machine",
    "screen",
    "community",
  ];
  const symbols = new Map([
    ["component", "C"],
    ["action", "A"],
    ["entity", "E"],
    ["rule", "R"],
    ["scenario", "S"],
    ["machine", "M"],
    ["screen", "UI"],
    ["community", "◉"],
  ]);
  function el(tag, text, className) {
    const item = document.createElement(tag);
    if (text !== undefined) item.textContent = text;
    if (className) item.className = className;
    return item;
  }
  function svg(tag, attrs = {}, text) {
    const item = document.createElementNS("http://www.w3.org/2000/svg", tag);
    for (const [key, value] of Object.entries(attrs))
      item.setAttribute(key, value);
    if (text !== undefined) item.textContent = text;
    return item;
  }
  const label = (id) => model.nodes[id].label || id;
  const short = (text) => (text.length > 27 ? text.slice(0, 24) + "…" : text);
  function project() {
    const available = new Set(data.manifest.shown_nodes);
    const query = state.search.trim().toLocaleLowerCase();
    const matchingEdges = new Set(
      model.edges
        .filter((e) =>
          data.text.edges[e.id].toLocaleLowerCase().includes(query),
        )
        .flatMap((e) => [e.from, e.to]),
    );
    const nodes = [...available].filter((id) => {
      const node = model.nodes[id];
      return (
        (!state.kind || node.kind === state.kind) &&
        (state.scenarios ||
          node.kind !== "scenario" ||
          state.kind === "scenario" ||
          Boolean(query)) &&
        (!state.community ||
          id === state.community ||
          memberships.get(id).includes(state.community)) &&
        (!query ||
          id.toLocaleLowerCase().includes(query) ||
          data.text.nodes[id].full.toLocaleLowerCase().includes(query) ||
          matchingEdges.has(id))
      );
    });
    const included = new Set(nodes);
    const edges = model.edges.filter(
      (e) =>
        included.has(e.from) &&
        included.has(e.to) &&
        (!state.relation || e.relation === state.relation),
    );
    const representatives = new Map(nodes.map((id) => [id, id]));
    if (state.fold) {
      // Overlap stays explicit in data/inspector. One deterministic visual representative per member.
      for (const id of nodes) {
        const candidates = memberships.get(id).filter((c) => included.has(c));
        if (model.nodes[id].kind !== "community" && candidates.length)
          representatives.set(id, candidates[0]);
      }
    }
    const glyphMap = new Map();
    for (const id of nodes) {
      const rep = representatives.get(id);
      if (!glyphMap.has(rep)) glyphMap.set(rep, { id: rep, members: [] });
      glyphMap.get(rep).members.push(id);
    }
    const bundleMap = new Map(),
      internal = [];
    for (const edge of edges) {
      const from = representatives.get(edge.from),
        to = representatives.get(edge.to);
      if (from === to && (from !== edge.from || to !== edge.to)) {
        internal.push(edge.id);
        continue;
      }
      const key = JSON.stringify([from, to]);
      if (!bundleMap.has(key)) bundleMap.set(key, { key, from, to, edges: [] });
      bundleMap.get(key).edges.push(edge);
    }
    projection = {
      nodes,
      edges,
      glyphs: [...glyphMap.values()],
      bundles: [...bundleMap.values()],
      internal,
      representatives,
    };
    updateManifest();
    return projection;
  }
  function manifest() {
    return {
      format: "gdd-view-state/0.2",
      generator: data.generator,
      spec_revision: data.spec_revision,
      observation_inputs: data.observation_inputs,
      scope: data.scope,
      artifact_scope: data.manifest,
      filters: {
        search: state.search,
        kind: state.kind,
        relation: state.relation,
        community: state.community,
        scenarios: state.scenarios,
      },
      folding: {
        enabled: state.fold,
        overlap_policy:
          "first visible community by canonical ID; all memberships retained",
      },
      shown_nodes: [...projection.nodes].sort(),
      shown_edges: projection.edges.map((e) => e.id).sort(),
      omitted_nodes: Object.keys(model.nodes)
        .filter((id) => !projection.nodes.includes(id))
        .sort(),
      omitted_edges: model.edges
        .filter((e) => !projection.edges.includes(e))
        .map((e) => e.id)
        .sort(),
      glyphs: projection.glyphs.map((g) => ({ id: g.id, node_ids: g.members })),
      bundles: projection.bundles.map((b) => ({
        from: b.from,
        to: b.to,
        edge_ids: b.edges.map((e) => e.id),
      })),
      folded_internal_edges: [...projection.internal].sort(),
    };
  }
  function updateManifest() {
    let item = $("gdd-view-manifest");
    if (!item) {
      item = el("script");
      item.id = "gdd-view-manifest";
      item.type = "application/json";
      document.body.append(item);
    }
    item.textContent = JSON.stringify(manifest());
  }
  function layout(reflow = true) {
    const glyphs = projection.glyphs;
    glyphs.forEach((g, i) => {
      if (positions.has(g.id)) return;
      const node = model.nodes[g.id],
        typeIndex = Math.max(0, kinds.indexOf(node.kind));
      const community = memberships.get(g.id)[0];
      const sector = community ? communities.indexOf(community) : typeIndex;
      const angle = i * 2.399963229728653,
        radius = 65 + 26 * Math.sqrt(i + 1);
      const base = (sector * 2 * Math.PI) / Math.max(communities.length, 8);
      positions.set(g.id, {
        x: 600 + Math.cos(base) * 170 + Math.cos(angle) * radius,
        y: 400 + Math.sin(base) * 140 + Math.sin(angle) * radius,
      });
    });
    if (!reflow || glyphs.length > 200) return; // Bounded layout work; larger graphs keep a deterministic seeded layout.
    for (let iteration = 0; iteration < 160; iteration++) {
      const forces = new Map(glyphs.map((g) => [g.id, { x: 0, y: 0 }]));
      for (let i = 0; i < glyphs.length; i++)
        for (let j = i + 1; j < glyphs.length; j++) {
          const a = positions.get(glyphs[i].id),
            b = positions.get(glyphs[j].id);
          const dx = a.x - b.x,
            dy = a.y - b.y,
            d2 = Math.max(dx * dx + dy * dy, 1),
            d = Math.sqrt(d2),
            force = Math.min(2400 / d2, 9);
          forces.get(glyphs[i].id).x += (dx / d) * force;
          forces.get(glyphs[i].id).y += (dy / d) * force;
          forces.get(glyphs[j].id).x -= (dx / d) * force;
          forces.get(glyphs[j].id).y -= (dy / d) * force;
        }
      for (const b of projection.bundles) {
        if (b.from === b.to) continue;
        const a = positions.get(b.from),
          z = positions.get(b.to),
          dx = z.x - a.x,
          dy = z.y - a.y,
          d = Math.max(Math.hypot(dx, dy), 1),
          force = (d - 135) * 0.012;
        forces.get(b.from).x += (dx / d) * force;
        forces.get(b.from).y += (dy / d) * force;
        forces.get(b.to).x -= (dx / d) * force;
        forces.get(b.to).y -= (dy / d) * force;
      }
      for (const g of glyphs) {
        const p = positions.get(g.id),
          f = forces.get(g.id),
          cool = 1 - iteration / 190;
        p.x += (f.x + (600 - p.x) * 0.004) * cool;
        p.y += (f.y + (400 - p.y) * 0.004) * cool;
      }
    }
  }
  function edgeGeometry(bundle) {
    const a = positions.get(bundle.from),
      b = positions.get(bundle.to);
    if (bundle.from === bundle.to)
      return {
        d: `M ${a.x - 12} ${a.y - 14} C ${a.x - 75} ${a.y - 95}, ${a.x + 75} ${a.y - 95}, ${a.x + 13} ${a.y - 16}`,
        x: a.x,
        y: a.y - 62,
      };
    const dx = b.x - a.x,
      dy = b.y - a.y,
      d = Math.max(Math.hypot(dx, dy), 1),
      ux = dx / d,
      uy = dy / d;
    const reverse = projection.bundles.some(
      (other) => other.from === bundle.to && other.to === bundle.from,
    );
    const offset = reverse ? 25 : 0,
      cx = (a.x + b.x) / 2 - uy * offset,
      cy = (a.y + b.y) / 2 + ux * offset;
    return {
      d: `M ${a.x + ux * 21} ${a.y + uy * 21} Q ${cx} ${cy} ${b.x - ux * 23} ${b.y - uy * 23}`,
      x: (a.x + b.x) / 2 - (uy * offset) / 2,
      y: (a.y + b.y) / 2 + (ux * offset) / 2 - 7,
    };
  }
  function draw() {
    $("edge-layer").replaceChildren();
    $("node-layer").replaceChildren();
    for (const bundle of projection.bundles) {
      const geometry = edgeGeometry(bundle),
        group = svg("g", {
          class: "edge-group",
          "data-edge-ids": JSON.stringify(bundle.edges.map((e) => e.id)),
        });
      group.append(
        svg("path", { class: "edge-path", d: geometry.d }),
        svg("path", { class: "edge-hit", d: geometry.d }),
      );
      const names = [...new Set(bundle.edges.map((e) => e.relation))];
      const text =
        names.length === 1 ? names[0] : `${names[0]} +${names.length - 1}`;
      group.append(
        svg(
          "text",
          { class: "edge-label", x: geometry.x, y: geometry.y },
          text + (bundle.edges.length > 1 ? ` ×${bundle.edges.length}` : ""),
        ),
      );
      group.append(
        svg(
          "title",
          {},
          bundle.edges
            .map((e) => `${e.id}: ${e.from} → ${e.to} · ${e.relation}`)
            .join("\n"),
        ),
      );
      group.addEventListener("click", () => GDD.select("bundle", bundle.key));
      $("edge-layer").append(group);
    }
    for (const glyph of projection.glyphs) {
      const node = model.nodes[glyph.id],
        p = positions.get(glyph.id),
        type = kinds.includes(node.kind) ? node.kind : "component";
      const folded = glyph.members.length > 1;
      const group = svg("g", {
        class: `node-group type-${type}${folded ? " folded" : ""}${node.maturity === "specified" ? "" : " sketch"}`,
        "data-node-id": glyph.id,
        "data-member-ids": JSON.stringify(glyph.members),
        transform: `translate(${p.x},${p.y})`,
      });
      group.append(
        svg("circle", { class: "node-glyph", r: folded ? 22 : 18 }),
        svg("circle", { class: "node-accent", r: 3, cx: 14, cy: -13 }),
      );
      group.append(
        svg(
          "text",
          { class: "node-symbol", y: 5 },
          folded ? String(glyph.members.length) : symbols.get(node.kind) || "?",
        ),
      );
      group.append(
        svg(
          "text",
          { class: "node-label", y: 40 },
          short(
            model.nodes[glyph.id].label ||
              glyph.id.split(".").slice(1).join(".") ||
              glyph.id,
          ),
        ),
      );
      group.append(
        svg(
          "text",
          { class: "node-kind", y: 54 },
          folded ? "foldet community" : node.kind,
        ),
      );
      group.append(svg("title", {}, `${glyph.id}\n${node.description}`));
      group.addEventListener("click", () => {
        if (!dragged) GDD.select("node", glyph.id);
      });
      group.addEventListener("pointerdown", (event) =>
        startDrag(event, glyph.id),
      );
      $("node-layer").append(group);
    }
    highlight();
    applyCamera();
  }
  function highlight() {
    const selection = state.selected,
      selectedNodes = new Set(),
      selectedEdges = new Set();
    if (selection?.type === "node") {
      selectedNodes.add(projection.representatives.get(selection.id));
      for (const e of projection.edges)
        if (e.from === selection.id || e.to === selection.id) {
          selectedNodes.add(projection.representatives.get(e.from));
          selectedNodes.add(projection.representatives.get(e.to));
          selectedEdges.add(e.id);
        }
    } else if (selection?.type === "edge") selectedEdges.add(selection.id);
    else if (selection?.type === "bundle")
      for (const b of projection.bundles)
        if (b.key === selection.id)
          b.edges.forEach((e) => selectedEdges.add(e.id));
    for (const b of projection.bundles)
      if (b.edges.some((e) => selectedEdges.has(e.id))) {
        selectedNodes.add(b.from);
        selectedNodes.add(b.to);
      }
    for (const item of $("node-layer").children) {
      const id = item.getAttribute("data-node-id");
      item.classList.toggle(
        "selected",
        selection?.type === "node" &&
          projection.representatives.get(selection.id) === id,
      );
      item.classList.toggle("connected", selectedNodes.has(id));
      item.classList.toggle(
        "dim",
        selectedNodes.size > 0 && !selectedNodes.has(id),
      );
    }
    for (const item of $("edge-layer").children) {
      const active = JSON.parse(item.getAttribute("data-edge-ids")).some((id) =>
        selectedEdges.has(id),
      );
      item.classList.toggle("connected", active);
      item.classList.toggle("dim", selectedNodes.size > 0 && !active);
    }
  }
  function applyCamera() {
    $("viewport").setAttribute(
      "transform",
      `translate(${camera.x},${camera.y}) scale(${camera.k})`,
    );
    $("zoom-level").textContent = Math.round(camera.k * 100) + "%";
  }
  function fit() {
    const bounds = $("graph").getBoundingClientRect(),
      all = projection.glyphs.map((g) => positions.get(g.id));
    if (!all.length) return;
    const minX = Math.min(...all.map((p) => p.x)) - 100,
      maxX = Math.max(...all.map((p) => p.x)) + 100;
    const minY = Math.min(...all.map((p) => p.y)) - 90,
      maxY = Math.max(...all.map((p) => p.y)) + 100;
    const k = Math.min(
      (bounds.width - 30) / (maxX - minX),
      (bounds.height - 120) / (maxY - minY),
      1.6,
    );
    camera = {
      k: Math.max(0.08, k),
      x: bounds.width / 2 - ((minX + maxX) / 2) * k,
      y: bounds.height / 2 - ((minY + maxY) / 2) * k + 10,
    };
    applyCamera();
  }
  function zoom(factor, x, y) {
    const bounds = $("graph").getBoundingClientRect();
    x ??= bounds.width / 2;
    y ??= bounds.height / 2;
    const next = Math.max(0.05, Math.min(4, camera.k * factor)),
      ratio = next / camera.k;
    camera = {
      x: x - (x - camera.x) * ratio,
      y: y - (y - camera.y) * ratio,
      k: next,
    };
    applyCamera();
  }
  let drag = null,
    dragged = false;
  function startDrag(event, id = null) {
    if (event.button !== 0) return;
    event.stopPropagation();
    dragged = false;
    drag = {
      id,
      x: event.clientX,
      y: event.clientY,
      startX: event.clientX,
      startY: event.clientY,
    };
    $("graph").setPointerCapture(event.pointerId);
  }
  $("graph").addEventListener("pointerdown", (event) => {
    if (!event.target.closest(".edge-group")) startDrag(event);
  });
  $("graph").addEventListener("pointermove", (event) => {
    if (!drag) return;
    const dx = event.clientX - drag.x,
      dy = event.clientY - drag.y;
    if (
      Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) > 3
    )
      dragged = true;
    if (drag.id) {
      const p = positions.get(drag.id);
      p.x += dx / camera.k;
      p.y += dy / camera.k;
      draw();
    } else {
      camera.x += dx;
      camera.y += dy;
      applyCamera();
    }
    drag.x = event.clientX;
    drag.y = event.clientY;
  });
  function endDrag() {
    if (drag?.id && !dragged) GDD.select("node", drag.id);
    drag = null;
    setTimeout(() => {
      dragged = false;
    }, 0);
  }
  $("graph").addEventListener("pointerup", endDrag);
  $("graph").addEventListener("pointercancel", () => {
    drag = null;
    dragged = false;
  });
  $("graph").addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      const r = $("graph").getBoundingClientRect();
      zoom(
        Math.exp(-event.deltaY * 0.0015),
        event.clientX - r.left,
        event.clientY - r.top,
      );
    },
    { passive: false },
  );
  $("zoom-in").onclick = () => zoom(1.2);
  $("zoom-out").onclick = () => zoom(1 / 1.2);
  $("fit").onclick = fit;
  return {
    data,
    model,
    $,
    state,
    el,
    label,
    memberships,
    communities,
    kinds,
    project,
    layout,
    draw,
    fit,
    highlight,
    manifest,
    get projection() {
      return projection;
    },
  };
})();
