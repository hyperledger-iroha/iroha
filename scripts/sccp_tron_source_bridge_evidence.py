#!/usr/bin/env python3
"""Render SCCP TRON source bridge deployment evidence.

This helper is offline by design: it does not deploy contracts or query TRON.
Operators pass the governed source bridge address, owner, network id, and the
deployment evidence hashes collected from the live deployment. The script
computes the same `sourceBridgeConfigHash()` value as
`contracts/tron/sccp/SccpTronSourceBridge.sol` and can render the matching
`zk.sccp_source_verifier_materials` plus
`zk.sccp_source_adapter_engine_deployments` TOML records. When destination
verifier deployment material is provided it also recomputes the
deployment-specific TRON Groth16 destination binding hash used by
`SccpTronGroth16Bn254MessageVerifier`. Runtime bytecode can be supplied
directly so the helper derives the deployed code hashes instead of relying on
manual transcription.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Iterable


REPO_ROOT = Path(__file__).resolve().parents[1]
PYTHON_CLIENT = REPO_ROOT / "python"
if str(PYTHON_CLIENT) not in sys.path:
    sys.path.insert(0, str(PYTHON_CLIENT))

from iroha_torii_client.sccp import _keccak_256  # noqa: E402


SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_TRON = 5
SCCP_SUPPORTED_DOMAINS = frozenset(range(0, 9))
TRON_SOURCE_BRIDGE_CONFIG_LABEL = b"iroha:sccp:tron-source-bridge-config:v1"
TRON_DESTINATION_BINDING_LABEL = b"iroha:sccp:tron-destination-binding:v1"
SCCP_ROUTE_ALLOWLIST_LABEL = b"sccp:route-allowlist:lane-evidence:v1"
TRON_DPOS_SOURCE_GATE_LABEL = b"sccp:tron:dpos-source-gate:v1"
TRON_ROUTE_CANARY_EVIDENCE_LABEL = b"iroha:sccp:tron-route-canary-evidence:v3"
TRON_SOURCE_MESSAGE_CALL_ABI = b"submitSccpSourceEvent(uint32,uint32,bytes32)"
TRON_TRIGGER_SMART_CONTRACT_TYPE_URL = (
    b"type.googleapis.com/protocol.TriggerSmartContract"
)
TRON_GROTH16_BACKEND = "tron-groth16-bn254-v1"
SCCP_PROOF_FAMILY_STARK_FRI = "stark-fri-v1"
SCCP_SOURCE_ADAPTER_CIRCUIT_ID = "sccp-source-adapter-v1"
SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET = "fastpq-lane-balanced"
TRON_SOURCE_PROOF_PLAN_CODE = 5
TRON_FINALITY_MODEL_CODE = 5
TRON_SOURCE_CALL_SIGNATURES = 1
TRON_MAX_TRANSACTION_BYTES = 64 * 1024
TRON_MAX_TRANSACTION_MERKLE_BRANCH_NODES = 64
TRON_MAX_WITNESSES = 64
TRON_MAX_SOLID_BLOCK_ANCESTOR_HEADERS = 64
TRON_MAX_SOLID_BLOCK_CONFIRMATION_HEADERS = 64
TRON_MAX_WITNESS_SCHEDULE_TRANSITIONS = 64
TRON_MAX_WITNESS_SCHEDULE_PAYLOAD_BYTES = 1 + 4 + TRON_MAX_WITNESSES * (21 + 8)
FASTPQ_BALANCED_TRACE_ROOT = 0x002A_247F_81C6_F850
FASTPQ_BALANCED_LDE_ROOT = 0x6026_3388_DBBF_9B2A
FASTPQ_BALANCED_OMEGA_COSET = 0x6AF3_25E8_25AD_5C18
BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
BASE58_INDEX = {symbol: index for index, symbol in enumerate(BASE58_ALPHABET)}

TRON_SOURCE_TRUST_ANCHOR_ID = (
    "sccp:tron:source-trust-anchor:mainnet-witness-schedule:v1"
)
TRON_CONSENSUS_VERIFIER_ID = (
    "sccp:tron:consensus-verifier:dpos-solid-block-mainnet:v1"
)
TRON_MESSAGE_INCLUSION_VERIFIER_ID = (
    "sccp:tron:message-inclusion-verifier:transaction-source-mainnet:v1"
)
TRON_SOURCE_BRIDGE_EMITTER_ID = "sccp:tron:source-bridge-emitter:tron-mainnet:v1"
TRON_FINALITY_POLICY_ID = "sccp:tron:finality-policy:solid-block-mainnet:v1"
TRON_DESTINATION_ANCHOR_ID = "sccp:tron:destination-anchor:tron-mainnet:v1"
TRON_ROUTE_ALLOWLIST_ID = "sccp:tron:route-allowlist:tron-mainnet:v1"
TRON_TEMPLATE_COMPONENTS = {
    "source_trust_anchor_hash": (
        TRON_SOURCE_TRUST_ANCHOR_ID,
        "source-trust-anchor",
    ),
    "consensus_verifier_hash": (
        TRON_CONSENSUS_VERIFIER_ID,
        "consensus-verifier",
    ),
    "message_inclusion_verifier_hash": (
        TRON_MESSAGE_INCLUSION_VERIFIER_ID,
        "message-inclusion-verifier",
    ),
    "finality_policy_hash": (
        TRON_FINALITY_POLICY_ID,
        "finality-policy",
    ),
}
TRON_TEMPLATE_TRANSCRIPT_PREFIXES = (
    b"sccp:tron:receipt-proof:v1",
    b"sccp:tron:receipt-state-proof:v1",
    b"sccp:tron:transaction-source-proof:v1",
    b"sccp:tron:event-log-source-policy:v1",
    b"sccp:tron:solid-block-header-proof:v1",
    b"sccp:tron:witness-schedule:v1",
    b"sccp:tron:witness-schedule-payload:v1",
    b"sccp:tron:solid-block-message:v1",
    b"sccp:tron:witness-seal:v1",
    b"sccp:tron:witness-schedule-transition-message:v1",
    b"sccp:tron:witness-schedule-transition-seal:v1",
)
TRON_DPOS_SOURCE_GATE_TRANSCRIPT_PREFIXES = (
    b"sccp:tron:receipt-state-proof:v1",
    b"sccp:tron:transaction-source-proof:v1",
    b"sccp:tron:event-log-source-policy:v1",
    b"sccp:tron:solid-block-header-proof:v1",
    b"sccp:tron:witness-schedule:v1",
    b"sccp:tron:witness-schedule-payload:v1",
    b"sccp:tron:solid-block-message:v1",
    b"sccp:tron:witness-seal:v1",
    b"sccp:tron:witness-schedule-transition-message:v1",
    b"sccp:tron:witness-schedule-transition-seal:v1",
    TRON_TRIGGER_SMART_CONTRACT_TYPE_URL,
    TRON_SOURCE_MESSAGE_CALL_ABI,
    TRON_SOURCE_BRIDGE_CONFIG_LABEL,
)


def _strip_0x(value: str) -> str:
    return value[2:] if value.lower().startswith("0x") else value


def _strip_lower_0x_hex(value: str, *, label: str) -> str:
    if value.startswith("0X"):
        raise argparse.ArgumentTypeError(f"{label} must use lowercase 0x prefix")
    text = value[2:] if value.startswith("0x") else value
    if text != text.lower():
        raise argparse.ArgumentTypeError(f"{label} must use lowercase hex")
    return text


def parse_hex_bytes(
    value: str,
    *,
    label: str,
    byte_length: int,
    nonzero: bool = True,
) -> bytes:
    """Parse a canonical fixed-width hex value."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = _strip_lower_0x_hex(value, label=label)
    if any(symbol.isspace() for symbol in text):
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    if len(text) != byte_length * 2:
        raise argparse.ArgumentTypeError(f"{label} must be {byte_length} bytes")
    try:
        raw = bytes.fromhex(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"{label} must be hex") from exc
    if nonzero and not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be zero")
    return raw


def _parse_runtime_bytecode_text(
    value: str,
    *,
    label: str,
    allow_whitespace: bool,
) -> bytes:
    if allow_whitespace:
        text = "".join(value.strip().split())
    else:
        if value != value.strip():
            raise argparse.ArgumentTypeError(
                f"{label} must not contain surrounding whitespace"
            )
        text = value
        if any(symbol.isspace() for symbol in _strip_0x(text)):
            raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = _strip_lower_0x_hex(text, label=label)
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
    return raw


def parse_runtime_bytecode_hex(value: str, *, label: str) -> bytes:
    """Parse exact non-empty runtime bytecode from inline hex text."""

    return _parse_runtime_bytecode_text(
        value,
        label=label,
        allow_whitespace=False,
    )


def parse_runtime_bytecode_file(value: str, *, label: str) -> bytes:
    """Parse runtime bytecode from a file containing hex text."""

    path = Path(value).expanduser()
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise argparse.ArgumentTypeError(f"{label} file cannot be read") from exc
    return _parse_runtime_bytecode_text(
        text,
        label=label,
        allow_whitespace=True,
    )


def _is_canonical_decimal_text(value: str) -> bool:
    return value == "0" or (
        bool(value)
        and value[0] in "123456789"
        and all("0" <= symbol <= "9" for symbol in value)
    )


def parse_u32(value: object, *, label: str) -> int:
    """Parse a canonical unsigned 32-bit integer."""

    if type(value) is int:
        parsed = value
    elif isinstance(value, str) and _is_canonical_decimal_text(value):
        parsed = int(value, 10)
    else:
        raise argparse.ArgumentTypeError(f"{label} must be a u32")
    if parsed < 0 or parsed > 0xFFFFFFFF:
        raise argparse.ArgumentTypeError(f"{label} must be a u32")
    return parsed


def parse_u64(value: object, *, label: str) -> int:
    """Parse a canonical unsigned 64-bit integer."""

    if type(value) is int:
        parsed = value
    elif isinstance(value, str) and _is_canonical_decimal_text(value):
        parsed = int(value, 10)
    else:
        raise argparse.ArgumentTypeError(f"{label} must be a u64")
    if parsed < 0 or parsed > 0xFFFFFFFFFFFFFFFF:
        raise argparse.ArgumentTypeError(f"{label} must be a u64")
    return parsed


def _require_exact_u32(value: object, label: str) -> int:
    if type(value) is not int or value < 0 or value > 0xFFFFFFFF:
        raise ValueError(f"{label} must be an exact u32")
    return value


def _require_exact_u64(value: object, label: str, *, positive: bool = False) -> int:
    if type(value) is not int or value < 0 or value > 0xFFFFFFFFFFFFFFFF:
        raise ValueError(f"{label} must be an exact u64")
    if positive and value == 0:
        raise ValueError(f"{label} must be a positive u64")
    return value


def _base58check_payload(value: str, *, label: str) -> bytes:
    numeric = 0
    for symbol in value:
        digit = BASE58_INDEX.get(symbol)
        if digit is None:
            raise argparse.ArgumentTypeError(f"{label} must be TRON base58check")
        numeric = numeric * 58 + digit
    leading_zeros = len(value) - len(value.lstrip("1"))
    payload = (
        b""
        if numeric == 0
        else numeric.to_bytes((numeric.bit_length() + 7) // 8, "big")
    )
    raw = (b"\x00" * leading_zeros) + payload
    if len(raw) != 25:
        raise argparse.ArgumentTypeError(f"{label} must be TRON base58check")
    payload, checksum = raw[:-4], raw[-4:]
    expected = hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4]
    if checksum != expected:
        raise argparse.ArgumentTypeError(f"{label} has invalid base58check checksum")
    return payload


def _base58check_encode(payload: bytes) -> str:
    checksum = hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4]
    raw = payload + checksum
    numeric = int.from_bytes(raw, "big")
    encoded = ""
    while numeric:
        numeric, digit = divmod(numeric, 58)
        encoded = BASE58_ALPHABET[digit] + encoded
    leading_zeros = len(raw) - len(raw.lstrip(b"\x00"))
    return ("1" * leading_zeros) + (encoded or "1")


def _require_unpadded_text(value: str, *, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain surrounding whitespace")
    return value


def tron_base58check_from_address20(address: bytes, *, label: str) -> str:
    """Return a checksummed TRON base58 address from trailing 20 address bytes."""

    address = _require_fixed_bytes(address, label=label, byte_length=20)
    return _base58check_encode(b"\x41" + address)


def parse_tron_address(value: str, *, label: str) -> bytes:
    """Parse a TRON address and return the trailing EVM-compatible 20 bytes."""

    text = _require_unpadded_text(value, label=label)
    if text.startswith("0X"):
        raise argparse.ArgumentTypeError(f"{label} must use lowercase 0x prefix")
    hex_text = text[2:] if text.startswith("0x") else text
    if len(hex_text) in {40, 42}:
        if hex_text != hex_text.lower():
            raise argparse.ArgumentTypeError(f"{label} must use lowercase hex")
        if any(symbol.isspace() for symbol in hex_text):
            raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
        try:
            raw = bytes.fromhex(hex_text)
        except ValueError as exc:
            raise argparse.ArgumentTypeError(f"{label} must be hex") from exc
        if len(raw) == 21:
            if raw[0] != 0x41:
                raise argparse.ArgumentTypeError(f"{label} must use TRON 0x41 prefix")
            raw = raw[1:]
        if len(raw) != 20:
            raise argparse.ArgumentTypeError(f"{label} must be 20 bytes")
        if not any(raw):
            raise argparse.ArgumentTypeError(f"{label} must not be zero")
        return raw

    payload = _base58check_payload(text, label=label)
    if len(payload) != 21 or payload[0] != 0x41:
        raise argparse.ArgumentTypeError(f"{label} must use TRON 0x41 prefix")
    address = payload[1:]
    if not any(address):
        raise argparse.ArgumentTypeError(f"{label} must not be zero")
    return address


def normalize_tron_base58check_address(value: str, *, label: str) -> str:
    """Validate a checksummed TRON base58 address and return it unchanged."""

    text = _require_unpadded_text(value, label=label)
    parse_tron_base58check_payload(text, label=label)
    return text


def parse_tron_base58check_payload(value: str, *, label: str) -> bytes:
    """Validate a checksummed TRON base58 address and return its 21-byte payload."""

    text = _require_unpadded_text(value, label=label)
    payload = _base58check_payload(text, label=label)
    if len(payload) != 21 or payload[0] != 0x41:
        raise argparse.ArgumentTypeError(f"{label} must use TRON 0x41 prefix")
    if not any(payload[1:]):
        raise argparse.ArgumentTypeError(f"{label} must not be zero")
    return payload


def _abi_word_address(address: bytes) -> bytes:
    return b"\x00" * 12 + address


def _abi_word_tron_address(payload: bytes) -> bytes:
    return b"\x00" * 11 + payload


def _abi_word_u32(value: int) -> bytes:
    return value.to_bytes(32, "big")


def _push_u8(out: bytearray, value: int) -> None:
    out.append(value)


def _push_u32(out: bytearray, value: int) -> None:
    out.extend(value.to_bytes(4, "little"))


def _push_u64(out: bytearray, value: int) -> None:
    out.extend(value.to_bytes(8, "little"))


def _push_vec(out: bytearray, value: bytes) -> None:
    _push_u32(out, len(value))
    out.extend(value)


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


def tron_source_bridge_config_hash(
    *,
    bridge_address: bytes,
    network_id: bytes,
    source_domain: int,
    target_domain: int,
    owner_address: bytes,
) -> bytes:
    """Compute `SccpTronSourceBridge.sourceBridgeConfigHash()`."""

    source_domain = _require_exact_u32(source_domain, "source_domain")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    bridge_address = _require_fixed_bytes(
        bridge_address,
        label="bridge_address",
        byte_length=20,
    )
    network_id = _require_fixed_bytes(network_id, label="network_id", byte_length=32)
    owner_address = _require_fixed_bytes(
        owner_address,
        label="owner_address",
        byte_length=20,
    )
    if source_domain != SCCP_DOMAIN_TRON:
        raise ValueError("source_domain must be TRON")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")
    payload = b"".join(
        (
            _keccak_256(TRON_SOURCE_BRIDGE_CONFIG_LABEL),
            _abi_word_address(bridge_address),
            network_id,
            _abi_word_u32(source_domain),
            _abi_word_u32(target_domain),
            _abi_word_address(owner_address),
        )
    )
    return _keccak_256(payload)


def tron_source_message_call_data(
    *,
    source_domain: int,
    target_domain: int,
    source_event_digest: bytes,
) -> bytes:
    """Return TVM calldata for `submitSccpSourceEvent(uint32,uint32,bytes32)`."""

    source_domain = _require_exact_u32(source_domain, "source_domain")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    source_event_digest = _require_fixed_bytes(
        source_event_digest,
        label="source_event_digest",
        byte_length=32,
    )
    if source_domain != SCCP_DOMAIN_TRON:
        raise ValueError("source_domain must be TRON")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")
    return b"".join(
        (
            _keccak_256(TRON_SOURCE_MESSAGE_CALL_ABI)[:4],
            _abi_word_u32(source_domain),
            _abi_word_u32(target_domain),
            source_event_digest,
        )
    )


def tron_source_adapter_verifier_vk_hash(
    *,
    source_domain: int = SCCP_DOMAIN_TRON,
    target_domain: int = SCCP_DOMAIN_SORA,
) -> bytes:
    """Compute Rust's canonical OpenVerify vk hash for TRON -> SORA."""

    source_domain = _require_exact_u32(source_domain, "source_domain")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_TRON:
        raise ValueError("source_domain must be TRON")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")

    verifier = bytearray()
    _push_u8(verifier, 1)
    _push_vec(verifier, SCCP_SOURCE_ADAPTER_CIRCUIT_ID.encode("utf-8"))
    _push_vec(verifier, b"tron")
    _push_u32(verifier, source_domain)
    _push_u32(verifier, target_domain)
    _push_u8(verifier, TRON_SOURCE_PROOF_PLAN_CODE)
    _push_u8(verifier, TRON_FINALITY_MODEL_CODE)
    _push_vec(verifier, SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET.encode("utf-8"))
    _push_u32(verifier, 128)
    _push_u32(verifier, 23)
    _push_u32(verifier, 16)
    _push_u64(verifier, FASTPQ_BALANCED_TRACE_ROOT)
    _push_u32(verifier, 19)
    _push_u64(verifier, FASTPQ_BALANCED_LDE_ROOT)
    _push_u32(verifier, 65_536)
    _push_u8(verifier, 1)
    _push_u32(verifier, 19)
    _push_u64(verifier, FASTPQ_BALANCED_OMEGA_COSET)
    _push_vec(verifier, b"Goldilocks")
    _push_vec(verifier, b"18446744069414584321")
    _push_u32(verifier, 2)
    _push_vec(verifier, b"Poseidon2(Goldilocks)")
    _push_vec(verifier, b"SHA3-256")
    _push_u32(verifier, 8)
    _push_u32(verifier, 8)
    _push_u32(verifier, 8)
    _push_u32(verifier, 46)
    return hashlib.sha256(
        SCCP_SOURCE_ADAPTER_CIRCUIT_ID.encode("utf-8") + bytes(verifier)
    ).digest()


def apply_source_adapter_verifier_vk_hash(args: argparse.Namespace) -> None:
    """Fill or verify the canonical source-adapter OpenVerify vk hash."""

    expected_hash = tron_source_adapter_verifier_vk_hash(
        source_domain=args.source_domain,
        target_domain=args.target_domain,
    )
    supplied_hash = getattr(args, "adapter_verifier_vk_hash", None)
    if supplied_hash is not None and supplied_hash != expected_hash:
        raise ValueError(
            "--adapter-verifier-vk-hash does not match the canonical "
            "TRON source-adapter verifier profile: "
            f"expected {_hex(expected_hash)}, got {_hex(supplied_hash)}"
        )
    args.adapter_verifier_vk_hash = expected_hash


def tron_destination_binding_hash(
    *,
    network_id: bytes,
    source_domain: int,
    target_domain: int,
    verifier_address: str,
    verifier_code_hash: bytes,
    verifier_key_hash: bytes,
    proof_family: str = SCCP_PROOF_FAMILY_STARK_FRI,
) -> bytes:
    """Compute the TRON destination binding used by the TVM Groth16 wrapper."""

    source_domain = _require_exact_u32(source_domain, "source_domain")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    network_id = _require_fixed_bytes(network_id, label="network_id", byte_length=32)
    verifier_code_hash = _require_fixed_bytes(
        verifier_code_hash,
        label="verifier_code_hash",
        byte_length=32,
    )
    verifier_key_hash = _require_fixed_bytes(
        verifier_key_hash,
        label="verifier_key_hash",
        byte_length=32,
    )
    if source_domain != SCCP_DOMAIN_SORA:
        raise ValueError("destination source_domain must be SORA")
    if target_domain != SCCP_DOMAIN_TRON:
        raise ValueError("destination target_domain must be TRON")
    if proof_family != SCCP_PROOF_FAMILY_STARK_FRI:
        raise ValueError("proof_family must be stark-fri-v1")
    verifier_payload = parse_tron_base58check_payload(
        verifier_address,
        label="destination verifier address",
    )
    payload = b"".join(
        (
            _keccak_256(TRON_DESTINATION_BINDING_LABEL),
            _keccak_256(TRON_GROTH16_BACKEND.encode("utf-8")),
            _keccak_256(proof_family.encode("utf-8")),
            network_id,
            _abi_word_u32(source_domain),
            _abi_word_u32(target_domain),
            _abi_word_tron_address(verifier_payload),
            verifier_code_hash,
            verifier_key_hash,
        )
    )
    return _keccak_256(payload)


def tron_destination_binding_key(
    *,
    network_id: bytes,
    source_domain: int,
    target_domain: int,
    verifier_address: str,
    verifier_code_hash: bytes,
    verifier_key_hash: bytes,
    proof_family: str = SCCP_PROOF_FAMILY_STARK_FRI,
) -> str:
    """Return the canonical Rust `SccpDestinationBindingV1.key` value."""

    source_domain = _require_exact_u32(source_domain, "source_domain")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    network_id = _require_fixed_bytes(network_id, label="network_id", byte_length=32)
    verifier_code_hash = _require_fixed_bytes(
        verifier_code_hash,
        label="verifier_code_hash",
        byte_length=32,
    )
    verifier_key_hash = _require_fixed_bytes(
        verifier_key_hash,
        label="verifier_key_hash",
        byte_length=32,
    )
    if source_domain != SCCP_DOMAIN_SORA:
        raise ValueError("destination source_domain must be SORA")
    if target_domain != SCCP_DOMAIN_TRON:
        raise ValueError("destination target_domain must be TRON")
    if proof_family != SCCP_PROOF_FAMILY_STARK_FRI:
        raise ValueError("proof_family must be stark-fri-v1")
    normalized_address = normalize_tron_base58check_address(
        verifier_address,
        label="destination verifier address",
    )
    return (
        f"tron:{source_domain}:{target_domain}:{network_id.hex()}:"
        f"{normalized_address}:0x{verifier_code_hash.hex()}:"
        f"0x{verifier_key_hash.hex()}"
    )


def tron_route_allowlist_hash(
    *,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes:
    """Compute Rust's canonical TRON route allowlist hash."""

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
    _push_u32(payload, SCCP_DOMAIN_TRON)
    _push_vec(payload, b"tron")
    _push_vec(payload, b"GovernanceAllowlist")
    _push_vec(payload, TRON_ROUTE_ALLOWLIST_ID.encode("utf-8"))
    payload.extend(source_verifier_material_hash)
    payload.extend(source_adapter_engine_deployment_hash)
    payload.extend(destination_binding_hash)
    return _prefixed_blake2b(SCCP_ROUTE_ALLOWLIST_LABEL, payload)


def _hex(raw: bytes) -> str:
    return "0x" + raw.hex()


def _prefixed_blake2b(prefix: bytes, payload: bytes) -> bytes:
    hasher = hashlib.blake2b(digest_size=32)
    hasher.update(prefix)
    hasher.update(payload)
    return hasher.digest()


def tron_source_verifier_material_record_hash(
    args: argparse.Namespace,
    config_hash: bytes,
) -> bytes:
    """Compute Rust's canonical TRON source verifier material record hash."""

    _require_live_source_component_hashes(args)
    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    config_hash = _require_fixed_bytes(
        config_hash,
        label="source_bridge_config_hash",
        byte_length=32,
    )
    if source_domain != SCCP_DOMAIN_TRON:
        raise ValueError("source_domain must be TRON")
    _require_source_role_hash_separation(args, config_hash)
    source_trust_anchor_hash = _require_fixed_bytes(
        args.source_trust_anchor_hash,
        label="source_trust_anchor_hash",
        byte_length=32,
    )
    consensus_verifier_hash = _require_fixed_bytes(
        args.consensus_verifier_hash,
        label="consensus_verifier_hash",
        byte_length=32,
    )
    message_inclusion_verifier_hash = _require_fixed_bytes(
        args.message_inclusion_verifier_hash,
        label="message_inclusion_verifier_hash",
        byte_length=32,
    )
    finality_policy_hash = _require_fixed_bytes(
        args.finality_policy_hash,
        label="finality_policy_hash",
        byte_length=32,
    )
    bridge_address = _require_fixed_bytes(
        args.bridge_address,
        label="bridge_address",
        byte_length=20,
    )
    source_bridge_emitter_code_hash = _require_fixed_bytes(
        args.source_bridge_emitter_code_hash,
        label="source_bridge_emitter_code_hash",
        byte_length=32,
    )
    network_id = _require_fixed_bytes(
        args.network_id,
        label="network_id",
        byte_length=32,
    )
    owner_address = _require_fixed_bytes(
        args.owner_address,
        label="owner_address",
        byte_length=20,
    )
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, source_domain)
    _push_vec(payload, b"tron")
    _push_u8(payload, TRON_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, TRON_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_SOURCE_ADAPTER_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, TRON_SOURCE_TRUST_ANCHOR_ID.encode("utf-8"))
    payload.extend(source_trust_anchor_hash)
    _push_vec(payload, TRON_CONSENSUS_VERIFIER_ID.encode("utf-8"))
    payload.extend(consensus_verifier_hash)
    _push_vec(payload, TRON_MESSAGE_INCLUSION_VERIFIER_ID.encode("utf-8"))
    payload.extend(message_inclusion_verifier_hash)
    _push_vec(payload, TRON_FINALITY_POLICY_ID.encode("utf-8"))
    payload.extend(finality_policy_hash)
    _push_vec(payload, b"")
    payload.extend(bytes(32))
    _push_vec(payload, TRON_SOURCE_BRIDGE_EMITTER_ID.encode("utf-8"))
    _push_vec(payload, bridge_address)
    payload.extend(source_bridge_emitter_code_hash)
    payload.extend(network_id)
    _push_vec(payload, owner_address)
    payload.extend(config_hash)
    _push_u8(payload, 0)
    return _prefixed_blake2b(
        b"sccp:source-verifier-material-record:v1",
        bytes(payload),
    )


def tron_source_adapter_engine_deployment_record_hash(
    args: argparse.Namespace,
    config_hash: bytes,
) -> bytes:
    """Compute Rust's canonical TRON source-adapter deployment record hash."""

    _require_live_source_component_hashes(args)
    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    config_hash = _require_fixed_bytes(
        config_hash,
        label="source_bridge_config_hash",
        byte_length=32,
    )
    if source_domain != SCCP_DOMAIN_TRON:
        raise ValueError("source_domain must be TRON")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")
    _require_source_role_hash_separation(args, config_hash)
    adapter_verifier_vk_hash = _require_fixed_bytes(
        args.adapter_verifier_vk_hash,
        label="adapter_verifier_vk_hash",
        byte_length=32,
    )
    expected_adapter_verifier_vk_hash = tron_source_adapter_verifier_vk_hash(
        source_domain=source_domain,
        target_domain=target_domain,
    )
    if adapter_verifier_vk_hash != expected_adapter_verifier_vk_hash:
        raise ValueError(
            "adapter_verifier_vk_hash must match the canonical "
            "TRON source-adapter verifier profile"
        )
    source_trust_anchor_hash = _require_fixed_bytes(
        args.source_trust_anchor_hash,
        label="source_trust_anchor_hash",
        byte_length=32,
    )
    consensus_verifier_hash = _require_fixed_bytes(
        args.consensus_verifier_hash,
        label="consensus_verifier_hash",
        byte_length=32,
    )
    message_inclusion_verifier_hash = _require_fixed_bytes(
        args.message_inclusion_verifier_hash,
        label="message_inclusion_verifier_hash",
        byte_length=32,
    )
    finality_policy_hash = _require_fixed_bytes(
        args.finality_policy_hash,
        label="finality_policy_hash",
        byte_length=32,
    )
    bridge_address = _require_fixed_bytes(
        args.bridge_address,
        label="bridge_address",
        byte_length=20,
    )
    source_bridge_emitter_code_hash = _require_fixed_bytes(
        args.source_bridge_emitter_code_hash,
        label="source_bridge_emitter_code_hash",
        byte_length=32,
    )
    network_id = _require_fixed_bytes(
        args.network_id,
        label="network_id",
        byte_length=32,
    )
    owner_address = _require_fixed_bytes(
        args.owner_address,
        label="owner_address",
        byte_length=20,
    )
    deployment_receipt_hash = _require_fixed_bytes(
        args.deployment_receipt_hash,
        label="deployment_receipt_hash",
        byte_length=32,
    )
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, source_domain)
    _push_u32(payload, target_domain)
    _push_vec(payload, b"tron")
    _push_u8(payload, TRON_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, TRON_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8"))
    _push_vec(payload, SCCP_SOURCE_ADAPTER_CIRCUIT_ID.encode("utf-8"))
    payload.extend(adapter_verifier_vk_hash)
    _push_vec(payload, TRON_SOURCE_TRUST_ANCHOR_ID.encode("utf-8"))
    payload.extend(source_trust_anchor_hash)
    _push_vec(payload, TRON_CONSENSUS_VERIFIER_ID.encode("utf-8"))
    payload.extend(consensus_verifier_hash)
    _push_vec(payload, TRON_MESSAGE_INCLUSION_VERIFIER_ID.encode("utf-8"))
    payload.extend(message_inclusion_verifier_hash)
    _push_vec(payload, TRON_FINALITY_POLICY_ID.encode("utf-8"))
    payload.extend(finality_policy_hash)
    _push_vec(payload, b"")
    payload.extend(bytes(32))
    _push_vec(payload, TRON_SOURCE_BRIDGE_EMITTER_ID.encode("utf-8"))
    _push_vec(payload, bridge_address)
    payload.extend(source_bridge_emitter_code_hash)
    payload.extend(network_id)
    _push_vec(payload, owner_address)
    payload.extend(config_hash)
    payload.extend(deployment_receipt_hash)
    return _prefixed_blake2b(
        b"sccp:source-adapter-engine-deployment:v1",
        bytes(payload),
    )


def tron_dpos_source_gate_hash(
    args: argparse.Namespace,
    config_hash: bytes,
) -> bytes:
    """Compute Rust's canonical TRON DPoS source deployment gate hash."""

    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    config_hash = _require_fixed_bytes(
        config_hash,
        label="source_bridge_config_hash",
        byte_length=32,
    )
    if source_domain != SCCP_DOMAIN_TRON:
        raise ValueError("source_domain must be TRON")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")
    bridge_address = _require_fixed_bytes(
        args.bridge_address,
        label="bridge_address",
        byte_length=20,
    )
    owner_address = _require_fixed_bytes(
        args.owner_address,
        label="owner_address",
        byte_length=20,
    )
    network_id = _require_fixed_bytes(
        args.network_id,
        label="network_id",
        byte_length=32,
    )
    expected_config_hash = tron_source_bridge_config_hash(
        bridge_address=bridge_address,
        network_id=network_id,
        source_domain=source_domain,
        target_domain=target_domain,
        owner_address=owner_address,
    )
    if config_hash != expected_config_hash:
        raise ValueError(
            "source_bridge_config_hash must match the governed TRON source bridge "
            f"configuration: expected {_hex(expected_config_hash)}, got {_hex(config_hash)}"
        )
    material_hash = tron_source_verifier_material_record_hash(args, config_hash)
    deployment_hash = tron_source_adapter_engine_deployment_record_hash(args, config_hash)
    adapter_verifier_vk_hash = _require_fixed_bytes(
        args.adapter_verifier_vk_hash,
        label="adapter_verifier_vk_hash",
        byte_length=32,
    )
    source_trust_anchor_hash = _require_fixed_bytes(
        args.source_trust_anchor_hash,
        label="source_trust_anchor_hash",
        byte_length=32,
    )
    consensus_verifier_hash = _require_fixed_bytes(
        args.consensus_verifier_hash,
        label="consensus_verifier_hash",
        byte_length=32,
    )
    message_inclusion_verifier_hash = _require_fixed_bytes(
        args.message_inclusion_verifier_hash,
        label="message_inclusion_verifier_hash",
        byte_length=32,
    )
    finality_policy_hash = _require_fixed_bytes(
        args.finality_policy_hash,
        label="finality_policy_hash",
        byte_length=32,
    )
    source_bridge_emitter_code_hash = _require_fixed_bytes(
        args.source_bridge_emitter_code_hash,
        label="source_bridge_emitter_code_hash",
        byte_length=32,
    )

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, source_domain)
    _push_u32(payload, target_domain)
    _push_vec(payload, b"tron")
    _push_u8(payload, TRON_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, TRON_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_SOURCE_ADAPTER_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, TRON_GROTH16_BACKEND.encode("utf-8"))
    payload.extend(material_hash)
    payload.extend(deployment_hash)
    payload.extend(adapter_verifier_vk_hash)
    _push_vec(payload, TRON_SOURCE_TRUST_ANCHOR_ID.encode("utf-8"))
    payload.extend(source_trust_anchor_hash)
    _push_vec(payload, TRON_CONSENSUS_VERIFIER_ID.encode("utf-8"))
    payload.extend(consensus_verifier_hash)
    _push_vec(payload, TRON_MESSAGE_INCLUSION_VERIFIER_ID.encode("utf-8"))
    payload.extend(message_inclusion_verifier_hash)
    _push_vec(payload, TRON_FINALITY_POLICY_ID.encode("utf-8"))
    payload.extend(finality_policy_hash)
    _push_vec(payload, TRON_SOURCE_BRIDGE_EMITTER_ID.encode("utf-8"))
    _push_vec(payload, bridge_address)
    payload.extend(source_bridge_emitter_code_hash)
    payload.extend(network_id)
    _push_vec(payload, owner_address)
    payload.extend(config_hash)
    for prefix in TRON_DPOS_SOURCE_GATE_TRANSCRIPT_PREFIXES:
        _push_vec(payload, prefix)
    for bound in (
        TRON_SOURCE_CALL_SIGNATURES,
        TRON_MAX_TRANSACTION_BYTES,
        TRON_MAX_TRANSACTION_MERKLE_BRANCH_NODES,
        TRON_MAX_WITNESSES,
        TRON_MAX_SOLID_BLOCK_ANCESTOR_HEADERS,
        TRON_MAX_SOLID_BLOCK_CONFIRMATION_HEADERS,
        TRON_MAX_WITNESS_SCHEDULE_TRANSITIONS,
        TRON_MAX_WITNESS_SCHEDULE_PAYLOAD_BYTES,
    ):
        _push_u32(payload, bound)
    return _prefixed_blake2b(TRON_DPOS_SOURCE_GATE_LABEL, bytes(payload))


def _tron_template_component_hash(component_id: str, component_kind: str) -> bytes:
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, SCCP_DOMAIN_TRON)
    _push_vec(payload, b"tron")
    _push_u8(payload, TRON_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, TRON_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_SOURCE_ADAPTER_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, TRON_GROTH16_BACKEND.encode("utf-8"))
    for prefix in TRON_TEMPLATE_TRANSCRIPT_PREFIXES:
        _push_vec(payload, prefix)
    _push_vec(payload, component_kind.encode("utf-8"))
    _push_vec(payload, component_id.encode("utf-8"))
    return _prefixed_blake2b(
        b"sccp:tron:source-verifier-material:v1",
        bytes(payload),
    )


def _require_live_source_component_hashes(args: argparse.Namespace) -> None:
    for field, (component_id, component_kind) in TRON_TEMPLATE_COMPONENTS.items():
        supplied = getattr(args, field, None)
        if supplied is None:
            continue
        if supplied == _tron_template_component_hash(component_id, component_kind):
            label = field.replace("_", " ")
            raise ValueError(
                f"TRON production source evidence requires live {label}; "
                f"template-derived {label} is not deployable"
            )


def _require_source_role_hash_separation(
    args: argparse.Namespace,
    config_hash: bytes,
) -> None:
    seen: dict[bytes, str] = {}
    role_hashes = (
        ("source_trust_anchor_hash", getattr(args, "source_trust_anchor_hash", None)),
        ("consensus_verifier_hash", getattr(args, "consensus_verifier_hash", None)),
        (
            "message_inclusion_verifier_hash",
            getattr(args, "message_inclusion_verifier_hash", None),
        ),
        ("finality_policy_hash", getattr(args, "finality_policy_hash", None)),
        (
            "source_bridge_emitter_code_hash",
            getattr(args, "source_bridge_emitter_code_hash", None),
        ),
        ("source_bridge_network_id", getattr(args, "network_id", None)),
        ("source_bridge_config_hash", config_hash),
        ("adapter_verifier_vk_hash", getattr(args, "adapter_verifier_vk_hash", None)),
        ("deployment_receipt_hash", getattr(args, "deployment_receipt_hash", None)),
    )
    for field, value in role_hashes:
        if value is None:
            continue
        previous_field = seen.get(value)
        if previous_field is not None:
            raise ValueError(
                "TRON source-adapter role hashes must be distinct: "
                f"{field} matches {previous_field}"
            )
        seen[value] = field


def runtime_bytecode_hash(runtime_bytecode: bytes) -> bytes:
    """Compute the deployed TVM/EVM runtime bytecode hash used in SCCP evidence."""

    if not runtime_bytecode or not any(runtime_bytecode):
        raise ValueError("runtime bytecode must not be empty or all zero")
    return _keccak_256(runtime_bytecode)


def _apply_runtime_bytecode_hash(
    args: argparse.Namespace,
    *,
    hash_attr: str,
    runtime_hex_attr: str,
    runtime_file_attr: str,
    hash_option: str,
    runtime_label: str,
) -> None:
    runtime_hex = getattr(args, runtime_hex_attr, None)
    runtime_file = getattr(args, runtime_file_attr, None)
    if runtime_hex is not None and runtime_file is not None:
        raise ValueError(
            f"--{runtime_hex_attr.replace('_', '-')} and "
            f"--{runtime_file_attr.replace('_', '-')} cannot both be supplied"
        )
    runtime_bytecode = runtime_hex if runtime_hex is not None else runtime_file
    if runtime_bytecode is None:
        return
    derived_hash = runtime_bytecode_hash(runtime_bytecode)
    setattr(args, runtime_hex_attr + "_text", _hex(bytes(runtime_bytecode)))
    supplied_hash = getattr(args, hash_attr, None)
    if supplied_hash is not None and supplied_hash != derived_hash:
        raise ValueError(
            f"--{hash_option} does not match {runtime_label}: "
            f"expected {_hex(supplied_hash)}, got {_hex(derived_hash)}"
        )
    setattr(args, hash_attr, derived_hash)


def apply_runtime_bytecode_hashes(args: argparse.Namespace) -> None:
    """Fill or verify deployment code hashes derived from runtime bytecode."""

    _apply_runtime_bytecode_hash(
        args,
        hash_attr="source_bridge_emitter_code_hash",
        runtime_hex_attr="source_bridge_runtime_bytecode_hex",
        runtime_file_attr="source_bridge_runtime_bytecode_file",
        hash_option="source-bridge-emitter-code-hash",
        runtime_label="source bridge runtime bytecode",
    )
    _apply_runtime_bytecode_hash(
        args,
        hash_attr="destination_verifier_code_hash",
        runtime_hex_attr="destination_verifier_runtime_bytecode_hex",
        runtime_file_attr="destination_verifier_runtime_bytecode_file",
        hash_option="destination-verifier-code-hash",
        runtime_label="destination verifier runtime bytecode",
    )


def _toml_line(key: str, value: str | int | bool) -> str:
    if isinstance(value, bool):
        encoded = "true" if value else "false"
    elif isinstance(value, int):
        encoded = str(value)
    else:
        encoded = json.dumps(value)
    return f"{key} = {encoded}"


def _material_lines(args: argparse.Namespace, config_hash: bytes) -> Iterable[str]:
    yield "[[zk.sccp_source_verifier_materials]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.source_domain)
    yield _toml_line("source_chain", "tron")
    yield _toml_line("source_proof_plan", "TronDposReceiptProof")
    yield _toml_line("finality_model", "TronDpos")
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("source_trust_anchor_id", TRON_SOURCE_TRUST_ANCHOR_ID)
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", TRON_CONSENSUS_VERIFIER_ID)
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        TRON_MESSAGE_INCLUSION_VERIFIER_ID,
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line("source_bridge_emitter_id", TRON_SOURCE_BRIDGE_EMITTER_ID)
    yield _toml_line("source_bridge_emitter_address", _hex(args.bridge_address))
    yield _toml_line(
        "source_bridge_emitter_code_hash",
        _hex(args.source_bridge_emitter_code_hash),
    )
    yield _toml_line("source_bridge_network_id", _hex(args.network_id))
    yield _toml_line("source_bridge_owner_address", _hex(args.owner_address))
    yield _toml_line("source_bridge_config_hash", _hex(config_hash))
    yield _toml_line("finality_policy_id", TRON_FINALITY_POLICY_ID)
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("placeholder_material", False)


def _deployment_lines(args: argparse.Namespace, config_hash: bytes) -> Iterable[str]:
    yield "[[zk.sccp_source_adapter_engine_deployments]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.source_domain)
    yield _toml_line("target_domain", args.target_domain)
    yield _toml_line("source_chain", "tron")
    yield _toml_line("source_proof_plan", "TronDposReceiptProof")
    yield _toml_line("finality_model", "TronDpos")
    yield _toml_line("adapter_proof_family", "stark-fri-v1")
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("adapter_verifier_vk_hash", _hex(args.adapter_verifier_vk_hash))
    yield _toml_line("source_trust_anchor_id", TRON_SOURCE_TRUST_ANCHOR_ID)
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", TRON_CONSENSUS_VERIFIER_ID)
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        TRON_MESSAGE_INCLUSION_VERIFIER_ID,
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line("source_bridge_emitter_id", TRON_SOURCE_BRIDGE_EMITTER_ID)
    yield _toml_line("source_bridge_emitter_address", _hex(args.bridge_address))
    yield _toml_line(
        "source_bridge_emitter_code_hash",
        _hex(args.source_bridge_emitter_code_hash),
    )
    yield _toml_line("source_bridge_network_id", _hex(args.network_id))
    yield _toml_line("source_bridge_owner_address", _hex(args.owner_address))
    yield _toml_line("source_bridge_config_hash", _hex(config_hash))
    yield _toml_line("finality_policy_id", TRON_FINALITY_POLICY_ID)
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("deployment_receipt_hash", _hex(args.deployment_receipt_hash))
    yield _toml_line(
        "tron_dpos_source_gate_hash",
        _hex(tron_dpos_source_gate_hash(args, config_hash)),
    )


def _destination_rollout_lines(args: argparse.Namespace) -> Iterable[str]:
    yield "[[zk.sccp_destination_rollouts]]"
    yield _toml_line("version", 1)
    yield _toml_line("domain", SCCP_DOMAIN_TRON)
    yield _toml_line("chain", "tron")
    yield _toml_line("verifier_plan", "TronContractGroth16Bn254")
    yield _toml_line("immutable_verifier_ready", True)
    yield _toml_line("anchors_ready", True)
    yield _toml_line("verifier_identity", args.destination_verifier_address)
    yield _toml_line("verifier_code_hash", _hex(args.destination_verifier_code_hash))
    yield _toml_line("verifier_key_hash", _hex(args.destination_verifier_key_hash))
    yield _toml_line("destination_network_id", _hex(args.network_id))
    yield _toml_line("destination_binding_key", _destination_binding_key_from_args(args))
    yield _toml_line(
        "destination_binding_hash", _hex(_destination_binding_hash_from_args(args))
    )
    yield _toml_line("anchor_id", TRON_DESTINATION_ANCHOR_ID)
    yield _toml_line("blockers", [])


def _route_allowlist_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
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
    yield _toml_line("domain", SCCP_DOMAIN_TRON)
    yield _toml_line("chain", "tron")
    yield _toml_line("activation_policy", "GovernanceAllowlist")
    yield _toml_line("route_allowlist_id", TRON_ROUTE_ALLOWLIST_ID)
    yield _toml_line("route_allowlist_hash", _hex(route_allowlist_hash))
    yield from _route_canary_toml_lines(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
    )
    yield _toml_line("routes_allowlisted", True)
    yield _toml_line("blockers", [])


def _route_canary_toml_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
) -> list[str]:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
    )
    if canary_hash is None:
        return []
    lines = [
        _toml_line("route_canary_status", "passed"),
        _toml_line("route_canary_evidence_hash", _hex(canary_hash)),
        _toml_line("route_canary_route_allowlist_hash", _hex(route_allowlist_hash)),
        _toml_line(
            "route_canary_destination_binding_hash",
            _hex(destination_binding_hash),
        ),
    ]
    values = _route_canary_transaction_values(args)
    if values is not None:
        lines.extend(
            [
                _toml_line(
                    "tron_route_canary_transaction_id",
                    _hex(values["transaction_id"]),
                ),
                _toml_line(
                    "tron_route_canary_transaction_owner_address",
                    _hex(values["transaction_owner_address"]),
                ),
                _toml_line(
                    "tron_route_canary_block_number",
                    values["block_number"],
                ),
                _toml_line(
                    "tron_route_canary_block_timestamp",
                    values["block_timestamp"],
                ),
                _toml_line("tron_route_canary_log_index", values["log_index"]),
                _toml_line(
                    "tron_route_canary_message_id",
                    _hex(values["message_id"]),
                ),
                _toml_line(
                    "tron_route_canary_call_data_sha256",
                    _hex(values["call_data_sha256"]),
                ),
                _toml_line(
                    "tron_route_canary_payload_hash",
                    _hex(values["payload_hash"]),
                ),
                _toml_line(
                    "tron_route_canary_target_domain",
                    values["target_domain"],
                ),
                _toml_line(
                    "tron_route_canary_statement_hash",
                    _hex(values["statement_hash"]),
                ),
                _toml_line(
                    "tron_route_canary_commitment_root",
                    _hex(values["commitment_root"]),
                ),
                _toml_line(
                    "tron_route_canary_finality_height",
                    _hex(values["finality_height"]),
                ),
                _toml_line(
                    "tron_route_canary_finality_block_hash",
                    _hex(values["finality_block_hash"]),
                ),
                _toml_line(
                    "tron_route_canary_proof_version",
                    values["proof_version"],
                ),
                _toml_line(
                    "tron_route_canary_proof_source_domain",
                    values["proof_source_domain"],
                ),
                _toml_line(
                    "tron_route_canary_used_message_proof",
                    values["used_message_proof"],
                ),
                _toml_line(
                    "tron_route_canary_raw_data_owner_matches_transaction",
                    values["raw_data_owner_matches_transaction"],
                ),
                _toml_line(
                    "tron_route_canary_signature_sha256",
                    _hex(values["signature_sha256"]),
                ),
                _toml_line(
                    "tron_route_canary_signature_recovered_address",
                    _hex(values["signature_recovered_address"]),
                ),
                _toml_line(
                    "tron_route_canary_signature_recovers_to_owner",
                    values["signature_recovers_to_owner"],
                ),
            ]
        )
    return lines


_ROUTE_CANARY_TRANSACTION_FIELDS = (
    "route_canary_transaction_id",
    "route_canary_transaction_owner_address",
    "route_canary_block_number",
    "route_canary_block_timestamp",
    "route_canary_log_index",
    "route_canary_message_id",
    "route_canary_call_data_sha256",
    "route_canary_payload_hash",
    "route_canary_target_domain",
    "route_canary_statement_hash",
    "route_canary_commitment_root",
    "route_canary_finality_height",
    "route_canary_finality_block_hash",
    "route_canary_proof_version",
    "route_canary_proof_source_domain",
    "route_canary_used_message_proof",
    "route_canary_raw_data_owner_matches_transaction",
    "route_canary_signature_sha256",
    "route_canary_signature_recovered_address",
    "route_canary_signature_recovers_to_owner",
)


_ROUTE_CANARY_TRANSCRIPT_HASH_FIELDS = (
    "transaction_id",
    "message_id",
    "call_data_sha256",
    "payload_hash",
    "statement_hash",
    "commitment_root",
    "finality_height",
    "finality_block_hash",
    "signature_sha256",
)


def _route_canary_transaction_supplied(args: argparse.Namespace) -> bool:
    return any(getattr(args, name, None) is not None for name in _ROUTE_CANARY_TRANSACTION_FIELDS)


def _require_route_canary_transcript_hashes_distinct(values: dict[str, object]) -> None:
    seen: dict[bytes, str] = {}
    for field in _ROUTE_CANARY_TRANSCRIPT_HASH_FIELDS:
        value = values[field]
        if not isinstance(value, (bytes, bytearray)):
            continue
        raw = bytes(value)
        if not any(raw):
            continue
        previous_field = seen.get(raw)
        if previous_field is not None:
            raise ValueError(
                "TRON route canary transcript hashes must be distinct: "
                f"{field} matches {previous_field}"
            )
        seen[raw] = field


def _route_canary_transaction_values(args: argparse.Namespace) -> dict[str, object] | None:
    if not _route_canary_transaction_supplied(args):
        return None
    missing = [
        name for name in _ROUTE_CANARY_TRANSACTION_FIELDS if getattr(args, name, None) is None
    ]
    if missing:
        formatted = ", ".join(f"--{name.replace('_', '-')}" for name in missing)
        raise ValueError("route canary transaction metadata requires " + formatted)
    destination_source_domain = _require_exact_u32(
        getattr(args, "destination_source_domain"),
        "destination_source_domain",
    )
    destination_target_domain = _require_exact_u32(
        getattr(args, "destination_target_domain"),
        "destination_target_domain",
    )
    if (
        destination_source_domain != SCCP_DOMAIN_SORA
        or destination_target_domain != SCCP_DOMAIN_TRON
    ):
        raise ValueError(
            "route canary transaction metadata requires the production "
            "SORA -> TRON destination lane"
        )
    log_index = _require_exact_u32(
        getattr(args, "route_canary_log_index"),
        "route_canary_log_index",
    )
    target_domain = _require_exact_u32(
        getattr(args, "route_canary_target_domain"),
        "route_canary_target_domain",
    )
    if target_domain != destination_target_domain:
        raise ValueError(
            "route canary target domain must match destination_target_domain"
        )
    proof_version = _require_exact_u32(
        getattr(args, "route_canary_proof_version"),
        "route_canary_proof_version",
    )
    if proof_version != 1:
        raise ValueError("route canary proof version must be 1")
    proof_source_domain = _require_exact_u32(
        getattr(args, "route_canary_proof_source_domain"),
        "route_canary_proof_source_domain",
    )
    if proof_source_domain != destination_source_domain:
        raise ValueError(
            "route canary proof source domain must match destination_source_domain"
        )
    if getattr(args, "route_canary_used_message_proof") is not True:
        raise ValueError(
            "route canary transaction metadata requires "
            "--route-canary-used-message-proof from live verifier state"
        )
    if getattr(args, "route_canary_raw_data_owner_matches_transaction") is not True:
        raise ValueError(
            "route canary transaction metadata requires "
            "--route-canary-raw-data-owner-matches-transaction from live transaction "
            "readback"
        )
    transaction_owner_address = _require_fixed_bytes(
        getattr(args, "route_canary_transaction_owner_address"),
        label="route_canary_transaction_owner_address",
        byte_length=21,
    )
    if transaction_owner_address[0] != 0x41 or not any(transaction_owner_address[1:]):
        raise ValueError(
            "route canary transaction owner address must be a non-zero "
            "0x41-prefixed TRON address"
        )
    block_number = _require_exact_u64(
        getattr(args, "route_canary_block_number"),
        "route_canary_block_number",
        positive=True,
    )
    block_timestamp = _require_exact_u64(
        getattr(args, "route_canary_block_timestamp"),
        "route_canary_block_timestamp",
    )
    signature_sha256 = _require_fixed_bytes(
        getattr(args, "route_canary_signature_sha256"),
        label="route_canary_signature_sha256",
        byte_length=32,
    )
    if not any(signature_sha256):
        raise ValueError("route canary signature hash must be non-zero")
    signature_recovered_address = _require_fixed_bytes(
        getattr(args, "route_canary_signature_recovered_address"),
        label="route_canary_signature_recovered_address",
        byte_length=21,
    )
    if signature_recovered_address[0] != 0x41 or not any(signature_recovered_address[1:]):
        raise ValueError(
            "route canary signature recovered address must be a non-zero "
            "0x41-prefixed TRON address"
        )
    if getattr(args, "route_canary_signature_recovers_to_owner") is not True:
        raise ValueError(
            "route canary transaction metadata requires "
            "--route-canary-signature-recovers-to-owner from live signature recovery"
        )
    if signature_recovered_address != transaction_owner_address:
        raise ValueError(
            "route canary signature recovered address must match the transaction owner"
        )
    values = {
        "transaction_id": _require_fixed_bytes(
            getattr(args, "route_canary_transaction_id"),
            label="route_canary_transaction_id",
            byte_length=32,
        ),
        "transaction_owner_address": transaction_owner_address,
        "block_number": block_number,
        "block_timestamp": block_timestamp,
        "log_index": log_index,
        "message_id": _require_fixed_bytes(
            getattr(args, "route_canary_message_id"),
            label="route_canary_message_id",
            byte_length=32,
        ),
        "call_data_sha256": _require_fixed_bytes(
            getattr(args, "route_canary_call_data_sha256"),
            label="route_canary_call_data_sha256",
            byte_length=32,
        ),
        "payload_hash": _require_fixed_bytes(
            getattr(args, "route_canary_payload_hash"),
            label="route_canary_payload_hash",
            byte_length=32,
        ),
        "target_domain": target_domain,
        "statement_hash": _require_fixed_bytes(
            getattr(args, "route_canary_statement_hash"),
            label="route_canary_statement_hash",
            byte_length=32,
        ),
        "commitment_root": _require_fixed_bytes(
            getattr(args, "route_canary_commitment_root"),
            label="route_canary_commitment_root",
            byte_length=32,
        ),
        "finality_height": _require_fixed_bytes(
            getattr(args, "route_canary_finality_height"),
            label="route_canary_finality_height",
            byte_length=32,
        ),
        "finality_block_hash": _require_fixed_bytes(
            getattr(args, "route_canary_finality_block_hash"),
            label="route_canary_finality_block_hash",
            byte_length=32,
        ),
        "proof_version": proof_version,
        "proof_source_domain": proof_source_domain,
        "used_message_proof": getattr(args, "route_canary_used_message_proof"),
        "raw_data_owner_matches_transaction": getattr(
            args,
            "route_canary_raw_data_owner_matches_transaction",
        ),
        "signature_sha256": signature_sha256,
        "signature_recovered_address": signature_recovered_address,
        "signature_recovers_to_owner": getattr(
            args,
            "route_canary_signature_recovers_to_owner",
        ),
    }
    _require_route_canary_transcript_hashes_distinct(values)
    return values


def _route_canary_transaction_evidence_hash(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes | None:
    values = _route_canary_transaction_values(args)
    if values is None:
        return None
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
    expected_destination_binding_hash = _destination_binding_hash_from_args(args)
    if destination_binding_hash != expected_destination_binding_hash:
        raise ValueError(
            "destination_binding_hash does not match canonical destination "
            "binding evidence"
        )
    network_id = _require_fixed_bytes(
        args.network_id,
        label="network_id",
        byte_length=32,
    )
    if args.destination_proof_family != SCCP_PROOF_FAMILY_STARK_FRI:
        raise ValueError(
            "route canary transaction metadata requires destination proof family "
            f"{SCCP_PROOF_FAMILY_STARK_FRI}"
        )
    verifier_address = parse_tron_address(
        args.destination_verifier_address,
        label="destination_verifier_address",
    )
    payload = bytearray()
    _push_u8(payload, 3)
    payload.extend(route_allowlist_hash)
    payload.extend(b"\x41" + verifier_address)
    payload.extend(values["transaction_id"])
    payload.extend(values["transaction_owner_address"])
    _push_u64(payload, values["block_number"])
    _push_u64(payload, values["block_timestamp"])
    _push_u32(payload, values["log_index"])
    payload.extend(values["call_data_sha256"])
    payload.extend(values["message_id"])
    _push_u32(payload, args.destination_source_domain)
    _push_u32(payload, values["target_domain"])
    payload.extend(values["payload_hash"])
    payload.extend(values["commitment_root"])
    payload.extend(values["finality_height"])
    payload.extend(values["finality_block_hash"])
    payload.extend(values["statement_hash"])
    _push_u32(payload, values["proof_version"])
    _push_u32(payload, values["proof_source_domain"])
    payload.extend(destination_binding_hash)
    payload.extend(_keccak_256(TRON_GROTH16_BACKEND.encode("utf-8")))
    payload.extend(_keccak_256(args.destination_proof_family.encode("utf-8")))
    payload.extend(network_id)
    _push_u8(payload, 1 if values["used_message_proof"] is True else 0)
    _push_u8(payload, 1 if values["raw_data_owner_matches_transaction"] is True else 0)
    payload.extend(values["signature_sha256"])
    payload.extend(values["signature_recovered_address"])
    _push_u8(payload, 1 if values["signature_recovers_to_owner"] is True else 0)
    return _prefixed_blake2b(TRON_ROUTE_CANARY_EVIDENCE_LABEL, payload)


def _route_canary_transaction_comment_lines(args: argparse.Namespace) -> list[str]:
    values = _route_canary_transaction_values(args)
    if values is None:
        return []
    return [
        "# sccp_tron_route_canary_transaction_id = "
        + json.dumps(_hex(values["transaction_id"])),
        "# sccp_tron_route_canary_transaction_owner_address = "
        + json.dumps(_hex(values["transaction_owner_address"])),
        "# sccp_tron_route_canary_block_number = "
        + json.dumps(str(values["block_number"])),
        "# sccp_tron_route_canary_block_timestamp = "
        + json.dumps(str(values["block_timestamp"])),
        "# sccp_tron_route_canary_log_index = "
        + json.dumps(str(values["log_index"])),
        "# sccp_tron_route_canary_message_id = "
        + json.dumps(_hex(values["message_id"])),
        "# sccp_tron_route_canary_call_data_sha256 = "
        + json.dumps(_hex(values["call_data_sha256"])),
        "# sccp_tron_route_canary_payload_hash = "
        + json.dumps(_hex(values["payload_hash"])),
        "# sccp_tron_route_canary_target_domain = "
        + json.dumps(str(values["target_domain"])),
        "# sccp_tron_route_canary_statement_hash = "
        + json.dumps(_hex(values["statement_hash"])),
        "# sccp_tron_route_canary_commitment_root = "
        + json.dumps(_hex(values["commitment_root"])),
        "# sccp_tron_route_canary_finality_height = "
        + json.dumps(_hex(values["finality_height"])),
        "# sccp_tron_route_canary_finality_block_hash = "
        + json.dumps(_hex(values["finality_block_hash"])),
        "# sccp_tron_route_canary_proof_version = "
        + json.dumps(str(values["proof_version"])),
        "# sccp_tron_route_canary_proof_source_domain = "
        + json.dumps(str(values["proof_source_domain"])),
        "# sccp_tron_route_canary_used_message_proof = "
        + json.dumps("true" if values["used_message_proof"] is True else "false"),
        "# sccp_tron_route_canary_raw_data_owner_matches_transaction = "
        + json.dumps(
            "true" if values["raw_data_owner_matches_transaction"] is True else "false"
        ),
        "# sccp_tron_route_canary_signature_sha256 = "
        + json.dumps(_hex(values["signature_sha256"])),
        "# sccp_tron_route_canary_signature_recovered_address = "
        + json.dumps(_hex(values["signature_recovered_address"])),
        "# sccp_tron_route_canary_signature_recovers_to_owner = "
        + json.dumps("true" if values["signature_recovers_to_owner"] is True else "false"),
    ]


def _route_canary_comment_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
) -> list[str]:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
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
        *_route_canary_transaction_comment_lines(args),
    ]


def _route_canary_summary(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
) -> dict[str, object] | None:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
    )
    if canary_hash is None:
        return None
    summary: dict[str, object] = {
        "status": "passed",
        "evidence_hash": _hex(canary_hash),
        "route_allowlist_hash": _hex(route_allowlist_hash),
        "destination_binding_hash": _hex(destination_binding_hash),
        "evidence_bound": True,
    }
    values = _route_canary_transaction_values(args)
    if values is not None:
        summary.update(
            {
                "evidence_source": "tron_message_proof_accepted_transaction",
                "transaction_id": _hex(values["transaction_id"]),
                "transaction_owner_address": _hex(
                    values["transaction_owner_address"]
                ),
                "block_number": values["block_number"],
                "block_timestamp": values["block_timestamp"],
                "log_index": values["log_index"],
                "message_id": _hex(values["message_id"]),
                "call_data_sha256": _hex(values["call_data_sha256"]),
                "payload_hash": _hex(values["payload_hash"]),
                "target_domain": values["target_domain"],
                "statement_hash": _hex(values["statement_hash"]),
                "commitment_root": _hex(values["commitment_root"]),
                "finality_height": _hex(values["finality_height"]),
                "finality_block_hash": _hex(values["finality_block_hash"]),
                "proof_version": values["proof_version"],
                "proof_source_domain": values["proof_source_domain"],
                "message_proof_used": values["used_message_proof"],
                "raw_data_owner_matches_transaction": values[
                    "raw_data_owner_matches_transaction"
                ],
                "signature_sha256": _hex(values["signature_sha256"]),
                "signature_recovered_address": _hex(
                    values["signature_recovered_address"]
                ),
                "signature_recovers_to_owner": values[
                    "signature_recovers_to_owner"
                ],
            }
        )
    return summary


def _route_canary_evidence_hash(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
) -> bytes | None:
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
    expected_route_allowlist_hash = tron_route_allowlist_hash(
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if route_allowlist_hash != expected_route_allowlist_hash:
        raise ValueError(
            "route_allowlist_hash does not match canonical source, deployment, "
            "and destination evidence"
        )
    canary_hash = getattr(args, "route_canary_evidence_hash", None)
    derived_canary_hash = _route_canary_transaction_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if canary_hash is None:
        canary_hash = derived_canary_hash
    else:
        canary_hash = _require_fixed_bytes(
            canary_hash,
            label="route_canary_evidence_hash",
            byte_length=32,
        )
        if derived_canary_hash is not None and canary_hash != derived_canary_hash:
            raise ValueError(
                "route_canary_evidence_hash does not match route canary "
                "transaction metadata"
            )
    if canary_hash is None:
        return None
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
    return canary_hash


def _destination_binding_hash_from_args(args: argparse.Namespace) -> bytes:
    return tron_destination_binding_hash(
        network_id=args.network_id,
        source_domain=args.destination_source_domain,
        target_domain=args.destination_target_domain,
        verifier_address=args.destination_verifier_address,
        verifier_code_hash=args.destination_verifier_code_hash,
        verifier_key_hash=args.destination_verifier_key_hash,
        proof_family=args.destination_proof_family,
    )


def _destination_binding_key_from_args(args: argparse.Namespace) -> str:
    return tron_destination_binding_key(
        network_id=args.network_id,
        source_domain=args.destination_source_domain,
        target_domain=args.destination_target_domain,
        verifier_address=args.destination_verifier_address,
        verifier_code_hash=args.destination_verifier_code_hash,
        verifier_key_hash=args.destination_verifier_key_hash,
        proof_family=args.destination_proof_family,
    )


def _destination_binding_material_args() -> tuple[str, ...]:
    return (
        "destination_verifier_address",
        "destination_verifier_code_hash",
        "destination_verifier_key_hash",
    )


def _required_toml_args() -> tuple[str, ...]:
    return (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "source_bridge_emitter_code_hash",
        "finality_policy_hash",
        "deployment_receipt_hash",
    )


def _required_full_toml_args() -> tuple[str, ...]:
    return (
        *_required_toml_args(),
        "expected_source_verifier_material_hash",
        "expected_source_adapter_engine_deployment_hash",
        *_destination_binding_material_args(),
        "route_allowlist_hash",
        *_ROUTE_CANARY_TRANSACTION_FIELDS,
    )


def _missing_full_toml_runtime_preimages(args: argparse.Namespace) -> list[str]:
    missing: list[str] = []
    if not isinstance(
        getattr(args, "source_bridge_runtime_bytecode_hex_text", None),
        str,
    ):
        missing.append(
            "--source-bridge-runtime-bytecode-hex or "
            "--source-bridge-runtime-bytecode-file"
        )
    if not isinstance(
        getattr(args, "destination_verifier_runtime_bytecode_hex_text", None),
        str,
    ):
        missing.append(
            "--destination-verifier-runtime-bytecode-hex or "
            "--destination-verifier-runtime-bytecode-file"
        )
    return missing


def _require_full_toml_runtime_preimages(args: argparse.Namespace) -> None:
    missing = _missing_full_toml_runtime_preimages(args)
    if missing:
        raise ValueError(
            "--full-toml requires deployed runtime bytecode preimages: "
            + ", ".join(missing)
        )


def _require_tron_sora_production_lane(args: argparse.Namespace, output: str) -> None:
    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_TRON or target_domain != SCCP_DOMAIN_SORA:
        raise ValueError(
            f"--{output} requires the production TRON -> SORA source lane "
            f"(--source-domain {SCCP_DOMAIN_TRON} --target-domain {SCCP_DOMAIN_SORA})"
        )


def _require_sora_tron_destination_lane(
    args: argparse.Namespace,
    output: str,
) -> None:
    destination_source_domain = _require_exact_u32(
        args.destination_source_domain,
        "destination_source_domain",
    )
    destination_target_domain = _require_exact_u32(
        args.destination_target_domain,
        "destination_target_domain",
    )
    if (
        destination_source_domain != SCCP_DOMAIN_SORA
        or destination_target_domain != SCCP_DOMAIN_TRON
    ):
        raise ValueError(
            f"--{output} requires the production SORA -> TRON destination lane "
            f"(--destination-source-domain {SCCP_DOMAIN_SORA} "
            f"--destination-target-domain {SCCP_DOMAIN_TRON})"
        )
    if args.destination_proof_family != SCCP_PROOF_FAMILY_STARK_FRI:
        raise ValueError(
            f"--{output} requires --destination-proof-family "
            f"{SCCP_PROOF_FAMILY_STARK_FRI}"
        )


def _require_expected_hash(
    args: argparse.Namespace,
    *,
    output: str,
    option_name: str,
    attr_name: str,
    actual_hash: bytes,
) -> None:
    expected_hash = getattr(args, attr_name, None)
    if expected_hash is None:
        raise ValueError(f"--{output} requires --{option_name}")
    if expected_hash != actual_hash:
        raise ValueError(
            f"--{option_name} does not match deployment inputs: "
            f"expected {_hex(expected_hash)}, got {_hex(actual_hash)}"
        )


def _require_expected_source_record_hashes(
    args: argparse.Namespace,
    config_hash: bytes,
    *,
    output: str | None = None,
) -> None:
    expected_material_hash = getattr(
        args,
        "expected_source_verifier_material_hash",
        None,
    )
    if expected_material_hash is None:
        if output is not None:
            raise ValueError(
                f"--{output} requires --expected-source-verifier-material-hash"
            )
    else:
        material_hash = tron_source_verifier_material_record_hash(args, config_hash)
        if expected_material_hash != material_hash:
            raise ValueError(
                "--expected-source-verifier-material-hash does not match the "
                "canonical TRON source verifier material record: "
                f"expected {_hex(expected_material_hash)}, got {_hex(material_hash)}"
            )

    expected_deployment_hash = getattr(
        args,
        "expected_source_adapter_engine_deployment_hash",
        None,
    )
    if expected_deployment_hash is None:
        if output is not None:
            raise ValueError(
                f"--{output} requires "
                "--expected-source-adapter-engine-deployment-hash"
            )
    else:
        deployment_hash = tron_source_adapter_engine_deployment_record_hash(
            args,
            config_hash,
        )
        if expected_deployment_hash != deployment_hash:
            raise ValueError(
                "--expected-source-adapter-engine-deployment-hash does not match "
                "the canonical TRON source-adapter deployment record: "
                f"expected {_hex(expected_deployment_hash)}, got {_hex(deployment_hash)}"
            )

    expected_gate_hash = getattr(args, "expected_tron_dpos_source_gate_hash", None)
    if expected_gate_hash is None:
        if output is not None:
            raise ValueError(
                f"--{output} requires --expected-tron-dpos-source-gate-hash"
            )
    else:
        gate_hash = tron_dpos_source_gate_hash(args, config_hash)
        if expected_gate_hash != gate_hash:
            raise ValueError(
                "--expected-tron-dpos-source-gate-hash does not match "
                "the canonical TRON DPoS source gate: "
                f"expected {_hex(expected_gate_hash)}, got {_hex(gate_hash)}"
            )


def _route_allowlist_hash_from_args(
    args: argparse.Namespace,
    config_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes:
    return tron_route_allowlist_hash(
        source_verifier_material_hash=tron_source_verifier_material_record_hash(
            args,
            config_hash,
        ),
        source_adapter_engine_deployment_hash=(
            tron_source_adapter_engine_deployment_record_hash(args, config_hash)
        ),
        destination_binding_hash=destination_binding_hash,
    )


def _require_expected_route_allowlist_hash(
    args: argparse.Namespace,
    config_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes:
    supplied_hash = _require_fixed_bytes(
        args.route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    expected_hash = _route_allowlist_hash_from_args(
        args,
        config_hash,
        destination_binding_hash,
    )
    if supplied_hash != expected_hash:
        raise ValueError(
            "--route-allowlist-hash does not match canonical source, deployment, "
            "and destination evidence: "
            f"expected {_hex(expected_hash)}, got {_hex(supplied_hash)}"
        )
    return expected_hash


def render_toml(args: argparse.Namespace, config_hash: bytes) -> str:
    """Render production source material and deployment TOML."""

    _require_tron_sora_production_lane(args, "toml")
    apply_runtime_bytecode_hashes(args)
    apply_source_adapter_verifier_vk_hash(args)
    _require_expected_hash(
        args,
        output="toml",
        option_name="expected-config-hash",
        attr_name="expected_config_hash",
        actual_hash=config_hash,
    )
    missing = [
        name for name in _required_toml_args() if getattr(args, name, None) is None
    ]
    if missing:
        formatted = ", ".join(f"--{name.replace('_', '-')}" for name in missing)
        raise ValueError(f"--toml requires {formatted}")
    _require_live_source_component_hashes(args)
    _require_source_role_hash_separation(args, config_hash)
    material_hash = tron_source_verifier_material_record_hash(args, config_hash)
    deployment_hash = tron_source_adapter_engine_deployment_record_hash(
        args,
        config_hash,
    )
    source_gate_hash = tron_dpos_source_gate_hash(args, config_hash)
    _require_expected_source_record_hashes(args, config_hash, output="toml")
    sections = [
        "# sccp_tron_source_verifier_material_hash = "
        + json.dumps(_hex(material_hash)),
        "# sccp_tron_source_bridge_address = " + json.dumps(_hex(args.bridge_address)),
        "# sccp_tron_source_bridge_runtime_code_hash = "
        + json.dumps(_hex(args.source_bridge_emitter_code_hash)),
    ]
    source_runtime_bytecode = getattr(
        args,
        "source_bridge_runtime_bytecode_hex_text",
        None,
    )
    if source_runtime_bytecode is not None:
        sections.append(
            "# sccp_tron_source_bridge_runtime_bytecode_hex = "
            + json.dumps(source_runtime_bytecode)
        )
    sections.extend(
        [
            "# sccp_tron_source_bridge_config_hash = " + json.dumps(_hex(config_hash)),
            *_material_lines(args, config_hash),
            "",
            "# sccp_tron_source_adapter_engine_deployment_hash = "
            + json.dumps(_hex(deployment_hash)),
            "# sccp_tron_dpos_source_gate_hash = "
            + json.dumps(_hex(source_gate_hash)),
            *_deployment_lines(args, config_hash),
        ]
    )
    return "\n".join(sections) + "\n"


def render_full_toml(args: argparse.Namespace, config_hash: bytes) -> str:
    """Render all TRON production lane TOML records."""

    _require_tron_sora_production_lane(args, "full-toml")
    apply_runtime_bytecode_hashes(args)
    apply_source_adapter_verifier_vk_hash(args)
    _require_sora_tron_destination_lane(args, "full-toml")
    _require_expected_hash(
        args,
        output="full-toml",
        option_name="expected-config-hash",
        attr_name="expected_config_hash",
        actual_hash=config_hash,
    )
    missing = [
        name for name in _required_full_toml_args() if getattr(args, name, None) is None
    ]
    if missing:
        formatted = ", ".join(f"--{name.replace('_', '-')}" for name in missing)
        raise ValueError(f"--full-toml requires {formatted}")
    if getattr(args, "_require_full_toml_runtime_preimages", False):
        _require_full_toml_runtime_preimages(args)
    _require_live_source_component_hashes(args)
    _require_source_role_hash_separation(args, config_hash)
    _require_expected_source_record_hashes(args, config_hash)
    destination_binding_key = _destination_binding_key_from_args(args)
    destination_binding_hash = _destination_binding_hash_from_args(args)
    _require_expected_hash(
        args,
        output="full-toml",
        option_name="expected-destination-binding-hash",
        attr_name="expected_destination_binding_hash",
        actual_hash=destination_binding_hash,
    )
    route_allowlist_hash = _require_expected_route_allowlist_hash(
        args,
        config_hash,
        destination_binding_hash,
    )
    source_verifier_material_hash = tron_source_verifier_material_record_hash(
        args,
        config_hash,
    )
    source_adapter_engine_deployment_hash = (
        tron_source_adapter_engine_deployment_record_hash(
            args,
            config_hash,
        )
    )
    source_gate_hash = tron_dpos_source_gate_hash(args, config_hash)
    if (
        _route_canary_evidence_hash(
            args,
            route_allowlist_hash=route_allowlist_hash,
            destination_binding_hash=destination_binding_hash,
            source_verifier_material_hash=source_verifier_material_hash,
            source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
        )
        is None
    ):
        raise ValueError(
            "--full-toml requires --route-canary-evidence-hash or complete "
            "--route-canary-transaction-* metadata"
        )
    sections = [
        "# sccp_tron_source_verifier_material_hash = "
        + json.dumps(_hex(source_verifier_material_hash)),
        "# sccp_tron_source_bridge_address = " + json.dumps(_hex(args.bridge_address)),
        "# sccp_tron_source_bridge_runtime_code_hash = "
        + json.dumps(_hex(args.source_bridge_emitter_code_hash)),
    ]
    source_runtime_bytecode = getattr(
        args,
        "source_bridge_runtime_bytecode_hex_text",
        None,
    )
    if source_runtime_bytecode is not None:
        sections.append(
            "# sccp_tron_source_bridge_runtime_bytecode_hex = "
            + json.dumps(source_runtime_bytecode)
        )
    sections.extend(
        [
            "# sccp_tron_source_bridge_config_hash = " + json.dumps(_hex(config_hash)),
            *_material_lines(args, config_hash),
            "",
            "# sccp_tron_source_adapter_engine_deployment_hash = "
            + json.dumps(_hex(source_adapter_engine_deployment_hash)),
            "# sccp_tron_dpos_source_gate_hash = "
            + json.dumps(_hex(source_gate_hash)),
            *_deployment_lines(args, config_hash),
            "",
            "# sccp_tron_destination_binding_hash = "
            + json.dumps(_hex(destination_binding_hash)),
            "# sccp_tron_destination_binding_key = "
            + json.dumps(destination_binding_key),
            "# sccp_tron_destination_verifier_address = "
            + json.dumps(args.destination_verifier_address),
            "# sccp_tron_destination_verifier_runtime_code_hash = "
            + json.dumps(_hex(args.destination_verifier_code_hash)),
        ]
    )
    destination_runtime_bytecode = getattr(
        args,
        "destination_verifier_runtime_bytecode_hex_text",
        None,
    )
    if destination_runtime_bytecode is not None:
        sections.append(
            "# sccp_tron_destination_verifier_runtime_bytecode_hex = "
            + json.dumps(destination_runtime_bytecode)
        )
    sections.extend(
        [
            "# sccp_tron_destination_verifier_key_hash = "
            + json.dumps(_hex(args.destination_verifier_key_hash)),
            "# sccp_tron_destination_verifier_backend_hash = "
            + json.dumps(_hex(_keccak_256(TRON_GROTH16_BACKEND.encode("utf-8")))),
            "# sccp_tron_destination_proof_family_hash = "
            + json.dumps(
                _hex(_keccak_256(SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8")))
            ),
            *_destination_rollout_lines(args),
            "",
            "# sccp_tron_route_allowlist_hash = "
            + json.dumps(_hex(route_allowlist_hash)),
            *_route_canary_comment_lines(
                args,
                route_allowlist_hash=route_allowlist_hash,
                destination_binding_hash=destination_binding_hash,
                source_verifier_material_hash=source_verifier_material_hash,
                source_adapter_engine_deployment_hash=(
                    source_adapter_engine_deployment_hash
                ),
            ),
            *_route_allowlist_lines(
                args,
                route_allowlist_hash=route_allowlist_hash,
                destination_binding_hash=destination_binding_hash,
                source_verifier_material_hash=source_verifier_material_hash,
                source_adapter_engine_deployment_hash=(
                    source_adapter_engine_deployment_hash
                ),
            ),
        ]
    )
    return "\n".join(sections) + "\n"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Compute SCCP TRON source bridge config evidence.",
    )
    parser.add_argument(
        "--bridge-address",
        required=True,
        type=lambda value: parse_tron_address(value, label="bridge address"),
        help="TRON source bridge address: 20-byte hex, 0x41-prefixed hex, or base58check.",
    )
    parser.add_argument(
        "--owner-address",
        required=True,
        type=lambda value: parse_tron_address(value, label="owner address"),
        help="Current owner address used by sourceBridgeConfigHash().",
    )
    parser.add_argument(
        "--network-id",
        required=True,
        type=lambda value: parse_hex_bytes(value, label="network id", byte_length=32),
        help="TRON network id as a non-zero bytes32 hex value.",
    )
    parser.add_argument(
        "--source-domain",
        default=SCCP_DOMAIN_TRON,
        type=lambda value: parse_u32(value, label="source domain"),
        help="SCCP source domain. Defaults to TRON (5).",
    )
    parser.add_argument(
        "--target-domain",
        default=SCCP_DOMAIN_SORA,
        type=lambda value: parse_u32(value, label="target domain"),
        help="SCCP target domain. Defaults to SORA (0).",
    )
    parser.add_argument(
        "--source-event-digest",
        type=lambda value: parse_hex_bytes(
            value,
            label="source event digest",
            byte_length=32,
        ),
        help=(
            "Optional non-zero SCCP source event digest. Compact JSON dry-runs "
            "include the owner-call calldata for "
            "submitSccpSourceEvent(uint32,uint32,bytes32)."
        ),
    )
    output_group = parser.add_mutually_exclusive_group()
    output_group.add_argument(
        "--toml",
        action="store_true",
        help=(
            "Render production TOML records instead of a compact JSON summary. "
            "Requires --expected-config-hash, both expected source record hashes, "
            "and --expected-tron-dpos-source-gate-hash."
        ),
    )
    output_group.add_argument(
        "--full-toml",
        action="store_true",
        help=(
            "Render source material, source deployment, destination rollout, "
            "and route allowlist TOML records. Requires --expected-config-hash, "
            "--expected-source-verifier-material-hash, "
            "--expected-source-adapter-engine-deployment-hash, "
            "--expected-tron-dpos-source-gate-hash, "
            "--expected-destination-binding-hash, --route-allowlist-hash, "
            "source/destination runtime bytecode preimages, and "
            "transaction-derived route canary evidence."
        ),
    )
    parser.add_argument(
        "--expected-config-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected config hash",
            byte_length=32,
        ),
        help=(
            "Optional sourceBridgeConfigHash(), SourceBridgeConfigured, or "
            "SourceBridgeConfigHash value to compare against the recomputed hash."
        ),
    )
    parser.add_argument(
        "--expected-destination-binding-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected destination binding hash",
            byte_length=32,
        ),
        help=(
            "Expected TRON destination binding hash to compare against the "
            "deployment inputs used by SccpTronGroth16Bn254MessageVerifier; "
            "required by --full-toml and optional for JSON dry-runs."
        ),
    )
    parser.add_argument(
        "--expected-source-verifier-material-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected source verifier material hash",
            byte_length=32,
        ),
        help=(
            "Optional governed TRON source verifier material record hash. "
            "Mismatches fail instead of rendering evidence; required by production TOML."
        ),
    )
    parser.add_argument(
        "--expected-source-adapter-engine-deployment-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected source adapter engine deployment hash",
            byte_length=32,
        ),
        help=(
            "Optional governed TRON source-adapter deployment record hash. "
            "Mismatches fail instead of rendering evidence; required by production TOML."
        ),
    )
    parser.add_argument(
        "--expected-tron-dpos-source-gate-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected TRON DPoS source gate hash",
            byte_length=32,
        ),
        help=(
            "Optional governed TRON DPoS source gate hash for JSON dry-runs; "
            "required by production TOML. Mismatches fail instead of rendering "
            "evidence."
        ),
    )

    for name in _required_toml_args():
        parser.add_argument(
            "--" + name.replace("_", "-"),
            type=lambda value, field=name: parse_hex_bytes(
                value,
                label=field.replace("_", " "),
                byte_length=32,
            ),
            help="Non-zero bytes32 deployment evidence for TOML output.",
        )
    parser.add_argument(
        "--adapter-verifier-vk-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="adapter verifier vk hash",
            byte_length=32,
        ),
        help=(
            "Optional OpenVerify vk hash for the TRON source adapter. When "
            "supplied, it must match the canonical TRON -> SORA verifier profile; "
            "when omitted, the helper derives it."
        ),
    )

    parser.add_argument(
        "--source-bridge-runtime-bytecode-hex",
        type=lambda value: parse_runtime_bytecode_hex(
            value,
            label="source bridge runtime bytecode",
        ),
        help=(
            "Hex-encoded deployed source bridge runtime bytecode. When supplied, "
            "the helper derives source_bridge_emitter_code_hash."
        ),
    )
    parser.add_argument(
        "--source-bridge-runtime-bytecode-file",
        type=lambda value: parse_runtime_bytecode_file(
            value,
            label="source bridge runtime bytecode",
        ),
        help=(
            "File containing hex-encoded deployed source bridge runtime bytecode. "
            "When supplied, the helper derives source_bridge_emitter_code_hash."
        ),
    )
    parser.add_argument(
        "--destination-verifier-address",
        type=lambda value: normalize_tron_base58check_address(
            value,
            label="destination verifier address",
        ),
        help="Checksummed TRON base58 verifier contract address for destination rollout TOML.",
    )
    parser.add_argument(
        "--destination-verifier-code-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="destination verifier code hash",
            byte_length=32,
        ),
        help="Non-zero deployed TRON destination verifier bytecode hash.",
    )
    parser.add_argument(
        "--destination-verifier-runtime-bytecode-hex",
        type=lambda value: parse_runtime_bytecode_hex(
            value,
            label="destination verifier runtime bytecode",
        ),
        help=(
            "Hex-encoded deployed destination verifier runtime bytecode. When "
            "supplied, the helper derives destination_verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--destination-verifier-runtime-bytecode-file",
        type=lambda value: parse_runtime_bytecode_file(
            value,
            label="destination verifier runtime bytecode",
        ),
        help=(
            "File containing hex-encoded deployed destination verifier runtime "
            "bytecode. When supplied, the helper derives destination_verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--destination-verifier-key-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="destination verifier key hash",
            byte_length=32,
        ),
        help="Non-zero deployed TRON Groth16 verifier key hash.",
    )
    parser.add_argument(
        "--destination-source-domain",
        default=SCCP_DOMAIN_SORA,
        type=lambda value: parse_u32(value, label="destination source domain"),
        help="SCCP source domain for TRON destination proof binding. Defaults to SORA (0).",
    )
    parser.add_argument(
        "--destination-target-domain",
        default=SCCP_DOMAIN_TRON,
        type=lambda value: parse_u32(value, label="destination target domain"),
        help="SCCP target domain for TRON destination proof binding. Defaults to TRON (5).",
    )
    parser.add_argument(
        "--destination-proof-family",
        default=SCCP_PROOF_FAMILY_STARK_FRI,
        help=(
            "Proof family string bound into the TRON destination proof binding. "
            "Must be stark-fri-v1."
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
            "Governed TRON route allowlist hash. For --full-toml or JSON "
            "dry-runs it must match the canonical source, deployment, and "
            "destination evidence tuple, and requires an expected destination "
            "binding hash pin."
        ),
    )
    parser.add_argument(
        "--route-canary-evidence-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary evidence hash",
            byte_length=32,
        ),
        help=(
            "Optional non-zero post-deploy route canary evidence hash to emit "
            "as all-lanes preflight metadata. For --full-toml it must match "
            "complete transaction-derived route canary metadata; if omitted, "
            "the helper derives it from that metadata."
        ),
    )
    parser.add_argument(
        "--route-canary-transaction-id",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary transaction id",
            byte_length=32,
        ),
        help="TRON MessageProofAccepted canary transaction id.",
    )
    parser.add_argument(
        "--route-canary-transaction-owner-address",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary transaction owner address",
            byte_length=21,
        ),
        help=(
            "0x41-prefixed visible TriggerSmartContract owner address from the "
            "route canary transaction."
        ),
    )
    parser.add_argument(
        "--route-canary-block-number",
        type=lambda value: parse_u64(value, label="route canary block number"),
        help="Positive block number of the route canary transaction.",
    )
    parser.add_argument(
        "--route-canary-block-timestamp",
        type=lambda value: parse_u64(value, label="route canary block timestamp"),
        help="Non-negative millisecond block timestamp of the route canary transaction.",
    )
    parser.add_argument(
        "--route-canary-log-index",
        type=lambda value: parse_u32(value, label="route canary log index"),
        help="Log index of the MessageProofAccepted canary event.",
    )
    parser.add_argument(
        "--route-canary-message-id",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary message id",
            byte_length=32,
        ),
        help="MessageProofAccepted indexed message id.",
    )
    parser.add_argument(
        "--route-canary-call-data-sha256",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary call data SHA-256",
            byte_length=32,
        ),
        help="SHA-256 hash of the exact submitSccpMessageProof calldata.",
    )
    parser.add_argument(
        "--route-canary-payload-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary payload hash",
            byte_length=32,
        ),
        help="Route canary publicInputs[1] payload hash.",
    )
    parser.add_argument(
        "--route-canary-target-domain",
        type=lambda value: parse_u32(value, label="route canary target domain"),
        help="Route canary publicInputs[2] target domain.",
    )
    parser.add_argument(
        "--route-canary-statement-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary statement hash",
            byte_length=32,
        ),
        help="MessageProofAccepted statement hash.",
    )
    parser.add_argument(
        "--route-canary-commitment-root",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary commitment root",
            byte_length=32,
        ),
        help="MessageProofAccepted commitment root.",
    )
    parser.add_argument(
        "--route-canary-finality-height",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary finality height",
            byte_length=32,
        ),
        help="Route canary publicInputs[4] finality height word.",
    )
    parser.add_argument(
        "--route-canary-finality-block-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary finality block hash",
            byte_length=32,
        ),
        help="Route canary publicInputs[5] finality block hash.",
    )
    parser.add_argument(
        "--route-canary-proof-version",
        type=lambda value: parse_u32(value, label="route canary proof version"),
        help="Route canary Groth16 proof header version.",
    )
    parser.add_argument(
        "--route-canary-proof-source-domain",
        type=lambda value: parse_u32(value, label="route canary proof source domain"),
        help="Route canary Groth16 proof header source domain.",
    )
    parser.add_argument(
        "--route-canary-used-message-proof",
        action="store_const",
        const=True,
        default=None,
        help=(
            "Assert live verifier state returned usedMessageProofs(messageId) = true "
            "for the route canary transaction."
        ),
    )
    parser.add_argument(
        "--route-canary-raw-data-owner-matches-transaction",
        action="store_const",
        const=True,
        default=None,
        help=(
            "Assert live transaction readback proved raw_data_hex owner_address "
            "matches the visible route canary TriggerSmartContract owner."
        ),
    )
    parser.add_argument(
        "--route-canary-signature-sha256",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary signature SHA-256",
            byte_length=32,
        ),
        help="SHA-256 hash of the verified route canary transaction signature.",
    )
    parser.add_argument(
        "--route-canary-signature-recovered-address",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary signature recovered address",
            byte_length=21,
        ),
        help=(
            "0x41-prefixed TRON address recovered from the route canary "
            "transaction signature."
        ),
    )
    parser.add_argument(
        "--route-canary-signature-recovers-to-owner",
        action="store_const",
        const=True,
        default=None,
        help=(
            "Assert the route canary transaction signature recovered to the "
            "transaction owner."
        ),
    )

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        apply_runtime_bytecode_hashes(args)
        config_hash = tron_source_bridge_config_hash(
            bridge_address=args.bridge_address,
            network_id=args.network_id,
            source_domain=args.source_domain,
            target_domain=args.target_domain,
            owner_address=args.owner_address,
        )
        if (
            args.expected_config_hash is not None
            and config_hash != args.expected_config_hash
        ):
            raise ValueError(
                "expected config hash does not match deployment inputs: "
                f"expected {_hex(args.expected_config_hash)}, got {_hex(config_hash)}"
            )
        if args.source_event_digest is not None and (args.toml or args.full_toml):
            raise ValueError(
                "--source-event-digest is only supported for compact JSON dry-runs"
            )
        apply_source_adapter_verifier_vk_hash(args)
        _require_live_source_component_hashes(args)
        source_material_complete = all(
            getattr(args, name) is not None for name in _required_toml_args()
        )
        if (
            args.expected_source_verifier_material_hash is not None
            or args.expected_source_adapter_engine_deployment_hash is not None
            or args.expected_tron_dpos_source_gate_hash is not None
        ) and not source_material_complete:
            missing = [
                name
                for name in _required_toml_args()
                if getattr(args, name) is None
            ]
            formatted = ", ".join(f"--{name.replace('_', '-')}" for name in missing)
            raise ValueError(
                "expected source record hashes require complete source material: "
                + formatted
            )
        if source_material_complete:
            _require_expected_source_record_hashes(args, config_hash)
        destination_binding_hash = None
        expected_route_allowlist_hash = None
        material_hash = None
        deployment_hash = None
        source_toml_ready = False
        destination_requested = (
            args.expected_destination_binding_hash is not None
            or args.route_allowlist_hash is not None
            or args.route_canary_evidence_hash is not None
            or _route_canary_transaction_supplied(args)
            or any(
                getattr(args, name) is not None
                for name in _destination_binding_material_args()
            )
        )
        if destination_requested:
            missing = [
                name
                for name in _destination_binding_material_args()
                if getattr(args, name) is None
            ]
            if missing:
                formatted = ", ".join(f"--{name.replace('_', '-')}" for name in missing)
                raise ValueError(
                    "TRON destination or route evidence requires " + formatted
                )
            destination_binding_hash = _destination_binding_hash_from_args(args)
            if (
                args.expected_destination_binding_hash is not None
                and destination_binding_hash != args.expected_destination_binding_hash
            ):
                raise ValueError(
                    "expected destination binding hash does not match deployment inputs: "
                    f"expected {_hex(args.expected_destination_binding_hash)}, "
                    f"got {_hex(destination_binding_hash)}"
                )
            if (
                args.route_canary_evidence_hash is not None
                and args.route_allowlist_hash is None
            ):
                raise ValueError(
                    "--route-canary-evidence-hash requires --route-allowlist-hash"
                )
            if (
                _route_canary_transaction_supplied(args)
                and args.route_allowlist_hash is None
            ):
                raise ValueError(
                    "--route-canary-transaction-id requires --route-allowlist-hash"
                )
            if args.route_allowlist_hash is not None:
                if args.expected_destination_binding_hash is None:
                    raise ValueError(
                        "--route-allowlist-hash requires "
                        "--expected-destination-binding-hash"
                    )
                if not source_material_complete:
                    missing = [
                        name
                        for name in _required_toml_args()
                        if getattr(args, name) is None
                    ]
                    formatted = ", ".join(
                        f"--{name.replace('_', '-')}" for name in missing
                    )
                    raise ValueError(
                        "TRON route allowlist evidence requires complete "
                        "source material: " + formatted
                    )
                expected_route_allowlist_hash = _require_expected_route_allowlist_hash(
                    args,
                    config_hash,
                    destination_binding_hash,
                )
        if args.full_toml:
            args._require_full_toml_runtime_preimages = True
            sys.stdout.write(render_full_toml(args, config_hash))
        elif args.toml:
            sys.stdout.write(render_toml(args, config_hash))
        else:
            summary = {
                "source_domain": args.source_domain,
                "target_domain": args.target_domain,
                "source_bridge_emitter_address": _hex(args.bridge_address),
                "source_bridge_network_id": _hex(args.network_id),
                "source_bridge_owner_address": _hex(args.owner_address),
                "source_bridge_config_hash": _hex(config_hash),
                "adapter_verifier_vk_hash": _hex(args.adapter_verifier_vk_hash),
                "full_toml_ready": False,
            }
            if args.expected_config_hash is not None:
                summary["expected_config_hash_matches"] = True
            if args.source_event_digest is not None:
                source_bridge_base58 = tron_base58check_from_address20(
                    args.bridge_address,
                    label="bridge_address",
                )
                owner_base58 = tron_base58check_from_address20(
                    args.owner_address,
                    label="owner_address",
                )
                source_event_call_data = tron_source_message_call_data(
                    source_domain=args.source_domain,
                    target_domain=args.target_domain,
                    source_event_digest=args.source_event_digest,
                )
                summary["source_event_digest"] = _hex(args.source_event_digest)
                summary["source_event_call_data"] = _hex(source_event_call_data)
                summary["source_event_call"] = {
                    "source_bridge_address": source_bridge_base58,
                    "source_bridge_emitter_address": _hex(args.bridge_address),
                    "source_bridge_owner_address": _hex(args.owner_address),
                    "source_bridge_owner_base58": owner_base58,
                    "source_domain": args.source_domain,
                    "target_domain": args.target_domain,
                    "source_event_digest": _hex(args.source_event_digest),
                    "source_event_call_data": _hex(source_event_call_data),
                    "submitted_source_events_checked": False,
                    "transaction_required": True,
                    "trigger_request": {
                        "endpoint": "wallet/triggersmartcontract",
                        "owner_address": owner_base58,
                        "contract_address": source_bridge_base58,
                        "function_selector": TRON_SOURCE_MESSAGE_CALL_ABI.decode(
                            "ascii"
                        ),
                        "parameter": source_event_call_data[4:].hex(),
                        "visible": True,
                        "call_value": 0,
                    },
                }
            source_runtime_bytecode = getattr(
                args,
                "source_bridge_runtime_bytecode_hex_text",
                None,
            )
            if isinstance(source_runtime_bytecode, str):
                summary["source_bridge_runtime_bytecode_hex"] = (
                    source_runtime_bytecode
                )
            if source_material_complete:
                material_hash = tron_source_verifier_material_record_hash(
                    args,
                    config_hash,
                )
                deployment_hash = tron_source_adapter_engine_deployment_record_hash(
                    args,
                    config_hash,
                )
                source_gate_hash = tron_dpos_source_gate_hash(args, config_hash)
                expected_material_matches = (
                    args.expected_source_verifier_material_hash == material_hash
                )
                expected_deployment_matches = (
                    args.expected_source_adapter_engine_deployment_hash
                    == deployment_hash
                )
                expected_source_gate_hash = getattr(
                    args,
                    "expected_tron_dpos_source_gate_hash",
                    None,
                )
                summary["source_verifier_material_hash"] = _hex(material_hash)
                summary["source_adapter_engine_deployment_hash"] = _hex(
                    deployment_hash
                )
                summary["tron_dpos_source_gate_hash"] = _hex(source_gate_hash)
                summary["expected_source_verifier_material_hash_matches"] = (
                    expected_material_matches
                )
                summary["expected_source_adapter_engine_deployment_hash_matches"] = (
                    expected_deployment_matches
                )
                if expected_source_gate_hash is not None:
                    summary["expected_tron_dpos_source_gate_hash_matches"] = (
                        expected_source_gate_hash == source_gate_hash
                    )
                else:
                    summary["expected_tron_dpos_source_gate_hash_matches"] = False
                source_toml_ready = (
                    args.expected_config_hash is not None
                    and expected_material_matches
                    and expected_deployment_matches
                    and summary["expected_tron_dpos_source_gate_hash_matches"]
                )
                summary["toml_ready"] = source_toml_ready
            if destination_binding_hash is not None:
                destination_binding_matches = (
                    args.expected_destination_binding_hash == destination_binding_hash
                )
                summary.update(
                    {
                        "destination_source_domain": args.destination_source_domain,
                        "destination_target_domain": args.destination_target_domain,
                        "destination_verifier_address": args.destination_verifier_address,
                        "destination_binding_key": _destination_binding_key_from_args(
                            args
                        ),
                        "destination_binding_hash": _hex(destination_binding_hash),
                        "expected_destination_binding_hash_matches": (
                            destination_binding_matches
                        ),
                    }
                )
                destination_runtime_bytecode = getattr(
                    args,
                    "destination_verifier_runtime_bytecode_hex_text",
                    None,
                )
                if isinstance(destination_runtime_bytecode, str):
                    summary["destination_verifier_runtime_bytecode_hex"] = (
                        destination_runtime_bytecode
                    )
                if args.route_allowlist_hash is not None:
                    summary["route_allowlist_hash"] = _hex(args.route_allowlist_hash)
                    summary["expected_route_allowlist_hash"] = _hex(
                        expected_route_allowlist_hash
                    )
                    summary["expected_route_allowlist_hash_matches"] = True
                    route_canary = _route_canary_summary(
                        args,
                        route_allowlist_hash=args.route_allowlist_hash,
                        destination_binding_hash=destination_binding_hash,
                        source_verifier_material_hash=material_hash,
                        source_adapter_engine_deployment_hash=deployment_hash,
                    )
                    if route_canary is not None:
                        summary["route_canary"] = route_canary
                        canary_from_transaction = (
                            route_canary.get("evidence_source")
                            == "tron_message_proof_accepted_transaction"
                        )
                        summary["full_toml_ready"] = (
                            source_toml_ready
                            and destination_binding_matches
                            and summary["expected_route_allowlist_hash_matches"]
                            and canary_from_transaction
                            and not _missing_full_toml_runtime_preimages(args)
                        )
            print(json.dumps(summary, indent=2, sort_keys=True))
    except ValueError as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
