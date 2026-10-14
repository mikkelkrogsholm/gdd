#!/usr/bin/env python3
"""Check the portable skill's generated references against their local hash index."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys


def verify(skill: Path) -> list[str]:
    skill = skill.resolve()
    errors = []
    index = json.loads((skill / "references/source-index.json").read_text(encoding="utf-8"))
    if index.get("format") != "gdd-skill-assets/1":
        raise ValueError("Unsupported skill asset index")
    expected = set()
    for entry in index["files"]:
        relative = entry["path"]
        path = skill / relative
        if not path.resolve().is_relative_to(skill / "references/gdd"):
            raise ValueError(f"Generated reference escapes its directory: {relative}")
        if relative in expected:
            raise ValueError(f"Duplicate generated reference: {relative}")
        expected.add(relative)
        if not path.is_file():
            errors.append(f"Missing reference: {relative}")
            continue
        data = path.read_bytes()
        if len(data) != entry["bytes"] or hashlib.sha256(data).hexdigest() != entry["sha256"]:
            errors.append(f"Reference hash mismatch: {relative}")
    actual = {path.relative_to(skill).as_posix()
              for path in (skill / "references/gdd").rglob("*") if path.is_file()}
    errors.extend(f"Unindexed reference: {relative}" for relative in sorted(actual - expected))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("skill", nargs="?", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    try:
        errors = verify(args.skill)
    except (OSError, ValueError, KeyError, TypeError) as exc:
        print(f"Skill integrity check failed: {exc}", file=sys.stderr)
        return 1
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Bundled reference hashes match. This checks local integrity, not authority or upstream freshness.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
