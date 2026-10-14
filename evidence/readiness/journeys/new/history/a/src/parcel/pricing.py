"""Complete parcel quote policy."""
from .tariffs import rate


def quote(weight_grams: int, zone: str) -> int:
    if isinstance(weight_grams, bool) or not isinstance(weight_grams, int) or weight_grams <= 0:
        raise ValueError("weight must be a positive integer in grams")
    begun_kg = (weight_grams + 999) // 1000
    return begun_kg * rate(zone)
