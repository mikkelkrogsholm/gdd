"""The baseline intentionally uses obsolete half-up rounding."""


def discount(amount: int, rate_bps: int) -> int:
    """Return the discount amount; repair rounding to match action.discount."""
    if type(amount) is not int or type(rate_bps) is not int:
        raise ValueError("InvalidInput")
    if amount < 0 or not 0 <= rate_bps <= 10000:
        raise ValueError("InvalidInput")
    return (amount * rate_bps + 5000) // 10000
