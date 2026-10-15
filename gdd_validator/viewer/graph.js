/* Canonical projection and the Cytoscape/fCoSE drawing adapter. */
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
  const nodeId = (id) => `n:${id}`;
  const edgeId = (key) => `e:${key}`;
  const nodeElement = (id) => cy.getElementById(nodeId(id));
  const bundleElement = (key) => cy.getElementById(edgeId(key));
  const palette = ["#157d73", "#a34d99", "#b27a12", "#3977ae", "#aa5837", "#687f2f", "#735eb0", "#277f92"];
  const communityColor = (id) => palette[communities.indexOf(id) % palette.length];
  const theme = (name) => getComputedStyle(document.body).getPropertyValue(name).trim();

  function glyphCommunities(glyph) {
    const ids = new Set(model.nodes[glyph.id].kind === "community" ? [glyph.id] : []);
    for (const id of glyph.members) memberships.get(id).forEach((c) => ids.add(c));
    return [...ids].sort();
  }
  function glyphImage(glyph) {
    const groups = glyphCommunities(glyph), parts = [];
    if (groups.length === 1) {
      parts.push(`<circle cx="24" cy="24" r="21" fill="none" stroke="${communityColor(groups[0])}" stroke-width="3"/>`);
    } else if (groups.length > 1) {
      groups.forEach((group, i) => {
        const a = i * 2 * Math.PI / groups.length - Math.PI / 2;
        const b = (i + 1) * 2 * Math.PI / groups.length - Math.PI / 2;
        parts.push(`<path d="M ${24 + 21 * Math.cos(a)} ${24 + 21 * Math.sin(a)} A 21 21 0 ${b - a > Math.PI ? 1 : 0} 1 ${24 + 21 * Math.cos(b)} ${24 + 21 * Math.sin(b)}" fill="none" stroke="${communityColor(group)}" stroke-width="3"/>`);
      });
    }
    const symbol = glyph.members.length > 1 ? String(glyph.members.length) : symbols.get(model.nodes[glyph.id].kind) || "?";
    parts.push(`<text x="24" y="29" text-anchor="middle" font-family="sans-serif" font-size="14" font-weight="600" fill="${theme('--ink')}">${symbol}</text>`);
    return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">${parts.join('')}</svg>`)}`;
  }
  function engineStyle() {
    return [
      {selector: "node", style: {
        width: 44, height: 44, "background-color": theme("--node"),
        "background-image": "data(image)", "background-fit": "contain",
        "border-color": theme("--line"), "border-width": 1.5,
        label: "data(caption)", color: theme("--ink"), "font-family": "system-ui, sans-serif",
        "font-size": 11, "font-weight": 500, "text-wrap": "wrap",
        "text-valign": "bottom", "text-margin-y": 8,
        "text-outline-color": theme("--canvas"), "text-outline-width": 2,
        "overlay-opacity": 0, "min-zoomed-font-size": 0,
      }},
      {selector: "node.sketch", style: {"border-style": "dashed"}},
      {selector: "node.folded", style: {width: 52, height: 52, "background-color": theme("--soft")}},
      {selector: "edge", style: {
        width: 1.3, "line-color": theme("--edge"), "target-arrow-color": theme("--edge"),
        "target-arrow-shape": "triangle", "arrow-scale": 0.8, "curve-style": "bezier",
        "control-point-step-size": 45, "loop-direction": "-90deg", "loop-sweep": "65deg",
        label: "data(caption)", "font-family": "system-ui, sans-serif", "font-size": 9,
        color: theme("--muted"), "text-opacity": 0, "text-rotation": "autorotate",
        "text-background-color": theme("--canvas"), "text-background-opacity": 0.9,
        "text-background-padding": 3, "overlay-opacity": 0,
      }},
      {selector: "node.connected", style: {"border-color": theme("--accent"), "border-width": 2.5}},
      {selector: "node.chosen", style: {"border-color": theme("--accent"), "border-width": 4}},
      {selector: "edge.connected, edge.hovered", style: {
        "line-color": theme("--accent"), "target-arrow-color": theme("--accent"),
        width: 2.2, "text-opacity": 1,
      }},
      {selector: ".dim", style: {opacity: 0.16}},
    ];
  }
  const cy = GDDVendor.cytoscape({
    container: $("graph"), elements: [], style: engineStyle(),
    layout: {name: "preset"}, minZoom: 0.03, maxZoom: 4,
    boxSelectionEnabled: false, autounselectify: true,
    selectionType: "single", pixelRatio: "auto",
  });
  let layoutRunning = false;
  function rememberPositions() {
    cy.nodes().forEach((node) => positions.set(node.data("canonicalId"), {...node.position()}));
  }

  function seedPositions(glyphs) {
    const counts = new Map(), seeds = new Map();
    for (const glyph of glyphs) {
      const group = affinities.get(glyph.id)[0] || "";
      counts.set(group, (counts.get(group) || 0) + 1);
    }
    const groups = [...counts.keys()].sort();
    const radius = groups.length < 2 ? 0 : Math.max(250, ...[...counts.values()].map((n) => Math.sqrt(n) * 85)) /
      Math.sin(Math.PI / groups.length);
    let added = false;
    for (const glyph of glyphs) {
      const group = affinities.get(glyph.id)[0] || "";
      const index = seeds.get(group) || 0;
      seeds.set(group, index + 1);
      if (positions.has(glyph.id)) continue;
      const angle = Math.PI + groups.indexOf(group) * 2 * Math.PI / groups.length;
      const spiral = index * 2.399963229728653, distance = 85 * Math.sqrt(index + 1);
      positions.set(glyph.id, {x: Math.cos(angle) * radius + Math.cos(spiral) * distance,
        y: Math.sin(angle) * radius + Math.sin(spiral) * distance});
      added = true;
    }
    return added;
  }

  function separateCaptionBounds(points) {
    // Captions extend below their node centers. Resolve their actual rectangles
    // after fCoSE, preserving horizontal placement and deterministic vertical order.
    points.sort((a, b) => a.y - b.y || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
    for (let i = 0; i < points.length; i++) {
      const point = points[i];
      for (let j = 0; j < i; j++) {
        const previous = points[j];
        if (point.x + point.left < previous.x + previous.right + 12 &&
            previous.x + previous.left < point.x + point.right + 12)
          point.y = Math.max(point.y, previous.y + previous.bottom + 12 - point.top);
      }
    }
    const x1 = Math.min(...points.map((p) => p.x + p.left));
    const y1 = Math.min(...points.map((p) => p.y + p.top));
    const x2 = Math.max(...points.map((p) => p.x + p.right));
    const y2 = Math.max(...points.map((p) => p.y + p.bottom));
    return {width: x2 - x1, height: y2 - y1,
      points: points.map((p) => ({id: p.id, x: p.x - x1, y: p.y - y1}))};
  }

  function layout(reflow = true) {
    rememberPositions();
    const glyphs = [...projection.glyphs].sort((a, b) => a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
    const added = seedPositions(glyphs);
    cy.startBatch();
    cy.elements().remove();
    cy.add(glyphs.map((glyph) => {
      const node = model.nodes[glyph.id];
      const title = short(node.label || glyph.id.split(".").slice(1).join(".") || glyph.id);
      const kindLabel = glyph.members.length > 1 ? "folded community" : node.kind;
      return {data: {id: nodeId(glyph.id), canonicalId: glyph.id, label: title, kindLabel,
        caption: `${title}\n${kindLabel}`, image: glyphImage(glyph)}, position: positions.get(glyph.id),
        classes: `${node.maturity === 'specified' ? '' : 'sketch'} ${glyph.members.length > 1 ? 'folded' : ''}`};
    }));
    cy.add(projection.bundles.map((bundle) => {
      const names = [...new Set(bundle.edges.map((edge) => edge.relation))];
      const caption = (names.length === 1 ? names[0] : `${names[0]} +${names.length - 1}`) +
        (bundle.edges.length > 1 ? ` ×${bundle.edges.length}` : "");
      return {data: {id: edgeId(bundle.key), source: nodeId(bundle.from), target: nodeId(bundle.to),
        bundleKey: bundle.key, edgeIds: bundle.edges.map((edge) => edge.id), caption}};
    }));
    cy.endBatch();
    if (glyphs.length < 2 || (!reflow && !added)) return;

    // Solve each visual affinity region with fCoSE, then pack the regions.
    // Affinity controls placement only; canonical memberships stay untouched.
    const groups = new Map();
    for (const glyph of glyphs) {
      const group = affinities.get(glyph.id)[0] || "";
      if (!groups.has(group)) groups.set(group, []);
      groups.get(group).push(glyph);
    }
    const regions = [];
    layoutRunning = true;
    try {
      for (const key of [...groups.keys()].sort((a, b) => a === "" ? 1 : b === "" ? -1 : a < b ? -1 : a > b ? 1 : 0)) {
        const members = groups.get(key), included = new Set(members.map((g) => g.id));
        const elements = members.map((glyph, index) => {
          const bounds = nodeElement(glyph.id).boundingBox({includeLabels: true, includeOverlays: false});
          const angle = index * 2.399963229728653, distance = 65 * Math.sqrt(index + 1);
          return {data: {id: nodeId(glyph.id), width: Math.max(65, bounds.w + 20), height: Math.max(115, bounds.h + 34)},
            position: {x: Math.cos(angle) * distance, y: Math.sin(angle) * distance}};
        });
        elements.push(...projection.bundles.filter((b) => b.from !== b.to && included.has(b.from) && included.has(b.to))
          .map((b) => ({data: {id: edgeId(b.key), source: nodeId(b.from), target: nodeId(b.to)}})));
        const calculation = GDDVendor.cytoscape({headless: true, styleEnabled: true, elements,
          layout: {name: "preset"},
          style: [{selector: "node", style: {width: "data(width)", height: "data(height)", padding: 0}}]});
        try {
          if (members.length > 1)
            calculation.layout({name: "fcose", quality: "proof", randomize: false, animate: false,
              fit: false, nodeDimensionsIncludeLabels: true, packComponents: false, tile: true,
              nodeRepulsion: 4500, idealEdgeLength: 85, edgeElasticity: 0.35,
              gravity: 0.3, numIter: 1800}).run();
          const points = members.map((glyph) => {
            const p = calculation.getElementById(nodeId(glyph.id)).position();
            if (!Number.isFinite(p.x) || !Number.isFinite(p.y)) throw new Error("Layout produced non-finite positions.");
            const node = nodeElement(glyph.id), center = node.position();
            const box = node.boundingBox({includeLabels: true, includeOverlays: false});
            return {id: glyph.id, x: p.x, y: p.y, left: box.x1 - center.x, right: box.x2 - center.x,
              top: box.y1 - center.y, bottom: box.y2 - center.y};
          });
          regions.push(separateCaptionBounds(points));
        } finally { calculation.destroy(); }
      }
      const gap = 70, view = $("graph").getBoundingClientRect();
      const area = regions.reduce((sum, r) => sum + (r.width + gap) * (r.height + gap), 0);
      const rowWidth = Math.max(...regions.map((r) => r.width), Math.sqrt(area * Math.max(0.5, view.width / view.height)) * 1.15);
      let x = 0, y = 0, rowHeight = 0;
      cy.batch(() => {
        for (const region of regions) {
          if (x && x + region.width > rowWidth) { x = 0; y += rowHeight + gap; rowHeight = 0; }
          for (const p of region.points) nodeElement(p.id).position({x: p.x + x, y: p.y + y});
          x += region.width + gap;
          rowHeight = Math.max(rowHeight, region.height);
        }
      });
      rememberPositions();
    } finally { layoutRunning = false; }
  }
  function highlight() {
    const selection = state.selected, selectedNodes = new Set(), selectedEdges = new Set();
    const community = selection?.type === "node" && model.nodes[selection.id]?.kind === "community";
    if (community) {
      for (const id of projection.nodes)
        if (id === selection.id || memberships.get(id).includes(selection.id))
          selectedNodes.add(projection.representatives.get(id));
      for (const edge of projection.edges)
        if (selectedNodes.has(projection.representatives.get(edge.from)) || selectedNodes.has(projection.representatives.get(edge.to)))
          selectedEdges.add(edge.id);
    } else if (selection?.type === "node") {
      selectedNodes.add(projection.representatives.get(selection.id));
      for (const edge of projection.edges)
        if (edge.from === selection.id || edge.to === selection.id) {
          selectedNodes.add(projection.representatives.get(edge.from));
          selectedNodes.add(projection.representatives.get(edge.to));
          selectedEdges.add(edge.id);
        }
    } else if (selection?.type === "edge") selectedEdges.add(selection.id);
    else if (selection?.type === "bundle")
      projection.bundles.find((bundle) => bundle.key === selection.id)?.edges.forEach((edge) => selectedEdges.add(edge.id));
    if (!community)
      for (const bundle of projection.bundles)
        if (bundle.edges.some((edge) => selectedEdges.has(edge.id))) {
          selectedNodes.add(bundle.from); selectedNodes.add(bundle.to);
        }
    cy.batch(() => {
      cy.nodes().forEach((node) => {
        const id = node.data("canonicalId");
        node.toggleClass("chosen", selection?.type === "node" && projection.representatives.get(selection.id) === id);
        node.toggleClass("connected", selectedNodes.has(id));
        node.toggleClass("dim", selectedNodes.size > 0 && !selectedNodes.has(id));
      });
      for (const bundle of projection.bundles) {
        const active = bundle.edges.some((edge) => selectedEdges.has(edge.id));
        bundleElement(bundle.key).toggleClass("connected", active).toggleClass("dim", selectedNodes.size > 0 && !active);
      }
    });
    GDD.drawCommunities?.();
  }
  function draw() { highlight(); }
  function fit() {
    cy.resize();
    if (!cy.nodes().length) return;
    cy.fit(cy.elements(), 85);
    if (cy.zoom() > 1.6) cy.zoom(1.6);
    cy.center();
    GDD.drawCommunities?.();
  }
  function zoom(factor) {
    const bounds = $("graph").getBoundingClientRect();
    cy.zoom({level: Math.max(cy.minZoom(), Math.min(cy.maxZoom(), cy.zoom() * factor)),
      renderedPosition: {x: bounds.width / 2, y: bounds.height / 2}});
  }
  cy.on("tap", "node", (event) => GDD.select("node", event.target.data("canonicalId")));
  cy.on("tap", "edge", (event) => GDD.select("bundle", event.target.data("bundleKey")));
  cy.on("mouseover", "edge", (event) => event.target.addClass("hovered"));
  cy.on("mouseout", "edge", (event) => event.target.removeClass("hovered"));
  cy.on("position", "node", () => {
    if (!layoutRunning) { rememberPositions(); GDD.drawCommunities?.(); }
  });
  cy.on("pan zoom", () => {
    $("zoom-level").textContent = Math.round(cy.zoom() * 100) + "%";
    GDD.drawCommunities?.();
  });
  $("zoom-in").onclick = () => zoom(1.2);
  $("zoom-out").onclick = () => zoom(1 / 1.2);
  $("fit").onclick = fit;
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    cy.style(engineStyle());
    for (const glyph of projection.glyphs) nodeElement(glyph.id).data("image", glyphImage(glyph));
    GDD.drawCommunities?.();
  });
  return {
    data, model, $, state, el, svg, label, memberships, communities, kinds, cy,
    project, layout, draw, fit, zoom, highlight, manifest, nodeElement, bundleElement, communityColor,
    get projection() { return projection; },
    get layoutRunning() { return layoutRunning; },
  };
})();
