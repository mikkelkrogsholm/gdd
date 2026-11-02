/* Real offline browser coverage for the connected product lenses. */
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");
const {execFileSync} = require("node:child_process");
const {pathToFileURL} = require("node:url");
const {chromium} = require(process.env.GDD_PLAYWRIGHT_MODULE || "playwright");
const root = path.resolve(__dirname, "..");
(async () => {
  const temporary = await fs.mkdtemp(path.join(os.tmpdir(), "gdd-perspectives-browser-"));
  const fixtures = path.join(temporary, "fixtures");
  execFileSync(process.env.GDD_PYTHON || path.join(root, ".venv/bin/python"),
    [path.join(root, "conformance/prepare_perspectives.py"), fixtures]);
  const browser = await chromium.launch({headless: true,
    ...(process.env.GDD_BROWSER_EXECUTABLE ? {executablePath: process.env.GDD_BROWSER_EXECUTABLE} : {})});
  const results = [], errors = [], requests = [];
  try {
    const context = await browser.newContext({viewport: {width: 1440, height: 1000}});
    await context.setOffline(true);
    const page = await context.newPage();
    page.on("pageerror", e => errors.push(e.message));
    page.on("console", e => {if (e.type() === "error") errors.push(e.text());});
    page.on("request", r => {if (!r.url().startsWith("file:") || r.resourceType() !== "document") requests.push(r.url());});
    const open = async name => {
      await page.goto(pathToFileURL(path.join(fixtures, name + ".html")).href);
      await page.waitForFunction(() => typeof GDD !== "undefined" && GDD.cy && document.getElementById("gdd-view-manifest"));
    };
    const manifest = () => page.evaluate(() => GDD.manifest());
    const data = () => page.locator("#gdd-data").evaluate(e => JSON.parse(e.textContent));
    const select = id => page.locator(`[data-list-node="${id}"]`).click();
    const test = async (name, run) => {try {await run(); results.push({name, result: "PASS"});} catch (error) {error.message = `${name}: ${error.message}`; throw error;}};
    const partition = async () => {
      const d = await data(), m = await manifest();
      for (const [all, shown, omitted] of [
        [Object.keys(d.model.nodes), m.shown_nodes, m.omitted_nodes],
        [d.model.edges.map(e => e.id), m.shown_edges, m.omitted_edges]]) {
        assert.deepEqual([...shown, ...omitted].sort(), all.sort());
        assert.equal(shown.filter(id => omitted.includes(id)).length, 0);
      }
      assert(m.shown_nodes.every(id => m.artifact_scope.shown_nodes.includes(id)));
      assert(m.shown_edges.every(id => m.artifact_scope.shown_edges.includes(id)));
      assert.equal(m.format, "gdd-view-state/0.3");
      assert.equal(m.generator.version, "0.4.0");
    };
    await test("lenses retain exact Python canonical sets and partitions", async () => {
      await open("greenfield");
      const d = await data();
      for (const perspective of ["journey", "components", "data", "all"]) {
        await page.getByLabel("Perspective", {exact: true}).selectOption(perspective);
        const m = await manifest();
        assert.equal(m.filters.perspective, perspective);
        assert.deepEqual(m.shown_nodes, perspective === "all" ? Object.keys(d.model.nodes).sort() : d.perspectives.views[perspective].shown.node_ids);
        assert.deepEqual(m.shown_edges, perspective === "all" ? d.model.edges.map(e => e.id).sort() : d.perspectives.views[perspective].shown.edge_ids);
        await partition();
      }
      await page.getByLabel("Perspective", {exact: true}).selectOption("journey");
      await page.getByLabel("Search nodes and relations").fill("no_matching_concept");
      assert((await manifest()).shown_nodes.length < d.perspectives.views.journey.shown.node_ids.length);
      await page.locator("#reset").click();
      assert.equal((await manifest()).filters.perspective, "all");
    });
    await test("journey action response role and canonical component to data navigation", async () => {
      await page.locator("#perspective").selectOption("journey");
      await select("actor.guest");
      assert((await page.locator("#detail").innerText()).includes("Reserve a seat."));
      await select("step.choose");
      assert((await page.locator("#detail").innerText()).includes("Show available seats."));
      assert((await page.locator("#detail").innerText()).includes("Select a performance."));
      await page.locator("#detail button").filter({hasText: "edge.choose.component"}).click();
      await page.locator("#detail button").filter({hasText: "→ component.booking"}).click();
      await page.locator('#detail [data-perspective="components"]').click();
      assert((await manifest()).shown_nodes.includes("entity.reservation"));
      await select("entity.reservation");
      await page.locator('#detail [data-perspective="data"]').click();
      assert((await page.locator("#detail").innerText()).includes('"target": "entity.performance"'));
      await page.locator("#detail button").filter({hasText: "edge.reservation.performance"}).click();
      const text = await page.locator("#detail").innerText();
      assert(text.includes('"from_cardinality": "0..*"') && text.includes('"to_cardinality": "1"'));
      await page.locator("#detail button").filter({hasText: "→ entity.performance"}).click();
      await page.locator("#detail button").filter({hasText: "edge.capacity.performance"}).click();
      await page.locator("#detail button").filter({hasText: "rule.capacity →"}).click();
      assert((await page.locator("#detail").innerText()).includes("Confirmed reservations must not exceed"));
      await partition();
    });
    await test("refactoring preserves journey IDs and changes canonical responsibility", async () => {
      await open("refactoring");
      await page.locator("#perspective").selectOption("journey");
      await select("step.reserve");
      await page.locator("#detail button").filter({hasText: "edge.reserve.component"}).click();
      assert((await page.locator("#detail").innerText()).includes("component.reservations"));
      await partition();
    });
    await test("partial sketches have advisory navigable gaps and empty Components", async () => {
      await open("partial");
      assert.equal(await page.locator("[data-gap-node]").count(), 5);
      assert((await page.getByRole("region", {name: "Advisory gaps"}).innerText()).includes("never establish readiness"));
      await page.locator("#perspective").selectOption("components");
      assert.equal((await manifest()).shown_nodes.length, 0);
      assert((await page.locator("#empty").innerText()).includes("No components concepts"));
      await page.locator('[data-gap-node="step.reserve"]').first().click();
      assert((await manifest()).shown_nodes.includes("step.reserve"));
      assert((await page.locator("#detail").innerText()).includes("Not declared (advisory gap)"));
      await partition();
    });
    await test("scope intersection and outside gap inspection never widen artifact", async () => {
      for (const file of ["scoped", "partial-scoped"]) {
        await open(file);
        const d = await data();
        assert(d.manifest.omitted_nodes.length > 0);
        for (const lens of ["journey", "components", "data", "all"]) {
          await page.locator("#perspective").selectOption(lens);
          await partition();
        }
        if (file === "partial-scoped") {
          await page.locator('[data-gap-node="entity.reservation"]').click();
          assert((await page.locator("#detail").innerText()).includes("Outside artifact scope"));
          assert(!(await manifest()).shown_nodes.includes("entity.reservation"));
        }
        await page.locator("#reset").click();
        assert.deepEqual((await manifest()).shown_nodes, d.manifest.shown_nodes);
        await partition();
      }
    });
    await test("legacy no journey has clear empty state and responsive controls", async () => {
      await open("legacy");
      await page.locator("#perspective").selectOption("journey");
      assert.equal((await manifest()).shown_nodes.length, 0);
      assert((await page.locator("#empty").innerText()).includes("No user journey concepts"));
      await page.setViewportSize({width: 390, height: 844});
      await page.getByLabel("Perspective", {exact: true}).selectOption("data");
      await page.locator("#reset").click();
      const box = await page.locator("#perspective").boundingBox();
      assert(box.width > 100 && box.x >= 0 && box.x + box.width <= 390);
      await partition();
    });
    await test("hostile human action response and goal are exact inert text", async () => {
      await open("adversarial");
      const d = await data();
      for (const [id, fields] of [["actor.guest", ["goal"]], ["step.choose", ["user_action", "system_response"]]]) {
        await select(id);
        const values = await page.locator("#detail dd").allTextContents();
        for (const field of fields) assert(values.includes(d.model.nodes[id].data[field]));
      }
      assert.equal(await page.evaluate(() => globalThis.gddInjected), undefined);
      assert.equal(await page.locator("#detail img, #detail script").count(), 0);
      await partition();
    });
    await test("lenses kinds searches communities and folding preserve exact intersection", async () => {
      await open("adversarial");
      const d = await data();
      for (const lens of ["journey", "components", "data", "all"]) {
        for (const folded of [false, true]) {
          await page.locator("#perspective").selectOption(lens);
          await page.locator("#community").selectOption("community.product");
          await page.locator("#fold").setChecked(folded);
          for (const kind of ["", "entity"]) {
            await page.locator("#kind").selectOption(kind);
            await page.locator("#search").fill("reservation");
            const m = await manifest();
            const eligible = lens === "all" ? Object.keys(d.model.nodes) : d.perspectives.views[lens].shown.node_ids;
            // Search includes relation text matches, so independently evaluate its canonical witnesses.
            const hits = new Set(d.model.edges.filter(e => d.text.edges[e.id].toLocaleLowerCase().includes("reservation")).flatMap(e => [e.from, e.to]));
            const members = new Set(d.model.edges.filter(e => e.relation === "member_of" && e.to === "community.product").map(e => e.from));
            const expected = eligible.filter(id => (!kind || d.model.nodes[id].kind === kind) &&
              (id === "community.product" || members.has(id)) &&
              (id.toLocaleLowerCase().includes("reservation") || d.text.nodes[id].full.toLocaleLowerCase().includes("reservation") || hits.has(id))).sort();
            assert.deepEqual(m.shown_nodes, expected);
            const eligibleEdges = lens === "all" ? d.model.edges.map(e => e.id) : d.perspectives.views[lens].shown.edge_ids;
            assert.deepEqual(m.shown_edges, d.model.edges.filter(e => eligibleEdges.includes(e.id) && expected.includes(e.from) && expected.includes(e.to)).map(e => e.id).sort());
            assert.equal(m.folding.enabled, folded);
            await partition();
          }
          await page.locator("#kind").selectOption("");
          await page.locator("#search").fill("");
        }
      }
      await page.locator("#reset").click();
      let m = await manifest();
      const forward = m.bundles.find(b => b.from === "step.choose" && b.to === "step.reserve");
      const reverse = m.bundles.find(b => b.from === "step.reserve" && b.to === "step.choose");
      assert.deepEqual(forward.edge_ids.sort(), ["edge.choose.reserve", "edge.parallel"]);
      assert.deepEqual(reverse.edge_ids, ["edge.reverse"]);
      await page.locator("#fold").check();
      m = await manifest();
      assert(m.glyphs.some(g => g.id === "community.product" && g.node_ids.includes("step.choose")));
      assert(["edge.choose.reserve", "edge.parallel", "edge.reverse"].every(id => m.folded_internal_edges.includes(id)));
      await select("step.choose");
      assert((await page.locator("#detail dd").allTextContents()).includes(d.model.nodes["step.choose"].data.user_action));
      await partition();
    });
    await test("related perspective navigation clears filters retains folding and honors scope", async () => {
      for (const file of ["adversarial", "adversarial-scoped"]) {
        await open(file);
        const d = await data();
        await page.locator("#fold").check();
        await page.locator("#kind").selectOption("gdd.product.journey_step");
        await page.locator("#community").selectOption("community.product");
        await page.locator("#search").fill("step.choose");
        await select("step.choose");
        await page.locator('#detail [data-perspective="journey"]').click();
        const m = await manifest();
        assert.equal(m.filters.perspective, "journey");
        assert.equal(m.filters.kind + m.filters.search + m.filters.community, "");
        assert.equal(m.folding.enabled, true);
        assert.deepEqual(m.shown_nodes, d.perspectives.views.journey.shown.node_ids.filter(id => d.manifest.shown_nodes.includes(id)));
        assert((await page.locator("#detail").innerText()).includes("step.choose"));
        if (m.shown_nodes.includes("community.product"))
          assert(m.glyphs.some(g => g.id === "community.product" && g.node_ids.includes("step.choose")));
        await partition();
      }
    });
    await test("gap navigation through empty filtered folded lenses remains scoped", async () => {
      for (const file of ["partial", "partial-scoped"]) {
        await open(file);
        await page.locator("#perspective").selectOption("data");
        await page.locator("#kind").selectOption("entity");
        await page.locator("#search").fill("no_match");
        await page.locator("#fold").check();
        await page.locator('[data-gap-node="step.reserve"]').first().click();
        let m = await manifest();
        assert(m.shown_nodes.includes("step.reserve"));
        assert.equal(m.filters.perspective, "all");
        assert.equal(m.filters.kind + m.filters.search, "");
        assert.equal(m.folding.enabled, true);
        await partition();
        if (file === "partial-scoped") {
          await page.locator("#search").fill("no_match");
          const before = await manifest();
          await page.locator('[data-gap-node="entity.reservation"]').click();
          m = await manifest();
          assert.deepEqual(m, before);
          assert((await page.locator("#detail").innerText()).includes("Outside artifact scope"));
          await page.locator('#detail [data-perspective="data"]').click();
          assert.deepEqual(await manifest(), before);
          await partition();
        }
      }
    });
    await test("portable offline file has no network assets or console errors", async () => {
      assert.deepEqual(errors, []); assert.deepEqual(requests, []);
    });
    console.log(JSON.stringify({format: "gdd-perspectives-browser-conformance/0.1", browser: await browser.version(), offline: true, passed: results.length, results}, null, 2));
  } finally {
    await browser.close();
    await fs.rm(temporary, {recursive: true, force: true});
  }
})().catch(e => {console.error(e); process.exitCode = 1;});
