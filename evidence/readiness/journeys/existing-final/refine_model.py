"""Author the explicit, authorized migration of the observation proposal."""
import copy
import hashlib
import json
from pathlib import Path

root = Path.cwd()

def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n")

proposal = json.loads(Path("proposal.json").read_text())
model = copy.deepcopy(proposal["model_candidate"])
save("gdd-proposal.json", model)
brief = """# Trusted task input

Continue genuine Graphify 0.9.75 AST observations without changing received bytes
or observation IDs. Observations are separate from accepted GDD 0.3 intent.

Implement Reservation with default draft state. Preserve
reserve(value: Reservation) -> Reservation. Reserve moves draft to reserved and
returns the same object; repeat reserve is harmless. Cancel moves reserved to
cancelled; repeat cancel is harmless. Reserving cancelled raises InvalidTransition
without changing state. The human task authorizes isolated modeling,
implementation, verification, integration and the requested acceptance trial.

Implementation choice: cancel also returns the same object. Cancel on draft is
rejected with InvalidTransition because the agreed machine has no draft-to-cancelled
transition. This fills a bounded unspecified API case; it changes no requested result.
Single-threaded local state is the agreed implementation boundary.
"""
Path("INPUT.md").write_text(brief)
brief_hash = hashlib.sha256(Path("INPUT.md").read_bytes()).hexdigest()
model["format"] = "gdd/0.3"
model["project"]["scope"] = "Single-module, single-threaded in-memory Reservation lifecycle."
model["sources"]["source.brief"] = {"role":"input", "title":"Trusted task input and bounded implementation choices", "uri":"INPUT.md", "format":"markdown", "normative":True, "revision":{"sha256":brief_hash}}
nodes = model["nodes"]
for node in nodes.values():
    node["maturity"] = "specified"
    node.setdefault("source_refs", []).append({"source":"source.brief", "purpose":"contract"})
nodes["component.reservations"].update({
    "interfaces":["action.reserve", "action.cancel"],
    "boundary":{"responsibility":"Own Reservation state transitions and stable Python operations.", "rationale":"Both transitions govern the same small local object; one module keeps their invariant coherent.", "excludes":["Persistence", "Network operations", "Concurrency orchestration", "Authorization policy"]},
})
nodes["entity.reservation"].update({"fields":{"state":{"type":"enum", "values":["draft", "reserved", "cancelled"], "description":"Initialized to draft; only the lifecycle operations change it."}}})

def risks(action):
    return {
        "authorization":{"relevant":False,"rationale":"Local object API has no identity or permission system.","scenarios":[]},
        "idempotency":{"relevant":True,"rationale":"Repeated successful operations must be harmless.","scenarios":["scenario.repeat-"+action]},
        "boundaries":{"relevant":True,"rationale":"A cancelled reservation cannot be reserved again.","scenarios":["scenario.cancelled"]},
        "concurrency":{"relevant":False,"rationale":"The bounded API is single-threaded and in memory.","scenarios":[]},
        "atomicity":{"relevant":True,"rationale":"InvalidTransition must leave state unchanged.","scenarios":["scenario.cancelled" if action=="reserve" else "scenario.draft-cancel"]},
    }

def action(name, post, errors, cases, behavior):
    return {"kind":"action", "description":post, "label":name+"()", "maturity":"specified", "source_refs":[{"source":"source.brief","purpose":"contract"}],
        "contract":{"input":{"value":{"type":"reference","target":"entity.reservation","origin":"input"}}, "context":{}, "output":{"type":"reference","target":"entity.reservation"}, "errors":errors,
            "preconditions":["value is a Reservation instance in the declared lifecycle."], "postconditions":[post, "The returned object is value by identity."], "external_effects":[], "effects_complete":True, "dependencies_complete":True, "risks":risks(name), "error_cases":cases}, "behavior":behavior}

reserve = action("reserve", "Draft becomes reserved; reserved remains reserved.", {"InvalidTransition":"Cancelled cannot be reserved; state and identity are unchanged."}, {"InvalidTransition":["scenario.cancelled"]}, "Read value.state. If cancelled, raise InvalidTransition before mutation. If draft, set reserved. If reserved, leave unchanged. Return value itself.")
reserve["source_refs"] = nodes["action.reserve"]["source_refs"]
nodes["action.reserve"] = reserve
nodes["action.cancel"] = action("cancel", "Reserved becomes cancelled; cancelled remains cancelled.", {"InvalidTransition":"Draft cannot be cancelled; state and identity are unchanged."}, {"InvalidTransition":["scenario.draft-cancel"]}, "Read value.state. If draft, raise InvalidTransition before mutation. If reserved, set cancelled. If cancelled, leave unchanged. Return value itself.")
nodes["machine.reservation"] = {"kind":"machine", "description":"Draft starts the lifecycle; cancellation is terminal for state-changing transitions.", "maturity":"specified", "machine":{"states":["draft","reserved","cancelled"], "initial":"draft", "terminal":["cancelled"], "transitions":[
    {"id":"transition.reserve","from":"draft","to":"reserved","trigger":"action.reserve","guard":"state equals draft","effects":"Set state to reserved; preserve object identity."},
    {"id":"transition.cancel","from":"reserved","to":"cancelled","trigger":"action.cancel","guard":"state equals reserved","effects":"Set state to cancelled; preserve object identity."}]}}
nodes["rule.lifecycle"] = {"kind":"rule", "description":"The agreed lifecycle, stable signature, identity and idempotency are fixed acceptance assertions.", "maturity":"specified", "source_refs":[{"source":"source.brief","purpose":"contract"}], "rule":{"category":"behavior","statement":"Reservation defaults to draft. reserve(value: Reservation) -> Reservation preserves identity, reserves draft, leaves repeat reserve harmless, and raises InvalidTransition without mutation on cancelled. cancel cancels reserved, leaves repeat cancel harmless, and preserves identity. Cancel on draft raises InvalidTransition without mutation.", "scope":"global", "blocking":True, "verification":["unittest"]}}
scenarios = [
    ("default","boundary","A new Reservation","Inspect state","State is draft","action.reserve"),
    ("reserve","normal","A draft Reservation","Call reserve(value)","State becomes reserved and result is value","action.reserve"),
    ("repeat-reserve","boundary","A reserved Reservation","Call reserve(value) twice","Both results are value and state remains reserved","action.reserve"),
    ("cancel","normal","A reserved Reservation","Call cancel(value)","State becomes cancelled and result is value","action.cancel"),
    ("repeat-cancel","boundary","A cancelled Reservation","Call cancel(value) twice","Both results are value and state remains cancelled","action.cancel"),
    ("cancelled","negative","A cancelled Reservation","Call reserve(value)","InvalidTransition is raised and state remains cancelled","action.reserve"),
    ("draft-cancel","negative","A draft Reservation","Call cancel(value)","InvalidTransition is raised and state remains draft","action.cancel"),
]
edges = model["edges"]
def edge(id, source, target, relation):
    edges.append({"id":"edge."+id,"from":source,"to":target,"relation":relation})
for id in ("entity.reservation","action.reserve","action.cancel","machine.reservation"):
    edge(id.replace('.','-')+"-owner",id,"component.reservations","owned_by")
edge("machine-state","machine.reservation","entity.reservation","state_of")
for id in ("action.reserve","action.cancel"):
    edge(id.replace('.','-')+"-changes",id,"entity.reservation","changes")
    edge(id.replace('.','-')+"-constrained","rule.lifecycle",id,"constrains")
for short,category,given,when,then,target in scenarios:
    id="scenario."+short
    nodes[id]={"kind":"scenario","description":then,"maturity":"specified","scenario":{"category":category,"given":given,"when":when,"then":then}}
    edge(short+"-exercises",id,target,"exercises")
    edge(short+"-verifies",id,"rule.lifecycle","verifies")
model["questions"]["question.graphify_contracts"].update({"status":"resolved", "resolution":{"decision":"decision.adopt-lifecycle"}})
model["record_files"]=["records.json"]
save("gdd.json", model)
save("records.json", {"format":"gdd-records/0.2", "project_id":model['project']['id'], "records":{}})
save("model-change.pending.json", {"status":"authorized refinement", "base":"gdd-proposal.json", "result":"gdd.json", "preserved_ids":list(proposal['model_candidate']['nodes']), "changed_ids":list(nodes), "rationale":"Adopt the explicit human lifecycle requirements and migrate to 0.3 with owned public operations, boundary rationale and authoritative postconditions.", "observation_artifact_revision":proposal['artifact_revision'], "observations_accepted_as_requirements":False})
print("Materialized the unaccepted 0.2 proposal and explicitly authored the intended 0.3 model.")
