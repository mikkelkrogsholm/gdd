"""Exact floor arithmetic for the action.tax contract."""


def tax(amount: int, rate_bps: int) -> int:
    """Return the floor surcharge amount after strict input validation."""
    if type(amount) is not int or type(rate_bps) is not int:
        raise ValueError("InvalidInput")
    if amount < 0 or not 0 <= rate_bps <= 10000:
        raise ValueError("InvalidInput")
    return (amount * rate_bps) // 10000
