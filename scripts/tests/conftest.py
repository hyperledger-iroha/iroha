"""Explicit external verifier selection for SoraFS cryptographic qualification."""

from __future__ import annotations

import pytest


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
