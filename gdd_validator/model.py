"""Deterministic semantics of the typed multigraph."""

from collections import Counter
from math import ceil, floor
import re
from typing import Any

from jsonschema import Draft202012Validator
from referencing import Registry, Resource
from referencing.jsonschema import DRAFT202012
from referencing.exceptions import NoSuchResource

from .loading import Bundle, canonical
from .numeric import exact_number, exact_numbers
from .report import check, pointer

__all__ = ["validate_model", "type_references", "CORE_KINDS", "RELATIONS", "has_cycle"]

CORE_KINDS = {
    "component",
    "entity",
    "action",
    "screen",
    "rule",
    "scenario",
    "machine",
    "community",
}
TARGETS = {"component", "entity", "action", "screen", "machine"}
RELATIONS = {
    "owned_by": ({"action", "entity", "screen", "machine"}, {"component"}),
    "stored_in": ({"entity"}, {"component"}),
    "imports": ({"component"}, {"component"}),
    "calls": ({"component", "action", "screen"}, {"action"}),
    "reads": ({"action"}, {"entity"}),
    "creates": ({"action"}, {"entity"}),
    "changes": ({"action"}, {"entity"}),
    "constrains": ({"rule"}, TARGETS),
    "exercises": ({"scenario"}, {"action", "screen", "machine"}),
    "verifies": ({"scenario"}, {"rule"}),
    "state_of": ({"machine"}, {"entity"}),
    "navigates_to": ({"screen"}, {"screen"}),
    "depends_on": (TARGETS, TARGETS),
    "refines": (
        CORE_KINDS - {"community"},
        {"component", "entity", "action", "screen"},
    ),
    "member_of": (CORE_KINDS - {"community"}, {"community"}),
}


def has_cycle(graph: dict[Any, list[Any]]) -> bool:
    indegree = Counter({n: 0 for n in graph})
    for children in graph.values():
        indegree.update(children)
    queue = [n for n, degree in indegree.items() if degree == 0]
    processed = 0
    while queue:
        node = queue.pop()
        processed += 1
        for child in graph.get(node, []):
            indegree[child] -= 1
            if indegree[child] == 0:
                queue.append(child)
    return processed != len(indegree)


def type_references(value: Any) -> set[str]:
    """References in one declared type, never arbitrary annotation or domain data."""
    if not isinstance(value, dict):
        return set()
    if value.get("type") == "reference":
        target = value.get("target")
        return {target} if isinstance(target, str) else set()
    if value.get("type") == "object":
        return set().union(
            *(type_references(v) for v in value.get("properties", {}).values())
        )
    if value.get("type") == "array":
        return type_references(value.get("items"))
    return set()


def validate_model(bundle: Bundle) -> None:
    model = bundle.model
    nodes, edges = model["nodes"], model["edges"]
    start = len(bundle.checks)
    identifiers: list[str] = []

    def issue(
        code: str, message: str, ident: str = "", suffix: str = "", result: str = "FAIL"
    ) -> None:
        file, location = bundle.locations.get(ident, (bundle.path.name, ""))
        bundle.issue(
            code,
            message,
            file,
            location + suffix,
            result=result,
            ids=(ident,) if ident else (),
        )

    def reference(
        target: str, kinds: set[str] | None, ident: str, suffix: str = ""
    ) -> None:
        if target not in nodes:
            issue("REFERENCE_UNRESOLVED", f"Unknown node {target}.", ident, suffix)
        elif kinds and nodes[target]["kind"] not in kinds:
            issue(
                "ENDPOINT_KIND_INVALID",
                f"{target} must have kind in {sorted(kinds)}.",
                ident,
                suffix,
            )

    def source_reference(ref: dict, ident: str, path: str) -> None:
        source = model.get("sources", {}).get(ref["source"])
        if source is None:
            issue(
                "REFERENCE_UNRESOLVED", f"Unknown source {ref['source']}.", ident, path
            )
        selector = ref.get("selector", {})
        if selector.get("kind") == "json-pointer" and not re.fullmatch(
            r"(?:/(?:[^~]|~[01])*)*", selector["value"]
        ):
            issue("CONSTRAINT_INVALID", "Invalid JSON Pointer.", ident, path)
        if selector.get("kind") == "line-range":
            match = re.fullmatch(r"L([1-9][0-9]*)-L([1-9][0-9]*)", selector["value"])
            if (
                not match
                or (len(match[1]), match[1]) > (len(match[2]), match[2])
                or not (source or {}).get("revision", {}).get("sha256")
            ):
                issue(
                    "CONSTRAINT_INVALID",
                    "Line range must be valid and source hash pinned.",
                    ident,
                    path,
                )

    for registry in ("nodes", "sources", "questions", "extensions"):
        identifiers.extend(model.get(registry, {}))
    identifiers.extend(e["id"] for e in edges)
    identifiers.extend(
        t["id"]
        for n in nodes.values()
        for t in n.get("machine", {}).get("transitions", [])
    )
    for ident, count in Counter(identifiers).items():
        if count > 1:
            issue("ID_COLLISION", f"ID occurs {count} times: {ident}.", ident)

    custom_kinds, custom_relations = {}, {}
    namespaces = [e["namespace"] for e in model.get("extensions", {}).values()]
    if len(set(namespaces)) != len(namespaces):
        issue("EXTENSION_OVERRIDE", "Multiple declarations for one namespace.")
    unsupported_names = set(namespaces) - set(bundle.extensions)

    def reject_fetch(uri: str) -> Any:
        raise NoSuchResource(ref=uri)

    unsupported_schemas: set[str] = set()

    def valid_data_schema(data_schema: dict, ident: str) -> bool:
        """Inspect schema positions and every active local reference target.

        A local pointer can activate an object under an otherwise inert keyword.
        Unreferenced const/annotation values remain ordinary data. Identity-based
        visitation permits recursive schemas without recursively walking them.
        """
        single = {
            "additionalProperties",
            "unevaluatedProperties",
            "propertyNames",
            "contains",
            "items",
            "unevaluatedItems",
            "not",
            "if",
            "then",
            "else",
            "contentSchema",
        }
        maps = {
            "properties",
            "patternProperties",
            "$defs",
            "definitions",
            "dependentSchemas",
        }
        arrays = {"allOf", "anyOf", "oneOf", "prefixItems"}
        data_schema = exact_numbers(data_schema)
        pending, visited = [data_schema], set()
        resolver = None
        try:
            while pending:
                value = pending.pop()
                if id(value) in visited:
                    continue
                visited.add(id(value))
                Draft202012Validator.check_schema(value)
                if not isinstance(value, dict):
                    continue  # Boolean subschemas are valid.
                if (
                    "$id" in value
                    or "x-executable" in value
                    or value.get(
                        "$schema", "https://json-schema.org/draft/2020-12/schema"
                    )
                    != "https://json-schema.org/draft/2020-12/schema"
                ):
                    raise ValueError(
                        "Active schemas must use Draft 2020-12 without IDs/hooks."
                    )
                for key in ("$ref", "$dynamicRef"):
                    if key not in value:
                        continue
                    ref = value[key]
                    if not isinstance(ref, str) or not ref.startswith("#"):
                        raise ValueError("Schema references must remain local.")
                    if resolver is None:
                        resolver = (
                            Registry(retrieve=reject_fetch)
                            .with_resource(
                                "",
                                Resource.from_contents(
                                    data_schema, default_specification=DRAFT202012
                                ),
                            )
                            .resolver()
                        )
                    pending.append(resolver.lookup(ref).contents)
                pending.extend(value[key] for key in single & value.keys())
                pending.extend(
                    child
                    for key in maps & value.keys()
                    for child in value[key].values()
                )
                pending.extend(
                    child for key in arrays & value.keys() for child in value[key]
                )
        except RecursionError:
            unsupported_schemas.add(ident)
            issue(
                "EXTENSION_UNSUPPORTED",
                "Extension schema exceeds supported validation depth.",
                ident,
                result="UNKNOWN",
            )
            return True  # Preserve the declaration; its data cannot get semantic PASS.
        except Exception as error:
            issue("EXTENSION_DATA_INVALID", str(error), ident)
            return False
        return True

    for namespace, extension in bundle.extensions.items():
        for registry, known in (
            ("node_kinds", custom_kinds),
            ("relations", custom_relations),
        ):
            for name, definition in extension[registry].items():
                if (
                    name in CORE_KINDS
                    or name in RELATIONS
                    or not name.startswith(namespace + ".")
                    or name in known
                ):
                    issue("EXTENSION_OVERRIDE", f"Invalid extension name {name}.")
                    continue
                if valid_data_schema(definition["data_schema"], name):
                    known[name] = definition
    for name, definition in custom_relations.items():
        for endpoint in definition["from"] + definition["to"]:
            if endpoint not in CORE_KINDS and endpoint not in custom_kinds:
                issue(
                    "ENDPOINT_KIND_INVALID",
                    f"Unknown extension endpoint kind {endpoint}.",
                )

    def custom(name: str, value: dict, registry: dict, ident: str) -> None:
        declaration = next((n for n in namespaces if name.startswith(n + ".")), None)
        if not declaration:
            issue("EXTENSION_UNDECLARED", f"No namespace declares {name}.", ident)
        elif declaration in unsupported_names:
            issue(
                "EXTENSION_UNSUPPORTED",
                f"Unsupported semantics for {name}.",
                ident,
                result="UNKNOWN",
            )
        elif name in unsupported_schemas:
            issue(
                "EXTENSION_UNSUPPORTED",
                f"Unsupported schema depth for {name}.",
                ident,
                result="UNKNOWN",
            )
        elif name not in registry:
            issue("EXTENSION_UNDECLARED", f"No definition for {name}.", ident)
        else:
            try:
                errors = list(
                    Draft202012Validator(
                        exact_numbers(registry[name]["data_schema"]),
                        registry=Registry(retrieve=reject_fetch),
                    ).iter_errors(exact_numbers(value.get("data", {})))
                )
                for error in errors:
                    issue(
                        "EXTENSION_DATA_INVALID",
                        error.message,
                        ident,
                        "/data" + pointer(*error.path),
                    )
            except RecursionError:
                issue(
                    "EXTENSION_UNSUPPORTED",
                    "Extension data exceeds supported validation depth.",
                    ident,
                    result="UNKNOWN",
                )
            except Exception as error:
                # Unresolvable local references are invalid, never silently accepted.
                issue(
                    "EXTENSION_DATA_INVALID",
                    f"Schema cannot resolve local data: {error}.",
                    ident,
                )

    outgoing: dict[tuple[str, str], list[str]] = {}
    for index, edge in enumerate(edges):
        ident = edge["id"]
        bundle.locations.setdefault(ident, (bundle.path.name, pointer("edges", index)))
        relation = edge["relation"]
        for side in ("from", "to"):
            reference(edge[side], None, ident, "/" + side)
        outgoing.setdefault((edge["from"], relation), []).append(edge["to"])
        if relation in RELATIONS:
            kinds_from, kinds_to = RELATIONS[relation]
            if relation in {"member_of", "refines"}:
                kinds_from = (CORE_KINDS | set(custom_kinds)) - {"community"}
            reference(edge["from"], kinds_from, ident, "/from")
            reference(edge["to"], kinds_to, ident, "/to")
        else:
            custom(relation, edge, custom_relations, ident)
            if relation in custom_relations:
                for side in ("from", "to"):
                    reference(
                        edge[side],
                        set(custom_relations[relation][side]),
                        ident,
                        "/" + side,
                    )
    parents = {n: outgoing.get((n, "refines"), []) for n in nodes}
    if has_cycle(parents):
        issue("REFINEMENT_CYCLE", "Refines contains a cycle or self-loop.")

    def ancestors(ident: str) -> set[str]:
        found, pending = set(), list(parents.get(ident, []))
        while pending:
            current = pending.pop()
            if current not in found:
                found.add(current)
                pending.extend(parents.get(current, []))
        return found

    def imports_path(source: str, target: str, reach: str) -> bool:
        # Start at an actual edge: a transitive self-forbid does not imply a
        # forbidden zero-length path, but does reject a self-edge or cycle.
        pending = list(outgoing.get((source, "imports"), []))
        if reach == "direct":
            return target in pending
        visited = set()
        while pending:
            current = pending.pop()
            if current == target:
                return True
            if (
                current not in visited
                and nodes.get(current, {}).get("kind") == "component"
            ):
                visited.add(current)
                pending.extend(outgoing.get((current, "imports"), []))
        return False

    def types(
        spec: dict,
        ident: str,
        path: str,
        field_entry: bool = False,
        value_required: bool = True,
    ) -> None:
        if "optional" in spec and not field_entry:
            issue(
                "TYPE_CONSTRAINT_INVALID",
                "optional is only legal in input/context/entity fieldmaps.",
                ident,
                path,
            )
        # A required outer value can still be null, an omitted field, an object
        # without an optional property, or an empty array. Such choices do not
        # require a value from every nested non-null domain.
        value_required = (
            value_required
            and not spec.get("nullable", False)
            and not (field_entry and spec.get("optional", False))
        )
        minimum = exact_number(spec["minimum"]) if "minimum" in spec else None
        maximum = exact_number(spec["maximum"]) if "maximum" in spec else None
        if minimum is not None and maximum is not None and minimum > maximum:
            issue("TYPE_CONSTRAINT_INVALID", "minimum exceeds maximum.", ident, path)
        elif (
            spec.get("type") == "integer"
            and minimum is not None
            and maximum is not None
            and value_required
            and ceil(minimum) > floor(maximum)
        ):
            issue(
                "TYPE_CONSTRAINT_INVALID",
                "Required non-null integer interval contains no integer.",
                ident,
                path,
            )
        if spec.get("type") == "reference":
            reference(spec["target"], {"entity"}, ident, path + "/target")
        if spec.get("type") == "object":
            properties = spec.get("properties", {})
            if set(spec.get("required", [])) - set(properties):
                issue(
                    "TYPE_CONSTRAINT_INVALID",
                    "required names must exist in properties.",
                    ident,
                    path,
                )
            for name, item in properties.items():
                types(
                    item,
                    ident,
                    path + pointer("properties", name),
                    value_required=value_required and name in spec.get("required", []),
                )
        if spec.get("type") == "array":
            # Core arrays have no minItems constraint; [] always inhabits them.
            types(spec["items"], ident, path + "/items", value_required=False)

    for ident, node in nodes.items():
        kind = node["kind"]
        if kind not in CORE_KINDS:
            custom(kind, node, custom_kinds, ident)
        if len(outgoing.get((ident, "owned_by"), [])) > 1:
            issue(
                "OWNERSHIP_AMBIGUOUS", "At most one ownership edge is allowed.", ident
            )
        if len(parents[ident]) > 1:
            issue(
                "REFINEMENT_PARENT_AMBIGUOUS",
                "At most one refinement parent is allowed.",
                ident,
            )
        if len(outgoing.get((ident, "state_of"), [])) > 1:
            issue("OWNERSHIP_AMBIGUOUS", "Machine state_of is ambiguous.", ident)
        for mapping in node.get("realization", []):
            for child in mapping["children"]:
                reference(child, None, ident, "/realization")
                if child == ident or ident not in ancestors(child):
                    issue(
                        "REALIZATION_INVALID",
                        f"{child} is not a descendant of {ident}.",
                        ident,
                    )
        for target in node.get("interfaces", []):
            reference(target, {"action"}, ident, "/interfaces")
        for name, spec in node.get("fields", {}).items():
            types(spec, ident, pointer("fields", name), True)
        contract = node.get("contract", {})
        for fieldmap in ("input", "context"):
            for name, spec in contract.get(fieldmap, {}).items():
                types(spec, ident, pointer("contract", fieldmap, name), True)
        if "output" in contract:
            types(contract["output"], ident, "/contract/output")
        for risk in contract.get("risks", {}).values():
            for target in risk["scenarios"]:
                reference(target, {"scenario"}, ident, "/contract/risks")
        for error, scenarios in contract.get("error_cases", {}).items():
            if error not in contract.get("errors", {}):
                issue(
                    "CONSTRAINT_INVALID",
                    f"Undeclared error outcome {error}.",
                    ident,
                    "/contract/error_cases",
                )
            for target in scenarios:
                reference(target, {"scenario"}, ident, "/contract/error_cases")
        rule = node.get("rule", {})
        if rule.get("scope") == "targeted" and not outgoing.get((ident, "constrains")):
            issue(
                "CONSTRAINT_INVALID", "Targeted rule has no constrains target.", ident
            )
        if "constraint" in rule:
            constraint = rule["constraint"]
            if rule.get("category") != "architecture":
                issue(
                    "CONSTRAINT_INVALID",
                    "Core constraints require architecture category.",
                    ident,
                )
            if constraint["relation"] != "imports":
                issue(
                    "CONSTRAINT_INVALID",
                    "Only imports constraints are supported by core 0.2.",
                    ident,
                )
            for side in ("from", "to"):
                reference(
                    constraint[side], {"component"}, ident, "/rule/constraint/" + side
                )
            if (
                rule.get("category") == "architecture"
                and constraint["relation"] == "imports"
                and constraint["policy"] == "forbid"
                and all(
                    nodes.get(constraint[side], {}).get("kind") == "component"
                    for side in ("from", "to")
                )
                and imports_path(
                    constraint["from"], constraint["to"], constraint["reach"]
                )
            ):
                issue(
                    "CONSTRAINT_INVALID",
                    f"Declared imports have a forbidden {constraint['reach']} path "
                    f"from {constraint['from']} to {constraint['to']}.",
                    ident,
                    "/rule/constraint",
                )
        machine = node.get("machine", {})
        if machine:
            states = machine["states"]
            for state in [machine["initial"], *machine["terminal"]]:
                if state not in states:
                    issue("STATE_UNDECLARED", f"Undeclared state {state}.", ident)
            seen_transitions = set()
            for transition in machine["transitions"]:
                for side in ("from", "to"):
                    if transition[side] not in states:
                        issue(
                            "STATE_UNDECLARED",
                            f"Undeclared state {transition[side]}.",
                            ident,
                        )
                key = transition["from"], transition["trigger"]
                if key in seen_transitions:
                    issue(
                        "TRANSITION_AMBIGUOUS",
                        f"Repeated (from,trigger): {key}.",
                        ident,
                    )
                seen_transitions.add(key)
                if transition["from"] in machine["terminal"]:
                    issue(
                        "TERMINAL_MUTATION",
                        "Terminal state has outgoing mutation.",
                        ident,
                    )
                reference(
                    transition["trigger"], {"action"}, ident, "/machine/transitions"
                )
        for field in ("source_refs", "design_refs"):
            for ref in node.get(field, []):
                if field == "design_refs" and ref["purpose"] != "contract":
                    issue(
                        "CONSTRAINT_INVALID",
                        "Design references must have purpose=contract.",
                        ident,
                        "/design_refs",
                    )
                source_reference(ref, ident, "/" + field)

    for ident, question in model.get("questions", {}).items():
        for target in question["targets"]:
            reference(target, None, ident, "/targets")
    for edge in edges:
        for ref in edge.get("source_refs", []):
            source_reference(ref, edge["id"], "/source_refs")
    for gate in model["project"].get("workflow", {}).get("gates", {}).values():
        for target in gate["scope"]:
            reference(target, None, "", "/project/workflow/gates")
    sources = model.get("sources", {})
    lineage: dict[Any, list[Any]] = {}
    for ident, source in sources.items():
        identity = ident, canonical(source.get("revision", {}))
        lineage[identity] = []
        if source["normative"] and not source.get("revision", {}).get("sha256"):
            issue("CONTRACT_SOURCE_UNPINNED", "Normative source needs bytehash.", ident)
        for parent in source.get("derived_from", []):
            target = parent["source"]
            if target not in sources:
                issue(
                    "REFERENCE_UNRESOLVED",
                    f"Unknown derivation source {target}.",
                    ident,
                )
                continue
            if "revision" not in parent or parent["revision"] != sources[target].get(
                "revision"
            ):
                issue(
                    "REVISION_UNRESOLVED",
                    "Historical derivation revision is unavailable; not rebound to current.",
                    ident,
                    result="UNKNOWN",
                )
            if "revision" in parent:
                lineage[identity].append((target, canonical(parent["revision"])))
    if has_cycle(lineage):
        issue("SOURCE_DERIVATION_CYCLE", "Source+revision derivation contains a cycle.")
    if len(bundle.checks) == start:
        bundle.checks.append(
            check(
                "model.semantics",
                "model",
                "PASS",
                "Core references, endpoints, identity, refinement, machines, types and declared extension data checked.",
            )
        )
