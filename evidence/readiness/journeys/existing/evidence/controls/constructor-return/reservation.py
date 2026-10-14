class InvalidTransition(ValueError):
    """The requested operation cannot run from the current lifecycle state."""


class Reservation:
    def __init__(self):
        self.state = "draft"


def reserve(value: Reservation) -> Reservation:
    if value.state == "draft":
        value.state = "reserved"
    elif value.state != "reserved":
        raise InvalidTransition(f"Cannot reserve a {value.state} Reservation")
    return Reservation()


def cancel(value: Reservation) -> Reservation:
    if value.state == "reserved":
        value.state = "cancelled"
    elif value.state != "cancelled":
        raise InvalidTransition(f"Cannot cancel a {value.state} Reservation")
    return value
