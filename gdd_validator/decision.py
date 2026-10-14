"""Compatibility for retiring a host-trusted decision receipt.

Callers establish trust, current revision and applicable actor authority first.
This helper only compares the context and coverage of two decision records.
"""

__all__ = ["compatible_decision_supersession"]


def compatible_decision_supersession(replacement: dict, previous: dict) -> bool:
    """A terminal decision can retire a receipt only in the same covered context."""
    return (
        replacement.get("kind") == previous.get("kind") == "decision"
        and replacement.get("outcome") in {"accepted", "rejected"}
        and all(
            replacement.get(key) == previous.get(key)
            for key in (
                "topic",
                "spec_revision",
                "policy_revision",
                "activity",
                "implementation",
                "code_revision",
            )
        )
        and (
            not replacement["scope"]
            or (
                bool(previous["scope"])
                and set(previous["scope"]) <= set(replacement["scope"])
            )
        )
    )
