"""Per-begun-kilogram tariff table, in integer cents."""

def rate(zone: str) -> int:
    if zone == "domestic":
        return 100
    if zone == "remote":
        return 150
    raise ValueError("zone must be domestic or remote")
