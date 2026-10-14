"""Deterministic, offline HTML projections of valid GDD models."""

from base64 import b64encode
from hashlib import sha256
import json
from pathlib import Path
import re
from typing import Any

from .loading import Bundle, InputError, canonical, load_bundle
from .model import validate_model
from .readiness import context_closure
from .report import check, diagnostic
from .boundaries import boundary_view

__all__ = ["render_view", "verify_view", "view_data", "html_document"]
GENERATOR = {"name": "gdd-viewer", "version": "0.2.1", "profile": "gdd-viewer-html/0.1"}
ASSETS = Path(__file__).parent / "viewer"
PAYLOAD = re.compile(
    r'<script id="gdd-data" type="application/json">(.*?)</script>', re.S
)


CAPACITY_REASON = "Viewer projection exceeded this runtime's nesting capacity."


def _json_text(value: Any, **options: Any) -> str:
    try:
        return json.dumps(value, **options)
    except RecursionError as error:
        raise InputError("FORMAT_UNSUPPORTED", CAPACITY_REASON) from error


def model_bundle(path: str | Path) -> Bundle:
    bundle = load_bundle(Path(path))
    if bundle.model.get("format") not in {"gdd/0.2", "gdd/0.3"}:
        raise ValueError("The viewer requires a supported GDD 0.2 or 0.3 model manifest.")
    if any(c["result"] != "PASS" for c in bundle.checks):
        raise ValueError("The viewer requires a resolved, valid model bundle.")
    validate_model(bundle)
    if any(c["result"] != "PASS" for c in bundle.checks) or not bundle.revision:
        raise ValueError("The viewer requires a semantically valid model and snapshot.")
    return bundle


def view_data(bundle: Bundle, scope: list[str] | None = None) -> dict:
    """Keep canonical data intact; scope bounds eligible canvas/list IDs."""
    if scope is not None and (
        not isinstance(scope, list) or any(not isinstance(node, str) for node in scope)
    ):
        raise ValueError("View scope must be a list of strings.")
    scope = sorted(set(scope or []))
    nodes = bundle.model["nodes"]
    if set(scope) - nodes.keys():
        raise ValueError("Unknown view scope.")
    shown = context_closure(bundle, scope) if scope else set(nodes)
    shown_edges = sorted(
        e["id"] for e in bundle.model["edges"] if {e["from"], e["to"]} <= shown
    )
    text = {}
    # JavaScript numbers can round large integers. Inspector text is serialized
    # here so every canonical field can be read without browser numeric loss.
    for registry in ("nodes", "sources", "questions"):
        text[registry] = {
            ident: {
                "full": _json_text(record, ensure_ascii=False, indent=2),
                "fields": {
                    key: (
                        _json_text(value, ensure_ascii=False, indent=2)
                        if not isinstance(value, str)
                        else value
                    )
                    for key, value in record.items()
                },
            }
            for ident, record in bundle.model.get(registry, {}).items()
        }
    text["edges"] = {
        e["id"]: _json_text(e, ensure_ascii=False, indent=2)
        for e in bundle.model["edges"]
    }
    text["references"] = {
        ident: [
            _json_text(ref, ensure_ascii=False, indent=2)
            for field in ("source_refs", "design_refs")
            for ref in record.get(field, [])
        ]
        for ident, record in [
            *bundle.model["nodes"].items(),
            *((edge["id"], edge) for edge in bundle.model["edges"]),
        ]
    }
    text["project"] = _json_text(bundle.model["project"], ensure_ascii=False, indent=2)
    text["snapshot"] = _json_text(bundle.snapshot, ensure_ascii=False, indent=2)
    payload = {
        "format": "gdd-view/0.2",
        "generator": dict(GENERATOR),
        "spec_revision": bundle.revision,
        "observation_inputs": [],
        "scope": scope,
        "model": bundle.model,
        "locations": {i: list(loc) for i, loc in sorted(bundle.locations.items())},
        "snapshot": bundle.snapshot,
        "text": text,
        "manifest": {
            "shown_nodes": sorted(shown),
            "shown_edges": shown_edges,
            "omitted_nodes": sorted(set(nodes) - shown),
            "omitted_edges": sorted(
                {e["id"] for e in bundle.model["edges"]} - set(shown_edges)
            ),
        },
    }
    if bundle.model["format"] == "gdd/0.3":
        payload["boundaries"] = boundary_view(bundle, sorted(shown))
    return payload


def html_document(data: dict) -> str:
    """Only trusted assets become markup. Model text stays JSON/textContent."""
    script = "\n".join(
        (ASSETS / name).read_text(encoding="utf-8")
        for name in ("graph.js", "inspector.js", "app.js")
    )
    script_hash = b64encode(sha256(script.encode("utf-8")).digest()).decode("ascii")
    payload = _json_text(data, sort_keys=True, ensure_ascii=True, allow_nan=False)
    for char, escaped in (("<", "\\u003c"), (">", "\\u003e"), ("&", "\\u0026")):
        payload = payload.replace(char, escaped)
    html = (ASSETS / "shell.html").read_text(encoding="utf-8")
    # A single replacement pass prevents model data becoming template tokens.
    values = {
        "CSS": (ASSETS / "style.css").read_text(encoding="utf-8"),
        "SCRIPT": script,
        "DATA": payload,
        "HASH": script_hash,
    }
    return re.sub(r"@@(CSS|SCRIPT|DATA|HASH)@@", lambda m: values[m[1]], html)


def report(bundle: Bundle, result: str, reason: str, code: str | None = None) -> dict:
    return {
        "format": "gdd-report/0.2",
        "tool": {"name": "gdd-viewer", "version": GENERATOR["version"]},
        "spec_revision": bundle.revision,
        "profiles": ["visualization"],
        "scope": [],
        "checks": [
            check(
                "integration.visualization",
                "integration",
                result,
                reason,
                diagnostics=[diagnostic(code, reason, result=result)] if code else [],
            )
        ],
    }


def render_view(
    path: str | Path, output: str | Path, *, scope: list[str] | None = None
) -> dict:
    """Write one portable HTML artifact without overwriting existing files."""
    bundle = model_bundle(path)
    data = view_data(bundle, scope)
    html = html_document(data)
    # Confirm the generated payload can be reloaded before publishing any bytes.
    try:
        json.loads(PAYLOAD.findall(html)[0])
    except RecursionError as error:
        raise InputError("FORMAT_UNSUPPORTED", CAPACITY_REASON) from error
    with Path(output).open("x", encoding="utf-8", newline="\n") as file:
        file.write(html)
    result = report(
        bundle, "PASS", "Generated a read-only gdd-viewer-html/0.1 projection."
    )
    result["scope"] = data["scope"]
    result["checks"][0]["scope"] = data["scope"]
    return result


def verify_view(path: str | Path, view: str | Path | None) -> dict:
    """Verify this renderer's exact artifact, not arbitrary third-party HTML."""
    bundle = model_bundle(path)
    if view is None:
        return report(
            bundle,
            "UNKNOWN",
            "Supply --view for artifact verification.",
            "VIEW_INPUT_MISSING",
        )
    try:
        html = Path(view).read_bytes().decode("utf-8")
        matches = PAYLOAD.findall(html)
        if len(matches) != 1:
            raise ValueError("Missing or ambiguous embedded view payload.")
        data = json.loads(matches[0])
        if not isinstance(data, dict) or data.get("generator") != GENERATOR:
            return report(
                bundle,
                "UNKNOWN",
                "Unsupported renderer/profile/version.",
                "FORMAT_UNSUPPORTED",
            )
        if data.get("spec_revision") != bundle.revision:
            return report(
                bundle,
                "FAIL",
                "View and current model revisions differ.",
                "VIEW_REVISION_MISMATCH",
            )
        scope = data.get("scope")
        if not isinstance(scope, list) or any(not isinstance(n, str) for n in scope):
            raise ValueError("Invalid view scope.")
        expected = view_data(bundle, scope)
        if canonical(data) != canonical(expected) or html != html_document(expected):
            raise ValueError("View payload or trusted renderer assets differ.")
    except OSError as error:
        return report(bundle, "UNKNOWN", str(error), "SOURCE_UNAVAILABLE")
    except RecursionError:
        return report(bundle, "UNKNOWN", CAPACITY_REASON, "FORMAT_UNSUPPORTED")
    except InputError as error:
        return report(
            bundle,
            "UNKNOWN" if error.code == "FORMAT_UNSUPPORTED" else "FAIL",
            str(error),
            error.code,
        )
    except (UnicodeError, ValueError, TypeError) as error:
        return report(bundle, "FAIL", str(error), "VIEW_ARTIFACT_MISMATCH")
    result = report(
        bundle,
        "PASS",
        "Exact artifact matches this renderer and current model snapshot; browser conformance is tested separately.",
    )
    result["scope"] = scope
    result["checks"][0]["scope"] = scope
    return result
