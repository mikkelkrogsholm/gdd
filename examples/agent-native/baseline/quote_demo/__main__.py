"""Actual CLI exercised by the integration tests; no runtime dependencies."""

import argparse
import json
import sys

from .quote import quote


def main() -> int:
    parser = argparse.ArgumentParser(description="Integer quote demo")
    parser.add_argument("subtotal")
    parser.add_argument("discount_bps")
    parser.add_argument("tax_bps")
    args = parser.parse_args()
    try:
        result = quote(int(args.subtotal), int(args.discount_bps), int(args.tax_bps))
    except ValueError:
        print("InvalidInput", file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
