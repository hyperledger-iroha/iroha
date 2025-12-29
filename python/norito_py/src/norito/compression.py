# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Compression helpers for Norito payloads."""

from __future__ import annotations

from typing import Final, Optional, Tuple

from .errors import UnsupportedCompressionError

_COMPRESSION_ZSTD: Final[int] = 1
_ZSTD_MIN_LEVEL: Final[int] = 1
_ZSTD_MAX_LEVEL: Final[int] = 22

_LEVEL_PROFILES: Final[
    dict[str, Tuple[Tuple[int, Optional[int], int], ...]]
] = {
    # Prioritise encode speed. Useful for development fixtures where latency matters
    # more than final size.
    "fast": (
        (0, 64 * 1024, 1),
        (64 * 1024, 512 * 1024, 2),
        (512 * 1024, 4 * 1024 * 1024, 3),
        (4 * 1024 * 1024, None, 4),
    ),
    # Matches the current Rust defaults: favour good compression while keeping
    # encode cost reasonable for mid-sized manifests (< 4 MiB).
    "balanced": (
        (0, 64 * 1024, 3),
        (64 * 1024, 512 * 1024, 5),
        (512 * 1024, 4 * 1024 * 1024, 7),
        (4 * 1024 * 1024, None, 9),
    ),
    # Trade CPU time for better ratios. Intended for large archival payloads
    # (e.g., telemetry exports, pin registry snapshots).
    "compact": (
        (0, 64 * 1024, 7),
        (64 * 1024, 512 * 1024, 11),
        (512 * 1024, 4 * 1024 * 1024, 15),
        (4 * 1024 * 1024, None, 19),
    ),
}

try:  # pragma: no cover - optional dependency branch
    import zstandard as _zstd  # type: ignore
except ImportError:  # pragma: no cover - import failure path
    _zstd = None  # type: ignore


def has_zstd() -> bool:
    """Return ``True`` if the `zstandard` module is available."""

    return _zstd is not None


def select_zstd_level(payload_len: int, profile: str = "balanced") -> int:
    """Select a compression level for the given payload length and profile.

    Args:
        payload_len: Raw (uncompressed) payload length in bytes.
        profile: Compression profile hint. Supported values:
            ``"fast"``, ``"balanced"``, ``"compact"``.

    Returns:
        Zstandard compression level within the canonical range ``[1, 22]``.

    Raises:
        ValueError: When *payload_len* is negative or *profile* is unknown.
    """

    if payload_len < 0:
        raise ValueError("payload_len must be non-negative")
    try:
        buckets = _LEVEL_PROFILES[profile.lower()]
    except KeyError as exc:
        raise ValueError(f"unknown compression profile {profile!r}") from exc
    for lower, upper, level in buckets:
        if payload_len >= lower and (upper is None or payload_len < upper):
            return _clamp_level(level)
    # Fallback to the most conservative entry (last bucket).
    return _clamp_level(buckets[-1][2])


def resolve_zstd_level(
    payload_len: int,
    *,
    level: Optional[int],
    profile: Optional[str],
) -> int:
    """Resolve the effective Zstandard compression level.

    Preference order:
    1. Explicit *level* provided by the caller.
    2. Profile-based selection via ``select_zstd_level``.
    3. Legacy default (level 3).
    """

    if level is not None:
        return _clamp_level(level)
    if profile is not None:
        return select_zstd_level(payload_len, profile)
    return 3


def _clamp_level(level: int) -> int:
    if not (_ZSTD_MIN_LEVEL <= level <= _ZSTD_MAX_LEVEL):
        raise ValueError(
            f"Zstandard level {level} must be between {_ZSTD_MIN_LEVEL} "
            f"and {_ZSTD_MAX_LEVEL}"
        )
    return level


def compress_zstd(data: bytes, *, level: int = 3) -> bytes:
    """Compress *data* using Zstandard.

    Raises ``UnsupportedCompressionError`` when the backend is unavailable.
    """

    if _zstd is None:
        raise UnsupportedCompressionError(_COMPRESSION_ZSTD)
    compressor = _zstd.ZstdCompressor(level=level)
    return compressor.compress(data)


def decompress_zstd(data: bytes, *, target_size: Optional[int] = None) -> bytes:
    """Decompress Zstandard ``data``.

    Args:
        data: The compressed payload.
        target_size: Optional hint for the expected uncompressed size. When
            provided, the output is capped to this many bytes.
    """

    if _zstd is None:
        raise UnsupportedCompressionError(_COMPRESSION_ZSTD)
    decompressor = _zstd.ZstdDecompressor()
    if target_size is None:
        return decompressor.decompress(data)
    return decompressor.decompress(data, max_output_size=target_size)


__all__ = [
    "compress_zstd",
    "decompress_zstd",
    "has_zstd",
    "select_zstd_level",
    "resolve_zstd_level",
]
