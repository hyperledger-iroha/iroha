#!/usr/bin/env python3
"""Retired compatibility entry point for the old Taira offline-gated reset.

Offline application protocols are a universal Iroha capability. They are not a
validator mode, asset enrollment, escrow catalog, or readiness admission gate.
Use ``prepare_taira_empty_reset_bundle.py`` for a fresh Taira storage reset.
"""

from __future__ import annotations

from collections.abc import Sequence
import sys


REPLACEMENT = "scripts/prepare_taira_empty_reset_bundle.py"
RETIREMENT_MESSAGE = (
    "prepare_taira_offline_reset_bundle.py is retired: offline application "
    "protocols are universally available and require no backend enablement, "
    f"asset catalog, or readiness gate; use {REPLACEMENT}"
)


def main(argv: Sequence[str] | None = None) -> int:
    """Refuse the obsolete workflow without inspecting or mutating its inputs."""

    del argv
    print(RETIREMENT_MESSAGE, file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
