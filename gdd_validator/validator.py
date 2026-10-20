"""Public API and profile orchestration."""

from pathlib import Path
import re

from jsonschema import Draft202012Validator

from . import __version__
from .graphify_io import verify_graphify
from .loading import (
    InputError,
    canonical,
    load_bundle,
    resolve_asset,
    resolve_local_path,
    schema_checks,
)
from .model import validate_model
from .planning import validate_package, validate_parallel
from .readiness import validate_readiness
from .records import authorize, evaluate_implementation, validate_records
from .report import check, diagnostic
from .visualization import verify_view

__all__ = ["validate", "input_schema"]
PROFILES = {
    "representation",
    "model",
    "readiness",
    "records",
    "import.graphify",
    "visualization",
}
STRING = {"type": "string", "minLength": 1}
STRINGS = {"type": "array", "items": STRING, "uniqueItems": True}


def object_schema(properties: dict, required: list[str] | None = None) -> dict:
    return {
        "type": "object",
        "properties": properties,
        "required": required if required is not None else list(properties),
        "additionalProperties": False,
    }


def input_schema(kind: str) -> dict:
    """Tool envelopes are local API contracts, separate from the GDD core standard."""
    policy = object_schema(
        {
            "revision": STRING,
            "actor": STRING,
            "activity": {"enum": ["model_changes", "implementation", "acceptance"]},
            "spec_revision": STRING,
            "scope": STRINGS,
            "requirement": {"enum": ["explicit", "standing", "none"]},
            "authorities": STRINGS,
            "basis": STRING,
            "denied": {"type": "boolean"},
        },
        [
            "revision",
            "actor",
            "activity",
            "spec_revision",
            "scope",
            "requirement",
            "authorities",
            "basis",
        ],
    )
    if kind == "host":
        return object_schema(
            {
                "implementation": STRING,
                "code_revision": STRING,
                "policy": policy,
                "trusted_decisions": STRINGS,
                "supported_methods": STRINGS,
                "trusted_evidence": STRINGS,
                "verified_bindings": STRINGS,
                "environment": {"type": "object", "additionalProperties": STRING},
                "negative_controls": STRINGS,
                "check_selection": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "object",
                        "additionalProperties": STRING,
                    },
                },
                "evidence_selection": {
                    "type": "object",
                    "additionalProperties": STRING,
                },
            },
            [],
        )
    if kind == "package":
        return object_schema(
            {
                "format": {"const": "gdd-context/0.2"},
                "spec_revision": STRING,
                "scope": STRINGS,
                "nodes": STRINGS,
                "edges": STRINGS,
                "sources": STRINGS,
                "questions": STRINGS,
                "extensions": STRINGS,
            }
        )
    if kind == "parallel":
        package = object_schema(
            {
                "id": STRING,
                "scope": {**STRINGS, "minItems": 1},
                "write_resources": {**STRINGS, "minItems": 1},
                "read_resources": STRINGS,
                "locked_contracts": STRINGS,
                "workspace": STRING,
                "base_revision": STRING,
                "integration_plan": STRING,
            },
            ["id", "scope", "write_resources", "base_revision", "integration_plan"],
        )
        return object_schema(
            {
                "format": {"const": "gdd-parallel-plan/0.2"},
                "packages": {"type": "array", "items": package, "minItems": 2},
                "serialized": {
                    "type": "array",
                    "items": {
                        "type": "array",
                        "items": STRING,
                        "minItems": 2,
                        "maxItems": 2,
                    },
                },
                "integration": object_schema({"owner": STRING, "checks": STRINGS}),
            },
            ["format", "packages", "serialized"],
        )
    if kind == "import":
        return object_schema(
            {
                "format": {"const": "gdd-import-audit/0.2"},
                "original": {},
                "reconstructed": {},
            }
        )
    raise ValueError(f"Unknown tool input kind: {kind}")


def validate(
    path: str | Path,
    *,
    profiles: list[str] | None = None,
    scope: list[str] | None = None,
    model_path: str | Path | None = None,
    host_context: dict | None = None,
    activity: str | None = None,
    package: dict | None = None,
    parallel_plan: dict | None = None,
    import_audit: dict | None = None,
    graphify_path: str | Path | None = None,
    graphify_profile: str | None = None,
    graphify_producer_version: str | None = None,
    view_path: str | Path | None = None,
    unsupported_extensions: frozenset[str] = frozenset(),
) -> dict:
    if profiles is not None and (
        not isinstance(profiles, list)
        or any(not isinstance(profile, str) for profile in profiles)
    ):
        raise ValueError("Profiles must be a list of strings.")
    if scope is not None and (
        not isinstance(scope, list) or any(not isinstance(node, str) for node in scope)
    ):
        raise ValueError("Scope must be a list of strings.")
    profiles = list(dict.fromkeys(profiles or ["model"]))
    scope = list(dict.fromkeys(scope or []))
    if not set(profiles) <= PROFILES:
        raise ValueError("Unknown requested profile.")
    if activity is not None and (
        not isinstance(activity, str)
        or activity not in {"model_changes", "implementation", "acceptance"}
    ):
        raise ValueError("Unknown activity.")
    if activity and "records" not in profiles:
        profiles.append("records")
    host = host_context if host_context is not None else {}
    supplied = {
        "host": host,
        "package": package,
        "parallel": parallel_plan,
        "import": import_audit,
    }
    for kind, value in supplied.items():
        if value is not None:
            errors = list(Draft202012Validator(input_schema(kind)).iter_errors(value))
            if errors:
                raise ValueError(
                    f"Invalid {kind} input: " + "; ".join(e.message for e in errors)
                )
    if package is not None and parallel_plan is not None:
        raise ValueError(
            "Context package and parallel plan cannot be supplied together."
        )
    envelope_scope = None
    if package is not None:
        envelope_scope = package["scope"]
    elif parallel_plan is not None:
        envelope_scope = list(
            dict.fromkeys(n for p in parallel_plan["packages"] for n in p["scope"])
        )
    if envelope_scope is not None:
        # The CLI and API use [] for an omitted top-level scope. Envelopes then
        # supply the one scope used for readiness, records and the report.
        if scope and set(scope) != set(envelope_scope):
            raise ValueError(
                "Requested scope conflicts with the supplied envelope scope."
            )
        scope = list(envelope_scope)
    if any(not re.fullmatch(r"[a-z][a-z0-9_]*(?:[.-][a-z0-9_]+)*", n) for n in scope):
        raise ValueError("Scope entries must be GDD IDs.")
    if profiles == ["import.graphify"] and graphify_path is None and model_path is None:
        report = verify_graphify(
            path, profile=graphify_profile, producer_version=graphify_producer_version
        )
        if import_audit and canonical(import_audit["original"]) != canonical(
            import_audit["reconstructed"]
        ):
            report["checks"].append(
                check(
                    "integration.loss",
                    "integration",
                    "FAIL",
                    "Raw reconstruction differs.",
                    diagnostics=[
                        diagnostic(
                            "IMPORT_LOSS_DETECTED",
                            "Raw original cannot be reconstructed exactly.",
                        )
                    ],
                )
            )
        return report
    bundle = load_bundle(Path(model_path or path), unsupported_extensions)
    declared_record_paths = set()
    for relative in bundle.model.get("record_files", []):
        try:
            declared_record_paths.add(resolve_asset(bundle.root, relative))
        except (InputError, OSError, RuntimeError):
            # The loader has already diagnosed invalid declared record assets.
            # Do not re-raise while checking whether a secondary input is loaded.
            continue
    secondary_path = None
    if model_path is not None:
        try:
            secondary_path = resolve_local_path(Path(path))
        except (InputError, OSError) as error:
            bundle.issue(
                error.code if isinstance(error, InputError) else "SOURCE_UNAVAILABLE",
                f"Secondary record input cannot be resolved: {error}.",
                str(path),
                result="FAIL" if isinstance(error, InputError) else "UNKNOWN",
            )
    if secondary_path is not None and secondary_path not in declared_record_paths:
        records_bundle = load_bundle(secondary_path)
        bundle.checks.extend(records_bundle.checks)
        if records_bundle.model:
            if records_bundle.model.get("format") != "gdd-records/0.2":
                bundle.issue(
                    "SCHEMA_INVALID", "Secondary input must be gdd-records/0.2."
                )
            elif (
                bundle.model.get("format") in {"gdd/0.2", "gdd/0.3"}
                and records_bundle.model["project_id"] != bundle.model["project"]["id"]
            ):
                bundle.issue(
                    "RECORD_INVALID", "Secondary records belong to another project."
                )
        for ident, record in records_bundle.records.items():
            if ident in bundle.records:
                bundle.issue("ID_COLLISION", f"Record loaded twice: {ident}.")
            else:
                bundle.records[ident] = record
    known_model = bundle.model.get("format") in {"gdd/0.2", "gdd/0.3"}
    representation_valid = not any(
        c["dimension"] == "model" and c["result"] != "PASS" for c in bundle.checks
    )
    if profiles != ["representation"]:
        if known_model and representation_valid:
            validate_model(bundle)
            if "records" in profiles or bundle.records:
                validate_records(bundle)
            if "readiness" in profiles:
                if package is not None:
                    validate_package(bundle, package)
                    validate_readiness(bundle, scope, host)
                elif parallel_plan is not None:
                    validate_parallel(bundle, parallel_plan)
                else:
                    validate_readiness(bundle, scope, host)
            if "records" in profiles:
                if activity:
                    authorize(bundle, activity, scope, host)
                else:
                    evaluate_implementation(bundle, scope, host)
        else:
            if not known_model:
                bundle.checks.append(
                    check(
                        "model.unavailable",
                        "model",
                        "UNKNOWN",
                        "Semantic model validation requires a supported GDD 0.2 or 0.3 model manifest; schema validation alone is representation-only.",
                        scope,
                        [
                            diagnostic(
                                "FORMAT_UNSUPPORTED",
                                "No model semantics for this artifact without its model manifest.",
                                result="UNKNOWN",
                            )
                        ],
                    )
                )
            for profile, dimension in (
                ("readiness", "readiness"),
                ("records", "implementation"),
            ):
                if profile in profiles:
                    bundle.checks.append(
                        check(
                            profile + ".unavailable",
                            dimension,
                            "UNKNOWN",
                            "A valid supported GDD model is required; no semantic PASS inferred.",
                            scope,
                            [
                                diagnostic(
                                    "REVISION_UNRESOLVED",
                                    "Model unavailable or representation invalid.",
                                    result="UNKNOWN",
                                )
                            ],
                        )
                    )
            if activity:
                bundle.checks.append(
                    check(
                        "authorization.unavailable",
                        "authorization",
                        "UNRESOLVED",
                        "Valid model and revision unavailable.",
                        scope,
                        [
                            diagnostic(
                                "AUTHORIZATION_UNRESOLVED",
                                "Authorization input incomplete.",
                                result="UNKNOWN",
                            )
                        ],
                        activity=activity,
                    )
                )
    if "import.graphify" in profiles:
        if import_audit and canonical(import_audit["original"]) != canonical(
            import_audit["reconstructed"]
        ):
            bundle.checks.append(
                check(
                    "integration.loss",
                    "integration",
                    "FAIL",
                    "Reconstructed original differs, including order, parallel records, direction or metadata.",
                    scope,
                    [
                        diagnostic(
                            "IMPORT_LOSS_DETECTED",
                            "Raw original cannot be reconstructed exactly.",
                        )
                    ],
                )
            )
        if graphify_path is not None:
            bundle.checks.extend(
                verify_graphify(
                    graphify_path,
                    profile=graphify_profile,
                    producer_version=graphify_producer_version,
                )["checks"]
            )
        else:
            bundle.checks.append(
                check(
                    "integration.graphify",
                    "integration",
                    "UNKNOWN",
                    "Supply a Graphify observations bundle or raw artifact with an explicit profile.",
                    scope,
                    [
                        diagnostic(
                            "FORMAT_UNSUPPORTED",
                            "No Graphify input was supplied for the model integration.",
                            result="UNKNOWN",
                        )
                    ],
                )
            )
    if "visualization" in profiles:
        if known_model and not any(
            c["dimension"] == "model" and c["result"] != "PASS" for c in bundle.checks
        ):
            bundle.checks.extend(verify_view(model_path or path, view_path)["checks"])
        else:
            bundle.checks.append(
                check(
                    "integration.visualization",
                    "integration",
                    "UNKNOWN",
                    "A valid supported GDD model is required for artifact verification.",
                    diagnostics=[
                        diagnostic(
                            "REVISION_UNRESOLVED",
                            "Model unavailable.",
                            result="UNKNOWN",
                        )
                    ],
                )
            )
    report = {
        "format": "gdd-report/0.2",
        "tool": {"name": "gdd-validator", "version": __version__},
        "spec_revision": bundle.revision,
        "profiles": profiles,
        "scope": scope,
        "checks": bundle.checks,
    }
    errors = [c for c in schema_checks(report, "<report>") if c["result"] != "PASS"]
    if errors:
        raise RuntimeError("Validator generated a nonconforming report: " + str(errors))
    return report
