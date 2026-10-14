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
  // Layout affinities are visual hints. They never add memberships or ownership.
  function layoutGroups() {
    const available = new Set(data.manifest.shown_nodes);
    const groups = new Map();
    for (const id of available) {
      const node = model.nodes[id];
      const explicit = memberships.get(id).filter((c) => available.has(c));
      groups.set(id, node.kind === "community" ? [id] : explicit);
    }
    const candidates = new Map();
    for (const edge of model.edges) {
      if (edge.relation !== "owned_by" || !available.has(edge.to)) continue;
      if (!candidates.has(edge.from)) candidates.set(edge.from, new Set());
      candidates.get(edge.from).add(edge.to);
    }
    const owners = new Map([...candidates].filter(([, values]) => values.size === 1)
      .map(([id, values]) => [id, [...values][0]]));
    for (const id of available)
      if (!groups.get(id).length && model.nodes[id].kind === "component" && !owners.has(id))
        groups.set(id, [id]);
    for (const id of available) {
      const path = [], seen = new Set();
      let current = id;
      while (!groups.get(current)?.length && owners.has(current) && !seen.has(current)) {
        path.push(current); seen.add(current); current = owners.get(current);
      }
      const inherited = groups.get(current) || [];
      if (inherited.length) path.forEach((child) => groups.set(child, inherited));
    }
    // Scenarios and rules follow the actual nodes they exercise/apply to.
    // Resolve each wave together so input edge order cannot change the result.
    for (let wave = 0; wave < 3; wave++) {
      const additions = new Map();
      for (const edge of model.edges) {
        if (!["exercises", "applies_to", "changes"].includes(edge.relation)) continue;
        if (!available.has(edge.from) || !available.has(edge.to) || groups.get(edge.from).length) continue;
        if (!additions.has(edge.from)) additions.set(edge.from, new Set());
        groups.get(edge.to).forEach((id) => additions.get(edge.from).add(id));
      }
      for (const [id, values] of additions) groups.set(id, [...values].sort());
    }
    return groups;
  }
  const affinities = layoutGroups();
  function layout(reflow = true) {
    const glyphs = [...projection.glyphs].sort((a, b) =>
      a.id < b.id ? -1 : a.id > b.id ? 1 : 0,
    );
    const counts = new Map();
    for (const glyph of glyphs) {
      const groups = affinities.get(glyph.id);
      for (const group of groups)
        counts.set(group, (counts.get(group) || 0) + 1 / groups.length);
    }
    const keys = [...counts.keys()].sort();
    const radii = keys.map((key) => Math.max(100, Math.sqrt(counts.get(key)) * 55));
    const ring = keys.length < 2 ? 0 : radii.reduce((sum, radius) => sum + radius, 0) /
      (keys.length * Math.sin(Math.PI / keys.length)) + 85;
    const centers = new Map(keys.map((key, i) => {
      const angle = Math.PI + i * 2 * Math.PI / keys.length;
      return [key, {x: 600 + Math.cos(angle) * ring, y: 400 + Math.sin(angle) * ring}];
    }));
    const textMeasure = document.createElement("canvas").getContext("2d");
    const family = getComputedStyle(document.body).fontFamily;
    function width(text, font) {
      if (!textMeasure) return text.length * 7;
      textMeasure.font = `550 ${font}px ${family}`;
      return textMeasure.measureText(text).width;
    }
    const seeds = new Map();
    let added = false;
    const nodes = glyphs.map((glyph) => {
      const groups = affinities.get(glyph.id);
      // Multiple memberships pull toward their mean, leaving shared nodes as bridges.
      const target = groups.length ? {
        x: groups.reduce((sum, key) => sum + centers.get(key).x, 0) / groups.length,
        y: groups.reduce((sum, key) => sum + centers.get(key).y, 0) / groups.length,
      } : {x: 600, y: 400};
      const seedKey = JSON.stringify(groups);
      const index = seeds.get(seedKey) || 0;
      seeds.set(seedKey, index + 1);
      const title = short(
        model.nodes[glyph.id].label || glyph.id.split(".").slice(1).join(".") || glyph.id,
      );
      const kind = glyph.members.length > 1
        ? "folded community" : model.nodes[glyph.id].kind;
      const halfWidth = Math.max(28, width(title, 11) / 2, width(kind, 9) / 2) + 12;
      if (!positions.has(glyph.id)) {
        const angle = index * 2.399963229728653;
        const radius = 70 * Math.sqrt(index);
        positions.set(glyph.id, {
          x: target.x + Math.cos(angle) * radius,
          y: target.y + Math.sin(angle) * radius,
        });
        added = true;
      }
      return {
        id: glyph.id, groups, target, halfWidth, p: positions.get(glyph.id),
        vx: 0, vy: 0, fx: 0, fy: 0,
      };
    });
    if (!nodes.length || (!reflow && !added)) return;
    const indexed = new Map(nodes.map((node) => [node.id, node]));
    const links = projection.bundles.filter((bundle) => bundle.from !== bundle.to).map((bundle) => {
      const a = indexed.get(bundle.from), b = indexed.get(bundle.to);
      const shared = a.groups.some((group) => b.groups.includes(group));
      return {a, b, length: shared ? 125 : 280, strength: shared ? 0.018 : 0.004};
    });
    // Rectangle envelopes reserve room for both the glyph and its two text lines.
    function collision(a, b, strength, direct = false) {
      const dx = a.p.x - b.p.x, dy = a.p.y - b.p.y;
      const x = a.halfWidth + b.halfWidth - Math.abs(dx), y = 96 - Math.abs(dy);
      if (x <= 0 || y <= 0) return;
      const axis = x < y ? "x" : "y";
      const distance = axis === "x" ? dx : dy;
      const shift = (axis === "x" ? x : y) * strength * (distance < 0 ? -1 : 1);
      if (direct) {
        a.p[axis] += shift;
        b.p[axis] -= shift;
      } else {
        const forceAxis = axis === "x" ? "fx" : "fy";
        a[forceAxis] += shift;
        b[forceAxis] -= shift;
      }
    }
    // Cap force work; larger projections retain separated, deterministic group seeds.
    const iterations = nodes.length <= 200 ? 240 : 0;
    for (let iteration = 0; iteration < iterations; iteration++) {
      const alpha = Math.max(0.08, 1 - iteration / iterations);
      for (const node of nodes) { node.fx = 0; node.fy = 0; }
      for (let i = 0; i < nodes.length; i++)
        for (let j = i + 1; j < nodes.length; j++) {
          const a = nodes[i], b = nodes[j];
          let dx = a.p.x - b.p.x, dy = a.p.y - b.p.y;
          if (dx === 0 && dy === 0) { dx = Math.cos(i + j); dy = Math.sin(i + j); }
          const d = Math.max(Math.hypot(dx, dy), 1);
          const force = Math.min(7200 / (d * d), 18);
          a.fx += dx / d * force; a.fy += dy / d * force;
          b.fx -= dx / d * force; b.fy -= dy / d * force;
          collision(a, b, 0.4);
        }
      for (const {a, b, length, strength} of links) {
        const dx = b.p.x - a.p.x, dy = b.p.y - a.p.y, d = Math.max(Math.hypot(dx, dy), 1);
        const force = (d - length) * strength;
        a.fx += dx / d * force; a.fy += dy / d * force;
        b.fx -= dx / d * force; b.fy -= dy / d * force;
      }
      for (const node of nodes) {
        node.vx = (node.vx + (node.fx + (node.target.x - node.p.x) * 0.045) * alpha) * 0.68;
        node.vy = (node.vy + (node.fy + (node.target.y - node.p.y) * 0.045) * alpha) * 0.68;
        const limit = Math.max(1, Math.hypot(node.vx, node.vy) / 16);
        node.p.x += node.vx / limit; node.p.y += node.vy / limit;
      }
    }
    if (iterations)
      for (let pass = 0; pass < 18; pass++)
        for (let i = 0; i < nodes.length; i++)
          for (let j = i + 1; j < nodes.length; j++) collision(nodes[i], nodes[j], 0.51, true);
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
          folded ? "folded community" : node.kind,
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
