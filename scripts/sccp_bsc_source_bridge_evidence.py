#!/usr/bin/env python3
"""Render SCCP BSC source bridge deployment evidence.

This helper is offline by design. Operators pass the governed BSC source bridge
address, deployment component hashes, adapter verifier key hash, and deployment
receipt hash collected from governance or deployment records. The script
validates that all production evidence hashes are non-zero and can render the
matching `zk.sccp_source_verifier_materials` and
`zk.sccp_source_adapter_engine_deployments` TOML records for the BSC -> SORA
source lane.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Iterable


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_DIR = REPO_ROOT / "scripts"
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sccp_client_loader import load_sccp_module  # noqa: E402


_keccak_256 = load_sccp_module()._keccak_256


SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_BSC = 2
BSC_RPC_CHAIN_ID = 56
BSC_TESTNET_RPC_CHAIN_ID = 97
SCCP_PROOF_FAMILY_STARK_FRI = "stark-fri-v1"
SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID = "sccp-source-adapter-v1"
SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET = "fastpq-lane-balanced"
SCCP_EVM_GROTH16_BN254_PROOF_BACKEND = "evm-groth16-bn254-v1"
SCCP_EVM_SOURCE_GATE_PREFIX = b"sccp:evm-family:source-gate:v1"
SCCP_EVM_RECEIPT_ROOT_VALUE_MARKER = b"sccp:evm:receipt-root-value:v1"
SCCP_EVM_SOURCE_EVENT_ABI = b"SccpSourceEvent(bytes32)"
BSC_SOURCE_PROOF_PLAN_CODE = 2
BSC_FINALITY_MODEL_CODE = 2
FASTPQ_BALANCED_TRACE_ROOT = 0x002A_247F_81C6_F850
FASTPQ_BALANCED_LDE_ROOT = 0x6026_3388_DBBF_9B2A
FASTPQ_BALANCED_OMEGA_COSET = 0x6AF3_25E8_25AD_5C18
SCCP_EVM_MAX_RECEIPT_VALUE_BYTES = 16 * 1024
SCCP_EVM_MAX_LOG_TOPICS = 4
SCCP_EVM_MAX_HEADER_RLP_BYTES = 16 * 1024
BSC_VALIDATOR_SET_CONTRACT_ADDRESS = bytes.fromhex(
    "0000000000000000000000000000000000001000"
)
BSC_PARLIA_EPOCH_LENGTH_BLOCKS = 200
BSC_VALIDATOR_SET_LENGTH_STORAGE_SLOT = 1
BSC_VALIDATOR_STRUCT_STORAGE_SLOTS = 4
BSC_MAX_PARLIA_VALIDATORS = 255
BSC_PARLIA_EXTRA_SEAL_BYTES = 65
BSC_MAX_VALIDATOR_SET_TRANSITIONS = 64

BSC_SOURCE_TRUST_ANCHOR_ID = (
    "sccp:bsc:source-trust-anchor:bsc-mainnet-validator-set:v1"
)
BSC_CONSENSUS_VERIFIER_ID = "sccp:bsc:consensus-verifier:validator-set-seal-mainnet:v1"
BSC_MESSAGE_INCLUSION_VERIFIER_ID = (
    "sccp:bsc:message-inclusion-verifier:receipt-trie-branch-mainnet:v1"
)
BSC_SOURCE_BRIDGE_EMITTER_ID = "sccp:bsc:source-bridge-emitter:bsc-mainnet:v1"
BSC_FINALITY_POLICY_ID = "sccp:bsc:finality-policy:validator-set-finality-mainnet:v1"
BSC_TEMPLATE_COMPONENTS = {
    "source_trust_anchor_hash": (
        BSC_SOURCE_TRUST_ANCHOR_ID,
        "source-trust-anchor",
    ),
    "consensus_verifier_hash": (
        BSC_CONSENSUS_VERIFIER_ID,
        "consensus-verifier",
    ),
    "message_inclusion_verifier_hash": (
        BSC_MESSAGE_INCLUSION_VERIFIER_ID,
        "message-inclusion-verifier",
    ),
    "finality_policy_hash": (
        BSC_FINALITY_POLICY_ID,
        "finality-policy",
    ),
}
BSC_NETWORK_PROFILES = {
    "mainnet": {
        "chain": "bsc",
        "rpc_chain_id": BSC_RPC_CHAIN_ID,
        "source_trust_anchor_id": BSC_SOURCE_TRUST_ANCHOR_ID,
        "consensus_verifier_id": BSC_CONSENSUS_VERIFIER_ID,
        "message_inclusion_verifier_id": BSC_MESSAGE_INCLUSION_VERIFIER_ID,
        "source_bridge_emitter_id": BSC_SOURCE_BRIDGE_EMITTER_ID,
        "finality_policy_id": BSC_FINALITY_POLICY_ID,
    },
    "testnet": {
        "chain": "bsc-testnet",
        "rpc_chain_id": BSC_TESTNET_RPC_CHAIN_ID,
        "source_trust_anchor_id": (
            "sccp:bsc:source-trust-anchor:bsc-testnet-validator-set:v1"
        ),
        "consensus_verifier_id": (
            "sccp:bsc:consensus-verifier:validator-set-seal-testnet:v1"
        ),
        "message_inclusion_verifier_id": (
            "sccp:bsc:message-inclusion-verifier:receipt-trie-branch-testnet:v1"
        ),
        "source_bridge_emitter_id": "sccp:bsc:source-bridge-emitter:bsc-testnet:v1",
        "finality_policy_id": (
            "sccp:bsc:finality-policy:validator-set-finality-testnet:v1"
        ),
    },
}
BSC_TEMPLATE_TRANSCRIPT_PREFIXES = (
    b"sccp:bsc:receipt-proof:v1",
    b"sccp:bsc:validator-set:v1",
    b"sccp:bsc:validator-set-payload:v1",
    b"sccp:bsc:commit-message:v1",
    b"sccp:bsc:commit-seal:v1",
    b"sccp:bsc:validator-set-transition-message:v1",
    b"sccp:bsc:validator-set-transition-seal:v1",
    b"sccp:bsc:validator-set-metadata:v1",
    b"sccp:bsc:validator-set-storage-value:v1",
)
BSC_SOURCE_BLOCK_TAGS = ("finalized", "safe", "latest")


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
    """Parse a fixed-width hex value."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = _strip_lower_0x_hex(value, label=label)
    if len(text) != byte_length * 2:
        raise argparse.ArgumentTypeError(f"{label} must be {byte_length} bytes")
    try:
        raw = bytes.fromhex(text)
    except (TypeError, ValueError):
        raise argparse.ArgumentTypeError(f"{label} must be hex") from None
    if nonzero and not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be zero")
    return raw


def parse_evm_address(value: str, *, label: str) -> bytes:
    """Parse a non-zero EVM address."""

    return parse_hex_bytes(value, label=label, byte_length=20)


def parse_runtime_bytecode_hex(value: str, *, label: str) -> bytes:
    """Parse non-empty runtime bytecode from hex text."""

    if value != value.strip() or any(symbol.isspace() for symbol in value):
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = _strip_lower_0x_hex(value, label=label)
    if not text:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    if len(text) % 2 != 0:
        raise argparse.ArgumentTypeError(f"{label} must have an even hex length")
    try:
        raw = bytes.fromhex(text)
    except (TypeError, ValueError):
        raise argparse.ArgumentTypeError(f"{label} must be hex") from None
    if not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be all zero")
    return raw


def parse_runtime_bytecode_file(value: str, *, label: str) -> bytes:
    """Parse runtime bytecode from a file containing hex text."""

    path = Path(value).expanduser()
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        raise argparse.ArgumentTypeError(f"{label} file cannot be read") from None
    return parse_runtime_bytecode_hex("".join(text.split()), label=label)


def parse_u32(value: str, *, label: str) -> int:
    """Parse an unsigned 32-bit integer."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must be a u32")
    text = value
    if not text or not text.isascii() or not text.isdecimal():
        raise argparse.ArgumentTypeError(f"{label} must be a u32")
    if len(text) > 1 and text.startswith("0"):
        raise argparse.ArgumentTypeError(f"{label} must be a u32")
    parsed = int(text, 10)
    if parsed < 0 or parsed > 0xFFFFFFFF:
        raise argparse.ArgumentTypeError(f"{label} must be a u32")
    return parsed


def parse_bsc_network(value: str) -> str:
    """Parse the BSC network profile selector."""

    if value != value.strip():
        raise argparse.ArgumentTypeError("BSC network must be mainnet or testnet")
    normalized = value.lower().replace("_", "-")
    aliases = {
        "mainnet": "mainnet",
        "bsc-mainnet": "mainnet",
        "56": "mainnet",
        "testnet": "testnet",
        "bsc-testnet": "testnet",
        "chapel": "testnet",
        "97": "testnet",
    }
    try:
        return aliases[normalized]
    except KeyError:
        raise argparse.ArgumentTypeError(
            "BSC network must be mainnet or testnet"
        ) from None


def _bsc_network_from_value(value: str | None) -> str:
    if value is None:
        return "mainnet"
    return parse_bsc_network(value)


def bsc_profile(bsc_network: str | None = None) -> dict[str, object]:
    """Return the BSC source profile selected for evidence rendering."""

    return BSC_NETWORK_PROFILES[_bsc_network_from_value(bsc_network)]


def _profile_from_args(args: argparse.Namespace) -> dict[str, object]:
    return bsc_profile(getattr(args, "bsc_network", None))


def bsc_template_components(
    bsc_network: str | None = None,
) -> dict[str, tuple[str, str]]:
    """Return template component IDs for the selected BSC profile."""

    profile = bsc_profile(bsc_network)
    return {
        "source_trust_anchor_hash": (
            str(profile["source_trust_anchor_id"]),
            "source-trust-anchor",
        ),
        "consensus_verifier_hash": (
            str(profile["consensus_verifier_id"]),
            "consensus-verifier",
        ),
        "message_inclusion_verifier_hash": (
            str(profile["message_inclusion_verifier_id"]),
            "message-inclusion-verifier",
        ),
        "finality_policy_hash": (
            str(profile["finality_policy_id"]),
            "finality-policy",
        ),
    }


def _require_exact_u32(value: object, label: str) -> int:
    if type(value) is not int or value < 0 or value > 0xFFFFFFFF:
        raise ValueError(f"{label} must be an exact u32")
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


def _hex(value: bytes) -> str:
    return "0x" + value.hex()


def _push_u8(out: bytearray, value: int) -> None:
    out.append(value)


def _push_u32(out: bytearray, value: int) -> None:
    out.extend(value.to_bytes(4, "little"))


def _push_u64(out: bytearray, value: int) -> None:
    out.extend(value.to_bytes(8, "little"))


def _push_vec(out: bytearray, value: bytes) -> None:
    _push_u32(out, len(value))
    out.extend(value)


def _prefixed_blake2b(prefix: bytes, payload: bytes) -> bytes:
    hasher = hashlib.blake2b(digest_size=32)
    hasher.update(prefix)
    hasher.update(payload)
    return hasher.digest()


def _require_fixed_bytes(value: bytes, *, label: str, byte_length: int) -> bytes:
    if not isinstance(value, (bytes, bytearray)):
        raise ValueError(f"{label} must be bytes")
    raw = bytes(value)
    if len(raw) != byte_length:
        raise ValueError(f"{label} must be {byte_length} bytes")
    return raw


def _require_nonzero_fixed_bytes(
    value: bytes,
    *,
    label: str,
    byte_length: int,
) -> bytes:
    raw = _require_fixed_bytes(value, label=label, byte_length=byte_length)
    if not any(raw):
        raise ValueError(f"{label} must not be zero")
    return raw


def _block_tag_from_args(args: argparse.Namespace) -> str:
    block_tag = getattr(args, "block_tag", None) or "latest"
    if block_tag not in BSC_SOURCE_BLOCK_TAGS:
        raise ValueError("block_tag must be finalized, safe, or latest")
    return block_tag


def _evm_family_template_component_hash(
    component_id: str,
    component_kind: str,
    *,
    bsc_network: str | None = None,
) -> bytes:
    profile = bsc_profile(bsc_network)
    payload = bytearray()
    payload.append(1)
    payload.extend(SCCP_DOMAIN_BSC.to_bytes(4, "little"))
    _push_vec(payload, str(profile["chain"]).encode("utf-8"))
    payload.append(2)  # BscValidatorSetReceiptProof
    payload.append(2)  # BscValidatorSet
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, SCCP_EVM_GROTH16_BN254_PROOF_BACKEND.encode("utf-8"))
    for prefix in BSC_TEMPLATE_TRANSCRIPT_PREFIXES:
        _push_vec(payload, prefix)
    _push_vec(payload, component_kind.encode("utf-8"))
    _push_vec(payload, component_id.encode("utf-8"))
    return _prefixed_blake2b(b"sccp:evm-family:source-verifier-material:v1", payload)


def bsc_source_adapter_verifier_vk_hash(
    *,
    source_domain: int = SCCP_DOMAIN_BSC,
    target_domain: int = SCCP_DOMAIN_SORA,
    bsc_network: str | None = None,
) -> bytes:
    """Compute Rust's canonical OpenVerify vk hash for BSC -> SORA."""

    profile = bsc_profile(bsc_network)
    source_domain = _require_exact_u32(source_domain, "source_domain")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_BSC:
        raise ValueError("source_domain must be BSC")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")

    verifier = bytearray()
    _push_u8(verifier, 1)
    _push_vec(verifier, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(verifier, str(profile["chain"]).encode("utf-8"))
    _push_u32(verifier, source_domain)
    _push_u32(verifier, target_domain)
    _push_u8(verifier, BSC_SOURCE_PROOF_PLAN_CODE)
    _push_u8(verifier, BSC_FINALITY_MODEL_CODE)
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
        SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8")
        + bytes(verifier)
    ).digest()


def bsc_source_verifier_material_record_hash(args: argparse.Namespace) -> bytes:
    """Compute Rust's canonical BSC source verifier material record hash."""

    profile = _profile_from_args(args)
    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    if source_domain != SCCP_DOMAIN_BSC:
        raise ValueError("source_domain must be BSC")
    _require_live_component_hashes(args)
    _require_source_role_hash_separation(args)
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, source_domain)
    _push_vec(payload, str(profile["chain"]).encode("utf-8"))
    _push_u8(payload, BSC_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, BSC_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, str(profile["source_trust_anchor_id"]).encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_trust_anchor_hash,
            label="source_trust_anchor_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, str(profile["consensus_verifier_id"]).encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.consensus_verifier_hash,
            label="consensus_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(
        payload,
        str(profile["message_inclusion_verifier_id"]).encode("utf-8"),
    )
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.message_inclusion_verifier_hash,
            label="message_inclusion_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, str(profile["finality_policy_id"]).encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.finality_policy_hash,
            label="finality_policy_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, b"")
    payload.extend(bytes(32))
    _push_vec(payload, str(profile["source_bridge_emitter_id"]).encode("utf-8"))
    _push_vec(
        payload,
        _require_nonzero_fixed_bytes(
            args.bridge_address,
            label="bridge_address",
            byte_length=20,
        ),
    )
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_bridge_emitter_code_hash,
            label="source_bridge_emitter_code_hash",
            byte_length=32,
        )
    )
    payload.extend(bytes(32))
    _push_vec(payload, b"")
    payload.extend(bytes(32))
    _push_u8(payload, 0)
    return _prefixed_blake2b(
        b"sccp:source-verifier-material-record:v1",
        bytes(payload),
    )


def bsc_source_adapter_engine_deployment_record_hash(
    args: argparse.Namespace,
) -> bytes:
    """Compute Rust's canonical BSC source-adapter deployment record hash."""

    profile = _profile_from_args(args)
    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_BSC:
        raise ValueError("source_domain must be BSC")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")
    _require_live_component_hashes(args)
    _require_source_role_hash_separation(args)
    adapter_verifier_vk_hash = _require_nonzero_fixed_bytes(
        args.adapter_verifier_vk_hash,
        label="adapter_verifier_vk_hash",
        byte_length=32,
    )
    expected_adapter_verifier_vk_hash = bsc_source_adapter_verifier_vk_hash(
        source_domain=source_domain,
        target_domain=target_domain,
        bsc_network=getattr(args, "bsc_network", None),
    )
    if adapter_verifier_vk_hash != expected_adapter_verifier_vk_hash:
        raise ValueError(
            "adapter_verifier_vk_hash must match the canonical "
            "BSC source-adapter verifier profile"
        )

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, source_domain)
    _push_u32(payload, target_domain)
    _push_vec(payload, str(profile["chain"]).encode("utf-8"))
    _push_u8(payload, BSC_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, BSC_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8"))
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    payload.extend(adapter_verifier_vk_hash)
    _push_vec(payload, str(profile["source_trust_anchor_id"]).encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_trust_anchor_hash,
            label="source_trust_anchor_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, str(profile["consensus_verifier_id"]).encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.consensus_verifier_hash,
            label="consensus_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(
        payload,
        str(profile["message_inclusion_verifier_id"]).encode("utf-8"),
    )
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.message_inclusion_verifier_hash,
            label="message_inclusion_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, str(profile["finality_policy_id"]).encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.finality_policy_hash,
            label="finality_policy_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, b"")
    payload.extend(bytes(32))
    _push_vec(payload, str(profile["source_bridge_emitter_id"]).encode("utf-8"))
    _push_vec(
        payload,
        _require_nonzero_fixed_bytes(
            args.bridge_address,
            label="bridge_address",
            byte_length=20,
        ),
    )
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_bridge_emitter_code_hash,
            label="source_bridge_emitter_code_hash",
            byte_length=32,
        )
    )
    payload.extend(bytes(32))
    _push_vec(payload, b"")
    payload.extend(bytes(32))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.deployment_receipt_hash,
            label="deployment_receipt_hash",
            byte_length=32,
        )
    )
    return _prefixed_blake2b(
        b"sccp:source-adapter-engine-deployment:v1",
        bytes(payload),
    )


def bsc_source_gate_hash(args: argparse.Namespace) -> bytes:
    """Compute Rust's canonical BSC EVM-family source gate hash."""

    profile = _profile_from_args(args)
    if profile.get("chain") != "bsc":
        raise ValueError("BSC source gate is only defined for mainnet")
    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_BSC:
        raise ValueError("source_domain must be BSC")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")
    source_material_hash = bsc_source_verifier_material_record_hash(args)
    deployment_hash = bsc_source_adapter_engine_deployment_record_hash(args)
    adapter_verifier_vk_hash = _require_nonzero_fixed_bytes(
        args.adapter_verifier_vk_hash,
        label="adapter_verifier_vk_hash",
        byte_length=32,
    )
    deployment_receipt_hash = _require_nonzero_fixed_bytes(
        args.deployment_receipt_hash,
        label="deployment_receipt_hash",
        byte_length=32,
    )
    bridge_address = _require_nonzero_fixed_bytes(
        args.bridge_address,
        label="bridge_address",
        byte_length=20,
    )
    source_bridge_code_hash = _require_nonzero_fixed_bytes(
        args.source_bridge_emitter_code_hash,
        label="source_bridge_emitter_code_hash",
        byte_length=32,
    )

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, source_domain)
    _push_u32(payload, target_domain)
    _push_vec(payload, b"bsc")
    _push_u8(payload, BSC_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, BSC_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, SCCP_EVM_GROTH16_BN254_PROOF_BACKEND.encode("utf-8"))
    payload.extend(source_material_hash)
    payload.extend(deployment_hash)
    payload.extend(adapter_verifier_vk_hash)
    payload.extend(deployment_receipt_hash)
    _push_vec(payload, BSC_SOURCE_TRUST_ANCHOR_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_trust_anchor_hash,
            label="source_trust_anchor_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, BSC_CONSENSUS_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.consensus_verifier_hash,
            label="consensus_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, BSC_MESSAGE_INCLUSION_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.message_inclusion_verifier_hash,
            label="message_inclusion_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, BSC_FINALITY_POLICY_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.finality_policy_hash,
            label="finality_policy_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, BSC_SOURCE_BRIDGE_EMITTER_ID.encode("utf-8"))
    _push_vec(payload, bridge_address)
    payload.extend(source_bridge_code_hash)
    payload.extend(bytes(32))
    _push_vec(payload, b"")
    payload.extend(bytes(32))
    _push_vec(payload, SCCP_EVM_RECEIPT_ROOT_VALUE_MARKER)
    _push_vec(payload, SCCP_EVM_SOURCE_EVENT_ABI)
    payload.extend(_keccak_256(SCCP_EVM_SOURCE_EVENT_ABI))
    _push_u32(payload, SCCP_EVM_MAX_RECEIPT_VALUE_BYTES)
    _push_u32(payload, SCCP_EVM_MAX_LOG_TOPICS)
    _push_u32(payload, SCCP_EVM_MAX_HEADER_RLP_BYTES)
    _push_vec(payload, b"sccp:bsc:receipt-proof:v1")
    for prefix in BSC_TEMPLATE_TRANSCRIPT_PREFIXES:
        _push_vec(payload, prefix)
    _push_vec(payload, BSC_VALIDATOR_SET_CONTRACT_ADDRESS)
    _push_u64(payload, BSC_PARLIA_EPOCH_LENGTH_BLOCKS)
    _push_u64(payload, BSC_VALIDATOR_SET_LENGTH_STORAGE_SLOT)
    _push_u64(payload, BSC_VALIDATOR_STRUCT_STORAGE_SLOTS)
    _push_u32(payload, BSC_MAX_PARLIA_VALIDATORS)
    _push_u32(payload, BSC_PARLIA_EXTRA_SEAL_BYTES)
    _push_u32(payload, BSC_MAX_VALIDATOR_SET_TRANSITIONS)
    return _prefixed_blake2b(SCCP_EVM_SOURCE_GATE_PREFIX, bytes(payload))


def _toml_string(value: str) -> str:
    return json.dumps(value)


def _toml_line(key: str, value: object) -> str:
    if isinstance(value, bool):
        rendered = "true" if value else "false"
    elif isinstance(value, int):
        rendered = str(value)
    elif isinstance(value, str):
        rendered = _toml_string(value)
    else:
        raise TypeError(f"unsupported TOML value for {key}")
    return f"{key} = {rendered}"


def _require_bsc_sora_lane(args: argparse.Namespace) -> None:
    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_BSC:
        raise ValueError("BSC production source evidence requires source_domain = 2")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("BSC production source evidence requires target_domain = 0")


def _require_live_component_hashes(args: argparse.Namespace) -> None:
    for field, (component_id, component_kind) in bsc_template_components(
        getattr(args, "bsc_network", None)
    ).items():
        if getattr(args, field) == _evm_family_template_component_hash(
            component_id,
            component_kind,
            bsc_network=getattr(args, "bsc_network", None),
        ):
            label = field.replace("_", " ")
            raise ValueError(
                f"BSC production source evidence requires live {label}; "
                f"template-derived {label} is not deployable"
            )


def _require_canonical_adapter_verifier_vk_hash(args: argparse.Namespace) -> None:
    expected_hash = bsc_source_adapter_verifier_vk_hash(
        source_domain=args.source_domain,
        target_domain=args.target_domain,
        bsc_network=getattr(args, "bsc_network", None),
    )
    if args.adapter_verifier_vk_hash != expected_hash:
        raise ValueError(
            "--adapter-verifier-vk-hash does not match the canonical "
            "BSC source-adapter verifier profile: "
            f"expected {_hex(expected_hash)}, got {_hex(args.adapter_verifier_vk_hash)}"
        )


def _require_source_role_hash_separation(args: argparse.Namespace) -> None:
    seen: dict[bytes, str] = {}
    for field in (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "finality_policy_hash",
        "source_bridge_emitter_code_hash",
        "adapter_verifier_vk_hash",
        "deployment_receipt_hash",
    ):
        value = getattr(args, field, None)
        if value is None:
            continue
        previous_field = seen.get(value)
        if previous_field is not None:
            raise ValueError(
                "BSC source-adapter role hashes must be distinct: "
                f"{field} matches {previous_field}"
            )
        seen[value] = field


def _require_expected_record_hashes(
    args: argparse.Namespace,
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
        material_hash = bsc_source_verifier_material_record_hash(args)
        if expected_material_hash != material_hash:
            raise ValueError(
                "--expected-source-verifier-material-hash does not match the "
                "canonical BSC source verifier material record: "
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
        deployment_hash = bsc_source_adapter_engine_deployment_record_hash(args)
        if expected_deployment_hash != deployment_hash:
            raise ValueError(
                "--expected-source-adapter-engine-deployment-hash does not match "
                "the canonical BSC source-adapter deployment record: "
                f"expected {_hex(expected_deployment_hash)}, got {_hex(deployment_hash)}"
            )


def _require_toml_receipt_metadata(
    args: argparse.Namespace,
    *,
    output: str,
) -> None:
    for field, flag in (
        ("deployment_transaction_hash", "--deployment-transaction-hash"),
        ("deployment_transaction_block_hash", "--deployment-transaction-block-hash"),
        (
            "deployment_transaction_block_number",
            "--deployment-transaction-block-number",
        ),
        (
            "deployment_transaction_input_sha256",
            "--deployment-transaction-input-sha256",
        ),
        ("deployment_receipt_contract_address", "--deployment-receipt-contract-address"),
        ("deployment_receipt_block_hash", "--deployment-receipt-block-hash"),
        ("deployment_receipt_block_number", "--deployment-receipt-block-number"),
        (
            "deployment_receipt_block_receipts_root",
            "--deployment-receipt-block-receipts-root",
        ),
    ):
        if getattr(args, field, None) is None:
            raise ValueError(f"--{output} requires {flag}")

    if args.deployment_receipt_contract_address != args.bridge_address:
        raise ValueError(
            "--deployment-receipt-contract-address must match --bridge-address"
        )
    block_number = args.deployment_receipt_block_number
    if type(block_number) is not int or block_number <= 0:
        raise ValueError("--deployment-receipt-block-number must be positive")
    transaction_block_number = args.deployment_transaction_block_number
    if type(transaction_block_number) is not int or transaction_block_number <= 0:
        raise ValueError("--deployment-transaction-block-number must be positive")
    if args.deployment_transaction_block_hash != args.deployment_receipt_block_hash:
        raise ValueError(
            "--deployment-transaction-block-hash must match "
            "--deployment-receipt-block-hash"
        )
    if args.deployment_transaction_block_number != args.deployment_receipt_block_number:
        raise ValueError(
            "--deployment-transaction-block-number must match "
            "--deployment-receipt-block-number"
        )


def _toml_receipt_metadata_ready(args: argparse.Namespace) -> bool:
    try:
        _require_toml_receipt_metadata(args, output="toml")
    except ValueError:
        return False
    return True


def _require_toml_runtime_bytecode_metadata(
    args: argparse.Namespace,
    *,
    output: str,
) -> None:
    if getattr(args, "source_bridge_runtime_bytecode_hex_text", None) is None:
        raise ValueError(
            f"--{output} requires --source-bridge-runtime-bytecode-hex or "
            "--source-bridge-runtime-bytecode-file"
        )


def _toml_runtime_bytecode_metadata_ready(args: argparse.Namespace) -> bool:
    try:
        _require_toml_runtime_bytecode_metadata(args, output="toml")
    except ValueError:
        return False
    return True


def _validate_bsc_source_evidence_args(args: argparse.Namespace) -> None:
    _require_bsc_sora_lane(args)
    _require_live_component_hashes(args)
    _require_canonical_adapter_verifier_vk_hash(args)
    _require_source_role_hash_separation(args)
    _require_expected_record_hashes(args)


def runtime_bytecode_hash(runtime_bytecode: bytes) -> bytes:
    """Compute the deployed EVM runtime bytecode hash used in SCCP evidence."""

    if not runtime_bytecode or not any(runtime_bytecode):
        raise ValueError("runtime bytecode must not be empty or all zero")
    return _keccak_256(runtime_bytecode)


def apply_runtime_bytecode_hash(args: argparse.Namespace) -> None:
    """Fill or verify the source bridge code hash from runtime bytecode."""

    runtime_hex = getattr(args, "source_bridge_runtime_bytecode_hex", None)
    runtime_file = getattr(args, "source_bridge_runtime_bytecode_file", None)
    if runtime_hex is not None and runtime_file is not None:
        raise ValueError(
            "--source-bridge-runtime-bytecode-hex and "
            "--source-bridge-runtime-bytecode-file cannot both be supplied"
    )
    runtime_bytecode = runtime_hex if runtime_hex is not None else runtime_file
    if runtime_bytecode is None:
        if getattr(args, "source_bridge_emitter_code_hash", None) is None:
            raise ValueError(
                "--source-bridge-emitter-code-hash or "
                "--source-bridge-runtime-bytecode-hex is required"
            )
        return
    derived_hash = runtime_bytecode_hash(runtime_bytecode)
    args.source_bridge_runtime_bytecode_bytes = runtime_bytecode
    args.source_bridge_runtime_bytecode_hex_text = _hex(runtime_bytecode)
    source_bridge_emitter_code_hash = getattr(
        args,
        "source_bridge_emitter_code_hash",
        None,
    )
    if (
        source_bridge_emitter_code_hash is not None
        and source_bridge_emitter_code_hash != derived_hash
    ):
        raise ValueError(
            "--source-bridge-emitter-code-hash does not match source bridge "
            f"runtime bytecode: expected {_hex(source_bridge_emitter_code_hash)}, "
            f"got {_hex(derived_hash)}"
        )
    args.source_bridge_emitter_code_hash = derived_hash


def _component_hash_args() -> tuple[str, ...]:
    return (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "source_bridge_emitter_code_hash",
        "finality_policy_hash",
        "adapter_verifier_vk_hash",
        "deployment_receipt_hash",
    )


def _material_lines(args: argparse.Namespace) -> Iterable[str]:
    profile = _profile_from_args(args)
    yield "[[zk.sccp_source_verifier_materials]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.source_domain)
    yield _toml_line("source_chain", str(profile["chain"]))
    yield _toml_line("source_proof_plan", "BscValidatorSetReceiptProof")
    yield _toml_line("finality_model", "BscValidatorSet")
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("source_trust_anchor_id", str(profile["source_trust_anchor_id"]))
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", str(profile["consensus_verifier_id"]))
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        str(profile["message_inclusion_verifier_id"]),
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line(
        "source_bridge_emitter_id",
        str(profile["source_bridge_emitter_id"]),
    )
    yield _toml_line("source_bridge_emitter_address", _hex(args.bridge_address))
    yield _toml_line(
        "source_bridge_emitter_code_hash",
        _hex(args.source_bridge_emitter_code_hash),
    )
    yield _toml_line("finality_policy_id", str(profile["finality_policy_id"]))
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("placeholder_material", False)


def _deployment_lines(args: argparse.Namespace) -> Iterable[str]:
    profile = _profile_from_args(args)
    yield "[[zk.sccp_source_adapter_engine_deployments]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.source_domain)
    yield _toml_line("target_domain", args.target_domain)
    yield _toml_line("source_chain", str(profile["chain"]))
    yield _toml_line("source_proof_plan", "BscValidatorSetReceiptProof")
    yield _toml_line("finality_model", "BscValidatorSet")
    yield _toml_line("adapter_proof_family", SCCP_PROOF_FAMILY_STARK_FRI)
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("adapter_verifier_vk_hash", _hex(args.adapter_verifier_vk_hash))
    yield _toml_line("source_trust_anchor_id", str(profile["source_trust_anchor_id"]))
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", str(profile["consensus_verifier_id"]))
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        str(profile["message_inclusion_verifier_id"]),
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line(
        "source_bridge_emitter_id",
        str(profile["source_bridge_emitter_id"]),
    )
    yield _toml_line("source_bridge_emitter_address", _hex(args.bridge_address))
    yield _toml_line(
        "source_bridge_emitter_code_hash",
        _hex(args.source_bridge_emitter_code_hash),
    )
    yield _toml_line("finality_policy_id", str(profile["finality_policy_id"]))
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("deployment_receipt_hash", _hex(args.deployment_receipt_hash))
    if profile["chain"] == "bsc":
        yield _toml_line("evm_source_gate_hash", _hex(bsc_source_gate_hash(args)))


def render_toml(args: argparse.Namespace) -> str:
    """Render BSC source material and source adapter deployment TOML records."""

    apply_runtime_bytecode_hash(args)
    _validate_bsc_source_evidence_args(args)
    profile = _profile_from_args(args)
    material_hash = bsc_source_verifier_material_record_hash(args)
    deployment_hash = bsc_source_adapter_engine_deployment_record_hash(args)
    _require_expected_record_hashes(args, output="toml")
    _require_toml_receipt_metadata(args, output="toml")
    _require_toml_runtime_bytecode_metadata(args, output="toml")
    block_tag = _block_tag_from_args(args)
    comments = [
        "# sccp_evm_source_rpc_chain_id = "
        + json.dumps(str(profile["rpc_chain_id"])),
        "# sccp_evm_source_block_tag = " + json.dumps(block_tag),
        "# sccp_evm_source_bridge_address = "
        + json.dumps(_hex(args.bridge_address)),
        "# sccp_evm_source_bridge_runtime_code_hash = "
        + json.dumps(_hex(args.source_bridge_emitter_code_hash)),
    ]
    runtime_bytecode_hex = getattr(
        args,
        "source_bridge_runtime_bytecode_hex_text",
        None,
    )
    if runtime_bytecode_hex is not None:
        comments.append(
            "# sccp_evm_source_bridge_runtime_bytecode_hex = "
            + json.dumps(runtime_bytecode_hex)
        )
    comments.extend(
        [
            "# sccp_evm_source_deployment_transaction_hash = "
            + json.dumps(_hex(args.deployment_transaction_hash)),
            "# sccp_evm_source_deployment_transaction_block_hash = "
            + json.dumps(_hex(args.deployment_transaction_block_hash)),
            "# sccp_evm_source_deployment_transaction_block_number = "
            + json.dumps(str(args.deployment_transaction_block_number)),
            "# sccp_evm_source_deployment_transaction_input_sha256 = "
            + json.dumps(args.deployment_transaction_input_sha256.hex()),
            "# sccp_evm_source_deployment_receipt_status = " + json.dumps("0x1"),
            "# sccp_evm_source_deployment_contract_address = "
            + json.dumps(_hex(args.deployment_receipt_contract_address)),
            "# sccp_evm_source_deployment_block_hash = "
            + json.dumps(_hex(args.deployment_receipt_block_hash)),
            "# sccp_evm_source_deployment_block_number = "
            + json.dumps(str(args.deployment_receipt_block_number)),
            "# sccp_evm_source_deployment_block_receipts_root = "
            + json.dumps(_hex(args.deployment_receipt_block_receipts_root)),
            "# sccp_bsc_source_verifier_material_hash = "
            + json.dumps(_hex(material_hash)),
        ]
    )
    return "\n".join(
        [
            *comments,
            *_material_lines(args),
            "",
            "# sccp_bsc_source_adapter_engine_deployment_hash = "
            + json.dumps(_hex(deployment_hash)),
            *_deployment_lines(args),
            "",
        ]
    )


def _json_summary(args: argparse.Namespace) -> dict[str, object]:
    apply_runtime_bytecode_hash(args)
    _validate_bsc_source_evidence_args(args)
    profile = _profile_from_args(args)
    material_hash = bsc_source_verifier_material_record_hash(args)
    deployment_hash = bsc_source_adapter_engine_deployment_record_hash(args)
    expected_material_matches = (
        getattr(args, "expected_source_verifier_material_hash", None) == material_hash
    )
    expected_deployment_matches = (
        getattr(args, "expected_source_adapter_engine_deployment_hash", None)
        == deployment_hash
    )
    toml_metadata_ready = _toml_receipt_metadata_ready(args)
    runtime_bytecode_ready = _toml_runtime_bytecode_metadata_ready(args)
    summary = {
        "source_domain": args.source_domain,
        "target_domain": args.target_domain,
        "source_chain": str(profile["chain"]),
        "rpc_chain_id": profile["rpc_chain_id"],
        "block_tag": _block_tag_from_args(args),
        "source_proof_plan": "BscValidatorSetReceiptProof",
        "finality_model": "BscValidatorSet",
        "source_trust_anchor_id": str(profile["source_trust_anchor_id"]),
        "consensus_verifier_id": str(profile["consensus_verifier_id"]),
        "message_inclusion_verifier_id": str(
            profile["message_inclusion_verifier_id"]
        ),
        "source_bridge_emitter_id": str(profile["source_bridge_emitter_id"]),
        "finality_policy_id": str(profile["finality_policy_id"]),
        "source_bridge_emitter_address": _hex(args.bridge_address),
        "source_bridge_emitter_code_hash": _hex(args.source_bridge_emitter_code_hash),
        "adapter_verifier_vk_hash": _hex(args.adapter_verifier_vk_hash),
        "deployment_receipt_hash": _hex(args.deployment_receipt_hash),
        "source_verifier_material_hash": _hex(material_hash),
        "source_adapter_engine_deployment_hash": _hex(deployment_hash),
        "expected_source_verifier_material_hash_matches": expected_material_matches,
        "expected_source_adapter_engine_deployment_hash_matches": (
            expected_deployment_matches
        ),
        "toml_ready": (
            expected_material_matches
            and expected_deployment_matches
            and toml_metadata_ready
            and runtime_bytecode_ready
        ),
    }
    runtime_bytecode_hex = getattr(
        args,
        "source_bridge_runtime_bytecode_hex_text",
        None,
    )
    if runtime_bytecode_hex is not None:
        summary["source_bridge_runtime_bytecode_hex"] = runtime_bytecode_hex
    if toml_metadata_ready:
        summary.update(
            {
                "deployment_transaction_hash": _hex(args.deployment_transaction_hash),
                "deployment_transaction_block_hash": _hex(
                    args.deployment_transaction_block_hash
                ),
                "deployment_transaction_block_number": (
                    args.deployment_transaction_block_number
                ),
                "deployment_transaction_input_sha256": (
                    args.deployment_transaction_input_sha256.hex()
                ),
            }
        )
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Render SCCP BSC source bridge deployment evidence.",
    )
    parser.add_argument(
        "--source-domain",
        default=SCCP_DOMAIN_BSC,
        type=lambda value: parse_u32(value, label="source domain"),
        help="SCCP source domain. Defaults to BSC (2).",
    )
    parser.add_argument(
        "--bsc-network",
        default="mainnet",
        type=parse_bsc_network,
        help=(
            "BSC network profile for source evidence: mainnet or testnet. "
            "Defaults to mainnet."
        ),
    )
    parser.add_argument(
        "--target-domain",
        default=SCCP_DOMAIN_SORA,
        type=lambda value: parse_u32(value, label="target domain"),
        help="SCCP target domain. Defaults to SORA (0).",
    )
    parser.add_argument(
        "--bridge-address",
        required=True,
        type=lambda value: parse_evm_address(value, label="bridge address"),
        help="BSC source bridge address as a non-zero 20-byte EVM hex address.",
    )
    parser.add_argument(
        "--block-tag",
        choices=BSC_SOURCE_BLOCK_TAGS,
        default="latest",
        help=(
            "BSC block tag represented by the audited source evidence. Defaults "
            "to latest."
        ),
    )
    parser.add_argument(
        "--source-bridge-runtime-bytecode-hex",
        type=lambda value: parse_runtime_bytecode_hex(
            value,
            label="source bridge runtime bytecode",
        ),
        help=(
            "Hex-encoded deployed BSC source bridge runtime bytecode. "
            "When supplied, the helper derives source_bridge_emitter_code_hash."
        ),
    )
    parser.add_argument(
        "--source-bridge-runtime-bytecode-file",
        type=lambda value: parse_runtime_bytecode_file(
            value,
            label="source bridge runtime bytecode",
        ),
        help=(
            "File containing deployed BSC source bridge runtime bytecode hex. "
            "When supplied, the helper derives source_bridge_emitter_code_hash."
        ),
    )
    for name in _component_hash_args():
        parser.add_argument(
            "--" + name.replace("_", "-"),
            required=name != "source_bridge_emitter_code_hash",
            type=lambda value, field=name: parse_hex_bytes(
                value,
                label=field.replace("_", " "),
                byte_length=32,
            ),
            help="Non-zero bytes32 BSC deployment evidence.",
        )
    parser.add_argument(
        "--expected-source-verifier-material-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected source verifier material hash",
            byte_length=32,
        ),
        help=(
            "Optional governed BSC source verifier material record hash. "
            "Mismatches fail instead of rendering evidence."
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
            "Optional governed BSC source-adapter deployment record hash. "
            "Mismatches fail instead of rendering evidence."
        ),
    )
    parser.add_argument(
        "--deployment-transaction-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="deployment transaction hash",
            byte_length=32,
        ),
        help="Audited source bridge deployment transaction hash; required for TOML.",
    )
    parser.add_argument(
        "--deployment-transaction-block-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="deployment transaction block hash",
            byte_length=32,
        ),
        help=(
            "Audited deployment transaction block hash. Must match the deployment "
            "receipt block hash and is required for TOML."
        ),
    )
    parser.add_argument(
        "--deployment-transaction-block-number",
        type=lambda value: parse_positive_u64(
            value,
            label="deployment transaction block number",
        ),
        help=(
            "Audited positive deployment transaction block number. Must match the "
            "deployment receipt block number and is required for TOML."
        ),
    )
    parser.add_argument(
        "--deployment-transaction-input-sha256",
        type=lambda value: parse_hex_bytes(
            value,
            label="deployment transaction input SHA-256",
            byte_length=32,
        ),
        help=(
            "SHA-256 of the audited non-empty contract-creation transaction input; "
            "required for TOML."
        ),
    )
    parser.add_argument(
        "--deployment-receipt-contract-address",
        type=lambda value: parse_evm_address(
            value,
            label="deployment receipt contract address",
        ),
        help=(
            "Audited deployment receipt contract address. Must match "
            "--bridge-address and is required for TOML."
        ),
    )
    parser.add_argument(
        "--deployment-receipt-block-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="deployment receipt block hash",
            byte_length=32,
        ),
        help="Audited deployment receipt block hash; required for TOML.",
    )
    parser.add_argument(
        "--deployment-receipt-block-number",
        type=lambda value: parse_positive_u64(
            value,
            label="deployment receipt block number",
        ),
        help="Audited positive deployment receipt block number; required for TOML.",
    )
    parser.add_argument(
        "--deployment-receipt-block-receipts-root",
        type=lambda value: parse_hex_bytes(
            value,
            label="deployment receipt block receiptsRoot",
            byte_length=32,
        ),
        help="Audited deployment receipt block receiptsRoot; required for TOML.",
    )
    parser.add_argument(
        "--toml",
        action="store_true",
        help="Render production TOML records instead of a compact JSON summary.",
    )
    return parser


SENSITIVE_CLI_ERROR_MARKERS = (
    "secret-token",
    "private-key",
    "private_key",
    "password",
    "passphrase",
    "bearer ",
    "authorization",
    "access-key",
    "access_key",
    "api-key",
    "api_key",
    "client-secret",
    "client_secret",
    "session=",
    "token=",
)


def _cli_error_detail(exc: BaseException, *, fallback: str) -> str:
    if isinstance(exc, OSError):
        return fallback
    text = str(exc)
    if not text:
        return fallback
    lowered = text.lower()
    if any(marker in lowered for marker in SENSITIVE_CLI_ERROR_MARKERS):
        return fallback
    if any((ord(ch) < 0x20 and ch not in "\n\t") or ord(ch) == 0x7F for ch in text):
        return fallback
    return text


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        apply_runtime_bytecode_hash(args)
        _validate_bsc_source_evidence_args(args)
        if args.toml:
            print(render_toml(args), end="")
        else:
            print(json.dumps(_json_summary(args), sort_keys=True, indent=2))
    except (OSError, ValueError) as exc:
        detail = _cli_error_detail(
            exc,
            fallback="SCCP BSC source bridge evidence rendering failed",
        )
        parser.exit(2, f"{parser.prog}: error: {detail}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
