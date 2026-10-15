/* Real Chromium tests. Install Playwright separately or use its bundled runtime. */
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const { existsSync } = require("node:fs");
const os = require("node:os");
const { execFileSync } = require("node:child_process");
const path = require("node:path");
const { pathToFileURL } = require("node:url");
const { chromium } = require(process.env.GDD_PLAYWRIGHT_MODULE || "playwright");
const root = path.resolve(__dirname, "..");
const folder = process.env.GDD_BROWSER_OUTPUT
  ? path.resolve(process.env.GDD_BROWSER_OUTPUT)
  : path.join(root, "evidence/visualization");
const results = [],
  errors = [],
  requests = [],
  localAssetRequests = [],
  consoleErrors = [];

(async () => {
  const temporary = await fs.mkdtemp(path.join(os.tmpdir(), "gdd-browser-"));
  const fixtures = path.join(temporary, "fixtures");
  const localPython = path.join(root, ".venv/bin/python");
  execFileSync(
    process.env.GDD_PYTHON ||
      (existsSync(localPython) ? localPython : "python3"),
    [path.join(root, "conformance/prepare_visualization.py"), fixtures],
  );
  await fs.mkdir(folder, { recursive: true });
  const browser = await chromium.launch({
    headless: true,
    ...(process.env.GDD_BROWSER_EXECUTABLE
      ? { executablePath: process.env.GDD_BROWSER_EXECUTABLE }
      : {}),
  });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
    acceptDownloads: true,
  });
  await context.setOffline(true);
  const page = await context.newPage();
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("request", (request) => {
    if (!request.url().startsWith("file:")) requests.push(request.url());
    else if (request.resourceType() !== "document") localAssetRequests.push(request.url());
  });
  const waitForGraph = () => page.waitForFunction(() =>
    typeof GDD !== "undefined" && GDD.cy && !GDD.cy.destroyed() &&
    GDD.cy.nodes().every((node) => {
      const {x, y} = node.position();
      return Number.isFinite(x) && Number.isFinite(y);
    }),
  );
  const openFile = async (file) => {
    await page.goto(pathToFileURL(file).href);
    await waitForGraph();
  };
  const navigate = (name) => openFile(path.join(fixtures, name + ".html"));
  const manifest = () =>
    page
      .locator("#gdd-view-manifest")
      .evaluate((item) => JSON.parse(item.textContent));
  const selectNode = (id) => page.locator(`[data-list-node="${id}"]`).click();
  const engineNodes = () => page.evaluate(() => GDD.cy.nodes().map((node) => ({
    id: node.data("canonicalId"),
    ...node.position(),
    box: node.renderedBoundingBox({includeLabels: true, includeOverlays: false}),
  })));
  const camera = () => page.evaluate(() => ({zoom: GDD.cy.zoom(), pan: GDD.cy.pan()}));
  const canvasPoint = async (kind, id) => {
    await page.locator("#graph").scrollIntoViewIfNeeded();
    return page.evaluate(({kind, id}) => {
      const item = kind === "node" ? GDD.nodeElement(id) : GDD.bundleElement(id);
      if (item.empty()) throw new Error(`Missing ${kind}: ${id}`);
      const point = kind === "node" ? item.renderedPosition() : item.renderedMidpoint();
      const rect = document.getElementById("graph").getBoundingClientRect();
      return {x: rect.left + point.x, y: rect.top + point.y};
    }, {kind, id});
  };
  const clickNode = async (id) => {
    const point = await canvasPoint("node", id);
    await page.mouse.click(point.x, point.y);
  };
  const bundleKey = (from, to) => page.evaluate(({from, to}) => {
    const bundle = GDD.projection.bundles.find((item) => item.from === from && item.to === to);
    if (!bundle) throw new Error(`Missing bundle: ${from} → ${to}`);
    return bundle.key;
  }, {from, to});
  const assertProjectionIdentity = async () => {
    const m = await manifest();
    const engine = await page.evaluate(() => ({
      nodes: GDD.cy.nodes().map((item) => item.data("canonicalId")).sort(),
      bundles: GDD.cy.edges().map((item) => item.data("bundleKey")).sort(),
      expectedBundles: GDD.projection.bundles.map((item) => item.key).sort(),
      model: GDD.model,
      embedded: JSON.parse(document.getElementById("gdd-data").textContent).model,
    }));
    assert.deepEqual(engine.model, engine.embedded, "Rendering mutated the canonical model");
    assert.deepEqual(engine.nodes, m.glyphs.map((item) => item.id).sort());
    assert.deepEqual(engine.bundles, engine.expectedBundles);
    const represented = m.glyphs.flatMap((item) => item.node_ids).sort();
    assert.deepEqual(represented, m.shown_nodes);
    assert.equal(new Set(represented).size, represented.length);
    const relations = m.bundles.flatMap((item) => item.edge_ids).concat(m.folded_internal_edges).sort();
    assert.deepEqual(relations, m.shown_edges);
    assert.equal(new Set(relations).size, relations.length);
  };
  const communityGeometry = () => page.evaluate(() => {
    const rect = document.getElementById("graph").getBoundingClientRect();
    return [...document.querySelectorAll("#community-layer path[data-community-id]")].map((path) => {
      const members = JSON.parse(path.dataset.members);
      const inverse = path.getScreenCTM().inverse();
      const outside = members.filter((id) => {
        const representative = GDD.projection.representatives.get(id);
        const node = GDD.nodeElement(representative);
        if (node.empty()) return true;
        const position = node.renderedPosition();
        const local = new DOMPoint(rect.left + position.x, rect.top + position.y).matrixTransform(inverse);
        return !path.isPointInFill(local);
      });
      const box = path.getBBox();
      const id = path.dataset.communityId;
      const expected = [...new Set(GDD.model.edges.filter((edge) =>
        edge.relation === "member_of" && edge.to === id && GDD.projection.nodes.includes(edge.from),
      ).map((edge) => edge.from))].sort();
      return {id, members, expected, outside, d: path.getAttribute("d"), width: box.width, height: box.height};
    });
  });
  const assertCommunityHeadings = async () => {
    const headings = await page.locator("#community-layer text[data-community-id]").evaluateAll((items) =>
      items.map((item) => ({id: item.dataset.communityId, ...item.getBoundingClientRect().toJSON()})),
    );
    for (const heading of headings)
      assert.ok(heading.height >= 12 && heading.width > 0,
        `Community heading shrank below readable size: ${heading.id}`);
    for (let i = 0; i < headings.length; i++)
      for (let j = i + 1; j < headings.length; j++) {
        const a = headings[i], b = headings[j];
        assert.ok(a.right <= b.left + 0.5 || b.right <= a.left + 0.5 ||
          a.bottom <= b.top + 0.5 || b.bottom <= a.top + 0.5,
          "Community headings overlap");
      }
  };
  async function test(id, action) {
    try {
      await action();
      results.push({ id, result: "PASS" });
    } catch (error) {
      results.push({ id, result: "FAIL", reason: error.message });
    }
  }
  await test("offline-portable-file", async () => {
    const portableDirectory = path.join(temporary, "portable");
    await fs.mkdir(portableDirectory);
    const portable = path.join(portableDirectory, "graph.html");
    await fs.copyFile(path.join(fixtures, "fixture.html"), portable);
    await openFile(portable);
    assert.equal(await page.locator("html").getAttribute("lang"), "en");
    assert.equal(await page.title(), "Viewer fixture · GDD");
    assert.equal((await manifest()).shown_nodes.length, 6);
    assert.equal((await engineNodes()).length, 6);
    assert.equal(await page.locator("script[src]").count(), 0);
    assert.equal(await page.locator("link[rel=stylesheet][href]").count(), 0);
  });
  await test("hostile-text-and-canvas-selection", async () => {
    await clickNode("constructor");
    const behavior = await page
      .locator("#detail details[open] pre")
      .first()
      .textContent();
    const expected = await page
      .locator("#gdd-data")
      .evaluate(
        (item) => JSON.parse(item.textContent).model.nodes.constructor.behavior,
      );
    assert.equal(behavior, expected);
    assert.equal(await page.evaluate(() => window.__gddPwned), undefined);
    assert.equal(await page.locator("img").count(), 0);
    assert.equal(await page.locator("script[src]").count(), 0);
  });
  await test("keyboard-inspection", async () => {
    await page.locator("#overview").click();
    await page.locator('[data-list-node="constructor"]').focus();
    await page.keyboard.press("Enter");
    assert.equal(
      await page.locator("#detail .canonical-id").textContent(),
      "constructor",
    );
    assert.equal(
      await page.locator('.object-row[aria-pressed="true"]').count(),
      1,
    );
  });
  await test("parallel-and-opposite-directed-edges", async () => {
    const m = await manifest();
    const forward = m.bundles.find(
      (b) => b.from === "constructor" && b.to === "action.second",
    );
    assert.deepEqual(forward.edge_ids, [
      "edge.call_one",
      "edge.call_two",
      "edge.depends",
    ]);
    assert.deepEqual(
      m.bundles.find(
        (b) => b.from === "action.second" && b.to === "constructor",
      ).edge_ids,
      ["edge.reverse"],
    );
    const key = await bundleKey("constructor", "action.second");
    const point = await canvasPoint("edge", key);
    await page.mouse.click(point.x, point.y);
    const text = await page.locator("#detail").innerText();
    assert.ok(text.includes("DIRECTED RELATION BUNDLE"));
    forward.edge_ids.forEach((id) => assert.ok(text.includes(id)));
    assert.notEqual(await page.evaluate((id) => GDD.bundleElement(id).style("target-arrow-shape"), key), "none");
  });
  await test("self-loop-is-inspectable", async () => {
    const key = await bundleKey("constructor", "constructor");
    assert.equal(await page.evaluate((id) => GDD.bundleElement(id).isLoop(), key), true);
    await page.locator("#tab-edges").click();
    await page.locator('[data-list-edge="edge.loop"]').click();
    assert.ok(
      (await page.locator("#detail").innerText()).includes("constructor →"),
    );
  });
  await test("numeric-contract-precision", async () => {
    await page.locator("#tab-nodes").click();
    await page.locator("#search").fill("9007199254740993");
    assert.deepEqual((await manifest()).shown_nodes, ["entity.precision"]);
    await page.locator("#search").fill("");
    await selectNode("entity.precision");
    const text = await page.locator("#detail").innerText();
    assert.ok(text.includes("9007199254740993"));
    assert.ok(!text.includes("9007199254740992"));
  });
  await test("scenario-reveal-as-first-action", async () => {
    await navigate("fixture");
    await page.locator("#scenarios").check();
    assert.equal((await engineNodes()).length, 7);
    assert.equal((await manifest()).shown_nodes.length, 7);
  });
  await test("kind-search-and-empty-filter", async () => {
    await navigate("fixture");
    await page.locator("#kind").selectOption("scenario");
    assert.deepEqual((await manifest()).shown_nodes, ["scenario.hidden"]);
    await page.locator("#kind").selectOption("");
    await page.locator("#search").fill("Toggle");
    assert.deepEqual((await manifest()).shown_nodes, ["scenario.hidden"]);
    await page.locator("#search").fill("no-match-at-all");
    assert.equal((await manifest()).shown_nodes.length, 0);
    assert.equal(await page.locator("#empty").isVisible(), true);
  });
  await test("relation-and-community-filters", async () => {
    await page.locator("#reset").click();
    await page.locator("#relation").selectOption("calls");
    assert.deepEqual((await manifest()).shown_edges, [
      "edge.call_one",
      "edge.call_two",
      "edge.loop",
      "edge.reverse",
    ]);
    await page.locator("#relation").selectOption("");
    await page.locator("#community").selectOption("community.frontend");
    assert.deepEqual((await manifest()).shown_nodes, [
      "action.second",
      "community.frontend",
      "constructor",
    ]);
  });
  await test("overlapping-community-folding-is-lossless", async () => {
    await page.locator("#reset").click();
    await page.locator("#fold").check();
    assert.match(await page.evaluate(() => GDD.nodeElement("community.backend").data("kindLabel")), /folded community/i);
    const m = await manifest();
    assert.equal(m.shown_nodes.length, 7);
    assert.equal(m.shown_edges.length, 9);
    const represented = m.glyphs.flatMap((g) => g.node_ids);
    assert.equal(new Set(represented).size, 7);
    assert.equal(represented.length, 7);
    const edges = m.bundles
      .flatMap((b) => b.edge_ids)
      .concat(m.folded_internal_edges);
    assert.deepEqual([...edges].sort(), m.shown_edges);
    assert.equal(new Set(edges).size, 9);
    await selectNode("constructor");
    const text = await page.locator("#detail").innerText();
    assert.ok(text.includes("community.backend"));
    assert.ok(text.includes("community.frontend"));
    await selectNode("community.backend");
    assert.ok(
      (await page.locator("#detail").innerText()).includes("edge.loop"),
    );
  });
  await test("zoom-pan-drag-and-reset", async () => {
    await page.locator("#reset").click();
    await page.locator("#overview").click();
    assert.equal(
      await page.locator('.object-row[aria-pressed="true"]').count(),
      0,
    );
    const before = await camera();
    await page.locator("#zoom-in").click();
    assert.notDeepEqual(await camera(), before);
    const start = await page.evaluate(() => GDD.nodeElement("constructor").position());
    const point = await canvasPoint("node", "constructor");
    await page.mouse.move(point.x, point.y);
    await page.mouse.down();
    await page.mouse.move(point.x + 20, point.y, {
      steps: 20,
    });
    await page.mouse.up();
    assert.notDeepEqual(await page.evaluate(() => GDD.nodeElement("constructor").position()), start);
    assert.equal(
      await page.locator("#detail .detail-tag").textContent(),
      "Model overview",
    );
    const beforePan = await camera(), graph = await page.locator("#graph").boundingBox();
    await page.mouse.move(graph.x + 15, graph.y + 25);
    await page.mouse.down();
    await page.mouse.move(graph.x + 40, graph.y + 55, {steps: 10});
    await page.mouse.up();
    assert.notDeepEqual(await camera(), beforePan);
    for (const area of await communityGeometry())
      assert.deepEqual(area.outside, [], `Community area lost alignment after camera movement: ${area.id}`);
    await page.locator("#fit").click();
    assert.equal((await manifest()).shown_nodes.length, 7);
  });
  await test("view-manifest-download", async () => {
    const pending = page.waitForEvent("download");
    await page.locator("#manifest-download").click();
    const download = await pending;
    const saved = JSON.parse(await fs.readFile(await download.path(), "utf8"));
    assert.deepEqual(saved, await manifest());
    await fs.writeFile(
      path.join(folder, "browser.view-state.json"),
      JSON.stringify(saved, null, 2) + "\n",
    );
  });
  await test("parallel-membership-is-not-overlap", async () => {
    await navigate("duplicate");
    await page.locator("#reset").click();
    await page.locator("#fold").check();
    assert.equal((await manifest()).shown_edges.length, 10);
    await selectNode("community.frontend");
    assert.equal(
      await page
        .getByRole("button", { name: "action.second · overlap", exact: true })
        .count(),
      0,
    );
    assert.equal(
      await page
        .getByRole("button", { name: "action.second", exact: true })
        .count(),
      1,
    );
  });
  await test("scoped-artifact-cannot-widen-on-reset", async () => {
    await navigate("scoped");
    await page.locator("#reset").click();
    const m = await manifest();
    assert.equal(m.shown_nodes.length, 34);
    assert.equal(m.shown_edges.length, 73);
    assert.equal(m.omitted_nodes.length, 4);
  });
  await test("extension-and-empty-model", async () => {
    await navigate("extension");
    await selectNode("sensor.room");
    assert.ok(
      (await page.locator("#detail").innerText()).includes(
        "EXAMPLE.SENSORS.SENSOR",
      ),
    );
    await navigate("empty");
    assert.equal((await manifest()).shown_nodes.length, 0);
    assert.equal(await page.locator("#empty").isVisible(), true);
  });
  await test("responsive-dark-reduced-motion", async () => {
    await navigate("concert");
    await page.setViewportSize({ width: 1440, height: 1000 });
    await selectNode("action.reserve");
    await page.screenshot({ path: path.join(folder, "concert-desktop.png") });
    await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
    assert.equal(
      await page.evaluate(
        () => getComputedStyle(document.body).backgroundColor,
      ),
      "rgb(29, 37, 36)",
    );
    await page.screenshot({ path: path.join(folder, "concert-dark.png") });
    await page.emulateMedia({ colorScheme: "light" });
    await page.setViewportSize({ width: 390, height: 844 });
    assert.ok(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    );
    await page.locator("#search").fill("action.reserve");
    await selectNode("action.reserve");
    assert.ok(
      (await page.locator("#detail").innerText()).includes("Pseudocode"),
    );
    await page.screenshot({
      path: path.join(folder, "concert-mobile.png"),
      fullPage: true,
    });
  });
  await test("exact-source-and-design-reference-selectors", async () => {
    await page.setViewportSize({ width: 1440, height: 1000 });
    await navigate("references");
    const referenceText = async () => {
      const details = page.locator("#detail details").filter({
        has: page.locator("summary", { hasText: /^Reference$/ }),
      });
      await details.locator("summary").click();
      return details.locator("pre").textContent();
    };
    const assertExact = (text) => {
      assert.ok(text.includes("9007199254740993"));
      assert.ok(!text.includes("9007199254740992"));
    };
    await selectNode("constructor");
    assertExact(await referenceText());
    await selectNode("screen.references");
    assertExact(await referenceText());
    await page.locator("#tab-edges").click();
    await page.locator('[data-list-edge="edge.call_one"]').click();
    assertExact(await referenceText());
    await page
      .getByRole("button", { name: "→ action.second", exact: true })
      .click();
    assert.equal(
      await page.locator("#detail .canonical-id").textContent(),
      "action.second",
    );
  });
  await test("new-selections-reset-inspector-scroll", async () => {
    await navigate("concert");
    await selectNode("action.reserve");
    await page.locator("#inspector").evaluate((item) => {
      item.scrollTop = item.scrollHeight;
    });
    assert.ok(
      await page.locator("#inspector").evaluate((item) => item.scrollTop > 0),
    );
    await selectNode("action.cancel");
    assert.equal(
      await page.locator("#inspector").evaluate((item) => item.scrollTop),
      0,
    );
    await page.locator("#inspector").evaluate((item) => {
      item.scrollTop = item.scrollHeight;
    });
    await page.locator("#tab-edges").click();
    await page.locator("[data-list-edge]").first().click();
    assert.equal(
      await page.locator("#inspector").evaluate((item) => item.scrollTop),
      0,
    );
    await page.locator("#detail .detail-link").first().click();
    assert.equal(
      await page.locator("#inspector").evaluate((item) => item.scrollTop),
      0,
    );
  });
  await test("mobile-selection-reveals-details-without-filter-jumps", async () => {
    await page.setViewportSize({ width: 390, height: 844 });
    await navigate("fixture");
    await clickNode("constructor");
    assert.equal(
      await page.locator("#detail .canonical-id").textContent(),
      "constructor",
    );
    const visibleHeading = await page
      .locator("#detail .detail-title")
      .boundingBox();
    assert.ok(visibleHeading.y >= 0 && visibleHeading.y < 844);
    assert.ok(await page.evaluate(() => window.scrollY > 0));
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.locator("#search").fill("action.second");
    assert.equal(
      await page.locator("#detail .detail-tag").textContent(),
      "Model overview",
    );
    const inspector = await page.locator("#inspector").boundingBox();
    assert.ok(inspector.y >= 844); // Filtering must not jump to details.
    await selectNode("action.second");
    const selectedHeading = await page
      .locator("#detail .detail-title")
      .boundingBox();
    assert.ok(selectedHeading.y >= 0 && selectedHeading.y < 844);
    await page.setViewportSize({ width: 1440, height: 1000 });
  });
  await test("responsive-panels-and-controls-do-not-overlap", async () => {
    for (const size of [
      { width: 740, height: 360 },
      { width: 850, height: 600 },
      { width: 800, height: 1024 },
      { width: 851, height: 600 },
      { width: 1151, height: 600 },
      { width: 1440, height: 600 },
      { width: 1440, height: 1000 },
      { width: 390, height: 844 },
    ]) {
      await page.setViewportSize(size);
      await navigate("concert");
      const checkPanels = async () => {
        const controlSelectors = [
          "#search", "#kind", "#relation", "#community", "#scenarios", "#fold",
          ".list-tabs", "#object-list", "#reset", "#show-communities",
        ];
        const layout = await page.evaluate(() => {
          const rect = (selector) => {
            const { left, right, top, bottom, height } = document
              .querySelector(selector)
              .getBoundingClientRect();
            return { left, right, top, bottom, height };
          };
          return {
            panels: [".stage", ".explorer", "#inspector"].map(rect),
            header: rect("header"),
            main: rect("main"),
            footer: rect("footer"),
            pageWidth: document.documentElement.scrollWidth,
          };
        });
        const { panels, header, main, footer, pageWidth } = layout;
        const atSize = `${size.width}x${size.height}`;
        assert.ok(pageWidth <= size.width, `Horizontal overflow at ${atSize}`);
        assert.ok(
          header.bottom <= main.top + 0.5,
          `Header overlaps main at ${atSize}`,
        );
        assert.ok(
          main.bottom <= footer.top + 0.5,
          `Main overlaps footer at ${atSize}`,
        );
        for (const panel of panels)
          assert.ok(
            panel.top >= main.top - 0.5 && panel.bottom <= main.bottom + 0.5,
            `Panel outside main at ${atSize}`,
          );
        for (let i = 0; i < panels.length; i++)
          for (let j = i + 1; j < panels.length; j++) {
            const a = panels[i],
              b = panels[j];
            assert.ok(
              a.right <= b.left + 0.5 ||
                b.right <= a.left + 0.5 ||
                a.bottom <= b.top + 0.5 ||
                b.bottom <= a.top + 0.5,
              `Overlapping panels at ${size.width}x${size.height}`,
            );
          }
        for (const selector of controlSelectors) {
          // Short screens may scroll the explorer; every control must be reachable.
          await page.locator(selector).scrollIntoViewIfNeeded();
          const {control, explorer} = await page.evaluate((selector) => {
            const rect = (item) => {
              const {top, bottom} = item.getBoundingClientRect();
              return {top, bottom};
            };
            return {control: rect(document.querySelector(selector)), explorer: rect(document.querySelector(".explorer"))};
          }, selector);
          assert.ok(
            control.top >= explorer.top - 0.5 &&
              control.bottom <= explorer.bottom + 0.5,
            `Unreachable explorer control ${selector} at ${atSize}`,
          );
        }
        if (size.width === 800)
          assert.ok(panels[0].height <= size.height * 0.65 + 1);
      };
      await checkPanels();
      await page.locator("#scenarios").check();
      assert.equal((await manifest()).shown_nodes.length, 38);
      await page.locator("#fold").check();
      await page.locator("#reset").click();
      assert.equal(await page.locator("#fold").isChecked(), false);
      assert.equal((await manifest()).shown_nodes.length, 38);
      await checkPanels();
    }
    await page.setViewportSize({ width: 1440, height: 1000 });
  });
  await test("desktop-fills-viewport-with-scrolling-panels", async () => {
    await page.setViewportSize({ width: 1440, height: 1000 });
    await navigate("concert");
    const layout = await page.evaluate(() => {
      const main = document.querySelector("main").getBoundingClientRect();
      const header = document.querySelector("header").getBoundingClientRect();
      const footer = document.querySelector("footer").getBoundingClientRect();
      const stage = document.querySelector(".stage").getBoundingClientRect();
      const graph = document.querySelector("#graph").getBoundingClientRect();
      const panels = ["#object-list", "#inspector"].map((selector) => {
        const item = document.querySelector(selector);
        item.scrollTop = item.scrollHeight;
        const result = {
          scrollTop: item.scrollTop,
          clientHeight: item.clientHeight,
          scrollHeight: item.scrollHeight,
        };
        item.scrollTop = 0;
        return result;
      });
      return {
        mainTop: main.top,
        mainBottom: main.bottom,
        headerBottom: header.bottom,
        footerTop: footer.top,
        footerBottom: footer.bottom,
        stageHeight: stage.height,
        graphHeight: graph.height,
        pageHeight: document.documentElement.scrollHeight,
        panels,
      };
    });
    assert.ok(Math.abs(layout.mainTop - layout.headerBottom) <= 0.5);
    assert.ok(Math.abs(layout.mainBottom - layout.footerTop) <= 0.5);
    assert.ok(Math.abs(layout.footerBottom - 1000) <= 0.5);
    assert.equal(layout.pageHeight, 1000);
    assert.ok(Math.abs(layout.graphHeight - layout.stageHeight) <= 0.5);
    for (const panel of layout.panels) {
      assert.ok(panel.scrollHeight > panel.clientHeight);
      assert.ok(panel.scrollTop > 0);
    }
  });
  await test("long-project-title-expands-header-without-overlap", async () => {
    for (const size of [
      { width: 740, height: 360 },
      { width: 850, height: 600 },
      { width: 851, height: 600 },
      { width: 1151, height: 600 },
      { width: 1440, height: 600 },
      { width: 1440, height: 1000 },
      { width: 390, height: 844 },
    ]) {
      await page.setViewportSize(size);
      await navigate("long-title");
      assert.equal(
        await page.locator("#project-title").textContent(),
        "A".repeat(500),
      );
      const layout = await page.evaluate(() => {
        const rect = (selector) => {
          const { left, right, top, bottom, height } = document
            .querySelector(selector)
            .getBoundingClientRect();
          return { left, right, top, bottom, height };
        };
        return {
          header: rect("header"),
          children: [".brand", ".heading", "#project-title", "#overview"].map(
            rect,
          ),
          main: rect("main"),
          footer: rect("footer"),
          pageWidth: document.documentElement.scrollWidth,
        };
      });
      const atSize = `${size.width}x${size.height}`;
      assert.ok(
        layout.header.height > 88,
        `Header failed to expand at ${atSize}`,
      );
      assert.ok(layout.header.top >= 0, `Header above viewport at ${atSize}`);
      assert.ok(
        layout.header.bottom <= layout.main.top + 0.5,
        `Header overlaps main at ${atSize}`,
      );
      assert.ok(
        layout.main.bottom <= layout.footer.top + 0.5,
        `Main overlaps footer at ${atSize}`,
      );
      assert.ok(
        layout.pageWidth <= size.width,
        `Horizontal overflow at ${atSize}`,
      );
      for (const child of layout.children)
        assert.ok(
          child.top >= layout.header.top &&
            child.bottom <= layout.header.bottom &&
            child.left >= layout.header.left &&
            child.right <= layout.header.right,
          `Header content clipped at ${atSize}`,
        );
    }
    await page.setViewportSize({ width: 1440, height: 1000 });
  });
  await test("component-boundary-rationale-and-contract-navigation", async () => {
    await navigate("concert-0.3");
    await selectNode("component.reservations");
    const section = page.locator(".detail-section").filter({
      has: page.getByRole("heading", { name: "Component responsibility and boundary rationale", exact: true }),
    });
    assert.match(await section.textContent(), /Keep availability, atomicity and cancellation rules consistent/);
    const contracts = page.locator(".detail-section").filter({
      has: page.getByRole("heading", { name: "Authoritative public action contracts", exact: true }),
    });
    await contracts.getByRole("button", { name: "action.reserve", exact: true }).click();
    assert.match(await page.locator("#detail").textContent(), /postconditions/);
    await page.screenshot({ path: path.join(folder, "gdd-0.3-contract.png") });
  });
  await test("overlapping-community-public-boundary", async () => {
    await selectNode("community.reservation_feature");
    const section = page.locator(".detail-section").filter({
      has: page.getByRole("heading", { name: "Derived public boundary", exact: true }),
    });
    const text = await section.textContent();
    assert.match(text, /Membership grants no permissions/);
    assert.match(text, /component.reservations/);
    assert.match(text, /edge.screen.reserve.calls/);
    assert.match(text, /internal/);
    await page.screenshot({ path: path.join(folder, "gdd-0.3-community.png") });
  });
  for (const fixture of ["clusters", "ownership"]) {
    await test(`${fixture}-stay-separated-with-readable-labels`, async () => {
      await page.setViewportSize({width: 1600, height: 1000});
      await navigate(fixture);
      const nodes = await engineNodes();
      assert.equal(nodes.length, fixture === "clusters" ? 30 : 27);
      assert.ok(nodes.every((node) => Number.isFinite(node.x) && Number.isFinite(node.y)));
      const groups = [0, 1, 2].map((group) => {
        const members = nodes.filter((node) => node.id.startsWith(`action.group${group}.`));
        const x = members.reduce((sum, node) => sum + node.x, 0) / members.length;
        const y = members.reduce((sum, node) => sum + node.y, 0) / members.length;
        const radius = Math.sqrt(members.reduce((sum, node) => sum + (node.x - x) ** 2 + (node.y - y) ** 2, 0) / members.length);
        return {x, y, radius};
      });
      for (let i = 0; i < groups.length; i++)
        for (let j = i + 1; j < groups.length; j++)
          assert.ok(Math.hypot(groups[i].x - groups[j].x, groups[i].y - groups[j].y) > groups[i].radius + groups[j].radius,
            "Community/component clusters collapsed into one mass");
      for (let i = 0; i < nodes.length; i++)
        for (let j = i + 1; j < nodes.length; j++)
          {
            const a = nodes[i].box, b = nodes[j].box;
            assert.ok(Math.min(a.x2, b.x2) - Math.max(a.x1, b.x1) <= 0.5 || Math.min(a.y2, b.y2) - Math.max(a.y1, b.y1) <= 0.5,
              `Visible node/label collision: ${nodes[i].id}, ${nodes[j].id}`);
          }
      if (fixture === "clusters") await assertCommunityHeadings();
      await page.screenshot({path: path.join(folder, `${fixture}-layout.png`)});
      const before = await manifest();
      await page.reload();
      await waitForGraph();
      assert.deepEqual((await engineNodes()).map(({id, x, y}) => ({id, x, y})), nodes.map(({id, x, y}) => ({id, x, y})));
      assert.deepEqual(await manifest(), before);
      await page.locator("#search").fill("Group 0");
      const filtered = await engineNodes();
      assert.ok(filtered.length < nodes.length);
      assert.ok(filtered.every((node) => nodes.some((original) => original.id === node.id && original.x === node.x && original.y === node.y)),
        "Filtering unexpectedly moved retained nodes");
    });
  }
  await test("community-areas-labels-and-shared-node-identity", async () => {
    await navigate("overlap");
    const before = await manifest();
    await assertProjectionIdentity();
    const areas = await communityGeometry();
    assert.deepEqual(areas.map((area) => area.id).sort(), ["community.group0", "community.group1", "community.group2"]);
    for (const area of areas) {
      assert.ok(area.width > 0 && area.height > 0, `Empty community area: ${area.id}`);
      assert.deepEqual(area.outside, [], `Community area misses members: ${area.id}`);
      assert.deepEqual(area.members, area.expected, `Community area changes explicit membership: ${area.id}`);
      assert.ok(area.members.includes("action.group0.0"), "Shared action must belong to all three areas");
      assert.equal(new Set(area.members).size, area.members.length);
      const label = page.locator("#community-layer text").filter({hasText: `Group ${area.id.at(-1)}`});
      assert.equal(await label.count(), 1);
      assert.equal(await label.isVisible(), true);
    }
    await assertCommunityHeadings();
    assert.equal((await engineNodes()).filter((node) => node.id === "action.group0.0").length, 1);
    assert.equal(before.shown_nodes.length, 30);
    assert.equal(before.shown_edges.length, 141);
    assert.match(await page.locator("#community-section").textContent(), /extent|membership/i);
    await page.screenshot({path: path.join(folder, "overlapping-communities.png")});
    await page.locator("#show-communities").uncheck();
    assert.equal(await page.locator("#community-layer path[data-community-id]:visible").count(), 0);
    assert.deepEqual(await manifest(), before, "Hiding visual areas changed graph semantics");
    await page.locator("#show-communities").check();
    assert.equal(await page.locator("#community-layer path[data-community-id]:visible").count(), 3);
    await page.locator('#community-legend [data-community-id="community.group1"]').click();
    assert.equal(await page.locator("#detail .canonical-id").textContent(), "community.group1");
    await assertProjectionIdentity();
  });
  await test("community-areas-follow-real-node-dragging", async () => {
    await navigate("overlap");
    const before = await communityGeometry();
    const point = await canvasPoint("node", "action.group0.0");
    const original = await page.evaluate(() => GDD.nodeElement("action.group0.0").position());
    await page.mouse.move(point.x, point.y);
    await page.mouse.down();
    await page.mouse.move(point.x + 65, point.y + 35, {steps: 20});
    await page.mouse.up();
    const moved = await page.evaluate(() => GDD.nodeElement("action.group0.0").position());
    assert.notDeepEqual(moved, original, "Canvas node did not move");
    await page.waitForFunction((old) => [...document.querySelectorAll("#community-layer path[data-community-id]")].some((path) => old.find((area) => area.id === path.dataset.communityId)?.d !== path.getAttribute("d")), before);
    const after = await communityGeometry();
    for (const area of after) {
      assert.deepEqual(area.members, before.find((old) => old.id === area.id).members);
      assert.deepEqual(area.members, area.expected);
      assert.deepEqual(area.outside, [], `Dragged member escaped its community area: ${area.id}`);
    }
    await assertProjectionIdentity();
  });
  await test("community-areas-preserve-folded-and-filtered-memberships", async () => {
    await navigate("overlap");
    const initial = await manifest();
    await page.locator("#fold").check();
    await assertProjectionIdentity();
    assert.deepEqual((await manifest()).shown_nodes, initial.shown_nodes);
    assert.deepEqual((await manifest()).shown_edges, initial.shown_edges);
    for (const area of await communityGeometry()) {
      assert.ok(area.members.includes("action.group0.0"));
      assert.deepEqual(area.members, area.expected);
      assert.deepEqual(area.outside, [], `Folded area misses represented members: ${area.id}`);
    }
    await page.locator("#community").selectOption("community.group1");
    await assertProjectionIdentity();
    const filtered = await manifest();
    assert.ok(filtered.shown_nodes.length < initial.shown_nodes.length);
    assert.ok(filtered.shown_nodes.includes("action.group0.0"));
    for (const area of await communityGeometry())
      assert.ok(area.members.every((id) => filtered.shown_nodes.includes(id)));
    await page.locator("#reset").click();
    await assertProjectionIdentity();
    assert.deepEqual((await manifest()).shown_nodes, initial.shown_nodes);
    assert.deepEqual((await manifest()).shown_edges, initial.shown_edges);
  });
  await test("bounded-300-node-layout-and-canvas-interactions", async () => {
    await page.setViewportSize({width: 1600, height: 1000});
    await navigate("large");
    const nodes = await engineNodes();
    assert.equal(nodes.length, 300);
    assert.equal(new Set(nodes.map((node) => node.id)).size, 300);
    assert.ok(nodes.every((node) => Number.isFinite(node.x) && Number.isFinite(node.y)));
    await assertProjectionIdentity();
    const initial = await manifest();
    assert.equal(initial.shown_edges.length, 1173);
    await page.locator("#search").fill("action.group2.97");
    await selectNode("action.group2.97");
    assert.equal(await page.locator("#detail .canonical-id").textContent(), "action.group2.97");
    await page.locator("#fit").click();
    await page.locator("#overview").click();
    await clickNode("action.group2.97");
    assert.equal(await page.locator("#detail .canonical-id").textContent(), "action.group2.97");
    const beforeZoom = await camera();
    await page.locator("#zoom-in").click();
    assert.notDeepEqual(await camera(), beforeZoom);
    await page.locator("#reset").click();
    await assertProjectionIdentity();
    assert.deepEqual((await manifest()).shown_nodes, initial.shown_nodes);
    assert.deepEqual((await manifest()).shown_edges, initial.shown_edges);
    await page.locator("#fit").click();
    await page.screenshot({path: path.join(folder, "bounded-300-nodes.png")});
  });
  await test("no-script-errors-or-network", async () => {
    assert.deepEqual(errors, []);
    assert.deepEqual(consoleErrors, []);
    assert.deepEqual(requests, []);
    assert.deepEqual(localAssetRequests, []);
  });
  const report = {
    format: "gdd-browser-conformance/0.1",
    browser: await browser.version(),
    offline: true,
    results,
    passed: results.filter((r) => r.result === "PASS").length,
    total: results.length,
    errors,
    consoleErrors,
    externalRequests: requests,
    localAssetRequests,
  };
  await fs.writeFile(
    path.join(folder, "browser-conformance.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
  await browser.close();
  await fs.rm(temporary, { recursive: true });
  process.exitCode = report.passed === report.total ? 0 : 1;
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
