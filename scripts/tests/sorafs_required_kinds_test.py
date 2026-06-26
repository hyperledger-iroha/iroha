"""Tests for shared SoraFS required-kind parsing."""

from __future__ import annotations

import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_required_kinds import parse_required_kinds  # noqa: E402


ALLOWED = {
    "feed_collector": object(),
    "billing_cycle": object(),
}
DEFAULT = ("feed_collector", "billing_cycle")


def test_absent_required_kinds_uses_default() -> None:
    parsed = parse_required_kinds(
        [],
        allowed_kinds=ALLOWED,
        default_required=DEFAULT,
    )

    assert parsed == DEFAULT


def test_comma_separated_required_kinds_pass() -> None:
    parsed = parse_required_kinds(
        ["feed_collector,billing_cycle"],
        allowed_kinds=ALLOWED,
        default_required=DEFAULT,
    )

    assert parsed == DEFAULT


def test_empty_required_kind_fails() -> None:
    try:
        parse_required_kinds(
            ["feed_collector,"],
            allowed_kinds=ALLOWED,
            default_required=DEFAULT,
        )
    except ValueError as error:
        assert "must be non-empty" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("empty required kind was accepted")


def test_duplicate_required_kind_fails() -> None:
    try:
        parse_required_kinds(
            ["feed_collector", "feed_collector"],
            allowed_kinds=ALLOWED,
            default_required=DEFAULT,
        )
    except ValueError as error:
        assert "duplicate required evidence kind" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("duplicate required kind was accepted")


def test_unknown_required_kind_fails() -> None:
    try:
        parse_required_kinds(
            ["unknown"],
            allowed_kinds=ALLOWED,
            default_required=DEFAULT,
        )
    except ValueError as error:
        assert "unknown required evidence kind `unknown`" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("unknown required kind was accepted")
