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
import tempfile
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
VALIDATOR_BUILD_ID_DOMAIN = b"sccp:release-evidence-validator-build:v1\x00"
CIRCUIT_POLICY_SIGNING_DOMAIN = b"iroha:sccp:circuit-policy-audit:v1\x00"
FORBIDDEN_ALGEBRAIC_SMOKE_VK = (
    "9ef8067d260532f88e60cfa4b458fe678fc46b9c242de18fc91ba646e0857fc4"
)
FORBIDDEN_SIGNAL_BINDING_CIRCUIT_SHA256_HEX = (
    "d7049de0f0b0ecb7ec4f64b885646ab99f85fcbab05dfaf710d3002f17632bb9"
)
PUBLIC_SIGNAL_SCHEMA_HASH_HEX = (
    "7567439f41173d6745a3d51923cb70371acc7d66f23cefb4100d6d5d7a432cbb"
)
SORA_TAIRA_CHAIN_ID_HASH_HEX = (
    "cf1cfc0f57b0bfa4c21882a9870317a1f4812f86533897095e3944be34c5bba7"
)
SEMANTIC_PROFILE_HASH_DOMAIN = b"sccp:semantic-proof-profile:v1"
SORA_FINALITY_ANCHOR_HASH_DOMAIN = b"sccp:sora-finality-anchor:v1"
REQUIRED_SEMANTICS = (
    "sccp-canonical-transfer-v1",
    "sccp-message-leaf-v1",
    "sccp-merkle-inclusion-v1",
    "sora-taira-block-commitment-v1",
    "sora-taira-v2-finality-artifact-v1",
    "sora-taira-v2-dual-quorum-v1",
    "sora-taira-anchor-continuity-v1",
)
RELEASE_CIRCUIT_IDS = (
    "sccp-sora-taira-to-ethereum-mainnet-groth16-bn254-v1",
    "sccp-sora-taira-to-bsc-mainnet-groth16-bn254-v1",
    "sccp-sora-taira-to-tron-mainnet-groth16-bn254-v1",
)
_SIGNAL_BINDING_CIRCUIT = (
    Path(__file__).resolve().parents[1]
    / "artifacts"
    / "sccp-bsc"
    / "circuits"
    / "sccp-bsc-labeled-signal-binding-v1.circom"
)

_U64_MASK = (1 << 64) - 1
_KECCAK_RATE = 136
_KECCAK_ROUND_CONSTANTS = (
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808A,
    0x8000000080008000,
    0x000000000000808B,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008A,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000A,
    0x000000008000808B,
    0x800000000000008B,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800A,
    0x800000008000000A,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
)
_KECCAK_RHO_OFFSETS = (
    (0, 36, 3, 41, 18),
    (1, 44, 10, 45, 2),
    (62, 6, 43, 15, 61),
    (28, 55, 25, 21, 56),
    (27, 20, 39, 8, 14),
)

MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
MAX_INDEX_BYTES = 512 * 1024
MAX_ARTIFACT_BYTES = 16 * 1024 * 1024
MAX_TRANSCRIPT_BYTES = 4 * 1024 * 1024
MAX_SEMANTIC_ARTIFACT_BYTES = 64 * 1024 * 1024
MAX_GROTH16_PROOF_ARTIFACT_BYTES = 16 * 1024 * 1024 + 64 * 1024
MAX_AUDIT_REPORT_BYTES = 2 * 1024 * 1024
MAX_TOTAL_ARTIFACT_BYTES = 256 * 1024 * 1024
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

HUB_CHAIN_IDS = {"sora-taira": "fc56984b-2be7-431d-840e-21514d1883f0"}

# Fixture keys are disposable and non-production. Production policy loading
# denies every published generation even if an attacker relabels the fixture
# schema/environment and signer IDs.
FORBIDDEN_FIXTURE_PUBLIC_KEYS = frozenset(
    (
        # Original SCCP release fixture generation.
        "3908a9df4eb45c2c3eb744f5a5fde5af87f346a59a4995378e95c3895b9e2d5d",
        "4baed4d3a15b3269ab5e710393de6f01944c3af9691dc7a8661474ced9a033f2",
        "0ffb0e0e942b1f2250eb5674aa5674334cb0e84a7374369cc9d9ec636392198e",
        "64bd5cff290fca9a6102466a0be471375712f102cb6548acf9cdec4d0505e6a9",
        "6c78a68b726ddad7bbedcad5d8e118d6c8bde280fa09c2ce543b83a68d339a5c",
        "2c3bc99608eb07dcd184bf8d459b616256bbcc08ae6b54339d3aa41ce18226a8",
        "56b99cacf316965f254d214d011b18fecc16db9bd4d849d484ee127f7ec9404e",
        # Ephemeral release-role keys from signed fixture generations.
        "d90ee0c2aa6e1f57f8aefe1d29dc8959664320e05885478920a9a9d50443d7fc",
        "3358d5cc6df49720a5e4930f2d265384ca54b9357ae4b0cabb365fa679e8cca1",
        "52c9bf4edf5edbfdee818f492da93d3bd9e5b7ccd729c5742f0b73b9654968e0",
        "f41d0ecf2085d23684181cb9f91e87ce8569504c5910f383578ebebb9c4501a2",
        "bc9b93208bca878fdc78dfad81c66aeee61648c2f2ee244e8e2248053854e0cc",
        "dd34325c20f1be9a0f4ff5486d841692a5aa0ed32db8b3fc4f7c1a2c2d82915d",
        "71855fa376f5bb419aa57d85b0a014b41811a6c4e18c776acdcf18c5f94d4309",
        "a2e5089b86562bc2994e55d4aa44d6923b208e7a29901b5d533798f29885775f",
        # Deterministic fixture-only generator generation.
        "bd0c9cca744a3bb392778a1f3925fe384ea16ea84dd80ac92f3fb453321593ee",
        "0568eb8928f1a3c9623ced2dcd749a000ee25a6922ba32686892357765ef3b91",
        "030af83691318aa2a4c6091d8f64afdc8af513c387b7cde2228e7c5589ba7c74",
        "3a6344e5b76fabf07f91ff396c82b36642ff30eb26d7d66d4acef8d389f354b1",
        "3b6b6fa357dcec265b24a70ce8808a4a75e2393994be06ad3958be3c9c68749a",
        "a5b2610c54fcf817d94fb832578cc477eaeade34bd0a58de9b503213ef908e64",
        "f40674938b1a40e4670d318b42b47ba9fef3582099bcfefc92790244b0f4cb68",
        # Current fixture-only generation.
        "7b93db743c32a07ccc2c48569645a3cf2a980a1733da7f07d60161a09cef679b",
        "1c0f6ccb3f6003808376dd4090ed76d9e1f4c830fcd4bf8df2aa8a0616ea754f",
        "4eb6252d1332fe20b1baa620e80635f3a4cd0a131d6d3abcb93cfa925732ce12",
        "05f80c4badfbc7015606fcb192dda45f7536f7c1191ef063260bf982ae4e52c0",
        "07ecef22532a6859823046b92b183b90e38b6c367fc1af6ead429be7cbbdc0f5",
        "1b60f8f63d68bb772e5cb5ff7dd98996895a5a7430d9e82f48f48d4776cd1a3b",
        "366e703d99bdbe0a2a4db1a664acd52c43b03f9d053025eb19bda13a5e0a6066",
        # Previous ephemeral release-role seal.
        "df62654404d5e37e3ba68dd14b97117eb199803f4a10a2473e3b7b848e67a1b5",
        "073fb6ce0ac504252d2fe848ad7cbf6afe92bc727a340667f9d2ca56e3331ad7",
        # Current in-memory-only fixture seal.
        "f34444167e0c2810cf4072d1c34b7175d380de2e0efdf48b762247b8bfd5d04b",
        "0666855cf4012140b0cea429d456f14cfbdc53982eab592af675f918d435947c",
        "38861629012e021d8fcfc202ae485b431adff8aa87d5b0b3b8c92048461c1779",
        "330dde2b028c8853134e29aa3ae92832df2ecbe1a5d36f4d800a233fd7e8f4ae",
    )
)

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

SEMANTIC_ARTIFACT_ROLES = (
    ("circuit-artifact", "semantic-circuit", "circuit.bin"),
    ("witness-generator", "witness-generator", "witness-generator.bin"),
    ("verifying-key", "verifying-key", "verifying-key.bin"),
    ("prover-build", "prover-build", "prover-build.bin"),
    ("toolchain-lock", "toolchain-lock", "toolchain.lock"),
    ("honest-witness", "honest-witness", "honest-witness.bin"),
    ("honest-proof", "honest-proof", "honest-proof.norito"),
)
SEMANTIC_POLICY_HASH_FIELDS = {
    "circuit-artifact": "circuit_artifact_sha256_hex",
    "witness-generator": "witness_generator_sha256_hex",
    "verifying-key": "verifying_key_sha256_hex",
    "prover-build": "prover_build_sha256_hex",
    "toolchain-lock": "toolchain_lock_sha256_hex",
}
ARTIFACT_KINDS = frozenset(
    (
        "phase-transcript",
        "lane-evidence",
        "circuit-audit-report",
        *(kind for _, kind, _ in SEMANTIC_ARTIFACT_ROLES),
    )
)
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
REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
SCCP_CRATE_MANIFEST = REPOSITORY_ROOT / "crates" / "iroha_sccp" / "Cargo.toml"
SCCP_BUILD_SCRIPT = REPOSITORY_ROOT / "crates" / "iroha_sccp" / "build.rs"
WORKSPACE_MANIFEST = REPOSITORY_ROOT / "Cargo.toml"
CARGO_LOCK = REPOSITORY_ROOT / "Cargo.lock"
RUST_TOOLCHAIN_LOCK = REPOSITORY_ROOT / "rust-toolchain.toml"
VALIDATOR_BUILD_HASH_FIELDS = (
    "source_sha256_hex",
    "crate_manifest_sha256_hex",
    "build_script_sha256_hex",
    "workspace_manifest_sha256_hex",
    "cargo_lock_sha256_hex",
    "toolchain_lock_sha256_hex",
    "executable_sha256_hex",
    "build_identity_hex",
)

_SAFE_SEGMENT_RE = re.compile(r"^[a-z0-9](?:[a-z0-9._-]{0,95})$")
_SAFE_ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9._:+-]{0,127})$")
_HEX_RE = re.compile(r"^[0-9a-f]+$")
_SENSITIVE_RE = re.compile(
    r"(?:"
    r"private[\s._-]*key|secret[\s._-]*key|seed[\s._-]*phrase|"
    r"recovery[\s._-]*phrase|mnemonic|bearer[\s._-]+[a-z0-9]|"
    r"authorization[\s._-]*(?::|=)|password[\s._-]*(?::|=)|"
    r"client[\s._-]*secret|(?:api|access|refresh)[\s._-]*(?:key|token)[\s._-]*(?::|=)"
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


def _rotl64(value: int, shift: int) -> int:
    if shift == 0:
        return value & _U64_MASK
    return ((value << shift) | (value >> (64 - shift))) & _U64_MASK


def keccak256(payload: bytes) -> bytes:
    """Return legacy Keccak-256 without relying on optional packages."""

    state = [0] * 25
    padded = bytearray(payload)
    padded.append(0x01)
    padded.extend(b"\x00" * ((_KECCAK_RATE - len(padded) % _KECCAK_RATE) % _KECCAK_RATE))
    padded[-1] |= 0x80
    for offset in range(0, len(padded), _KECCAK_RATE):
        block = padded[offset : offset + _KECCAK_RATE]
        for index in range(_KECCAK_RATE // 8):
            state[index] ^= int.from_bytes(block[index * 8 : index * 8 + 8], "little")
        for round_constant in _KECCAK_ROUND_CONSTANTS:
            columns = [
                state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20]
                for x in range(5)
            ]
            deltas = [
                columns[(x - 1) % 5] ^ _rotl64(columns[(x + 1) % 5], 1)
                for x in range(5)
            ]
            for x in range(5):
                for y in range(5):
                    state[x + 5 * y] ^= deltas[x]
            rotated = [0] * 25
            for x in range(5):
                for y in range(5):
                    rotated[y + 5 * ((2 * x + 3 * y) % 5)] = _rotl64(
                        state[x + 5 * y], _KECCAK_RHO_OFFSETS[x][y]
                    )
            for x in range(5):
                for y in range(5):
                    state[x + 5 * y] = rotated[x + 5 * y] ^ (
                        (~rotated[(x + 1) % 5 + 5 * y])
                        & rotated[(x + 2) % 5 + 5 * y]
                    )
            state[0] ^= round_constant
    return b"".join(word.to_bytes(8, "little") for word in state)[:32]


def semantic_proof_profile_hash(
    circuit_artifact_sha256: bytes,
    witness_generator_sha256: bytes,
    public_signal_schema_hash: bytes,
) -> bytes:
    """Derive the exact governed V1 semantic-profile hash."""

    commitments = (
        circuit_artifact_sha256,
        witness_generator_sha256,
        public_signal_schema_hash,
    )
    if any(
        type(commitment) is not bytes or len(commitment) != 32
        for commitment in commitments
    ):
        _fail("semantic proof profile commitments must each be exactly 32 bytes")
    if any(not any(commitment) for commitment in commitments) or len(set(commitments)) != 3:
        _fail("semantic proof profile commitments must be nonzero and role-distinct")
    canonical = (
        b"\x01\x00\x01"
        + circuit_artifact_sha256
        + witness_generator_sha256
        + public_signal_schema_hash
    )
    return keccak256(SEMANTIC_PROFILE_HASH_DOMAIN + canonical)


def sora_finality_anchor_hash(anchor: Mapping[str, Any]) -> bytes:
    """Validate and derive the exact governed V1 Taira anchor hash."""

    value = _require_object(
        anchor,
        label="SORA finality anchor",
        keys=(
            "version",
            "source_profile",
            "protocol_version",
            "chain_id_hash_hex",
            "checkpoint_height",
            "checkpoint_block_hash_hex",
            "checkpoint_context_id_hex",
            "checkpoint_finality_artifact_hash_hex",
        ),
    )
    anchor_version = _require_int(
        value["version"], label="SORA anchor version", minimum=1, maximum=1
    )
    source_profile = _require_string(
        value["source_profile"], label="SORA anchor source_profile", maximum=32
    )
    if anchor_version != 1 or source_profile != "sora-taira":
        _fail("SORA finality anchor must identify exact Taira V1")
    protocol_version = _require_int(
        value["protocol_version"],
        label="SORA anchor protocol_version",
        minimum=2,
        maximum=2,
    )
    chain_id_hash = bytes.fromhex(
        _require_hex(
            value["chain_id_hash_hex"], label="SORA anchor chain id", byte_length=32
        )
    )
    if chain_id_hash.hex() != SORA_TAIRA_CHAIN_ID_HASH_HEX:
        _fail("SORA finality anchor has the wrong Taira chain id")
    checkpoint_height = _require_int(
        value["checkpoint_height"],
        label="SORA anchor checkpoint_height",
        minimum=1,
        maximum=2**64 - 1,
    )
    block_hash = bytes.fromhex(
        _require_hex(
            value["checkpoint_block_hash_hex"],
            label="SORA anchor checkpoint block",
            byte_length=32,
        )
    )
    context_id = bytes.fromhex(
        _require_hex(
            value["checkpoint_context_id_hex"],
            label="SORA anchor checkpoint context id",
            byte_length=32,
        )
    )
    finality_artifact_hash = bytes.fromhex(
        _require_hex(
            value["checkpoint_finality_artifact_hash_hex"],
            label="SORA anchor checkpoint finality artifact hash",
            byte_length=32,
        )
    )
    _require_pairwise_distinct(
        (
            ("SORA anchor chain id", chain_id_hash.hex()),
            ("SORA anchor checkpoint block", block_hash.hex()),
            ("SORA anchor checkpoint context id", context_id.hex()),
            (
                "SORA anchor checkpoint finality artifact",
                finality_artifact_hash.hex(),
            ),
        )
    )
    canonical = (
        b"\x01\x01"
        + _push_u16(protocol_version)
        + chain_id_hash
        + checkpoint_height.to_bytes(8, "little")
        + block_hash
        + context_id
        + finality_artifact_hash
    )
    return keccak256(SORA_FINALITY_ANCHOR_HASH_DOMAIN + canonical)


def _fail(message: str) -> None:
    raise SccpReleaseError(message)


def public_error(error: BaseException) -> str:
    """Return a bounded error message with common secret shapes redacted."""

    text = unicodedata.normalize("NFKC", str(error))
    text = re.sub(r"(?i)(?:https?://)[^/@\s]+@", "https://<redacted>@", text)
    text = _SENSITIVE_RE.sub("<redacted>", text)
    # Errors are embedded after a fixed CLI prefix. Keep them on one physical
    # line and remove terminal controls (including bidi/zero-width format
    # characters) so an untrusted validator cannot forge another diagnostic.
    text = "".join(
        " " if ch in "\r\n\t" else "?" if unicodedata.category(ch).startswith("C") else ch
        for ch in text
    )
    text = " ".join(text.split())
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
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )
    try:
        descriptor = os.open(path, flags)
    except OSError:
        _fail(f"{label} could not be opened safely")
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1:
            _fail(f"{label} changed file type while opening")
        if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns) != (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_ctime_ns,
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
    identity = (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    )
    if identity != (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    ):
        _fail(f"{label} changed while reading")
    if identity != (
        after_open.st_dev,
        after_open.st_ino,
        after_open.st_size,
        after_open.st_mtime_ns,
        after_open.st_ctime_ns,
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
            with os.scandir(directory) as iterator:
                entries = []
                for entry in iterator:
                    visited_entries += 1
                    if visited_entries > 2 * MAX_ARTIFACTS + 8:
                        _fail("bundle directory tree contains too many entries")
                    entries.append(entry)
            entries.sort(key=lambda entry: entry.name)
        except OSError:
            _fail("bundle directory could not be enumerated safely")
        if prefix and not entries:
            _fail("bundle must not contain uncommitted empty directories")
        for entry in entries:
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


def artifact_limit(kind: str) -> int:
    """Return the closed byte limit for one signed release-artifact kind."""

    if kind == "phase-transcript":
        return MAX_TRANSCRIPT_BYTES
    if kind == "circuit-audit-report":
        return MAX_AUDIT_REPORT_BYTES
    if kind == "honest-proof":
        return MAX_GROTH16_PROOF_ARTIFACT_BYTES
    if kind in {entry[1] for entry in SEMANTIC_ARTIFACT_ROLES}:
        return MAX_SEMANTIC_ARTIFACT_BYTES
    if kind == "lane-evidence":
        return MAX_ARTIFACT_BYTES
    _fail("artifact kind is not part of the SCCP V1 release schema")


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


def _push_u16(value: int) -> bytes:
    return struct.pack("<H", value)


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


def _secret_scan_variants(data: bytes, *, label: str) -> Iterable[str]:
    text = data.decode("utf-8", "ignore")
    variants: set[str] = set()
    pending = [(text, 0)]
    while pending:
        current, depth = pending.pop()
        if current in variants:
            continue
        variants.add(current)
        if len(variants) > 32:
            _fail(f"{label} uses excessively nested public encodings")
        transformed = {
            urllib.parse.unquote(current),
            html.unescape(current),
            unicodedata.normalize("NFKC", current),
        }
        transformed.discard(current)
        if transformed and depth >= 8:
            _fail(f"{label} uses excessively nested public encodings")
        pending.extend((item, depth + 1) for item in transformed if item not in variants)
    return variants


def reject_secret_material(data: bytes, *, label: str) -> None:
    """Reject public artifacts that contain common credential material markers."""

    for variant in _secret_scan_variants(data, label=label):
        if _SENSITIVE_RE.search(variant):
            _fail(f"{label} contains forbidden credential material")
    for token in _BASE64_TOKEN_RE.findall(data):
        try:
            decoded = base64.b64decode(token, validate=True)
        except (binascii.Error, ValueError):
            continue
        for variant in _secret_scan_variants(decoded, label=label):
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
        if (
            signer_id == key
            or signer_id in signer_ids
            or signer_id in keys
            or key in keys
            or key in signer_ids
        ):
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
            attestor_id == key
            or attestor_id in signer_ids
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
            auditor_id == key
            or auditor_id in signer_ids
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

    if not allow_test_policy and (
        keys & FORBIDDEN_FIXTURE_PUBLIC_KEYS
        or any(identity.startswith("fixture-") for identity in signer_ids)
    ):
        _fail("production trust policy contains a published fixture-only identity or key")

    proof_systems = _require_list(
        policy["proof_systems"],
        label="release trust policy proof_systems",
        length=len(PROFILE_ORDER),
    )
    audit_signatures: set[bytes] = set()
    audit_report_hashes: set[str] = set()
    global_hash_roles: dict[str, str] = {}
    for index, expected_profile in enumerate(PROFILE_ORDER):
        proof = _require_object(
            proof_systems[index],
            label=f"release trust policy proof_systems[{index}]",
            keys=(
                "counterparty_profile",
                "circuit_id",
                "semantics",
                "circuit_artifact_sha256_hex",
                "witness_generator_sha256_hex",
                "public_signal_schema_hash_hex",
                "semantic_proof_profile_hash_hex",
                "sora_finality_anchor",
                "sora_finality_anchor_hash_hex",
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
        if circuit_id != RELEASE_CIRCUIT_IDS[index]:
            _fail("proof system must use the exact profile-specific SCCP circuit id")
        if any(
            marker in circuit_id
            for marker in ("smoke", "test", "signal-binding", "labeled-signal")
        ):
            _fail("production proof policy must not approve fixture-only circuits")
        semantics = _require_list(
            proof["semantics"],
            label="proof-system semantics",
            length=len(REQUIRED_SEMANTICS),
        )
        if tuple(semantics) != REQUIRED_SEMANTICS:
            _fail("proof-system semantics do not prove the complete anchored SCCP statement")
        for field in (
            "circuit_artifact_sha256_hex",
            "witness_generator_sha256_hex",
            "public_signal_schema_hash_hex",
            "semantic_proof_profile_hash_hex",
            "sora_finality_anchor_hash_hex",
            "verifier_key_hash_hex",
            "verifying_key_sha256_hex",
            "prover_build_sha256_hex",
            "toolchain_lock_sha256_hex",
        ):
            _require_hex(proof[field], label=f"proof-system {field}", byte_length=32)
        circuit_artifact = bytes.fromhex(proof["circuit_artifact_sha256_hex"])
        witness_generator = bytes.fromhex(proof["witness_generator_sha256_hex"])
        public_signal_schema = bytes.fromhex(proof["public_signal_schema_hash_hex"])
        if public_signal_schema.hex() != PUBLIC_SIGNAL_SCHEMA_HASH_HEX:
            _fail("proof system uses a different public-signal schema")
        if circuit_artifact.hex() == FORBIDDEN_SIGNAL_BINDING_CIRCUIT_SHA256_HEX:
            _fail("labeled-signal-only circuit is forbidden in release policy")
        profile_hash = semantic_proof_profile_hash(
            circuit_artifact, witness_generator, public_signal_schema
        )
        if profile_hash.hex() != proof["semantic_proof_profile_hash_hex"]:
            _fail("semantic proof profile hash does not match its commitments")
        anchor_hash = sora_finality_anchor_hash(proof["sora_finality_anchor"])
        if anchor_hash.hex() != proof["sora_finality_anchor_hash_hex"]:
            _fail("SORA finality anchor hash does not match its checkpoint")
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
        # Hash values with different semantic roles must never alias. Keeping a
        # single role table also prevents a digest from being relabelled across
        # the circuit, anchor, verifying-key, build, and deployment boundaries.
        anchor = proof["sora_finality_anchor"]
        proof_hash_roles = (
            ("circuit_artifact_sha256_hex", circuit_artifact.hex()),
            ("witness_generator_sha256_hex", witness_generator.hex()),
            ("public_signal_schema_hash_hex", public_signal_schema.hex()),
            ("semantic_proof_profile_hash_hex", profile_hash.hex()),
            ("sora_finality_anchor_hash_hex", anchor_hash.hex()),
            ("anchor_chain_id_hash_hex", anchor["chain_id_hash_hex"]),
            ("anchor_checkpoint_block_hash_hex", anchor["checkpoint_block_hash_hex"]),
            ("anchor_checkpoint_context_id_hex", anchor["checkpoint_context_id_hex"]),
            (
                "anchor_checkpoint_finality_artifact_hash_hex",
                anchor["checkpoint_finality_artifact_hash_hex"],
            ),
            ("verifier_key_hash_hex", proof["verifier_key_hash_hex"]),
            ("verifying_key_sha256_hex", proof["verifying_key_sha256_hex"]),
            ("prover_build_sha256_hex", proof["prover_build_sha256_hex"]),
            ("toolchain_lock_sha256_hex", proof["toolchain_lock_sha256_hex"]),
            *tuple(destination_build.items()),
        )
        _require_pairwise_distinct(proof_hash_roles)
        for role, digest in proof_hash_roles:
            previous_role = global_hash_roles.setdefault(digest, role)
            if previous_role != role:
                _fail("proof-system digest is aliased across profiles and roles")
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
            if report_hash in audit_report_hashes:
                _fail("each circuit audit role and profile must use a distinct report")
            audit_report_hashes.add(report_hash)
            previous_role = global_hash_roles.setdefault(
                report_hash, "audit_report_sha256_hex"
            )
            if previous_role != "audit_report_sha256_hex":
                _fail("circuit audit report aliases another proof-system hash role")
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


def _workspace_crate_version() -> str:
    data = read_direct_file(
        WORKSPACE_MANIFEST,
        label="workspace Cargo manifest",
        maximum=256 * 1024,
    )
    try:
        text = data.decode("utf-8", "strict")
    except UnicodeDecodeError:
        _fail("workspace Cargo manifest is not UTF-8")
    match = re.search(
        r"(?ms)^\[workspace\.package\]\s*$\n(?P<section>.*?)(?=^\[|\Z)",
        text,
    )
    if match is None:
        _fail("workspace Cargo manifest has no workspace.package section")
    version = re.search(
        r'(?m)^version\s*=\s*"(?P<version>[0-9]+(?:\.[0-9]+){2}(?:[-+][A-Za-z0-9.-]+)?)"\s*$',
        match.group("section"),
    )
    if version is None:
        _fail("workspace Cargo manifest has no canonical package version")
    return version.group("version")


def _locked_rust_version() -> str:
    data = read_direct_file(
        RUST_TOOLCHAIN_LOCK,
        label="Rust toolchain lock",
        maximum=16 * 1024,
    )
    try:
        text = data.decode("utf-8", "strict")
    except UnicodeDecodeError:
        _fail("Rust toolchain lock is not UTF-8")
    match = re.fullmatch(
        r'\[toolchain\]\nchannel = "(?P<version>[0-9]+\.[0-9]+\.[0-9]+)"\n',
        text,
    )
    if match is None:
        _fail("Rust toolchain lock must pin one exact stable compiler version")
    return match.group("version")


def validator_build_identity_hex(identity: Mapping[str, Any]) -> str:
    """Derive the non-self-referential build ID for a validator attestation."""

    payload = bytearray(VALIDATOR_BUILD_ID_DOMAIN)
    payload.append(identity["protocol_version"])
    for field in (
        "crate_name",
        "crate_version",
        "build_profile",
        "target_triple",
        "rustc_version",
    ):
        payload.extend(_length_prefixed(identity[field].encode("ascii")))
    features = identity["enabled_features"]
    payload.extend(_push_u32(len(features)))
    for feature in features:
        payload.extend(_length_prefixed(feature.encode("ascii")))
    for field in VALIDATOR_BUILD_HASH_FIELDS:
        if field not in ("executable_sha256_hex", "build_identity_hex"):
            payload.extend(bytes.fromhex(identity[field]))
    return hashlib.sha256(payload).hexdigest()


def _validate_validator_identity(value: Any) -> dict[str, Any]:
    identity = _require_object(
        value,
        label="validator identity",
        keys=(
            "protocol_version",
            "crate_name",
            "crate_version",
            "enabled_features",
            "build_profile",
            "target_triple",
            "rustc_version",
            "source_sha256_hex",
            "crate_manifest_sha256_hex",
            "build_script_sha256_hex",
            "workspace_manifest_sha256_hex",
            "cargo_lock_sha256_hex",
            "toolchain_lock_sha256_hex",
            "executable_sha256_hex",
            "build_identity_hex",
        ),
    )
    _require_int(
        identity["protocol_version"],
        label="validator protocol_version",
        minimum=1,
        maximum=1,
    )
    if identity["crate_name"] != "iroha_sccp":
        _fail("validator crate_name must be exactly iroha_sccp")
    crate_version = _require_string(identity["crate_version"], label="validator crate_version")
    if not crate_version.isascii() or not _SAFE_VERSION_RE.fullmatch(crate_version):
        _fail("validator crate_version must be a canonical semantic version")
    if crate_version != _workspace_crate_version():
        _fail("validator crate_version does not match the workspace release version")
    features = _require_list(
        identity["enabled_features"], label="validator enabled_features"
    )
    if features:
        _fail("validator must use the exact production feature set []")
    profile = _require_id(identity["build_profile"], label="validator build_profile")
    if profile not in ("debug", "release"):
        _fail("validator build_profile must be exactly debug or release")
    target = _require_string(identity["target_triple"], label="validator target_triple")
    if not re.fullmatch(r"[a-z0-9_]+(?:-[a-z0-9_.]+){2,3}", target):
        _fail("validator target_triple must be an exact canonical Rust target")
    rustc_version = _require_string(
        identity["rustc_version"], label="validator rustc_version", maximum=192
    )
    locked_rust = _locked_rust_version()
    if not re.fullmatch(
        rf"rustc {re.escape(locked_rust)} \([0-9a-f]{{9,40}} [0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}}\)",
        rustc_version,
    ):
        _fail("validator rustc_version does not match the exact locked compiler")
    for marker in ("unknown", "placeholder", "dirty", "fixture", "test-only"):
        if marker in f"{profile} {target} {rustc_version}".lower():
            _fail("validator build metadata contains a forbidden placeholder")

    for field in VALIDATOR_BUILD_HASH_FIELDS:
        _require_hex(identity[field], label=f"validator {field}", byte_length=32)
    _require_pairwise_distinct(tuple((field, identity[field]) for field in VALIDATOR_BUILD_HASH_FIELDS))

    local_files = (
        ("source_sha256_hex", RUST_VALIDATOR_SOURCE, "canonical Rust release validator source", 2 * 1024 * 1024),
        ("crate_manifest_sha256_hex", SCCP_CRATE_MANIFEST, "SCCP crate manifest", 256 * 1024),
        ("build_script_sha256_hex", SCCP_BUILD_SCRIPT, "SCCP build script", 256 * 1024),
        ("workspace_manifest_sha256_hex", WORKSPACE_MANIFEST, "workspace Cargo manifest", 256 * 1024),
        ("cargo_lock_sha256_hex", CARGO_LOCK, "workspace Cargo lock", 2 * 1024 * 1024),
        ("toolchain_lock_sha256_hex", RUST_TOOLCHAIN_LOCK, "Rust toolchain lock", 16 * 1024),
    )
    for field, path, label, maximum in local_files:
        data = read_direct_file(path, label=label, maximum=maximum)
        if identity[field] != sha256_hex(data):
            _fail(f"validator {field} does not match the canonical repository input")
    if identity["build_identity_hex"] != validator_build_identity_hex(identity):
        _fail("validator build identity does not bind its exact build inputs")
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
        counterparty_domain = _require_int(
            lane["counterparty_domain"],
            label=f"{expected_profile} counterparty domain",
            minimum=0,
            maximum=2**32 - 1,
        )
        if counterparty_domain != PROFILE_DOMAINS[expected_profile]:
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
        limit = artifact_limit(kind)
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
    signatures = {
        _canonical_base64(
            audit["signature_b64"],
            label="trusted circuit audit signature",
            decoded_length=64,
        )
        for proof in trust_policy["proof_systems"]
        for audit in proof["audit_attestations"]
    }
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
            _fail("detached signatures must not be replayed across trust roles")
        public_keys.add(public_key)
        signatures.add(signature)
        if not verify_ed25519(public_key, signature, payload):
            _fail(f"provenance[{index}] has an invalid detached Ed25519 signature")


def _semantic_artifact_path(role: str, digest: str, filename: str) -> str:
    """Return the sole production path for one content-addressed semantic artifact."""

    return f"artifacts/semantic/{role}/{digest}-{filename}"


def _circuit_audit_report_path(profile: str, role: str) -> str:
    return f"artifacts/semantic/audits/{profile}-{role}.json"


def _production_semantic_inventory_metadata(
    artifact_by_path: Mapping[str, Mapping[str, Any]],
    trust_policy: Mapping[str, Any],
) -> set[str]:
    """Validate the production-only semantic artifact inventory known from policy."""

    semantic_paths = {
        path
        for path, artifact in artifact_by_path.items()
        if artifact["kind"] not in ("phase-transcript", "lane-evidence")
    }
    if trust_policy["environment"] != "production":
        if semantic_paths:
            _fail("test-fixture evidence must not contain production semantic artifacts")
        return set()

    known_paths: set[str] = set()
    for proof in trust_policy["proof_systems"]:
        profile = proof["counterparty_profile"]
        for role, kind, filename in SEMANTIC_ARTIFACT_ROLES:
            field = SEMANTIC_POLICY_HASH_FIELDS.get(role)
            if field is None:
                continue
            digest = proof[field]
            path = _semantic_artifact_path(role, digest, filename)
            artifact = artifact_by_path.get(path)
            if (
                artifact is None
                or artifact["kind"] != kind
                or artifact["sha256_hex"] != digest
            ):
                _fail(f"production {profile} {role} artifact is absent or substituted")
            known_paths.add(path)
        for index, role in enumerate(CIRCUIT_AUDITOR_ROLES):
            path = _circuit_audit_report_path(profile, role)
            artifact = artifact_by_path.get(path)
            expected_hash = proof["audit_attestations"][index]["report_sha256_hex"]
            if (
                artifact is None
                or artifact["kind"] != "circuit-audit-report"
                or artifact["sha256_hex"] != expected_hash
            ):
                _fail(f"production {profile} {role} report is absent or substituted")
            known_paths.add(path)

    counts: dict[str, int] = {}
    for path in semantic_paths:
        kind = artifact_by_path[path]["kind"]
        counts[kind] = counts.get(kind, 0) + 1
    if counts.get("circuit-audit-report") != len(PROFILE_ORDER) * len(CIRCUIT_AUDITOR_ROLES):
        _fail("production evidence must contain exactly two independent audit reports per profile")
    for _, kind, _ in SEMANTIC_ARTIFACT_ROLES:
        if not 1 <= counts.get(kind, 0) <= len(PROFILE_ORDER):
            _fail(f"production evidence has an invalid {kind} artifact cardinality")
    if not known_paths <= semantic_paths:
        _fail("production evidence is missing policy-bound semantic artifacts")
    return semantic_paths


def _positive_u64_text(value: Any, *, label: str) -> str:
    if (
        type(value) is not str
        or not re.fullmatch(r"[1-9][0-9]{0,19}", value)
        or int(value) > 2**64 - 1
    ):
        _fail(f"{label} must be a canonical positive u64 string")
    return value


def _validate_honest_proof_claim(
    value: Any, *, profile: str, proof_system: Mapping[str, Any]
) -> dict[str, Any]:
    claim = _require_object(
        value,
        label="honest proof claim",
        keys=(
            "source_profile",
            "target_profile",
            "target_domain",
            "route_revision",
            "message_id_hex",
            "payload_hash_hex",
            "commitment_root_hex",
            "finality_height",
            "finality_block_hash_hex",
            "destination_binding_hash_hex",
            "route_configuration_hash_hex",
            "statement_hash_hex",
            "request_hash_hex",
            "result_hash_hex",
            "verifier_key_hash_hex",
            "semantic_proof_profile_hash_hex",
            "sora_finality_anchor_hash_hex",
            "public_signal_words_hex",
        ),
    )
    if claim["source_profile"] != "sora-taira" or claim["target_profile"] != profile:
        _fail("honest proof claim selects the wrong source or destination profile")
    if _require_int(
        claim["target_domain"], label="honest proof target_domain", maximum=2**32 - 1
    ) != PROFILE_DOMAINS[profile]:
        _fail("honest proof claim selects the wrong target domain")
    if _require_int(
        claim["route_revision"],
        label="honest proof route_revision",
        minimum=1,
        maximum=2**32 - 1,
    ) != proof_system["route_revision"]:
        _fail("honest proof claim selects the wrong governed route revision")
    _positive_u64_text(claim["finality_height"], label="honest proof finality_height")
    hash_fields = tuple(
        field
        for field in claim
        if field.endswith("_hex") and field != "public_signal_words_hex"
    )
    hashes = [
        (field, _require_hex(claim[field], label=f"honest proof {field}", byte_length=32))
        for field in hash_fields
    ]
    _require_pairwise_distinct(hashes)
    for field in (
        "verifier_key_hash_hex",
        "semantic_proof_profile_hash_hex",
        "sora_finality_anchor_hash_hex",
    ):
        if claim[field] != proof_system[field]:
            _fail(f"honest proof claim {field} does not match audited policy")
    signal_words = _require_list(
        claim["public_signal_words_hex"],
        label="honest proof public_signal_words_hex",
        length=11,
    )
    for index, word in enumerate(signal_words):
        _require_hex(
            word,
            label=f"honest proof public signal word {index}",
            byte_length=32,
        )
    return claim


def _validate_circuit_audit_report(
    data: bytes,
    *,
    profile: str,
    role: str,
    auditor_id: str,
    proof_system: Mapping[str, Any],
) -> tuple[tuple[dict[str, Any], ...], dict[str, Any]]:
    value = parse_json_bytes(
        data, label="circuit audit report", maximum=MAX_AUDIT_REPORT_BYTES
    )
    require_canonical_json_file(data, value, label="circuit audit report")
    report = _require_object(
        value,
        label="circuit audit report",
        keys=(
            "schema",
            "role",
            "auditor_id",
            "counterparty_profile",
            "circuit_id",
            "semantics",
            "artifacts",
            "honest_proof_claim",
        ),
    )
    if (
        report["schema"] != "sccp-circuit-audit-report-v1"
        or report["role"] != role
        or report["auditor_id"] != auditor_id
        or report["counterparty_profile"] != profile
        or report["circuit_id"] != proof_system["circuit_id"]
        or tuple(report["semantics"]) != REQUIRED_SEMANTICS
    ):
        _fail("circuit audit report scope does not match its trusted policy role")
    raw_artifacts = _require_list(
        report["artifacts"],
        label="circuit audit report artifacts",
        length=len(SEMANTIC_ARTIFACT_ROLES),
    )
    artifacts: list[dict[str, Any]] = []
    role_hashes: list[tuple[str, str]] = []
    for index, (expected_role, expected_kind, filename) in enumerate(
        SEMANTIC_ARTIFACT_ROLES
    ):
        artifact = _require_object(
            raw_artifacts[index],
            label=f"circuit audit report artifacts[{index}]",
            keys=("role", "kind", "path", "sha256_hex", "size_bytes"),
        )
        digest = _require_hex(
            artifact["sha256_hex"],
            label=f"circuit audit report {expected_role} hash",
            byte_length=32,
        )
        expected_path = _semantic_artifact_path(expected_role, digest, filename)
        if (
            artifact["role"] != expected_role
            or artifact["kind"] != expected_kind
            or artifact["path"] != expected_path
        ):
            _fail("circuit audit report artifact roles, kinds, and paths must be exact")
        _require_int(
            artifact["size_bytes"],
            label=f"circuit audit report {expected_role} size",
            minimum=1,
            maximum=artifact_limit(expected_kind),
        )
        field = SEMANTIC_POLICY_HASH_FIELDS.get(expected_role)
        if field is not None and digest != proof_system[field]:
            _fail(f"circuit audit report {expected_role} hash does not match policy")
        role_hashes.append((expected_role, digest))
        artifacts.append(artifact)
    _require_pairwise_distinct(role_hashes)
    claim = _validate_honest_proof_claim(
        report["honest_proof_claim"], profile=profile, proof_system=proof_system
    )
    return tuple(artifacts), claim


def verify_production_semantic_artifacts(
    evidence: Mapping[str, Any],
    artifact_contents: Mapping[str, bytes],
    trust_policy: Mapping[str, Any],
) -> tuple[tuple[str, str, dict[str, Any]], ...]:
    """Verify closed audited semantic manifests before production signatures count.

    Circuit, witness, and proof bytes remain opaque here. The authenticated Rust
    validator performs canonical decoding and BN254 pairing verification of each
    honest proof after these byte hashes and independent reports agree.
    """

    if trust_policy["environment"] != "production":
        return ()
    artifact_by_path = {entry["path"]: entry for entry in evidence["artifacts"]}
    semantic_paths = _production_semantic_inventory_metadata(
        artifact_by_path, trust_policy
    )
    expected: dict[str, tuple[str, str, int]] = {}
    proof_records: list[tuple[str, str, dict[str, Any]]] = []
    global_role_hashes: dict[str, str] = {}
    for proof_system in trust_policy["proof_systems"]:
        profile = proof_system["counterparty_profile"]
        baseline_artifacts: tuple[dict[str, Any], ...] | None = None
        baseline_claim: dict[str, Any] | None = None
        for index, role in enumerate(CIRCUIT_AUDITOR_ROLES):
            report_path = _circuit_audit_report_path(profile, role)
            report_data = artifact_contents.get(report_path)
            if report_data is None:
                _fail("signed circuit audit report bytes are absent")
            artifacts, claim = _validate_circuit_audit_report(
                report_data,
                profile=profile,
                role=role,
                auditor_id=trust_policy["circuit_auditors"][index]["auditor_id"],
                proof_system=proof_system,
            )
            if baseline_artifacts is None:
                baseline_artifacts, baseline_claim = artifacts, claim
            elif artifacts != baseline_artifacts or claim != baseline_claim:
                _fail("independent circuit auditors did not attest the same artifacts and claim")
        assert baseline_artifacts is not None
        assert baseline_claim is not None
        profile_proof_path: str | None = None
        for artifact in baseline_artifacts:
            path = artifact["path"]
            metadata = artifact_by_path.get(path)
            if metadata is None or (
                metadata["kind"], metadata["sha256_hex"], metadata["size_bytes"]
            ) != (
                artifact["kind"],
                artifact["sha256_hex"],
                artifact["size_bytes"],
            ):
                _fail("audited semantic artifact does not match signed evidence inventory")
            previous = expected.setdefault(
                path,
                (artifact["kind"], artifact["sha256_hex"], artifact["size_bytes"]),
            )
            if previous != (
                artifact["kind"], artifact["sha256_hex"], artifact["size_bytes"]
            ):
                _fail("semantic artifact path is reused for different audited bytes")
            previous_role = global_role_hashes.setdefault(
                artifact["sha256_hex"], artifact["role"]
            )
            if previous_role != artifact["role"]:
                _fail("semantic artifact digest is substituted across artifact roles")
            content = artifact_contents.get(path)
            if content is None:
                _fail("audited semantic artifact bytes are absent")
            if not any(content) or any(
                marker in content.lower()
                for marker in (b"fixture-only", b"sccp:test:", b"smoke-test")
            ):
                _fail("production semantic artifact contains zero or fixture-only material")
            if artifact["role"] == "honest-proof":
                profile_proof_path = path
        if profile_proof_path is None:
            _fail("circuit audit report does not bind an honest proof")
        proof_records.append((profile, profile_proof_path, baseline_claim))
    report_paths = {
        _circuit_audit_report_path(profile, role)
        for profile in PROFILE_ORDER
        for role in CIRCUIT_AUDITOR_ROLES
    }
    if semantic_paths != set(expected) | report_paths:
        _fail("production semantic artifact inventory is not closed")
    proof_paths = [path for _, path, _ in proof_records]
    if len(proof_paths) != len(PROFILE_ORDER) or len(set(proof_paths)) != len(proof_paths):
        _fail("production requires one distinct honest proof artifact per profile")
    return tuple(proof_records)


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
            "trust_policy_sha256_hex",
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
    _require_int(
        evidence["protocol_version"],
        label="protocol_version",
        minimum=1,
        maximum=1,
    )
    hub_profile = _require_string(evidence["hub_profile"], label="hub_profile")
    if hub_profile not in HUB_CHAIN_IDS or evidence["hub_chain_id"] != HUB_CHAIN_IDS[hub_profile]:
        _fail("hub profile and chain id must identify an exact SCCP V1 SORA network")
    _require_int(evidence["created_at_unix_ms"], label="created_at_unix_ms", minimum=1)
    if evidence["trust_policy_id"] != trust_policy["policy_id"]:
        _fail("release evidence trust_policy_id does not match the external trust policy")
    trust_policy_hash = _require_hex(
        evidence["trust_policy_sha256_hex"],
        label="release evidence trust_policy_sha256_hex",
        byte_length=32,
    )
    if trust_policy_hash != sha256_hex(canonical_json_file_bytes(trust_policy)):
        _fail("release evidence does not bind the exact external trust policy")
    validator = _validate_validator_identity(evidence["validator"])
    if trust_policy["environment"] == "production" and validator["build_profile"] != "release":
        _fail("production release evidence requires a release-profile validator build")
    artifacts, artifact_by_path = _validate_artifacts(evidence["artifacts"])
    referenced = _validate_validation(evidence["validation"], artifact_by_path)
    referenced |= _validate_lanes(evidence["lanes"], artifact_by_path)
    referenced |= _production_semantic_inventory_metadata(
        artifact_by_path, trust_policy
    )
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
        limit = artifact_limit(entry["kind"])
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


def authenticate_validator_executable(
    validator_path: Path, validator_identity: Mapping[str, Any]
) -> tuple[bytes, str]:
    """Authenticate the selected executable against signed evidence before execution."""

    executable = _read_validator_executable(validator_path)
    digest = sha256_hex(executable)
    if digest != validator_identity["executable_sha256_hex"]:
        _fail("selected Rust validator executable does not match signed release evidence")
    return executable, digest


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


def _run_bounded_validator_process(
    executable_path: str,
    arguments: Sequence[str],
    safe_environment: Mapping[str, str],
    *,
    popen_extra: Mapping[str, Any] | None = None,
    close_after_spawn: int | None = None,
) -> tuple[bytes, bytes, int]:
    try:
        process = subprocess.Popen(
            [executable_path, *arguments],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=dict(safe_environment),
            shell=False,
            **dict(popen_extra or {}),
        )
    except OSError:
        _fail("canonical Rust release validator could not be started")
    finally:
        if close_after_spawn is not None:
            os.close(close_after_spawn)
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
    return stdout[0], stderr[0], return_code


def _write_staged_validator(path: Path, executable: bytes) -> None:
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags, 0o500)
    except OSError:
        _fail("authenticated Rust validator could not be staged safely")
    try:
        view = memoryview(executable)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail("authenticated Rust validator staging did not make progress")
            view = view[written:]
        if hasattr(os, "fchmod"):
            os.fchmod(descriptor, 0o500)
        os.fsync(descriptor)
        if os.fstat(descriptor).st_size != len(executable):
            _fail("authenticated Rust validator staging changed size")
    finally:
        os.close(descriptor)


def _invoke_validator_command(
    validator: Path,
    arguments: Sequence[str],
    expected_executable_hash: str,
) -> tuple[bytes, bytes, int, str]:
    safe_environment = {"PATH": os.defpath, "LANG": "C", "LC_ALL": "C", "TZ": "UTC"}
    for name in ("SYSTEMROOT", "WINDIR"):
        if name in os.environ:
            safe_environment[name] = os.environ[name]
    validator_descriptor: int | None = None
    executed_validator_hash: str
    if sys.platform.startswith("linux"):
        flags = (
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_NONBLOCK", 0)
        )
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
        except OSError:
            if validator_descriptor is not None:
                os.close(validator_descriptor)
            _fail("canonical Rust release validator could not be opened for execution")
        if executed_validator_hash != expected_executable_hash:
            os.close(validator_descriptor)
            _fail("canonical Rust release validator changed before execution")
        stdout, stderr, return_code = _run_bounded_validator_process(
            f"/proc/self/fd/{validator_descriptor}",
            arguments,
            safe_environment,
            popen_extra={"pass_fds": (validator_descriptor,)},
            close_after_spawn=validator_descriptor,
        )
    else:
        executed_bytes = _read_validator_executable(validator)
        executed_validator_hash = sha256_hex(executed_bytes)
        if executed_validator_hash != expected_executable_hash:
            _fail("canonical Rust release validator changed before execution")
        with tempfile.TemporaryDirectory(prefix="iroha-sccp-validator-") as directory:
            name = "validator.exe" if os.name == "nt" else "validator"
            staged = Path(directory) / name
            _write_staged_validator(staged, executed_bytes)
            stdout, stderr, return_code = _run_bounded_validator_process(
                str(staged),
                arguments,
                safe_environment,
            )
    return stdout, stderr, return_code, executed_validator_hash


def _invoke_lane_validator(
    validator: Path,
    artifact: Path,
    trust_policy_path: Path,
    evidence_path: Path,
    environment: str,
    expected_executable_hash: str,
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
        expected_executable_hash,
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
    _, executable_hash = authenticate_validator_executable(
        validator_path, evidence["validator"]
    )
    stdout, stderr, return_code, executed_hash = _invoke_validator_command(
        validator_path,
        (
            "validate-release",
            str(trust_policy_path.absolute()),
            str(evidence_path.absolute()),
            environment,
        ),
        executable_hash,
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


def verify_rust_semantic_proofs(
    *,
    evidence: Mapping[str, Any],
    evidence_bytes: bytes,
    artifact_root: Path,
    semantic_records: Sequence[tuple[str, str, Mapping[str, Any]]],
    trust_policy: Mapping[str, Any],
    trust_policy_bytes: bytes,
    trust_policy_path: Path,
    evidence_path: Path,
    validator_path: Path,
    expected_executable_hash: str,
) -> tuple[dict[str, Any], ...]:
    """Require the authenticated Rust validator to decode and pair every audited proof."""

    if trust_policy["environment"] != "production":
        if semantic_records:
            _fail("test-fixture evidence cannot request semantic proof validation")
        return ()
    if len(semantic_records) != len(PROFILE_ORDER):
        _fail("production semantic proof validation requires every launch profile")
    artifact_by_path = {entry["path"]: entry for entry in evidence["artifacts"]}
    receipts: list[dict[str, Any]] = []
    for expected_profile, proof_path, audited_claim in semantic_records:
        metadata = artifact_by_path.get(proof_path)
        if metadata is None or metadata["kind"] != "honest-proof":
            _fail("audited honest proof is absent from signed evidence")
        stdout, stderr, return_code, executed_hash = _invoke_validator_command(
            validator_path,
            (
                "validate-semantic-proof",
                str((artifact_root / proof_path).absolute()),
                str(trust_policy_path.absolute()),
                str(evidence_path.absolute()),
                expected_profile,
                "production",
            ),
            expected_executable_hash,
        )
        if executed_hash != expected_executable_hash:
            _fail("canonical Rust validator changed before semantic proof validation")
        if return_code != 0:
            detail = public_error(stderr.decode("utf-8", "replace"))
            _fail(f"canonical Rust semantic proof validation failed: {detail}")
        if stderr or not stdout.endswith(b"\n") or stdout.count(b"\n") != 1:
            _fail("canonical Rust semantic proof validator emitted invalid output")
        value = parse_json_bytes(
            stdout[:-1],
            label="Rust semantic proof receipt",
            maximum=MAX_VALIDATOR_OUTPUT_BYTES,
        )
        if canonical_json_file_bytes(value) != stdout:
            _fail("Rust semantic proof receipt is not canonical JSON plus one LF")
        receipt = _require_object(
            value,
            label="Rust semantic proof receipt",
            keys=(
                "schema",
                "environment",
                "policy_id",
                "release_id",
                "policy_sha256_hex",
                "evidence_sha256_hex",
                "proof_artifact_path",
                "proof_artifact_sha256_hex",
                "canonical_norito_verified",
                "pairing_verified",
                "claim",
            ),
        )
        if (
            receipt["schema"] != "sccp-semantic-proof-validation-v1"
            or receipt["environment"] != "production"
            or receipt["policy_id"] != trust_policy["policy_id"]
            or receipt["release_id"] != evidence["release_id"]
            or receipt["policy_sha256_hex"] != sha256_hex(trust_policy_bytes)
            or receipt["evidence_sha256_hex"] != sha256_hex(evidence_bytes)
            or receipt["proof_artifact_path"] != proof_path
            or receipt["proof_artifact_sha256_hex"] != metadata["sha256_hex"]
            or receipt["canonical_norito_verified"] is not True
            or receipt["pairing_verified"] is not True
            or receipt["claim"] != audited_claim
        ):
            _fail("Rust semantic proof receipt does not match the signed audited claim")
        receipts.append(receipt)
    if tuple(receipt["claim"]["target_profile"] for receipt in receipts) != PROFILE_ORDER:
        _fail("Rust semantic proof receipts are not in the exact launch-profile order")
    if sha256_hex(_read_validator_executable(validator_path)) != expected_executable_hash:
        _fail("canonical Rust validator changed during semantic proof validation")
    return tuple(receipts)


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
            "witness_generator_sha256_hex",
            "public_signal_schema_hash_hex",
            "semantic_proof_profile_hash_hex",
            "sora_finality_anchor_hash_hex",
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
        "witness_generator_sha256_hex",
        "public_signal_schema_hash_hex",
        "semantic_proof_profile_hash_hex",
        "sora_finality_anchor_hash_hex",
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
        expected_circuit_id = RELEASE_CIRCUIT_IDS[
            PROFILE_ORDER.index(lane["counterparty_profile"])
        ]
        if circuit_id != expected_circuit_id:
            _fail("Rust receipt selected the wrong profile-specific semantic circuit")
        for field in (
            "destination_statement_sha256_hex",
            "destination_finality_block_hash_hex",
            "destination_binding_hash_hex",
            "route_configuration_hash_hex",
            "governed_route_configuration_hash_hex",
            "verifier_key_hash_hex",
            "verifying_key_sha256_hex",
            "circuit_artifact_sha256_hex",
            "witness_generator_sha256_hex",
            "public_signal_schema_hash_hex",
            "semantic_proof_profile_hash_hex",
            "sora_finality_anchor_hash_hex",
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

    _, executable_hash = authenticate_validator_executable(
        validator_path, evidence["validator"]
    )
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
            executable_hash,
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
                    or receipt["witness_generator_sha256_hex"]
                    != proof_system["witness_generator_sha256_hex"]
                    or receipt["public_signal_schema_hash_hex"]
                    != proof_system["public_signal_schema_hash_hex"]
                    or receipt["semantic_proof_profile_hash_hex"]
                    != proof_system["semantic_proof_profile_hash_hex"]
                    or receipt["sora_finality_anchor_hash_hex"]
                    != proof_system["sora_finality_anchor_hash_hex"]
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


def _validate_bundle_artifact_kind_counts(kind_counts: Mapping[str, int]) -> None:
    """Require either the closed fixture or complete production inventory shape."""

    if kind_counts["release-evidence"] != 1:
        _fail("bundle must contain exactly one release-evidence entry")
    if kind_counts["phase-transcript"] != len(REQUIRED_PHASES):
        _fail("bundle phase-transcript count does not match the signed corridor")
    if kind_counts["lane-evidence"] != len(PROFILE_ORDER):
        _fail("bundle lane-evidence count does not match the SCCP V1 profile set")

    semantic_kinds = tuple(kind for _, kind, _ in SEMANTIC_ARTIFACT_ROLES)
    semantic_count = kind_counts["circuit-audit-report"] + sum(
        kind_counts[kind] for kind in semantic_kinds
    )
    if semantic_count == 0:
        return

    if kind_counts["circuit-audit-report"] != (
        len(PROFILE_ORDER) * len(CIRCUIT_AUDITOR_ROLES)
    ):
        _fail("production bundle must contain exactly two audit reports per profile")
    for kind in semantic_kinds:
        count = kind_counts[kind]
        if not 1 <= count <= len(PROFILE_ORDER):
            _fail(f"production bundle has an invalid {kind} entry count")
    if kind_counts["honest-proof"] != len(PROFILE_ORDER):
        _fail("production bundle must contain one distinct honest proof per profile")


def validate_bundle_index(value: Any) -> dict[str, Any]:
    """Validate the bounded standalone SCCP release-bundle index schema.

    This pass deliberately does not guess the complete artifact inventory from
    kind counts.  After the signed evidence is loaded, callers must also invoke
    :func:`validate_bundle_index_against_evidence` to compare the index with the
    exact signed paths, kinds, sizes, and hashes.
    """

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
    minimum_entry_count = 1 + len(REQUIRED_PHASES) + len(PROFILE_ORDER)
    maximum_entry_count = 1 + MAX_ARTIFACTS
    if not minimum_entry_count <= len(entries) <= maximum_entry_count:
        _fail(
            "bundle entries must contain the release evidence and a bounded "
            "signed artifact inventory"
        )
    previous = ""
    seen_hashes: set[str] = set()
    allowed_kinds = ARTIFACT_KINDS | {"release-evidence"}
    kind_counts = {kind: 0 for kind in allowed_kinds}
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
        if kind not in allowed_kinds:
            _fail("bundle entry kind is not part of the SCCP V1 schema")
        kind_counts[kind] += 1
        digest = _require_hex(
            entry["sha256_hex"], label=f"bundle entries[{position}].sha256_hex", byte_length=32
        )
        if digest in seen_hashes:
            _fail("bundle entries must have distinct SHA-256 digests")
        seen_hashes.add(digest)
        limit = MAX_EVIDENCE_BYTES if kind == "release-evidence" else artifact_limit(kind)
        size = _require_int(
            entry["size_bytes"],
            label=f"bundle entries[{position}].size_bytes",
            minimum=1,
            maximum=limit,
        )
        total_size += size
        if total_size > MAX_TOTAL_ARTIFACT_BYTES + MAX_EVIDENCE_BYTES:
            _fail("bundle entries exceed the total SCCP release size bound")
    _validate_bundle_artifact_kind_counts(kind_counts)
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


def validate_bundle_index_against_evidence(
    index: Mapping[str, Any],
    evidence: Mapping[str, Any],
    evidence_bytes: bytes,
) -> Mapping[str, Any]:
    """Bind a structurally valid index to the exact signed evidence inventory."""

    expected_entries = [
        {
            "path": "evidence.json",
            "kind": "release-evidence",
            "sha256_hex": sha256_hex(evidence_bytes),
            "size_bytes": len(evidence_bytes),
        },
        *[dict(entry) for entry in evidence["artifacts"]],
    ]
    expected_entries.sort(key=lambda entry: entry["path"])
    if index["release_id"] != evidence["release_id"]:
        _fail("bundle release_id does not match signed release evidence")
    if (
        index["trust_policy_id"] != evidence["trust_policy_id"]
        or index["trust_policy_sha256_hex"]
        != evidence["trust_policy_sha256_hex"]
    ):
        _fail("bundle trust-policy commitment does not match signed release evidence")
    if index["validator"] != evidence["validator"]:
        _fail("bundle validator identity does not match signed release evidence")
    if (
        index["validator_executable_sha256_hex"]
        != evidence["validator"]["executable_sha256_hex"]
    ):
        _fail("bundle executable commitment does not match signed release evidence")
    if index["entries"] != expected_entries:
        _fail("bundle entry inventory does not exactly equal signed release evidence")
    return index


def make_bundle_index(
    evidence: Mapping[str, Any],
    evidence_bytes: bytes,
    trust_policy: Mapping[str, Any],
    trust_policy_bytes: bytes,
    validator_executable_sha256_hex: str,
) -> dict[str, Any]:
    if validator_executable_sha256_hex != evidence["validator"]["executable_sha256_hex"]:
        _fail("bundle validator executable does not match signed release evidence")
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
    validated = validate_bundle_index(index)
    validate_bundle_index_against_evidence(validated, evidence, evidence_bytes)
    return validated


def _directory_open_flags() -> int:
    return (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )


def open_direct_directory(path: Path, *, label: str) -> int:
    """Open a direct directory and bind subsequent operations to its inode."""

    before = _require_direct_directory(path, label=label)
    try:
        descriptor = os.open(path, _directory_open_flags())
    except OSError:
        _fail(f"{label} could not be opened safely")
    opened = os.fstat(descriptor)
    if (
        not stat.S_ISDIR(opened.st_mode)
        or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
    ):
        os.close(descriptor)
        _fail(f"{label} changed while opening")
    return descriptor


def open_directory_at(parent_descriptor: int, name: str, *, label: str) -> int:
    """Open one direct child directory without resolving a link."""

    if not _SAFE_SEGMENT_RE.fullmatch(name):
        _fail(f"{label} has an unsafe directory name")
    try:
        descriptor = os.open(
            name,
            _directory_open_flags(),
            dir_fd=parent_descriptor,
        )
    except (OSError, TypeError, NotImplementedError):
        _fail(f"{label} could not be opened safely")
    opened = os.fstat(descriptor)
    if not stat.S_ISDIR(opened.st_mode):
        os.close(descriptor)
        _fail(f"{label} must be a direct directory")
    return descriptor


def create_new_directory_at(parent_descriptor: int, name: str, *, label: str) -> int:
    """Reserve and open one new directory relative to a stable parent."""

    if not _SAFE_SEGMENT_RE.fullmatch(name):
        _fail(f"{label} has an unsafe directory name")
    try:
        os.mkdir(name, mode=0o755, dir_fd=parent_descriptor)
    except FileExistsError:
        _fail(f"{label} already exists; SCCP bundle creation never overwrites")
    except (OSError, TypeError, NotImplementedError):
        _fail(f"{label} could not be reserved safely")
    return open_directory_at(parent_descriptor, name, label=label)


def write_new_file_at(
    directory_descriptor: int,
    name: str,
    data: bytes,
    *,
    label: str = "bundle output file",
) -> None:
    """Create one new regular file relative to a stable directory inode."""

    if not _SAFE_SEGMENT_RE.fullmatch(name):
        _fail(f"{label} has an unsafe file name")

    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(name, flags, 0o644, dir_fd=directory_descriptor)
    except (OSError, TypeError, NotImplementedError):
        _fail(f"{label} could not be created safely")
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
            _fail(f"{label} is not a direct single-link regular file")
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail(f"{label} write did not make progress")
            view = view[written:]
        os.fsync(descriptor)
        if os.fstat(descriptor).st_size != len(data):
            _fail(f"{label} has the wrong size after writing")
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
