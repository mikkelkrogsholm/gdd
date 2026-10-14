"""A local append-only queue for existing agents; no agent or command runner.

The host publishes transitions after doing the corresponding external work.
Receipts are audit claims, not signed evidence or a replacement for current gates.
"""

from copy import deepcopy
import json
from pathlib import Path
import re
import tempfile

from .atomic import rename_exclusive
from .decision import compatible_decision_supersession
from .loading import canonical, digest, load_bundle, parse
from .planning import _write_resource
from .readiness import context_closure
from .record_integrity import invalid_record_ids
from .report import aggregate, check
from .validator import input_schema, validate
from .workflow import require, verify_workflow, workflow_schema

__all__ = ["create_run", "run_status", "advance_run"]


def _write(path: Path, value: dict) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n",
        encoding="utf-8",
    )


def _initial(plan: dict) -> dict:
    return {
        "status": "active",
        "code_revision": plan["request"]["base_code_revision"],
        "packages": {
            p["id"]: {"status": "pending", "attempt": None} for p in plan["packages"]
        },
        "attempts": [],
    }


def _owned(resources: list[str], declared: list[str]) -> bool:
    """Changed files must be contained, not merely overlap a declared directory."""
    areas = [_write_resource(r) for r in declared]
    for resource in resources:
        parsed = _write_resource(resource)
        if parsed is None or parsed[1]:
            return False
        if not any(
            area is not None
            and (
                parsed[0] == area[0]
                or area[1]
                and (area[0] == "." or parsed[0].startswith(area[0] + "/"))
            )
            for area in areas
        ):
            return False
    return True


def _apply(state: dict, plan: dict, event: dict) -> dict:
    """Replay structural transitions only. Current authority is checked elsewhere."""
    next_state = deepcopy(state)
    kind = event["type"]
    package_id = event.get("package")
    package = next_state["packages"].get(package_id)
    if package_id is not None and package is None:
        raise ValueError("Unknown workflow package.")
    if state["status"] == "accepted":
        raise ValueError("Accepted runs are terminal; create a new run for new work.")
    if state["status"] == "cancelled" and kind not in {"submit", "resume"}:
        raise ValueError("Cancelled runs preserve results but cannot advance work.")
    if kind == "dispatch":
        predecessors = [a for a, b in plan["request"]["serialized"] if b == package_id]
        if any(p not in state["packages"] for p in predecessors):
            raise ValueError("Invalid prerequisite package.")
        if package["status"] != "pending" or any(
            state["packages"][p]["status"] != "integrated" for p in predecessors
        ):
            raise ValueError(
                "Package is not available; prerequisites must be integrated."
            )
        if event["attempt"] in state["attempts"]:
            raise ValueError("Attempt tokens cannot be reused.")
        next_state["attempts"].append(event["attempt"])
        package.update(
            status="dispatched",
            attempt=event["attempt"],
            base_code_revision=state["code_revision"],
        )
    elif kind == "submit":
        if package["status"] != "dispatched" or package["attempt"] != event["attempt"]:
            raise ValueError("Result is not for the current dispatched attempt.")
        declaration = next(p for p in plan["packages"] if p["id"] == package_id)
        if not _owned(event["changed_resources"], declaration["write_resources"]):
            raise ValueError("Reported changes escape package write ownership.")
        package.update(status="submitted", result=deepcopy(event))
    elif kind == "integrate":
        if package["status"] != "submitted" or package["attempt"] != event["attempt"]:
            raise ValueError("Integration needs the current submitted attempt.")
        if event["base_code_revision"] != state["code_revision"]:
            raise ValueError("Integration base differs from current integrated code.")
        declaration = next(p for p in plan["packages"] if p["id"] == package_id)
        if not _owned(
            event["verified_resources"], declaration["write_resources"]
        ) or set(event["verified_resources"]) != set(
            package["result"]["changed_resources"]
        ):
            raise ValueError(
                "Host-verified changes must match submitted changes and ownership."
            )
        package.update(status="integrated", integration=deepcopy(event))
        next_state["code_revision"] = event["code_revision"]
    elif kind == "retry":
        if package["status"] not in {"dispatched", "submitted"}:
            raise ValueError("Only unfinished attempts can be retried.")
        package.clear()
        package.update(status="pending", attempt=None)
    elif kind == "cancel":
        next_state["status"] = "cancelled"
    elif kind == "resume":
        if state["status"] != "cancelled":
            raise ValueError("Only cancelled runs can be resumed.")
        next_state["status"] = "active"
    elif kind == "accept":
        if not all(p["status"] == "integrated" for p in state["packages"].values()):
            raise ValueError("All packages must be integrated before acceptance.")
        next_state["status"] = "accepted"
    return next_state


def _load(directory: Path) -> tuple[dict, dict, list, str]:
    manifest = parse((directory / "run.json").read_bytes())
    if (
        not isinstance(manifest, dict)
        or set(manifest) != {"format", "plan", "revision"}
        or manifest["format"] != "gdd-run/0.1"
    ):
        raise ValueError("Invalid run manifest.")
    require(manifest["plan"], workflow_schema("plan"), "stored workplan")
    if manifest["revision"] != digest(
        {k: v for k, v in manifest.items() if k != "revision"}
    ):
        raise ValueError("Run manifest hash differs.")
    plan = manifest["plan"]
    if plan["revision"] != digest({k: v for k, v in plan.items() if k != "revision"}):
        raise ValueError("Stored workplan hash differs.")
    state, head, events, keys = _initial(plan), manifest["revision"], [], set()
    paths = sorted((directory / "events").iterdir())
    for path in paths:
        if path.name.startswith("."):
            continue  # An unpublished concurrent temporary file is not an event.
        if (
            not re.fullmatch(r"[0-9]{6}\.json", path.name)
            or path.name != f"{len(events)+1:06d}.json"
        ):
            raise ValueError("Workflow log has a gap or unsupported filename.")
        receipt = parse(path.read_bytes())
        if (
            not isinstance(receipt, dict)
            or set(receipt) != {"format", "previous_revision", "event", "revision"}
            or receipt["format"] != "gdd-run-event/0.1"
        ):
            raise ValueError("Invalid workflow event receipt.")
        require(receipt["event"], workflow_schema("event"), "stored workflow event")
        event = receipt["event"]
        if (
            receipt["previous_revision"] != head
            or event["expected_revision"] != head
            or receipt["revision"]
            != digest({k: v for k, v in receipt.items() if k != "revision"})
        ):
            raise ValueError("Workflow log hash chain differs.")
        if event["idempotency_key"] in keys:
            raise ValueError("Duplicate workflow event key.")
        keys.add(event["idempotency_key"])
        state = _apply(state, plan, event)
        head = receipt["revision"]
        events.append(receipt)
    return plan, state, events, head


def _ready(report: dict) -> bool:
    # Unrelated historical record integrity diagnostics remain visible, but are
    # not authorization or scoped implementation-readiness verdicts.
    return aggregate(report["checks"], "readiness") == "PASS"


def _bound_host(host: dict, plan: dict, state: dict) -> bool:
    return (
        host.get("implementation") == plan["request"]["implementation"]
        and host.get("code_revision") == state["code_revision"]
    )


def _current_report(path: Path, plan: dict, host: dict) -> dict:
    """Control-plane bookkeeping survives a missing or invalid current model."""
    try:
        return verify_workflow(path, plan, host_context=host)
    except (ValueError, OSError) as error:
        return {
            "format": "gdd-report/0.2",
            "tool": {"name": "gdd-validator", "version": "0.6.0"},
            "spec_revision": None,
            "profiles": ["readiness"],
            "scope": plan["scope"],
            "checks": [
                check(
                    "workflow.model_unavailable",
                    "readiness",
                    "UNKNOWN",
                    str(error),
                    plan["scope"],
                )
            ],
        }


def _acceptance_questions(path: Path, scope: list[str], host: dict) -> list[dict]:
    bundle = load_bundle(path)
    closure = context_closure(bundle, scope)
    invalid = invalid_record_ids(bundle)
    retired = {
        r["supersedes"]
        for ident, r in bundle.records.items()
        if ident in host.get("trusted_decisions", [])
        and ident not in invalid
        and r.get("supersedes") not in invalid
        and compatible_decision_supersession(
            r, bundle.records.get(r.get("supersedes"), {})
        )
    }
    checks = []
    for ident, q in bundle.model.get("questions", {}).items():
        if "acceptance" not in q["blocks"] or not set(q["targets"]) & closure:
            continue
        receipt_id = q.get("resolution", {}).get("decision")
        receipt = bundle.records.get(receipt_id, {})
        resolved = (
            q["status"] != "open"
            and receipt_id not in retired
            and receipt_id not in invalid
            and receipt_id in host.get("trusted_decisions", [])
            and receipt.get("kind") == "decision"
            and receipt.get("outcome") == "accepted"
            and receipt.get("spec_revision") == bundle.revision
            and (not receipt.get("scope") or set(q["targets"]) <= set(receipt["scope"]))
        )
        checks.append(
            check(
                "workflow.question." + ident,
                "integration",
                "PASS" if resolved else "UNKNOWN",
                "Acceptance blocker needs a trusted current resolution.",
                [ident],
            )
        )
    return checks


def _accept_report(path: Path, plan: dict, state: dict, host: dict) -> dict:
    report = _current_report(path, plan, host)
    if not _ready(report):
        return report
    report["profiles"] = ["model", "readiness", "records"]
    report["checks"].append(
        check(
            "workflow.code_binding",
            "integration",
            "PASS" if _bound_host(host, plan, state) else "UNKNOWN",
            "Host implementation and code revision must equal this run's integrated result.",
            plan["scope"],
        )
    )
    selection = {}
    for package in plan["packages"]:
        for required in package["required_checks"]:
            if len(required["checks"]) == 1:
                selection.setdefault(required["rule"], {})[required["method"]] = (
                    required["checks"][0]
                )
    conflict = any(
        host.get("check_selection", {}).get(rule, {}).get(method, ident) != ident
        for rule, methods in selection.items()
        for method, ident in methods.items()
    )
    report["checks"].append(
        check(
            "workflow.check_selection",
            "integration",
            "FAIL" if conflict else "PASS",
            "Verification uses the exact check plans frozen in this workplan.",
            plan["scope"],
        )
    )
    evidence_host = {**host, "check_selection": selection}
    implementation = validate(
        path, profiles=["records"], scope=plan["scope"], host_context=evidence_host
    )
    authorization = validate(
        path,
        profiles=["records"],
        scope=plan["scope"],
        activity="acceptance",
        host_context=host,
    )
    report["checks"].extend(
        c for c in implementation["checks"] if c["dimension"] == "implementation"
    )
    report["checks"].extend(
        c for c in authorization["checks"] if c["dimension"] == "authorization"
    )
    report["checks"].extend(_acceptance_questions(path, plan["scope"], host))
    return report


def _accepted(report: dict) -> bool:
    return _ready(report) and all(
        aggregate(report["checks"], d) == v
        for d, v in (
            ("implementation", "PASS"),
            ("authorization", "GRANTED"),
            ("integration", "PASS"),
        )
    )


def create_run(
    path: Path, plan: dict, directory: Path, *, host_context: dict | None = None
) -> dict:
    """Create a draft queue atomically; this does not dispatch or authorize work."""
    report = verify_workflow(path, plan, host_context=host_context)
    if any(
        c["check_id"] == "workflow.packet_integrity" and c["result"] != "PASS"
        for c in report["checks"]
    ):
        raise ValueError("Cannot publish a run from a stale or altered workplan.")
    directory = Path(directory)
    with tempfile.TemporaryDirectory(prefix=".gdd-run-", dir=directory.parent) as temp:
        staging = Path(temp) / "run"
        staging.mkdir()
        (staging / "events").mkdir()
        manifest = {"format": "gdd-run/0.1", "plan": deepcopy(plan)}
        manifest["revision"] = digest(manifest)
        _write(staging / "run.json", manifest)
        rename_exclusive(staging, directory)
    return run_status(path, directory, host_context=host_context)


def run_status(
    path: Path, directory: Path, *, host_context: dict | None = None
) -> dict:
    """Read and reconcile current gates. Never redispatch a historical attempt."""
    host = host_context if host_context is not None else {}
    require(host, input_schema("host"), "host context")
    plan, state, events, head = _load(Path(directory))
    report = (
        _accept_report(path, plan, state, host)
        if state["status"] == "accepted"
        else _current_report(path, plan, host)
    )
    available = []
    if state["status"] == "active" and _ready(report):
        available = [
            ident
            for ident, p in state["packages"].items()
            if p["status"] == "pending"
            and all(
                state["packages"][a]["status"] == "integrated"
                for a, b in plan["request"]["serialized"]
                if b == ident
            )
        ]
    return {
        "format": "gdd-run-status/0.1",
        "revision": head,
        "state": state,
        "available_packages": sorted(available),
        "event_count": len(events),
        "acceptance_current": state["status"] == "accepted" and _accepted(report),
        "report": report,
    }


def advance_run(
    path: Path, directory: Path, event: dict, *, host_context: dict | None = None
) -> dict:
    """Compare-and-publish one transition. Only the host invokes gated transitions.

    Caller-supplied host context is never imported from a worker submission. A
    successful dispatch returns a packet, but transporting it remains external.
    """
    require(event, workflow_schema("event"), "workflow event")
    host = host_context if host_context is not None else {}
    require(host, input_schema("host"), "host context")
    directory = Path(directory)
    plan, state, events, head = _load(directory)
    for receipt in events:
        if receipt["event"]["idempotency_key"] == event["idempotency_key"]:
            if canonical(receipt["event"]) != canonical(event):
                raise ValueError(
                    "Idempotency key already refers to a different request."
                )
            return {
                "format": "gdd-workflow-result/0.1",
                "applied": False,
                "replayed": True,
                "receipt": receipt,
                "status": run_status(path, directory, host_context=host),
            }
    if event["expected_revision"] != head:
        raise ValueError(
            "Workflow head changed; read status before publishing a new event."
        )
    next_state = _apply(state, plan, event)
    kind = event["type"]
    report = _current_report(path, plan, host)
    allowed = True
    if kind in {"dispatch", "integrate", "resume", "accept"}:
        allowed = _ready(report)
    if kind in {"dispatch", "integrate", "resume"}:
        report["profiles"] = ["model", "readiness", "records"]
        package = (
            next(p for p in plan["packages"] if p["id"] == event["package"])
            if kind != "resume"
            else {"scope": plan["scope"]}
        )
        authorization = validate(
            path,
            profiles=["records"],
            scope=package["scope"],
            activity="implementation",
            host_context=host,
        )
        report["checks"].extend(
            c for c in authorization["checks"] if c["dimension"] == "authorization"
        )
        allowed &= aggregate(authorization["checks"], "authorization") == "GRANTED"
    if kind in {"dispatch", "resume"}:
        report["checks"].append(
            check(
                "workflow.code_binding",
                "integration",
                "PASS" if _bound_host(host, plan, state) else "UNKNOWN",
                "Host must attest this run implementation and current code base.",
                package["scope"],
            )
        )
        allowed &= report["checks"][-1]["result"] == "PASS"
    if kind == "integrate":
        report["checks"].append(
            check(
                "workflow.code_binding",
                "integration",
                (
                    "PASS"
                    if host.get("implementation") == plan["request"]["implementation"]
                    and host.get("code_revision") == event["code_revision"]
                    else "UNKNOWN"
                ),
                "Host must independently attest the reviewed integrated code revision.",
                package["scope"],
            )
        )
        allowed &= report["checks"][-1]["result"] == "PASS"
    if kind == "accept":
        report = _accept_report(path, plan, state, host)
        allowed = _accepted(report)
    if not allowed:
        return {
            "format": "gdd-workflow-result/0.1",
            "applied": False,
            "replayed": False,
            "report": report,
        }
    receipt = {
        "format": "gdd-run-event/0.1",
        "previous_revision": head,
        "event": deepcopy(event),
    }
    receipt["revision"] = digest(receipt)
    with tempfile.NamedTemporaryFile(
        prefix=".pending-", dir=directory / "events", delete=False
    ) as file:
        temporary = Path(file.name)
    try:
        _write(temporary, receipt)
        rename_exclusive(temporary, directory / "events" / f"{len(events)+1:06d}.json")
    finally:
        temporary.unlink(missing_ok=True)
    result = {
        "format": "gdd-workflow-result/0.1",
        "applied": True,
        "replayed": False,
        "receipt": receipt,
        "report": report,
    }
    if kind == "dispatch":
        result["artifact"] = {
            "format": "gdd-dispatch/0.1",
            "plan_revision": plan["revision"],
            "spec_revision": plan["spec_revision"],
            "implementation": plan["request"]["implementation"],
            "attempt": event["attempt"],
            "base_code_revision": state["code_revision"],
            "package": deepcopy(package),
        }
    result["state"] = next_state
    return result
