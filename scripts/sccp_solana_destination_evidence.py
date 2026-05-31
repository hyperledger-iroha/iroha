#!/usr/bin/env python3
"""Render SCCP Solana destination rollout evidence.

This helper is offline by design. Operators pass the deployed Solana verifier
program id and verifier code hash to check the destination binding. Production
TOML additionally requires replayable verifier program bytes, pinned source
record hashes, the governed route allowlist hash, and the expected destination
binding hash collected from independent governance or deployment records.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
from pathlib import Path
from typing import Iterable


SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_SOL = 3
SCCP_STARK_FRI_PROOF_FAMILY = "stark-fri-v1"
SCCP_DESTINATION_BINDING_PREFIX = b"sccp:destination:binding:v1"
SCCP_ROUTE_ALLOWLIST_LABEL = b"sccp:route-allowlist:lane-evidence:v1"
SCCP_SOLANA_ROUTE_CANARY_LIVE_PROGRAM_LABEL = (
    b"iroha:sccp:solana-route-canary-live-program:v1"
)
SOLANA_VERIFIER_BACKEND = "solana-program-v1"
SOLANA_VERIFIER_TARGET_CODE = 2
SOLANA_VERIFIER_BACKEND_FAMILY_CODE = 2
SOLANA_DESTINATION_ANCHOR_ID = "sccp:sol:destination-anchor:solana-mainnet-beta:v1"
SOLANA_ROUTE_ALLOWLIST_ID = "sccp:sol:route-allowlist:solana-mainnet-beta:v1"
SOLANA_UPGRADEABLE_LOADER_ID = "BPFLoaderUpgradeab1e11111111111111111111111"
SOLANA_UPGRADEABLE_LOADER_PROGRAM_TAG = 2
SOLANA_UPGRADEABLE_LOADER_PROGRAMDATA_TAG = 3
SOLANA_PROGRAMDATA_METADATA_LEN = 45
SOLANA_BPF_ELF_MAGIC = b"\x7fELF"
SOLANA_BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
SOLANA_BASE58_INDEX = {
    symbol: index for index, symbol in enumerate(SOLANA_BASE58_ALPHABET)
}


def _strip_0x(value: str) -> str:
    return value[2:] if value.lower().startswith("0x") else value


def parse_hex_bytes(
    value: str,
    *,
    label: str,
    byte_length: int,
    nonzero: bool = True,
) -> bytes:
    """Parse a fixed-width hex value."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = _strip_0x(value)
    if len(text) != byte_length * 2:
        raise argparse.ArgumentTypeError(f"{label} must be {byte_length} bytes")
    try:
        raw = bytes.fromhex(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"{label} must be hex") from exc
    if nonzero and not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be zero")
    return raw


def parse_program_bytes_hex(value: str, *, label: str) -> bytes:
    """Parse non-empty Solana program bytes from hex text."""

    if value != value.strip() or any(symbol.isspace() for symbol in value):
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = value
    text = _strip_0x(text)
    if not text:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    if len(text) % 2 != 0:
        raise argparse.ArgumentTypeError(f"{label} must have an even hex length")
    try:
        raw = bytes.fromhex(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"{label} must be hex") from exc
    if not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be all zero")
    if not raw.startswith(SOLANA_BPF_ELF_MAGIC):
        raise argparse.ArgumentTypeError(f"{label} must be a BPF ELF executable")
    return raw


def parse_program_bytes_base64(value: str, *, label: str) -> bytes:
    """Parse non-empty Solana program bytes from base64 text."""

    if value != value.strip() or any(symbol.isspace() for symbol in value):
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = value
    if not text:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    try:
        raw = base64.b64decode(text, validate=True)
    except (ValueError, binascii.Error) as exc:
        raise argparse.ArgumentTypeError(f"{label} must be base64") from exc
    if base64.b64encode(raw).decode("ascii") != text:
        raise argparse.ArgumentTypeError(f"{label} must be canonical base64")
    if not raw:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    if not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be all zero")
    if not raw.startswith(SOLANA_BPF_ELF_MAGIC):
        raise argparse.ArgumentTypeError(f"{label} must be a BPF ELF executable")
    return raw


def parse_program_bytes_file(value: str, *, label: str) -> bytes:
    """Parse non-empty Solana program bytes from a raw binary file."""

    path = Path(value).expanduser()
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise argparse.ArgumentTypeError(f"{label} file cannot be read") from exc
    if not raw:
        raise argparse.ArgumentTypeError(f"{label} file must not be empty")
    if not any(raw):
        raise argparse.ArgumentTypeError(f"{label} file must not be all zero")
    if not raw.startswith(SOLANA_BPF_ELF_MAGIC):
        raise argparse.ArgumentTypeError(f"{label} file must be a BPF ELF executable")
    return raw


def decode_solana_base58(value: str, *, label: str) -> bytes:
    """Decode canonical Solana base58 text."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = value
    if not text:
        raise argparse.ArgumentTypeError(f"{label} must be non-empty")
    numeric = 0
    for symbol in text:
        digit = SOLANA_BASE58_INDEX.get(symbol)
        if digit is None:
            raise argparse.ArgumentTypeError(f"{label} must be canonical base58")
        numeric = numeric * 58 + digit
    leading_zeros = len(text) - len(text.lstrip("1"))
    payload = (
        b""
        if numeric == 0
        else numeric.to_bytes((numeric.bit_length() + 7) // 8, "big")
    )
    return (b"\x00" * leading_zeros) + payload


def normalize_solana_program_id(value: str, *, label: str) -> str:
    """Validate a non-zero 32-byte Solana program id and return it unchanged."""

    raw = decode_solana_base58(value, label=label)
    if len(raw) != 32:
        raise argparse.ArgumentTypeError(f"{label} must decode to 32 bytes")
    if not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not decode to zero")
    return value


def parse_positive_u64(value: str, *, label: str) -> int:
    """Parse a positive unsigned 64-bit integer."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must be a positive u64")
    text = value
    if not text or not text.isascii() or not text.isdecimal():
        raise argparse.ArgumentTypeError(f"{label} must be a positive u64")
    if len(text) > 1 and text.startswith("0"):
        raise argparse.ArgumentTypeError(f"{label} must be a positive u64")
    parsed = int(text, 10)
    if parsed <= 0 or parsed > 0xFFFF_FFFF_FFFF_FFFF:
        raise argparse.ArgumentTypeError(f"{label} must be a positive u64")
    return parsed


def _require_fixed_bytes(
    value: bytes,
    *,
    label: str,
    byte_length: int,
    nonzero: bool = True,
) -> bytes:
    if not isinstance(value, (bytes, bytearray)):
        raise ValueError(f"{label} must be {byte_length} bytes")
    raw = bytes(value)
    if len(raw) != byte_length:
        raise ValueError(f"{label} must be {byte_length} bytes")
    if nonzero and not any(raw):
        raise ValueError(f"{label} must not be zero")
    return raw


def _require_verifier_code_hash_role_separation(
    *,
    verifier_code_hash: bytes,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
) -> None:
    for role, raw in (
        ("route_allowlist_hash", route_allowlist_hash),
        ("destination_binding_hash", destination_binding_hash),
        ("source_verifier_material_hash", source_verifier_material_hash),
        (
            "source_adapter_engine_deployment_hash",
            source_adapter_engine_deployment_hash,
        ),
    ):
        if verifier_code_hash == raw:
            raise ValueError(f"verifier_code_hash must differ from {role}")


def _require_solana_program_id(value: str, *, label: str) -> str:
    try:
        return normalize_solana_program_id(value, label=label)
    except argparse.ArgumentTypeError as exc:
        raise ValueError(str(exc)) from exc


def _require_destination_evidence(args: argparse.Namespace) -> None:
    args.verifier_program_id = _require_solana_program_id(
        args.verifier_program_id,
        label="verifier_program_id",
    )
    args.verifier_code_hash = _require_fixed_bytes(
        args.verifier_code_hash,
        label="verifier_code_hash",
        byte_length=32,
    )


def _hex(value: bytes) -> str:
    return "0x" + value.hex()


def _push_u8(out: bytearray, value: int) -> None:
    out.append(value)


def _push_u32(out: bytearray, value: int) -> None:
    out.extend(value.to_bytes(4, "little", signed=False))


def _push_u64(out: bytearray, value: int) -> None:
    out.extend(value.to_bytes(8, "little", signed=False))


def _push_vec(out: bytearray, value: bytes) -> None:
    _push_u32(out, len(value))
    out.extend(value)


def _prefixed_blake2b(prefix: bytes, payload: bytes) -> bytes:
    hasher = hashlib.blake2b(digest_size=32)
    hasher.update(prefix)
    hasher.update(payload)
    return hasher.digest()


def solana_verifier_program_code_hash(program_bytes: bytes) -> bytes:
    """Compute the deployed Solana verifier program code hash used in evidence."""

    if not isinstance(program_bytes, (bytes, bytearray)):
        raise ValueError("Solana verifier program bytes must be bytes")
    raw = bytes(program_bytes)
    if not raw or not any(raw):
        raise ValueError("Solana verifier program bytes must not be empty or all zero")
    if not raw.startswith(SOLANA_BPF_ELF_MAGIC):
        raise ValueError("Solana verifier program bytes must be a BPF ELF executable")
    return hashlib.blake2b(raw, digest_size=32).digest()


def solana_upgradeable_program_account_data(programdata_address: str) -> bytes:
    """Return the canonical upgradeable Program account bytes for ProgramData."""

    programdata_address = _require_solana_program_id(
        programdata_address,
        label="programdata_address",
    )
    programdata_raw = decode_solana_base58(
        programdata_address,
        label="programdata_address",
    )
    if len(programdata_raw) != 32:
        raise ValueError("programdata_address must decode to 32 bytes")
    return (
        SOLANA_UPGRADEABLE_LOADER_PROGRAM_TAG.to_bytes(4, "little")
        + programdata_raw
    )


def solana_immutable_programdata_metadata(programdata_slot: int) -> bytes:
    """Return the immutable ProgramData metadata header bytes for a slot."""

    if (
        type(programdata_slot) is not int
        or programdata_slot <= 0
        or programdata_slot > 0xFFFF_FFFF_FFFF_FFFF
    ):
        raise ValueError("programdata_slot must be a positive u64")
    return (
        SOLANA_UPGRADEABLE_LOADER_PROGRAMDATA_TAG.to_bytes(4, "little")
        + programdata_slot.to_bytes(8, "little")
        + b"\x00"
        + bytes(32)
    )


def solana_programdata_metadata_hash(programdata_slot: int) -> bytes:
    """Compute the BLAKE2b-256 hash of immutable ProgramData metadata."""

    return hashlib.blake2b(
        solana_immutable_programdata_metadata(programdata_slot),
        digest_size=32,
    ).digest()


def apply_verifier_program_code_hash(args: argparse.Namespace) -> None:
    """Fill or verify the Solana verifier code hash from deployed program bytes."""

    program_hex = getattr(args, "verifier_program_bytes_hex", None)
    program_base64 = getattr(args, "verifier_program_bytes_base64", None)
    program_file = getattr(args, "verifier_program_bytes_file", None)
    supplied = [
        value
        for value in (program_hex, program_base64, program_file)
        if value is not None
    ]
    if len(supplied) > 1:
        raise ValueError(
            "--verifier-program-bytes-hex, --verifier-program-bytes-base64, "
            "and --verifier-program-bytes-file are mutually exclusive"
        )
    program_bytes = supplied[0] if supplied else None
    if program_bytes is None:
        if getattr(args, "verifier_code_hash", None) is None:
            raise ValueError(
                "--verifier-code-hash, --verifier-program-bytes-hex, "
                "--verifier-program-bytes-base64, or "
                "--verifier-program-bytes-file is required"
            )
        return
    derived_hash = solana_verifier_program_code_hash(program_bytes)
    args.verifier_program_bytes_bytes = bytes(program_bytes)
    args.verifier_program_bytes_base64_text = base64.b64encode(
        bytes(program_bytes)
    ).decode("ascii")
    if args.verifier_code_hash is not None and args.verifier_code_hash != derived_hash:
        raise ValueError(
            "--verifier-code-hash does not match Solana verifier program bytes: "
            f"expected {_hex(args.verifier_code_hash)}, got {_hex(derived_hash)}"
        )
    args.verifier_code_hash = derived_hash


def solana_destination_binding_key() -> str:
    """Return Rust's canonical SORA -> Solana destination binding key."""

    return (
        f"sccp:{SCCP_DOMAIN_SORA}:{SCCP_DOMAIN_SOL}:sol:"
        f"{SOLANA_VERIFIER_BACKEND}:{SOLANA_VERIFIER_TARGET_CODE}"
    )


def solana_destination_binding_hash() -> bytes:
    """Compute Rust's canonical SORA -> Solana destination binding hash."""

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, SCCP_DOMAIN_SORA)
    _push_u32(payload, SCCP_DOMAIN_SOL)
    _push_u8(payload, 1)  # RecursiveZk
    _push_u8(payload, 1)  # CryptographicProof
    _push_u8(payload, SOLANA_VERIFIER_TARGET_CODE)
    _push_u8(payload, SOLANA_VERIFIER_BACKEND_FAMILY_CODE)
    _push_vec(payload, solana_destination_binding_key().encode("utf-8"))
    _push_vec(
        payload,
        b"iroha:sccp:bridge-proof:message:stark-fri:v1:sol",
    )
    _push_vec(payload, SCCP_STARK_FRI_PROOF_FAMILY.encode("utf-8"))
    _push_vec(payload, SOLANA_VERIFIER_BACKEND.encode("utf-8"))
    return _prefixed_blake2b(SCCP_DESTINATION_BINDING_PREFIX, bytes(payload))


def solana_route_allowlist_hash(
    *,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes:
    """Compute Rust's canonical SORA -> Solana route allowlist hash."""

    source_verifier_material_hash = _require_fixed_bytes(
        source_verifier_material_hash,
        label="source_verifier_material_hash",
        byte_length=32,
    )
    source_adapter_engine_deployment_hash = _require_fixed_bytes(
        source_adapter_engine_deployment_hash,
        label="source_adapter_engine_deployment_hash",
        byte_length=32,
    )
    destination_binding_hash = _require_fixed_bytes(
        destination_binding_hash,
        label="destination_binding_hash",
        byte_length=32,
    )
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, SCCP_DOMAIN_SOL)
    _push_vec(payload, b"sol")
    _push_vec(payload, b"GovernanceAllowlist")
    _push_vec(payload, SOLANA_ROUTE_ALLOWLIST_ID.encode("utf-8"))
    payload.extend(source_verifier_material_hash)
    payload.extend(source_adapter_engine_deployment_hash)
    payload.extend(destination_binding_hash)
    return _prefixed_blake2b(SCCP_ROUTE_ALLOWLIST_LABEL, payload)


def _require_exact_string(value: str, *, label: str, expected: str | None = None) -> str:
    if not isinstance(value, str) or value != value.strip() or not value:
        raise ValueError(f"{label} must be a non-empty canonical string")
    if expected is not None and value != expected:
        raise ValueError(f"{label} must be {expected!r}")
    return value


def _require_positive_u64(value: int, *, label: str) -> int:
    if type(value) is not int or value <= 0 or value > 0xFFFF_FFFF_FFFF_FFFF:
        raise ValueError(f"{label} must be a positive u64")
    return value


def _require_byte_string(value: bytes, *, label: str) -> bytes:
    if not isinstance(value, (bytes, bytearray)):
        raise ValueError(f"{label} must be bytes")
    raw = bytes(value)
    if not raw:
        raise ValueError(f"{label} must not be empty")
    return raw


def solana_route_canary_evidence_hash(
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
    verifier_program_id: str,
    verifier_code_hash: bytes,
    rpc_commitment: str,
    program_owner: str,
    programdata_owner: str,
    program_immutable: bool,
    program_account_data: bytes,
    programdata_address: str,
    programdata_slot: int,
    expected_programdata_slot: int,
    program_account_context_slot: int,
    programdata_account_context_slot: int,
    programdata_metadata: bytes,
    programdata_executable: bytes,
) -> bytes:
    """Hash live Solana verifier metadata used by route-canary preflight."""

    route_allowlist_hash = _require_fixed_bytes(
        route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    destination_binding_hash = _require_fixed_bytes(
        destination_binding_hash,
        label="destination_binding_hash",
        byte_length=32,
    )
    source_verifier_material_hash = _require_fixed_bytes(
        source_verifier_material_hash,
        label="source_verifier_material_hash",
        byte_length=32,
    )
    source_adapter_engine_deployment_hash = _require_fixed_bytes(
        source_adapter_engine_deployment_hash,
        label="source_adapter_engine_deployment_hash",
        byte_length=32,
    )
    verifier_program_id = _require_solana_program_id(
        verifier_program_id,
        label="verifier_program_id",
    )
    verifier_program_raw = decode_solana_base58(
        verifier_program_id,
        label="verifier_program_id",
    )
    verifier_code_hash = _require_fixed_bytes(
        verifier_code_hash,
        label="verifier_code_hash",
        byte_length=32,
    )
    _require_verifier_code_hash_role_separation(
        verifier_code_hash=verifier_code_hash,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=(
            source_adapter_engine_deployment_hash
        ),
    )
    _require_exact_string(
        rpc_commitment,
        label="rpc_commitment",
        expected="finalized",
    )
    _require_exact_string(
        program_owner,
        label="program_owner",
        expected=SOLANA_UPGRADEABLE_LOADER_ID,
    )
    _require_exact_string(
        programdata_owner,
        label="programdata_owner",
        expected=SOLANA_UPGRADEABLE_LOADER_ID,
    )
    if program_immutable is not True:
        raise ValueError("program_immutable must be true")

    programdata_address = _require_solana_program_id(
        programdata_address,
        label="programdata_address",
    )
    if programdata_address == verifier_program_id:
        raise ValueError("programdata_address must differ from verifier_program_id")
    programdata_raw = decode_solana_base58(
        programdata_address,
        label="programdata_address",
    )

    program_account_data = _require_byte_string(
        program_account_data,
        label="program_account_data",
    )
    expected_program_account_data = solana_upgradeable_program_account_data(
        programdata_address,
    )
    if program_account_data != expected_program_account_data:
        raise ValueError("program_account_data must reference programdata_address")

    programdata_slot = _require_positive_u64(
        programdata_slot,
        label="programdata_slot",
    )
    expected_programdata_slot = _require_positive_u64(
        expected_programdata_slot,
        label="expected_programdata_slot",
    )
    if programdata_slot != expected_programdata_slot:
        raise ValueError("programdata_slot must match expected_programdata_slot")
    program_account_context_slot = _require_positive_u64(
        program_account_context_slot,
        label="program_account_context_slot",
    )
    if program_account_context_slot < programdata_slot:
        raise ValueError(
            "program_account_context_slot must be at or after programdata_slot"
        )
    programdata_account_context_slot = _require_positive_u64(
        programdata_account_context_slot,
        label="programdata_account_context_slot",
    )
    if programdata_account_context_slot < programdata_slot:
        raise ValueError(
            "programdata_account_context_slot must be at or after programdata_slot"
        )

    programdata_metadata = _require_byte_string(
        programdata_metadata,
        label="programdata_metadata",
    )
    expected_metadata = solana_immutable_programdata_metadata(programdata_slot)
    if programdata_metadata != expected_metadata:
        raise ValueError("programdata_metadata must encode immutable ProgramData")

    programdata_executable = _require_byte_string(
        programdata_executable,
        label="programdata_executable",
    )
    executable_hash = solana_verifier_program_code_hash(programdata_executable)
    if executable_hash != verifier_code_hash:
        raise ValueError("programdata_executable must match verifier_code_hash")

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, SCCP_DOMAIN_SORA)
    _push_u32(payload, SCCP_DOMAIN_SOL)
    payload.extend(route_allowlist_hash)
    payload.extend(destination_binding_hash)
    payload.extend(source_verifier_material_hash)
    payload.extend(source_adapter_engine_deployment_hash)
    payload.extend(verifier_program_raw)
    payload.extend(verifier_code_hash)
    _push_vec(payload, rpc_commitment.encode("ascii"))
    _push_vec(payload, program_owner.encode("ascii"))
    _push_vec(payload, programdata_owner.encode("ascii"))
    _push_u8(payload, 1)
    _push_vec(payload, program_account_data)
    payload.extend(programdata_raw)
    _push_u64(payload, programdata_slot)
    _push_u64(payload, expected_programdata_slot)
    _push_u64(payload, program_account_context_slot)
    _push_u64(payload, programdata_account_context_slot)
    _push_vec(payload, programdata_metadata)
    _push_vec(payload, programdata_executable)
    return _prefixed_blake2b(
        SCCP_SOLANA_ROUTE_CANARY_LIVE_PROGRAM_LABEL,
        payload,
    )


def _toml_string(value: str) -> str:
    return json.dumps(value)


def _toml_line(key: str, value: object) -> str:
    if isinstance(value, bool):
        rendered = "true" if value else "false"
    elif isinstance(value, int):
        rendered = str(value)
    elif isinstance(value, str):
        rendered = _toml_string(value)
    elif isinstance(value, list) and all(isinstance(item, str) for item in value):
        rendered = "[" + ", ".join(_toml_string(item) for item in value) + "]"
    else:
        raise TypeError(f"unsupported TOML value for {key}")
    return f"{key} = {rendered}"


def _destination_rollout_lines(args: argparse.Namespace) -> Iterable[str]:
    program_account_data = solana_upgradeable_program_account_data(args.programdata_address)
    programdata_metadata = solana_immutable_programdata_metadata(args.programdata_slot)
    verifier_program_bytes_base64 = getattr(args, "verifier_program_bytes_base64_text", None)
    if verifier_program_bytes_base64 is None:
        verifier_program_bytes_base64 = base64.b64encode(
            args.verifier_program_bytes_bytes
        ).decode("ascii")
    yield "[[zk.sccp_destination_rollouts]]"
    yield _toml_line("version", 1)
    yield _toml_line("domain", SCCP_DOMAIN_SOL)
    yield _toml_line("chain", "sol")
    yield _toml_line("verifier_plan", "SolanaProgramNativeRecursive")
    yield _toml_line("immutable_verifier_ready", True)
    yield _toml_line("anchors_ready", True)
    yield _toml_line("verifier_identity", args.verifier_program_id)
    yield _toml_line("verifier_code_hash", _hex(args.verifier_code_hash))
    yield _toml_line("destination_binding_key", solana_destination_binding_key())
    yield _toml_line("destination_binding_hash", _hex(solana_destination_binding_hash()))
    yield _toml_line("anchor_id", SOLANA_DESTINATION_ANCHOR_ID)
    yield _toml_line("solana_rpc_commitment", "finalized")
    yield _toml_line("solana_program_owner", SOLANA_UPGRADEABLE_LOADER_ID)
    yield _toml_line("solana_programdata_owner", SOLANA_UPGRADEABLE_LOADER_ID)
    yield _toml_line("solana_program_immutable", True)
    yield _toml_line(
        "solana_program_account_data_base64",
        base64.b64encode(program_account_data).decode("ascii"),
    )
    yield _toml_line("solana_programdata_address", str(args.programdata_address))
    yield _toml_line("solana_programdata_slot", str(args.programdata_slot))
    yield _toml_line("solana_expected_programdata_slot", str(args.programdata_slot))
    yield _toml_line(
        "solana_program_account_context_slot",
        str(args.program_account_context_slot),
    )
    yield _toml_line(
        "solana_programdata_account_context_slot",
        str(args.programdata_account_context_slot),
    )
    yield _toml_line(
        "solana_programdata_metadata_blake2b256",
        _hex(hashlib.blake2b(programdata_metadata, digest_size=32).digest()),
    )
    yield _toml_line(
        "solana_programdata_metadata_base64",
        base64.b64encode(programdata_metadata).decode("ascii"),
    )
    yield _toml_line("solana_programdata_executable_blake2b256", _hex(args.verifier_code_hash))
    yield _toml_line("solana_programdata_executable_base64", verifier_program_bytes_base64)
    yield _toml_line("blockers", [])


def _route_allowlist_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> Iterable[str]:
    supplied_route_allowlist_hash = _require_fixed_bytes(
        args.route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    if supplied_route_allowlist_hash != route_allowlist_hash:
        raise ValueError("route_allowlist_hash does not match validated lane evidence")
    yield "[[zk.sccp_route_allowlists]]"
    yield _toml_line("version", 1)
    yield _toml_line("domain", SCCP_DOMAIN_SOL)
    yield _toml_line("chain", "sol")
    yield _toml_line("activation_policy", "GovernanceAllowlist")
    yield _toml_line("route_allowlist_id", SOLANA_ROUTE_ALLOWLIST_ID)
    yield _toml_line("route_allowlist_hash", _hex(route_allowlist_hash))
    yield from _route_canary_toml_lines(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    yield _toml_line("routes_allowlisted", True)
    yield _toml_line("blockers", [])


def _route_canary_toml_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> list[str]:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if canary_hash is None:
        return []
    return [
        _toml_line("route_canary_status", "passed"),
        _toml_line("route_canary_evidence_hash", _hex(canary_hash)),
        _toml_line("route_canary_route_allowlist_hash", _hex(route_allowlist_hash)),
        _toml_line(
            "route_canary_destination_binding_hash",
            _hex(destination_binding_hash),
        ),
    ]


def _route_canary_comment_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> list[str]:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if canary_hash is None:
        return []
    return [
        "# sccp_route_canary_status = " + json.dumps("passed"),
        "# sccp_route_canary_evidence_hash = " + json.dumps(_hex(canary_hash)),
        "# sccp_route_canary_route_allowlist_hash = "
        + json.dumps(_hex(route_allowlist_hash)),
        "# sccp_route_canary_destination_binding_hash = "
        + json.dumps(_hex(destination_binding_hash)),
    ]


def _route_canary_summary(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> dict[str, object] | None:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if canary_hash is None:
        return None
    return {
        "status": "passed",
        "evidence_hash": _hex(canary_hash),
        "route_allowlist_hash": _hex(route_allowlist_hash),
        "destination_binding_hash": _hex(destination_binding_hash),
        "evidence_bound": True,
    }


def _route_canary_evidence_hash(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes | None:
    canary_hash = getattr(args, "route_canary_evidence_hash", None)
    if canary_hash is None:
        return None
    canary_hash = _require_fixed_bytes(
        canary_hash,
        label="route_canary_evidence_hash",
        byte_length=32,
    )
    source_verifier_material_hash = _require_fixed_bytes(
        getattr(args, "source_verifier_material_hash", None),
        label="source_verifier_material_hash",
        byte_length=32,
    )
    source_adapter_engine_deployment_hash = _require_fixed_bytes(
        getattr(args, "source_adapter_engine_deployment_hash", None),
        label="source_adapter_engine_deployment_hash",
        byte_length=32,
    )
    if canary_hash in (
        route_allowlist_hash,
        destination_binding_hash,
        source_verifier_material_hash,
        source_adapter_engine_deployment_hash,
    ):
        raise ValueError(
            "route_canary_evidence_hash must be distinct from route_allowlist_hash, "
            "destination_binding_hash, source_verifier_material_hash, and "
            "source_adapter_engine_deployment_hash"
        )
    _require_toml_programdata_metadata(args, output="route-canary-evidence-hash")
    _require_toml_verifier_program_bytes_base64(
        args,
        output="route-canary-evidence-hash",
    )
    expected_hash = solana_route_canary_evidence_hash(
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
        verifier_program_id=args.verifier_program_id,
        verifier_code_hash=args.verifier_code_hash,
        rpc_commitment="finalized",
        program_owner=SOLANA_UPGRADEABLE_LOADER_ID,
        programdata_owner=SOLANA_UPGRADEABLE_LOADER_ID,
        program_immutable=True,
        program_account_data=solana_upgradeable_program_account_data(
            args.programdata_address,
        ),
        programdata_address=args.programdata_address,
        programdata_slot=args.programdata_slot,
        expected_programdata_slot=args.programdata_slot,
        program_account_context_slot=args.program_account_context_slot,
        programdata_account_context_slot=args.programdata_account_context_slot,
        programdata_metadata=solana_immutable_programdata_metadata(
            args.programdata_slot,
        ),
        programdata_executable=args.verifier_program_bytes_bytes,
    )
    if canary_hash != expected_hash:
        raise ValueError(
            "route_canary_evidence_hash must match live Solana verifier "
            "program metadata"
        )
    return canary_hash


def _route_allowlist_hash_from_args(
    args: argparse.Namespace,
    destination_binding_hash: bytes,
) -> bytes:
    return solana_route_allowlist_hash(
        source_verifier_material_hash=getattr(
            args,
            "source_verifier_material_hash",
            None,
        ),
        source_adapter_engine_deployment_hash=(
            getattr(args, "source_adapter_engine_deployment_hash", None)
        ),
        destination_binding_hash=destination_binding_hash,
    )


def _require_expected_route_allowlist_hash(
    args: argparse.Namespace,
    destination_binding_hash: bytes,
) -> bytes:
    supplied_hash = _require_fixed_bytes(
        args.route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    expected_hash = _route_allowlist_hash_from_args(args, destination_binding_hash)
    if supplied_hash != expected_hash:
        raise ValueError(
            "--route-allowlist-hash does not match canonical source, deployment, "
            "and destination evidence: "
            f"expected {_hex(expected_hash)}, got {_hex(supplied_hash)}"
        )
    return expected_hash


def _require_toml_programdata_metadata(
    args: argparse.Namespace,
    *,
    output: str,
) -> None:
    programdata_address = getattr(args, "programdata_address", None)
    if programdata_address is None:
        raise ValueError(f"--{output} requires --programdata-address")
    args.programdata_address = _require_solana_program_id(
        programdata_address,
        label="programdata_address",
    )
    if args.programdata_address == args.verifier_program_id:
        raise ValueError("programdata_address must differ from verifier_program_id")
    programdata_slot = getattr(args, "programdata_slot", None)
    if type(programdata_slot) is not int or programdata_slot <= 0:
        raise ValueError(f"--{output} requires --programdata-slot")
    program_context_slot = getattr(args, "program_account_context_slot", None)
    if type(program_context_slot) is not int or program_context_slot <= 0:
        raise ValueError(f"--{output} requires --program-account-context-slot")
    if program_context_slot < programdata_slot:
        raise ValueError(
            "--program-account-context-slot must be at or after "
            "--programdata-slot"
        )
    programdata_context_slot = getattr(
        args,
        "programdata_account_context_slot",
        None,
    )
    if type(programdata_context_slot) is not int or programdata_context_slot <= 0:
        raise ValueError(f"--{output} requires --programdata-account-context-slot")
    if programdata_context_slot < programdata_slot:
        raise ValueError(
            "--programdata-account-context-slot must be at or after "
            "--programdata-slot"
        )


def _toml_programdata_metadata_ready(args: argparse.Namespace) -> bool:
    try:
        _require_toml_programdata_metadata(args, output="toml")
    except ValueError:
        return False
    return True


def _has_programdata_metadata_input(args: argparse.Namespace) -> bool:
    return any(
        getattr(args, name, None) is not None
        for name in (
            "programdata_address",
            "programdata_slot",
            "program_account_context_slot",
            "programdata_account_context_slot",
        )
    )


def _has_verifier_program_byte_input(args: argparse.Namespace) -> bool:
    return any(
        getattr(args, name, None) is not None
        for name in (
            "verifier_program_bytes_hex",
            "verifier_program_bytes_base64",
            "verifier_program_bytes_file",
        )
    )


def _validated_verifier_program_bytes_base64(
    args: argparse.Namespace,
) -> str | None:
    program_bytes = getattr(args, "verifier_program_bytes_bytes", None)
    if program_bytes is None or not _has_verifier_program_byte_input(args):
        return None
    derived_hash = solana_verifier_program_code_hash(program_bytes)
    if derived_hash != args.verifier_code_hash:
        raise ValueError(
            "Solana verifier program bytes do not match verifier_code_hash: "
            f"expected {_hex(args.verifier_code_hash)}, got {_hex(derived_hash)}"
        )
    encoded = base64.b64encode(bytes(program_bytes)).decode("ascii")
    supplied = getattr(args, "verifier_program_bytes_base64_text", None)
    if supplied is not None and supplied != encoded:
        raise ValueError("Solana verifier program byte metadata is inconsistent")
    args.verifier_program_bytes_base64_text = encoded
    return encoded


def _require_toml_verifier_program_bytes_base64(
    args: argparse.Namespace,
    *,
    output: str,
) -> str:
    encoded = _validated_verifier_program_bytes_base64(args)
    if encoded is None:
        raise ValueError(
            f"--{output} requires --verifier-program-bytes-hex, "
            "--verifier-program-bytes-base64, or --verifier-program-bytes-file"
        )
    return encoded


def _missing_route_allowlist_args(args: argparse.Namespace) -> list[str]:
    return [
        name
        for name in (
            "route_allowlist_hash",
            "source_verifier_material_hash",
            "source_adapter_engine_deployment_hash",
        )
        if getattr(args, name, None) is None
    ]


def render_toml(
    args: argparse.Namespace,
    destination_binding_hash: bytes | None = None,
) -> str:
    """Render production Solana destination rollout and route allowlist TOML."""

    apply_verifier_program_code_hash(args)
    _require_destination_evidence(args)
    expected_hash = solana_destination_binding_hash()
    expected_pin = getattr(args, "expected_destination_binding_hash", None)
    if expected_pin is None:
        raise ValueError(
            "--expected-destination-binding-hash is required before rendering production TOML"
        )
    if expected_pin != expected_hash:
        raise ValueError(
            "expected destination binding hash does not match the canonical "
            f"SORA -> Solana binding: expected {_hex(expected_pin)}, "
            f"got {_hex(expected_hash)}"
        )
    if destination_binding_hash is None:
        destination_binding_hash = expected_pin
    elif destination_binding_hash != expected_hash:
        raise ValueError(
            "destination_binding_hash must match the canonical "
            f"SORA -> Solana binding: expected {_hex(expected_hash)}, "
            f"got {_hex(destination_binding_hash)}"
        )
    missing_route_args = _missing_route_allowlist_args(args)
    if missing_route_args:
        formatted = ", ".join(f"--{name.replace('_', '-')}" for name in missing_route_args)
        raise ValueError(f"--toml requires {formatted}")
    route_allowlist_hash = _require_expected_route_allowlist_hash(
        args,
        destination_binding_hash,
    )
    if getattr(args, "route_canary_evidence_hash", None) is None:
        raise ValueError("--toml requires --route-canary-evidence-hash")
    _require_toml_programdata_metadata(args, output="toml")
    verifier_program_bytes_base64 = _require_toml_verifier_program_bytes_base64(
        args,
        output="toml",
    )
    program_account_data_base64 = base64.b64encode(
        solana_upgradeable_program_account_data(args.programdata_address)
    ).decode("ascii")
    programdata_metadata = solana_immutable_programdata_metadata(args.programdata_slot)
    programdata_metadata_base64 = base64.b64encode(programdata_metadata).decode("ascii")
    programdata_metadata_hash = hashlib.blake2b(
        programdata_metadata,
        digest_size=32,
    ).digest()
    comments = [
        "# sccp_solana_rpc_commitment = " + json.dumps("finalized"),
        "# sccp_solana_program_owner = "
        + json.dumps(SOLANA_UPGRADEABLE_LOADER_ID),
        "# sccp_solana_programdata_owner = "
        + json.dumps(SOLANA_UPGRADEABLE_LOADER_ID),
        "# sccp_solana_program_immutable = " + json.dumps("true"),
        "# sccp_solana_program_account_data_len = " + json.dumps("36"),
        "# sccp_solana_program_account_data_base64 = "
        + json.dumps(program_account_data_base64),
        "# sccp_solana_programdata_address = "
        + json.dumps(str(args.programdata_address)),
        "# sccp_solana_programdata_slot = "
        + json.dumps(str(args.programdata_slot)),
        "# sccp_solana_expected_programdata_slot = "
        + json.dumps(str(args.programdata_slot)),
        "# sccp_solana_program_account_context_slot = "
        + json.dumps(str(args.program_account_context_slot)),
        "# sccp_solana_programdata_account_context_slot = "
        + json.dumps(str(args.programdata_account_context_slot)),
        "# sccp_solana_programdata_metadata_blake2b256 = "
        + json.dumps(_hex(programdata_metadata_hash)),
        "# sccp_solana_programdata_metadata_base64 = "
        + json.dumps(programdata_metadata_base64),
        "# sccp_solana_programdata_executable_blake2b256 = "
        + json.dumps(_hex(args.verifier_code_hash)),
        "# sccp_solana_programdata_executable_base64 = "
        + json.dumps(verifier_program_bytes_base64),
    ]
    comments.extend(
        [
            "# sccp_solana_destination_binding_hash = "
            + json.dumps(_hex(destination_binding_hash)),
            "# sccp_solana_route_allowlist_hash = "
            + json.dumps(_hex(route_allowlist_hash)),
        ]
    )
    return "\n".join(
        [
            *comments,
            *_destination_rollout_lines(args),
            "",
            *_route_canary_comment_lines(
                args,
                route_allowlist_hash=route_allowlist_hash,
                destination_binding_hash=destination_binding_hash,
            ),
            *_route_allowlist_lines(
                args,
                route_allowlist_hash=route_allowlist_hash,
                destination_binding_hash=destination_binding_hash,
            ),
            "",
        ]
    )


def _json_summary(
    args: argparse.Namespace,
    destination_binding_hash: bytes,
    expected_matches: bool,
) -> dict[str, object]:
    apply_verifier_program_code_hash(args)
    _require_destination_evidence(args)
    expected_hash = solana_destination_binding_hash()
    if destination_binding_hash != expected_hash:
        raise ValueError(
            "destination_binding_hash must match the canonical "
            f"SORA -> Solana binding: expected {_hex(expected_hash)}, "
            f"got {_hex(destination_binding_hash)}"
        )
    expected_pin = getattr(args, "expected_destination_binding_hash", None)
    if expected_pin is not None and expected_pin != expected_hash:
        raise ValueError(
            "expected destination binding hash does not match the canonical "
            f"SORA -> Solana binding: expected {_hex(expected_pin)}, "
            f"got {_hex(expected_hash)}"
        )
    route_requested = any(
        getattr(args, name, None) is not None
        for name in (
            "route_allowlist_hash",
            "source_verifier_material_hash",
            "source_adapter_engine_deployment_hash",
            "route_canary_evidence_hash",
        )
    )
    summary = {
        "source_domain": SCCP_DOMAIN_SORA,
        "domain": SCCP_DOMAIN_SOL,
        "chain": "sol",
        "verifier_plan": "SolanaProgramNativeRecursive",
        "verifier_identity": args.verifier_program_id,
        "verifier_code_hash": _hex(args.verifier_code_hash),
        "anchor_id": SOLANA_DESTINATION_ANCHOR_ID,
        "destination_binding_key": solana_destination_binding_key(),
        "destination_binding_hash": _hex(destination_binding_hash),
        "expected_destination_binding_hash_matches": expected_matches,
        "route_allowlist_evidence_ready": False,
        "route_canary_ready": False,
        "programdata_metadata_ready": False,
        "verifier_program_bytes_present": False,
        "full_toml_ready": False,
        "toml_ready": False,
    }
    verifier_program_bytes_base64 = _validated_verifier_program_bytes_base64(args)
    summary["verifier_program_bytes_present"] = verifier_program_bytes_base64 is not None
    if (
        isinstance(verifier_program_bytes_base64, str)
        and verifier_program_bytes_base64.strip()
    ):
        summary["verifier_program_bytes_base64"] = verifier_program_bytes_base64
        summary["verifier_program_bytes_base64_sha256"] = hashlib.sha256(
            verifier_program_bytes_base64.encode("ascii")
        ).hexdigest()
    if route_requested:
        if expected_pin is None:
            raise ValueError(
                "--route-allowlist-hash requires "
                "--expected-destination-binding-hash"
            )
        missing_route_args = _missing_route_allowlist_args(args)
        if missing_route_args:
            formatted = ", ".join(
                f"--{name.replace('_', '-')}" for name in missing_route_args
            )
            raise ValueError("route allowlist evidence requires " + formatted)
        route_allowlist_hash = _require_fixed_bytes(
            args.route_allowlist_hash,
            label="route_allowlist_hash",
            byte_length=32,
        )
        expected_route_allowlist_hash = _require_expected_route_allowlist_hash(
            args,
            destination_binding_hash,
        )
        if _has_programdata_metadata_input(args):
            _require_toml_programdata_metadata(args, output="json")
        programdata_metadata_ready = _toml_programdata_metadata_ready(args)
        route_canary = _route_canary_summary(
            args,
            route_allowlist_hash=route_allowlist_hash,
            destination_binding_hash=destination_binding_hash,
        )
        route_canary_ready = route_canary is not None
        full_toml_ready = (
            expected_matches
            and programdata_metadata_ready
            and verifier_program_bytes_base64 is not None
            and route_canary_ready
        )
        summary.update(
            {
                "source_verifier_material_hash": _hex(
                    args.source_verifier_material_hash
                ),
                "source_adapter_engine_deployment_hash": _hex(
                    args.source_adapter_engine_deployment_hash
                ),
                "route_allowlist_id": SOLANA_ROUTE_ALLOWLIST_ID,
                "route_allowlist_hash": _hex(route_allowlist_hash),
                "expected_route_allowlist_hash": _hex(
                    expected_route_allowlist_hash
                ),
                "expected_route_allowlist_hash_matches": True,
                "route_allowlist_evidence_ready": True,
                "route_canary_ready": route_canary_ready,
                "programdata_metadata_ready": programdata_metadata_ready,
                "full_toml_ready": full_toml_ready,
                "toml_ready": full_toml_ready,
            }
        )
        if route_canary is not None:
            summary["route_canary"] = route_canary
        else:
            summary["full_toml_ready"] = False
            summary["toml_ready"] = False
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Render SCCP Solana destination rollout evidence.",
    )
    parser.add_argument(
        "--verifier-program-id",
        required=True,
        type=lambda value: normalize_solana_program_id(
            value,
            label="verifier program id",
        ),
        help="Deployed Solana verifier program id as a non-zero 32-byte base58 address.",
    )
    parser.add_argument(
        "--verifier-code-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="verifier code hash",
            byte_length=32,
        ),
        help="Non-zero deployed Solana verifier program code hash.",
    )
    parser.add_argument(
        "--verifier-program-bytes-hex",
        type=lambda value: parse_program_bytes_hex(
            value,
            label="verifier program bytes",
        ),
        help=(
            "Hex-encoded deployed Solana verifier program bytes. When "
            "supplied, the helper derives verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--verifier-program-bytes-base64",
        type=lambda value: parse_program_bytes_base64(
            value,
            label="verifier program bytes",
        ),
        help=(
            "Base64-encoded deployed Solana verifier ProgramData executable "
            "bytes. When supplied, the helper derives verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--verifier-program-bytes-file",
        type=lambda value: parse_program_bytes_file(
            value,
            label="verifier program bytes",
        ),
        help=(
            "Raw binary file containing deployed Solana verifier program "
            "bytes. When supplied, the helper derives verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--route-allowlist-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route allowlist hash",
            byte_length=32,
        ),
        help=(
            "Governed Solana route allowlist hash. Must match the canonical "
            "source material, source adapter deployment, and destination "
            "binding tuple."
        ),
    )
    parser.add_argument(
        "--source-verifier-material-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="source verifier material hash",
            byte_length=32,
        ),
        help="Source verifier material record hash bound into the route allowlist.",
    )
    parser.add_argument(
        "--source-adapter-engine-deployment-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="source adapter engine deployment hash",
            byte_length=32,
        ),
        help="Source adapter engine deployment record hash bound into the route allowlist.",
    )
    parser.add_argument(
        "--route-canary-evidence-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary evidence hash",
            byte_length=32,
        ),
        help=(
            "Non-zero post-deploy route canary evidence hash. The helper "
            "recomputes it from finalized Solana verifier program metadata "
            "before emitting all-lanes preflight TOML."
        ),
    )
    parser.add_argument(
        "--expected-destination-binding-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected destination binding hash",
            byte_length=32,
        ),
        help="Expected canonical SORA -> Solana destination binding hash.",
    )
    parser.add_argument(
        "--programdata-address",
        type=lambda value: normalize_solana_program_id(
            value,
            label="programdata address",
        ),
        help="Audited Solana ProgramData account address; required for TOML.",
    )
    parser.add_argument(
        "--programdata-slot",
        type=lambda value: parse_positive_u64(
            value,
            label="programdata slot",
        ),
        help="Audited positive Solana ProgramData deployment slot; required for TOML.",
    )
    parser.add_argument(
        "--program-account-context-slot",
        type=lambda value: parse_positive_u64(
            value,
            label="program account context slot",
        ),
        help="Audited verifier program RPC context slot; required for TOML.",
    )
    parser.add_argument(
        "--programdata-account-context-slot",
        type=lambda value: parse_positive_u64(
            value,
            label="programdata account context slot",
        ),
        help="Audited ProgramData RPC context slot; required for TOML.",
    )
    parser.add_argument(
        "--toml",
        action="store_true",
        help="Render production TOML records instead of a compact JSON summary.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        apply_verifier_program_code_hash(args)
        destination_binding_hash = solana_destination_binding_hash()
        expected_matches = False
        if args.expected_destination_binding_hash is not None:
            if args.expected_destination_binding_hash != destination_binding_hash:
                raise ValueError(
                    "expected destination binding hash does not match the canonical "
                    "SORA -> Solana binding: "
                    f"expected {_hex(args.expected_destination_binding_hash)}, "
                    f"got {_hex(destination_binding_hash)}"
                )
            expected_matches = True
        if args.toml:
            print(render_toml(args, destination_binding_hash), end="")
        else:
            print(
                json.dumps(
                    _json_summary(args, destination_binding_hash, expected_matches),
                    sort_keys=True,
                    indent=2,
                )
            )
    except ValueError as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
