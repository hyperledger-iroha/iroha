#!/usr/bin/env python3
"""Refuse retired global-V2 release receipts during native qualification."""

from __future__ import annotations

import sys


NATIVE_QUALIFICATION_BLOCKER = (
    "Native release qualification is incomplete: require source-bound actual "
    "native lane and G execution captures, cross-SDK codec parity, exact committee "
    "and context rejection controls, and cross-dataspace atomic settlement (S6). "
    "Retired global-V2/grouped fixtures, the historical 58-control count, "
    "and prior receipt logs cannot qualify this candidate."
)


def main() -> int:
    """Fail closed without loading retired receipt components or publishing output."""
    # TODO: Replace this retired entry point with the actual native evidence
    # verifier only after every retained security obligation has genuine captures.
    print(NATIVE_QUALIFICATION_BLOCKER, file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
