/* Community extent envelopes are presentation; explicit member_of edges are truth. */
(() => {
  const { $, cy, svg, el, model, memberships, communities, nodeElement } = GDD;
  function hull(points) {
    const ordered = [...new Map(points.map((p) => [JSON.stringify(p), p])).values()]
      .sort((a, b) => a.x - b.x || a.y - b.y);
    const turn = (a, b, c) => (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    const chain = (list) => {
      const result = [];
      for (const p of list) {
        while (result.length > 1 && turn(result.at(-2), result.at(-1), p) <= 0) result.pop();
        result.push(p);
      }
      return result;
    };
    return [...chain(ordered).slice(0, -1), ...chain([...ordered].reverse()).slice(0, -1)];
  }
  function corners(box, padding = 0) {
    return [{x: box.x1 - padding, y: box.y1 - padding}, {x: box.x2 + padding, y: box.y1 - padding},
      {x: box.x2 + padding, y: box.y2 + padding}, {x: box.x1 - padding, y: box.y2 + padding}];
  }
  function overlaps(a, b) {
    return a.x1 < b.x2 && b.x1 < a.x2 && a.y1 < b.y2 && b.y1 < a.y2;
  }
  function groups() {
    const p = GDD.projection;
    return communities.map((id, index) => {
      const members = p.nodes.filter((node) => memberships.get(node).includes(id)).sort();
      const reps = new Set(members.map((node) => p.representatives.get(node)));
      if (p.nodes.includes(id)) reps.add(p.representatives.get(id));
      return {id, index, members, reps: [...reps].sort()};
    }).filter((group) => group.reps.length);
  }
  function name(id) {
    return model.nodes[id].label || id.split(".").slice(1).join(".").replaceAll("_", " ") || id;
  }
  let legendKey = "";
  function legend(items) {
    const key = JSON.stringify(items.map((g) => [g.id, g.members]));
    if (key !== legendKey) {
      $("community-legend").replaceChildren();
      for (const group of items) {
        const button = el("button", `${group.index + 1}. ${name(group.id)} · ${group.members.length}`, "community-button");
        button.dataset.communityId = group.id;
        button.style.setProperty("--community-color", GDD.communityColor(group.id));
        button.title = `${group.id}: highlight and inspect ${group.members.length} explicit visible members`;
        button.onclick = () => GDD.select("node", group.id);
        $("community-legend").append(button);
      }
      legendKey = key;
    }
    for (const button of $("community-legend").children)
      button.setAttribute("aria-pressed", GDD.state.selected?.type === "node" && GDD.state.selected.id === button.dataset.communityId);
    $("community-section").hidden = !items.length;
  }
  GDD.drawCommunities = () => {
    const items = groups(), layer = $("community-layer");
    legend(items);
    layer.replaceChildren();
    layer.style.display = $("show-communities").checked ? "" : "none";
    const pan = cy.pan(), zoom = cy.zoom();
    const shapes = svg("g", {transform: `translate(${pan.x},${pan.y}) scale(${zoom})`});
    const labels = svg("g");
    layer.append(shapes, labels);
    const occupied = cy.nodes().map((node) => node.renderedBoundingBox({includeLabels: true, includeOverlays: false}));
    const view = $("graph").getBoundingClientRect();
    const selected = GDD.state.selected;
    for (const group of items) {
      const boxes = group.reps.map((id) => nodeElement(id).boundingBox({includeLabels: true, includeOverlays: false}));
      const points = hull(boxes.flatMap((box) => corners(box, 25)));
      if (!points.length) continue;
      const color = GDD.communityColor(group.id);
      const path = svg("path", {d: points.map((p, i) => `${i ? 'L' : 'M'} ${p.x} ${p.y}`).join(" ") + " Z",
        class: "community-extent", fill: color, stroke: color,
        "data-community-id": group.id, "data-members": JSON.stringify(group.members),
        "data-representatives": JSON.stringify(group.reps)});
      path.append(svg("title", {}, `${name(group.id)}: ${group.members.length} explicit visible members. Extent is not membership.`));
      if (selected?.type === "node" && model.nodes[selected.id]?.kind === "community")
        path.classList.toggle("community-dim", selected.id !== group.id);
      shapes.append(path);
      const text = `${group.index + 1}. ${name(group.id)}`;
      const width = Math.min(340, text.length * 8 + 20);
      const minX = Math.min(...points.map((p) => p.x)) * zoom + pan.x;
      const maxX = Math.max(...points.map((p) => p.x)) * zoom + pan.x;
      const minY = Math.min(...points.map((p) => p.y)) * zoom + pan.y;
      const maxY = Math.max(...points.map((p) => p.y)) * zoom + pan.y;
      let heading;
      headingSearch: for (let row = 0; row < 12; row++)
        for (const candidate of [minY - 10 - row * 26, maxY + 25 + row * 26])
          for (const candidateX of [minX + width / 2, (minX + maxX) / 2, maxX - width / 2]) {
            const x = Math.max(width / 2 + 12, Math.min(view.width - width / 2 - 12, candidateX));
            const y = Math.max(78, Math.min(view.height - 62, candidate));
            const box = {x1: x - width / 2, x2: x + width / 2, y1: y - 18, y2: y + 4};
            if (!occupied.some((other) => overlaps(box, other))) { heading = {x, y, box}; break headingSearch; }
          }
      heading ||= {x: minX + width / 2, y: minY - 10,
        box: {x1: minX, x2: minX + width, y1: minY - 30, y2: minY - 6}};
      occupied.push(heading.box);
      labels.append(svg("text", {x: heading.x, y: heading.y, class: "community-heading", fill: color,
        "data-community-id": group.id}, text));
    }
  };
  $("show-communities").onchange = GDD.drawCommunities;
})();
