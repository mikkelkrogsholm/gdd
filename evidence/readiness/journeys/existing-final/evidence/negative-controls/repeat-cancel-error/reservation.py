class InvalidTransition(ValueError):
    """The requested operation cannot occur in the current lifecycle state."""


class Reservation:
    def __init__(self):
        self.state = "draft"

def reserve(value: Reservation) -> Reservation:
    if value.state not in ("draft", "reserved"):
        raise InvalidTransition("Only draft or reserved reservations can be reserved.")
    value.state = "reserved"
    return value


def cancel(value: Reservation) -> Reservation:
    if value.state not in ("reserved",):
        raise InvalidTransition("Only reserved or cancelled reservations can be cancelled.")
    value.state = "cancelled"
    return value
