"""Transient exact numbers for JSON comparisons, never stored model values."""

from fractions import Fraction
from typing import Any

__all__ = ["exact_number", "exact_numbers"]


def exact_number(value: Any) -> Any:
    """Retain the parsed decimal spelling, including integral float values.

    Loading has already rejected floats without a precise decimal round trip.
    Integral fractions become builtin ints for JSON Schema's integer semantics;
    nonintegral fractions support exact comparison, equality and multipleOf.
    """
    if not isinstance(value, float):
        return value
    exact = Fraction(str(value))
    return exact.numerator if exact.denominator == 1 else exact


def exact_numbers(value: Any) -> Any:
    """Copy JSON containers iteratively, normalizing only numeric leaf values.

    The caller must keep this validation copy separate from Bundle, canonical
    serialization and artifacts. Iteration preserves deep inert annotations
    without spending the recursive schema validator's nesting capacity.
    """
    if not isinstance(value, (dict, list)):
        return exact_number(value)
    result = {} if isinstance(value, dict) else [None] * len(value)
    pending = [(value, result)]
    while pending:
        source, target = pending.pop()
        items = source.items() if isinstance(source, dict) else enumerate(source)
        for key, item in items:
            if isinstance(item, (dict, list)):
                child = {} if isinstance(item, dict) else [None] * len(item)
                target[key] = child
                pending.append((item, child))
            else:
                target[key] = exact_number(item)
    return result
