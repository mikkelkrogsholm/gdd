"""Composition boundary: discount first, then tax on the remaining amount."""

from .discount import discount
from .tax import tax


def quote(subtotal: int, discount_bps: int, tax_bps: int) -> dict[str, int]:
    """Return the exact fields specified by action.quote."""
    if any(type(value) is not int for value in (subtotal, discount_bps, tax_bps)):
        raise ValueError("InvalidInput")
    if subtotal < 0 or not 0 <= discount_bps <= 10000 or not 0 <= tax_bps <= 10000:
        raise ValueError("InvalidInput")
    discount_amount = discount(subtotal, discount_bps)
    net = subtotal - discount_amount
    tax_amount = tax(net, tax_bps)
    return {"subtotal": subtotal, "discount": discount_amount, "net": net,
            "tax": tax_amount, "total": net + tax_amount}
