import cytoscape from "cytoscape";
import fcose from "cytoscape-fcose";

cytoscape.use(fcose);
window.GDDVendor = {
  cytoscape,
  versions: { cytoscape: "3.34.4", fcose: "2.2.0" },
};
