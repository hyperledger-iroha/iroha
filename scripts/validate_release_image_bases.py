#!/usr/bin/env python3
"""Validate the two digest-pinned base refs required by the root Dockerfile."""

from __future__ import annotations

import argparse
import re
import sys


DIGEST_REFERENCE = re.compile(
    r"[a-z0-9][a-z0-9._:/-]{0,200}@sha256:[0-9a-f]{64}"
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--builder", required=True)
    parser.add_argument("--runtime", required=True)
    args = parser.parse_args()
    for label, value in (
        ("builder", args.builder),
        ("runtime", args.runtime),
    ):
        if DIGEST_REFERENCE.fullmatch(value) is None:
            print(
                f"{label} base image must be a bounded lowercase "
                "ref@sha256 digest",
                file=sys.stderr,
            )
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
