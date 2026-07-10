#!/usr/bin/env python3
"""Strict SCCP V1 public release-evidence and bundle primitives.

This module deliberately contains no signing or deployment code.  Release
operators provide detached Ed25519 signatures made outside the repository;
the tools in this directory only verify those signatures and public evidence.
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import html
import json
import os
import re
import stat
import struct
import subprocess
import sys
import threading
import unicodedata
import urllib.parse
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Mapping, Sequence


EVIDENCE_SCHEMA = "sccp-release-evidence-v1"
BUNDLE_SCHEMA = "sccp-release-bundle-v1"
READINESS_SCHEMA = "sccp-release-readiness-v1"
TRUST_POLICY_SCHEMA = "sccp-release-trust-policy-v1"
TEST_TRUST_POLICY_SCHEMA = "sccp-release-test-trust-policy-v1"
RUST_VALIDATION_SCHEMA = "sccp-release-lane-validation-v1"
SIGNING_DOMAIN = b"iroha:sccp:release-evidence:v1\x00"
BUNDLE_HASH_DOMAIN = b"iroha:sccp:release-bundle:v1\x00"
VALIDATOR_BUILD_ID_DOMAIN = b"sccp:release-evidence-validator:v1\x00"
CIRCUIT_POLICY_SIGNING_DOMAIN = b"iroha:sccp:circuit-policy-audit:v1\x00"
FORBIDDEN_ALGEBRAIC_SMOKE_VK = (
    "9ef8067d260532f88e60cfa4b458fe678fc46b9c242de18fc91ba646e0857fc4"
)

MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
MAX_INDEX_BYTES = 512 * 1024
MAX_ARTIFACT_BYTES = 16 * 1024 * 1024
MAX_TRANSCRIPT_BYTES = 4 * 1024 * 1024
MAX_TOTAL_ARTIFACT_BYTES = 64 * 1024 * 1024
MAX_ARTIFACTS = 64
MAX_JSON_DEPTH = 32
MAX_JSON_NODES = 32_768
MAX_PUBLIC_ERROR_BYTES = 1024
MAX_TRUST_POLICY_BYTES = 64 * 1024
MAX_VALIDATOR_BINARY_BYTES = 128 * 1024 * 1024
MAX_VALIDATOR_OUTPUT_BYTES = 16 * 1024
MAX_VALIDATOR_ERROR_BYTES = 4096
MAX_VALIDATOR_SECONDS = 30
MAX_DESTINATION_ATTESTATION_AGE_MS = 24 * 60 * 60 * 1000

REQUIRED_PHASES = (
    "rust-sccp",
    "evidence-scripts",
    "js-sdk",
    "python-sdk",
    "swift-sdk",
    "kotlin-sdk",
    "java-android",
    "dotnet-sdk",
    "contract-smoke",
    "core-admission",
)

PROFILE_ORDER = (
    "ethereum-mainnet",
    "bsc-mainnet",
    "tron-mainnet",
)

HUB_CHAIN_IDS = {
    "sora-nexus": "00000000-0000-0000-0000-000000000753",
    "sora-taira": "809574f5-fee7-5e69-bfcf-52451e42d50f",
}

PROFILE_DOMAINS = {
    "ethereum-mainnet": 1,
    "bsc-mainnet": 2,
    "tron-mainnet": 5,
}

UNAVAILABLE_INBOUND_REASONS = {
    profile: "authenticated-native-inbound-proof-is-unavailable"
    for profile in PROFILE_ORDER
}

OUTBOUND_UNAVAILABLE_REASON = (
    "authenticated-destination-state-is-unavailable"
)

EXPECTED_INBOUND_STATUS = {
    "ethereum-mainnet": "verified",
    "bsc-mainnet": "verified",
    "tron-mainnet": "verified",
}

EXPECTED_OUTBOUND_STATUS = {profile: "verified" for profile in PROFILE_ORDER}

ARTIFACT_KINDS = frozenset(("phase-transcript", "lane-evidence"))
PROVENANCE_ROLES = ("release-engineering", "release-security")
CIRCUIT_AUDITOR_ROLES = (
    "semantic-security-audit",
    "prover-reproducibility-audit",
)
RUST_VALIDATOR_SOURCE = (
    Path(__file__).resolve().parents[1]
    / "crates"
    / "iroha_sccp"
    / "src"
    / "bin"
    / "sccp_release_evidence.rs"
)

_SAFE_SEGMENT_RE = re.compile(r"^[a-z0-9](?:[a-z0-9._-]{0,95})$")
_SAFE_ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9._:+-]{0,127})$")
_HEX_RE = re.compile(r"^[0-9a-f]+$")
_SENSITIVE_RE = re.compile(
    r"(?:"
    r"private[\s._-]*key|secret[\s._-]*key|seed[\s._-]*phrase|"
    r"recovery[\s._-]*phrase|mnemonic|bearer[\s._-]+[a-z0-9]|"
    r"authorization[\s._-]*:|password[\s._-]*=|"
    r"client[\s._-]*secret|api[\s._-]*(?:key|token)[\s._-]*="
    r")",
    re.IGNORECASE,
)
_BASE64_TOKEN_RE = re.compile(
    rb"(?<![A-Za-z0-9+/=])(?:[A-Za-z0-9+/]{4}){4,}"
    rb"(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?(?![A-Za-z0-9+/=])"
)
_SAFE_VERSION_RE = re.compile(r"^[0-9]+(?:\.[0-9]+){2}(?:[-+][A-Za-z0-9.-]+)?$")
_UNAVAILABLE_REASON_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")


class SccpReleaseError(ValueError):
    """A bounded, public-safe SCCP release validation failure."""


def _fail(message: str) -> None:
    raise SccpReleaseError(message)


def public_error(error: BaseException) -> str:
    """Return a bounded error message with common secret shapes redacted."""

    text = unicodedata.normalize("NFKC", str(error))
    text = re.sub(r"(?i)(?:https?://)[^/@\s]+@", "https://<redacted>@", text)
    text = _SENSITIVE_RE.sub("<redacted>", text)
    text = "".join(ch if ch in "\n\t" or ord(ch) >= 0x20 else "?" for ch in text)
    encoded = text.encode("utf-8", "replace")[:MAX_PUBLIC_ERROR_BYTES]
    return encoded.decode("utf-8", "ignore") or "SCCP release validation failed"


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if type(key) is not str or key in result:
            _fail("JSON contains a duplicate or non-string object key")
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    _fail(f"JSON contains forbidden non-finite number {value}")


def _json_shape(value: Any, *, depth: int = 0) -> int:
    if depth > MAX_JSON_DEPTH:
        _fail("JSON nesting exceeds the SCCP release limit")
    if value is None or type(value) in (bool, int, str):
        return 1
    if type(value) is list:
        total = 1
        for item in value:
            total += _json_shape(item, depth=depth + 1)
            if total > MAX_JSON_NODES:
                _fail("JSON node count exceeds the SCCP release limit")
        return total
    if type(value) is dict:
        total = 1
        for key, item in value.items():
            if type(key) is not str:
                _fail("JSON object keys must be strings")
            total += 1 + _json_shape(item, depth=depth + 1)
            if total > MAX_JSON_NODES:
                _fail("JSON node count exceeds the SCCP release limit")
        return total
    _fail("JSON contains a value outside the canonical SCCP subset")


def parse_json_bytes(data: bytes, *, label: str, maximum: int) -> Any:
    """Decode strict UTF-8 JSON with duplicate-key and shape limits."""

    if type(data) is not bytes or not data or len(data) > maximum:
        _fail(f"{label} must contain between 1 and {maximum} bytes")
    if data.startswith(b"\xef\xbb\xbf") or b"\x00" in data:
        _fail(f"{label} must be canonical UTF-8 JSON without BOM or NUL")
    try:
        text = data.decode("utf-8", "strict")
    except UnicodeDecodeError:
        _fail(f"{label} is not valid UTF-8")
    try:
        value = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=_reject_json_constant,
        )
    except SccpReleaseError:
        raise
    except (json.JSONDecodeError, RecursionError, ValueError):
        _fail(f"{label} is not valid canonical JSON")
    _json_shape(value)
    return value


def canonical_json_bytes(value: Any) -> bytes:
    """Return the single canonical JSON encoding used by SCCP release tools."""

    _json_shape(value)
    try:
        return json.dumps(
            value,
            ensure_ascii=True,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("ascii")
    except (TypeError, ValueError, RecursionError):
        _fail("value cannot be encoded as canonical SCCP JSON")


def canonical_json_file_bytes(value: Any) -> bytes:
    """Return canonical JSON followed by exactly one LF."""

    return canonical_json_bytes(value) + b"\n"


def require_canonical_json_file(data: bytes, value: Any, *, label: str) -> None:
    if data != canonical_json_file_bytes(value):
        _fail(f"{label} must use canonical sorted compact JSON and one trailing LF")


def _safe_relative_parts(value: Any, *, label: str) -> tuple[str, ...]:
    if type(value) is not str or not value or len(value.encode("utf-8", "strict")) > 240:
        _fail(f"{label} must be a bounded relative POSIX path")
    if value != value.strip() or "\\" in value or any(ord(ch) < 0x20 for ch in value):
        _fail(f"{label} must be a canonical relative POSIX path")
    path = PurePosixPath(value)
    if path.is_absolute() or str(path) != value:
        _fail(f"{label} must be a canonical relative POSIX path")
    parts = path.parts
    if not parts or any(part in ("", ".", "..") or not _SAFE_SEGMENT_RE.fullmatch(part) for part in parts):
        _fail(f"{label} contains an unsafe path component")
    return parts


def _require_direct_directory(path: Path, *, label: str) -> os.stat_result:
    try:
        metadata = path.lstat()
    except OSError:
        _fail(f"{label} is not an accessible directory")
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        _fail(f"{label} must be a direct non-symlink directory")
    return metadata


def read_direct_file(path: Path, *, label: str, maximum: int) -> bytes:
    """Read one regular, single-link file while rejecting common swap attacks."""

    try:
        before = path.lstat()
    except OSError:
        _fail(f"{label} is not an accessible file")
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        _fail(f"{label} must be a direct regular file")
    if before.st_nlink != 1:
        _fail(f"{label} must not be hard-linked")
    if before.st_size <= 0 or before.st_size > maximum:
        _fail(f"{label} must contain between 1 and {maximum} bytes")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError:
        _fail(f"{label} could not be opened safely")
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1:
            _fail(f"{label} changed file type while opening")
        if (opened.st_dev, opened.st_ino, opened.st_size) != (
            before.st_dev,
            before.st_ino,
            before.st_size,
        ):
            _fail(f"{label} changed while opening")
        chunks: list[bytes] = []
        remaining = maximum + 1
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        after_open = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    try:
        after = path.lstat()
    except OSError:
        _fail(f"{label} disappeared while reading")
    identity = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
    if identity != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns):
        _fail(f"{label} changed while reading")
    if identity != (
        after_open.st_dev,
        after_open.st_ino,
        after_open.st_size,
        after_open.st_mtime_ns,
    ):
        _fail(f"{label} changed while reading")
    if not data or len(data) > maximum or len(data) != before.st_size:
        _fail(f"{label} has an invalid or unstable size")
    return data


def read_relative_file(root: Path, relative: str, *, label: str, maximum: int) -> bytes:
    """Read a contained artifact after rejecting symlinked path components."""

    _require_direct_directory(root, label="artifact root")
    parts = _safe_relative_parts(relative, label=label)
    current = root
    for part in parts[:-1]:
        current = current / part
        _require_direct_directory(current, label=f"{label} parent")
    return read_direct_file(current / parts[-1], label=label, maximum=maximum)


def enumerate_direct_files(root: Path) -> tuple[str, ...]:
    """Enumerate a bounded tree while refusing links and unsafe names."""

    _require_direct_directory(root, label="bundle directory")
    files: list[str] = []
    stack: list[tuple[Path, tuple[str, ...]]] = [(root, ())]
    visited_entries = 0
    while stack:
        directory, prefix = stack.pop()
        try:
            entries = sorted(os.scandir(directory), key=lambda entry: entry.name)
        except OSError:
            _fail("bundle directory could not be enumerated safely")
        if prefix and not entries:
            _fail("bundle must not contain uncommitted empty directories")
        for entry in entries:
            visited_entries += 1
            if visited_entries > 2 * MAX_ARTIFACTS + 8:
                _fail("bundle directory tree contains too many entries")
            parts = (*prefix, entry.name)
            relative = "/".join(parts)
            _safe_relative_parts(relative, label="bundle entry path")
            try:
                metadata = entry.stat(follow_symlinks=False)
            except OSError:
                _fail("bundle entry metadata changed during enumeration")
            if stat.S_ISLNK(metadata.st_mode):
                _fail("bundle must not contain symbolic links")
            if stat.S_ISDIR(metadata.st_mode):
                stack.append((Path(entry.path), parts))
            elif stat.S_ISREG(metadata.st_mode):
                if metadata.st_nlink != 1:
                    _fail("bundle must not contain hard-linked files")
                files.append(relative)
                if len(files) > MAX_ARTIFACTS + 2:
                    _fail("bundle contains too many files")
            else:
                _fail("bundle contains a non-regular filesystem entry")
    return tuple(sorted(files))


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _require_object(value: Any, *, label: str, keys: Iterable[str]) -> dict[str, Any]:
    if type(value) is not dict:
        _fail(f"{label} must be an object")
    expected = frozenset(keys)
    actual = frozenset(value)
    if actual != expected:
        missing = sorted(expected - actual)
        unknown = sorted(actual - expected)
        suffix = []
        if missing:
            suffix.append("missing " + ",".join(missing))
        if unknown:
            suffix.append("unknown " + ",".join(unknown))
        _fail(f"{label} has an inexact field set ({'; '.join(suffix)})")
    return value


def _require_list(value: Any, *, label: str, length: int | None = None) -> list[Any]:
    if type(value) is not list:
        _fail(f"{label} must be an array")
    if length is not None and len(value) != length:
        _fail(f"{label} must contain exactly {length} entries")
    return value


def _require_string(value: Any, *, label: str, maximum: int = 256) -> str:
    if (
        type(value) is not str
        or not value
        or value != value.strip()
        or not value.isascii()
        or len(value.encode("ascii")) > maximum
        or any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value)
    ):
        _fail(f"{label} must be bounded canonical ASCII text")
    return value


def _require_id(value: Any, *, label: str) -> str:
    text = _require_string(value, label=label, maximum=128)
    if not _SAFE_ID_RE.fullmatch(text):
        _fail(f"{label} must use the canonical identifier alphabet")
    return text


def _require_int(value: Any, *, label: str, minimum: int = 0, maximum: int = 2**63 - 1) -> int:
    if type(value) is not int or value < minimum or value > maximum:
        _fail(f"{label} must be an integer in [{minimum}, {maximum}]")
    return value


def _require_true(value: Any, *, label: str) -> None:
    if value is not True:
        _fail(f"{label} must be true")


def _require_hex(value: Any, *, label: str, byte_length: int, nonzero: bool = True) -> str:
    if type(value) is not str or len(value) != byte_length * 2 or not _HEX_RE.fullmatch(value):
        _fail(f"{label} must be exactly {byte_length} bytes of lowercase hex without 0x")
    if nonzero and not any(bytes.fromhex(value)):
        _fail(f"{label} must not be zero")
    return value


def _require_optional_none(value: Any, *, label: str) -> None:
    if value is not None:
        _fail(f"{label} must be null")


def _require_pairwise_distinct(values: Sequence[tuple[str, str]]) -> None:
    seen: dict[str, str] = {}
    for label, value in values:
        previous = seen.get(value)
        if previous is not None:
            _fail(f"{label} must be distinct from {previous}")
        seen[value] = label


def _push_u32(value: int) -> bytes:
    return struct.pack("<I", value)


def _push_u64(value: int) -> bytes:
    return struct.pack("<Q", value)


def _length_prefixed(value: bytes) -> bytes:
    return _push_u32(len(value)) + value


_ED_Q = 2**255 - 19
_ED_L = 2**252 + 27742317777372353535851937790883648493
_ED_D = (-121665 * pow(121666, _ED_Q - 2, _ED_Q)) % _ED_Q
_ED_I = pow(2, (_ED_Q - 1) // 4, _ED_Q)
_ED_IDENTITY = (0, 1)


def _ed_xrecover(y: int) -> int | None:
    xx = (y * y - 1) * pow(_ED_D * y * y + 1, _ED_Q - 2, _ED_Q) % _ED_Q
    x = pow(xx, (_ED_Q + 3) // 8, _ED_Q)
    if (x * x - xx) % _ED_Q != 0:
        x = x * _ED_I % _ED_Q
    if (x * x - xx) % _ED_Q != 0:
        return None
    return x


def _ed_decode(encoded: bytes) -> tuple[int, int] | None:
    if len(encoded) != 32:
        return None
    raw = int.from_bytes(encoded, "little")
    sign_bit = raw >> 255
    y = raw & ((1 << 255) - 1)
    if y >= _ED_Q:
        return None
    x = _ed_xrecover(y)
    if x is None:
        return None
    if (x & 1) != sign_bit:
        x = (-x) % _ED_Q
    if x == 0 and sign_bit:
        return None
    point = (x, y)
    if _ed_encode(point) != encoded:
        return None
    return point


def _ed_encode(point: tuple[int, int]) -> bytes:
    x, y = point
    return (y | ((x & 1) << 255)).to_bytes(32, "little")


def _ed_extended(point: tuple[int, int]) -> tuple[int, int, int, int]:
    x, y = point
    return x, y, 1, x * y % _ED_Q


_ED_EXTENDED_IDENTITY = (0, 1, 1, 0)


def _ed_add_extended(
    left: tuple[int, int, int, int], right: tuple[int, int, int, int]
) -> tuple[int, int, int, int]:
    x1, y1, z1, t1 = left
    x2, y2, z2, t2 = right
    a = (y1 - x1) * (y2 - x2) % _ED_Q
    b = (y1 + x1) * (y2 + x2) % _ED_Q
    c = 2 * _ED_D * t1 * t2 % _ED_Q
    d = 2 * z1 * z2 % _ED_Q
    e = (b - a) % _ED_Q
    f = (d - c) % _ED_Q
    g = (d + c) % _ED_Q
    h = (b + a) % _ED_Q
    return e * f % _ED_Q, g * h % _ED_Q, f * g % _ED_Q, e * h % _ED_Q


def _ed_scalar_multiply_extended(
    point: tuple[int, int, int, int], scalar: int
) -> tuple[int, int, int, int]:
    result = _ED_EXTENDED_IDENTITY
    addend = point
    value = scalar
    while value:
        if value & 1:
            result = _ed_add_extended(result, addend)
        addend = _ed_add_extended(addend, addend)
        value >>= 1
    return result


def _ed_extended_equal(
    left: tuple[int, int, int, int], right: tuple[int, int, int, int]
) -> bool:
    return (
        left[0] * right[2] - right[0] * left[2]
    ) % _ED_Q == 0 and (
        left[1] * right[2] - right[1] * left[2]
    ) % _ED_Q == 0


def _ed_extended_to_affine(point: tuple[int, int, int, int]) -> tuple[int, int]:
    inverse = pow(point[2], _ED_Q - 2, _ED_Q)
    return point[0] * inverse % _ED_Q, point[1] * inverse % _ED_Q


def _ed_scalar_multiply(point: tuple[int, int], scalar: int) -> tuple[int, int]:
    return _ed_extended_to_affine(
        _ed_scalar_multiply_extended(_ed_extended(point), scalar)
    )


_ED_BASE_Y = 4 * pow(5, _ED_Q - 2, _ED_Q) % _ED_Q
_ED_BASE_X = _ed_xrecover(_ED_BASE_Y)
assert _ED_BASE_X is not None
if _ED_BASE_X & 1:
    _ED_BASE_X = _ED_Q - _ED_BASE_X
_ED_BASE = (_ED_BASE_X, _ED_BASE_Y)


def verify_ed25519(public_key: bytes, signature: bytes, message: bytes) -> bool:
    """Verify a strict, canonical, prime-subgroup Ed25519 signature."""

    if len(public_key) != 32 or len(signature) != 64:
        return False
    public_point = _ed_decode(public_key)
    r_point = _ed_decode(signature[:32])
    scalar = int.from_bytes(signature[32:], "little")
    if public_point is None or r_point is None or scalar >= _ED_L:
        return False
    if public_point == _ED_IDENTITY or r_point == _ED_IDENTITY:
        return False
    public_extended = _ed_extended(public_point)
    r_extended = _ed_extended(r_point)
    if not _ed_extended_equal(
        _ed_scalar_multiply_extended(public_extended, _ED_L),
        _ED_EXTENDED_IDENTITY,
    ):
        return False
    if not _ed_extended_equal(
        _ed_scalar_multiply_extended(r_extended, _ED_L),
        _ED_EXTENDED_IDENTITY,
    ):
        return False
    challenge = int.from_bytes(
        hashlib.sha512(signature[:32] + public_key + message).digest(), "little"
    ) % _ED_L
    return _ed_extended_equal(
        _ed_scalar_multiply_extended(_ed_extended(_ED_BASE), scalar),
        _ed_add_extended(
            r_extended,
            _ed_scalar_multiply_extended(public_extended, challenge),
        ),
    )


def evidence_signing_payload(evidence: Mapping[str, Any]) -> bytes:
    """Return the exact public payload external release signers must sign."""

    unsigned = dict(evidence)
    unsigned.pop("provenance", None)
    return SIGNING_DOMAIN + canonical_json_bytes(unsigned)


def circuit_policy_signing_payload(
    proof_system: Mapping[str, Any], report_sha256_hex: str
) -> bytes:
    """Return the payload independently signed by one circuit-policy auditor."""

    unsigned = dict(proof_system)
    unsigned.pop("audit_attestations", None)
    return (
        CIRCUIT_POLICY_SIGNING_DOMAIN
        + canonical_json_bytes(unsigned)
        + bytes.fromhex(report_sha256_hex)
    )


def _canonical_base64(value: Any, *, label: str, decoded_length: int) -> bytes:
    if type(value) is not str or value != value.strip() or not value.isascii():
        _fail(f"{label} must be canonical padded base64")
    try:
        decoded = base64.b64decode(value, validate=True)
    except (binascii.Error, ValueError):
        _fail(f"{label} must be canonical padded base64")
    if len(decoded) != decoded_length or base64.b64encode(decoded).decode("ascii") != value:
        _fail(f"{label} must decode to exactly {decoded_length} bytes")
    return decoded


def _secret_scan_variants(data: bytes) -> Iterable[str]:
    text = data.decode("utf-8", "ignore")
    variants = {text}
    current = text
    for _ in range(3):
        decoded = urllib.parse.unquote(current)
        if decoded == current:
            break
        variants.add(decoded)
        current = decoded
    variants.add(html.unescape(text))
    for item in tuple(variants):
        variants.add(unicodedata.normalize("NFKC", item))
    return variants


def reject_secret_material(data: bytes, *, label: str) -> None:
    """Reject public artifacts that contain common credential material markers."""

    for variant in _secret_scan_variants(data):
        if _SENSITIVE_RE.search(variant):
            _fail(f"{label} contains forbidden credential material")
    for token in _BASE64_TOKEN_RE.findall(data):
        try:
            decoded = base64.b64decode(token, validate=True)
        except (binascii.Error, ValueError):
            continue
        for variant in _secret_scan_variants(decoded):
            if _SENSITIVE_RE.search(variant):
                _fail(f"{label} contains encoded forbidden credential material")


def load_trust_policy(
    path: Path, *, allow_test_policy: bool = False
) -> tuple[dict[str, Any], bytes]:
    """Load a canonical external role-to-key trust root.

    Production callers never set ``allow_test_policy``. The separate fixture
    runner is the only entrypoint allowed to consume the deliberately distinct
    test policy schema.
    """

    data = read_direct_file(path, label="release trust policy", maximum=MAX_TRUST_POLICY_BYTES)
    value = parse_json_bytes(data, label="release trust policy", maximum=MAX_TRUST_POLICY_BYTES)
    require_canonical_json_file(data, value, label="release trust policy")
    policy = _require_object(
        value,
        label="release trust policy",
        keys=(
            "schema",
            "environment",
            "policy_id",
            "roles",
            "destination_attestors",
            "circuit_auditors",
            "proof_systems",
        ),
    )
    expected_schema = TEST_TRUST_POLICY_SCHEMA if allow_test_policy else TRUST_POLICY_SCHEMA
    expected_environment = "test-fixture" if allow_test_policy else "production"
    if policy["schema"] != expected_schema or policy["environment"] != expected_environment:
        _fail("release trust policy schema/environment is not valid for this entrypoint")
    _require_id(policy["policy_id"], label="release trust policy policy_id")
    roles = _require_list(policy["roles"], label="release trust policy roles", length=2)
    keys: set[str] = set()
    signer_ids: set[str] = set()
    for index, expected_role in enumerate(PROVENANCE_ROLES):
        entry = _require_object(
            roles[index],
            label=f"release trust policy roles[{index}]",
            keys=("role", "signer_id", "public_key_hex"),
        )
        if entry["role"] != expected_role:
            _fail("release trust policy roles must be exact and ordered")
        signer_id = _require_id(entry["signer_id"], label="trusted signer_id")
        key = _require_hex(entry["public_key_hex"], label="trusted public key", byte_length=32)
        if signer_id in signer_ids or signer_id in keys or key in keys or key in signer_ids:
            _fail("release trust policy roles must have distinct signer ids and keys")
        signer_ids.add(signer_id)
        keys.add(key)
        point = _ed_decode(bytes.fromhex(key))
        if (
            point is None
            or point == _ED_IDENTITY
            or _ed_scalar_multiply(point, _ED_L) != _ED_IDENTITY
        ):
            _fail("release trust policy contains an invalid Ed25519 public key")
    attestors = _require_list(
        policy["destination_attestors"],
        label="release trust policy destination_attestors",
        length=len(PROFILE_ORDER),
    )
    for index, expected_profile in enumerate(PROFILE_ORDER):
        entry = _require_object(
            attestors[index],
            label=f"release trust policy destination_attestors[{index}]",
            keys=("counterparty_profile", "attestor_id", "public_key_hex"),
        )
        if entry["counterparty_profile"] != expected_profile:
            _fail("destination attestors must cover exact production profiles in order")
        attestor_id = _require_id(entry["attestor_id"], label="destination attestor_id")
        key = _require_hex(
            entry["public_key_hex"], label="destination attestor public key", byte_length=32
        )
        if (
            attestor_id in signer_ids
            or attestor_id in keys
            or key in keys
            or key in signer_ids
        ):
            _fail("release signer and destination-attestor identities must be distinct")
        point = _ed_decode(bytes.fromhex(key))
        if (
            point is None
            or point == _ED_IDENTITY
            or _ed_scalar_multiply(point, _ED_L) != _ED_IDENTITY
        ):
            _fail("destination attestor has an invalid Ed25519 public key")
        signer_ids.add(attestor_id)
        keys.add(key)
    auditors = _require_list(
        policy["circuit_auditors"],
        label="release trust policy circuit_auditors",
        length=len(CIRCUIT_AUDITOR_ROLES),
    )
    for index, expected_role in enumerate(CIRCUIT_AUDITOR_ROLES):
        entry = _require_object(
            auditors[index],
            label=f"release trust policy circuit_auditors[{index}]",
            keys=("role", "auditor_id", "public_key_hex"),
        )
        if entry["role"] != expected_role:
            _fail("circuit auditor roles must be exact and ordered")
        auditor_id = _require_id(entry["auditor_id"], label="circuit auditor_id")
        key = _require_hex(
            entry["public_key_hex"], label="circuit auditor public key", byte_length=32
        )
        if (
            auditor_id in signer_ids
            or auditor_id in keys
            or key in keys
            or key in signer_ids
        ):
            _fail("every release trust-policy identity and key must be independent")
        point = _ed_decode(bytes.fromhex(key))
        if (
            point is None
            or point == _ED_IDENTITY
            or _ed_scalar_multiply(point, _ED_L) != _ED_IDENTITY
        ):
            _fail("circuit auditor has an invalid Ed25519 public key")
        signer_ids.add(auditor_id)
        keys.add(key)

    proof_systems = _require_list(
        policy["proof_systems"],
        label="release trust policy proof_systems",
        length=len(PROFILE_ORDER),
    )
    audit_signatures: set[bytes] = set()
    for index, expected_profile in enumerate(PROFILE_ORDER):
        proof = _require_object(
            proof_systems[index],
            label=f"release trust policy proof_systems[{index}]",
            keys=(
                "counterparty_profile",
                "circuit_id",
                "semantics",
                "circuit_artifact_sha256_hex",
                "verifier_key_hash_hex",
                "route_revision",
                "verifying_key_sha256_hex",
                "prover_build_sha256_hex",
                "toolchain_lock_sha256_hex",
                "destination_build",
                "audit_attestations",
            ),
        )
        if proof["counterparty_profile"] != expected_profile:
            _fail("proof systems must cover exact production profiles in order")
        circuit_id = _require_id(proof["circuit_id"], label="proof-system circuit_id")
        if "smoke" in circuit_id or "test" in circuit_id:
            _fail("production proof policy must not approve smoke or test circuits")
        semantics = _require_list(
            proof["semantics"], label="proof-system semantics", length=2
        )
        if semantics != ["nexus-finality-v1", "sccp-exact-statement-v1"]:
            _fail("proof-system semantics must bind Nexus finality and the exact statement")
        for field in (
            "circuit_artifact_sha256_hex",
            "verifier_key_hash_hex",
            "verifying_key_sha256_hex",
            "prover_build_sha256_hex",
            "toolchain_lock_sha256_hex",
        ):
            _require_hex(proof[field], label=f"proof-system {field}", byte_length=32)
        _require_int(
            proof["route_revision"],
            label="proof-system route_revision",
            minimum=1,
            maximum=2**32 - 1,
        )
        destination_build = _require_object(
            proof["destination_build"],
            label="proof-system destination_build",
            keys=(
                "source_bundle_sha256_hex",
                "compiler_build_sha256_hex",
                "token_artifact_sha256_hex",
                "token_interface_sha256_hex",
                "token_runtime_hash_hex",
                "verifier_artifact_sha256_hex",
                "verifier_interface_sha256_hex",
                "verifier_runtime_hash_hex",
                "route_artifact_sha256_hex",
                "route_interface_sha256_hex",
                "route_runtime_hash_hex",
            ),
        )
        for field, digest in destination_build.items():
            _require_hex(digest, label=f"destination build {field}", byte_length=32)
        _require_pairwise_distinct(tuple(destination_build.items()))
        if proof["verifier_key_hash_hex"] == FORBIDDEN_ALGEBRAIC_SMOKE_VK:
            _fail("algebraic SCCP smoke-test verifying key is forbidden in release policy")
        attestations = _require_list(
            proof["audit_attestations"],
            label="proof-system audit_attestations",
            length=len(CIRCUIT_AUDITOR_ROLES),
        )
        for audit_index, expected_role in enumerate(CIRCUIT_AUDITOR_ROLES):
            trusted = auditors[audit_index]
            audit = _require_object(
                attestations[audit_index],
                label=f"proof-system audit_attestations[{audit_index}]",
                keys=(
                    "role",
                    "auditor_id",
                    "algorithm",
                    "public_key_hex",
                    "report_sha256_hex",
                    "signature_b64",
                ),
            )
            if (
                audit["role"] != expected_role
                or audit["auditor_id"] != trusted["auditor_id"]
                or audit["public_key_hex"] != trusted["public_key_hex"]
                or audit["algorithm"] != "ed25519"
            ):
                _fail("proof-system audit does not match the independent trusted auditor")
            report_hash = _require_hex(
                audit["report_sha256_hex"], label="circuit audit report hash", byte_length=32
            )
            signature = _canonical_base64(
                audit["signature_b64"],
                label="circuit audit signature",
                decoded_length=64,
            )
            if signature in audit_signatures:
                _fail("circuit audit signatures must be unique")
            audit_signatures.add(signature)
            if not verify_ed25519(
                bytes.fromhex(trusted["public_key_hex"]),
                signature,
                circuit_policy_signing_payload(proof, report_hash),
            ):
                _fail("proof-system audit has an invalid detached signature")
    reject_secret_material(data, label="release trust policy")
    return policy, data


def _validate_validator_identity(value: Any) -> dict[str, Any]:
    identity = _require_object(
        value,
        label="validator identity",
        keys=("protocol_version", "crate_version", "source_sha256_hex", "build_identity_hex"),
    )
    if identity["protocol_version"] != 1:
        _fail("validator protocol_version must be exactly 1")
    crate_version = _require_string(identity["crate_version"], label="validator crate_version")
    if not crate_version.isascii() or not _SAFE_VERSION_RE.fullmatch(crate_version):
        _fail("validator crate_version must be a canonical semantic version")
    source_hash = _require_hex(
        identity["source_sha256_hex"], label="validator source_sha256_hex", byte_length=32
    )
    build_identity = _require_hex(
        identity["build_identity_hex"], label="validator build_identity_hex", byte_length=32
    )
    expected = hashlib.sha256(
        VALIDATOR_BUILD_ID_DOMAIN + bytes.fromhex(source_hash) + crate_version.encode("ascii")
    ).hexdigest()
    if build_identity != expected:
        _fail("validator build identity does not bind its source and crate version")
    source = read_direct_file(
        RUST_VALIDATOR_SOURCE,
        label="canonical Rust release validator source",
        maximum=2 * 1024 * 1024,
    )
    if source_hash != sha256_hex(source):
        _fail("validator identity does not match the canonical Rust validator source")
    return identity


def _validate_lanes(
    value: Any, artifact_by_path: Mapping[str, Mapping[str, Any]]
) -> set[str]:
    lanes = _require_list(value, label="lanes", length=len(PROFILE_ORDER))
    referenced: set[str] = set()
    for index, expected_profile in enumerate(PROFILE_ORDER):
        lane = _require_object(
            lanes[index],
            label=f"lanes[{index}]",
            keys=(
                "counterparty_profile",
                "counterparty_domain",
                "inbound_status",
                "outbound_status",
                "evidence_artifact_path",
            ),
        )
        if lane["counterparty_profile"] != expected_profile:
            _fail("lanes must contain exact production profiles in canonical order")
        if lane["counterparty_domain"] != PROFILE_DOMAINS[expected_profile]:
            _fail(f"{expected_profile} counterparty domain is not canonical")
        for direction in ("inbound_status", "outbound_status"):
            if lane[direction] not in ("verified", "unavailable"):
                _fail(f"{expected_profile} {direction} must be verified or unavailable")
        path = lane["evidence_artifact_path"]
        _safe_relative_parts(path, label=f"{expected_profile} lane evidence path")
        artifact = artifact_by_path.get(path)
        if artifact is None or artifact["kind"] != "lane-evidence":
            _fail(f"{expected_profile} must reference one lane-evidence artifact")
        if path in referenced:
            _fail("each SCCP profile must use a distinct typed lane-evidence artifact")
        referenced.add(path)
    return referenced


def _validate_artifacts(value: Any) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    artifacts = _require_list(value, label="artifacts")
    if not artifacts or len(artifacts) > MAX_ARTIFACTS:
        _fail(f"artifacts must contain between 1 and {MAX_ARTIFACTS} entries")
    parsed: list[dict[str, Any]] = []
    by_path: dict[str, dict[str, Any]] = {}
    seen_hashes: dict[str, str] = {}
    total = 0
    previous_path = ""
    for index, value_item in enumerate(artifacts):
        item = _require_object(
            value_item,
            label=f"artifacts[{index}]",
            keys=("path", "kind", "sha256_hex", "size_bytes"),
        )
        path = item["path"]
        _safe_relative_parts(path, label=f"artifacts[{index}].path")
        if path <= previous_path:
            _fail("artifacts must be strictly sorted by unique path")
        previous_path = path
        kind = _require_string(item["kind"], label=f"artifacts[{index}].kind")
        if kind not in ARTIFACT_KINDS:
            _fail("artifact kind is not part of the SCCP V1 release schema")
        digest = _require_hex(item["sha256_hex"], label=f"artifacts[{index}].sha256_hex", byte_length=32)
        limit = MAX_TRANSCRIPT_BYTES if kind == "phase-transcript" else MAX_ARTIFACT_BYTES
        size = _require_int(item["size_bytes"], label=f"artifacts[{index}].size_bytes", minimum=1, maximum=limit)
        total += size
        if total > MAX_TOTAL_ARTIFACT_BYTES:
            _fail("artifact total size exceeds the SCCP release limit")
        if digest in seen_hashes:
            _fail(f"artifact digest for {path} reuses the digest of {seen_hashes[digest]}")
        seen_hashes[digest] = path
        by_path[path] = item
        parsed.append(item)
    return parsed, by_path


def _validate_validation(value: Any, artifact_by_path: Mapping[str, Mapping[str, Any]]) -> set[str]:
    validation = _require_object(
        value,
        label="validation",
        keys=("corridor", "phases"),
    )
    if validation["corridor"] != "sccp-production-corridor-v1":
        _fail("validation.corridor must select the exact V1 corridor")
    phases = _require_list(validation["phases"], label="validation.phases", length=len(REQUIRED_PHASES))
    referenced: set[str] = set()
    for index, expected_name in enumerate(REQUIRED_PHASES):
        phase = _require_object(
            phases[index],
            label=f"validation.phases[{index}]",
            keys=("name", "status", "artifact_path"),
        )
        if phase["name"] != expected_name or phase["status"] != "passed":
            _fail(f"validation phase {index} must be passed {expected_name}")
        path = phase["artifact_path"]
        _safe_relative_parts(path, label=f"validation phase {expected_name} artifact path")
        artifact = artifact_by_path.get(path)
        if artifact is None or artifact["kind"] != "phase-transcript":
            _fail(f"validation phase {expected_name} must reference a phase-transcript artifact")
        if path in referenced:
            _fail("validation phases must reference distinct transcript artifacts")
        referenced.add(path)
    return referenced


def _validate_provenance(
    value: Any,
    evidence: Mapping[str, Any],
    trust_policy: Mapping[str, Any],
) -> None:
    provenance = _require_list(value, label="provenance", length=len(PROVENANCE_ROLES))
    payload = evidence_signing_payload(evidence)
    public_keys: set[bytes] = set()
    signatures: set[bytes] = set()
    for index, expected_role in enumerate(PROVENANCE_ROLES):
        trusted = trust_policy["roles"][index]
        entry = _require_object(
            provenance[index],
            label=f"provenance[{index}]",
            keys=("role", "signer_id", "algorithm", "public_key_hex", "signature_b64"),
        )
        if entry["role"] != expected_role:
            _fail("provenance roles must be exact, ordered, and independently signed")
        signer_id = _require_id(entry["signer_id"], label=f"provenance[{index}].signer_id")
        if signer_id != trusted["signer_id"]:
            _fail(f"provenance[{index}] signer is not trusted for {expected_role}")
        if entry["algorithm"] != "ed25519":
            _fail("provenance algorithm must be exactly ed25519")
        public_key_hex = _require_hex(
            entry["public_key_hex"],
            label=f"provenance[{index}].public_key_hex",
            byte_length=32,
        )
        if public_key_hex != trusted["public_key_hex"]:
            _fail(f"provenance[{index}] key is not trusted for {expected_role}")
        public_key = bytes.fromhex(public_key_hex)
        signature = _canonical_base64(
            entry["signature_b64"], label=f"provenance[{index}].signature_b64", decoded_length=64
        )
        if public_key in public_keys or signature in signatures:
            _fail("provenance roles must use distinct keys and signatures")
        public_keys.add(public_key)
        signatures.add(signature)
        if not verify_ed25519(public_key, signature, payload):
            _fail(f"provenance[{index}] has an invalid detached Ed25519 signature")


def validate_evidence(
    value: Any, trust_policy: Mapping[str, Any]
) -> dict[str, Any]:
    """Validate one complete SCCP release document against an external trust root."""

    evidence = _require_object(
        value,
        label="release evidence",
        keys=(
            "schema",
            "release_id",
            "protocol_version",
            "hub_profile",
            "hub_chain_id",
            "created_at_unix_ms",
            "trust_policy_id",
            "validator",
            "lanes",
            "artifacts",
            "validation",
            "provenance",
        ),
    )
    if evidence["schema"] != EVIDENCE_SCHEMA:
        _fail(f"release evidence schema must be exactly {EVIDENCE_SCHEMA}")
    _require_id(evidence["release_id"], label="release_id")
    if evidence["protocol_version"] != 1:
        _fail("protocol_version must be exactly 1")
    hub_profile = _require_string(evidence["hub_profile"], label="hub_profile")
    if hub_profile not in HUB_CHAIN_IDS or evidence["hub_chain_id"] != HUB_CHAIN_IDS[hub_profile]:
        _fail("hub profile and chain id must identify an exact SCCP V1 SORA network")
    _require_int(evidence["created_at_unix_ms"], label="created_at_unix_ms", minimum=1)
    if evidence["trust_policy_id"] != trust_policy["policy_id"]:
        _fail("release evidence trust_policy_id does not match the external trust policy")
    _validate_validator_identity(evidence["validator"])
    artifacts, artifact_by_path = _validate_artifacts(evidence["artifacts"])
    referenced = _validate_validation(evidence["validation"], artifact_by_path)
    referenced |= _validate_lanes(evidence["lanes"], artifact_by_path)
    if referenced != set(artifact_by_path):
        missing = sorted(set(artifact_by_path) - referenced)
        _fail("release evidence contains unreferenced artifacts: " + ",".join(missing))
    _validate_provenance(evidence["provenance"], evidence, trust_policy)
    reject_secret_material(canonical_json_bytes(evidence), label="release evidence")
    return evidence


def load_evidence_file(
    path: Path, trust_policy: Mapping[str, Any]
) -> tuple[dict[str, Any], bytes]:
    data = read_direct_file(path, label="release evidence", maximum=MAX_EVIDENCE_BYTES)
    value = parse_json_bytes(data, label="release evidence", maximum=MAX_EVIDENCE_BYTES)
    require_canonical_json_file(data, value, label="release evidence")
    return validate_evidence(value, trust_policy), data


def verify_evidence_artifacts(
    evidence: Mapping[str, Any], artifact_root: Path
) -> dict[str, bytes]:
    """Read and verify every evidence-bound public artifact."""

    contents: dict[str, bytes] = {}
    total = 0
    for entry in evidence["artifacts"]:
        limit = MAX_TRANSCRIPT_BYTES if entry["kind"] == "phase-transcript" else MAX_ARTIFACT_BYTES
        data = read_relative_file(
            artifact_root,
            entry["path"],
            label=f"artifact {entry['path']}",
            maximum=limit,
        )
        total += len(data)
        if total > MAX_TOTAL_ARTIFACT_BYTES:
            _fail("artifact total size exceeds the SCCP release limit")
        if len(data) != entry["size_bytes"] or sha256_hex(data) != entry["sha256_hex"]:
            _fail(f"artifact {entry['path']} does not match its signed size and SHA-256")
        reject_secret_material(data, label=f"artifact {entry['path']}")
        contents[entry["path"]] = data
    return contents


def _read_validator_executable(path: Path) -> bytes:
    try:
        mode = path.lstat().st_mode
    except OSError:
        _fail("canonical Rust release validator is not accessible")
    if os.name != "nt" and mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH) == 0:
        _fail("canonical Rust release validator must be executable")
    return read_direct_file(
        path,
        label="canonical Rust release validator",
        maximum=MAX_VALIDATOR_BINARY_BYTES,
    )


def _bounded_pipe_reader(
    pipe: Any, maximum: int, result: list[bytes], overflow: list[bool]
) -> None:
    chunks: list[bytes] = []
    remaining = maximum + 1
    try:
        while remaining:
            chunk = pipe.read(min(8192, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
    finally:
        pipe.close()
    data = b"".join(chunks)
    overflow.append(len(data) > maximum)
    result.append(data[:maximum])


def _invoke_validator_command(
    validator: Path, arguments: Sequence[str]
) -> tuple[bytes, bytes, int, str]:
    safe_environment = {"PATH": os.defpath, "LANG": "C", "LC_ALL": "C", "TZ": "UTC"}
    for name in ("SYSTEMROOT", "WINDIR"):
        if name in os.environ:
            safe_environment[name] = os.environ[name]
    validator_descriptor: int | None = None
    executed_validator_hash: str
    executable_path = str(validator.absolute())
    popen_extra: dict[str, Any] = {}
    if sys.platform.startswith("linux"):
        flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
        try:
            validator_descriptor = os.open(validator, flags)
            metadata = os.fstat(validator_descriptor)
            if (
                not stat.S_ISREG(metadata.st_mode)
                or metadata.st_nlink != 1
                or metadata.st_size <= 0
                or metadata.st_size > MAX_VALIDATOR_BINARY_BYTES
                or metadata.st_mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH) == 0
            ):
                os.close(validator_descriptor)
                validator_descriptor = None
                _fail("canonical Rust release validator changed before execution")
            chunks: list[bytes] = []
            remaining = MAX_VALIDATOR_BINARY_BYTES + 1
            while remaining:
                chunk = os.read(validator_descriptor, min(1024 * 1024, remaining))
                if not chunk:
                    break
                chunks.append(chunk)
                remaining -= len(chunk)
            executed_bytes = b"".join(chunks)
            if len(executed_bytes) != metadata.st_size:
                os.close(validator_descriptor)
                validator_descriptor = None
                _fail("canonical Rust release validator changed while opening")
            executed_validator_hash = sha256_hex(executed_bytes)
            os.lseek(validator_descriptor, 0, os.SEEK_SET)
            executable_path = f"/proc/self/fd/{validator_descriptor}"
            popen_extra["pass_fds"] = (validator_descriptor,)
        except OSError:
            if validator_descriptor is not None:
                os.close(validator_descriptor)
            _fail("canonical Rust release validator could not be opened for execution")
    else:
        executed_validator_hash = sha256_hex(_read_validator_executable(validator))
    try:
        process = subprocess.Popen(
            [executable_path, *arguments],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=safe_environment,
            shell=False,
            **popen_extra,
        )
    except OSError:
        if validator_descriptor is not None:
            os.close(validator_descriptor)
        _fail("canonical Rust release validator could not be started")
    if validator_descriptor is not None:
        os.close(validator_descriptor)
    assert process.stdout is not None and process.stderr is not None
    stdout: list[bytes] = []
    stderr: list[bytes] = []
    stdout_overflow: list[bool] = []
    stderr_overflow: list[bool] = []
    threads = (
        threading.Thread(
            target=_bounded_pipe_reader,
            args=(process.stdout, MAX_VALIDATOR_OUTPUT_BYTES, stdout, stdout_overflow),
            daemon=True,
        ),
        threading.Thread(
            target=_bounded_pipe_reader,
            args=(process.stderr, MAX_VALIDATOR_ERROR_BYTES, stderr, stderr_overflow),
            daemon=True,
        ),
    )
    for thread in threads:
        thread.start()
    try:
        return_code = process.wait(timeout=MAX_VALIDATOR_SECONDS)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
        for thread in threads:
            thread.join(timeout=1)
        _fail("canonical Rust release validator exceeded its time limit")
    for thread in threads:
        thread.join(timeout=1)
    if any(thread.is_alive() for thread in threads):
        _fail("canonical Rust release validator output did not close")
    if stdout_overflow != [False] or stderr_overflow != [False]:
        _fail("canonical Rust release validator exceeded its output limit")
    return stdout[0], stderr[0], return_code, executed_validator_hash


def _invoke_lane_validator(
    validator: Path,
    artifact: Path,
    trust_policy_path: Path,
    evidence_path: Path,
    environment: str,
) -> tuple[bytes, bytes, int, str]:
    return _invoke_validator_command(
        validator,
        (
            "validate",
            str(artifact.absolute()),
            str(trust_policy_path.absolute()),
            str(evidence_path.absolute()),
            environment,
        ),
    )


def verify_rust_release_signatures(
    *,
    trust_policy_path: Path,
    trust_policy: Mapping[str, Any],
    trust_policy_bytes: bytes,
    evidence_path: Path,
    evidence: Mapping[str, Any],
    evidence_bytes: bytes,
    validator_path: Path,
    environment: str,
) -> tuple[dict[str, Any], str]:
    """Require Rust/iroha_crypto to independently verify every trust signature."""

    if environment not in ("production", "test-fixture"):
        _fail("Rust release signature environment is invalid")
    executable_hash = sha256_hex(_read_validator_executable(validator_path))
    stdout, stderr, return_code, executed_hash = _invoke_validator_command(
        validator_path,
        (
            "validate-release",
            str(trust_policy_path.absolute()),
            str(evidence_path.absolute()),
            environment,
        ),
    )
    if executed_hash != executable_hash:
        _fail("canonical Rust release validator changed before signature validation")
    if return_code != 0:
        detail = public_error(stderr.decode("utf-8", "replace"))
        _fail(f"canonical Rust release signature validation failed: {detail}")
    if stderr or not stdout.endswith(b"\n") or stdout.count(b"\n") != 1:
        _fail("canonical Rust release signature validator emitted invalid output")
    value = parse_json_bytes(
        stdout[:-1],
        label="Rust release signature receipt",
        maximum=MAX_VALIDATOR_OUTPUT_BYTES,
    )
    receipt = _require_object(
        value,
        label="Rust release signature receipt",
        keys=(
            "schema",
            "environment",
            "policy_id",
            "release_id",
            "policy_sha256_hex",
            "evidence_sha256_hex",
            "release_signatures_verified",
            "circuit_audit_signatures_verified",
            "destination_attestors_validated",
            "distinct_trust_identities",
        ),
    )
    if (
        receipt["schema"] != "sccp-release-signature-validation-v1"
        or receipt["environment"] != environment
        or receipt["policy_id"] != trust_policy["policy_id"]
        or receipt["release_id"] != evidence["release_id"]
        or receipt["policy_sha256_hex"] != sha256_hex(trust_policy_bytes)
        or receipt["evidence_sha256_hex"] != sha256_hex(evidence_bytes)
        or receipt["release_signatures_verified"] != len(PROVENANCE_ROLES)
        or receipt["circuit_audit_signatures_verified"]
        != len(PROFILE_ORDER) * len(CIRCUIT_AUDITOR_ROLES)
        or receipt["destination_attestors_validated"] != len(PROFILE_ORDER)
        or receipt["distinct_trust_identities"]
        != len(PROVENANCE_ROLES) + len(PROFILE_ORDER) + len(CIRCUIT_AUDITOR_ROLES)
    ):
        _fail("Rust release signature receipt does not match exact trusted inputs")
    if sha256_hex(_read_validator_executable(validator_path)) != executable_hash:
        _fail("canonical Rust release validator changed during signature validation")
    return receipt, executable_hash


def _validate_rust_receipt(
    value: Any,
    *,
    evidence: Mapping[str, Any],
    lane: Mapping[str, Any],
    artifact: Mapping[str, Any],
) -> dict[str, Any]:
    receipt = _require_object(
        value,
        label="Rust lane validation receipt",
        keys=(
            "schema",
            "validator",
            "trust_policy_id",
            "trust_policy_sha256_hex",
            "release_id",
            "release_evidence_sha256_hex",
            "artifact_sha256_hex",
            "profile",
            "inbound_status",
            "outbound_status",
            "unavailable_reasons",
            "source_profile",
            "target_profile",
            "lane_hash_hex",
            "source_identity_hash_hex",
            "native_anchor_hash_hex",
            "message_id_hex",
            "payload_hash_hex",
            "source_event_digest_hex",
            "finality_height",
            "finality_block_hash_hex",
            "destination_attestor_id",
            "destination_statement_sha256_hex",
            "destination_observed_at_unix_ms",
            "destination_finality_height",
            "destination_finality_block_hash_hex",
            "destination_binding_hash_hex",
            "route_configuration_hash_hex",
            "governed_route_configuration_hash_hex",
            "verifier_key_hash_hex",
            "route_revision",
            "verifying_key_sha256_hex",
            "semantic_circuit_id",
            "circuit_artifact_sha256_hex",
            "prover_build_sha256_hex",
            "toolchain_lock_sha256_hex",
            "destination_build_policy_sha256_hex",
        ),
    )
    if receipt["schema"] != RUST_VALIDATION_SCHEMA:
        _fail("Rust lane validation receipt has the wrong schema")
    identity = _validate_validator_identity(receipt["validator"])
    if identity != evidence["validator"]:
        _fail("Rust lane validator identity does not match signed evidence")
    if (
        receipt["release_id"] != evidence["release_id"]
        or receipt["release_evidence_sha256_hex"]
        != sha256_hex(canonical_json_file_bytes(evidence))
    ):
        _fail("Rust lane validator used different signed release evidence")
    _require_id(receipt["trust_policy_id"], label="Rust receipt trust_policy_id")
    _require_hex(
        receipt["trust_policy_sha256_hex"],
        label="Rust receipt trust_policy_sha256_hex",
        byte_length=32,
    )
    if receipt["artifact_sha256_hex"] != artifact["sha256_hex"]:
        _fail("Rust lane validator did not validate the signed artifact bytes")
    if receipt["profile"] != lane["counterparty_profile"]:
        _fail("Rust lane validator returned the wrong counterparty profile")
    if receipt["inbound_status"] != lane["inbound_status"]:
        _fail("Rust lane inbound result does not match signed evidence")
    if receipt["outbound_status"] != lane["outbound_status"]:
        _fail("Rust lane outbound result does not match signed evidence")

    reasons = _require_list(
        receipt["unavailable_reasons"], label="Rust unavailable reasons"
    )
    expected_reason_count = int(receipt["outbound_status"] == "unavailable") + int(
        receipt["inbound_status"] == "unavailable"
    )
    if len(reasons) != expected_reason_count:
        _fail("Rust lane receipt has the wrong unavailable reason count")
    for reason in reasons:
        if (
            type(reason) is not str
            or len(reason) > 160
            or not _UNAVAILABLE_REASON_RE.fullmatch(reason)
        ):
            _fail("Rust lane receipt contains a non-canonical unavailable reason")
    position = 0
    if receipt["outbound_status"] == "unavailable":
        if not reasons or reasons[0] != OUTBOUND_UNAVAILABLE_REASON:
            _fail("Rust lane receipt does not use the exact outbound fail-closed reason")
        position = 1
    if receipt["inbound_status"] == "unavailable":
        expected = UNAVAILABLE_INBOUND_REASONS.get(lane["counterparty_profile"])
        if expected is not None and reasons[position] != expected:
            _fail("Rust lane receipt does not use the exact inbound fail-closed reason")

    detail_fields = (
        "source_profile",
        "target_profile",
        "lane_hash_hex",
        "source_identity_hash_hex",
        "native_anchor_hash_hex",
        "message_id_hex",
        "payload_hash_hex",
        "source_event_digest_hex",
        "finality_height",
        "finality_block_hash_hex",
    )
    if receipt["inbound_status"] == "unavailable":
        for field in detail_fields:
            _require_optional_none(receipt[field], label=f"Rust receipt {field}")
    else:
        if receipt["source_profile"] != lane["counterparty_profile"]:
            _fail("verified inbound source profile does not match the signed lane")
        if receipt["target_profile"] != evidence["hub_profile"]:
            _fail("verified inbound target profile does not match the signed SORA hub")
        for field in (
            "lane_hash_hex",
            "source_identity_hash_hex",
            "native_anchor_hash_hex",
            "message_id_hex",
            "payload_hash_hex",
            "source_event_digest_hex",
            "finality_block_hash_hex",
        ):
            _require_hex(receipt[field], label=f"Rust receipt {field}", byte_length=32)
        finality_height = receipt["finality_height"]
        if (
            type(finality_height) is not str
            or not re.fullmatch(r"[1-9][0-9]{0,19}", finality_height)
            or int(finality_height) > 2**64 - 1
        ):
            _fail("Rust receipt finality_height must be a canonical positive u64")

    destination_fields = (
        "destination_attestor_id",
        "destination_statement_sha256_hex",
        "destination_observed_at_unix_ms",
        "destination_finality_height",
        "destination_finality_block_hash_hex",
        "destination_binding_hash_hex",
        "route_configuration_hash_hex",
        "governed_route_configuration_hash_hex",
        "verifier_key_hash_hex",
        "route_revision",
        "verifying_key_sha256_hex",
        "semantic_circuit_id",
        "circuit_artifact_sha256_hex",
        "prover_build_sha256_hex",
        "toolchain_lock_sha256_hex",
        "destination_build_policy_sha256_hex",
    )
    if receipt["outbound_status"] == "unavailable":
        for field in destination_fields:
            _require_optional_none(receipt[field], label=f"Rust receipt {field}")
    else:
        _require_id(
            receipt["destination_attestor_id"],
            label="Rust receipt destination_attestor_id",
        )
        circuit_id = _require_id(
            receipt["semantic_circuit_id"], label="Rust receipt semantic_circuit_id"
        )
        if "smoke" in circuit_id or "test" in circuit_id:
            _fail("Rust receipt selected a forbidden smoke or test circuit")
        for field in (
            "destination_statement_sha256_hex",
            "destination_finality_block_hash_hex",
            "destination_binding_hash_hex",
            "route_configuration_hash_hex",
            "governed_route_configuration_hash_hex",
            "verifier_key_hash_hex",
            "verifying_key_sha256_hex",
            "circuit_artifact_sha256_hex",
            "prover_build_sha256_hex",
            "toolchain_lock_sha256_hex",
            "destination_build_policy_sha256_hex",
        ):
            _require_hex(receipt[field], label=f"Rust receipt {field}", byte_length=32)
        for field in (
            "destination_observed_at_unix_ms",
            "destination_finality_height",
            "route_revision",
        ):
            text = receipt[field]
            if (
                type(text) is not str
                or not re.fullmatch(r"[1-9][0-9]{0,19}", text)
                or int(text) > 2**64 - 1
            ):
                _fail(f"Rust receipt {field} must be a canonical positive u64")
        if int(receipt["route_revision"]) > 2**32 - 1:
            _fail("Rust receipt route_revision exceeds u32")
        observed = int(receipt["destination_observed_at_unix_ms"])
        created = evidence["created_at_unix_ms"]
        if observed > created or created - observed > MAX_DESTINATION_ATTESTATION_AGE_MS:
            _fail("destination state attestation is future-dated or stale")
    return receipt


def verify_rust_lane_evidence(
    evidence: Mapping[str, Any],
    artifact_root: Path,
    validator_path: Path,
    trust_policy: Mapping[str, Any],
    *,
    trust_policy_path: Path,
    evidence_path: Path,
    environment: str,
) -> tuple[list[dict[str, Any]], str]:
    """Independently validate every typed lane artifact with the Rust verifier."""

    if (
        environment not in ("production", "test-fixture")
        or trust_policy["environment"] != environment
    ):
        _fail("Rust lane validation environment does not match the trust policy")

    executable = _read_validator_executable(validator_path)
    executable_hash = sha256_hex(executable)
    artifacts = {entry["path"]: entry for entry in evidence["artifacts"]}
    receipts: list[dict[str, Any]] = []
    attestors = {
        entry["counterparty_profile"]: entry
        for entry in trust_policy["destination_attestors"]
    }
    proof_systems = {
        entry["counterparty_profile"]: entry
        for entry in trust_policy["proof_systems"]
    }
    for lane in evidence["lanes"]:
        relative = lane["evidence_artifact_path"]
        artifact = artifacts[relative]
        parts = _safe_relative_parts(relative, label="typed lane evidence path")
        artifact_path = artifact_root.joinpath(*parts)
        attestor = attestors[lane["counterparty_profile"]]
        proof_system = proof_systems[lane["counterparty_profile"]]
        stdout, stderr, return_code, executed_hash = _invoke_lane_validator(
            validator_path,
            artifact_path,
            trust_policy_path,
            evidence_path,
            environment,
        )
        if executed_hash != executable_hash:
            _fail("canonical Rust release validator changed before execution")
        if return_code != 0:
            detail = public_error(stderr.decode("utf-8", "replace"))
            _fail(f"canonical Rust lane validation failed: {detail}")
        if stderr:
            _fail("canonical Rust lane validator wrote unexpected stderr")
        if not stdout.endswith(b"\n") or stdout.count(b"\n") != 1:
            _fail("canonical Rust lane validator must emit exactly one JSON line")
        value = parse_json_bytes(
            stdout[:-1],
            label="Rust lane validation receipt",
            maximum=MAX_VALIDATOR_OUTPUT_BYTES,
        )
        receipt = _validate_rust_receipt(
            value,
            evidence=evidence,
            lane=lane,
            artifact=artifact,
        )
        if (
            receipt["trust_policy_id"] != trust_policy["policy_id"]
            or receipt["trust_policy_sha256_hex"]
            != sha256_hex(canonical_json_file_bytes(trust_policy))
            or (
                receipt["outbound_status"] == "verified"
                and (
                    receipt["destination_attestor_id"] != attestor["attestor_id"]
                    or receipt["semantic_circuit_id"] != proof_system["circuit_id"]
                    or receipt["circuit_artifact_sha256_hex"]
                    != proof_system["circuit_artifact_sha256_hex"]
                    or receipt["verifier_key_hash_hex"]
                    != proof_system["verifier_key_hash_hex"]
                    or receipt["route_revision"] != str(proof_system["route_revision"])
                    or receipt["verifying_key_sha256_hex"]
                    != proof_system["verifying_key_sha256_hex"]
                    or receipt["prover_build_sha256_hex"]
                    != proof_system["prover_build_sha256_hex"]
                    or receipt["toolchain_lock_sha256_hex"]
                    != proof_system["toolchain_lock_sha256_hex"]
                    or receipt["destination_build_policy_sha256_hex"]
                    != sha256_hex(canonical_json_bytes(proof_system["destination_build"]))
                )
            )
        ):
            _fail("Rust destination validation does not match external trust policy")
        receipts.append(receipt)
        post_bytes = read_relative_file(
            artifact_root,
            relative,
            label=f"artifact {relative} after Rust validation",
            maximum=MAX_ARTIFACT_BYTES,
        )
        if sha256_hex(post_bytes) != artifact["sha256_hex"]:
            _fail("typed lane artifact changed during Rust validation")
    if sha256_hex(_read_validator_executable(validator_path)) != executable_hash:
        _fail("canonical Rust release validator changed during validation")
    return receipts, executable_hash


def bundle_root_hash_hex(
    entries: Sequence[Mapping[str, Any]],
    *,
    trust_policy_id: str,
    trust_policy_sha256_hex: str,
    validator: Mapping[str, Any],
    validator_executable_sha256_hex: str,
) -> str:
    """Hash trust roots, validator identity, and sorted entries with framing."""

    payload = bytearray(BUNDLE_HASH_DOMAIN)
    payload.extend(_length_prefixed(trust_policy_id.encode("ascii")))
    payload.extend(bytes.fromhex(trust_policy_sha256_hex))
    payload.extend(_length_prefixed(canonical_json_bytes(validator)))
    payload.extend(bytes.fromhex(validator_executable_sha256_hex))
    payload.extend(_push_u32(len(entries)))
    previous = ""
    for entry in entries:
        path = entry["path"]
        kind = entry["kind"]
        if path <= previous:
            _fail("bundle entries must be strictly sorted by path")
        previous = path
        path_bytes = path.encode("ascii")
        kind_bytes = kind.encode("ascii")
        payload.extend(_length_prefixed(path_bytes))
        payload.extend(_length_prefixed(kind_bytes))
        payload.extend(_push_u64(entry["size_bytes"]))
        payload.extend(bytes.fromhex(entry["sha256_hex"]))
    return hashlib.sha256(payload).hexdigest()


def validate_bundle_index(value: Any) -> dict[str, Any]:
    """Validate the exact deterministic SCCP release-bundle index schema."""

    index = _require_object(
        value,
        label="bundle index",
        keys=(
            "schema",
            "release_id",
            "evidence_path",
            "trust_policy_id",
            "trust_policy_sha256_hex",
            "validator",
            "validator_executable_sha256_hex",
            "entries",
            "bundle_root_hash_hex",
        ),
    )
    if index["schema"] != BUNDLE_SCHEMA:
        _fail(f"bundle index schema must be exactly {BUNDLE_SCHEMA}")
    _require_id(index["release_id"], label="bundle release_id")
    if index["evidence_path"] != "evidence.json":
        _fail("bundle evidence_path must be exactly evidence.json")
    _require_id(index["trust_policy_id"], label="bundle trust_policy_id")
    _require_hex(
        index["trust_policy_sha256_hex"],
        label="bundle trust_policy_sha256_hex",
        byte_length=32,
    )
    _validate_validator_identity(index["validator"])
    _require_hex(
        index["validator_executable_sha256_hex"],
        label="bundle validator_executable_sha256_hex",
        byte_length=32,
    )
    entries = _require_list(index["entries"], label="bundle entries")
    expected_entry_count = 1 + len(REQUIRED_PHASES) + len(PROFILE_ORDER)
    if len(entries) != expected_entry_count:
        _fail(f"bundle entries must contain exactly {expected_entry_count} files")
    previous = ""
    seen_hashes: set[str] = set()
    kind_counts = {"release-evidence": 0, "phase-transcript": 0, "lane-evidence": 0}
    total_size = 0
    for position, raw in enumerate(entries):
        entry = _require_object(
            raw,
            label=f"bundle entries[{position}]",
            keys=("path", "kind", "sha256_hex", "size_bytes"),
        )
        path = entry["path"]
        _safe_relative_parts(path, label=f"bundle entries[{position}].path")
        if path <= previous:
            _fail("bundle entries must be strictly sorted by unique path")
        previous = path
        kind = _require_string(entry["kind"], label=f"bundle entries[{position}].kind")
        allowed_kinds = ARTIFACT_KINDS | {"release-evidence"}
        if kind not in allowed_kinds:
            _fail("bundle entry kind is not part of the SCCP V1 schema")
        kind_counts[kind] += 1
        digest = _require_hex(
            entry["sha256_hex"], label=f"bundle entries[{position}].sha256_hex", byte_length=32
        )
        if digest in seen_hashes:
            _fail("bundle entries must have distinct SHA-256 digests")
        seen_hashes.add(digest)
        limit = MAX_EVIDENCE_BYTES if kind == "release-evidence" else (
            MAX_TRANSCRIPT_BYTES if kind == "phase-transcript" else MAX_ARTIFACT_BYTES
        )
        size = _require_int(
            entry["size_bytes"],
            label=f"bundle entries[{position}].size_bytes",
            minimum=1,
            maximum=limit,
        )
        total_size += size
        if total_size > MAX_TOTAL_ARTIFACT_BYTES + MAX_EVIDENCE_BYTES:
            _fail("bundle entries exceed the total SCCP release size bound")
    if kind_counts != {
        "release-evidence": 1,
        "phase-transcript": len(REQUIRED_PHASES),
        "lane-evidence": len(PROFILE_ORDER),
    }:
        _fail("bundle entry kinds do not match the exact SCCP V1 inventory")
    evidence_entries = [entry for entry in entries if entry["kind"] == "release-evidence"]
    if len(evidence_entries) != 1 or evidence_entries[0]["path"] != "evidence.json":
        _fail("bundle must contain exactly one release-evidence entry at evidence.json")
    root_hash = _require_hex(
        index["bundle_root_hash_hex"], label="bundle_root_hash_hex", byte_length=32
    )
    if root_hash != bundle_root_hash_hex(
        entries,
        trust_policy_id=index["trust_policy_id"],
        trust_policy_sha256_hex=index["trust_policy_sha256_hex"],
        validator=index["validator"],
        validator_executable_sha256_hex=index["validator_executable_sha256_hex"],
    ):
        _fail("bundle_root_hash_hex does not match the canonical entry inventory")
    return index


def make_bundle_index(
    evidence: Mapping[str, Any],
    evidence_bytes: bytes,
    trust_policy: Mapping[str, Any],
    trust_policy_bytes: bytes,
    validator_executable_sha256_hex: str,
) -> dict[str, Any]:
    entries = [
        {
            "path": "evidence.json",
            "kind": "release-evidence",
            "sha256_hex": sha256_hex(evidence_bytes),
            "size_bytes": len(evidence_bytes),
        },
        *[dict(entry) for entry in evidence["artifacts"]],
    ]
    entries.sort(key=lambda entry: entry["path"])
    index = {
        "schema": BUNDLE_SCHEMA,
        "release_id": evidence["release_id"],
        "evidence_path": "evidence.json",
        "trust_policy_id": trust_policy["policy_id"],
        "trust_policy_sha256_hex": sha256_hex(trust_policy_bytes),
        "validator": dict(evidence["validator"]),
        "validator_executable_sha256_hex": validator_executable_sha256_hex,
        "entries": entries,
    }
    index["bundle_root_hash_hex"] = bundle_root_hash_hex(
        entries,
        trust_policy_id=index["trust_policy_id"],
        trust_policy_sha256_hex=index["trust_policy_sha256_hex"],
        validator=index["validator"],
        validator_executable_sha256_hex=index["validator_executable_sha256_hex"],
    )
    return validate_bundle_index(index)


def write_new_file(path: Path, data: bytes) -> None:
    """Create a new regular file without following or replacing links."""

    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags, 0o644)
    except OSError:
        _fail("bundle output file could not be created safely")
    try:
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail("bundle output write did not make progress")
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def ensure_new_output_parent(output: Path) -> Path:
    """Validate that an output has a direct parent and does not yet exist."""

    if output.exists() or output.is_symlink():
        _fail("output directory already exists; SCCP bundle creation never overwrites")
    parent = output.parent
    _require_direct_directory(parent, label="output parent")
    if not _SAFE_SEGMENT_RE.fullmatch(output.name):
        _fail("output directory name must use the canonical artifact alphabet")
    return parent


def readiness_summary(evidence: Mapping[str, Any], *, bundle_root_hash: str | None) -> dict[str, Any]:
    """Build the small public readiness projection from validated evidence."""

    lanes = []
    blockers: list[str] = []
    for lane in evidence["lanes"]:
        profile = lane["counterparty_profile"]
        expected_inbound = EXPECTED_INBOUND_STATUS[profile]
        expected_outbound = EXPECTED_OUTBOUND_STATUS[profile]
        inbound = lane["inbound_status"]
        outbound = lane["outbound_status"]
        if inbound != expected_inbound:
            blockers.append(f"{profile}:inbound:{inbound}:requires:{expected_inbound}")
        if outbound != expected_outbound:
            blockers.append(f"{profile}:outbound:{outbound}:requires:{expected_outbound}")
        lanes.append(
            {
                "counterparty_profile": profile,
                "inbound_status": inbound,
                "required_inbound_status": expected_inbound,
                "outbound_status": outbound,
                "required_outbound_status": expected_outbound,
            }
        )
    return {
        "schema": READINESS_SCHEMA,
        "ready": not blockers,
        "release_id": evidence["release_id"],
        "bundle_root_hash_hex": bundle_root_hash,
        "lanes": lanes,
        "blocking_capabilities": blockers,
        "validation_phases": list(REQUIRED_PHASES),
        "provenance_roles": list(PROVENANCE_ROLES),
    }
