"""Fixed-point context closure and structural implementation readiness."""

from hashlib import sha256
import re

from .decision import compatible_decision_supersession
from .record_integrity import invalid_record_ids, model_ids as referenceable_model_ids
from .loading import Bundle, InputError, parse, parse_json, resolve_asset
from .model import CORE_KINDS, type_references
from .report import check, pointer
from .boundaries import call_boundaries, owners

__all__ = [
    "context_closure",
    "implementation_targets",
    "validate_readiness",
]


def _refinement_members(bundle: Bundle, seeds: list[str]) -> set[str]:
    selected = set(seeds)
    while True:
        before = set(selected)
        for edge in bundle.model["edges"]:
            if edge["relation"] == "refines" and edge["to"] in selected:
                selected.add(edge["from"])
        if selected == before:
            return selected


def implementation_targets(bundle: Bundle, seeds: list[str]) -> set[str]:
    """Expand implementation work without making support descendants write targets."""
    return set(seeds) | {
        ident
        for ident in _refinement_members(bundle, seeds)
        if bundle.model["nodes"].get(ident, {}).get("kind")
        not in {"rule", "scenario", "community"}
    }


def context_closure(bundle: Bundle, seeds: list[str]) -> set[str]:
    nodes, edges = bundle.model["nodes"], bundle.model["edges"]
    found = _refinement_members(bundle, seeds)
    definitions = {
        name: definition
        for ext in bundle.extensions.values()
        for name, definition in ext["relations"].items()
    }
    while True:
        before = set(found)
        for ident in list(found):
            node = nodes.get(ident, {})
            found.update(node.get("interfaces", []))
            contract = node.get("contract", {})
            type_specs = list(node.get("fields", {}).values())
            for fieldmap in ("input", "context"):
                type_specs.extend(contract.get(fieldmap, {}).values())
            if "output" in contract:
                type_specs.append(contract["output"])
            for spec in type_specs:
                found.update(type_references(spec))
            found.update(
                t["trigger"] for t in node.get("machine", {}).get("transitions", [])
            )
            for risk in node.get("contract", {}).get("risks", {}).values():
                found.update(risk["scenarios"])
            for cases in node.get("contract", {}).get("error_cases", {}).values():
                found.update(cases)
            constraint = node.get("rule", {}).get("constraint", {})
            found.update(constraint.get(k) for k in ("from", "to") if k in constraint)
        for ident, node in nodes.items():
            if node.get("rule", {}).get("scope") == "global":
                found.add(ident)
        for edge in edges:
            source, target, relation = edge["from"], edge["to"], edge["relation"]
            if source in found and relation in {
                "owned_by",
                "stored_in",
                "calls",
                "depends_on",
                "reads",
                "creates",
                "changes",
                "refines",
                "state_of",
                "imports",
                "navigates_to",
                "constrains",
                "exercises",
                "verifies",
            }:
                found.add(target)
            if bundle.model["format"] == "gdd/0.3" and source in found and relation == "member_of":
                found.add(target)
            if target in found and relation in {
                "constrains",
                "exercises",
                "verifies",
                "state_of",
            }:
                found.add(source)
            definition = definitions.get(relation, {})
            if source in found and (
                definition.get("context") in {"outgoing", "both"}
                or definition.get("dependency")
                or definition.get("effect", "none") != "none"
            ):
                found.add(target)
            if target in found and definition.get("context") in {"incoming", "both"}:
                found.add(source)
        if found == before:
            return found


def validate_readiness(bundle: Bundle, scope: list[str], host: dict) -> None:
    nodes, edges = bundle.model["nodes"], bundle.model["edges"]
    start = len(bundle.checks)
    selected = set(
        scope
        or [
            n
            for n, v in nodes.items()
            if v["kind"] not in {"rule", "scenario", "community"}
        ]
    )

    def issue(
        code: str, message: str, ident: str, result: str = "FAIL", suffix: str = ""
    ) -> None:
        file, location = bundle.locations.get(ident, (bundle.path.name, ""))
        bundle.issue(
            code,
            message,
            file,
            location + suffix,
            result=result,
            dimension="readiness",
            ids=(ident,),
        )

    def field_issue(code: str, message: str, ident: str, *fields: str) -> None:
        """Point to the requested field, or its nearest existing parent.

        Bundle locations already identify the owning file for fragmented models.
        Never manufacture a JSON pointer to a missing object or property.
        """
        value = nodes.get(ident, {})
        existing = []
        for field in fields:
            if not isinstance(value, dict) or field not in value:
                break
            existing.append(field)
            value = value[field]
        issue(code, message, ident, suffix=pointer(*existing))

    for ident in selected - nodes.keys():
        issue("REFERENCE_UNRESOLVED", "Unknown readiness target.", ident)
    # Parent implementation includes its decomposition; an owner in read context does not.
    selected = implementation_targets(bundle, list(selected))
    for ident in selected & nodes.keys():
        if nodes[ident]["kind"] in {"rule", "scenario", "community"}:
            issue(
                "READINESS_CONTRACT_MISSING",
                "Implementation scope must be component/entity/action/screen/machine or a declared extension kind.",
                ident,
            )
    closure = context_closure(bundle, sorted(selected))
    model_issues = [
        c for c in bundle.checks if c["dimension"] == "model" and c["result"] != "PASS"
    ]
    model_ids = referenceable_model_ids(bundle.model)
    invalid_records = invalid_record_ids(bundle)
    record_only_ids = bundle.records.keys() - model_ids
    for entry in model_issues:
        related = {i for d in entry["diagnostics"] for i in d["related_ids"]}
        if related and related <= record_only_ids:
            continue
        if not related or related & closure or any(i not in nodes for i in related):
            code = (
                "EXTENSION_UNSUPPORTED"
                if entry["result"] == "UNKNOWN"
                else "READINESS_CONTRACT_MISSING"
            )
            issue(
                code,
                "Model defect or unknown semantics intersects readiness context: "
                + entry["reason"],
                next(iter(selected), "project"),
                entry["result"],
            )

    def scenarios_for(ident: str, relation: str = "exercises") -> set[str]:
        return {
            e["from"] for e in edges if e["relation"] == relation and e["to"] == ident
        }

    def category(ident: str) -> str | None:
        return nodes.get(ident, {}).get("scenario", {}).get("category")

    def contract_source(ref: dict, ident: str) -> None:
        source_id = ref["source"]
        source = bundle.model.get("sources", {}).get(source_id)
        if source is None:
            return  # model reports the unresolved reference
        if source["format"] not in {
            "text",
            "text/plain",
            "markdown",
            "text/markdown",
            "json",
            "application/json",
            "yaml",
            "application/yaml",
            "design-md/alpha",
            "okf/0.2",
        }:
            issue(
                "SOURCE_UNAVAILABLE",
                f"Unsupported contract source format: {source['format']}.",
                ident,
                "UNKNOWN",
            )
        if not source["normative"] or not source.get("revision", {}).get("sha256"):
            issue(
                "CONTRACT_SOURCE_UNPINNED",
                f"Contract source {source_id} is not normative and pinned.",
                ident,
            )
        try:
            local = resolve_asset(bundle.root, source["uri"])
            raw = local.read_bytes()
            relative = local.relative_to(bundle.root).as_posix()
            if (
                relative not in bundle.assets
                or sha256(raw).hexdigest() != source.get("revision", {}).get("sha256")
                or bundle.revision is None
            ):
                issue(
                    "CONTRACT_SOURCE_UNPINNED",
                    f"Contract source {source_id} is not in a verified snapshot.",
                    ident,
                )
            selector = ref.get("selector", {"kind": "whole-document"})
            # Declared text/structured formats must be readable even for a whole-document
            # selector. JSON cannot silently become YAML through the generic loader.
            text = raw.decode("utf-8")
            structured = None
            if source["format"] in {"json", "application/json"}:
                structured = parse_json(raw)
            elif source["format"] in {"yaml", "application/yaml"}:
                structured = parse(raw)
            kind = selector["kind"]
            if kind == "opaque":
                issue(
                    "SOURCE_UNAVAILABLE",
                    "Opaque selector semantics are unsupported.",
                    ident,
                    "UNKNOWN",
                )
            elif kind == "json-pointer":
                if source["format"] == "design-md/alpha":
                    lines = text.splitlines()
                    if not lines or lines[0] != "---" or "---" not in lines[1:]:
                        raise ValueError("DESIGN.md has no parseable YAML frontmatter.")
                    structured = parse("\n".join(lines[1 : lines.index("---", 1)]))
                elif source["format"] not in {
                    "json",
                    "application/json",
                    "yaml",
                    "application/yaml",
                }:
                    raise ValueError(
                        "JSON Pointer requires JSON/YAML tokens or DESIGN.md frontmatter."
                    )
                value = structured
                for part in selector["value"].split("/")[1:]:
                    part = part.replace("~1", "/").replace("~0", "~")
                    value = (
                        value[int(part)]
                        if isinstance(value, list)
                        and re.fullmatch(r"0|[1-9][0-9]*", part)
                        else value[part]
                    )
            elif kind == "line-range":
                match = re.fullmatch(
                    r"L([1-9][0-9]*)-L([1-9][0-9]*)", selector["value"]
                )
                if not match or int(match[2]) > len(text.splitlines()):
                    raise ValueError("Line range outside source.")
            elif kind == "markdown-heading":
                # Parsing Markdown heading identity is deliberately left to an integration adapter.
                issue(
                    "SOURCE_UNAVAILABLE",
                    "Markdown-heading selectors are preserved but not resolved by this version.",
                    ident,
                    "UNKNOWN",
                )
        except (
            OSError,
            InputError,
            ValueError,
            KeyError,
            IndexError,
            TypeError,
        ) as error:
            issue(
                (
                    error.code
                    if isinstance(error, InputError)
                    and error.code == "FORMAT_UNSUPPORTED"
                    else "SOURCE_UNAVAILABLE"
                ),
                f"Contract source unavailable: {error}.",
                ident,
                "UNKNOWN",
            )

    owned = owners(bundle.model)
    for ident in sorted(closure & nodes.keys()):
        node, kind = nodes[ident], nodes[ident]["kind"]
        # All action boundary contracts in read context are necessary; UI sketches reached only
        # through incoming calls are never included. Context does not become write ownership.
        if kind == "action":
            contract = node.get("contract", {})
            required = {
                "input",
                "context",
                "output",
                "errors",
                "preconditions",
                "external_effects",
                "effects_complete",
                "dependencies_complete",
                "risks",
                "error_cases",
            }
            if bundle.model["format"] == "gdd/0.3":
                required.add("postconditions")
            missing = sorted(required - contract.keys())
            if missing:
                field_issue(
                    "READINESS_CONTRACT_MISSING",
                    "Action contract is missing fields: " + ", ".join(missing) + ".",
                    ident, "contract",
                )
            if not node.get("behavior", "").strip():
                field_issue(
                    "READINESS_CONTRACT_MISSING",
                    "Action behavior is missing or blank; specify its ordered behavior.",
                    ident, "behavior",
                )
            incomplete = [field for field in ("effects_complete", "dependencies_complete")
                          if contract.get(field) is not True]
            if incomplete:
                field_issue(
                    "EFFECTS_INCOMPLETE",
                    "Action contract must declare true: " + ", ".join(incomplete) + ".",
                    ident, "contract", incomplete[0],
                )
            owner_edges = [
                e for e in edges if e["from"] == ident and e["relation"] == "owned_by"
            ]
            if len(owner_edges) != 1:
                issue(
                    "READINESS_CONTRACT_MISSING",
                    "Action requires exactly one owner.",
                    ident,
                )
            exercise = scenarios_for(ident)
            if not any(category(s) == "normal" for s in exercise):
                issue(
                    "SCENARIO_CATEGORY_MISSING",
                    "Action needs a normal scenario.",
                    ident,
                )
            risks = contract.get("risks", {})
            for risk_name in (
                "authorization",
                "idempotency",
                "boundaries",
                "concurrency",
                "atomicity",
            ):
                risk = risks.get(risk_name)
                if not risk or not risk.get("rationale", "").strip():
                    field_issue(
                        "READINESS_CONTRACT_MISSING",
                        f"Risk assessment {risk_name} needs a nonblank rationale.",
                        ident, "contract", "risks", risk_name, "rationale",
                    )
                elif risk["relevant"]:
                    allowed = (
                        {"concurrency"}
                        if risk_name == "concurrency"
                        else {"negative", "boundary"}
                    )
                    refs = risk["scenarios"]
                    if not refs or any(
                        s not in exercise or category(s) not in allowed for s in refs
                    ):
                        issue(
                            "SCENARIO_CATEGORY_MISSING",
                            f"Risk {risk_name} needs exercising scenarios of category {allowed}.",
                            ident,
                        )
            for error_name in contract.get("errors", {}):
                refs = contract.get("error_cases", {}).get(error_name, [])
                if not refs or any(
                    s not in exercise or category(s) not in {"negative", "boundary"}
                    for s in refs
                ):
                    issue(
                        "SCENARIO_CATEGORY_MISSING",
                        f"Error {error_name} needs negative/boundary scenario.",
                        ident,
                    )
        elif kind == "entity" and "fields" not in node:
            field_issue(
                "READINESS_CONTRACT_MISSING",
                "Entity fields are missing; specify the field/type map.",
                ident, "fields",
            )
        elif kind == "machine" and "machine" not in node:
            field_issue(
                "READINESS_CONTRACT_MISSING",
                "Machine definition is missing; specify states and transitions.",
                ident, "machine",
            )
        elif kind == "component":
            if "interfaces" not in node:
                field_issue("READINESS_CONTRACT_MISSING", "Component interfaces are missing; list public action IDs or explicitly use [].", ident, "interfaces")
            if bundle.model["format"] == "gdd/0.3":
                boundary = node.get("boundary", {})
                missing = [field for field in ("responsibility", "rationale", "excludes")
                           if field not in boundary or
                           (field != "excludes" and not boundary[field].strip())]
                if missing:
                    field_issue("BOUNDARY_RATIONALE_MISSING",
                                "Component boundary needs fields: " + ", ".join(missing) +
                                "; responsibility and rationale must be nonblank; excludes may be [].",
                                ident, "boundary", missing[0])
                invalid = sorted(
                    action for action in node.get("interfaces", [])
                    if owned.get(action) != ident
                )
                if invalid:
                    field_issue(
                        "READINESS_CONTRACT_MISSING",
                        "Public interface actions need exactly one owned_by edge to this component: "
                        + ", ".join(invalid) + ".",
                        ident, "interfaces",
                    )
        elif kind == "screen" and ident in selected:
            interaction = node.get("interaction", {})
            if any(
                not interaction.get(k)
                for k in ("states", "behavior", "accessibility", "responsive")
            ):
                issue(
                    "READINESS_CONTRACT_MISSING",
                    "Screen interaction contract is incomplete.",
                    ident,
                )
            if not node.get("design_refs"):
                issue(
                    "READINESS_CONTRACT_MISSING",
                    "Screen requires resolved design_refs.",
                    ident,
                )
            if not any(category(s) == "normal" for s in scenarios_for(ident)):
                issue(
                    "SCENARIO_CATEGORY_MISSING", "Screen needs normal scenario.", ident
                )
            superseded = {
                r["supersedes"]
                for rid, r in bundle.records.items()
                if rid in host.get("trusted_decisions", [])
                and rid not in invalid_records
                and r.get("supersedes") not in invalid_records
                and compatible_decision_supersession(
                    r, bundle.records.get(r.get("supersedes"), {})
                )
            }
            reviews = [
                (rid, r)
                for rid, r in bundle.records.items()
                if rid not in superseded
                and rid not in invalid_records
                and r["kind"] == "decision"
                and r["topic"] == "readiness_review"
                and (not r["scope"] or ident in r["scope"])
                and r["outcome"] == "accepted"
                and r["spec_revision"] == bundle.revision
            ]
            if not any(rid in host.get("trusted_decisions", []) for rid, _ in reviews):
                issue(
                    "RECORD_INVALID",
                    "Screen requires current trusted readiness_review decision.",
                    ident,
                    "UNKNOWN",
                )
        elif kind not in CORE_KINDS:
            issue(
                "EXTENSION_UNSUPPORTED",
                "Custom kinds have no core implementation-readiness contract.",
                ident,
                "UNKNOWN",
            )
        rule = node.get("rule", {})
        if kind == "rule":
            if not rule:
                issue(
                    "READINESS_CONTRACT_MISSING",
                    "Applicable rule is unspecified.",
                    ident,
                )
            elif rule["blocking"] and not any(
                nodes.get(s, {}).get("scenario")
                for s in scenarios_for(ident, "verifies")
            ):
                issue(
                    "SCENARIO_CATEGORY_MISSING",
                    "Blocking rule lacks planned verifying scenario.",
                    ident,
                )
        for ref in node.get("source_refs", []):
            if ref["purpose"] == "contract":
                contract_source(ref, ident)
        for ref in node.get("design_refs", []):
            if ref["purpose"] != "contract":
                issue(
                    "READINESS_CONTRACT_MISSING",
                    "Design references must have purpose=contract.",
                    ident,
                )
            contract_source(ref, ident)
    if bundle.model["format"] == "gdd/0.3":
        for interaction in call_boundaries(bundle.model):
            if interaction["source"] in closure and interaction["action"] in closure and interaction["status"] == "unresolved":
                issue("PUBLIC_CONTRACT_UNRESOLVED", "Call needs unique consumer/provider owners and a specified public interface.", interaction["action"])
    for ident, question in bundle.model.get("questions", {}).items():
        if (
            not set(question["targets"]) & closure
            or "implementation" not in question["blocks"]
        ):
            continue
        if question["status"] == "open":
            issue(
                "QUESTION_BLOCKS_SCOPE",
                "Open question blocks implementation context.",
                ident,
            )
        else:
            receipt_id = question["resolution"]["decision"]
            receipt = bundle.records.get(receipt_id, {})
            superseded = {
                r["supersedes"]
                for rid, r in bundle.records.items()
                if rid in host.get("trusted_decisions", [])
                and rid not in invalid_records
                and r.get("supersedes") not in invalid_records
                and compatible_decision_supersession(
                    r, bundle.records.get(r.get("supersedes"), {})
                )
            }
            if not (
                receipt_id not in superseded
                and receipt_id not in invalid_records
                and receipt.get("kind") == "decision"
                and receipt.get("outcome") == "accepted"
                and receipt.get("spec_revision") == bundle.revision
                and (
                    not receipt.get("scope")
                    or set(question["targets"]) <= set(receipt["scope"])
                )
                and receipt_id in host.get("trusted_decisions", [])
            ):
                issue(
                    "QUESTION_BLOCKS_SCOPE",
                    "Resolved question has no applicable trusted current decision.",
                    ident,
                    "UNKNOWN",
                )
    for edge in edges:
        if edge["from"] in closure and edge["to"] in closure:
            for ref in edge.get("source_refs", []):
                if ref["purpose"] == "contract":
                    contract_source(ref, edge["id"])
    if len(bundle.checks) == start:
        bundle.checks.append(
            check(
                "readiness.structural",
                "readiness",
                "PASS",
                "Selected targets and required boundary contracts are structurally specified; no behavioral proof claimed.",
                sorted(selected),
            )
        )
