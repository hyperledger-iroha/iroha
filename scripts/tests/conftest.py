"""Canonical script-test imports and explicit external verifier selection."""

from __future__ import annotations

from pathlib import Path
import sys

import pytest


# Executable script modules use their scripts directory for sibling imports.
# Establish both canonical roots before pytest imports any test module so every
# selected module has the same ownership independently of collection order.
REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(REPOSITORY_ROOT), str(REPOSITORY_ROOT / "scripts")]


def pytest_addoption(parser: pytest.Parser) -> None:
    """Register independently supplied cosign executable and exact-byte trust pin."""

    group = parser.getgroup("sorafs-cosign", "SoraFS external cosign qualification")
    group.addoption(
        "--sorafs-cosign-verifier",
        action="store",
        default=None,
        help="Explicit cosign executable path; never discovered from PATH.",
    )
    group.addoption(
        "--sorafs-cosign-verifier-sha256",
        action="store",
        default=None,
        help="Independently reviewed lowercase SHA-256 of that exact executable.",
    )
