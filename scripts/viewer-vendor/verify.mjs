import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import vm from "node:vm";

const bundle = await readFile(new URL("../../gdd_validator/viewer/vendor/cytoscape-fcose.js", import.meta.url), "utf8");
const context = vm.createContext({ window: {}, console, setTimeout, clearTimeout, performance }, {
  codeGeneration: { strings: false, wasm: false },
});
new vm.Script(bundle).runInContext(context, { timeout: 10000 });
const vendor = context.window.GDDVendor;
assert.equal(vendor.cytoscape.version, "3.34.4");
assert.equal(vendor.versions.cytoscape, "3.34.4");
assert.equal(vendor.versions.fcose, "2.2.0");
const result = new vm.Script(`(() => {
  const graph = window.GDDVendor.cytoscape({
    headless: true,
    elements: [
      { data: { id: "a" } },
      { data: { id: "b" } },
      { data: { id: "c" } },
      { data: { id: "a-b", source: "a", target: "b" } },
      { data: { id: "b-c", source: "b", target: "c" } },
      { data: { id: "c-a", source: "c", target: "a" } },
    ],
  });
  graph.layout({ name: "fcose", quality: "default", randomize: true, animate: false, fit: false }).run();
  const result = { nodes: graph.nodes().length, edges: graph.edges().length,
    positions: graph.nodes().map((node) => ({ id: node.id(), ...node.position() })) };
  graph.destroy();
  return result;
})()`).runInContext(context, { timeout: 10000 });
assert.equal(result.nodes, 3);
assert.equal(result.edges, 3);
for (const { id, x, y } of result.positions) {
  assert(Number.isFinite(x) && Number.isFinite(y), `Non-finite fCoSE position: ${id}`);
}
const positions = result.positions.map(({ x, y }) => JSON.stringify({ x, y }));
assert.equal(new Set(positions).size, 3, "fCoSE did not distribute the fixture nodes");
console.log("Verified registered fCoSE, finite headless positions, and execution without dynamic code generation or network globals.");
