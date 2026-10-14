"""Explicit, unaccepted model proposals from selected code observations."""

from copy import deepcopy
import json
from pathlib import Path

from jsonschema import Draft202012Validator

from .graphify_io import load_observations
from .loading import Bundle, schema, schema_checks
from .model import validate_model

__all__ = ["propose_graphify"]


def propose_graphify(
    path: str | Path, mapping: dict, *, source_uri: str = "graphify.raw.json"
) -> dict:
    try:
        # Validate original Python strings too: a JSON escape roundtrip can
        # normalize surrogate pairs while leaving the caller's input unchanged.
        json.dumps(
            {"mapping": mapping, "source_uri": source_uri},
            ensure_ascii=False,
            allow_nan=False,
        ).encode("utf-8")
        errors = list(
            Draft202012Validator(schema("graphify-mapping")).iter_errors(mapping)
        )
    except (TypeError, ValueError, RecursionError) as error:
        raise ValueError(
            "Proposal inputs must be finite JSON values with valid Unicode scalars and supported nesting."
        ) from error
    if errors:
        raise ValueError(
            "Invalid explicit mapping: " + "; ".join(e.message for e in errors)
        )
    index, raw, report = load_observations(path)
    results = {c["check_id"]: c["result"] for c in report["checks"]}
    if (
        index is None
        or raw is None
        or any(c["result"] == "FAIL" for c in report["checks"])
        or any(
            results.get(name) != "PASS"
            for name in ("graphify.integrity", "graphify.projection")
        )
    ):
        raise ValueError(
            "Proposal requires an intact observations bundle without known import violations."
        )
    observations = {n["id"]: n for n in index["nodes"]}
    observed_edges = {e["id"]: e for e in index["edges"]}
    nodes, edges, selected = {}, [], {}
    source_id = index["source_id"]
    for selection in mapping["nodes"]:
        observation = observations.get(selection["observation"])
        if observation is None or observation["classification"] != "code_candidate":
            raise ValueError(
                "Select a producer-claimed code observation explicitly; noncode/unknown inputs cannot stand for code."
            )
        target = selection["target"]
        if target in nodes or selection["observation"] in selected:
            raise ValueError("Node mapping must be one-to-one within this proposal.")
        selected[selection["observation"]] = target
        node = {
            "kind": selection["kind"],
            "description": selection["description"],
            "maturity": "sketch",
            "source_refs": [
                {
                    "source": source_id,
                    "purpose": "provenance",
                    "selector": {
                        "kind": "json-pointer",
                        "value": observation["identity"]["pointer"],
                    },
                }
            ],
        }
        label = observation["raw"].get("label")
        if isinstance(label, str) and label:
            node["label"] = label
        nodes[target] = node
    for selection in mapping["edges"]:
        observation = observed_edges.get(selection["observation"])
        if (
            observation is None
            or observation["direction"]["value"] != "source_to_target"
        ):
            raise ValueError(
                "A proposed relation requires selected observation direction; ambiguous exported order cannot become intended direction."
            )
        expected_from = selected.get(observation["source_observation"])
        expected_to = selected.get(observation["target_observation"])
        if (
            not expected_from
            or not expected_to
            or (selection["from"], selection["to"]) != (expected_from, expected_to)
        ):
            raise ValueError(
                "Edge mapping must preserve the selected endpoint identities and direction."
            )
        edges.append(
            {
                "id": selection["id"],
                "from": expected_from,
                "to": expected_to,
                "relation": selection["relation"],
                "source_refs": [
                    {
                        "source": source_id,
                        "purpose": "provenance",
                        "selector": {
                            "kind": "json-pointer",
                            "value": observation["identity"]["pointer"],
                        },
                    }
                ],
            }
        )
    source = {
        "role": "observation",
        "title": "Original Graphify observation snapshot; producer claims",
        "uri": source_uri,
        "format": index["importer"]["profile"],
        "normative": False,
        "revision": {"sha256": index["artifact"]["revision"].removeprefix("sha256:")},
    }
    model = {
        "format": "gdd/0.2",
        "project": deepcopy(mapping["project"]),
        "nodes": nodes,
        "edges": edges,
        "sources": {source_id: source},
        "questions": {
            "question.graphify_contracts": {
                "statement": "Clarify the intended behavior, contracts, rules and scenarios; code observations describe the existing implementation and do not automatically define intent.",
                "targets": list(nodes),
                "blocks": ["implementation", "acceptance"],
                "status": "open",
            }
        },
    }
    checks = schema_checks(model, "<model_candidate>", "gdd/0.2")
    if any(c["result"] != "PASS" for c in checks):
        raise ValueError(
            "Explicit mapping does not form a valid GDD representation: " + str(checks)
        )
    bundle = Bundle(Path("candidate.gdd.json"), model=model)
    validate_model(bundle)
    if any(c["result"] != "PASS" for c in bundle.checks):
        raise ValueError(
            "Explicit mapping violates GDD model semantics: " + str(bundle.checks)
        )
    return {
        "format": "gdd-graphify-proposal/0.2",
        "status": "proposed",
        "artifact_revision": index["artifact"]["revision"],
        "mapping": deepcopy(mapping),
        "model_candidate": model,
        "binding_candidates": [
            deepcopy(b)
            for b in index["binding_candidates"]
            if b["observation"] in selected
        ],
        "omitted_observations": [
            n["id"]
            for n in index["nodes"] + index["edges"] + index["opaque"]
            if n["id"] not in selected
            and n["id"] not in {e["observation"] for e in mapping["edges"]}
        ],
        "required_actions": [
            "Review the explicit mapping and intended behavior under the project's trusted mandate.",
            "Apply through the ordinary authorized source_promotion/model_change process; no approval receipt is fabricated.",
            "Keep original observations and verify source/code revisions before registering observed or checked bindings.",
        ],
    }
