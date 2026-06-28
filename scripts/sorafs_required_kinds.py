"""Shared required-kind parsing for SoraFS evidence gates."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any


def _is_canonical_kind_name(value: str) -> bool:
    return (
        bool(value)
        and value == value.strip()
        and not any(ord(character) < 32 or ord(character) == 127 for character in value)
    )


def _validate_allowed_kinds(allowed_kinds: Any) -> Mapping[str, object]:
    """Return allowed kind mapping or reject malformed registries."""

    if not isinstance(allowed_kinds, Mapping):
        raise ValueError("allowed required evidence kinds must be a mapping")
    if not all(
        isinstance(kind, str) and _is_canonical_kind_name(kind)
        for kind in allowed_kinds
    ):
        raise ValueError(
            "allowed required evidence kind names must be non-empty canonical strings"
        )
    return allowed_kinds


def _validate_default_required(
    default_required: Any,
    *,
    allowed_kinds: Mapping[str, object],
) -> tuple[str, ...]:
    """Return default required kinds or reject malformed defaults."""

    if isinstance(default_required, (str, bytes, bytearray, Mapping)) or not isinstance(
        default_required,
        Sequence,
    ):
        raise ValueError("default required evidence kinds must be a sequence")
    defaults: list[str] = []
    for candidate in default_required:
        if not isinstance(candidate, str) or not _is_canonical_kind_name(candidate):
            raise ValueError(
                "default required evidence kind names must be non-empty canonical strings"
            )
        if candidate not in allowed_kinds:
            raise ValueError(f"unknown default required evidence kind `{candidate}`")
        if candidate in defaults:
            raise ValueError(f"duplicate default required evidence kind `{candidate}`")
        defaults.append(candidate)
    return tuple(defaults)


def parse_required_kinds(
    raw_values: Any,
    *,
    allowed_kinds: Any,
    default_required: Any,
) -> tuple[str, ...]:
    """Parse --require-kind values and reject ambiguous narrowed gates."""

    allowed = _validate_allowed_kinds(allowed_kinds)
    defaults = _validate_default_required(default_required, allowed_kinds=allowed)
    if isinstance(raw_values, (str, bytes, bytearray, Mapping)) or not isinstance(
        raw_values,
        Sequence,
    ):
        raise ValueError("--require-kind values must be a sequence")
    if not raw_values:
        return defaults

    required: list[str] = []
    for raw in raw_values:
        if not isinstance(raw, str):
            raise ValueError("--require-kind values must be strings")
        names = raw.split(",")
        for name in names:
            candidate = name
            if not _is_canonical_kind_name(candidate):
                raise ValueError(
                    "--require-kind entries must be non-empty canonical strings"
                )
            if candidate not in allowed:
                raise ValueError(f"unknown required evidence kind `{candidate}`")
            if candidate in required:
                raise ValueError(f"duplicate required evidence kind `{candidate}`")
            required.append(candidate)
    if not required:
        raise ValueError("at least one required evidence kind must be specified")
    return tuple(required)
