#!/usr/bin/env python3
"""Synchronize the reviewed transaction fixture family across SDK copies.

The shared Rust exporter writes the Android resource directory. This helper
validates that complete source family before atomically mirroring only the
shared transaction artifacts into the canonical, Python, and Swift trees.
Swift-only parity fixtures and unrelated resources are deliberately preserved.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import errno
import hashlib
import json
import os
import re
import stat
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Sequence


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SOURCE = REPO_ROOT / "java/iroha_android/src/test/resources"
DEFAULT_TARGETS: Mapping[str, Path] = {
    "canonical": REPO_ROOT / "fixtures/norito_rpc",
    "python": REPO_ROOT / "python/iroha_python/tests/fixtures",
    "swift": REPO_ROOT / "IrohaSwift/Fixtures",
}
MANIFEST_NAME = "transaction_fixtures.manifest.json"
PAYLOADS_NAME = "transaction_payloads.json"
FIXTURE_NAME_RE = re.compile(r"^[a-z][a-z0-9_]*$")
MANIFEST_ROOT_KEYS = frozenset({"fixtures"})
MANIFEST_FIXTURE_KEYS = frozenset(
    {
        "authority",
        "chain",
        "creation_time_ms",
        "encoded_file",
        "encoded_len",
        "name",
        "nonce",
        "payload_base64",
        "payload_hash",
        "signed_base64",
        "signed_hash",
        "signed_len",
        "time_to_live_ms",
    }
)
PAYLOAD_ENTRY_KEYS = frozenset(
    {
        "authority",
        "chain",
        "creation_time_ms",
        "encoded",
        "name",
        "nonce",
        "payload",
        "payload_base64",
        "payload_hash",
        "signed_base64",
        "signed_hash",
        "time_to_live_ms",
    }
)
PAYLOAD_SPEC_KEYS = frozenset(
    {
        "authority",
        "chain",
        "creation_time_ms",
        "executable",
        "fee_payment",
        "metadata",
        "nonce",
        "time_to_live_ms",
    }
)


class FixtureSyncError(RuntimeError):
    """Raised when source validation or target synchronization fails closed."""


@dataclass(frozen=True)
class FixtureFamily:
    files: Mapping[str, bytes]
    norito_names: frozenset[str]
    fixture_count: int


@dataclass(frozen=True)
class FixtureMetadata:
    chain: str
    authority: str
    creation_time_ms: int
    time_to_live_ms: int | None
    nonce: int | None
    payload_base64: str
    payload_hash: str
    signed_base64: str
    signed_hash: str


def _reject_duplicate_keys(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise FixtureSyncError(f"duplicate JSON object key {key!r}")
        result[key] = value
    return result


def _decode_json(raw: bytes, path: Path) -> object:
    try:
        return json.loads(raw.decode("utf-8"), object_pairs_hook=_reject_duplicate_keys)
    except UnicodeDecodeError as exc:
        raise FixtureSyncError(f"{path} is not valid UTF-8: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise FixtureSyncError(f"{path} is not valid JSON: {exc}") from exc


def _read_regular_file(path: Path, label: str) -> bytes:
    try:
        before_path = path.lstat()
    except FileNotFoundError as exc:
        raise FixtureSyncError(f"{label} is missing: {path}") from exc
    if stat.S_ISLNK(before_path.st_mode):
        raise FixtureSyncError(f"{label} must not be a symlink: {path}")
    if not stat.S_ISREG(before_path.st_mode):
        raise FixtureSyncError(f"{label} must be a regular file: {path}")

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        if exc.errno in {errno.ELOOP, errno.ENOENT, errno.ENOTDIR}:
            raise FixtureSyncError(
                f"{label} changed during read or became a symlink: {path}"
            ) from exc
        raise FixtureSyncError(f"failed to open {label}: {path}: {exc}") from exc

    try:
        handle = os.fdopen(descriptor, "rb")
    except BaseException:
        try:
            os.close(descriptor)
        except OSError:
            pass
        raise
    with handle:
        opened = os.fstat(handle.fileno())
        if not stat.S_ISREG(opened.st_mode):
            raise FixtureSyncError(f"{label} must be a regular file: {path}")
        if (opened.st_dev, opened.st_ino) != (before_path.st_dev, before_path.st_ino):
            raise FixtureSyncError(f"{label} changed during read: {path}")
        raw = handle.read()
        after_read = os.fstat(handle.fileno())

    try:
        after_path = path.lstat()
    except FileNotFoundError as exc:
        raise FixtureSyncError(f"{label} changed during read: {path}") from exc
    stable_fields_before = (
        opened.st_dev,
        opened.st_ino,
        opened.st_mode,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
    )
    stable_fields_after = (
        after_read.st_dev,
        after_read.st_ino,
        after_read.st_mode,
        after_read.st_size,
        after_read.st_mtime_ns,
        after_read.st_ctime_ns,
    )
    if stable_fields_before != stable_fields_after or len(raw) != after_read.st_size:
        raise FixtureSyncError(f"{label} changed while it was being read: {path}")
    if stat.S_ISLNK(after_path.st_mode) or (
        after_path.st_dev,
        after_path.st_ino,
    ) != (opened.st_dev, opened.st_ino):
        raise FixtureSyncError(f"{label} changed during read: {path}")
    return raw


def _require_exact_keys(
    entry: Mapping[str, object],
    expected: frozenset[str],
    context: str,
) -> None:
    actual = set(entry)
    if actual == expected:
        return
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    raise FixtureSyncError(
        f"{context} key inventory differs; missing={missing}, extra={extra}"
    )


def _required_string(entry: Mapping[str, object], field: str, context: str) -> str:
    value = entry.get(field)
    if not isinstance(value, str) or not value:
        raise FixtureSyncError(f"{context}.{field} must be a non-empty string")
    return value


def _required_nonnegative_int(
    entry: Mapping[str, object], field: str, context: str
) -> int:
    value = entry.get(field)
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise FixtureSyncError(f"{context}.{field} must be a non-negative integer")
    return value


def _optional_nonnegative_int(
    entry: Mapping[str, object], field: str, context: str
) -> int | None:
    value = entry.get(field)
    if value is None:
        return None
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise FixtureSyncError(f"{context}.{field} must be null or a non-negative integer")
    return value


def _canonical_base64(value: str, context: str) -> bytes:
    try:
        decoded = base64.b64decode(value, validate=True)
    except (ValueError, binascii.Error) as exc:
        raise FixtureSyncError(f"{context} is invalid base64: {exc}") from exc
    if base64.b64encode(decoded).decode("ascii") != value:
        raise FixtureSyncError(f"{context} is non-canonical base64")
    return decoded


def _iroha_hash(data: bytes) -> str:
    digest = bytearray(hashlib.blake2b(data, digest_size=32).digest())
    digest[-1] |= 1
    return digest.hex()


def _compact_length(value: int) -> bytes:
    output = bytearray()
    remaining = value
    while True:
        byte = remaining & 0x7F
        remaining >>= 7
        if remaining:
            byte |= 0x80
        output.append(byte)
        if not remaining:
            return bytes(output)


def _read_compact_length(data: bytes, offset: int, context: str) -> tuple[int, int]:
    start = offset
    value = 0
    for index in range(10):
        if offset >= len(data):
            raise FixtureSyncError(f"{context} has a truncated compact length")
        byte = data[offset]
        offset += 1
        chunk = byte & 0x7F
        if index == 9 and chunk > 1:
            raise FixtureSyncError(f"{context} compact length overflows u64")
        value |= chunk << (index * 7)
        if byte & 0x80 == 0:
            if data[start:offset] != _compact_length(value):
                raise FixtureSyncError(f"{context} has a non-canonical compact length")
            return value, offset
    raise FixtureSyncError(f"{context} compact length exceeds ten bytes")


def _read_sized_field(data: bytes, offset: int, context: str) -> tuple[bytes, int]:
    length, payload_offset = _read_compact_length(data, offset, context)
    end = payload_offset + length
    if end > len(data):
        raise FixtureSyncError(f"{context} length exceeds the signed transaction")
    return data[payload_offset:end], end


def _signed_transaction_hash(data: bytes) -> str:
    _, offset = _read_sized_field(data, 0, "signed transaction signature")
    payload, offset = _read_sized_field(data, offset, "signed transaction payload")
    _, offset = _read_sized_field(
        data, offset, "signed transaction multisig signatures"
    )
    if offset != len(data):
        raise FixtureSyncError("signed transaction has trailing bytes")
    external_payload = b"\x00\x00\x00\x00" + _compact_length(len(payload)) + payload
    return _iroha_hash(external_payload)


def _validate_payload_source(
    raw: bytes,
    path: Path,
    manifest_metadata: Mapping[str, FixtureMetadata],
) -> None:
    payload = _decode_json(raw, path)
    if not isinstance(payload, list):
        raise FixtureSyncError(f"{path} must contain a JSON array")
    seen: set[str] = set()
    for index, raw_entry in enumerate(payload):
        context = f"{path}[{index}]"
        if not isinstance(raw_entry, dict):
            raise FixtureSyncError(f"{context} must be a JSON object")
        _require_exact_keys(raw_entry, PAYLOAD_ENTRY_KEYS, context)
        name = _required_string(raw_entry, "name", context)
        if name in seen:
            raise FixtureSyncError(f"{path} contains duplicate fixture name {name!r}")
        seen.add(name)
        raw_spec = raw_entry.get("payload")
        if not isinstance(raw_spec, dict):
            raise FixtureSyncError(f"{context}.payload must be a JSON object")
        _require_exact_keys(raw_spec, PAYLOAD_SPEC_KEYS, f"{context}.payload")
        expected = manifest_metadata.get(name)
        if expected is None:
            raise FixtureSyncError(f"{path} contains unknown fixture name {name!r}")
        payload_metadata = (
            _required_string(raw_spec, "chain", f"{context}.payload"),
            _required_string(raw_spec, "authority", f"{context}.payload"),
            _required_nonnegative_int(raw_spec, "creation_time_ms", f"{context}.payload"),
            _optional_nonnegative_int(raw_spec, "time_to_live_ms", f"{context}.payload"),
            _optional_nonnegative_int(raw_spec, "nonce", f"{context}.payload"),
        )
        expected_metadata = (
            expected.chain,
            expected.authority,
            expected.creation_time_ms,
            expected.time_to_live_ms,
            expected.nonce,
        )
        if payload_metadata != expected_metadata:
            raise FixtureSyncError(
                f"{context}.payload metadata does not match the generated manifest"
            )
        outer_metadata = (
            _required_string(raw_entry, "chain", context),
            _required_string(raw_entry, "authority", context),
            _required_nonnegative_int(raw_entry, "creation_time_ms", context),
            _optional_nonnegative_int(raw_entry, "time_to_live_ms", context),
            _optional_nonnegative_int(raw_entry, "nonce", context),
        )
        if outer_metadata != expected_metadata:
            raise FixtureSyncError(
                f"{context} metadata does not match the generated manifest"
            )
        encoded = _required_string(raw_entry, "encoded", context)
        redundant_values = (
            encoded,
            _required_string(raw_entry, "payload_base64", context),
            _required_string(raw_entry, "payload_hash", context),
            _required_string(raw_entry, "signed_base64", context),
            _required_string(raw_entry, "signed_hash", context),
        )
        expected_values = (
            expected.payload_base64,
            expected.payload_base64,
            expected.payload_hash,
            expected.signed_base64,
            expected.signed_hash,
        )
        if redundant_values != expected_values:
            raise FixtureSyncError(
                f"{context} encoded/hash fields do not match the generated manifest"
            )
    if seen != set(manifest_metadata):
        missing = sorted(set(manifest_metadata) - seen)
        raise FixtureSyncError(f"{path} is missing fixture names: {', '.join(missing)}")


def load_fixture_family(source: Path, expected_count: int) -> FixtureFamily:
    if expected_count <= 0:
        raise FixtureSyncError("expected fixture count must be positive")
    if source.is_symlink():
        raise FixtureSyncError(f"source directory must not be a symlink: {source}")
    if not source.is_dir():
        raise FixtureSyncError(f"source directory is missing: {source}")

    manifest_path = source / MANIFEST_NAME
    payloads_path = source / PAYLOADS_NAME
    manifest_raw = _read_regular_file(manifest_path, "source manifest")
    payloads_raw = _read_regular_file(payloads_path, "source payload catalogue")
    manifest = _decode_json(manifest_raw, manifest_path)
    if not isinstance(manifest, dict):
        raise FixtureSyncError(f"{manifest_path} must contain a JSON object")
    _require_exact_keys(manifest, MANIFEST_ROOT_KEYS, str(manifest_path))
    raw_fixtures = manifest.get("fixtures")
    if not isinstance(raw_fixtures, list):
        raise FixtureSyncError(f"{manifest_path} must contain a fixtures array")
    if len(raw_fixtures) != expected_count:
        raise FixtureSyncError(
            f"{manifest_path} fixture count is {len(raw_fixtures)}, expected {expected_count}"
        )

    files: dict[str, bytes] = {
        MANIFEST_NAME: manifest_raw,
        PAYLOADS_NAME: payloads_raw,
    }
    norito_names: set[str] = set()
    fixture_names: set[str] = set()
    manifest_metadata: dict[str, FixtureMetadata] = {}
    for index, raw_entry in enumerate(raw_fixtures):
        context = f"{manifest_path}:fixtures[{index}]"
        if not isinstance(raw_entry, dict):
            raise FixtureSyncError(f"{context} must be a JSON object")
        _require_exact_keys(raw_entry, MANIFEST_FIXTURE_KEYS, context)
        name = _required_string(raw_entry, "name", context)
        if not FIXTURE_NAME_RE.fullmatch(name):
            raise FixtureSyncError(f"{context}.name is not a safe fixture name: {name!r}")
        if name in fixture_names:
            raise FixtureSyncError(f"{manifest_path} contains duplicate fixture name {name!r}")
        fixture_names.add(name)

        encoded_file = _required_string(raw_entry, "encoded_file", context)
        expected_file = f"{name}.norito"
        if encoded_file != expected_file or Path(encoded_file).name != encoded_file:
            raise FixtureSyncError(
                f"{context}.encoded_file must be exactly {expected_file!r}, got {encoded_file!r}"
            )
        if encoded_file in norito_names:
            raise FixtureSyncError(f"{manifest_path} duplicates encoded file {encoded_file!r}")
        norito_names.add(encoded_file)

        encoded = _read_regular_file(source / encoded_file, f"{context} encoded fixture")
        payload_base64 = _required_string(raw_entry, "payload_base64", context)
        decoded_payload = _canonical_base64(payload_base64, f"{context}.payload_base64")
        if decoded_payload != encoded:
            raise FixtureSyncError(
                f"{context}.payload_base64 does not match {encoded_file}"
            )
        encoded_len = _required_nonnegative_int(raw_entry, "encoded_len", context)
        if encoded_len != len(encoded):
            raise FixtureSyncError(
                f"{context}.encoded_len is {encoded_len}, actual length is {len(encoded)}"
            )
        payload_hash = _required_string(raw_entry, "payload_hash", context)
        if payload_hash != _iroha_hash(encoded):
            raise FixtureSyncError(f"{context}.payload_hash does not match canonical bytes")

        signed = _canonical_base64(
            _required_string(raw_entry, "signed_base64", context),
            f"{context}.signed_base64",
        )
        signed_len = _required_nonnegative_int(raw_entry, "signed_len", context)
        if signed_len != len(signed):
            raise FixtureSyncError(
                f"{context}.signed_len is {signed_len}, actual length is {len(signed)}"
            )
        signed_hash = _required_string(raw_entry, "signed_hash", context)
        if signed_hash != _signed_transaction_hash(signed):
            raise FixtureSyncError(
                f"{context}.signed_hash does not match the compact External entrypoint"
            )

        metadata = FixtureMetadata(
            chain=_required_string(raw_entry, "chain", context),
            authority=_required_string(raw_entry, "authority", context),
            creation_time_ms=_required_nonnegative_int(
                raw_entry, "creation_time_ms", context
            ),
            time_to_live_ms=_optional_nonnegative_int(
                raw_entry, "time_to_live_ms", context
            ),
            nonce=_optional_nonnegative_int(raw_entry, "nonce", context),
            payload_base64=payload_base64,
            payload_hash=payload_hash,
            signed_base64=_required_string(raw_entry, "signed_base64", context),
            signed_hash=signed_hash,
        )
        manifest_metadata[name] = metadata
        files[encoded_file] = encoded

    actual_norito = {
        path.name
        for path in source.iterdir()
        if path.name.endswith(".norito")
    }
    if actual_norito != norito_names:
        missing = sorted(norito_names - actual_norito)
        extra = sorted(actual_norito - norito_names)
        raise FixtureSyncError(
            f"{source} Norito inventory differs from manifest; missing={missing}, extra={extra}"
        )
    _validate_payload_source(payloads_raw, payloads_path, manifest_metadata)
    return FixtureFamily(
        files=files,
        norito_names=frozenset(norito_names),
        fixture_count=len(raw_fixtures),
    )


def _target_inventory(target: Path, label: str, family: FixtureFamily) -> tuple[list[str], list[Path]]:
    differences: list[str] = []
    stale: list[Path] = []
    for name, expected in family.files.items():
        path = target / name
        try:
            metadata = path.lstat()
        except FileNotFoundError:
            differences.append(f"missing {name}")
            continue
        if stat.S_ISLNK(metadata.st_mode):
            differences.append(f"managed path is a symlink: {name}")
            continue
        if not stat.S_ISREG(metadata.st_mode):
            differences.append(f"managed path is not a regular file: {name}")
            continue
        try:
            actual = _read_regular_file(path, f"{label} managed path")
        except FixtureSyncError as exc:
            differences.append(f"unsafe managed path {name}: {exc}")
            continue
        if actual != expected:
            differences.append(f"content differs: {name}")

    if target.exists():
        for path in target.iterdir():
            if not path.name.endswith(".norito") or path.name in family.norito_names:
                continue
            if label == "swift" and path.name.startswith("swift_"):
                continue
            stale.append(path)
            differences.append(f"stale shared fixture: {path.name}")
    return differences, stale


def _atomic_write(path: Path, payload: bytes) -> None:
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent, prefix=f".{path.name}.", suffix=".tmp"
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        temporary.chmod(0o644)
        os.replace(temporary, path)
        _fsync_directory(path.parent)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise


def _fsync_directory(path: Path) -> None:
    if os.name != "posix":
        return
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    descriptor = os.open(path, flags)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def sync_targets(
    family: FixtureFamily,
    targets: Mapping[str, Path],
    *,
    check: bool = False,
) -> None:
    if not targets:
        raise FixtureSyncError("at least one target is required")
    failures: list[str] = []
    for label, target in targets.items():
        if target.is_symlink():
            raise FixtureSyncError(f"{label} target directory must not be a symlink: {target}")
        if target.exists() and not target.is_dir():
            raise FixtureSyncError(f"{label} target is not a directory: {target}")
        differences, stale = _target_inventory(target, label, family)
        if check:
            if differences:
                failures.append(f"{label}: " + "; ".join(differences))
            else:
                print(
                    f"[fixture-sync] {label}: {family.fixture_count} fixtures are byte-aligned"
                )
            continue

        target.mkdir(parents=True, exist_ok=True)
        for name, payload in family.files.items():
            path = target / name
            if path.is_symlink():
                raise FixtureSyncError(f"{label} managed path must not be a symlink: {path}")
            if path.exists() and not path.is_file():
                raise FixtureSyncError(f"{label} managed path is not a regular file: {path}")
            try:
                current = _read_regular_file(path, f"{label} managed path")
            except FixtureSyncError as exc:
                if path.exists() or path.is_symlink():
                    raise
                current = None
            if current != payload:
                _atomic_write(path, payload)
        removed_stale = False
        for path in stale:
            if path.is_dir() and not path.is_symlink():
                raise FixtureSyncError(f"{label} stale fixture is a directory: {path}")
            path.unlink()
            removed_stale = True
        if removed_stale:
            _fsync_directory(target)
        post_differences, _ = _target_inventory(target, label, family)
        if post_differences:
            raise FixtureSyncError(
                f"{label} verification failed after synchronization: "
                + "; ".join(post_differences)
            )
        print(f"[fixture-sync] {label}: synchronized {family.fixture_count} fixtures")
    if failures:
        raise FixtureSyncError("fixture family drift detected: " + " | ".join(failures))


def _parse_target(value: str) -> tuple[str, Path]:
    if "=" not in value:
        raise argparse.ArgumentTypeError("target must use label=path syntax")
    label, raw_path = value.split("=", 1)
    if not re.fullmatch(r"[a-z][a-z0-9_-]*", label):
        raise argparse.ArgumentTypeError(f"invalid target label: {label!r}")
    if not raw_path:
        raise argparse.ArgumentTypeError("target path must not be empty")
    return label, Path(raw_path)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument(
        "--target",
        action="append",
        type=_parse_target,
        help="target in label=path form; defaults to canonical, Python, and Swift",
    )
    parser.add_argument("--expected-count", type=int, default=28)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify exact alignment without changing files",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        target_items = args.target or list(DEFAULT_TARGETS.items())
        targets: dict[str, Path] = {}
        for label, path in target_items:
            if label in targets:
                raise FixtureSyncError(f"duplicate target label: {label}")
            targets[label] = path
        family = load_fixture_family(args.source, args.expected_count)
        source_real = args.source.resolve()
        for label, target in targets.items():
            if target.resolve() == source_real:
                raise FixtureSyncError(f"{label} target must differ from source: {target}")
        sync_targets(family, targets, check=args.check)
    except FixtureSyncError as exc:
        print(f"[fixture-sync] error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
