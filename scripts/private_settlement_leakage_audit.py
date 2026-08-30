#!/usr/bin/env python3
"""Scan atomic-private-settlement evidence for planted secret canaries.

The scanner is deliberately format-agnostic so it can cover Torii/P2P packet
captures, Kura artifacts, snapshots, logs, metrics, and query responses with
one manifest. It also compares paired public captures and rejects changes in
file inventory, byte length, JSON structure, or message/record counts when only
secrets were varied.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import importlib.util
import json
import os
import stat
import sys
from collections.abc import Iterable, Iterator, Sequence
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Any
from urllib.parse import quote

REPORT_VERSION = 1
DEFAULT_CHUNK_BYTES = 1024 * 1024
DEFAULT_MAX_FILE_BYTES = 2 * 1024 * 1024 * 1024
DEFAULT_MAX_TOTAL_BYTES = 32 * 1024 * 1024 * 1024
MAX_MANIFEST_BYTES = 16 * 1024 * 1024
MAX_ARTIFACT_FILES = 1_000_000
ASSET_ADDRESS_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
REQUIRED_COUNT_CHANNELS = (
    "torii_requests",
    "torii_responses",
    "public_p2p_messages",
    "restricted_p2p_messages",
    "block_messages",
    "query_responses",
    "event_records",
    "log_records",
    "telemetry_records",
)


class AuditInputError(ValueError):
    """Raised when an audit manifest or artifact set is malformed."""


@dataclass(frozen=True)
class EncodedCanary:
    """One byte representation of a named planted secret."""

    name: str
    encoding: str
    value: bytes


def _strict_json_loads(raw: bytes, label: str) -> Any:
    """Decode UTF-8 JSON while rejecting duplicate keys and non-finite values."""

    def object_from_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise AuditInputError(f"{label} contains duplicate key {key!r}")
            result[key] = value
        return result

    def reject_constant(value: str) -> Any:
        raise AuditInputError(f"{label} contains non-JSON constant {value}")

    try:
        text = raw.decode("utf-8")
    except UnicodeError as error:
        raise AuditInputError(f"{label} is not UTF-8: {error}") from error
    try:
        return json.loads(
            text,
            object_pairs_hook=object_from_pairs,
            parse_constant=reject_constant,
        )
    except json.JSONDecodeError as error:
        raise AuditInputError(f"{label} is not valid JSON: {error}") from error


def _open_regular_nofollow(path: Path, label: str) -> tuple[int, os.stat_result]:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise AuditInputError(
            f"{label} must be a readable regular non-symlink file"
        ) from error
    metadata = os.fstat(descriptor)
    if not stat.S_ISREG(metadata.st_mode):
        os.close(descriptor)
        raise AuditInputError(f"{label} must be a regular file")
    return descriptor, metadata


def _stable_metadata(before: os.stat_result, after: os.stat_result) -> bool:
    fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    return all(getattr(before, field) == getattr(after, field) for field in fields)


def _read_regular_file(path: Path, label: str, limit: int) -> bytes:
    descriptor, before = _open_regular_nofollow(path, label)
    if before.st_size > limit:
        os.close(descriptor)
        raise AuditInputError(f"{label} exceeds the bounded file size")
    try:
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            raw = stream.read(limit + 1)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if len(raw) > limit or len(raw) != before.st_size:
        raise AuditInputError(f"{label} exceeds or changed across its bounded read")
    if not _stable_metadata(before, after):
        raise AuditInputError(f"{label} changed while it was read")
    return raw


@lru_cache(maxsize=1)
def _account_id_decoder() -> Any | None:
    """Load the dependency-free canonical-I105 codec without importing the SDK."""

    module_path = (
        Path(__file__).resolve().parents[1]
        / "python"
        / "iroha_torii_client"
        / "_account_id.py"
    )
    if not module_path.is_file() or module_path.is_symlink():
        return None
    spec = importlib.util.spec_from_file_location(
        "_private_settlement_leakage_account_id", module_path
    )
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    sys.modules.setdefault(spec.name, module)
    spec.loader.exec_module(module)
    return module.decode_canonical_i105_account_id


def _decode_base58(value: str) -> bytes:
    number = 0
    try:
        for character in value:
            number = number * 58 + ASSET_ADDRESS_ALPHABET.index(character)
    except ValueError as error:
        raise AuditInputError("asset identifier canary is not Base58") from error
    payload = b"" if number == 0 else number.to_bytes((number.bit_length() + 7) // 8, "big")
    leading = len(value) - len(value.lstrip("1"))
    return b"\x00" * leading + payload


def _encode_base58(value: bytes) -> str:
    """Encode bytes with the canonical Bitcoin Base58 alphabet."""

    number = int.from_bytes(value, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = ASSET_ADDRESS_ALPHABET[remainder] + encoded
    leading = len(value) - len(value.lstrip(b"\x00"))
    return "1" * leading + encoded


def _protocol_identifier_variants(name: str, value: str) -> list[EncodedCanary]:
    """Expand canonical identifiers into the binary forms persisted by Norito."""

    variants: list[EncodedCanary] = []
    if name.startswith("account_id"):
        decoder = _account_id_decoder()
        if decoder is None:
            raise AuditInputError(
                "canonical AccountId decoder is unavailable for an account_id canary"
            )
        try:
            canonical = decoder(value)
        except (TypeError, ValueError) as error:
            raise AuditInputError(
                f"account identifier canary {name!r} is not canonical I105"
            ) from error
        if not canonical:
            raise AuditInputError(
                f"account identifier canary {name!r} decoded to an empty payload"
            )
        variants.append(EncodedCanary(name, "canonical_account_bytes", canonical))
    if name.startswith("asset_id"):
        try:
            payload = _decode_base58(value)
        except AuditInputError as error:
            raise AuditInputError(
                f"asset identifier canary {name!r} is not canonical Base58"
            ) from error
        # AssetDefinitionId addresses are version || UUIDv4 || checksum. Norito
        # stores the UUID bytes while text and packet boundaries may retain the
        # complete address payload. The Rust request builder performs the full
        # BLAKE3 checksum check before a campaign starts; this dependency-free
        # scanner independently enforces the wire length, version, canonical
        # Base58 spelling, and UUIDv4 version/variant bits.
        uuid = payload[1:17] if len(payload) == 21 else b""
        if (
            len(payload) != 21
            or payload[0] != 1
            or _encode_base58(payload) != value
            or not uuid
            or uuid[6] >> 4 != 0b0100
            or uuid[8] & 0b1100_0000 != 0b1000_0000
        ):
            raise AuditInputError(
                f"asset identifier canary {name!r} is not a canonical V1 UUIDv4 address"
            )
        variants.extend(
            (
                EncodedCanary(name, "asset_address_payload", payload),
                EncodedCanary(name, "asset_uuid_bytes", uuid),
            )
        )
    return variants


def _leb128(value: int) -> bytes:
    encoded = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            encoded.append(byte | 0x80)
        else:
            encoded.append(byte)
            return bytes(encoded)


def _text_variants(name: str, value: str) -> list[EncodedCanary]:
    raw = value.encode("utf-8")
    variants = {
        "utf8": raw,
        "utf16le": value.encode("utf-16le"),
        "utf16be": value.encode("utf-16be"),
        "hex_lower": raw.hex().encode("ascii"),
        "hex_upper": raw.hex().upper().encode("ascii"),
        "base64": base64.b64encode(raw),
        "base64url": base64.urlsafe_b64encode(raw).rstrip(b"="),
        "url_percent": quote(value, safe="").encode("ascii"),
        "json_string": json.dumps(value, ensure_ascii=True).encode("ascii"),
    }
    result = [
        EncodedCanary(name, encoding, encoded)
        for encoding, encoded in variants.items()
        if encoded
    ]
    result.extend(_protocol_identifier_variants(name, value))
    return result


def _integer_variants(name: str, value: int) -> list[EncodedCanary]:
    if value < 0:
        raise AuditInputError(f"integer canary {name!r} must be non-negative")
    variants: dict[str, bytes] = {
        "decimal": str(value).encode("ascii"),
        "decimal_json_string": json.dumps(str(value)).encode("ascii"),
        "hex_lower": format(value, "x").encode("ascii"),
        "hex_upper": format(value, "X").encode("ascii"),
        "leb128": _leb128(value),
    }
    for width in (2, 4, 8, 16, 32):
        if value < 1 << (width * 8):
            variants[f"u{width * 8}_le"] = value.to_bytes(width, "little")
            variants[f"u{width * 8}_be"] = value.to_bytes(width, "big")
    return [
        EncodedCanary(name, encoding, encoded) for encoding, encoded in variants.items()
    ]


def _binary_variants(name: str, value: bytes) -> list[EncodedCanary]:
    if not value:
        raise AuditInputError(f"binary canary {name!r} must not be empty")
    return [
        EncodedCanary(name, "raw", value),
        EncodedCanary(name, "hex_lower", value.hex().encode("ascii")),
        EncodedCanary(name, "hex_upper", value.hex().upper().encode("ascii")),
        EncodedCanary(name, "base64", base64.b64encode(value)),
        EncodedCanary(name, "base64url", base64.urlsafe_b64encode(value).rstrip(b"=")),
    ]


def load_canaries(path: Path) -> list[EncodedCanary]:
    """Load and expand the versioned canary manifest."""

    document = _strict_json_loads(
        _read_regular_file(path, "canary manifest", MAX_MANIFEST_BYTES),
        "canary manifest",
    )
    if not isinstance(document, dict) or document.get("version") != REPORT_VERSION:
        raise AuditInputError("canary manifest version must be 1")
    entries = document.get("canaries")
    if not isinstance(entries, list) or not entries:
        raise AuditInputError("canary manifest must contain a non-empty canaries list")
    names: set[str] = set()
    expanded: list[EncodedCanary] = []
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise AuditInputError(f"canaries[{index}] must be an object")
        name = entry.get("name")
        kind = entry.get("kind")
        value = entry.get("value")
        if not isinstance(name, str) or not name or name in names:
            raise AuditInputError(
                f"canaries[{index}].name must be unique and non-empty"
            )
        names.add(name)
        if kind == "text" and isinstance(value, str) and value:
            expanded.extend(_text_variants(name, value))
        elif (
            kind == "integer" and isinstance(value, int) and not isinstance(value, bool)
        ):
            expanded.extend(_integer_variants(name, value))
        elif kind == "binary_base64" and isinstance(value, str):
            try:
                decoded = base64.b64decode(value, validate=True)
            except ValueError as error:
                raise AuditInputError(
                    f"canaries[{index}].value is not canonical base64"
                ) from error
            expanded.extend(_binary_variants(name, decoded))
        else:
            raise AuditInputError(
                f"canaries[{index}] must use text, integer, or binary_base64"
            )
    return expanded


def iter_artifact_files(paths: Sequence[Path]) -> Iterator[Path]:
    """Yield explicit regular files without following symlinks."""

    seen: set[Path] = set()
    for supplied in paths:
        try:
            supplied_metadata = os.lstat(supplied)
        except OSError as error:
            raise AuditInputError(f"artifact path does not exist: {supplied}") from error
        if stat.S_ISLNK(supplied_metadata.st_mode):
            raise AuditInputError(f"artifact path must not be a symlink: {supplied}")
        if stat.S_ISREG(supplied_metadata.st_mode):
            candidates: Iterable[Path] = (supplied,)
        elif stat.S_ISDIR(supplied_metadata.st_mode):
            discovered: list[Path] = []
            for path in supplied.rglob("*"):
                metadata = os.lstat(path)
                if stat.S_ISLNK(metadata.st_mode):
                    raise AuditInputError(f"artifact tree contains a symlink: {path}")
                if stat.S_ISREG(metadata.st_mode):
                    discovered.append(path)
                elif not stat.S_ISDIR(metadata.st_mode):
                    raise AuditInputError(f"artifact tree contains a special file: {path}")
                if len(discovered) > MAX_ARTIFACT_FILES:
                    raise AuditInputError("artifact tree exceeds the file-count bound")
            candidates = discovered
        else:
            raise AuditInputError(
                f"artifact path is not a regular file or directory: {supplied}"
            )
        for candidate in sorted(candidates):
            resolved = candidate.resolve(strict=True)
            if resolved not in seen:
                seen.add(resolved)
                yield resolved


def _artifact_binding(path: Path) -> dict[str, Any]:
    binding, _ = _inspect_file(
        path,
        (),
        chunk_bytes=DEFAULT_CHUNK_BYTES,
        maximum_bytes=DEFAULT_MAX_FILE_BYTES,
    )
    return binding


def _inspect_file(
    path: Path,
    canaries: Sequence[EncodedCanary],
    *,
    chunk_bytes: int,
    maximum_bytes: int,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Hash and scan one stable descriptor in a single bounded pass."""

    if chunk_bytes <= 0 or maximum_bytes <= 0:
        raise AuditInputError("scan bounds must be positive")
    descriptor, before = _open_regular_nofollow(path, f"artifact {path}")
    if before.st_size > maximum_bytes:
        os.close(descriptor)
        raise AuditInputError(f"artifact exceeds max-file-bytes: {path}")
    max_pattern = max((len(canary.value) for canary in canaries), default=1)
    overlap = max(0, max_pattern - 1)
    tail = b""
    absolute = 0
    digest = hashlib.sha256()
    hits: set[tuple[str, str, int]] = set()
    try:
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            while chunk := stream.read(chunk_bytes):
                absolute += len(chunk)
                if absolute > maximum_bytes:
                    raise AuditInputError(f"artifact exceeds max-file-bytes: {path}")
                digest.update(chunk)
                window = tail + chunk
                window_start = absolute - len(chunk) - len(tail)
                for canary in canaries:
                    offset = window.find(canary.value)
                    while offset >= 0:
                        absolute_offset = window_start + offset
                        if absolute_offset >= 0:
                            hits.add(
                                (canary.name, canary.encoding, absolute_offset)
                            )
                        offset = window.find(canary.value, offset + 1)
                tail = window[-overlap:] if overlap else b""
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if absolute != before.st_size or not _stable_metadata(before, after):
        raise AuditInputError(f"artifact changed while it was scanned: {path}")
    return {
        "bytes": absolute,
        "sha256": digest.hexdigest(),
    }, [
        {"canary": name, "encoding": encoding, "offset": offset}
        for name, encoding, offset in sorted(
            hits, key=lambda item: (item[2], item[0], item[1])
        )
    ]


def scan_file(
    path: Path,
    canaries: Sequence[EncodedCanary],
    *,
    chunk_bytes: int = DEFAULT_CHUNK_BYTES,
) -> list[dict[str, Any]]:
    """Return redacted canary hits with stable byte offsets."""

    _, hits = _inspect_file(
        path,
        canaries,
        chunk_bytes=chunk_bytes,
        maximum_bytes=DEFAULT_MAX_FILE_BYTES,
    )
    return hits


def _json_shape(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: _json_shape(value[key]) for key in sorted(value)}
    if isinstance(value, list):
        return [_json_shape(item) for item in value]
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        return "number"
    if isinstance(value, str):
        return "string"
    raise AuditInputError(f"unsupported JSON value type: {type(value).__name__}")


def _relative_inventory(root: Path) -> dict[str, Path]:
    if root.is_symlink() or not root.is_dir():
        raise AuditInputError(f"differential root must be a real directory: {root}")
    inventory: dict[str, Path] = {}
    for path in root.rglob("*"):
        metadata = os.lstat(path)
        if stat.S_ISLNK(metadata.st_mode):
            raise AuditInputError(f"differential root contains a symlink: {path}")
        if stat.S_ISREG(metadata.st_mode):
            inventory[path.relative_to(root).as_posix()] = path
        elif not stat.S_ISDIR(metadata.st_mode):
            raise AuditInputError(f"differential root contains a special file: {path}")
        if len(inventory) > MAX_ARTIFACT_FILES:
            raise AuditInputError("differential root exceeds the file-count bound")
    return inventory


def compare_capture_roots(
    left: Path, right: Path, max_file_bytes: int
) -> dict[str, Any]:
    """Compare public artifact inventory, byte sizes, and JSON structure."""

    left_files = _relative_inventory(left)
    right_files = _relative_inventory(right)
    left_names = set(left_files)
    right_names = set(right_files)
    common = sorted(left_names & right_names)
    size_mismatches: list[dict[str, Any]] = []
    json_shape_mismatches: list[str] = []
    for relative in common:
        left_path = left_files[relative]
        right_path = right_files[relative]
        left_size = left_path.stat().st_size
        right_size = right_path.stat().st_size
        if left_size != right_size:
            size_mismatches.append(
                {"path": relative, "left_bytes": left_size, "right_bytes": right_size}
            )
        if relative.lower().endswith(".json"):
            if left_size > max_file_bytes or right_size > max_file_bytes:
                raise AuditInputError(
                    f"JSON differential file exceeds max-file-bytes: {relative}"
                )
            try:
                left_json = _strict_json_loads(
                    _read_regular_file(left_path, relative, max_file_bytes),
                    f"left differential {relative}",
                )
                right_json = _strict_json_loads(
                    _read_regular_file(right_path, relative, max_file_bytes),
                    f"right differential {relative}",
                )
            except AuditInputError as error:
                raise AuditInputError(
                    f"cannot parse differential JSON file {relative}: {error}"
                ) from error
            if _json_shape(left_json) != _json_shape(right_json):
                json_shape_mismatches.append(relative)
    return {
        "left_only": sorted(left_names - right_names),
        "right_only": sorted(right_names - left_names),
        "size_mismatches": size_mismatches,
        "json_shape_mismatches": json_shape_mismatches,
    }


def load_message_counts(path: Path) -> dict[str, int]:
    """Load one strict capture-derived message/record count manifest."""

    if path.is_symlink() or not path.is_file():
        raise AuditInputError(f"message-count manifest must be a regular file: {path}")
    document = _strict_json_loads(
        _read_regular_file(
            path,
            f"message-count manifest {path}",
            MAX_MANIFEST_BYTES,
        ),
        f"message-count manifest {path}",
    )
    if not isinstance(document, dict) or set(document) != {"version", "channels"}:
        raise AuditInputError(
            "message-count manifest must contain only version and channels"
        )
    if document["version"] != REPORT_VERSION:
        raise AuditInputError("message-count manifest version must be 1")
    channels = document["channels"]
    if not isinstance(channels, dict) or set(channels) != set(REQUIRED_COUNT_CHANNELS):
        raise AuditInputError(
            f"message-count channels must be exactly {list(REQUIRED_COUNT_CHANNELS)}"
        )
    for name, count in channels.items():
        if isinstance(count, bool) or not isinstance(count, int) or count < 0:
            raise AuditInputError(
                f"message-count channel {name!r} must be a non-negative integer"
            )
    return {name: channels[name] for name in REQUIRED_COUNT_CHANNELS}


def compare_message_counts(left: Path, right: Path) -> list[dict[str, Any]]:
    """Reject traffic-count differences in paired secret-only experiments."""

    left_counts = load_message_counts(left)
    right_counts = load_message_counts(right)
    return [
        {
            "channel": channel,
            "left": left_counts[channel],
            "right": right_counts[channel],
        }
        for channel in REQUIRED_COUNT_CHANNELS
        if left_counts[channel] != right_counts[channel]
    ]


def run_audit(
    manifest: Path,
    artifacts: Sequence[Path],
    *,
    differential_left: Path | None = None,
    differential_right: Path | None = None,
    message_counts_left: Path | None = None,
    message_counts_right: Path | None = None,
    max_file_bytes: int = DEFAULT_MAX_FILE_BYTES,
    max_total_bytes: int = DEFAULT_MAX_TOTAL_BYTES,
) -> dict[str, Any]:
    """Run one bounded leakage and optional differential audit."""

    if max_file_bytes <= 0 or max_total_bytes <= 0:
        raise AuditInputError("byte limits must be positive")
    if manifest.is_symlink() or not manifest.is_file():
        raise AuditInputError("canary manifest must be a regular non-symlink file")
    if (differential_left is None) != (differential_right is None):
        raise AuditInputError("both differential roots must be supplied together")
    if (message_counts_left is None) != (message_counts_right is None):
        raise AuditInputError("both message-count manifests must be supplied together")
    if differential_left is not None and message_counts_left is None:
        raise AuditInputError(
            "differential experiments require both message-count manifests"
        )
    if differential_left is None and message_counts_left is not None:
        raise AuditInputError(
            "message-count manifests require paired differential roots"
        )
    canaries = load_canaries(manifest)
    files = list(iter_artifact_files(artifacts))
    scanned_paths = set(files)
    if differential_left is not None and differential_right is not None:
        differential_paths = {
            path.resolve(strict=True)
            for root in (differential_left, differential_right)
            for path in _relative_inventory(root).values()
        }
        if not differential_paths.issubset(scanned_paths):
            raise AuditInputError(
                "every differential capture file must also be supplied as an artifact"
            )
    if message_counts_left is not None and message_counts_right is not None:
        count_paths = {
            message_counts_left.resolve(strict=True),
            message_counts_right.resolve(strict=True),
        }
        if not count_paths.issubset(scanned_paths):
            raise AuditInputError(
                "both message-count manifests must also be supplied as artifacts"
            )
    total_bytes = 0
    findings: list[dict[str, Any]] = []
    scanned_artifacts: list[dict[str, Any]] = []
    for path in files:
        binding, hits = _inspect_file(
            path,
            canaries,
            chunk_bytes=DEFAULT_CHUNK_BYTES,
            maximum_bytes=max_file_bytes,
        )
        size = binding["bytes"]
        total_bytes += size
        if total_bytes > max_total_bytes:
            raise AuditInputError("artifact set exceeds max-total-bytes")
        scanned_artifacts.append(binding)
        if hits:
            findings.append({"path": str(path), "bytes": size, "hits": hits})
    differential = None
    if differential_left is not None and differential_right is not None:
        differential = compare_capture_roots(
            differential_left, differential_right, max_file_bytes
        )
    message_count_mismatches = None
    if message_counts_left is not None and message_counts_right is not None:
        message_count_mismatches = compare_message_counts(
            message_counts_left, message_counts_right
        )
    differential_failed = (
        differential is not None and any(differential.values())
    ) or bool(message_count_mismatches)
    scanned_artifacts.sort(key=lambda item: (item["sha256"], item["bytes"]))
    message_count_manifests = None
    if message_counts_left is not None and message_counts_right is not None:
        message_count_manifests = sorted(
            (
                _artifact_binding(message_counts_left.resolve(strict=True)),
                _artifact_binding(message_counts_right.resolve(strict=True)),
            ),
            key=lambda item: (item["sha256"], item["bytes"]),
        )
    return {
        "version": REPORT_VERSION,
        "passed": not findings and not differential_failed,
        "canary_manifest": _artifact_binding(manifest.resolve(strict=True)),
        "scanned_artifacts": scanned_artifacts,
        "scanned_files": len(files),
        "scanned_bytes": total_bytes,
        "canary_names": sorted({canary.name for canary in canaries}),
        "findings": findings,
        "differential": differential,
        "message_count_manifests": message_count_manifests,
        "message_count_mismatches": message_count_mismatches,
    }


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--canary-manifest", required=True, type=Path)
    parser.add_argument("--artifact", action="append", required=True, type=Path)
    parser.add_argument("--differential-left", type=Path)
    parser.add_argument("--differential-right", type=Path)
    parser.add_argument("--message-counts-left", type=Path)
    parser.add_argument("--message-counts-right", type=Path)
    parser.add_argument("--max-file-bytes", type=int, default=DEFAULT_MAX_FILE_BYTES)
    parser.add_argument("--max-total-bytes", type=int, default=DEFAULT_MAX_TOTAL_BYTES)
    parser.add_argument("--output", type=Path)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        report = run_audit(
            args.canary_manifest,
            args.artifact,
            differential_left=args.differential_left,
            differential_right=args.differential_right,
            message_counts_left=args.message_counts_left,
            message_counts_right=args.message_counts_right,
            max_file_bytes=args.max_file_bytes,
            max_total_bytes=args.max_total_bytes,
        )
        rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
        if args.output is None:
            sys.stdout.write(rendered)
        else:
            args.output.write_text(rendered, encoding="utf-8")
    except (AuditInputError, OSError) as error:
        print(f"private-settlement leakage audit input error: {error}", file=sys.stderr)
        return 2
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
