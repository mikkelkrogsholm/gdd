"""Safe YAML/JSON loading, local schemas and byte snapshots."""

from dataclasses import dataclass, field
from decimal import Decimal, InvalidOperation
from hashlib import sha256
import json
import math
from pathlib import Path, PurePosixPath
import re
from typing import Any

import yaml
from yaml.nodes import MappingNode, ScalarNode, SequenceNode
from jsonschema import Draft202012Validator, FormatChecker

from .numeric import exact_numbers
from .report import check, diagnostic, pointer

__all__ = [
    "Bundle",
    "InputError",
    "parse",
    "parse_json",
    "schema_checks",
    "load_bundle",
    "digest",
    "canonical",
    "resolve_asset",
    "schema",
]

FORMATS = {
    "gdd/0.2": "gdd",
    "gdd-fragment/0.2": "fragment",
    "gdd-extension/0.2": "extension",
    "gdd-records/0.2": "records",
    "gdd-report/0.2": "report",
    "gdd-snapshot/0.2": "snapshot",
}
NUMBER = re.compile(r"-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?\Z")


class InputError(ValueError):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


def canonical(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        ensure_ascii=False,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def digest(value: Any) -> str:
    return "sha256:" + sha256(canonical(value)).hexdigest()


def _integer(raw: str) -> int:
    try:
        return int(raw)
    except ValueError as error:
        raise InputError(
            "FORMAT_UNSUPPORTED",
            "JSON integer exceeds this runtime's conversion capacity.",
        ) from error


def _number(raw: str) -> float:
    """Keep builtin floats only when their decimal round trip retains the input.

    Decimal is used transiently for comparison, never stored in parsed data.
    Finite JSON lexemes outside this consumer's range/precision are unresolved,
    rather than being silently rounded into different constraints or identities.
    """
    parsed = float(raw)
    try:
        precise = math.isfinite(parsed) and Decimal(raw) == Decimal(str(parsed))
    except InvalidOperation as error:
        raise InputError(
            "FORMAT_UNSUPPORTED",
            "JSON number exceeds this runtime's decimal exponent capacity.",
        ) from error
    if not precise:
        raise InputError(
            "FORMAT_UNSUPPORTED",
            "JSON number exceeds this consumer's numeric range or decimal precision.",
        )
    return parsed


def parse_json(raw: bytes | str) -> Any:
    """Load the strict JSON profile; syntax errors never fall back to YAML."""
    try:
        text = raw.decode("utf-8") if isinstance(raw, bytes) else raw

        def json_pairs(pairs: list) -> dict:
            result = {}
            for key, val in pairs:
                if key == "<<":
                    raise InputError("YAML_FEATURE_FORBIDDEN", "Merge keys forbidden.")
                if key in result:
                    raise InputError(
                        "YAML_DUPLICATE_KEY", f"Duplicate mapping key: {key!r}"
                    )
                result[key] = val
            return result

        decoded = json.loads(
            text, object_pairs_hook=json_pairs, parse_int=_integer, parse_float=_number
        )
        pending = [decoded]
        while pending:
            current = pending.pop()
            if isinstance(current, str) and any(
                0xD800 <= ord(c) <= 0xDFFF for c in current
            ):
                raise InputError("YAML_FEATURE_FORBIDDEN", "Invalid Unicode scalar.")
            if isinstance(current, float) and not math.isfinite(current):
                raise InputError("YAML_FEATURE_FORBIDDEN", "Non-finite number.")
            if isinstance(current, dict):
                pending.extend(current.keys())
                pending.extend(current.values())
            elif isinstance(current, list):
                pending.extend(current)
        return decoded
    except json.JSONDecodeError:
        raise
    except RecursionError as error:
        raise InputError(
            "FORMAT_UNSUPPORTED",
            "JSON parsing exceeded this runtime's nesting capacity; input validity is unresolved.",
        ) from error
    except (UnicodeError, ValueError) as error:
        if isinstance(error, InputError):
            raise
        raise InputError("YAML_FEATURE_FORBIDDEN", str(error)) from error


def parse(raw: bytes | str) -> Any:
    """YAML 1.2 JSON scalar resolution, without constructors or alias expansion."""
    try:
        text = raw.decode("utf-8") if isinstance(raw, bytes) else raw

        # JSON strings keep literal U+2028/U+2029; YAML may fold them.
        try:
            return parse_json(text)
        except json.JSONDecodeError:
            pass
        explicit_tags = set()
        builtin_tags = {
            "tag:yaml.org,2002:" + name
            for name in ("str", "int", "float", "bool", "null", "seq", "map")
        }
        for token in yaml.scan(text):
            if isinstance(token, (yaml.tokens.AnchorToken, yaml.tokens.AliasToken)):
                raise InputError(
                    "YAML_FEATURE_FORBIDDEN",
                    "Anchors and aliases are forbidden.",
                )
            if isinstance(token, yaml.tokens.TagToken):
                explicit_tags.add(token.start_mark.index)
        node = yaml.compose(text, Loader=yaml.BaseLoader)

        def value(item: Any) -> Any:
            # TAG directives can rebind '!!' or map another handle to a builtin.
            # Validate the composed URI, then retain explicit-tag scalar coercion.
            if item is not None and item.tag not in builtin_tags:
                raise InputError(
                    "YAML_FEATURE_FORBIDDEN",
                    "Only JSON-compatible built-in tags are supported.",
                )
            tag = (
                item.tag.rsplit(":", 1)[-1]
                if item is not None and item.start_mark.index in explicit_tags
                else None
            )
            if isinstance(item, ScalarNode):
                s = item.value
                if any(0xD800 <= ord(c) <= 0xDFFF for c in s):
                    raise InputError(
                        "YAML_FEATURE_FORBIDDEN", "Invalid Unicode scalar."
                    )
                if tag == "str":
                    return s
                if tag in {"seq", "map"}:
                    raise InputError(
                        "YAML_FEATURE_FORBIDDEN", "Tag does not match node kind."
                    )
                if (
                    tag == "null"
                    and s not in {"null", ""}
                    or tag == "bool"
                    and s not in {"true", "false"}
                ):
                    raise InputError(
                        "YAML_FEATURE_FORBIDDEN",
                        "Tagged value is outside the JSON scalar profile.",
                    )
                if tag in {"int", "float"} and (
                    not NUMBER.fullmatch(s)
                    or tag == "int"
                    and any(c in s for c in ".eE")
                ):
                    raise InputError(
                        "YAML_FEATURE_FORBIDDEN", "Invalid tagged JSON number."
                    )
                if item.style is None or tag:
                    if s in {"null", ""}:
                        return None
                    if s in {"true", "false"}:
                        return s == "true"
                    if NUMBER.fullmatch(s):
                        return (
                            _number(s)
                            if tag == "float" or any(c in s for c in ".eE")
                            else _integer(s)
                        )
                    if s.lower() in {".nan", ".inf", "+.inf", "-.inf"}:
                        raise InputError("YAML_FEATURE_FORBIDDEN", "Non-finite number.")
                return s
            if isinstance(item, SequenceNode):
                if tag not in {None, "seq"}:
                    raise InputError("YAML_FEATURE_FORBIDDEN", "Invalid sequence tag.")
                return [value(x) for x in item.value]
            if isinstance(item, MappingNode):
                if tag not in {None, "map"}:
                    raise InputError("YAML_FEATURE_FORBIDDEN", "Invalid mapping tag.")
                result = {}
                for key, val in item.value:
                    k = value(key)
                    if not isinstance(k, str) or k == "<<":
                        raise InputError(
                            "YAML_FEATURE_FORBIDDEN",
                            "String keys required; merge keys forbidden.",
                        )
                    if k in result:
                        raise InputError(
                            "YAML_DUPLICATE_KEY", f"Duplicate mapping key: {k}"
                        )
                    result[k] = value(val)
                return result
            return None

        return value(node)
    except RecursionError as error:
        raise InputError(
            "FORMAT_UNSUPPORTED",
            "YAML parsing exceeded this runtime's nesting capacity; input validity is unresolved.",
        ) from error
    except (yaml.YAMLError, UnicodeError, ValueError) as error:
        if isinstance(error, InputError):
            raise
        raise InputError("YAML_FEATURE_FORBIDDEN", str(error)) from error


def schema(name: str) -> dict:
    directory = Path(__file__).parent / "schemas"
    if not directory.is_dir():
        directory = Path(__file__).parent.parent / "schemas"
    return json.loads((directory / f"{name}.schema.json").read_text())


def schema_checks(data: Any, file: str, expected: str | None = None) -> list[dict]:
    fmt = data.get("format") if isinstance(data, dict) else None
    if not isinstance(fmt, str):
        return [
            check(
                "representation.format",
                "model",
                "FAIL",
                "Document requires a string format identifier.",
                diagnostics=[
                    diagnostic(
                        "SCHEMA_INVALID",
                        "Missing or invalid format identifier.",
                        file,
                        "/format",
                    )
                ],
            )
        ]
    if fmt not in FORMATS:
        return [
            check(
                "representation.format",
                "model",
                "UNKNOWN",
                "Format is not supported.",
                diagnostics=[
                    diagnostic(
                        "FORMAT_UNSUPPORTED",
                        str(fmt),
                        file,
                        "/format",
                        result="UNKNOWN",
                    )
                ],
            )
        ]
    if expected and fmt != expected:
        return [
            check(
                "representation.schema",
                "model",
                "FAIL",
                f"Expected {expected}.",
                diagnostics=[
                    diagnostic(
                        "SCHEMA_INVALID",
                        f"Expected {expected}, got {fmt}.",
                        file,
                        "/format",
                    )
                ],
            )
        ]
    try:
        errors = sorted(
            Draft202012Validator(
                exact_numbers(schema(FORMATS[fmt])), format_checker=FormatChecker()
            ).iter_errors(exact_numbers(data)),
            key=lambda e: str(list(e.path)),
        )
        diagnostics = [
            diagnostic("SCHEMA_INVALID", e.message, file, pointer(*e.path))
            for e in errors
        ]
    except RecursionError:
        reason = "Local JSON Schema validation exceeded this runtime's recursion depth; input validity is unresolved."
        return [
            check(
                "representation.schema",
                "model",
                "UNKNOWN",
                reason,
                diagnostics=[
                    diagnostic("FORMAT_UNSUPPORTED", reason, file, result="UNKNOWN")
                ],
            )
        ]
    if fmt == "gdd-extension/0.2":
        from .model import CORE_KINDS, RELATIONS

        for registry in ("node_kinds", "relations"):
            if isinstance(data.get(registry), dict):
                for name in data[registry]:
                    if name in CORE_KINDS or name in RELATIONS:
                        diagnostics.append(
                            diagnostic(
                                "EXTENSION_OVERRIDE",
                                f"Core name cannot be redefined: {name}.",
                                file,
                                pointer(registry, name),
                            )
                        )
    return [
        check(
            "representation.schema",
            "model",
            "FAIL" if diagnostics else "PASS",
            "Local JSON Schema validation.",
            diagnostics=diagnostics,
        )
    ]


def resolve_asset(root: Path, relative: str) -> Path:
    p = PurePosixPath(relative)
    if (
        p.is_absolute()
        or "\\" in relative
        or not relative
        or ":" in relative
        or "\x00" in relative
    ):
        raise InputError("ASSET_PATH_INVALID", f"Invalid local asset: {relative}")
    try:
        resolved = (root / relative).resolve()
        resolved_root = root.resolve()
    except RuntimeError as error:
        raise InputError(
            "ASSET_PATH_INVALID", f"Cannot resolve local asset: {relative}: {error}"
        ) from error
    if not resolved.is_relative_to(resolved_root):
        raise InputError("ASSET_PATH_INVALID", f"Asset escapes bundle root: {relative}")
    return resolved


@dataclass(slots=True)
class Bundle:
    path: Path
    model: dict = field(default_factory=dict)
    checks: list[dict] = field(default_factory=list)
    assets: dict[str, str] = field(default_factory=dict)
    records: dict[str, dict] = field(default_factory=dict)
    extensions: dict[str, dict] = field(default_factory=dict)
    locations: dict[str, tuple[str, str]] = field(default_factory=dict)
    revision: str | None = None
    snapshot: dict | None = None

    @property
    def root(self) -> Path:
        return self.path.parent

    def issue(
        self,
        code: str,
        message: str,
        file: str | None = None,
        path: str = "",
        result: str = "FAIL",
        dimension: str = "model",
        ids: tuple[str, ...] = (),
    ) -> None:
        self.checks.append(
            check(
                "check." + code.lower(),
                dimension,
                result,
                message,
                diagnostics=[diagnostic(code, message, file, path, ids, result)],
            )
        )


def load_bundle(
    path: Path,
    unsupported_extensions: frozenset[str] = frozenset(),
) -> Bundle:
    try:
        resolved = path.resolve()
    except RuntimeError as error:
        bundle = Bundle(path.absolute())
        bundle.issue("ASSET_PATH_INVALID", str(error), str(bundle.path))
        return bundle
    bundle = Bundle(resolved)
    seen: set[Path] = set()
    snapshot_known = True

    def read(
        relative: str, expected: str | None = None, asset: bool = False
    ) -> dict | None:
        nonlocal snapshot_known
        try:
            local = resolve_asset(bundle.root, relative)
            if local in seen:
                raise InputError(
                    "ASSET_PATH_INVALID", f"Asset loaded twice: {relative}"
                )
            seen.add(local)
            raw = local.read_bytes()
            data = parse(raw)
            checks = schema_checks(data, relative, expected)
            bundle.checks.extend(checks)
            if any(c["result"] != "PASS" for c in checks):
                if asset:
                    snapshot_known = False
                return None
            if asset:
                bundle.assets[local.relative_to(bundle.root).as_posix()] = sha256(
                    raw
                ).hexdigest()
            return data
        except (InputError, OSError) as error:
            code = error.code if isinstance(error, InputError) else "SOURCE_UNAVAILABLE"
            result = (
                "FAIL"
                if isinstance(error, InputError) and code != "FORMAT_UNSUPPORTED"
                else "UNKNOWN"
            )
            bundle.issue(code, str(error), relative, result=result)
            if asset:
                snapshot_known = False
            return None

    data = read(bundle.path.name, asset=True)
    if data is None:
        return bundle
    bundle.model = data
    if data["format"] != "gdd/0.2":
        if data["format"] == "gdd-records/0.2":
            bundle.records = data["records"]
        return bundle
    for registry in ("nodes", "sources", "questions", "extensions"):
        for ident in data.get(registry, {}):
            bundle.locations[ident] = (bundle.path.name, pointer(registry, ident))
    for index, edge in enumerate(data["edges"]):
        bundle.locations[edge["id"]] = (bundle.path.name, pointer("edges", index))
    for relative in data.get("includes", []):
        fragment = read(relative, "gdd-fragment/0.2", asset=True)
        if fragment is None:
            continue
        for ident, node in fragment["nodes"].items():
            if ident in data["nodes"]:
                bundle.issue(
                    "ID_COLLISION",
                    f"Fragment overrides {ident}.",
                    relative,
                    pointer("nodes", ident),
                    ids=(ident,),
                )
            else:
                data["nodes"][ident] = node
                bundle.locations[ident] = (relative, pointer("nodes", ident))
        for index, edge in enumerate(fragment["edges"]):
            bundle.locations[edge["id"]] = (relative, pointer("edges", index))
        data["edges"].extend(fragment["edges"])
    for ident, declaration in data.get("extensions", {}).items():
        relative = declaration["path"]
        extension = read(relative, "gdd-extension/0.2", asset=True)
        if extension is None:
            bundle.issue(
                "EXTENSION_UNSUPPORTED",
                f"Unavailable extension {ident}.",
                relative,
                result="UNKNOWN",
            )
            continue
        actual = bundle.assets.get(
            resolve_asset(bundle.root, relative).relative_to(bundle.root).as_posix()
        )
        if actual != declaration["sha256"]:
            bundle.issue(
                "EXTENSION_DATA_INVALID", "Extension bytehash mismatch.", relative
            )
            snapshot_known = False
        if (extension["namespace"], extension["version"]) != (
            declaration["namespace"],
            declaration["version"],
        ):
            bundle.issue(
                "EXTENSION_IDENTITY_MISMATCH",
                "Extension namespace/version mismatch.",
                relative,
            )
        if declaration["namespace"] in unsupported_extensions:
            bundle.issue(
                "EXTENSION_UNSUPPORTED",
                "Extension explicitly unsupported by this consumer.",
                relative,
                result="UNKNOWN",
            )
        else:
            bundle.extensions[declaration["namespace"]] = extension
    for ident, source in data.get("sources", {}).items():
        if not source["normative"]:
            continue
        try:
            local = resolve_asset(bundle.root, source["uri"])
            if local in seen:
                raise InputError(
                    "ASSET_PATH_INVALID", "Duplicate normative asset path."
                )
            seen.add(local)
            raw = local.read_bytes()
            actual = sha256(raw).hexdigest()
            expected_hash = source.get("revision", {}).get("sha256")
            if actual != expected_hash:
                bundle.issue(
                    "CONTRACT_SOURCE_UNPINNED",
                    "Normative source hash mismatch.",
                    bundle.path.name,
                    pointer("sources", ident),
                )
                snapshot_known = False
            else:
                bundle.assets[local.relative_to(bundle.root).as_posix()] = actual
        except (InputError, OSError) as error:
            code = error.code if isinstance(error, InputError) else "SOURCE_UNAVAILABLE"
            result = (
                "FAIL"
                if isinstance(error, InputError) and ":" not in source["uri"]
                else "UNKNOWN"
            )
            bundle.issue(
                code if result == "FAIL" else "SOURCE_UNAVAILABLE",
                str(error),
                bundle.path.name,
                pointer("sources", ident),
                result=result,
                dimension="integration",
            )
            snapshot_known = False
    for relative in data.get("record_files", []):
        record_file = read(relative, "gdd-records/0.2")
        if record_file is None:
            continue
        if record_file["project_id"] != data["project"]["id"]:
            bundle.issue("RECORD_INVALID", "Record project mismatch.", relative)
        for ident, record in record_file["records"].items():
            if ident in bundle.records:
                bundle.issue("ID_COLLISION", f"Duplicate record {ident}.", relative)
            else:
                bundle.records[ident] = record
    if snapshot_known:
        pairs = sorted(bundle.assets.items())
        bundle.revision = digest(pairs)
        bundle.snapshot = {
            "format": "gdd-snapshot/0.2",
            "revision": bundle.revision,
            "assets": [{"path": p, "sha256": h} for p, h in pairs],
        }
    return bundle
