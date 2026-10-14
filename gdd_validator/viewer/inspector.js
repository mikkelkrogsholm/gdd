/* Canonical model records are always rendered as text. */
(() => {
  const { data, model, $, state, el, label, memberships } = GDD;
  const detail = $("detail");
  function section(title) {
    const box = el("section", undefined, "detail-section");
    box.append(el("h3", title));
    detail.append(box);
    return box;
  }
  function raw(parent, title, value, open = false, exact = null) {
    const box = el("details"),
      summary = el("summary", title);
    box.open = open;
    box.append(
      summary,
      el(
        "pre",
        exact ??
          (typeof value === "string" ? value : JSON.stringify(value, null, 2)),
      ),
    );
    parent.append(box);
  }
  function button(parent, text, type, id) {
    const item = el("button", text, "detail-link");
    item.onclick = () => GDD.select(type, id);
    parent.append(item);
    return item;
  }
  function heading(tag, title, id, description) {
    detail.replaceChildren(
      el("span", tag, "detail-tag"),
      el("h2", title, "detail-title"),
    );
    if (id) detail.append(el("p", id, "canonical-id"));
    if (description) detail.append(el("p", description, "detail-description"));
  }
  function kv(parent, key, value) {
    const row = el("dl", undefined, "kv");
    row.append(el("dt", key), el("dd", String(value)));
    parent.append(row);
  }
  function location(id) {
    const box = section("Canonical location");
    const loc = data.locations[id];
    if (loc) kv(box, "File / pointer", `${loc[0]}${loc[1]}`);
    kv(box, "Model revision", data.spec_revision);
  }
  function sourceReferences(node, id) {
    const references = [
      ...(node.source_refs || []),
      ...(node.design_refs || []),
    ];
    if (!references.length) return;
    const box = section("Sources and design references");
    references.forEach((ref, index) => {
      button(box, ref.source, "source", ref.source);
      raw(box, "Reference", ref, false, data.text.references[id][index]);
    });
  }
  function edgeDetails(edge) {
    heading("Canonical relation", edge.relation, edge.id, edge.description);
    const box = section("Direction");
    button(box, `${edge.from} →`, "node", edge.from);
    button(box, `→ ${edge.to}`, "node", edge.to);
    const peers = model.edges.filter(
      (e) => e.from === edge.from && e.to === edge.to,
    );
    if (peers.length > 1) {
      const group = section(`${peers.length} parallel relations`);
      peers.forEach((e) =>
        button(group, `${e.id} · ${e.relation}`, "edge", e.id),
      );
    }
    sourceReferences(edge, edge.id);
    location(edge.id);
    raw(detail, "All relation fields", edge, true, data.text.edges[edge.id]);
  }
  function nodeDetails(id) {
    const node = model.nodes[id];
    heading(node.kind, label(id), id, node.description);
    detail.append(
      el(
        "span",
        node.maturity === "specified" ? "Specified" : "Sketch (default)",
        "badge",
      ),
    );
    detail.append(
      el(
        "p",
        "Maturity describes the model's level of detail. Readiness and implementation are assessed separately.",
        "notice",
      ),
    );
    const memberOf = memberships.get(id);
    if (memberOf.length) {
      const box = section("Member of communities");
      memberOf.forEach((c) => button(box, label(c), "node", c));
    }
    if (node.behavior !== undefined) {
      const box = section("Pseudocode / behavior");
      raw(
        box,
        "Show behavior",
        node.behavior,
        true,
        data.text.nodes[id].fields.behavior,
      );
    }
    if (node.contract) {
      const box = section("Contract");
      raw(
        box,
        "Input, output, errors and effects",
        node.contract,
        true,
        data.text.nodes[id].fields.contract,
      );
    }
    for (const key of [
      "fields",
      "rule",
      "scenario",
      "machine",
      "interaction",
      "realization",
    ])
      if (node[key] !== undefined)
        raw(
          section(key),
          `Show ${key}`,
          node[key],
          true,
          data.text.nodes[id].fields[key],
        );
    if (node.kind === "community") {
      const members = [...memberships]
        .filter(([, groups]) => groups.includes(id))
        .map(([member]) => member);
      const box = section(`${members.length} members`);
      members.forEach((member) =>
        button(
          box,
          `${label(member)}${memberships.get(member).length > 1 ? " · overlap" : ""}`,
          "node",
          member,
        ),
      );
      const internals = GDD.projection.edges.filter(
        (e) =>
          GDD.projection.internal.includes(e.id) &&
          GDD.projection.representatives.get(e.from) === id,
      );
      if (internals.length) {
        const internalBox = section(
          `${internals.length} folded internal relations`,
        );
        internals.forEach((e) =>
          button(internalBox, `${e.id} · ${e.relation}`, "edge", e.id),
        );
      }
    }
    const related = model.edges.filter((e) => e.from === id || e.to === id);
    if (related.length) {
      const box = section(`Relations · ${related.length}`);
      related.forEach((e) =>
        button(
          box,
          `${e.from === id ? "→" : "←"} ${e.relation} · ${e.from === id ? e.to : e.from}\n${e.id}`,
          "edge",
          e.id,
        ),
      );
    }
    const questions = Object.entries(model.questions || {}).filter(([, q]) =>
      q.targets.includes(id),
    );
    if (questions.length) {
      const box = section("Open questions / model data");
      questions.forEach(([qid, q]) =>
        raw(box, qid, q, true, data.text.questions[qid].full),
      );
    }
    sourceReferences(node, id);
    location(id);
    raw(detail, "All node fields", node, false, data.text.nodes[id].full);
  }
  function overview() {
    heading(
      "Model overview",
      model.project.title,
      model.project.id,
      model.project.goal,
    );
    detail.append(
      el(
        "p",
        "Explore intent, contracts and pseudocode. This view never writes back to the model.",
        "notice",
      ),
    );
    const counts = section("The graph");
    kv(counts, "Nodes", Object.keys(model.nodes).length);
    kv(counts, "Relations", model.edges.length);
    kv(
      counts,
      "Scope",
      data.scope.length ? data.scope.join(", ") : "Entire model",
    );
    const legend = section("Node types");
    for (const kind of [
      ...new Set(Object.values(model.nodes).map((n) => n.kind)),
    ].sort()) {
      const row = el("p"),
        dot = el(
          "span",
          undefined,
          `type-dot type-${GDD.kinds.includes(kind) ? kind : "component"}`,
        );
      row.append(dot, document.createTextNode(kind));
      legend.append(row);
    }
    legend.append(
      el(
        "p",
        "Dashed node: sketch or folded community. Arrows show relation direction. ×N is an inspectable relation bundle.",
      ),
    );
    const sources = Object.keys(model.sources || {});
    if (sources.length) {
      const box = section("Sources · separate from intent");
      sources.forEach((id) => button(box, id, "source", id));
    }
    const questions = Object.entries(model.questions || {});
    if (questions.length) {
      const box = section("Questions");
      questions.forEach(([id, q]) =>
        raw(box, id, q, false, data.text.questions[id].full),
      );
    }
    const revisions = section("Traceability");
    kv(revisions, "Revision", data.spec_revision);
    kv(
      revisions,
      "Generator",
      `${data.generator.name} ${data.generator.version}`,
    );
    kv(
      revisions,
      "Observations",
      "No observation inputs in this projection",
    );
    revisions.append(
      el(
        "p",
        "The view displays model data. It does not calculate readiness or implementation evidence.",
      ),
    );
    raw(detail, "Model snapshot", data.snapshot, false, data.text.snapshot);
    raw(detail, "Current view manifest", GDD.manifest());
    raw(detail, "Project fields", model.project, false, data.text.project);
  }
  GDD.select = (type, id) => {
    state.selected = type ? { type, id } : null;
    $("selection-status").textContent = type
      ? `Selected ${type}: ${id}`
      : "Model overview";
    for (const item of $("object-list").children) {
      if (item.tagName === "BUTTON")
        item.setAttribute(
          "aria-pressed",
          (type === "node" || type === "edge") &&
            item.getAttribute(
              type === "node" ? "data-list-node" : "data-list-edge",
            ) === id,
        );
    }
    if (type === "node") nodeDetails(id);
    else if (type === "edge") edgeDetails(model.edges.find((e) => e.id === id));
    else if (type === "bundle") {
      const bundle = GDD.projection.bundles.find((b) => b.key === id);
      heading(
        "Directed relation bundle",
        `${bundle.edges.length} relations`,
        `${bundle.from} → ${bundle.to}`,
      );
      detail.append(
        el(
          "p",
          "Each ID is an independent canonical relation. The bundle changes only the drawing.",
          "notice",
        ),
      );
      const box = section("All relations");
      bundle.edges.forEach((e) =>
        button(
          box,
          `${e.id}\n${e.from} → ${e.to} · ${e.relation}`,
          "edge",
          e.id,
        ),
      );
    } else if (type === "source") {
      heading("Source · separate from intent", id, id);
      detail.append(
        el(
          "p",
          "Source claims and planned references are not verified implementation evidence.",
          "notice",
        ),
      );
      raw(
        detail,
        "Source data",
        model.sources[id],
        true,
        data.text.sources[id].full,
      );
      location(id);
    } else overview();
    $("inspector").scrollTop = 0;
    if (type && window.matchMedia("(max-width: 850px)").matches)
      $("inspector").scrollIntoView({ block: "start", behavior: "instant" });
    GDD.highlight();
  };
  GDD.showOverview = () => GDD.select(null, null);
})();
