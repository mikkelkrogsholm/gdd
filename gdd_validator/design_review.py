"""Check advisory review envelopes and applicability; do not judge code quality."""

from copy import deepcopy

from jsonschema import Draft202012Validator

from .boundaries import PRINCIPLES
from .loading import Bundle, canonical, schema
from .readiness import context_closure
from .model import validate_model

__all__ = ["review_design"]


def review_design(bundle: Bundle, review: dict | None, *, implementation: str | None,
                  code_revision: str | None, expected_scope: list[str] | None = None) -> dict:
    """Host-provided review claims never become blocking acceptance results."""
    result = {"format": "gdd-design-review-status/0.3", "spec_revision": bundle.revision,
              "status": "UNKNOWN", "reasons": [], "findings": [],
              "scope": [], "requested_scope": list(expected_scope or []),
              "implementation": implementation, "code_revision": code_revision,
              "notice": "Advisory review applicability only. This tool does not prove review execution or KISS/DRY compliance and does not grant acceptance."}
    if bundle.model.get("format") != "gdd/0.3" or bundle.revision is None:
        result["reasons"].append("A resolved GDD 0.3 model snapshot is required.")
        return result
    if any(c["dimension"] == "model" and c["result"] != "PASS" for c in bundle.checks):
        result["reasons"].append("Model representation is unresolved or invalid.")
        return result
    validate_model(bundle)
    if any(c["dimension"] == "model" and c["result"] != "PASS" for c in bundle.checks):
        result["reasons"].append("Model semantics are unresolved or invalid.")
        return result
    if review is None:
        result["reasons"].append("No design review was supplied.")
        return result
    if isinstance(review, dict) and isinstance(review.get("format"), str) and review["format"] != "gdd-design-review/0.3":
        result["reasons"].append("Design review format is unsupported.")
        return result
    canonical(review)
    errors = list(Draft202012Validator(schema("design-review")).iter_errors(review))
    if errors:
        result.update(status="INVALID", reasons=[e.message for e in errors])
        return result
    result["scope"] = deepcopy(review["scope"])
    result["review_identity"] = {key: deepcopy(review[key]) for key in
                                 ("spec_revision", "implementation", "code_revision", "scope", "reviewer")}
    if expected_scope is not None:
        if not expected_scope or set(expected_scope) - bundle.model["nodes"].keys():
            result.update(status="INVALID", reasons=["Requested review scope must name existing model nodes."])
            return result
        if set(expected_scope) - set(review["scope"]):
            result["reasons"].append("Review does not cover the requested scope.")
    if set(review["scope"]) - bundle.model["nodes"].keys():
        result.update(status="INVALID", reasons=["Review scope names unknown model nodes."])
        return result
    closure = context_closure(bundle, review["scope"])
    targets = closure | {e["id"] for e in bundle.model["edges"]
                         if e["from"] in closure and e["to"] in closure}
    seen = set()
    for assessment in review["assessments"]:
        principle = assessment["principle"]
        if principle in seen:
            result.update(status="INVALID", reasons=["A principle is assessed more than once."])
            return result
        seen.add(principle)
        if assessment["status"] == "not_applicable" and assessment["findings"]:
            result.update(status="INVALID", reasons=["A not-applicable principle cannot also report findings."])
            return result
        if assessment["status"] == "unknown":
            result["reasons"].append(f"Review of {principle} is unresolved.")
        for finding in assessment["findings"]:
            if set(finding["targets"]) - targets:
                result.update(status="INVALID", reasons=["Finding targets must belong to the review's model context."])
                return result
            result["findings"].append({"principle": principle, **deepcopy(finding)})
    if set(PRINCIPLES) - seen:
        result["reasons"].append("All four design principles require an explicit assessment.")
    if not review["executed"]:
        result["reasons"].append("The supplied document declares a planned review.")
    if review["spec_revision"] != bundle.revision:
        result["reasons"].append("Review belongs to another model revision.")
    if not implementation or not code_revision:
        result["reasons"].append("The host must identify the current implementation and code revision.")
    elif (review["implementation"], review["code_revision"]) != (implementation, code_revision):
        result["reasons"].append("Review belongs to another implementation or code revision.")
    if not result["reasons"]:
        result.update(status="COMPLETE", reasons=["All four advisory assessments are present and match the supplied revisions; findings require judgment."])
    return result
