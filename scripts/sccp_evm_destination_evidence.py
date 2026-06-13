#!/usr/bin/env python3
"""Render SCCP EVM-family destination rollout evidence.

This helper is offline by design. Operators pass the live ETH or BSC
destination verifier deployment material and bridge wrapper address; the
script defaults the EVM network id to the selected domain/profile's canonical
EIP-155 chain id and rejects mismatched overrides while recomputing the EVM
Groth16 destination binding hash. With independently pinned destination binding
and source record hashes, the script also validates the governed route
allowlist hash and can render the matching
`zk.sccp_destination_rollouts` and `zk.sccp_route_allowlists` TOML records.
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
SCCP_DOMAIN_ETH = 1
SCCP_DOMAIN_BSC = 2
SCCP_EVM_GROTH16_BACKEND = "evm-groth16-bn254-v1"
SCCP_PROOF_FAMILY_STARK_FRI = "stark-fri-v1"
EVM_DESTINATION_BINDING_LABEL = b"iroha:sccp:evm-destination-binding:v1"
SCCP_ROUTE_ALLOWLIST_LABEL = b"sccp:route-allowlist:lane-evidence:v1"
EVM_ROUTE_CANARY_EVIDENCE_LABEL = b"iroha:sccp:evm-route-canary-evidence:v4"
ETH_MAINNET_NETWORK_ID = (1).to_bytes(32, "big")
BSC_MAINNET_NETWORK_ID = (56).to_bytes(32, "big")
BSC_TESTNET_NETWORK_ID = (97).to_bytes(32, "big")

BSC_NETWORK_PROFILES = {
    "mainnet": {
        "chain": "bsc",
        "network_label": "BSC mainnet",
        "rpc_chain_id": "56",
        "block_tag": "latest",
        "network_id": BSC_MAINNET_NETWORK_ID,
        "anchor_id": "sccp:bsc:destination-anchor:bsc-mainnet:v1",
        "route_allowlist_id": "sccp:bsc:route-allowlist:bsc-mainnet:v1",
    },
    "testnet": {
        "chain": "bsc-testnet",
        "network_label": "BSC testnet",
        "rpc_chain_id": "97",
        "block_tag": "latest",
        "network_id": BSC_TESTNET_NETWORK_ID,
        "anchor_id": "sccp:bsc:destination-anchor:bsc-testnet:v1",
        "route_allowlist_id": "sccp:bsc:route-allowlist:bsc-testnet:v1",
    },
}

DOMAIN_PROFILES = {
    SCCP_DOMAIN_ETH: {
        "chain": "eth",
        "network_label": "ETH mainnet",
        "rpc_chain_id": "1",
        "block_tag": "finalized",
        "network_id": ETH_MAINNET_NETWORK_ID,
        "anchor_id": "sccp:eth:destination-anchor:ethereum-mainnet:v1",
        "route_allowlist_id": "sccp:eth:route-allowlist:ethereum-mainnet:v1",
    },
    SCCP_DOMAIN_BSC: BSC_NETWORK_PROFILES["mainnet"],
}
EVM_BLOCK_TAGS = ("finalized", "safe", "latest")


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


def parse_destination_domain(value: str) -> int:
    """Parse the destination domain selector for ETH or BSC."""

    if value != value.strip():
        raise argparse.ArgumentTypeError("domain must be eth or bsc")
    normalized = value.lower()
    aliases = {
        "eth": SCCP_DOMAIN_ETH,
        "ethereum": SCCP_DOMAIN_ETH,
        "1": SCCP_DOMAIN_ETH,
        "bsc": SCCP_DOMAIN_BSC,
        "bnb": SCCP_DOMAIN_BSC,
        "2": SCCP_DOMAIN_BSC,
    }
    try:
        return aliases[normalized]
    except KeyError:
        raise argparse.ArgumentTypeError("domain must be eth or bsc") from None


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


def profile_for_domain(domain: int, *, bsc_network: str | None = None) -> dict[str, str]:
    """Return the destination profile for an EVM SCCP domain."""

    domain = _require_exact_u32(domain, "domain")
    if domain == SCCP_DOMAIN_BSC:
        return BSC_NETWORK_PROFILES[_bsc_network_from_value(bsc_network)]
    try:
        return DOMAIN_PROFILES[domain]
    except KeyError:
        raise ValueError("domain must be ETH or BSC") from None


def evm_network_id_for_domain(domain: int, *, bsc_network: str | None = None) -> bytes:
    """Return the canonical bytes32 EIP-155 network id for an EVM SCCP profile."""

    try:
        return bytes(profile_for_domain(domain, bsc_network=bsc_network)["network_id"])
    except KeyError:  # pragma: no cover - profile validation is above.
        raise ValueError("domain must be ETH or BSC") from None


def evm_mainnet_network_id_for_domain(domain: int) -> bytes:
    """Return the canonical mainnet bytes32 EIP-155 network id for an EVM SCCP domain."""

    return evm_network_id_for_domain(domain, bsc_network="mainnet")


def _require_domain_network_id(
    domain: int,
    network_id: bytes | None,
    *,
    bsc_network: str | None = None,
) -> bytes:
    network_id = _require_fixed_bytes(network_id, label="network_id", byte_length=32)
    profile = profile_for_domain(domain, bsc_network=bsc_network)
    expected = bytes(profile["network_id"])
    if network_id != expected:
        raise ValueError(
            "network_id must match "
            f"{profile['network_label']} EIP-155 chain id "
            f"{profile['rpc_chain_id']}: expected {_hex(expected)}, got {_hex(network_id)}"
        )
    return network_id


def parse_u32_decimal(value: str, *, label: str) -> int:
    """Parse a canonical non-negative decimal u32."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must be a canonical u32 decimal")
    if not value or not value.isascii() or not value.isdecimal():
        raise argparse.ArgumentTypeError(f"{label} must be a canonical u32 decimal")
    if len(value) > 1 and value.startswith("0"):
        raise argparse.ArgumentTypeError(f"{label} must be a canonical u32 decimal")
    parsed = int(value, 10)
    if parsed > 0xFFFFFFFF:
        raise argparse.ArgumentTypeError(f"{label} must fit u32")
    return parsed


def parse_u64_decimal(value: str, *, label: str) -> int:
    """Parse a canonical non-negative decimal u64."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must be a canonical u64 decimal")
    if not value or not value.isascii() or not value.isdecimal():
        raise argparse.ArgumentTypeError(f"{label} must be a canonical u64 decimal")
    if len(value) > 1 and value.startswith("0"):
        raise argparse.ArgumentTypeError(f"{label} must be a canonical u64 decimal")
    parsed = int(value, 10)
    if parsed > 0xFFFFFFFFFFFFFFFF:
        raise argparse.ArgumentTypeError(f"{label} must fit u64")
    return parsed


def parse_bool_literal(value: str, *, label: str) -> bool:
    """Parse an exact true/false literal."""

    if value == "true":
        return True
    if value == "false":
        return False
    raise argparse.ArgumentTypeError(f"{label} must be true or false")


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


def _require_exact_u32(value: object, label: str) -> int:
    if type(value) is not int or value < 0 or value > 0xFFFFFFFF:
        raise ValueError(f"{label} must be an exact u32")
    return value


def _prefixed_blake2b(prefix: bytes, payload: bytes) -> bytes:
    hasher = hashlib.blake2b(digest_size=32)
    hasher.update(prefix)
    hasher.update(payload)
    return hasher.digest()


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


def _abi_word_address(address: bytes) -> bytes:
    return b"\x00" * 12 + address


def _abi_word_u32(value: int) -> bytes:
    return value.to_bytes(32, "big")


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


def _require_distinct_hash_roles(
    fields: tuple[tuple[str, bytes], ...],
    *,
    label: str,
) -> None:
    seen: dict[bytes, str] = {}
    for field, raw in fields:
        if not any(raw):
            continue
        previous_field = seen.get(raw)
        if previous_field is not None:
            raise ValueError(
                f"{label} must be distinct: {field} matches {previous_field}"
            )
        seen[raw] = field


def runtime_bytecode_hash(runtime_bytecode: bytes) -> bytes:
    """Compute the deployed EVM runtime bytecode hash used in SCCP evidence."""

    if not runtime_bytecode or not any(runtime_bytecode):
        raise ValueError("runtime bytecode must not be empty or all zero")
    return _keccak_256(runtime_bytecode)


def evm_verifier_backend_hash() -> bytes:
    """Return the production EVM destination verifier backend hash."""

    return _keccak_256(SCCP_EVM_GROTH16_BACKEND.encode("utf-8"))


def evm_proof_family_hash() -> bytes:
    """Return the production EVM destination proof family hash."""

    return _keccak_256(SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8"))


def apply_runtime_bytecode_hash(args: argparse.Namespace) -> None:
    """Fill or verify the destination verifier code hash from runtime bytecode."""

    runtime_hex = getattr(args, "verifier_runtime_bytecode_hex", None)
    runtime_file = getattr(args, "verifier_runtime_bytecode_file", None)
    if runtime_hex is not None and runtime_file is not None:
        raise ValueError(
            "--verifier-runtime-bytecode-hex and "
            "--verifier-runtime-bytecode-file cannot both be supplied"
        )
    runtime_bytecode = runtime_hex if runtime_hex is not None else runtime_file
    if runtime_bytecode is None:
        if getattr(args, "verifier_code_hash", None) is None:
            raise ValueError(
                "--verifier-code-hash or --verifier-runtime-bytecode-hex is required"
            )
        return
    runtime_bytecode = bytes(runtime_bytecode)
    derived_hash = runtime_bytecode_hash(runtime_bytecode)
    verifier_code_hash = getattr(args, "verifier_code_hash", None)
    if verifier_code_hash is not None and verifier_code_hash != derived_hash:
        raise ValueError(
            "--verifier-code-hash does not match verifier runtime bytecode: "
            f"expected {_hex(verifier_code_hash)}, got {_hex(derived_hash)}"
        )
    args.verifier_code_hash = derived_hash
    args.verifier_runtime_bytecode_bytes = runtime_bytecode
    args.verifier_runtime_bytecode_hex_text = "0x" + runtime_bytecode.hex()


def apply_bridge_runtime_bytecode_hash(args: argparse.Namespace) -> None:
    """Fill or verify the bridge wrapper code hash from runtime bytecode."""

    runtime_hex = getattr(args, "bridge_runtime_bytecode_hex", None)
    runtime_file = getattr(args, "bridge_runtime_bytecode_file", None)
    if runtime_hex is not None and runtime_file is not None:
        raise ValueError(
            "--bridge-runtime-bytecode-hex and "
            "--bridge-runtime-bytecode-file cannot both be supplied"
        )
    runtime_bytecode = runtime_hex if runtime_hex is not None else runtime_file
    if runtime_bytecode is None:
        return
    runtime_bytecode = bytes(runtime_bytecode)
    derived_hash = runtime_bytecode_hash(runtime_bytecode)
    bridge_code_hash = getattr(args, "bridge_code_hash", None)
    if bridge_code_hash is not None and bridge_code_hash != derived_hash:
        raise ValueError(
            "--bridge-code-hash does not match bridge runtime bytecode: "
            f"expected {_hex(bridge_code_hash)}, got {_hex(derived_hash)}"
        )
    args.bridge_code_hash = derived_hash
    args.bridge_runtime_bytecode_bytes = runtime_bytecode
    args.bridge_runtime_bytecode_hex_text = "0x" + runtime_bytecode.hex()


def _require_runtime_bytecode_evidence(args: argparse.Namespace, *, output: str) -> None:
    """Require replayable runtime bytecode for production EVM TOML."""

    def invalid_runtime_bytecode_evidence_error(label: str) -> ValueError:
        return ValueError(f"--{output} has invalid {label} evidence")

    for bytecode_attr, text_attr, code_hash_attr, label, flag in (
        (
            "bridge_runtime_bytecode_bytes",
            "bridge_runtime_bytecode_hex_text",
            "bridge_code_hash",
            "bridge runtime bytecode",
            "--bridge-runtime-bytecode-hex",
        ),
        (
            "verifier_runtime_bytecode_bytes",
            "verifier_runtime_bytecode_hex_text",
            "verifier_code_hash",
            "verifier runtime bytecode",
            "--verifier-runtime-bytecode-hex",
        ),
    ):
        runtime_bytecode = getattr(args, bytecode_attr, None)
        if not isinstance(runtime_bytecode, (bytes, bytearray)):
            runtime_text = getattr(args, text_attr, None)
            if isinstance(runtime_text, str) and runtime_text.strip():
                try:
                    runtime_bytecode = parse_runtime_bytecode_hex(
                        runtime_text,
                        label=label,
                    )
                except argparse.ArgumentTypeError:
                    raise invalid_runtime_bytecode_evidence_error(label) from None
                setattr(args, bytecode_attr, runtime_bytecode)
                setattr(args, text_attr, "0x" + bytes(runtime_bytecode).hex())
        if not isinstance(runtime_bytecode, (bytes, bytearray)):
            raise ValueError(f"--{output} requires {flag}")
        derived_hash = runtime_bytecode_hash(bytes(runtime_bytecode))
        code_hash = getattr(args, code_hash_attr, None)
        if code_hash != derived_hash:
            raise ValueError(
                f"{label} must hash to {code_hash_attr}: "
                f"expected {_hex(code_hash)}, got {_hex(derived_hash)}"
            )


def _runtime_bytecode_evidence_ready(args: argparse.Namespace) -> bool:
    """Return whether runtime bytecode evidence is complete for TOML output."""

    try:
        _require_runtime_bytecode_evidence(args, output="toml")
    except ValueError as exc:
        if "requires --" in str(exc):
            return False
        raise
    return True


def evm_destination_binding_hash(
    *,
    network_id: bytes,
    source_domain: int,
    target_domain: int,
    verifier_address: bytes,
    bridge_address: bytes,
    verifier_code_hash: bytes,
    verifier_key_hash: bytes,
    verifier_backend: str = SCCP_EVM_GROTH16_BACKEND,
    proof_family: str = SCCP_PROOF_FAMILY_STARK_FRI,
    bsc_network: str | None = None,
) -> bytes:
    """Compute the EVM destination binding used by the SCCP bridge wrapper."""

    source_domain = _require_exact_u32(source_domain, "source_domain")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_SORA:
        raise ValueError("source_domain must be SORA for EVM destination evidence")
    try:
        profile_for_domain(target_domain, bsc_network=bsc_network)
    except ValueError:
        raise ValueError("target_domain must be ETH or BSC") from None
    if source_domain == target_domain:
        raise ValueError("source_domain and target_domain must differ")
    if verifier_backend != SCCP_EVM_GROTH16_BACKEND:
        raise ValueError(f"verifier_backend must be {SCCP_EVM_GROTH16_BACKEND}")
    if proof_family != SCCP_PROOF_FAMILY_STARK_FRI:
        raise ValueError(f"proof_family must be {SCCP_PROOF_FAMILY_STARK_FRI}")

    network_id = _require_domain_network_id(
        target_domain,
        network_id,
        bsc_network=bsc_network,
    )
    verifier_address = _require_fixed_bytes(
        verifier_address,
        label="verifier_address",
        byte_length=20,
    )
    bridge_address = _require_fixed_bytes(
        bridge_address,
        label="bridge_address",
        byte_length=20,
    )
    if verifier_address == bridge_address:
        raise ValueError("verifier_address must differ from bridge_address")
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

    payload = b"".join(
        (
            _keccak_256(EVM_DESTINATION_BINDING_LABEL),
            _keccak_256(verifier_backend.encode("utf-8")),
            _keccak_256(proof_family.encode("utf-8")),
            network_id,
            _abi_word_u32(source_domain),
            _abi_word_u32(target_domain),
            _abi_word_address(verifier_address),
            _abi_word_address(bridge_address),
            verifier_code_hash,
            verifier_key_hash,
        )
    )
    return _keccak_256(payload)


def evm_destination_binding_key(
    *,
    network_id: bytes,
    source_domain: int,
    target_domain: int,
    verifier_address: bytes,
    bridge_address: bytes,
    verifier_code_hash: bytes,
    verifier_key_hash: bytes,
    verifier_backend: str = SCCP_EVM_GROTH16_BACKEND,
    proof_family: str = SCCP_PROOF_FAMILY_STARK_FRI,
    bsc_network: str | None = None,
) -> str:
    """Return the canonical Rust `SccpDestinationBindingV1.key` value."""

    source_domain = _require_exact_u32(source_domain, "source_domain")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_SORA:
        raise ValueError("source_domain must be SORA for EVM destination evidence")
    try:
        profile_for_domain(target_domain, bsc_network=bsc_network)
    except ValueError:
        raise ValueError("target_domain must be ETH or BSC") from None
    if source_domain == target_domain:
        raise ValueError("source_domain and target_domain must differ")
    if verifier_backend != SCCP_EVM_GROTH16_BACKEND:
        raise ValueError(f"verifier_backend must be {SCCP_EVM_GROTH16_BACKEND}")
    if proof_family != SCCP_PROOF_FAMILY_STARK_FRI:
        raise ValueError(f"proof_family must be {SCCP_PROOF_FAMILY_STARK_FRI}")

    network_id = _require_domain_network_id(
        target_domain,
        network_id,
        bsc_network=bsc_network,
    )
    verifier_address = _require_fixed_bytes(
        verifier_address,
        label="verifier_address",
        byte_length=20,
    )
    bridge_address = _require_fixed_bytes(
        bridge_address,
        label="bridge_address",
        byte_length=20,
    )
    if verifier_address == bridge_address:
        raise ValueError("verifier_address must differ from bridge_address")
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
    return (
        f"evm:{source_domain}:{target_domain}:{network_id.hex()}:"
        f"0x{verifier_address.hex()}:0x{bridge_address.hex()}:"
        f"0x{verifier_code_hash.hex()}:0x{verifier_key_hash.hex()}"
    )


def evm_route_allowlist_hash(
    *,
    domain: int,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
    destination_binding_hash: bytes,
    bsc_network: str | None = None,
) -> bytes:
    """Compute Rust's canonical EVM route allowlist hash."""

    domain = _require_exact_u32(domain, "domain")
    profile = profile_for_domain(domain, bsc_network=bsc_network)
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
    _require_distinct_hash_roles(
        (
            ("source_verifier_material_hash", source_verifier_material_hash),
            (
                "source_adapter_engine_deployment_hash",
                source_adapter_engine_deployment_hash,
            ),
            ("destination_binding_hash", destination_binding_hash),
        ),
        label="EVM route allowlist evidence hashes",
    )
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, domain)
    _push_vec(payload, profile["chain"].encode("utf-8"))
    _push_vec(payload, b"GovernanceAllowlist")
    _push_vec(payload, profile["route_allowlist_id"].encode("utf-8"))
    payload.extend(source_verifier_material_hash)
    payload.extend(source_adapter_engine_deployment_hash)
    payload.extend(destination_binding_hash)
    return _prefixed_blake2b(SCCP_ROUTE_ALLOWLIST_LABEL, payload)


def evm_route_canary_transaction_evidence_hash(
    *,
    route_allowlist_hash: bytes,
    bridge_address: bytes,
    transaction_hash: bytes,
    log_index: int,
    receipt_block_number: int,
    receipt_block_hash: bytes,
    block_receipts_root: bytes,
    call_data_sha256: bytes,
    message_id: bytes,
    payload_hash: bytes,
    source_domain: int,
    target_domain: int,
    commitment_root: bytes,
    finality_height: bytes,
    finality_block_hash: bytes,
    statement_hash: bytes,
    proof_version: int,
    proof_source_domain: int,
    destination_binding_hash: bytes,
    verifier_backend_hash: bytes,
    proof_family_hash: bytes,
    network_id: bytes,
    used_message_proof: bool,
    receipt_block_finalized: bool,
    bsc_network: str | None = None,
) -> bytes:
    """Compute the EVM MessageProofAccepted route canary evidence hash."""

    source_domain = _require_exact_u32(source_domain, "source_domain")
    if source_domain != SCCP_DOMAIN_SORA:
        raise ValueError("source_domain must be SORA for EVM route canaries")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    try:
        profile_for_domain(target_domain, bsc_network=bsc_network)
    except ValueError:
        raise ValueError("target_domain must be ETH or BSC for EVM route canaries") from None
    if source_domain == target_domain:
        raise ValueError("source_domain and target_domain must differ")
    proof_version = _require_exact_u32(proof_version, "proof_version")
    if proof_version != 1:
        raise ValueError("proof_version must be 1 for EVM route canaries")
    proof_source_domain = _require_exact_u32(
        proof_source_domain,
        "proof_source_domain",
    )
    if proof_source_domain != source_domain:
        raise ValueError(
            "proof_source_domain must match source_domain for EVM route canaries"
        )
    if used_message_proof is not True:
        raise ValueError("used_message_proof must be true for EVM route canaries")
    if type(receipt_block_finalized) is not bool:
        raise ValueError("receipt_block_finalized must be a boolean for EVM route canaries")
    log_index = _require_exact_u32(log_index, "log_index")
    receipt_block_number = parse_u64_decimal(
        str(receipt_block_number),
        label="receipt_block_number",
    )
    if receipt_block_number == 0:
        raise ValueError("receipt_block_number must be positive for EVM route canaries")
    route_allowlist_hash = _require_fixed_bytes(
        route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    bridge_address = _require_fixed_bytes(
        bridge_address,
        label="bridge_address",
        byte_length=20,
    )
    transaction_hash = _require_fixed_bytes(
        transaction_hash,
        label="transaction_hash",
        byte_length=32,
    )
    receipt_block_hash = _require_fixed_bytes(
        receipt_block_hash,
        label="receipt_block_hash",
        byte_length=32,
    )
    block_receipts_root = _require_fixed_bytes(
        block_receipts_root,
        label="block_receipts_root",
        byte_length=32,
    )
    call_data_sha256 = _require_fixed_bytes(
        call_data_sha256,
        label="call_data_sha256",
        byte_length=32,
    )
    message_id = _require_fixed_bytes(
        message_id,
        label="message_id",
        byte_length=32,
    )
    payload_hash = _require_fixed_bytes(
        payload_hash,
        label="payload_hash",
        byte_length=32,
    )
    commitment_root = _require_fixed_bytes(
        commitment_root,
        label="commitment_root",
        byte_length=32,
    )
    finality_height = _require_fixed_bytes(
        finality_height,
        label="finality_height",
        byte_length=32,
    )
    finality_block_hash = _require_fixed_bytes(
        finality_block_hash,
        label="finality_block_hash",
        byte_length=32,
    )
    statement_hash = _require_fixed_bytes(
        statement_hash,
        label="statement_hash",
        byte_length=32,
    )
    destination_binding_hash = _require_fixed_bytes(
        destination_binding_hash,
        label="destination_binding_hash",
        byte_length=32,
    )
    verifier_backend_hash = _require_fixed_bytes(
        verifier_backend_hash,
        label="verifier_backend_hash",
        byte_length=32,
    )
    proof_family_hash = _require_fixed_bytes(
        proof_family_hash,
        label="proof_family_hash",
        byte_length=32,
    )
    network_id = _require_domain_network_id(
        target_domain,
        network_id,
        bsc_network=bsc_network,
    )
    _require_distinct_hash_roles(
        (
            ("route_allowlist_hash", route_allowlist_hash),
            ("destination_binding_hash", destination_binding_hash),
        ),
        label="EVM route canary governed hashes",
    )
    _require_distinct_hash_roles(
        (
            ("transaction_hash", transaction_hash),
            ("receipt_block_hash", receipt_block_hash),
            ("block_receipts_root", block_receipts_root),
            ("call_data_sha256", call_data_sha256),
            ("message_id", message_id),
            ("payload_hash", payload_hash),
            ("commitment_root", commitment_root),
            ("finality_height", finality_height),
            ("finality_block_hash", finality_block_hash),
            ("statement_hash", statement_hash),
        ),
        label="EVM route canary transcript hashes",
    )

    payload = bytearray()
    _push_u8(payload, 4)
    payload.extend(route_allowlist_hash)
    payload.extend(bridge_address)
    payload.extend(transaction_hash)
    _push_u32(payload, log_index)
    _push_u64(payload, receipt_block_number)
    payload.extend(receipt_block_hash)
    payload.extend(block_receipts_root)
    payload.extend(call_data_sha256)
    payload.extend(message_id)
    _push_u32(payload, source_domain)
    _push_u32(payload, target_domain)
    payload.extend(payload_hash)
    payload.extend(commitment_root)
    payload.extend(finality_height)
    payload.extend(finality_block_hash)
    payload.extend(statement_hash)
    _push_u32(payload, proof_version)
    _push_u32(payload, proof_source_domain)
    payload.extend(destination_binding_hash)
    payload.extend(verifier_backend_hash)
    payload.extend(proof_family_hash)
    payload.extend(network_id)
    _push_u8(payload, 1)
    _push_u8(payload, 1 if receipt_block_finalized else 0)
    return _prefixed_blake2b(EVM_ROUTE_CANARY_EVIDENCE_LABEL, payload)


def _profile(args: argparse.Namespace) -> dict[str, str]:
    args.domain = _require_exact_u32(args.domain, "domain")
    return profile_for_domain(
        args.domain,
        bsc_network=getattr(args, "bsc_network", None),
    )


def _block_tag_from_args(args: argparse.Namespace) -> str:
    profile = _profile(args)
    block_tag = getattr(args, "block_tag", None) or profile["block_tag"]
    if block_tag not in EVM_BLOCK_TAGS:
        raise ValueError("block_tag must be finalized, safe, or latest")
    return block_tag


def _destination_rollout_lines(args: argparse.Namespace) -> Iterable[str]:
    profile = _profile(args)
    yield "[[zk.sccp_destination_rollouts]]"
    yield _toml_line("version", 1)
    yield _toml_line("domain", args.domain)
    yield _toml_line("chain", profile["chain"])
    yield _toml_line("verifier_plan", "EvmGroth16Bn254Adapter")
    yield _toml_line("immutable_verifier_ready", True)
    yield _toml_line("anchors_ready", True)
    yield _toml_line("verifier_identity", _hex(args.verifier_address))
    yield _toml_line("verifier_code_hash", _hex(args.verifier_code_hash))
    yield _toml_line("verifier_key_hash", _hex(args.verifier_key_hash))
    yield _toml_line("destination_network_id", _hex(args.network_id))
    yield _toml_line("destination_bridge_address", _hex(args.bridge_address))
    yield _toml_line("destination_binding_key", _destination_binding_key_from_args(args))
    yield _toml_line(
        "destination_binding_hash", _hex(_destination_binding_hash_from_args(args))
    )
    yield _toml_line("anchor_id", profile["anchor_id"])
    yield _toml_line("blockers", [])


def _route_allowlist_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> Iterable[str]:
    profile = _profile(args)
    supplied_route_allowlist_hash = _require_fixed_bytes(
        args.route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    if supplied_route_allowlist_hash != route_allowlist_hash:
        raise ValueError("route_allowlist_hash does not match validated lane evidence")
    yield "[[zk.sccp_route_allowlists]]"
    yield _toml_line("version", 1)
    yield _toml_line("domain", args.domain)
    yield _toml_line("chain", profile["chain"])
    yield _toml_line("activation_policy", "GovernanceAllowlist")
    yield _toml_line("route_allowlist_id", profile["route_allowlist_id"])
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
        *_route_canary_transaction_toml_lines(args),
    ]


_ROUTE_CANARY_TRANSACTION_FIELDS = (
    "route_canary_transaction_hash",
    "route_canary_transaction_block_number",
    "route_canary_transaction_block_hash",
    "route_canary_log_index",
    "route_canary_receipt_block_number",
    "route_canary_receipt_block_hash",
    "route_canary_block_receipts_root",
    "route_canary_call_data_sha256",
    "route_canary_message_id",
    "route_canary_payload_hash",
    "route_canary_target_domain",
    "route_canary_statement_hash",
    "route_canary_commitment_root",
    "route_canary_finality_height",
    "route_canary_finality_block_hash",
    "route_canary_proof_version",
    "route_canary_proof_source_domain",
    "route_canary_used_message_proof",
    "route_canary_receipt_block_finalized",
)


_ROUTE_CANARY_TRANSCRIPT_HASH_FIELDS = (
    "transaction_hash",
    "receipt_block_hash",
    "block_receipts_root",
    "call_data_sha256",
    "message_id",
    "payload_hash",
    "statement_hash",
    "commitment_root",
    "finality_block_hash",
)


def _route_canary_transaction_toml_lines(args: argparse.Namespace) -> list[str]:
    values = _route_canary_transaction_values(args)
    if values is None:
        return []
    return [
        _toml_line(
            "evm_route_canary_transaction_hash",
            _hex(values["transaction_hash"]),
        ),
        _toml_line(
            "evm_route_canary_transaction_block_number",
            values["transaction_block_number"],
        ),
        _toml_line(
            "evm_route_canary_transaction_block_hash",
            _hex(values["transaction_block_hash"]),
        ),
        _toml_line("evm_route_canary_log_index", values["log_index"]),
        _toml_line(
            "evm_route_canary_receipt_block_number",
            values["receipt_block_number"],
        ),
        _toml_line(
            "evm_route_canary_receipt_block_hash",
            _hex(values["receipt_block_hash"]),
        ),
        _toml_line(
            "evm_route_canary_block_receipts_root",
            _hex(values["block_receipts_root"]),
        ),
        _toml_line(
            "evm_route_canary_call_data_sha256",
            _hex(values["call_data_sha256"]),
        ),
        _toml_line("evm_route_canary_message_id", _hex(values["message_id"])),
        _toml_line("evm_route_canary_payload_hash", _hex(values["payload_hash"])),
        _toml_line("evm_route_canary_target_domain", values["target_domain"]),
        _toml_line(
            "evm_route_canary_statement_hash",
            _hex(values["statement_hash"]),
        ),
        _toml_line(
            "evm_route_canary_commitment_root",
            _hex(values["commitment_root"]),
        ),
        _toml_line(
            "evm_route_canary_finality_height",
            _hex(values["finality_height"]),
        ),
        _toml_line(
            "evm_route_canary_finality_block_hash",
            _hex(values["finality_block_hash"]),
        ),
        _toml_line("evm_route_canary_proof_version", values["proof_version"]),
        _toml_line(
            "evm_route_canary_proof_source_domain",
            values["proof_source_domain"],
        ),
        _toml_line(
            "evm_route_canary_used_message_proof",
            values["used_message_proof"],
        ),
        _toml_line(
            "evm_route_canary_receipt_block_finalized",
            values["receipt_block_finalized"],
        ),
    ]


def _route_canary_transaction_supplied(args: argparse.Namespace) -> bool:
    return any(
        getattr(args, name, None) is not None for name in _ROUTE_CANARY_TRANSACTION_FIELDS
    )


def _route_canary_transaction_values(args: argparse.Namespace) -> dict[str, object] | None:
    if not _route_canary_transaction_supplied(args):
        return None
    missing = [
        name
        for name in _ROUTE_CANARY_TRANSACTION_FIELDS
        if getattr(args, name, None) is None
    ]
    if missing:
        formatted = ", ".join(f"--{name.replace('_', '-')}" for name in missing)
        raise ValueError("EVM route canary transaction metadata requires " + formatted)
    log_index = _require_exact_u32(
        getattr(args, "route_canary_log_index"),
        "route_canary_log_index",
    )
    target_domain = _require_exact_u32(
        getattr(args, "route_canary_target_domain"),
        "route_canary_target_domain",
    )
    if target_domain != args.domain:
        raise ValueError(
            "EVM route canary target domain metadata must match the destination domain"
        )
    proof_version = _require_exact_u32(
        getattr(args, "route_canary_proof_version"),
        "route_canary_proof_version",
    )
    if proof_version != 1:
        raise ValueError("EVM route canary proof version metadata must be 1")
    proof_source_domain = _require_exact_u32(
        getattr(args, "route_canary_proof_source_domain"),
        "route_canary_proof_source_domain",
    )
    if proof_source_domain != SCCP_DOMAIN_SORA:
        raise ValueError(
            "EVM route canary proof source domain metadata must be SORA"
        )
    if getattr(args, "route_canary_used_message_proof") is not True:
        raise ValueError(
            "EVM route canary transaction metadata requires "
            "--route-canary-used-message-proof=true from live bridge state"
        )
    if getattr(args, "route_canary_receipt_block_finalized") is not True:
        raise ValueError(
            "EVM route canary transaction metadata requires "
            "--route-canary-receipt-block-finalized=true from finalized live reads"
        )
    receipt_block_number = parse_u64_decimal(
        str(getattr(args, "route_canary_receipt_block_number")),
        label="route_canary_receipt_block_number",
    )
    if receipt_block_number == 0:
        raise ValueError("EVM route canary receipt block number must be positive")
    transaction_block_number = parse_u64_decimal(
        str(getattr(args, "route_canary_transaction_block_number")),
        label="route_canary_transaction_block_number",
    )
    if transaction_block_number == 0:
        raise ValueError("EVM route canary transaction block number must be positive")
    transaction_block_hash = _require_fixed_bytes(
        getattr(args, "route_canary_transaction_block_hash"),
        label="route_canary_transaction_block_hash",
        byte_length=32,
    )
    receipt_block_hash = _require_fixed_bytes(
        getattr(args, "route_canary_receipt_block_hash"),
        label="route_canary_receipt_block_hash",
        byte_length=32,
    )
    if transaction_block_number != receipt_block_number:
        raise ValueError(
            "EVM route canary transaction block number must match receipt block number"
        )
    if transaction_block_hash != receipt_block_hash:
        raise ValueError(
            "EVM route canary transaction block hash must match receipt block hash"
        )
    values = {
        "transaction_hash": _require_fixed_bytes(
            getattr(args, "route_canary_transaction_hash"),
            label="route_canary_transaction_hash",
            byte_length=32,
        ),
        "transaction_block_number": transaction_block_number,
        "transaction_block_hash": transaction_block_hash,
        "log_index": log_index,
        "receipt_block_number": receipt_block_number,
        "receipt_block_hash": receipt_block_hash,
        "block_receipts_root": _require_fixed_bytes(
            getattr(args, "route_canary_block_receipts_root"),
            label="route_canary_block_receipts_root",
            byte_length=32,
        ),
        "call_data_sha256": _require_fixed_bytes(
            getattr(args, "route_canary_call_data_sha256"),
            label="route_canary_call_data_sha256",
            byte_length=32,
        ),
        "message_id": _require_fixed_bytes(
            getattr(args, "route_canary_message_id"),
            label="route_canary_message_id",
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
        "receipt_block_finalized": getattr(
            args,
            "route_canary_receipt_block_finalized",
        ),
    }
    _require_distinct_hash_roles(
        tuple(
            (field, values[field])
            for field in _ROUTE_CANARY_TRANSCRIPT_HASH_FIELDS
            if isinstance(values[field], bytes)
        ),
        label="EVM route canary transcript hashes",
    )
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
    return evm_route_canary_transaction_evidence_hash(
        route_allowlist_hash=route_allowlist_hash,
        bridge_address=args.bridge_address,
        transaction_hash=values["transaction_hash"],
        log_index=values["log_index"],
        receipt_block_number=values["receipt_block_number"],
        receipt_block_hash=values["receipt_block_hash"],
        block_receipts_root=values["block_receipts_root"],
        call_data_sha256=values["call_data_sha256"],
        message_id=values["message_id"],
        payload_hash=values["payload_hash"],
        source_domain=SCCP_DOMAIN_SORA,
        target_domain=values["target_domain"],
        commitment_root=values["commitment_root"],
        finality_height=values["finality_height"],
        finality_block_hash=values["finality_block_hash"],
        statement_hash=values["statement_hash"],
        proof_version=values["proof_version"],
        proof_source_domain=values["proof_source_domain"],
        destination_binding_hash=destination_binding_hash,
        verifier_backend_hash=evm_verifier_backend_hash(),
        proof_family_hash=evm_proof_family_hash(),
        network_id=args.network_id,
        used_message_proof=values["used_message_proof"],
        receipt_block_finalized=values["receipt_block_finalized"],
        bsc_network=getattr(args, "bsc_network", None),
    )


def _route_canary_transaction_comment_lines(args: argparse.Namespace) -> list[str]:
    values = _route_canary_transaction_values(args)
    if values is None:
        return []
    return [
        "# sccp_evm_route_canary_transaction_hash = "
        + json.dumps(_hex(values["transaction_hash"])),
        "# sccp_evm_route_canary_transaction_block_number = "
        + json.dumps(str(values["transaction_block_number"])),
        "# sccp_evm_route_canary_transaction_block_hash = "
        + json.dumps(_hex(values["transaction_block_hash"])),
        "# sccp_evm_route_canary_log_index = "
        + json.dumps(str(values["log_index"])),
        "# sccp_evm_route_canary_receipt_block_number = "
        + json.dumps(str(values["receipt_block_number"])),
        "# sccp_evm_route_canary_receipt_block_hash = "
        + json.dumps(_hex(values["receipt_block_hash"])),
        "# sccp_evm_route_canary_block_receipts_root = "
        + json.dumps(_hex(values["block_receipts_root"])),
        "# sccp_evm_route_canary_call_data_sha256 = "
        + json.dumps(_hex(values["call_data_sha256"])),
        "# sccp_evm_route_canary_message_id = "
        + json.dumps(_hex(values["message_id"])),
        "# sccp_evm_route_canary_payload_hash = "
        + json.dumps(_hex(values["payload_hash"])),
        "# sccp_evm_route_canary_target_domain = "
        + json.dumps(str(values["target_domain"])),
        "# sccp_evm_route_canary_statement_hash = "
        + json.dumps(_hex(values["statement_hash"])),
        "# sccp_evm_route_canary_commitment_root = "
        + json.dumps(_hex(values["commitment_root"])),
        "# sccp_evm_route_canary_finality_height = "
        + json.dumps(_hex(values["finality_height"])),
        "# sccp_evm_route_canary_finality_block_hash = "
        + json.dumps(_hex(values["finality_block_hash"])),
        "# sccp_evm_route_canary_proof_version = "
        + json.dumps(str(values["proof_version"])),
        "# sccp_evm_route_canary_proof_source_domain = "
        + json.dumps(str(values["proof_source_domain"])),
        "# sccp_evm_route_canary_used_message_proof = "
        + json.dumps("true" if values["used_message_proof"] is True else "false"),
        "# sccp_evm_route_canary_receipt_block_finalized = "
        + json.dumps("true" if values["receipt_block_finalized"] is True else "false"),
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
        *_route_canary_transaction_comment_lines(args),
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
                "evidence_source": "evm_message_proof_accepted_transaction",
                "transaction_hash": _hex(values["transaction_hash"]),
                "log_index": values["log_index"],
                "receipt_block_number": values["receipt_block_number"],
                "receipt_block_hash": _hex(values["receipt_block_hash"]),
                "block_receipts_root": _hex(values["block_receipts_root"]),
                "call_data_sha256": _hex(values["call_data_sha256"]),
                "message_id": _hex(values["message_id"]),
                "payload_hash": _hex(values["payload_hash"]),
                "target_domain": values["target_domain"],
                "statement_hash": _hex(values["statement_hash"]),
                "commitment_root": _hex(values["commitment_root"]),
                "finality_height": _hex(values["finality_height"]),
                "finality_block_hash": _hex(values["finality_block_hash"]),
                "proof_version": values["proof_version"],
                "proof_source_domain": values["proof_source_domain"],
                "message_proof_used": values["used_message_proof"],
                "receipt_block_finalized": values["receipt_block_finalized"],
            }
        )
    return summary


def _route_canary_evidence_hash(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes | None:
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
        if derived_canary_hash is None:
            raise ValueError(
                "route_canary_evidence_hash requires EVM route canary "
                "transaction metadata"
            )
        if canary_hash != derived_canary_hash:
            raise ValueError(
                "route_canary_evidence_hash does not match EVM route canary "
                "transaction metadata"
            )
    if canary_hash is None:
        return None
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
    return canary_hash


def _destination_binding_hash_from_args(args: argparse.Namespace) -> bytes:
    return evm_destination_binding_hash(
        network_id=args.network_id,
        source_domain=SCCP_DOMAIN_SORA,
        target_domain=args.domain,
        verifier_address=args.verifier_address,
        bridge_address=args.bridge_address,
        verifier_code_hash=args.verifier_code_hash,
        verifier_key_hash=args.verifier_key_hash,
        bsc_network=getattr(args, "bsc_network", None),
    )


def _destination_binding_key_from_args(args: argparse.Namespace) -> str:
    return evm_destination_binding_key(
        network_id=args.network_id,
        source_domain=SCCP_DOMAIN_SORA,
        target_domain=args.domain,
        verifier_address=args.verifier_address,
        bridge_address=args.bridge_address,
        verifier_code_hash=args.verifier_code_hash,
        verifier_key_hash=args.verifier_key_hash,
        bsc_network=getattr(args, "bsc_network", None),
    )


def apply_canonical_network_id(args: argparse.Namespace) -> None:
    """Default or validate the EVM network id for the selected domain profile."""

    if getattr(args, "network_id", None) is None:
        args.network_id = evm_network_id_for_domain(
            args.domain,
            bsc_network=getattr(args, "bsc_network", None),
        )
        return
    args.network_id = _require_domain_network_id(
        args.domain,
        args.network_id,
        bsc_network=getattr(args, "bsc_network", None),
    )


def validate_bsc_network_scope(args: argparse.Namespace) -> None:
    """Reject BSC-specific profile selection for non-BSC domains."""

    bsc_network = getattr(args, "bsc_network", "mainnet")
    if args.domain != SCCP_DOMAIN_BSC and bsc_network != "mainnet":
        raise ValueError("--bsc-network only applies when --domain bsc")


def _route_allowlist_hash_from_args(
    args: argparse.Namespace,
    destination_binding_hash: bytes,
) -> bytes:
    return evm_route_allowlist_hash(
        domain=args.domain,
        source_verifier_material_hash=getattr(
            args,
            "source_verifier_material_hash",
            None,
        ),
        source_adapter_engine_deployment_hash=(
            getattr(args, "source_adapter_engine_deployment_hash", None)
        ),
        destination_binding_hash=destination_binding_hash,
        bsc_network=getattr(args, "bsc_network", None),
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


def render_toml(args: argparse.Namespace, destination_binding_hash: bytes) -> str:
    """Render production destination rollout and route allowlist TOML."""

    apply_runtime_bytecode_hash(args)
    apply_bridge_runtime_bytecode_hash(args)
    block_tag = _block_tag_from_args(args)
    if args.domain == SCCP_DOMAIN_ETH and block_tag != "finalized":
        raise ValueError("Ethereum destination TOML requires --block-tag finalized")
    expected_hash = _destination_binding_hash_from_args(args)
    expected_pin = getattr(args, "expected_destination_binding_hash", None)
    if expected_pin is None:
        raise ValueError(
            "--expected-destination-binding-hash is required before rendering production TOML"
        )
    if expected_pin != expected_hash:
        raise ValueError(
            "expected destination binding hash does not match deployment inputs: "
            f"expected {_hex(expected_pin)}, got {_hex(expected_hash)}"
        )
    if destination_binding_hash != expected_hash:
        raise ValueError(
            "destination_binding_hash must match the canonical "
            f"SORA -> {_profile(args)['chain'].upper()} binding: "
            f"expected {_hex(expected_hash)}, got {_hex(destination_binding_hash)}"
        )
    missing_route_args = _missing_route_allowlist_args(args)
    if missing_route_args:
        formatted = ", ".join(f"--{name.replace('_', '-')}" for name in missing_route_args)
        raise ValueError(f"--toml requires {formatted}")
    _require_runtime_bytecode_evidence(args, output="toml")
    if getattr(args, "bridge_code_hash", None) is None:
        raise ValueError("--toml requires --bridge-code-hash")
    route_allowlist_hash = _require_expected_route_allowlist_hash(
        args,
        destination_binding_hash,
    )
    route_canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if route_canary_hash is None:
        raise ValueError(
            "--toml requires EVM route canary transaction metadata "
            "(--route-canary-transaction-hash, --route-canary-log-index, "
            "--route-canary-receipt-block-number, "
            "--route-canary-receipt-block-hash, "
            "--route-canary-block-receipts-root, "
            "--route-canary-call-data-sha256, --route-canary-message-id, "
            "--route-canary-payload-hash, --route-canary-target-domain, "
            "--route-canary-statement-hash, --route-canary-commitment-root, "
            "--route-canary-finality-height, "
            "--route-canary-finality-block-hash, "
            "--route-canary-proof-version, "
            "--route-canary-proof-source-domain, and "
            "--route-canary-used-message-proof=true)"
        )
    sections = [
        "# sccp_evm_rpc_chain_id = "
        + json.dumps(str(_profile(args)["rpc_chain_id"])),
        "# sccp_evm_block_tag = " + json.dumps(block_tag),
        "# sccp_evm_bridge_runtime_code_hash = "
        + json.dumps(_hex(args.bridge_code_hash)),
    ]
    bridge_bytecode = getattr(args, "bridge_runtime_bytecode_hex_text", None)
    if isinstance(bridge_bytecode, str) and bridge_bytecode.strip():
        sections.append(
            "# sccp_evm_bridge_runtime_bytecode_hex = " + json.dumps(bridge_bytecode)
        )
    sections.append(
        "# sccp_evm_verifier_runtime_code_hash = "
        + json.dumps(_hex(args.verifier_code_hash))
    )
    verifier_bytecode = getattr(args, "verifier_runtime_bytecode_hex_text", None)
    if isinstance(verifier_bytecode, str) and verifier_bytecode.strip():
        sections.append(
            "# sccp_evm_verifier_runtime_bytecode_hex = "
            + json.dumps(verifier_bytecode)
        )
    sections.extend(
        [
            "# sccp_evm_verifier_key_hash = "
            + json.dumps(_hex(args.verifier_key_hash)),
            "# sccp_evm_verifier_backend_hash = "
            + json.dumps(_hex(evm_verifier_backend_hash())),
            "# sccp_evm_proof_family_hash = "
            + json.dumps(_hex(evm_proof_family_hash())),
            "# sccp_evm_destination_network_id = "
            + json.dumps(_hex(args.network_id)),
            "# sccp_evm_destination_bridge_address = "
            + json.dumps(_hex(args.bridge_address)),
            "# sccp_evm_destination_binding_key = "
            + json.dumps(_destination_binding_key_from_args(args)),
            "# sccp_evm_destination_binding_hash = "
            + json.dumps(_hex(destination_binding_hash)),
            "# sccp_evm_route_allowlist_hash = "
            + json.dumps(_hex(route_allowlist_hash)),
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
        ]
    )
    return "\n".join(sections) + "\n"


def _json_summary(
    args: argparse.Namespace,
    destination_binding_hash: bytes,
    expected_matches: bool,
) -> dict[str, object]:
    apply_runtime_bytecode_hash(args)
    apply_bridge_runtime_bytecode_hash(args)
    profile = _profile(args)
    expected_hash = _destination_binding_hash_from_args(args)
    if destination_binding_hash != expected_hash:
        raise ValueError(
            "destination_binding_hash must match the canonical "
            f"SORA -> {profile['chain'].upper()} binding: "
            f"expected {_hex(expected_hash)}, got {_hex(destination_binding_hash)}"
        )
    expected_pin = getattr(args, "expected_destination_binding_hash", None)
    if expected_pin is not None and expected_pin != expected_hash:
        raise ValueError(
            "expected destination binding hash does not match deployment inputs: "
            f"expected {_hex(expected_pin)}, got {_hex(expected_hash)}"
        )
    route_requested = any(
        getattr(args, name, None) is not None
        for name in (
            "route_allowlist_hash",
            "source_verifier_material_hash",
            "source_adapter_engine_deployment_hash",
            "route_canary_evidence_hash",
            *_ROUTE_CANARY_TRANSACTION_FIELDS,
        )
    )
    summary = {
        "source_domain": SCCP_DOMAIN_SORA,
        "target_domain": args.domain,
        "chain": profile["chain"],
        "block_tag": _block_tag_from_args(args),
        "verifier_backend": SCCP_EVM_GROTH16_BACKEND,
        "proof_family": SCCP_PROOF_FAMILY_STARK_FRI,
        "network_id": _hex(args.network_id),
        "verifier_address": _hex(args.verifier_address),
        "bridge_address": _hex(args.bridge_address),
        "verifier_code_hash": _hex(args.verifier_code_hash),
        "verifier_key_hash": _hex(args.verifier_key_hash),
        "verifier_backend_hash": _hex(evm_verifier_backend_hash()),
        "proof_family_hash": _hex(evm_proof_family_hash()),
        "destination_binding_key": _destination_binding_key_from_args(args),
        "destination_binding_hash": _hex(destination_binding_hash),
        "expected_destination_binding_hash_matches": expected_matches,
        "toml_ready": False,
    }
    if getattr(args, "bridge_code_hash", None) is not None:
        summary["bridge_code_hash"] = _hex(args.bridge_code_hash)
    bridge_bytecode = getattr(args, "bridge_runtime_bytecode_hex_text", None)
    if isinstance(bridge_bytecode, str) and bridge_bytecode.strip():
        summary["bridge_runtime_bytecode_hex"] = bridge_bytecode
    verifier_bytecode = getattr(args, "verifier_runtime_bytecode_hex_text", None)
    if isinstance(verifier_bytecode, str) and verifier_bytecode.strip():
        summary["verifier_runtime_bytecode_hex"] = verifier_bytecode
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
        summary.update(
            {
                "source_verifier_material_hash": _hex(
                    args.source_verifier_material_hash
                ),
                "source_adapter_engine_deployment_hash": _hex(
                    args.source_adapter_engine_deployment_hash
                ),
                "route_allowlist_hash": _hex(route_allowlist_hash),
                "expected_route_allowlist_hash": _hex(expected_route_allowlist_hash),
                "expected_route_allowlist_hash_matches": True,
                "toml_ready": expected_matches
                and _runtime_bytecode_evidence_ready(args),
            }
        )
        route_canary = _route_canary_summary(
            args,
            route_allowlist_hash=route_allowlist_hash,
            destination_binding_hash=destination_binding_hash,
        )
        if route_canary is not None:
            summary["route_canary"] = route_canary
            summary["toml_ready"] = bool(summary["toml_ready"])
        else:
            summary["toml_ready"] = False
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Render SCCP ETH/BSC destination rollout evidence.",
    )
    parser.add_argument(
        "--domain",
        required=True,
        type=parse_destination_domain,
        help="Destination domain to render: eth or bsc.",
    )
    parser.add_argument(
        "--bsc-network",
        default="mainnet",
        type=parse_bsc_network,
        help=(
            "BSC network profile when --domain bsc: mainnet or testnet. "
            "Defaults to mainnet."
        ),
    )
    parser.add_argument(
        "--network-id",
        type=lambda value: parse_hex_bytes(value, label="network id", byte_length=32),
        help=(
            "Optional EVM chain/network id override as a non-zero bytes32 hex "
            "value. Defaults to the selected domain/profile's canonical EIP-155 "
            "chain id and rejects any mismatch."
        ),
    )
    parser.add_argument(
        "--verifier-address",
        required=True,
        type=lambda value: parse_evm_address(value, label="verifier address"),
        help="Deployed immutable SCCP verifier contract address.",
    )
    parser.add_argument(
        "--bridge-address",
        required=True,
        type=lambda value: parse_evm_address(value, label="bridge address"),
        help="Deployed SCCP message bridge wrapper address.",
    )
    parser.add_argument(
        "--block-tag",
        choices=EVM_BLOCK_TAGS,
        help=(
            "EVM block tag represented by the audited rollout evidence. "
            "Defaults to finalized for Ethereum and latest for BSC; production "
            "Ethereum TOML requires finalized."
        ),
    )
    parser.add_argument(
        "--bridge-code-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="bridge code hash",
            byte_length=32,
        ),
        help=(
            "Non-zero deployed bridge wrapper runtime bytecode hash. Required "
            "for production TOML metadata."
        ),
    )
    parser.add_argument(
        "--bridge-runtime-bytecode-hex",
        type=lambda value: parse_runtime_bytecode_hex(
            value,
            label="bridge runtime bytecode",
        ),
        help=(
            "Hex-encoded deployed bridge wrapper runtime bytecode. When "
            "supplied, the helper derives bridge_code_hash."
        ),
    )
    parser.add_argument(
        "--bridge-runtime-bytecode-file",
        type=lambda value: parse_runtime_bytecode_file(
            value,
            label="bridge runtime bytecode",
        ),
        help=(
            "File containing deployed bridge wrapper runtime bytecode hex. "
            "When supplied, the helper derives bridge_code_hash."
        ),
    )
    parser.add_argument(
        "--verifier-code-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="verifier code hash",
            byte_length=32,
        ),
        help="Non-zero deployed verifier runtime bytecode hash.",
    )
    parser.add_argument(
        "--verifier-runtime-bytecode-hex",
        type=lambda value: parse_runtime_bytecode_hex(
            value,
            label="verifier runtime bytecode",
        ),
        help=(
            "Hex-encoded deployed verifier runtime bytecode. When supplied, "
            "the helper derives verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--verifier-runtime-bytecode-file",
        type=lambda value: parse_runtime_bytecode_file(
            value,
            label="verifier runtime bytecode",
        ),
        help=(
            "File containing deployed verifier runtime bytecode hex. When supplied, "
            "the helper derives verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--verifier-key-hash",
        required=True,
        type=lambda value: parse_hex_bytes(
            value,
            label="verifier key hash",
            byte_length=32,
        ),
        help="Non-zero Groth16 verifier key hash.",
    )
    parser.add_argument(
        "--route-allowlist-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route allowlist hash",
            byte_length=32,
        ),
        help=(
            "Governed route allowlist hash. Must match the canonical source "
            "material, source adapter deployment, and destination binding tuple "
            "after --expected-destination-binding-hash pins the binding."
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
            "Non-zero post-deploy route canary evidence hash to emit as "
            "all-lanes preflight metadata."
        ),
    )
    parser.add_argument(
        "--route-canary-transaction-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary transaction hash",
            byte_length=32,
        ),
        help="EVM MessageProofAccepted canary transaction hash.",
    )
    parser.add_argument(
        "--route-canary-log-index",
        type=lambda value: parse_u32_decimal(
            value,
            label="route canary log index",
        ),
        help="Canonical decimal log index of the MessageProofAccepted canary event.",
    )
    parser.add_argument(
        "--route-canary-transaction-block-number",
        type=lambda value: parse_u64_decimal(
            value,
            label="route canary transaction block number",
        ),
        help=(
            "Canonical decimal block number returned by eth_getTransactionByHash "
            "for the canary transaction. Must match the receipt block number."
        ),
    )
    parser.add_argument(
        "--route-canary-transaction-block-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary transaction block hash",
            byte_length=32,
        ),
        help=(
            "Block hash returned by eth_getTransactionByHash for the canary "
            "transaction. Must match the receipt block hash."
        ),
    )
    parser.add_argument(
        "--route-canary-receipt-block-number",
        type=lambda value: parse_u64_decimal(
            value,
            label="route canary receipt block number",
        ),
        help="Canonical decimal EVM block number containing the canary receipt.",
    )
    parser.add_argument(
        "--route-canary-receipt-block-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary receipt block hash",
            byte_length=32,
        ),
        help="EVM block hash returned by the canary transaction receipt.",
    )
    parser.add_argument(
        "--route-canary-block-receipts-root",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary block receiptsRoot",
            byte_length=32,
        ),
        help="EVM receiptsRoot from the block containing the canary receipt.",
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
        help="SHA-256 of the submitted submitSccpMessageProof calldata.",
    )
    parser.add_argument(
        "--route-canary-payload-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary payload hash",
            byte_length=32,
        ),
        help="submitSccpMessageProof publicInputs[1] payload hash.",
    )
    parser.add_argument(
        "--route-canary-target-domain",
        type=lambda value: parse_u32_decimal(
            value,
            label="route canary target domain",
        ),
        help="submitSccpMessageProof publicInputs[2] target domain.",
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
        help="submitSccpMessageProof publicInputs[4] finality height word.",
    )
    parser.add_argument(
        "--route-canary-finality-block-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary finality block hash",
            byte_length=32,
        ),
        help="submitSccpMessageProof publicInputs[5] finality block hash.",
    )
    parser.add_argument(
        "--route-canary-proof-version",
        type=lambda value: parse_u32_decimal(
            value,
            label="route canary proof version",
        ),
        help="Groth16 proof tuple version decoded from proofWords[0].",
    )
    parser.add_argument(
        "--route-canary-proof-source-domain",
        type=lambda value: parse_u32_decimal(
            value,
            label="route canary proof source domain",
        ),
        help="Groth16 proof tuple source domain decoded from proofWords[2].",
    )
    parser.add_argument(
        "--route-canary-used-message-proof",
        type=lambda value: parse_bool_literal(
            value,
            label="route canary used message proof",
        ),
        help=(
            "Assert live bridge state returned usedMessageProofs(messageId) = true "
            "for the canary transaction."
        ),
    )
    parser.add_argument(
        "--route-canary-receipt-block-finalized",
        type=lambda value: parse_bool_literal(
            value,
            label="route canary receipt block finalized",
        ),
        help=(
            "Assert live finalized execution head proved the canary receipt block "
            "was finalized."
        ),
    )
    parser.add_argument(
        "--expected-destination-binding-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected destination binding hash",
            byte_length=32,
        ),
        help="Expected bridge destination binding hash to compare against inputs.",
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
        validate_bsc_network_scope(args)
        apply_canonical_network_id(args)
        apply_runtime_bytecode_hash(args)
        apply_bridge_runtime_bytecode_hash(args)
        destination_binding_hash = _destination_binding_hash_from_args(args)
        expected_matches = False
        if args.expected_destination_binding_hash is not None:
            if args.expected_destination_binding_hash != destination_binding_hash:
                raise ValueError(
                    "expected destination binding hash does not match deployment inputs: "
                    f"expected {_hex(args.expected_destination_binding_hash)}, "
                    f"got {_hex(destination_binding_hash)}"
                )
            expected_matches = True
        if args.toml:
            sys.stdout.write(render_toml(args, destination_binding_hash))
        else:
            print(
                json.dumps(
                    _json_summary(args, destination_binding_hash, expected_matches),
                    indent=2,
                    sort_keys=True,
                )
            )
    except (OSError, RuntimeError, TypeError, ValueError) as exc:
        detail = _cli_error_detail(
            exc,
            fallback="SCCP EVM destination evidence rendering failed",
        )
        parser.exit(2, f"{parser.prog}: error: {detail}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
