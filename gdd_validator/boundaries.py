"""Canonical component interfaces and derived, ownership-aware boundary views."""

from copy import deepcopy

from .loading import Bundle

__all__ = ["PRINCIPLES", "owners", "call_boundaries", "validate_boundaries", "boundary_view", "design_checklist"]

PRINCIPLES = {
    "kiss": "Do the layers and abstractions serve a current requirement?",
    "dry": "Is the same authoritative business knowledge implemented in multiple places?",
    "separation_of_concerns": "Are decisions and state changes with the responsible component?",
    "modularity": "Can components change through public contracts without knowledge of other internals?",
}


def owners(model: dict) -> dict[str, str]:
    """Resolve only a unique explicit owned_by edge; never infer an owner."""
    candidates = {}
    for edge in model["edges"]:
        if edge["relation"] == "owned_by":
            candidates.setdefault(edge["from"], []).append(edge["to"])
    return {ident: targets[0] for ident, targets in candidates.items() if len(targets) == 1}


def call_boundaries(model: dict) -> list[dict]:
    nodes, owned = model["nodes"], owners(model)
    result = []
    for edge in sorted(model["edges"], key=lambda e: e["id"]):
        if edge["relation"] != "calls":
            continue
        source, action = edge["from"], edge["to"]
        consumer = source if nodes.get(source, {}).get("kind") == "component" else owned.get(source)
        provider = owned.get(action)
        interface = nodes.get(provider, {})
        status = "unresolved"
        if consumer and provider:
            if consumer == provider:
                status = "internal"
            elif "interfaces" in interface:
                status = "public" if action in interface["interfaces"] else "private"
        result.append({"edge": edge["id"], "source": source, "consumer": consumer, "provider": provider,
                       "action": action, "status": status})
    return result


def validate_boundaries(bundle: Bundle) -> None:
    """0.3 model checks reject contradictions while retaining incomplete sketches."""
    if bundle.model["format"] != "gdd/0.3":
        return
    nodes, owned = bundle.model["nodes"], owners(bundle.model)

    def issue(code: str, message: str, ident: str, suffix: str, related: tuple = ()) -> None:
        file, location = bundle.locations.get(ident, (bundle.path.name, ""))
        bundle.issue(code, message, file, location + suffix, ids=related or (ident,))

    for ident, node in nodes.items():
        if node["kind"] == "component":
            for action in node.get("interfaces", []):
                if action in owned and owned[action] != ident:
                    issue("INTERFACE_OWNER_MISMATCH", f"Public action {action} belongs to {owned[action]}, not {ident}.",
                          ident, "/interfaces", (ident, action, owned[action]))
        rule = node.get("rule", {})
        if "principle" in rule and (rule["category"] != "design" or rule["blocking"]):
            issue("DESIGN_PRINCIPLE_INVALID", "Broad design-principle judgments require category=design and blocking=false.",
                  ident, "/rule/principle")
    for interaction in call_boundaries(bundle.model):
        if interaction["status"] == "private":
            issue("PRIVATE_ACTION_CALL", f"Cross-component call must use {interaction['provider']}'s public interface.",
                  interaction["edge"], "", (interaction["source"], interaction["action"], interaction["consumer"], interaction["provider"]))


def boundary_view(bundle: Bundle, scope: list[str] | None = None) -> dict:
    """Derive views without changing ownership, permission, or canonical contracts."""
    if bundle.model.get("format") != "gdd/0.3":
        raise ValueError("Boundary views require an explicit gdd/0.3 model.")
    nodes, owned = bundle.model["nodes"], owners(bundle.model)
    selected = set(nodes if scope is None else scope)
    if selected - nodes.keys():
        raise ValueError("Unknown boundary view scope.")
    components = {ident for ident in selected if nodes[ident]["kind"] == "component"}
    components.update(owned[ident] for ident in selected if ident in owned)
    components &= nodes.keys()
    all_calls = call_boundaries(bundle.model)
    calls = [item for item in all_calls
             if item["source"] in selected
             and item["action"] in selected]

    def contract(provider: str, action: str) -> dict:
        return {"project_id": bundle.model["project"]["id"], "provider": provider,
                "action": action, "spec_revision": bundle.revision}

    communities = {}
    for ident in sorted(selected):
        if nodes[ident]["kind"] != "community":
            continue
        basis = {}
        members = {e["from"] for e in bundle.model["edges"]
                   if e["relation"] == "member_of" and e["to"] == ident}
        for edge in bundle.model["edges"]:
            if edge["relation"] != "member_of" or edge["to"] != ident:
                continue
            member = edge["from"]
            provider = member if nodes.get(member, {}).get("kind") == "component" else owned.get(member)
            if provider:
                basis.setdefault(provider, set()).add(member)
        visible = components & basis.keys()
        classified = {"inbound": [], "outbound": [], "internal": [], "unresolved": []}
        for item in calls:
            consumer, provider = item["consumer"], item["provider"]
            relevant = (consumer in visible or provider in visible
                        or item["source"] in members or item["action"] in members)
            if not relevant:
                continue
            if not consumer or not provider:
                classified["unresolved"].append(item["edge"])
            elif consumer in visible or provider in visible:
                kind = ("internal" if consumer in visible and provider in visible else
                        "outbound" if consumer in visible else "inbound")
                classified[kind].append(item["edge"])
        communities[ident] = {
            "membership_basis": {c: sorted(basis[c]) for c in sorted(visible)},
            "omitted_components": sorted(basis.keys() - components),
            "omitted_calls": [item["edge"] for item in all_calls if item not in calls
                              and (item["consumer"] in basis or item["provider"] in basis
                                   or item["source"] in members or item["action"] in members)],
            "unresolved_members": sorted(e["from"] for e in bundle.model["edges"]
                                         if e["relation"] == "member_of" and e["to"] == ident
                                         and nodes.get(e["from"], {}).get("kind") in {"action", "entity", "screen", "machine"}
                                         and e["from"] not in owned),
            "public_contracts": [contract(c, a) for c in sorted(visible) for a in nodes[c].get("interfaces", [])],
            "calls": classified,
        }
    return {
        "format": "gdd-boundaries/0.3", "spec_revision": bundle.revision,
        "scope": sorted(selected),
        "components": {c: {"boundary": deepcopy(nodes[c].get("boundary", {})),
                           "interfaces_known": "interfaces" in nodes[c],
                           "public_contracts": [contract(c, a) for a in nodes[c].get("interfaces", [])]}
                       for c in sorted(components)},
        "communities": communities, "calls": calls,
        "omitted_calls": [item["edge"] for item in all_calls if item not in calls],
        "unresolved": sorted({c for c in components if "interfaces" not in nodes[c]}
                             | {c for c in components if not nodes[c].get("boundary", {}).get("responsibility", "").strip()
                                or not nodes[c].get("boundary", {}).get("rationale", "").strip()
                                or "excludes" not in nodes[c].get("boundary", {})}
                             | {a for c in components for a in nodes[c].get("interfaces", []) if owned.get(a) != c}
                             | {i["edge"] for i in calls if i["status"] == "unresolved"}),
    }


def design_checklist(bundle: Bundle, scope: list[str]) -> list[dict]:
    """Review questions are advisory context, never computed quality scores."""
    return [{"principle": principle, "question": question, "mode": "advisory",
             "rules": sorted(ident for ident in scope
                             if bundle.model["nodes"][ident].get("rule", {}).get("principle") == principle)}
            for principle, question in PRINCIPLES.items()]
