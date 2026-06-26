"""Shared required-kind parsing for SoraFS evidence gates."""

from __future__ import annotations

from collections.abc import Mapping, Sequence


def parse_required_kinds(
    raw_values: Sequence[str],
    *,
    allowed_kinds: Mapping[str, object],
    default_required: Sequence[str],
) -> tuple[str, ...]:
    """Parse --require-kind values and reject ambiguous narrowed gates."""

    if not raw_values:
        return tuple(default_required)

    required: list[str] = []
    for raw in raw_values:
        names = raw.split(",")
        for name in names:
            candidate = name.strip()
            if not candidate:
                raise ValueError("--require-kind entries must be non-empty")
            if candidate not in allowed_kinds:
                raise ValueError(f"unknown required evidence kind `{candidate}`")
            if candidate in required:
                raise ValueError(f"duplicate required evidence kind `{candidate}`")
            required.append(candidate)
    if not required:
        raise ValueError("at least one required evidence kind must be specified")
    return tuple(required)
