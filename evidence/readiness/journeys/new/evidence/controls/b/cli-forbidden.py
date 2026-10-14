"""Parse parcel command arguments and present one quote."""
import argparse
from .pricing import quote


def positive_grams(text: str) -> int:
    try:
        value = int(text)
    except ValueError:
        raise argparse.ArgumentTypeError("weight must be a positive integer in grams") from None
    if value <= 0:
        raise argparse.ArgumentTypeError("weight must be a positive integer in grams")
    return value


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="Quote a parcel in integer cents.")
    parser.add_argument("weight", type=positive_grams, help="positive integer weight in grams")
    parser.add_argument("zone", choices=("domestic", "remote"))
    args = parser.parse_args(argv)
    print(quote(args.weight, args.zone))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

# Deliberate negative control: a real forbidden component import.
from . import tariffs
