"""Schema-shaped checks and conservative aggregation."""

from typing import Any

__all__ = ["check", "diagnostic", "aggregate", "exit_code", "pointer"]


def pointer(*parts: object) -> str:
    return "".join("/" + str(p).replace("~", "~0").replace("/", "~1") for p in parts)


def diagnostic(
    code: str,
    message: str,
    file: str | None = None,
    path: str = "",
    ids: tuple[str, ...] = (),
    result: str = "FAIL",
) -> dict:
    return {
        "code": code,
        "severity": "error" if result == "FAIL" else "warning",
        "message": message,
        "location": {"file": file, "pointer": path},
        "related_ids": list(dict.fromkeys(ids)),
    }


def check(
    name: str,
    dimension: str,
    result: str,
    reason: str,
    scope: list[str] | None = None,
    diagnostics: list[dict] | None = None,
    **details: Any,
) -> dict:
    return {
        "check_id": name,
        "dimension": dimension,
        "result": result,
        "scope": scope or [],
        "reason": reason,
        "diagnostics": diagnostics or [],
        **details,
    }


RESULTS = {
    dimension: frozenset({"PASS", "FAIL", "UNKNOWN"})
    for dimension in ("model", "readiness", "implementation", "integration")
}
RESULTS["authorization"] = frozenset({"GRANTED", "DENIED", "UNRESOLVED"})


def _validate_results(checks: list[dict]) -> None:
    """Reject malformed or dimension-incompatible results before aggregation."""
    if not isinstance(checks, list):
        raise ValueError("Checks must be a list.")
    for item in checks:
        if not isinstance(item, dict):
            raise ValueError(
                "Each check must be an object with a dimension and result."
            )
        dimension, result = item.get("dimension"), item.get("result")
        if not isinstance(dimension, str) or dimension not in RESULTS:
            raise ValueError("Unknown check dimension.")
        if not isinstance(result, str) or result not in RESULTS[dimension]:
            raise ValueError(f"Invalid result for {dimension}: {result!r}.")


def aggregate(checks: list[dict], dimension: str) -> str:
    """Aggregate valid results; absent checks leave the dimension unresolved."""
    if not isinstance(dimension, str) or dimension not in RESULTS:
        raise ValueError("Unknown aggregate dimension.")
    _validate_results(checks)
    results = {c["result"] for c in checks if c["dimension"] == dimension}
    if dimension == "authorization":
        return (
            "DENIED"
            if "DENIED" in results
            else "UNRESOLVED" if not results or "UNRESOLVED" in results else "GRANTED"
        )
    return (
        "FAIL"
        if "FAIL" in results
        else "UNKNOWN" if not results or "UNKNOWN" in results else "PASS"
    )


def exit_code(report: dict) -> int:
    """Return 2 for unassessed work; reject malformed result vocabularies."""
    if not isinstance(report, dict) or "checks" not in report:
        raise ValueError("Report must be an object with checks.")
    _validate_results(report["checks"])
    results = {c["result"] for c in report["checks"]}
    return (
        1
        if results & {"FAIL", "DENIED"}
        else 2 if not results or results & {"UNKNOWN", "UNRESOLVED"} else 0
    )
