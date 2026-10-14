"""Lossless Graphify JSON observations; producer claims never grant authority."""

from collections import Counter, defaultdict
from decimal import Decimal, InvalidOperation
from hashlib import sha256
import json
import math
import re
from pathlib import PurePosixPath
from typing import Any

from jsonschema import Draft202012Validator

from .loading import InputError, schema
from .report import check, diagnostic, pointer

__all__ = ["PROFILES", "decode_json", "observe_graphify", "import_report"]
PROFILES = {"graphify-0.9.75-ast": "edges", "graphify-0.9.75-node-link": "links"}
MAX_RAW_DEPTH = 256
IMPORTER = {"name": "gdd-graphify", "version": "0.1.2"}
CODE_EXTENSIONS = {
    ".py",
    ".pyi",
    ".js",
    ".jsx",
    ".ts",
    ".tsx",
    ".mjs",
    ".cjs",
    ".go",
    ".rs",
    ".java",
    ".kt",
    ".kts",
    ".c",
    ".h",
    ".cpp",
    ".hpp",
    ".cs",
    ".rb",
    ".php",
    ".swift",
    ".scala",
    ".sh",
    ".bash",
    ".zsh",
    ".sql",
    ".vue",
    ".svelte",
    ".ex",
    ".exs",
    ".erl",
    ".hs",
    ".lua",
    ".r",
    ".R",
    ".dart",
    ".html",
    ".css",
    ".scss",
}
NONCODE_TYPES = {"document", "paper", "image", "rationale", "concept", "video", "audio"}


class ProjectionDepthError(InputError):
    """The runtime cannot determine a JSON projection; it is not malformed data."""

    def __init__(self, message: str):
        super().__init__("FORMAT_UNSUPPORTED", message)


def compact_observations(index: dict) -> dict:
    """Keep the byte-identity envelope without asserting producer projections."""
    return {
        "format": index["format"],
        "source_id": index["source_id"],
        "artifact": index["artifact"].copy(),
        "importer": index["importer"].copy(),
        "input_claims": index["input_claims"].copy(),
        "producer_claims": {},
        "nodes": [],
        "edges": [],
        "opaque": [],
        "binding_candidates": [],
    }


def _raw_depth_exceeded(data: Any) -> bool:
    """Root container has depth one; each object/array ancestor adds one."""
    pending = [(data, 0)]
    while pending:
        value, parent_depth = pending.pop()
        if isinstance(value, (dict, list)):
            depth = parent_depth + 1
            if depth > MAX_RAW_DEPTH:
                return True
            items = value.values() if isinstance(value, dict) else value
            pending.extend((item, depth) for item in items)
    return False


def decode_json(raw: bytes) -> Any:
    """Strict UTF-8 JSON, with duplicate keys and non-JSON numbers rejected."""

    def pairs(items: list[tuple[str, Any]]) -> dict:
        result = {}
        for key, value in items:
            if key in result:
                raise InputError("SCHEMA_INVALID", f"Duplicate JSON key: {key!r}")
            result[key] = value
        return result

    def constant(value: str) -> Any:
        raise InputError("SCHEMA_INVALID", f"Non-JSON number: {value}")

    def number(value: str) -> float:
        parsed = float(value)
        if not math.isfinite(parsed):
            raise InputError(
                "FORMAT_UNSUPPORTED",
                "JSON number exceeds this projection's numeric range; original bytes retained.",
            )
        try:
            exact = Decimal(value)
        except InvalidOperation as error:
            raise InputError(
                "FORMAT_UNSUPPORTED",
                "Numeric precision/exponent unsupported; original bytes retained.",
            ) from error
        if exact != Decimal(str(parsed)):
            raise InputError(
                "FORMAT_UNSUPPORTED",
                "JSON number requires precision beyond this projection; original bytes retained without rounded identities.",
            )
        return parsed

    def integer(value: str) -> int:
        try:
            return int(value)
        except ValueError as error:
            raise InputError(
                "FORMAT_UNSUPPORTED",
                "JSON integer exceeds this runtime's conversion limit; original bytes retained.",
            ) from error

    def values(value: Any) -> None:
        pending = [value]
        while pending:
            current = pending.pop()
            if isinstance(current, float) and not math.isfinite(current):
                raise InputError("SCHEMA_INVALID", "Non-finite JSON number.")
            if isinstance(current, str) and any(
                0xD800 <= ord(c) <= 0xDFFF for c in current
            ):
                raise InputError("SCHEMA_INVALID", "Invalid Unicode scalar.")
            if isinstance(current, dict):
                pending.extend(current.keys())
                pending.extend(current.values())
            elif isinstance(current, list):
                pending.extend(current)

    try:
        data = json.loads(
            raw.decode("utf-8-sig"),
            object_pairs_hook=pairs,
            parse_constant=constant,
            parse_float=number,
            parse_int=integer,
        )
        values(data)
        return data
    except RecursionError as error:
        raise ProjectionDepthError(
            "JSON decoding exceeded this runtime's nesting capacity; original bytes retained."
        ) from error
    except (ValueError, UnicodeError) as error:
        if isinstance(error, InputError):
            raise
        raise InputError("SCHEMA_INVALID", str(error)) from error


def _id_key(value: Any) -> tuple | None:
    if isinstance(value, str) and value:
        return "string", value
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return "number", Decimal(str(value))
    return None


def _classification(node: dict) -> str:
    file_type = node.get("file_type")
    if isinstance(file_type, str) and file_type in NONCODE_TYPES:
        return "noncode"
    resource = node.get("source_file")
    if (
        file_type == "code"
        and isinstance(resource, str)
        and PurePosixPath(resource).suffix in CODE_EXTENSIONS
    ):
        return "code_candidate"
    return "unknown"


def import_report(checks: list[dict]) -> dict:
    return {
        "format": "gdd-report/0.2",
        "tool": IMPORTER.copy(),
        "spec_revision": None,
        "profiles": ["import.graphify"],
        "scope": [],
        "checks": checks,
    }


def observe_graphify(
    raw: bytes,
    *,
    profile: str | None,
    producer_version: str | None = None,
    code_revision: str | None = None,
    source_id: str = "source.graphify",
    locator: str = "",
) -> tuple[dict, dict]:
    """Index one supplied snapshot. JSON Pointer + artifact revision is identity.

    AST order asserts direction under the selected extractor contract. For a
    node-link export, directed=false is storage metadata: keep endpoint order,
    but don't claim recovered direction without original _src/_tgt markers.
    """
    if not isinstance(source_id, str) or not re.fullmatch(
        r"[a-z][a-z0-9_]*(?:[.-][a-z0-9_]+)*", source_id
    ):
        raise ValueError("source_id must be a GDD identifier.")
    if any(
        v is not None and not isinstance(v, str)
        for v in (profile, producer_version, code_revision)
    ) or not isinstance(locator, str):
        raise ValueError(
            "Profile, version, revision and locator must be strings when supplied."
        )
    revision = "sha256:" + sha256(raw).hexdigest()
    index = {
        "format": "gdd-observations/0.2",
        "source_id": source_id,
        "artifact": {
            "path": "graphify.raw.json",
            "revision": revision,
            "byte_count": len(raw),
        },
        "importer": {**IMPORTER, "profile": profile},
        "input_claims": {
            "locator": locator,
            "producer_version": producer_version,
            "code_revision": code_revision,
            "trust": "unverified",
        },
        "producer_claims": {},
        "nodes": [],
        "edges": [],
        "opaque": [],
        "binding_candidates": [],
    }
    checks = [
        check(
            "graphify.raw",
            "integration",
            "PASS",
            f"Supplied artifact {revision}: original bytes hashed; producer completeness and code correctness are not inferred.",
        )
    ]

    def result(
        name: str, status: str, reason: str, code: str | None = None, path: str = ""
    ) -> None:
        diagnostics = (
            [diagnostic(code, reason, "graphify.raw.json", path, result=status)]
            if code
            else []
        )
        checks.append(
            check(
                "graphify." + name,
                "integration",
                status,
                reason,
                diagnostics=diagnostics,
            )
        )

    if profile is None:
        result(
            "layout",
            "UNKNOWN",
            "No Graphify interpretation profile supplied; input bytes retained without guessing their format.",
            "FORMAT_UNSUPPORTED",
        )
        return index, import_report(checks)
    try:
        data = decode_json(raw)
    except InputError as error:
        result(
            "depth" if isinstance(error, ProjectionDepthError) else "json",
            "UNKNOWN" if error.code == "FORMAT_UNSUPPORTED" else "FAIL",
            str(error),
            error.code,
        )
        return index, import_report(checks)
    if not isinstance(data, dict):
        result(
            "layout",
            "FAIL",
            "Graphify artifact must be a JSON object.",
            "SCHEMA_INVALID",
        )
        return index, import_report(checks)
    if _raw_depth_exceeded(data):
        result(
            "depth",
            "UNKNOWN",
            f"Raw JSON container depth exceeds {MAX_RAW_DEPTH}; no producer projection is asserted. Original bytes retained.",
            "FORMAT_UNSUPPORTED",
        )
        return index, import_report(checks)
    # No observed producer version is invented from a layout match.
    claim_fields = (
        "graphify_version",
        "producer_version",
        "schema_version",
        "format",
        "built_at_commit",
        "code_revision",
    )
    for container, prefix in ((data, ""), (data.get("graph"), "/graph")):
        if isinstance(container, dict):
            for field in claim_fields:
                if field in container:
                    index["producer_claims"][prefix + "/" + field] = container[field]
    if profile not in PROFILES:
        result(
            "layout",
            "UNKNOWN",
            "Select an explicitly supported Graphify layout profile.",
            "FORMAT_UNSUPPORTED",
        )
        return index, import_report(checks)
    declared_versions = [
        v
        for p, v in index["producer_claims"].items()
        if p.rsplit("/", 1)[-1] in {"graphify_version", "producer_version"}
    ]
    if producer_version is not None:
        declared_versions.append(producer_version)
    if any(v != "0.9.75" for v in declared_versions) or any(
        p.rsplit("/", 1)[-1] in {"schema_version", "format"}
        for p in index["producer_claims"]
    ):
        result(
            "layout",
            "UNKNOWN",
            "Declared producer/schema version or format is outside the inspected profiles.",
            "FORMAT_UNSUPPORTED",
        )
        return index, import_report(checks)
    edge_key = PROFILES[profile]
    if "edges" in data and "links" in data:
        result(
            "layout",
            "UNKNOWN",
            "Both edges and links are present; no collection is silently discarded.",
            "FORMAT_UNSUPPORTED",
        )
        return index, import_report(checks)
    errors = []
    if not isinstance(data.get("nodes"), list) or not isinstance(
        data.get(edge_key), list
    ):
        errors.append(
            diagnostic(
                "SCHEMA_INVALID",
                f"Profile requires nodes and {edge_key} arrays.",
                "graphify.raw.json",
            )
        )
    if profile.endswith("node-link"):
        if any(
            not isinstance(data.get(k), bool) for k in ("directed", "multigraph")
        ) or not isinstance(data.get("graph"), dict):
            errors.append(
                diagnostic(
                    "SCHEMA_INVALID",
                    "Node-link profile requires boolean directed/multigraph and object graph.",
                    "graphify.raw.json",
                )
            )
    elif "directed" in data and not isinstance(data["directed"], bool):
        errors.append(
            diagnostic(
                "SCHEMA_INVALID",
                "directed must be boolean when supplied.",
                "graphify.raw.json",
                "/directed",
            )
        )
    if errors:
        checks.append(
            check(
                "graphify.layout",
                "integration",
                "FAIL",
                "Invalid selected Graphify layout.",
                diagnostics=errors,
            )
        )
        return index, import_report(checks)
    result(
        "layout",
        "PASS",
        f"Explicit profile {profile} matches the supplied collection layout; unknown fields preserved in original bytes.",
    )

    def record(kind: str, path: str, value: Any) -> dict:
        identifier = (
            "observation."
            + kind
            + ".h"
            + sha256((revision + path).encode()).hexdigest()
        )
        return {
            "id": identifier,
            "identity": {"artifact_revision": revision, "pointer": path},
            "raw": value,
        }

    identities = defaultdict(list)
    node_errors = []
    for number, node in enumerate(data["nodes"]):
        path = pointer("nodes", number)
        item = record("node", path, node)
        item.update(
            producer_id=node.get("id") if isinstance(node, dict) else None,
            classification=(
                _classification(node) if isinstance(node, dict) else "unknown"
            ),
        )
        index["nodes"].append(item)
        if profile.endswith("ast") and isinstance(node, dict):
            if any(
                not isinstance(node.get(k), str) or not node[k]
                for k in ("label", "file_type", "source_file")
            ) or node.get("file_type") not in {
                "code",
                "document",
                "paper",
                "image",
                "rationale",
                "concept",
            }:
                node_errors.append(
                    diagnostic(
                        "SCHEMA_INVALID",
                        "AST extraction node requires label, source_file and a known file_type.",
                        "graphify.raw.json",
                        path,
                    )
                )
        if not isinstance(node, dict) or _id_key(node.get("id")) is None:
            node_errors.append(
                diagnostic(
                    "SCHEMA_INVALID",
                    "Node needs a nonempty string or finite numeric id; booleans/null/objects are not identities.",
                    "graphify.raw.json",
                    path,
                )
            )
            continue
        identities[_id_key(node["id"])].append(item["id"])
        if isinstance(node.get("source_file"), str) and node["source_file"]:
            index["binding_candidates"].append(
                {
                    "observation": item["id"],
                    "resource_claim": node["source_file"],
                    "selector_claim": node.get("source_location"),
                    "status": "unverified",
                    "classification": item["classification"],
                    "code_revision_claims": {
                        "caller": code_revision,
                        "producer": {
                            p: v
                            for p, v in index["producer_claims"].items()
                            if p.rsplit("/", 1)[-1]
                            in {"built_at_commit", "code_revision"}
                        },
                    },
                }
            )
        for field in ("community", "community_name"):
            if field in node:
                index["opaque"].append(
                    record("community", path + "/" + field, node[field])
                )
    for key, found in identities.items():
        if len(found) > 1:
            node_errors.append(
                diagnostic(
                    "ID_COLLISION",
                    f"Duplicate producer node identity {key!r}; no first/last-wins resolution.",
                    "graphify.raw.json",
                    "/nodes",
                )
            )
    checks.append(
        check(
            "graphify.nodes",
            "integration",
            "FAIL" if node_errors else "PASS",
            "Each supplied node retained; typed producer identities checked.",
            diagnostics=node_errors,
        )
    )

    edge_errors, direction_unknown = [], []
    for number, edge in enumerate(data[edge_key]):
        path = pointer(edge_key, number)
        item = record("edge", path, edge)
        item.update(
            producer_id=edge.get("id") if isinstance(edge, dict) else None,
            source_observation=None,
            target_observation=None,
            direction={"value": "unknown", "basis": None},
        )
        index["edges"].append(item)
        if profile.endswith("ast") and isinstance(edge, dict):
            if (
                not isinstance(edge.get("source_file"), str)
                or not edge["source_file"]
                or not isinstance(edge.get("confidence"), str)
                or edge["confidence"] not in {"EXTRACTED", "INFERRED", "AMBIGUOUS"}
            ):
                edge_errors.append(
                    diagnostic(
                        "SCHEMA_INVALID",
                        "AST extraction edge requires source_file and a known confidence label.",
                        "graphify.raw.json",
                        path,
                    )
                )
        if (
            not isinstance(edge, dict)
            or not isinstance(edge.get("relation"), str)
            or not edge["relation"]
        ):
            edge_errors.append(
                diagnostic(
                    "SCHEMA_INVALID",
                    "Edge needs an object with a nonempty relation label.",
                    "graphify.raw.json",
                    path,
                )
            )
            continue
        source, target = edge.get("source"), edge.get("target")
        markers = "_src" in edge or "_tgt" in edge
        if markers:
            marker_keys = [_id_key(edge.get(k)) for k in ("_src", "_tgt")]
            endpoints = [_id_key(source), _id_key(target)]
            if (
                None in marker_keys
                or None in endpoints
                or Counter(marker_keys) != Counter(endpoints)
            ):
                edge_errors.append(
                    diagnostic(
                        "SCHEMA_INVALID",
                        "Direction markers must be complete and refer to the stored endpoints.",
                        "graphify.raw.json",
                        path,
                    )
                )
                continue
            source, target = edge["_src"], edge["_tgt"]
            if data.get("directed") is True and (_id_key(source), _id_key(target)) != (
                _id_key(edge.get("source")),
                _id_key(edge.get("target")),
            ):
                edge_errors.append(
                    diagnostic(
                        "SCHEMA_INVALID",
                        "Direction markers contradict the explicitly directed stored endpoints.",
                        "graphify.raw.json",
                        path,
                    )
                )
                continue
            item["direction"] = {"value": "source_to_target", "basis": path + "/_src"}
        elif data.get("directed") is True:
            item["direction"] = {"value": "source_to_target", "basis": "/directed"}
        elif profile.endswith("ast") and "directed" not in data:
            item["direction"] = {
                "value": "source_to_target",
                "basis": "profile:graphify-0.9.75-ast",
            }
        else:
            direction_unknown.append(
                diagnostic(
                    "GRAPHIFY_DIRECTION_UNKNOWN",
                    "Stored endpoint order is preserved, but original direction is not established by this export.",
                    "graphify.raw.json",
                    path,
                    result="UNKNOWN",
                )
            )
        for side, endpoint in (("source", source), ("target", target)):
            key = _id_key(endpoint)
            matches = identities.get(key, []) if key is not None else []
            if len(matches) != 1:
                edge_errors.append(
                    diagnostic(
                        "REFERENCE_UNRESOLVED",
                        f"{side} endpoint is missing, invalid or ambiguous.",
                        "graphify.raw.json",
                        path + "/" + side,
                    )
                )
            else:
                item[side + "_observation"] = matches[0]
    checks.append(
        check(
            "graphify.edges",
            "integration",
            "FAIL" if edge_errors else "PASS",
            "Every edge indexed independently, including identical records and duplicate producer edge IDs.",
            diagnostics=edge_errors,
        )
    )
    checks.append(
        check(
            "graphify.direction",
            "integration",
            (
                "UNKNOWN"
                if direction_unknown
                else "PASS" if not edge_errors else "UNKNOWN"
            ),
            "Direction is a producer/profile assertion, never proof of implementation behavior.",
            diagnostics=direction_unknown,
        )
    )
    # Opaque metadata keeps its actual JSON pointer. Never normalize hyperedges to cliques.
    for field in ("hyperedges", "communities"):
        if field in data:
            index["opaque"].append(record("opaque", pointer(field), data[field]))
        graph = data.get("graph")
        if isinstance(graph, dict) and field in graph:
            index["opaque"].append(
                record("opaque", pointer("graph", field), graph[field])
            )
    unknown_nodes = [n for n in index["nodes"] if n["classification"] == "unknown"]
    code_nodes = [n for n in index["nodes"] if n["classification"] == "code_candidate"]
    result(
        "code_candidates",
        "UNKNOWN" if unknown_nodes or not code_nodes else "PASS",
        f"{len(code_nodes)} producer-claimed code candidates; {len(index['nodes']) - len(code_nodes) - len(unknown_nodes)} noncode observations retained; {len(unknown_nodes)} unclassified. No implementation verdict is produced.",
        (
            "GRAPHIFY_INPUT_PROVENANCE_UNKNOWN"
            if unknown_nodes or not code_nodes
            else None
        ),
    )
    claims = [
        v
        for v in [code_revision]
        + [
            v
            for p, v in index["producer_claims"].items()
            if p.rsplit("/", 1)[-1] in {"built_at_commit", "code_revision"}
        ]
        if v is not None
    ]
    if claims and (
        any(not isinstance(v, str) or not v for v in claims)
        or len(set(map(repr, claims))) > 1
    ):
        result(
            "revision",
            "UNKNOWN",
            "Code revision claims conflict or are malformed; artifact revision is not code revision.",
            "REVISION_UNRESOLVED",
        )
    try:
        reloaded = decode_json(
            json.dumps(index, ensure_ascii=False, indent=2, allow_nan=False).encode(
                "utf-8"
            )
        )
        list(Draft202012Validator(schema("observations")).iter_errors(reloaded))
    except (RecursionError, ProjectionDepthError):
        result(
            "depth",
            "UNKNOWN",
            "The observations projection exceeds runtime serialization/reload capacity; original bytes retained without producer projections.",
            "FORMAT_UNSUPPORTED",
        )
        return compact_observations(index), import_report(checks)
    return index, import_report(checks)
