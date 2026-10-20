"""Read-only composition of existing assessments, without a completion verdict."""

from copy import deepcopy
from pathlib import Path
import shlex
import errno

from . import __version__
from .loading import InputError, canonical, load_bundle
from .report import aggregate, exit_code, check, diagnostic
from .validator import validate
from .workflow_run import run_status

__all__ = ["project_status", "status_text"]


def _ordered(report: dict) -> dict:
    report["checks"] = sorted(report["checks"], key=canonical)
    return report


def project_status(
    path: str | Path, *, scope: list[str] | None = None,
    run: str | Path | None = None, host_context: dict | None = None,
    implementation: str | None = None, code_revision: str | None = None,
    activity: str = "implementation",
) -> dict:
    """Assess the supplied inputs only; never discover trust, runs or revisions.

    Unassessed dimensions have a null result and do not affect the exit code.
    Known failures outrank unresolved assessed information. Authorization and
    evidence are assessed when host context or explicit code identity is supplied.
    """
    host = deepcopy(host_context) if host_context is not None else {}
    for key, value in (("implementation", implementation), ("code_revision", code_revision)):
        if value is not None:
            if key in host and host[key] != value:
                raise ValueError(f"Explicit {key} conflicts with host context.")
            host[key] = value
    if not isinstance(activity, str) or activity not in {"implementation", "acceptance", "model_changes"}:
        raise ValueError("Unknown activity.")
    # Leave invalid input shape handling to the existing validator.
    selected = scope
    validated_model = validate(path, profiles=["model"], scope=selected, host_context=host)
    selected = sorted(validated_model["scope"])
    # validate owns input validation and the semantics of every assessment.
    reports = {
        "model": _ordered(validated_model),
        "readiness": _ordered(validate(path, profiles=["readiness"], scope=selected, host_context=host)),
    }
    assess_records = host_context is not None or implementation is not None or code_revision is not None
    if assess_records:
        reports["authorization"] = _ordered(validate(
            path, profiles=["records"], scope=selected, host_context=host, activity=activity,
        ))
        reports["implementation"] = _ordered(validate(
            path, profiles=["records"], scope=selected, host_context=host,
        ))
    assessments = {}
    assessed_checks = []
    for dimension in ("model", "readiness", "authorization", "implementation"):
        report = reports.get(dimension)
        checks = [] if report is None else [
            (index, item) for index, item in enumerate(report["checks"])
            if item["dimension"] == dimension
        ]
        assessments[dimension] = {
            "assessed": report is not None,
            "result": aggregate([item for _, item in checks], dimension) if report else None,
            "check_refs": [f"/reports/{dimension}/checks/{index}" for index, _ in checks],
        }
        assessed_checks.extend(item for _, item in checks)
    workflow = None
    workflow_error = None
    if run is not None:
        try:
            workflow = run_status(Path(path), Path(run), host_context=host)
        except (OSError, ValueError) as error:
            unsupported = isinstance(error, InputError) and error.code == "FORMAT_UNSUPPORTED"
            unavailable = isinstance(error, OSError) and error.errno != errno.ELOOP
            result = "UNKNOWN" if unsupported or unavailable else "FAIL"
            code = error.code if isinstance(error, InputError) else "SOURCE_UNAVAILABLE" if unavailable else "ASSET_PATH_INVALID" if isinstance(error, OSError) else "RECORD_INVALID"
            entry = check("status.workflow_input", "integration", result, str(error),
                          diagnostics=[diagnostic(code,
                                                  str(error), str(run), result=result)])
            workflow_error = {"input": str(run), "check": entry}
            assessed_checks.append(entry)
    if workflow is not None:
        _ordered(workflow["report"])
        assessed_checks.extend(workflow["report"]["checks"])
    bundle = load_bundle(Path(path))
    questions = []
    for ident, item in sorted(bundle.model.get("questions", {}).items()):
        if item["status"] != "open":
            continue
        file, pointer = bundle.locations.get(ident, (bundle.path.name, ""))
        questions.append({"id": ident, "question": deepcopy(item),
                          "location": {"file": file, "pointer": pointer}})
    base = ["gdd", "validate", str(path)]
    scoped = [part for ident in selected for part in ("--scope", ident)]
    actions = []
    seen = set()
    sources = [(f"/reports/{name}", report, name) for name, report in reports.items()]
    if workflow:
        sources.append(("/workflow/report", workflow["report"], "workflow"))
    for prefix, report, name in sources:
        for index, item in enumerate(report["checks"]):
            if item["result"] in {"PASS", "GRANTED"}:
                continue
            for number, diag in enumerate(item["diagnostics"]):
                key = canonical(diag)
                if key in seen:
                    continue
                seen.add(key)
                profile = "records" if item["dimension"] in {"implementation", "authorization"} else "readiness" if item["dimension"] == "readiness" else "model"
                actions.append({
                    "reason": diag["message"],
                    "source": f"{prefix}/checks/{index}/diagnostics/{number}",
                    "diagnostic_code": diag["code"],
                    "location": deepcopy(diag["location"]),
                    "argv": base + ["--profile", profile] + scoped,
                })
    actions.sort(key=lambda item: (item["diagnostic_code"], item["reason"], item["source"]))
    if workflow_error:
        actions.append({"reason": workflow_error["check"]["reason"],
                        "source": "/workflow_error/check/diagnostics/0",
                        "diagnostic_code": workflow_error["check"]["diagnostics"][0]["code"],
                        "location": deepcopy(workflow_error["check"]["diagnostics"][0]["location"]),
                        "argv": ["gdd", "workflow", "status", str(path), "--run", str(run)]})
    missing = []
    for name, absent in (("run", run is None), ("host_context", host_context is None),
                         ("implementation", not host.get("implementation")),
                         ("code_revision", not host.get("code_revision"))):
        if absent:
            missing.append(name)
            actions.append({
                "reason": f"{name} was not supplied; no corresponding state or trust is inferred.",
                "source": f"/inputs/missing/{len(missing)-1}",
                "diagnostic_code": None,
                "location": None,
                "argv": ["gdd", "status", "--help"],
            })
    return {
        "format": "gdd-status/0.1",
        "tool": {"name": "gdd-validator", "version": __version__},
        "spec_revision": reports["model"]["spec_revision"],
        "scope": selected,
        "inputs": {"model": str(path), "run": None if run is None else str(run),
                   "host_context_supplied": host_context is not None,
                   "implementation": host.get("implementation"),
                   "code_revision": host.get("code_revision"), "activity": activity,
                   "missing": missing},
        "assessments": assessments, "reports": reports, "workflow": workflow,
        "workflow_error": workflow_error,
        "open_questions": questions,
        "next_actions": actions[:12], "next_action_count": len(actions),
        "exit_code": exit_code({"checks": assessed_checks}),
    }


def status_text(result: dict) -> str:
    """Render the same assessment results without reassessing any input."""
    lines = ["GDD status", f"Revision: {result['spec_revision'] or 'unresolved'}",
             "Scope: " + (", ".join(result["scope"]) or "default model scope")]
    for name, assessment in result["assessments"].items():
        lines.append(f"{name}: {assessment['result'] if assessment['assessed'] else 'UNASSESSED'}")
    workflow = result["workflow"]
    if workflow:
        lines.append(f"Workflow: {workflow['state']['status']}; acceptance_current={str(workflow['acceptance_current']).lower()}")
        for ident, package in sorted(workflow["state"]["packages"].items()):
            lines.append(f"  {ident}: {package['status']}; attempt={package.get('attempt')}")
    elif result["workflow_error"]:
        lines.append("Workflow: " + result["workflow_error"]["check"]["result"] + "; " + result["workflow_error"]["check"]["reason"])
    else:
        lines.append("Workflow: UNASSESSED (no run supplied)")
    for question in result["open_questions"]:
        lines.append(f"Open question {question['id']}: {question['question']['statement']}")
    for action in result["next_actions"]:
        location = action["location"]
        source = f"{location['file'] or '<input>'}:{location['pointer']}" if location else action["source"]
        lines.append(f"Next [{action['source']}] {source}: {action['reason']}\n  {shlex.join(action['argv'])}")
    if result["next_action_count"] > len(result["next_actions"]):
        lines.append(f"Additional actions: {result['next_action_count'] - len(result['next_actions'])}; consult underlying reports.")
    lines.append(f"Exit code: {result['exit_code']}")
    return "\n".join(lines) + "\n"
