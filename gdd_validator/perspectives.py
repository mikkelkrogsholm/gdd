"""Advisory journey, component and data projections of one validated model."""

from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path

from .loading import Bundle, InputError, canonical, load_bundle
from .model import validate_model

__all__ = ["perspectives", "perspective_data", "load_perspective_bundle"]

_PRODUCT_BYTES = (Path(__file__).parent / "extensions" / "product-0.1.json").read_bytes()
_PRODUCT = json.loads(_PRODUCT_BYTES)
_PRODUCT_HASH = sha256(_PRODUCT_BYTES).hexdigest()
PREFIX = "gdd.product."


def _product_supported(bundle: Bundle) -> bool:
    """Names alone never authorize interpretation as the shipped product profile."""
    found = False
    for declaration in bundle.model.get("extensions", {}).values():
        definition = bundle.extensions.get(declaration["namespace"], {})
        names = set(definition.get("node_kinds", {})) | set(definition.get("relations", {}))
        if declaration["namespace"] == "gdd.product" or any(n.startswith(PREFIX) for n in names):
            if (declaration["namespace"] != "gdd.product"
                    or declaration["version"] != "0.1.0"
                    or declaration["sha256"] != _PRODUCT_HASH
                    or canonical(definition) != canonical(_PRODUCT)):
                raise InputError("FORMAT_UNSUPPORTED", "Unsupported gdd.product semantics: requires the exact shipped product-0.1.json definition and bytehash (version 0.1.0).")
            found = True
    return found


def load_perspective_bundle(path: str | Path) -> Bundle:
    """Load and validate local assets before exposing any projection."""
    bundle = load_bundle(Path(path))
    if bundle.model.get("format") not in {"gdd/0.2", "gdd/0.3"}:
        raise ValueError("Perspectives require a supported GDD 0.2 or 0.3 manifest.")
    if any(c["result"] != "PASS" for c in bundle.checks):
        raise ValueError("Perspectives require a resolved, valid model bundle; run gdd validate for diagnostics.")
    validate_model(bundle)
    if any(c["result"] != "PASS" for c in bundle.checks) or not bundle.revision:
        raise ValueError("Perspectives require a semantically valid model and snapshot; run gdd validate for diagnostics.")
    _product_supported(bundle)
    return bundle


def perspectives(path: str | Path) -> dict:
    """Inspect a validated manifest; gaps are advisory and never evidence."""
    return perspective_data(load_perspective_bundle(path))


def perspective_data(bundle: Bundle) -> dict:
    """Pure projection of an already validated Bundle, with no model-path reads.

    Call load_perspective_bundle for untrusted manifests. Viewer integrations can
    reuse the validated bundle and retain its exact snapshot revision.
    """
    if not bundle.revision or any(c["result"] != "PASS" for c in bundle.checks):
        raise ValueError("Projection requires an already validated bundle and revision.")
    product = _product_supported(bundle)
    nodes = bundle.model["nodes"]
    edges = sorted(bundle.model["edges"], key=lambda edge: edge["id"])
    all_nodes, all_edges = set(nodes), {e["id"] for e in edges}
    kinds = {ident: node["kind"] for ident, node in nodes.items()}
    primary = {
        "journey": {i for i, k in kinds.items() if product and k in {PREFIX + "actor", PREFIX + "journey_step"}},
        "components": {i for i, k in kinds.items() if k in {"component", "action", "screen"}},
        "data": {i for i, k in kinds.items() if k in {"entity", "rule", "machine"}},
    }
    # Derived links carry canonical witnesses rather than invented graph edges.
    links = []
    for edge in edges:
        relation = edge["relation"]
        if product and relation == PREFIX + "uses":
            links.append({"component": edge["from"], "entity": edge["to"], "via": "uses", "edge_ids": [edge["id"]]})
        if relation in {"owned_by", "stored_in"} and kinds[edge["from"]] == "entity":
            links.append({"component": edge["to"], "entity": edge["from"], "via": relation, "edge_ids": [edge["id"]]})
        if relation in {"reads", "creates", "changes"}:
            for owner in edges:
                if owner["relation"] == "owned_by" and owner["from"] == edge["from"]:
                    links.append({"component": owner["to"], "entity": edge["to"], "via": relation,
                                  "edge_ids": sorted([owner["id"], edge["id"]])})
    links.sort(key=lambda link: (link["component"], link["entity"], link["via"], link["edge_ids"]))
    views = {}
    for name, selected in primary.items():
        shown = set(selected)
        relevant = [e for e in edges if e["from"] in selected or e["to"] in selected]
        for edge in relevant:
            shown.update([edge["from"], edge["to"]])
        edge_ids = {e["id"] for e in relevant}
        views[name] = {
            "primary_node_ids": sorted(selected),
            "nodes": {i: deepcopy(nodes[i]) for i in sorted(shown)},
            "edges": deepcopy(relevant),
            "cross_links": [{"node_id": i, "views": sorted(v for v, ids in primary.items() if v != name and i in ids),
                             "edge_ids": sorted(e["id"] for e in relevant if i in {e["from"], e["to"]})}
                            for i in sorted(shown) if any(i in ids for v, ids in primary.items() if v != name)],
            "shown": {"node_ids": sorted(shown), "edge_ids": sorted(edge_ids)},
            "omitted": {"node_ids": sorted(all_nodes - shown), "edge_ids": sorted(all_edges - edge_ids)},
        }
    gaps = []

    def gap(code, ident, message):
        gaps.append({"code": code, "node_id": ident, "message": message})

    for ident in sorted(nodes):
        node = nodes[ident]
        if product and node["kind"] == PREFIX + "journey_step":
            if not any(e["relation"] == PREFIX + "participates" and e["to"] == ident for e in edges):
                gap("MISSING_ACTOR", ident, "No human actor is linked to this step.")
            for field in ("user_action", "system_response"):
                if field not in node["data"]:
                    gap("MISSING_" + field.upper(), ident, "The step has no declared " + field.replace("_", " ") + ".")
            if not any(e["relation"] == PREFIX + "realized_by" and e["from"] == ident for e in edges):
                gap("MISSING_RESPONSIBLE_COMPONENT", ident, "No responsible component is linked to this step.")
        if node["kind"] == "entity" and not any(link["entity"] == ident for link in links):
            gap("MISSING_DATA_COMPONENT", ident, "No responsible or using component is linked to this entity.")
        if node["kind"] == "rule" and node.get("rule", {}).get("category") == "invariant":
            if not any(e["relation"] == "constrains" and e["from"] == ident for e in edges):
                gap("MISSING_RULE_TARGET", ident, "This invariant has no explicit constrains targets; review whether it is a data rule.")
    return {
        "format": "gdd-perspectives/0.1", "spec_revision": bundle.revision,
        "advisory": True,
        "product_extension": {"namespace": "gdd.product", "version": "0.1.0", "sha256": _PRODUCT_HASH} if product else None,
        "unsupported_extensions": sorted(n for n in bundle.extensions if n != "gdd.product"),
        "views": views, "component_data_links": links, "gaps": gaps,
    }
