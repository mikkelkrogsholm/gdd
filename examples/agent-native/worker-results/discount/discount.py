"""Pure discount arithmetic for the accepted floor contract."""


def discount(amount: int, rate_bps: int) -> int:
    """Return the discount amount rounded down using exact integer arithmetic."""
    if type(amount) is not int or type(rate_bps) is not int:
        raise ValueError("InvalidInput")
    if amount < 0 or not 0 <= rate_bps <= 10000:
        raise ValueError("InvalidInput")
    return amount * rate_bps // 10000
