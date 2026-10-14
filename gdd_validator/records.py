"""Record integrity, explicit host authorization and evidence applicability.

Host assertions are a trust boundary. Reading a model or receipt never supplies
authority or proves an execution. This module executes no application checks.
"""

from .decision import compatible_decision_supersession
from .record_integrity import (
    ambiguous_model_ids,
    invalid_record_ids,
    model_ids as referenceable_model_ids,
)
from .loading import Bundle, digest
from .model import has_cycle
from .readiness import context_closure
from .report import check, diagnostic

__all__ = ["validate_records", "authorize", "evaluate_implementation"]


def validate_records(bundle: Bundle) -> None:
    model, records = bundle.model, bundle.records
    model_ids = referenceable_model_ids(model)
    ambiguous = ambiguous_model_ids(model)
    start = len(bundle.checks)
    for ident, record in records.items():
        if ident in model_ids:
            bundle.issue(
                "ID_COLLISION", f"Record collides with model ID {ident}.", ids=(ident,)
            )
        previous = record.get("supersedes")
        if previous and (
            previous not in records or records[previous]["kind"] != record["kind"]
        ):
            bundle.issue(
                "RECORD_INVALID",
                f"Invalid supersedes reference in {ident}.",
                ids=(ident,),
            )
        if record["kind"] in {"check", "binding", "decision", "evidence"}:
            targets = record.get("targets", record.get("scope", []))
            if bundle.revision is None or record["spec_revision"] != bundle.revision:
                bundle.issue(
                    "REVISION_UNRESOLVED",
                    f"Historical snapshot unavailable for {ident}; targets not rebound to current model.",
                    result="UNKNOWN",
                    ids=(ident,),
                )
            else:
                for target in targets:
                    if target not in model_ids:
                        bundle.issue(
                            "RECORD_INVALID",
                            f"Record {ident} has unknown current target {target}.",
                            ids=(ident,),
                        )
                    elif target in ambiguous:
                        bundle.issue(
                            "RECORD_INVALID",
                            f"Record {ident} has ambiguous current target {target}.",
                            ids=(ident,),
                        )
        references = (
            record.get("checks", [])
            if record["kind"] == "decision"
            else [record["check"]] if record["kind"] == "evidence" else []
        )
        for target in references:
            if records.get(target, {}).get("kind") != "check":
                bundle.issue(
                    "RECORD_INVALID",
                    f"Record {ident} requires check record {target}.",
                    ids=(ident,),
                )
        if record["kind"] == "evidence" and record["check"] in records:
            target = records[record["check"]]
            if target["kind"] == "check" and record["check_revision"] != digest(target):
                bundle.issue(
                    "CHECK_REVISION_MISMATCH",
                    f"Evidence {ident} does not match immutable check digest.",
                    ids=(ident,),
                )
            if record["spec_revision"] is not None and record[
                "spec_revision"
            ] != target.get("spec_revision"):
                bundle.issue(
                    "RECORD_INVALID",
                    f"Evidence {ident} and its check have different spec revisions.",
                    ids=(ident,),
                )
            if not set(target.get("scope", [])) <= set(record["scope"]):
                bundle.issue(
                    "RECORD_INVALID",
                    f"Evidence {ident} does not cover check scope.",
                    ids=(ident,),
                )
    if has_cycle(
        {i: [r["supersedes"]] if "supersedes" in r else [] for i, r in records.items()}
    ):
        bundle.issue("RECORD_INVALID", "Record supersession contains a cycle.")
    for ident, question in model.get("questions", {}).items():
        if (
            question["status"] == "resolved"
            and records.get(question["resolution"]["decision"], {}).get("kind")
            != "decision"
        ):
            bundle.issue(
                "RECORD_INVALID",
                "Question resolution must refer to a decision record.",
                ids=(ident,),
            )
    if len(bundle.checks) == start:
        bundle.checks.append(
            check(
                "records.integrity",
                "model",
                "PASS",
                "Record identities, references, supersession and current snapshot scopes checked.",
            )
        )


def authorize(bundle: Bundle, activity: str, scope: list[str], host: dict) -> None:
    policy = host.get("policy")
    invalid_records = invalid_record_ids(bundle)
    result, code, reason = (
        "UNRESOLVED",
        "AUTHORIZATION_UNRESOLVED",
        "No applicable host-trusted policy and actor authority.",
    )
    details = {"activity": activity}
    if set(scope) - bundle.model.get("nodes", {}).keys():
        bundle.checks.append(
            check(
                "authorization.scope",
                "authorization",
                "UNRESOLVED",
                "Unknown scope cannot be authorized.",
                scope,
                [
                    diagnostic(
                        "AUTHORIZATION_UNRESOLVED",
                        "Requested node does not exist.",
                        result="UNKNOWN",
                    )
                ],
                activity=activity,
            )
        )
        return
    if policy:
        details["policy_revision"] = policy["revision"]
        matches = (
            policy["activity"] == activity
            and policy["spec_revision"] == bundle.revision
            and bundle.revision is not None
        )
        scope_allowed = not policy["scope"] or (
            bool(scope) and set(scope) <= set(policy["scope"])
        )
        if policy.get("denied") or (matches and not scope_allowed):
            result, code, reason = (
                "DENIED",
                "AUTHORIZATION_DENIED",
                "Host policy denies activity or requested scope.",
            )
        elif matches and scope_allowed:
            gate = (
                bundle.model.get("project", {})
                .get("workflow", {})
                .get("gates", {})
                .get(activity)
            )
            if (
                gate
                and gate["scope"]
                and (not scope or not set(scope) <= set(gate["scope"]))
            ):
                result, code, reason = (
                    "DENIED",
                    "AUTHORIZATION_DENIED",
                    "Requested scope exceeds model gate.",
                )
            else:
                requirement = (
                    "explicit"
                    if gate and gate["requirement"] == "explicit"
                    else policy["requirement"]
                )
                actor = policy["actor"]
                authorities = set(policy["authorities"])
                if gate and gate["authorities"]:
                    authorities &= set(gate["authorities"])
                if actor in authorities and (
                    activity != "acceptance"
                    or (host.get("implementation") and host.get("code_revision"))
                ):
                    # Resolve current receipts before request coverage: a narrow
                    # approval may retire a narrow rejection under a broad mandate.
                    applicable = {
                        i: r
                        for i, r in bundle.records.items()
                        if i in host.get("trusted_decisions", [])
                        and r["kind"] == "decision"
                        and r["topic"] == "approval"
                        and r.get("activity") == activity
                        and r["outcome"] in {"accepted", "rejected"}
                        and r["spec_revision"] == bundle.revision
                        and r["policy_revision"] == policy["revision"]
                        and r["actor"]["id"] in authorities
                        and (
                            activity != "acceptance"
                            or (
                                r.get("implementation") == host.get("implementation")
                                and r.get("code_revision") == host.get("code_revision")
                            )
                        )
                    }
                    superseded = {
                        r["supersedes"]
                        for ident, r in applicable.items()
                        if ident not in invalid_records
                        and r.get("supersedes") in applicable
                        and r["supersedes"] not in invalid_records
                        and compatible_decision_supersession(
                            r, applicable[r["supersedes"]]
                        )
                    }
                    receipts = [
                        (i, r) for i, r in applicable.items() if i not in superseded
                    ]
                    approvals = [
                        (i, r)
                        for i, r in receipts
                        if i not in invalid_records
                        and r["outcome"] == "accepted"
                        and (
                            not r["scope"]
                            or (bool(scope) and set(scope) <= set(r["scope"]))
                        )
                    ]
                    if any(
                        r["outcome"] == "rejected"
                        and (
                            not r["scope"]
                            or not scope
                            or bool(set(scope) & set(r["scope"]))
                        )
                        for _, r in receipts
                    ):
                        result, code, reason = (
                            "DENIED",
                            "AUTHORIZATION_DENIED",
                            "Applicable explicit rejection.",
                        )
                    elif requirement in {"standing", "none"}:
                        result, code, reason = (
                            "GRANTED",
                            None,
                            "Applicable host-trusted standing mandate.",
                        )
                        details["authority_basis"] = policy["basis"]
                    elif len(approvals) == 1:
                        result, code, reason = (
                            "GRANTED",
                            None,
                            "Applicable host-authenticated approval receipt.",
                        )
                        details["authority_basis"] = approvals[0][0]
    requested_ids = set(scope) if scope else referenceable_model_ids(bundle.model)
    if result == "GRANTED" and requested_ids & ambiguous_model_ids(bundle.model):
        result, code, reason = (
            "UNRESOLVED",
            "AUTHORIZATION_UNRESOLVED",
            "Requested scope contains an ambiguous model identity.",
        )
        details.pop("authority_basis", None)
    diagnostics = (
        []
        if code is None
        else [
            diagnostic(
                code, reason, result="UNKNOWN" if result == "UNRESOLVED" else "FAIL"
            )
        ]
    )
    bundle.checks.append(
        check(
            "authorization.activity",
            "authorization",
            result,
            reason,
            scope,
            diagnostics,
            **details,
        )
    )


def evaluate_implementation(bundle: Bundle, scope: list[str], host: dict) -> None:
    records, nodes = bundle.records, bundle.model.get("nodes", {})
    closure = context_closure(bundle, scope) if scope else set(nodes)
    model_ids = referenceable_model_ids(bundle.model)
    invalid_records = invalid_record_ids(bundle)
    ambiguous = ambiguous_model_ids(bundle.model)
    superseded = set()
    for ident, record in records.items():
        previous_id = record.get("supersedes")
        previous = records.get(previous_id, {})
        if (
            ident in invalid_records
            or previous_id in invalid_records
            or not previous
            or record["kind"] != previous["kind"]
        ):
            continue
        if record["kind"] == "evidence":
            compatible = all(
                record.get(k) == previous.get(k)
                for k in ("check", "implementation", "spec_revision", "code_revision")
            ) and set(previous["scope"]) <= set(record["scope"])
            if ident in host.get("trusted_evidence", []) and compatible:
                superseded.add(record["supersedes"])
        elif record["kind"] == "binding":
            compatible = all(
                record.get(k) == previous.get(k)
                for k in ("implementation", "spec_revision", "code_revision")
            ) and set(previous["targets"]) <= set(record["targets"])
            if ident in host.get("verified_bindings", []) and compatible:
                superseded.add(record["supersedes"])
        elif record["kind"] == "check":
            compatible = all(
                record.get(k) == previous.get(k) for k in ("spec_revision", "method")
            ) and set(previous["scope"]) <= set(record["scope"])
            if compatible:
                superseded.add(record["supersedes"])
    active = {i: r for i, r in records.items() if i not in superseded}
    implementation, code_revision = host.get("implementation"), host.get(
        "code_revision"
    )
    if set(scope) - nodes.keys():
        bundle.checks.append(
            check(
                "implementation.scope",
                "implementation",
                "UNKNOWN",
                "Requested scope does not resolve.",
                scope,
                [
                    diagnostic(
                        "REFERENCE_UNRESOLVED",
                        "Unknown requested scope.",
                        result="UNKNOWN",
                    )
                ],
            )
        )
        return
    requested_ids = set(scope) if scope else model_ids
    ambiguous_request = requested_ids & ambiguous
    if ambiguous_request:
        bundle.checks.append(
            check(
                "implementation.scope",
                "implementation",
                "UNKNOWN",
                "Requested scope contains ambiguous model identities.",
                scope,
                [
                    diagnostic(
                        "REFERENCE_UNRESOLVED",
                        "Requested scope does not resolve unambiguously.",
                        ids=tuple(sorted(ambiguous_request)),
                        result="UNKNOWN",
                    )
                ],
            )
        )
    required = [
        (i, method)
        for i in sorted(closure & nodes.keys())
        if nodes[i].get("rule", {}).get("blocking")
        for method in nodes[i]["rule"]["verification"]
    ]
    if not required:
        bundle.checks.append(
            check(
                "implementation.requirements",
                "implementation",
                "UNKNOWN",
                "No explicit required check set; no vacuous implementation PASS.",
                scope,
                [
                    diagnostic(
                        "CHECK_PLAN_MISSING",
                        "Required check set is absent.",
                        result="UNKNOWN",
                    )
                ],
            )
        )
        return
    for number, (rule, method) in enumerate(required):
        diagnostics = []

        def unknown(code: str, message: str) -> None:
            diagnostics.append(diagnostic(code, message, ids=(rule,), result="UNKNOWN"))

        candidates = {
            i: r
            for i, r in active.items()
            if r["kind"] == "check"
            and r["spec_revision"] == bundle.revision
            and rule in r["scope"]
            and method == r["method"]
        }
        choice = host.get("check_selection", {}).get(rule, {}).get(method)
        if choice:
            candidates = {i: r for i, r in candidates.items() if i == choice}
        evidence_id, evidence = None, None
        if len(candidates) != 1:
            unknown(
                "CHECK_PLAN_MISSING", f"No unambiguous active plan for {rule}/{method}."
            )
        else:
            check_id, plan = next(iter(candidates.items()))
            if check_id in invalid_records:
                unknown(
                    "RECORD_INVALID",
                    "Selected check plan has invalid record integrity.",
                )
            observations = {
                i: r
                for i, r in active.items()
                if r["kind"] == "evidence"
                and r["check"] == check_id
                and (implementation is None or r["implementation"] == implementation)
            }
            evidence_choice = host.get("evidence_selection", {}).get(check_id)
            if evidence_choice:
                observations = {
                    i: r for i, r in observations.items() if i == evidence_choice
                }
            if len(observations) != 1:
                unknown(
                    "REVISION_UNRESOLVED", f"No unambiguous evidence for {check_id}."
                )
            else:
                evidence_id, evidence = next(iter(observations.items()))
                if evidence_id in invalid_records:
                    unknown(
                        "RECORD_INVALID",
                        "Selected evidence has invalid record integrity.",
                    )
                if (
                    bundle.revision is not None
                    and evidence["spec_revision"] == bundle.revision
                    and not set(evidence["scope"]) <= model_ids
                ):
                    unknown(
                        "RECORD_INVALID",
                        "Evidence scope contains unknown current model IDs.",
                    )
                if evidence["check_revision"] != digest(plan):
                    unknown(
                        "CHECK_REVISION_MISMATCH",
                        "Evidence does not match immutable check digest.",
                    )
                if (
                    bundle.revision is None
                    or evidence["spec_revision"] != bundle.revision
                    or not implementation
                    or evidence["implementation"] != implementation
                    or not code_revision
                    or evidence["code_revision"] != code_revision
                ):
                    unknown(
                        "EVIDENCE_STALE",
                        "Evidence does not match requested spec, implementation and code revision.",
                    )
                if (
                    method not in host.get("supported_methods", [])
                    or evidence_id not in host.get("trusted_evidence", [])
                    or not evidence["executed"]
                ):
                    unknown(
                        "REVISION_UNRESOLVED",
                        "Checker method/execution provenance is not host-verified.",
                    )
                for environment in plan["required_environment"]:
                    if environment not in evidence["environment"] or evidence[
                        "environment"
                    ].get(environment) != host.get("environment", {}).get(environment):
                        unknown(
                            "REQUIRED_ENVIRONMENT_MISSING",
                            f"Required actual environment not verified: {environment}.",
                        )
                if not set(plan["negative_controls"]) <= set(
                    host.get("negative_controls", [])
                ):
                    unknown(
                        "REVISION_UNRESOLVED",
                        "Declared negative controls lack host-verified execution.",
                    )
                if not set(plan["scope"]) <= set(evidence["scope"]):
                    unknown(
                        "REVISION_UNRESOLVED", "Evidence does not cover plan scope."
                    )
                bindings = [
                    r
                    for i, r in active.items()
                    if r["kind"] == "binding"
                    and i not in invalid_records
                    and i in host.get("verified_bindings", [])
                    and r["status"] in {"observed", "checked"}
                    and r["spec_revision"] == bundle.revision
                    and r["implementation"] == implementation
                    and r.get("code_revision") == code_revision
                    and r["role"] in {"code", "test"}
                ]
                mapped = {target for r in bindings for target in r["targets"]}
                relevant = {
                    e["to"]
                    for e in bundle.model["edges"]
                    if e["relation"] == "constrains" and e["from"] == rule
                }
                constraint = nodes[rule]["rule"].get("constraint", {})
                relevant.update(
                    constraint[k] for k in ("from", "to") if k in constraint
                )
                # A rule-target mapping is explicit coverage; otherwise its actual targets must map.
                if rule not in mapped and (not relevant or not relevant <= mapped):
                    unknown(
                        "BINDING_REVISION_MISSING",
                        "No host-verified current implementation binding.",
                    )
        result = "UNKNOWN" if diagnostics or evidence is None else evidence["result"]
        if (
            result == "PASS"
            and ambiguous
            and ambiguous & context_closure(bundle, [rule])
        ):
            unknown(
                "REFERENCE_UNRESOLVED",
                "Required rule context contains ambiguous model node identities.",
            )
            result = "UNKNOWN"
        details = {}
        if result in {"PASS", "FAIL"}:
            details = {
                "implementation": implementation,
                "code_revision": code_revision,
                "evidence_refs": [evidence_id],
            }
        bundle.checks.append(
            check(
                f"implementation.required.{number}",
                "implementation",
                result,
                f"Applicability of recorded {rule}/{method}; application check not executed by validator.",
                [rule],
                diagnostics,
                **details,
            )
        )
