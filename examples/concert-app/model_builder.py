"""Author the bounded demo intent; no application code is inferred or executed."""

from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path

from gdd_validator.loading import parse

ROOT = Path(__file__).resolve().parent
SEED = ROOT.parent / "concert-0.3.gdd.yaml"
RISKS = ("authorization", "idempotency", "boundaries", "concurrency", "atomicity")


def text_type(optional=False):
    return {"type": "string", "min_length": 1, **({"optional": True} if optional else {})}


def object_type(properties):
    return {"type": "object", "properties": properties, "required": list(properties)}


def build(phase):
    model = parse(SEED.read_bytes())
    model.pop("record_files")
    model["project"]["title"] = "Local Sessions concert reservations"
    model["project"]["scope"] = (
        "Active phase A: no per-principal reservation limit. Phase B is a separately revised experiment; keep its future obligations inactive until that revision."
        if phase == "a" else
        "Active phase B: at most two reserved reservations per principal across all concerts; preserve successful keys and compatible phase-A history."
    )
    model["project"]["workflow"] = {"mode": "autonomous", "mandate": "The human authorized this bounded local app and the A-to-B requirement experiment. The host supplies operational authority separately.",
        "gates": {name: {"requirement": "none", "authorities": ["actor.concert.lead"], "scope": []}
                  for name in ("model_changes", "implementation", "acceptance")}}
    model["sources"] = {"source.seed": {"role": "input", "title": "Preserved GDD 0.3 reservation intent",
        "uri": "https://github.com/mikkelkrogsholm/gdd/blob/d9f930b34e93351c85180273c1ef4c1aacf60ac1/examples/concert-0.3.gdd.yaml",
        "format": "gdd/0.3", "normative": False, "revision": {"sha256": sha256(SEED.read_bytes()).hexdigest()}}}
    for ident, file, role, format_name in [("source.requirements", "REQUIREMENTS.md", "knowledge", "markdown"),
            ("source.http", "HTTP.md", "knowledge", "markdown"), ("source.design", "DESIGN.md", "design", "design-md/alpha")]:
        model["sources"][ident] = {"role": role, "title": file, "uri": file, "format": format_name, "normative": True,
                                    "revision": {"sha256": sha256((ROOT / file).read_bytes()).hexdigest()}}
    nodes, edges = model["nodes"], model["edges"]

    def edge(source, relation, target, suffix=""):
        ident = "edge.app." + source + "." + relation + "." + target + suffix
        if not any(e["id"] == ident for e in edges):
            edges.append({"id": ident, "from": source, "relation": relation, "to": target})

    for node in nodes.values():
        node["source_refs"] = [{"source": "source.requirements", "purpose": "contract"}]
        if node["kind"] == "scenario":
            for key, value in node["scenario"].items():
                if isinstance(value, str):
                    node["scenario"][key] = value.replace("Reserve kaldes.", "Reserve is called.").replace("Cancel kaldes.", "Cancel is called.").replace("A ejer én reserved reservation.", "A owns one reserved reservation.")
    nodes["entity.concert"]["fields"].update({k: text_type() for k in ("title", "date", "venue")})
    nodes["component.identity"] = {"kind": "component", "description": "Authenticate fictional credentials and own the opaque session registry.",
        "boundary": {"responsibility": "Credential validation, session resolution and revocation.", "rationale": "Session decisions can change independently of transport and reservation policy.",
                     "excludes": ["Reservation decisions and physical reservation persistence."]},
        "interfaces": ["action.login", "action.logout", "action.current_session"]}
    nodes["entity.session"] = {"kind": "entity", "description": "An opaque in-memory session mapped to one fictional principal.",
                               "fields": {"token": text_type(), "principal_id": text_type()}}
    edge("entity.session", "owned_by", "component.identity")
    edge("entity.session", "stored_in", "component.identity")
    edge("component.identity", "member_of", "community.backend")
    edge("component.api", "imports", "component.identity")
    profile = object_type({"id": text_type(), "name": text_type()})
    concert = object_type({"id": text_type(), "title": text_type(), "date": text_type(), "venue": text_type(),
                          "capacity": {"type": "integer", "minimum": 0}, "available": {"type": "integer", "minimum": 0}})
    reservation = object_type({"id": text_type(), "concert_id": text_type(), "state": {"type": "enum", "values": ["reserved", "cancelled"]}})
    principal_context = {"principal_id": {**text_type(), "origin": "authenticated_context"}}

    def scenario(ident, category, given, when, then, targets):
        nodes[ident] = {"kind": "scenario", "description": then,
                       "scenario": {"category": category, "given": given, "when": when, "then": then}}
        for target in targets:
            edge(ident, "exercises", target)

    def action(ident, owner, inputs, context, output, errors, behavior, effects, auth=False):
        normal, negative = "scenario.app." + ident + ".normal", "scenario.app." + ident + ".negative"
        scenario(normal, "normal", "The operation's declared preconditions hold.", "Execute the public operation.", behavior, [ident])
        scenario(negative, "negative", "Input, session or persistence cannot satisfy the declared contract.", "Attempt the operation.", "Return the declared failure without exposing private data or partial effects.", [ident])
        nodes[ident] = {"kind": "action", "description": behavior, "maturity": "specified",
            "source_refs": [{"source": "source.http", "purpose": "contract"}], "behavior": behavior,
            "contract": {"input": inputs, "context": context, "output": output, "errors": errors,
                         "preconditions": ["Validate the declared input and resolve any required session before exposing private data."],
                         "postconditions": [behavior], "external_effects": effects,
                         "effects_complete": True, "dependencies_complete": True,
                         "risks": {risk: {"relevant": auth and risk == "authorization", "rationale":
                             "The negative scenario covers session/credential rejection." if auth and risk == "authorization" else "This bounded operation has no additional transactional or idempotency obligation beyond its explicit contract.",
                             "scenarios": [negative] if auth and risk == "authorization" else []} for risk in RISKS},
                         "error_cases": {error: [negative] for error in errors}}}
        edge(ident, "owned_by", owner)
        edge("component.api", "calls", ident)
        edge("screen.reserve", "calls", ident)

    action("action.catalog", "component.reservations", {}, {}, object_type({"concerts": {"type": "array", "items": concert}}),
           {"PersistenceUnavailable": "No partial response or mutation."}, "Return fictional concerts with observational availability; this does not promise a subsequent booking.", [], False)
    action("action.list_own_reservations", "component.reservations", {}, principal_context,
           object_type({"reservations": {"type": "array", "items": reservation}, "active_count": {"type": "integer", "minimum": 0},
                        "active_limit": {"type": "integer", "minimum": 1, "nullable": True}}),
           {"Unauthenticated": "Reject before persistence lookup.", "PersistenceUnavailable": "Return no partial private data."},
           "Return only this principal's complete reserved/cancelled history, active count and current nullable domain-owned limit.", [], True)
    if phase == "a":
        nodes["action.list_own_reservations"]["contract"]["postconditions"].append(
            "In phase A return active_limit=null; no per-principal limit is enforced."
        )
    token_context = {"session_token": {"type": "string", "optional": True, "nullable": True}}
    action("action.login", "component.identity", {"username": text_type(), "password": text_type()}, {}, object_type({"principal": profile}),
           {"InvalidCredentials": "Establish no session and return no credential details."},
           "Validate server-owned fictional credentials, establish an unpredictable opaque session, and return public principal information.",
           ["Issue an HttpOnly SameSite=Strict session cookie."], True)
    action("action.logout", "component.identity", {}, token_context, object_type({"principal": {"type": "any", "nullable": True}}), {},
           "Revoke the presented session and expire its cookie; repeated logout is harmless and must not revoke other tokens.", ["Expire the presented session cookie."], False)
    action("action.current_session", "component.identity", {}, token_context, object_type({"principal": {**profile, "nullable": True}}), {},
           "Resolve an opaque token server-side; return its public principal or null without trusting request-body identity.", [], False)
    for action_id, relation, entity in [("action.catalog", "reads", "entity.concert"), ("action.catalog", "reads", "entity.reservation"),
            ("action.list_own_reservations", "reads", "entity.reservation"), ("action.login", "creates", "entity.session"),
            ("action.logout", "changes", "entity.session"), ("action.current_session", "reads", "entity.session")]:
        edge(action_id, relation, entity)
    nodes["component.reservations"]["interfaces"] += ["action.catalog", "action.list_own_reservations"]
    for ident in ("action.reserve", "action.cancel"):
        nodes[ident]["contract"]["errors"]["PersistenceUnavailable"] = "Rollback before returning failure; no partial effects or new key binding."
        nodes[ident]["contract"]["error_cases"]["PersistenceUnavailable"] = ["scenario.atomic_failure"]
        nodes[ident]["source_refs"].append({"source": "source.http", "purpose": "contract"})
    nodes["component.frontend"].update(description="Present the local concert catalog, session flow and personal reservation history.", maturity="specified")
    nodes["screen.reserve"].update(description="Choose a concert, sign in, reserve, inspect history and cancel.", maturity="specified",
        interaction={"states": ["loading", "signed_out", "ready", "submitting", "empty", "error", "sold_out", "at_limit"],
                     "behavior": "Use the shared HTTP contract. Retain a reserve key after uncertain transport failure. Clear private data on identity change and ignore stale responses. Render server text as text; show server-owned limits.",
                     "accessibility": ["Labelled native forms and buttons, visible keyboard focus, semantic headings and status/error announcements.", "Availability, cancellation and errors include text; no color-only meaning."],
                     "responsive": "One event column at 320px, up to three at 1120px, no page overflow, controls at least 44px high and usable at 200% zoom."})
    edge("screen.reserve", "calls", "action.cancel")
    scenario("scenario.ui_complete", "normal", "A signed-out visitor opens the real local app.", "Sign in, reserve a seat, inspect personal history, cancel and sign out.", "Availability and private history reflect actual server state and the complete task is usable with keyboard and narrow layout.", ["screen.reserve"])
    scenario("scenario.ui_recovery", "negative", "Credentials, session, capacity or network cannot satisfy a request.", "Attempt login/reserve/cancel and recover.", "Show an actionable error, preserve uncertain reserve keys, expose no foreign history and do not duplicate effects.", ["screen.reserve"])
    for rule_id, statement, targets, method, scenario_id in [
        ("rule.auth_context", "Only the server-resolved session principal can access personal data or mutate reservations; request-body identity has no authority.", ["action.reserve", "action.cancel", "action.list_own_reservations"], "pytest_concert", "scenario.foreign_cancel"),
        ("rule.ui_complete", "The complete local user flow, responsive layout, keyboard operation and failure recovery follow the pinned design contract.", ["screen.reserve"], "browser_concert", "scenario.ui_complete")]:
        nodes[rule_id] = {"kind": "rule", "description": statement, "rule": {"category": "design" if method == "browser_concert" else "invariant",
            "statement": statement, "scope": "targeted", "blocking": True, "verification": [method]}}
        for target in targets: edge(rule_id, "constrains", target)
        edge(scenario_id, "verifies", rule_id)
    for node in nodes.values():
        if node.get("rule", {}).get("blocking"):
            node["rule"]["verification"] = ["python_import_graph" if "constraint" in node["rule"] else
                                            "browser_concert" if node is nodes["rule.ui_complete"] else "pytest_concert"]
    if phase == "b":
        for action_id in ("action.reserve", "action.cancel", "action.list_own_reservations", "action.logout"):
            nodes[action_id]["contract"]["errors"]["SessionChanged"] = "At the HTTP boundary reject a stale presented identity before private reads or effects; the expectation never selects the authenticated principal."
            nodes[action_id]["contract"]["error_cases"]["SessionChanged"] = ["scenario.ui_session_change"]
            nodes[action_id]["contract"]["postconditions"].append("Reject a stale browser identity expectation against the server-resolved session before reading personal data or applying effects.")
        scenario("scenario.ui_session_change", "negative", "Two tabs share a cookie; one changes the session from Alice to Bob.",
                 "The stale Alice tab tries a private refresh, reservation or logout.",
                 "Return SessionChanged before effects; clear the stale personal view and require explicit sign-in. Bob's session and history remain intact.",
                 ["screen.reserve", "action.reserve", "action.cancel", "action.list_own_reservations", "action.logout"])
        edge("scenario.ui_session_change", "verifies", "rule.auth_context")
        edge("scenario.ui_session_change", "verifies", "rule.ui_complete")
        statement = "Each principal has at most two reserved reservations across all concerts. Cancelled history does not consume allowance."
        nodes["rule.active_limit"] = {"kind": "rule", "description": statement, "rule": {"category": "invariant", "statement": statement,
            "scope": "targeted", "blocking": True, "verification": ["pytest_concert"]}}
        for target in ("action.reserve", "action.list_own_reservations", "entity.reservation"): edge("rule.active_limit", "constrains", target)
        for ident, category, when in [("scenario.active_limit", "boundary", "Attempt a third distinct-key reservation in a concert with free capacity."),
                ("scenario.active_limit_concurrent", "concurrency", "Submit two new keys concurrently across different concerts when the principal has one active reservation."),
                ("scenario.active_limit_replay", "boundary", "Replay a successful active or cancelled key while already at the limit; cancel and retry an unbound failed-limit key.")]:
            scenario(ident, category, "Real persistent reservations exist for separate principals.", when,
                     "Preserve replay semantics and enforce the global limit atomically without binding failed keys; cancellation frees allowance.", ["action.reserve", "action.cancel", "action.list_own_reservations"])
            edge(ident, "verifies", "rule.active_limit")
        nodes["action.reserve"]["contract"]["errors"]["ActiveReservationLimitReached"] = "Reject a new key at two active reservations without mutation or key binding."
        nodes["action.reserve"]["contract"]["error_cases"]["ActiveReservationLimitReached"] = ["scenario.active_limit"]
        nodes["action.reserve"]["contract"]["postconditions"].append(statement)
        nodes["action.reserve"]["behavior"] += "\nFor a new key, after resolving concert existence enforce the principal's global active limit of two BEFORE concert capacity. Both counts, insert and key binding are in one protected transaction. Successful replay remains before either limit."
        nodes["action.reserve"]["contract"]["risks"]["concurrency"]["scenarios"].append("scenario.active_limit_concurrent")
        for transition in nodes["machine.reservation"]["machine"]["transitions"]:
            if transition["trigger"] == "action.reserve": transition["guard"] += " AND the principal has fewer than two active reservations across all concerts."
        nodes["action.list_own_reservations"]["contract"]["postconditions"].append("Return active_limit=2 from the domain's single policy value.")
        model["project"]["assumptions"].append("Phase B requires a compatible existing database; refuse incompatible over-limit state without changing history.")
    model["record_files"] = ["gdd.records.json"]
    return model


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("phase", choices=["a", "b"])
    args = parser.parse_args()
    (ROOT / "gdd.json").write_text(json.dumps(build(args.phase), ensure_ascii=False, indent=2) + "\n")
