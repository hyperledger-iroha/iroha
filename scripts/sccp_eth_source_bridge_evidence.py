#!/usr/bin/env python3
"""Render SCCP Ethereum source bridge deployment evidence.

This helper is offline by design. Operators pass the governed Ethereum source
bridge address, deployment component hashes, adapter verifier key hash, and
deployment receipt hash collected from governance or deployment records. The
script validates that all production evidence hashes are non-zero and can
render the matching `zk.sccp_source_verifier_materials` and
`zk.sccp_source_adapter_engine_deployments` TOML records for the ETH -> SORA
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
PYTHON_CLIENT = REPO_ROOT / "python"
if str(PYTHON_CLIENT) not in sys.path:
    sys.path.insert(0, str(PYTHON_CLIENT))

from iroha_torii_client.sccp import _keccak_256  # noqa: E402


SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_ETH = 1
ETH_RPC_CHAIN_ID = 1
SCCP_PROOF_FAMILY_STARK_FRI = "stark-fri-v1"
SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID = "sccp-source-adapter-v1"
SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET = "fastpq-lane-balanced"
SCCP_EVM_GROTH16_BN254_PROOF_BACKEND = "evm-groth16-bn254-v1"
ETH_SOURCE_PROOF_PLAN_CODE = 1
ETH_FINALITY_MODEL_CODE = 1
FASTPQ_BALANCED_TRACE_ROOT = 0x002A_247F_81C6_F850
FASTPQ_BALANCED_LDE_ROOT = 0x6026_3388_DBBF_9B2A
FASTPQ_BALANCED_OMEGA_COSET = 0x6AF3_25E8_25AD_5C18

ETH_SOURCE_TRUST_ANCHOR_ID = (
    "sccp:eth:source-trust-anchor:ethereum-mainnet-beacon-finalized-checkpoint:v1"
)
ETH_CONSENSUS_VERIFIER_ID = (
    "sccp:eth:consensus-verifier:beacon-sync-committee-execution-header-mainnet:v1"
)
ETH_MESSAGE_INCLUSION_VERIFIER_ID = (
    "sccp:eth:message-inclusion-verifier:execution-receipt-trie-branch-mainnet:v1"
)
ETH_SOURCE_BRIDGE_EMITTER_ID = "sccp:eth:source-bridge-emitter:ethereum-mainnet:v1"
ETH_FINALITY_POLICY_ID = (
    "sccp:eth:finality-policy:beacon-finalized-checkpoint-mainnet:v1"
)
ETH_TEMPLATE_COMPONENTS = {
    "source_trust_anchor_hash": (
        ETH_SOURCE_TRUST_ANCHOR_ID,
        "source-trust-anchor",
    ),
    "consensus_verifier_hash": (
        ETH_CONSENSUS_VERIFIER_ID,
        "consensus-verifier",
    ),
    "message_inclusion_verifier_hash": (
        ETH_MESSAGE_INCLUSION_VERIFIER_ID,
        "message-inclusion-verifier",
    ),
    "finality_policy_hash": (
        ETH_FINALITY_POLICY_ID,
        "finality-policy",
    ),
}
ETH_TEMPLATE_TRANSCRIPT_PREFIXES = (
    b"sccp:evm:receipt-proof:v1",
    b"sccp:eth:sync-committee:v1",
    b"sccp:eth:sync-committee-payload:v1",
    b"sccp:eth:sync-committee-message:v1",
    b"sccp:eth:sync-committee-aggregate:v1",
    b"sccp:eth:sync-committee-transition-message:v1",
    b"sccp:eth:sync-committee-transition-signature:v1",
    b"sccp:eth:ssz-execution-payload-header:deneb-fulu:v1",
    b"sccp:eth:ssz-beacon-block-header:v1",
    b"sccp:eth:ssz-execution-payload-branch:deneb-fulu:v1",
)


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


def parse_evm_address(value: str, *, label: str) -> bytes:
    """Parse a non-zero EVM address."""

    return parse_hex_bytes(value, label=label, byte_length=20)


def parse_runtime_bytecode_hex(value: str, *, label: str) -> bytes:
    """Parse non-empty runtime bytecode from hex text."""

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
    return raw


def parse_runtime_bytecode_file(value: str, *, label: str) -> bytes:
    """Parse runtime bytecode from a file containing hex text."""

    path = Path(value).expanduser()
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise argparse.ArgumentTypeError(f"{label} file cannot be read") from exc
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


def _evm_family_template_component_hash(component_id: str, component_kind: str) -> bytes:
    payload = bytearray()
    payload.append(1)
    payload.extend(SCCP_DOMAIN_ETH.to_bytes(4, "little"))
    _push_vec(payload, b"eth")
    payload.append(1)  # EthereumBeaconReceiptProof
    payload.append(1)  # EthereumBeaconExecution
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, SCCP_EVM_GROTH16_BN254_PROOF_BACKEND.encode("utf-8"))
    for prefix in ETH_TEMPLATE_TRANSCRIPT_PREFIXES:
        _push_vec(payload, prefix)
    _push_vec(payload, component_kind.encode("utf-8"))
    _push_vec(payload, component_id.encode("utf-8"))
    return _prefixed_blake2b(b"sccp:evm-family:source-verifier-material:v1", payload)


def eth_source_adapter_verifier_vk_hash(
    *,
    source_domain: int = SCCP_DOMAIN_ETH,
    target_domain: int = SCCP_DOMAIN_SORA,
) -> bytes:
    """Compute Rust's canonical OpenVerify vk hash for ETH -> SORA."""

    source_domain = _require_exact_u32(source_domain, "source_domain")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_ETH:
        raise ValueError("source_domain must be ETH")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")

    verifier = bytearray()
    _push_u8(verifier, 1)
    _push_vec(verifier, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(verifier, b"eth")
    _push_u32(verifier, source_domain)
    _push_u32(verifier, target_domain)
    _push_u8(verifier, ETH_SOURCE_PROOF_PLAN_CODE)
    _push_u8(verifier, ETH_FINALITY_MODEL_CODE)
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


def eth_source_verifier_material_record_hash(args: argparse.Namespace) -> bytes:
    """Compute Rust's canonical ETH source verifier material record hash."""

    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    if source_domain != SCCP_DOMAIN_ETH:
        raise ValueError("source_domain must be ETH")
    _require_live_component_hashes(args)
    _require_source_role_hash_separation(args)
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, source_domain)
    _push_vec(payload, b"eth")
    _push_u8(payload, ETH_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, ETH_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, ETH_SOURCE_TRUST_ANCHOR_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_trust_anchor_hash,
            label="source_trust_anchor_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, ETH_CONSENSUS_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.consensus_verifier_hash,
            label="consensus_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, ETH_MESSAGE_INCLUSION_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.message_inclusion_verifier_hash,
            label="message_inclusion_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, ETH_FINALITY_POLICY_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.finality_policy_hash,
            label="finality_policy_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, b"")
    payload.extend(bytes(32))
    _push_vec(payload, ETH_SOURCE_BRIDGE_EMITTER_ID.encode("utf-8"))
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


def eth_source_adapter_engine_deployment_record_hash(
    args: argparse.Namespace,
) -> bytes:
    """Compute Rust's canonical ETH source-adapter deployment record hash."""

    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_ETH:
        raise ValueError("source_domain must be ETH")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")
    _require_live_component_hashes(args)
    _require_source_role_hash_separation(args)
    adapter_verifier_vk_hash = _require_nonzero_fixed_bytes(
        args.adapter_verifier_vk_hash,
        label="adapter_verifier_vk_hash",
        byte_length=32,
    )
    expected_adapter_verifier_vk_hash = eth_source_adapter_verifier_vk_hash(
        source_domain=source_domain,
        target_domain=target_domain,
    )
    if adapter_verifier_vk_hash != expected_adapter_verifier_vk_hash:
        raise ValueError(
            "adapter_verifier_vk_hash must match the canonical "
            "ETH source-adapter verifier profile"
        )

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, source_domain)
    _push_u32(payload, target_domain)
    _push_vec(payload, b"eth")
    _push_u8(payload, ETH_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, ETH_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8"))
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    payload.extend(adapter_verifier_vk_hash)
    _push_vec(payload, ETH_SOURCE_TRUST_ANCHOR_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_trust_anchor_hash,
            label="source_trust_anchor_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, ETH_CONSENSUS_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.consensus_verifier_hash,
            label="consensus_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, ETH_MESSAGE_INCLUSION_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.message_inclusion_verifier_hash,
            label="message_inclusion_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, ETH_FINALITY_POLICY_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.finality_policy_hash,
            label="finality_policy_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, b"")
    payload.extend(bytes(32))
    _push_vec(payload, ETH_SOURCE_BRIDGE_EMITTER_ID.encode("utf-8"))
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


def _require_eth_sora_lane(args: argparse.Namespace) -> None:
    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_ETH:
        raise ValueError("ETH production source evidence requires source_domain = 1")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("ETH production source evidence requires target_domain = 0")


def _require_live_component_hashes(args: argparse.Namespace) -> None:
    for field, (component_id, component_kind) in ETH_TEMPLATE_COMPONENTS.items():
        if getattr(args, field) == _evm_family_template_component_hash(
            component_id,
            component_kind,
        ):
            label = field.replace("_", " ")
            raise ValueError(
                f"ETH production source evidence requires live {label}; "
                f"template-derived {label} is not deployable"
            )


def _require_canonical_adapter_verifier_vk_hash(args: argparse.Namespace) -> None:
    expected_hash = eth_source_adapter_verifier_vk_hash(
        source_domain=args.source_domain,
        target_domain=args.target_domain,
    )
    if args.adapter_verifier_vk_hash != expected_hash:
        raise ValueError(
            "--adapter-verifier-vk-hash does not match the canonical "
            "ETH source-adapter verifier profile: "
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
                "ETH source-adapter role hashes must be distinct: "
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
        material_hash = eth_source_verifier_material_record_hash(args)
        if expected_material_hash != material_hash:
            raise ValueError(
                "--expected-source-verifier-material-hash does not match the "
                "canonical ETH source verifier material record: "
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
        deployment_hash = eth_source_adapter_engine_deployment_record_hash(args)
        if expected_deployment_hash != deployment_hash:
            raise ValueError(
                "--expected-source-adapter-engine-deployment-hash does not match "
                "the canonical ETH source-adapter deployment record: "
                f"expected {_hex(expected_deployment_hash)}, got {_hex(deployment_hash)}"
            )


def _require_toml_receipt_metadata(
    args: argparse.Namespace,
    *,
    output: str,
) -> None:
    for field, flag in (
        ("deployment_transaction_hash", "--deployment-transaction-hash"),
        ("deployment_receipt_contract_address", "--deployment-receipt-contract-address"),
        ("deployment_receipt_block_hash", "--deployment-receipt-block-hash"),
        ("deployment_receipt_block_number", "--deployment-receipt-block-number"),
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


def _validate_eth_source_evidence_args(args: argparse.Namespace) -> None:
    _require_eth_sora_lane(args)
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
    yield "[[zk.sccp_source_verifier_materials]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.source_domain)
    yield _toml_line("source_chain", "eth")
    yield _toml_line("source_proof_plan", "EthereumBeaconReceiptProof")
    yield _toml_line("finality_model", "EthereumBeaconExecution")
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("source_trust_anchor_id", ETH_SOURCE_TRUST_ANCHOR_ID)
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", ETH_CONSENSUS_VERIFIER_ID)
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        ETH_MESSAGE_INCLUSION_VERIFIER_ID,
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line("source_bridge_emitter_id", ETH_SOURCE_BRIDGE_EMITTER_ID)
    yield _toml_line("source_bridge_emitter_address", _hex(args.bridge_address))
    yield _toml_line(
        "source_bridge_emitter_code_hash",
        _hex(args.source_bridge_emitter_code_hash),
    )
    yield _toml_line("finality_policy_id", ETH_FINALITY_POLICY_ID)
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("placeholder_material", False)


def _deployment_lines(args: argparse.Namespace) -> Iterable[str]:
    yield "[[zk.sccp_source_adapter_engine_deployments]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.source_domain)
    yield _toml_line("target_domain", args.target_domain)
    yield _toml_line("source_chain", "eth")
    yield _toml_line("source_proof_plan", "EthereumBeaconReceiptProof")
    yield _toml_line("finality_model", "EthereumBeaconExecution")
    yield _toml_line("adapter_proof_family", SCCP_PROOF_FAMILY_STARK_FRI)
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("adapter_verifier_vk_hash", _hex(args.adapter_verifier_vk_hash))
    yield _toml_line("source_trust_anchor_id", ETH_SOURCE_TRUST_ANCHOR_ID)
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", ETH_CONSENSUS_VERIFIER_ID)
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        ETH_MESSAGE_INCLUSION_VERIFIER_ID,
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line("source_bridge_emitter_id", ETH_SOURCE_BRIDGE_EMITTER_ID)
    yield _toml_line("source_bridge_emitter_address", _hex(args.bridge_address))
    yield _toml_line(
        "source_bridge_emitter_code_hash",
        _hex(args.source_bridge_emitter_code_hash),
    )
    yield _toml_line("finality_policy_id", ETH_FINALITY_POLICY_ID)
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("deployment_receipt_hash", _hex(args.deployment_receipt_hash))


def render_toml(args: argparse.Namespace) -> str:
    """Render ETH source material and source adapter deployment TOML records."""

    apply_runtime_bytecode_hash(args)
    _validate_eth_source_evidence_args(args)
    material_hash = eth_source_verifier_material_record_hash(args)
    deployment_hash = eth_source_adapter_engine_deployment_record_hash(args)
    _require_expected_record_hashes(args, output="toml")
    _require_toml_receipt_metadata(args, output="toml")
    _require_toml_runtime_bytecode_metadata(args, output="toml")
    comments = [
        "# sccp_evm_source_rpc_chain_id = " + json.dumps(str(ETH_RPC_CHAIN_ID)),
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
            "# sccp_evm_source_deployment_receipt_status = " + json.dumps("0x1"),
            "# sccp_evm_source_deployment_contract_address = "
            + json.dumps(_hex(args.deployment_receipt_contract_address)),
            "# sccp_evm_source_deployment_block_hash = "
            + json.dumps(_hex(args.deployment_receipt_block_hash)),
            "# sccp_evm_source_deployment_block_number = "
            + json.dumps(str(args.deployment_receipt_block_number)),
            "# sccp_eth_source_verifier_material_hash = "
            + json.dumps(_hex(material_hash)),
        ]
    )
    return "\n".join(
        [
            *comments,
            *_material_lines(args),
            "",
            "# sccp_eth_source_adapter_engine_deployment_hash = "
            + json.dumps(_hex(deployment_hash)),
            *_deployment_lines(args),
            "",
        ]
    )


def _json_summary(args: argparse.Namespace) -> dict[str, object]:
    apply_runtime_bytecode_hash(args)
    _validate_eth_source_evidence_args(args)
    material_hash = eth_source_verifier_material_record_hash(args)
    deployment_hash = eth_source_adapter_engine_deployment_record_hash(args)
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
        "source_chain": "eth",
        "source_proof_plan": "EthereumBeaconReceiptProof",
        "finality_model": "EthereumBeaconExecution",
        "source_bridge_emitter_id": ETH_SOURCE_BRIDGE_EMITTER_ID,
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
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Render SCCP Ethereum source bridge deployment evidence.",
    )
    parser.add_argument(
        "--source-domain",
        default=SCCP_DOMAIN_ETH,
        type=lambda value: parse_u32(value, label="source domain"),
        help="SCCP source domain. Defaults to Ethereum (1).",
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
        help="Ethereum source bridge address as a non-zero 20-byte EVM hex address.",
    )
    parser.add_argument(
        "--source-bridge-runtime-bytecode-hex",
        type=lambda value: parse_runtime_bytecode_hex(
            value,
            label="source bridge runtime bytecode",
        ),
        help=(
            "Hex-encoded deployed Ethereum source bridge runtime bytecode. "
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
            "File containing deployed Ethereum source bridge runtime bytecode hex. "
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
            help="Non-zero bytes32 Ethereum deployment evidence.",
        )
    parser.add_argument(
        "--expected-source-verifier-material-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected source verifier material hash",
            byte_length=32,
        ),
        help=(
            "Optional governed ETH source verifier material record hash. "
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
            "Optional governed ETH source-adapter deployment record hash. "
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
        "--toml",
        action="store_true",
        help="Render production TOML records instead of a compact JSON summary.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        apply_runtime_bytecode_hash(args)
        _validate_eth_source_evidence_args(args)
        if args.toml:
            print(render_toml(args), end="")
        else:
            print(json.dumps(_json_summary(args), sort_keys=True, indent=2))
    except ValueError as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
