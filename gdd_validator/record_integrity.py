"""Record identity and reference integrity shared by semantic consumers."""

from collections import Counter

from .loading import Bundle, digest

__all__ = ["model_ids", "ambiguous_model_ids", "invalid_record_ids"]


def _model_id_counts(model: dict) -> Counter[str]:
    """Keep occurrences: global identity spans all registries and graph objects."""
    counts = Counter()
    for registry in ("nodes", "sources", "questions", "extensions"):
        counts.update(model.get(registry, {}).keys())
    counts.update(e["id"] for e in model.get("edges", []))
    counts.update(
        t["id"]
        for n in model.get("nodes", {}).values()
        for t in n.get("machine", {}).get("transitions", [])
    )
    return counts


def model_ids(model: dict) -> set[str]:
    """All referenceable model IDs, including edges and machine transitions."""
    return set(_model_id_counts(model))


def ambiguous_model_ids(model: dict) -> set[str]:
    """IDs that cannot resolve uniquely within the current model snapshot."""
    return {ident for ident, count in _model_id_counts(model).items() if count > 1}


def invalid_record_ids(bundle: Bundle) -> set[str]:
    """Records whose known integrity defects prevent positive use or retirement.

    Historical targets are not rebound. Trust and revision applicability remain
    separate checks at each consumer; unrelated invalid records do not block it.
    """
    records, known_model_ids = bundle.records, model_ids(bundle.model)
    ambiguous = ambiguous_model_ids(bundle.model)
    invalid = set()
    for ident, record in records.items():
        targets = record.get("targets", record.get("scope", []))
        if ident in known_model_ids or (
            bundle.revision is not None
            and record["spec_revision"] == bundle.revision
            and (not set(targets) <= known_model_ids or bool(set(targets) & ambiguous))
        ):
            invalid.add(ident)
        previous = record.get("supersedes")
        if previous is not None and (
            previous in known_model_ids
            or previous not in records
            or records[previous]["kind"] != record["kind"]
        ):
            invalid.add(ident)
        references = (
            record.get("checks", [])
            if record["kind"] == "decision"
            else [record["check"]] if record["kind"] == "evidence" else []
        )
        if any(
            target in known_model_ids or records.get(target, {}).get("kind") != "check"
            for target in references
        ):
            invalid.add(ident)
        if record["kind"] == "evidence":
            plan = records.get(record["check"], {})
            if plan.get("kind") == "check" and record["check_revision"] != digest(plan):
                invalid.add(ident)
            if (
                record["spec_revision"] is not None
                and record["spec_revision"] != plan.get("spec_revision")
            ) or not set(plan.get("scope", [])) <= set(record["scope"]):
                invalid.add(ident)
    # Each record has at most one supersedes edge. Find the cycle members only;
    # a separate valid record must not inherit an unrelated ledger defect.
    visited = set()
    for start in records:
        chain, positions = [], {}
        current = start
        while current in records and current not in visited:
            if current in positions:
                invalid.update(chain[positions[current] :])
                break
            positions[current] = len(chain)
            chain.append(current)
            current = records[current].get("supersedes")
        visited.update(chain)
    return invalid
