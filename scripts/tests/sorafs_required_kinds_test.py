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


def test_malformed_required_kind_name_text_fails() -> None:
    for value in (" feed_collector", "feed_collector ", "feed\ncollector"):
        try:
            parse_required_kinds(
                [value],
                allowed_kinds=ALLOWED,
                default_required=DEFAULT,
            )
        except ValueError as error:
            assert "--require-kind entries must be non-empty canonical strings" in str(
                error
            )
        else:  # pragma: no cover - defensive
            raise AssertionError(f"malformed required kind {value!r} was accepted")


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


def test_malformed_required_kind_values_fail() -> None:
    for raw_values in (
        "feed_collector",
        b"feed_collector",
        bytearray(b"feed_collector"),
        {"kind": "feed_collector"},
    ):
        try:
            parse_required_kinds(
                raw_values,
                allowed_kinds=ALLOWED,
                default_required=DEFAULT,
            )
        except ValueError as error:
            assert "--require-kind values must be a sequence" in str(error)
        else:  # pragma: no cover - defensive
            raise AssertionError("malformed required kind values were accepted")


def test_non_string_required_kind_value_fails() -> None:
    try:
        parse_required_kinds(
            ["feed_collector", 7],
            allowed_kinds=ALLOWED,
            default_required=DEFAULT,
        )
    except ValueError as error:
        assert "--require-kind values must be strings" in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("non-string required kind value was accepted")


def test_malformed_allowed_kind_registry_fails() -> None:
    for allowed_kinds in (
        "feed_collector",
        {"": object()},
        {" feed_collector": object()},
        {"feed_collector\n": object()},
        {7: object()},
    ):
        try:
            parse_required_kinds(
                [],
                allowed_kinds=allowed_kinds,
                default_required=DEFAULT,
            )
        except ValueError as error:
            assert "allowed required evidence" in str(error)
        else:  # pragma: no cover - defensive
            raise AssertionError("malformed allowed kind registry was accepted")


def test_malformed_default_required_kinds_fail() -> None:
    for default_required, expected in (
        ("feed_collector", "must be a sequence"),
        (bytearray(b"feed_collector"), "must be a sequence"),
        ({"feed_collector": True}, "must be a sequence"),
        (("feed_collector", ""), "must be non-empty canonical strings"),
        (("feed_collector", " billing_cycle"), "must be non-empty canonical strings"),
        (("feed_collector", "billing\ncycle"), "must be non-empty canonical strings"),
        (("feed_collector", "unknown"), "unknown default required evidence kind"),
        (("feed_collector", "feed_collector"), "duplicate default required evidence kind"),
    ):
        try:
            parse_required_kinds(
                [],
                allowed_kinds=ALLOWED,
                default_required=default_required,
            )
        except ValueError as error:
            assert expected in str(error)
        else:  # pragma: no cover - defensive
            raise AssertionError("malformed default required kinds were accepted")
