#!/usr/bin/env python3
"""Render SCCP TON source-state verifier deployment evidence.

This helper is offline by design. Operators pass the live TON mainnet source
trust anchor, verifier component hashes, source-state verifier hash, adapter
verifier key hash, and deployment receipt hash collected from governance or
deployment records. The script validates that all production evidence hashes are
non-zero and can render the matching `zk.sccp_source_verifier_materials` and
`zk.sccp_source_adapter_engine_deployments` TOML records for the TON -> SORA
source lane. Optional TON full light-client verifier hashes are emitted as an
all-or-nothing audit bundle and, when complete, are bound into the source
adapter deployment record hash.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from typing import Iterable


SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_TON = 4
SCCP_PROOF_FAMILY_STARK_FRI = "stark-fri-v1"
SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID = "sccp-source-adapter-v1"
SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET = "fastpq-lane-balanced"
SCCP_TON_CONTRACT_PROOF_BACKEND = "ton-contract-v1"
SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID = (
    "sccp-ton-shard-state-light-client-v1"
)
TON_SOURCE_PROOF_PLAN_CODE = 4
TON_FINALITY_MODEL_CODE = 4
FASTPQ_BALANCED_TRACE_ROOT = 0x002A_247F_81C6_F850
FASTPQ_BALANCED_LDE_ROOT = 0x6026_3388_DBBF_9B2A
FASTPQ_BALANCED_OMEGA_COSET = 0x6AF3_25E8_25AD_5C18

TON_SOURCE_TRUST_ANCHOR_ID = (
    "sccp:ton:source-trust-anchor:ton-mainnet-masterchain:v1"
)
TON_CONSENSUS_VERIFIER_ID = "sccp:ton:consensus-verifier:masterchain-block-proof:v1"
TON_MESSAGE_INCLUSION_VERIFIER_ID = (
    "sccp:ton:message-inclusion-verifier:shard-transaction-branch:v1"
)
TON_SOURCE_STATE_VERIFIER_ID = (
    "sccp:ton:source-state-verifier:shard-state-light-client-mainnet:v1"
)
TON_FINALITY_POLICY_ID = "sccp:ton:finality-policy:masterchain-finality:v1"
TON_MASTERCHAIN_CONFIG_VERIFIER_ID = (
    "sccp:ton:light-client:masterchain-config-mainnet:v1"
)
TON_VALIDATOR_SET_TRANSITION_VERIFIER_ID = (
    "sccp:ton:light-client:validator-set-transition-mainnet:v1"
)
TON_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID = (
    "sccp:ton:light-client:shard-accounts-dictionary-mainnet:v1"
)
TON_FULL_LIGHT_CLIENT_GATE_PREFIX = b"sccp:ton:full-light-client-gate:v1"
TON_TEMPLATE_COMPONENTS = {
    "source_trust_anchor_hash": (
        TON_SOURCE_TRUST_ANCHOR_ID,
        "source-trust-anchor",
    ),
    "consensus_verifier_hash": (
        TON_CONSENSUS_VERIFIER_ID,
        "consensus-verifier",
    ),
    "message_inclusion_verifier_hash": (
        TON_MESSAGE_INCLUSION_VERIFIER_ID,
        "message-inclusion-verifier",
    ),
    "source_state_verifier_hash": (
        TON_SOURCE_STATE_VERIFIER_ID,
        "source-state-verifier",
    ),
    "finality_policy_hash": (
        TON_FINALITY_POLICY_ID,
        "finality-policy",
    ),
}
TON_TEMPLATE_TRANSCRIPT_PREFIXES = (
    b"sccp:ton:shard-proof:v1",
    b"sccp:ton:validator-set:v1",
    b"sccp:ton:validator-set-payload:v1",
    b"sccp:ton:masterchain-config-leaf:v1",
    b"sccp:ton:masterchain-config-proof:v1",
    b"sccp:ton:masterchain-block-message:v1",
    b"sccp:ton:masterchain-signatures:v1",
    b"sccp:ton:validator-set-transition-message:v1",
    b"sccp:ton:validator-set-transition-signatures:v1",
    b"sccp:ton:shard-state-proof-public-inputs:v1",
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


def _require_exact_u32(
    value: object,
    label: str,
    error_type: type[Exception] = ValueError,
) -> int:
    if type(value) is not int or value < 0 or value > 0xFFFFFFFF:
        raise error_type(f"{label} must be an exact u32")
    return value


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


def _ton_template_component_hash(component_id: str, component_kind: str) -> bytes:
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, SCCP_DOMAIN_TON)
    _push_vec(payload, b"ton")
    _push_u8(payload, TON_SOURCE_PROOF_PLAN_CODE)  # TonMasterchainShardProof
    _push_u8(payload, TON_FINALITY_MODEL_CODE)  # TonMasterchain
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, SCCP_TON_CONTRACT_PROOF_BACKEND.encode("utf-8"))
    for prefix in TON_TEMPLATE_TRANSCRIPT_PREFIXES:
        _push_vec(payload, prefix)
    _push_vec(payload, component_kind.encode("utf-8"))
    _push_vec(payload, component_id.encode("utf-8"))
    return _prefixed_blake2b(b"sccp:ton:source-verifier-material:v1", payload)


def ton_source_adapter_verifier_vk_hash(
    *,
    source_domain: int = SCCP_DOMAIN_TON,
    target_domain: int = SCCP_DOMAIN_SORA,
) -> bytes:
    """Compute Rust's canonical OpenVerify vk hash for TON -> SORA."""

    source_domain = _require_exact_u32(source_domain, "source_domain")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_TON:
        raise ValueError("source_domain must be TON")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")

    verifier = bytearray()
    _push_u8(verifier, 1)
    _push_vec(verifier, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(verifier, b"ton")
    _push_u32(verifier, source_domain)
    _push_u32(verifier, target_domain)
    _push_u8(verifier, TON_SOURCE_PROOF_PLAN_CODE)
    _push_u8(verifier, TON_FINALITY_MODEL_CODE)
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


def ton_source_verifier_material_record_hash(args: argparse.Namespace) -> bytes:
    """Compute Rust's canonical TON source verifier material record hash."""

    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    if source_domain != SCCP_DOMAIN_TON:
        raise ValueError("source_domain must be TON")
    _require_live_component_hashes(args)
    _require_source_role_hash_separation(args)
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, source_domain)
    _push_vec(payload, b"ton")
    _push_u8(payload, TON_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, TON_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, TON_SOURCE_TRUST_ANCHOR_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_trust_anchor_hash,
            label="source_trust_anchor_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, TON_CONSENSUS_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.consensus_verifier_hash,
            label="consensus_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, TON_MESSAGE_INCLUSION_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.message_inclusion_verifier_hash,
            label="message_inclusion_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, TON_FINALITY_POLICY_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.finality_policy_hash,
            label="finality_policy_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, TON_SOURCE_STATE_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_state_verifier_hash,
            label="source_state_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, b"")
    _push_vec(payload, b"")
    payload.extend(bytes(32))
    payload.extend(bytes(32))
    _push_vec(payload, b"")
    payload.extend(bytes(32))
    _push_u8(payload, 0)
    return _prefixed_blake2b(
        b"sccp:source-verifier-material-record:v1",
        bytes(payload),
    )


def ton_source_adapter_engine_deployment_record_hash(args: argparse.Namespace) -> bytes:
    """Compute Rust's canonical TON source-adapter deployment record hash."""

    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_TON:
        raise ValueError("source_domain must be TON")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")
    _require_live_component_hashes(args)
    _require_source_role_hash_separation(args)
    adapter_verifier_vk_hash = _require_nonzero_fixed_bytes(
        args.adapter_verifier_vk_hash,
        label="adapter_verifier_vk_hash",
        byte_length=32,
    )
    expected_adapter_verifier_vk_hash = ton_source_adapter_verifier_vk_hash(
        source_domain=source_domain,
        target_domain=target_domain,
    )
    if adapter_verifier_vk_hash != expected_adapter_verifier_vk_hash:
        raise ValueError(
            "adapter_verifier_vk_hash must match the canonical "
            "TON source-adapter verifier profile"
        )

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, source_domain)
    _push_u32(payload, target_domain)
    _push_vec(payload, b"ton")
    _push_u8(payload, TON_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, TON_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8"))
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    payload.extend(adapter_verifier_vk_hash)
    _push_vec(payload, TON_SOURCE_TRUST_ANCHOR_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_trust_anchor_hash,
            label="source_trust_anchor_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, TON_CONSENSUS_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.consensus_verifier_hash,
            label="consensus_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, TON_MESSAGE_INCLUSION_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.message_inclusion_verifier_hash,
            label="message_inclusion_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, TON_FINALITY_POLICY_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.finality_policy_hash,
            label="finality_policy_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, TON_SOURCE_STATE_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_state_verifier_hash,
            label="source_state_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, b"")
    _push_vec(payload, b"")
    payload.extend(bytes(32))
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
    supplied_light_client_hashes = _require_complete_light_client_evidence_hashes(args)
    if supplied_light_client_hashes:
        _push_u8(payload, 2)
        for field, engine_id in _light_client_evidence_fields():
            _push_vec(payload, engine_id.encode("utf-8"))
            payload.extend(supplied_light_client_hashes[field])
    return _prefixed_blake2b(
        b"sccp:source-adapter-engine-deployment:v1",
        bytes(payload),
    )


def _light_client_evidence_fields() -> tuple[tuple[str, str], ...]:
    return (
        ("masterchain_config_verifier_hash", TON_MASTERCHAIN_CONFIG_VERIFIER_ID),
        (
            "validator_set_transition_verifier_hash",
            TON_VALIDATOR_SET_TRANSITION_VERIFIER_ID,
        ),
        (
            "shard_accounts_dictionary_verifier_hash",
            TON_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID,
        ),
    )


def _light_client_evidence_hashes(args: argparse.Namespace) -> dict[str, bytes]:
    hashes: dict[str, bytes] = {}
    for field, _engine_id in _light_client_evidence_fields():
        value = getattr(args, field, None)
        if value is not None:
            hashes[field] = _require_nonzero_fixed_bytes(
                value,
                label=field,
                byte_length=32,
            )
    return hashes


def _missing_light_client_evidence_ids(hashes: dict[str, bytes]) -> list[str]:
    return [
        engine_id
        for field, engine_id in _light_client_evidence_fields()
        if field not in hashes
    ]


def _require_complete_light_client_evidence_hashes(
    args: argparse.Namespace,
) -> dict[str, bytes]:
    hashes = _light_client_evidence_hashes(args)
    missing = _missing_light_client_evidence_ids(hashes)
    if hashes and missing:
        raise SystemExit(
            "TON full light-client evidence must include all verifier hashes: "
            + ", ".join(missing)
        )
    if hashes:
        _require_light_client_evidence_role_separation(args, hashes)
    return hashes


def ton_full_light_client_gate_hash(args: argparse.Namespace) -> bytes | None:
    """Compute the governed TON full light-client gate hash."""

    hashes = _require_complete_light_client_evidence_hashes(args)
    if len(hashes) != len(_light_client_evidence_fields()):
        return None
    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, source_domain)
    _push_u32(payload, target_domain)
    _push_vec(payload, b"ton")
    _push_u8(payload, TON_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, TON_FINALITY_MODEL_CODE)
    payload.extend((-239).to_bytes(4, "little", signed=True))
    _push_vec(payload, SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET.encode("utf-8"))
    _push_vec(payload, TON_SOURCE_STATE_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_state_verifier_hash,
            label="source_state_verifier_hash",
            byte_length=32,
        )
    )
    payload.extend(ton_source_verifier_material_record_hash(args))
    payload.extend(ton_source_adapter_engine_deployment_record_hash(args))
    for field, engine_id in _light_client_evidence_fields():
        _push_vec(payload, engine_id.encode("utf-8"))
        payload.extend(hashes[field])
    return _prefixed_blake2b(TON_FULL_LIGHT_CLIENT_GATE_PREFIX, bytes(payload))


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


def _require_ton_sora_lane(args: argparse.Namespace) -> None:
    source_domain = _require_exact_u32(args.source_domain, "source_domain", SystemExit)
    target_domain = _require_exact_u32(args.target_domain, "target_domain", SystemExit)
    if source_domain != SCCP_DOMAIN_TON:
        raise SystemExit("TON production source evidence requires source_domain = 4")
    if target_domain != SCCP_DOMAIN_SORA:
        raise SystemExit("TON production source evidence requires target_domain = 0")


def _require_live_component_hashes(args: argparse.Namespace) -> None:
    for field, (component_id, component_kind) in TON_TEMPLATE_COMPONENTS.items():
        if getattr(args, field) == _ton_template_component_hash(
            component_id,
            component_kind,
        ):
            label = field.replace("_", " ")
            raise SystemExit(
                f"TON production source evidence requires live {label}; "
                f"template-derived {label} is not deployable"
            )


def _template_component_hashes() -> dict[str, bytes]:
    return {
        field: _ton_template_component_hash(component_id, component_kind)
        for field, (component_id, component_kind) in TON_TEMPLATE_COMPONENTS.items()
    }


def _require_canonical_adapter_verifier_vk_hash(args: argparse.Namespace) -> None:
    expected_hash = ton_source_adapter_verifier_vk_hash(
        source_domain=args.source_domain,
        target_domain=args.target_domain,
    )
    if args.adapter_verifier_vk_hash != expected_hash:
        raise SystemExit(
            "--adapter-verifier-vk-hash does not match the canonical "
            "TON source-adapter verifier profile: "
            f"expected {_hex(expected_hash)}, got {_hex(args.adapter_verifier_vk_hash)}"
        )


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
            raise SystemExit(
                f"--{output} requires --expected-source-verifier-material-hash"
            )
    else:
        material_hash = ton_source_verifier_material_record_hash(args)
        if expected_material_hash != material_hash:
            raise SystemExit(
                "--expected-source-verifier-material-hash does not match the "
                "canonical TON source verifier material record: "
                f"expected {_hex(expected_material_hash)}, got {_hex(material_hash)}"
            )

    expected_deployment_hash = getattr(
        args,
        "expected_source_adapter_engine_deployment_hash",
        None,
    )
    if expected_deployment_hash is None:
        if output is not None:
            raise SystemExit(
                f"--{output} requires "
                "--expected-source-adapter-engine-deployment-hash"
            )
    else:
        deployment_hash = ton_source_adapter_engine_deployment_record_hash(args)
        if expected_deployment_hash != deployment_hash:
            raise SystemExit(
                "--expected-source-adapter-engine-deployment-hash does not match "
                "the canonical TON source-adapter deployment record: "
                f"expected {_hex(expected_deployment_hash)}, got {_hex(deployment_hash)}"
            )


def _require_full_light_client_evidence_consistency(
    args: argparse.Namespace,
    *,
    output: str | None = None,
) -> None:
    supplied = _require_complete_light_client_evidence_hashes(args)
    _require_light_client_evidence_role_separation(args, supplied)
    expected_gate_hash = getattr(args, "expected_full_light_client_gate_hash", None)
    gate_hash = ton_full_light_client_gate_hash(args)
    if output is not None and gate_hash is None:
        raise SystemExit(
            f"--{output} requires the masterchain config, validator-set "
            "transition, shard-accounts dictionary verifier hashes, and "
            "--expected-full-light-client-gate-hash"
        )
    if expected_gate_hash is not None and gate_hash is None:
        raise SystemExit(
            "--expected-full-light-client-gate-hash requires the masterchain "
            "config, validator-set transition, and shard-accounts dictionary "
            "verifier hashes"
        )
    if output is not None and supplied and expected_gate_hash is None:
        raise SystemExit(
            f"--{output} with TON full light-client audit evidence requires "
            "--expected-full-light-client-gate-hash"
        )
    if expected_gate_hash is not None and expected_gate_hash != gate_hash:
        raise SystemExit(
            "--expected-full-light-client-gate-hash does not match the TON "
            "full light-client audit record: "
            f"expected {_hex(expected_gate_hash)}, got {_hex(gate_hash)}"
        )


def _validate_ton_source_evidence_args(args: argparse.Namespace) -> None:
    _require_ton_sora_lane(args)
    _require_live_component_hashes(args)
    _require_canonical_adapter_verifier_vk_hash(args)
    _require_source_role_hash_separation(args)
    _require_expected_record_hashes(args)
    _require_full_light_client_evidence_consistency(args)


def _component_hash_args() -> tuple[str, ...]:
    return (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "finality_policy_hash",
        "source_state_verifier_hash",
        "adapter_verifier_vk_hash",
        "deployment_receipt_hash",
    )


def _require_source_role_hash_separation(args: argparse.Namespace) -> None:
    seen: dict[bytes, str] = {}
    for field in _component_hash_args():
        value = getattr(args, field, None)
        if value is None:
            continue
        previous_field = seen.get(value)
        if previous_field is not None:
            raise SystemExit(
                "TON source-adapter role hashes must be distinct: "
                f"{field} matches {previous_field}"
            )
        seen[value] = field


def _require_light_client_evidence_role_separation(
    args: argparse.Namespace,
    hashes: dict[str, bytes],
) -> None:
    seen: dict[bytes, str] = {}
    for field, _engine_id in _light_client_evidence_fields():
        value = hashes.get(field)
        if value is None:
            continue
        previous_field = seen.get(value)
        if previous_field is not None:
            raise SystemExit(
                "TON full-light-client verifier hashes must be role-separated: "
                f"{field} matches {previous_field}"
            )
        seen[value] = field

    for field, value in hashes.items():
        for role_field in _component_hash_args():
            role_value = getattr(args, role_field, None)
            if role_value is not None and value == role_value:
                raise SystemExit(
                    "TON full-light-client verifier hashes must not reuse "
                    f"existing source-adapter material: {field} matches {role_field}"
                )
        for template_field, template_hash in _template_component_hashes().items():
            if value == template_hash:
                raise SystemExit(
                    "TON full-light-client verifier hashes must not reuse "
                    f"built-in template material: {field} matches {template_field}"
                )


def _material_lines(args: argparse.Namespace) -> Iterable[str]:
    yield "[[zk.sccp_source_verifier_materials]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.source_domain)
    yield _toml_line("source_chain", "ton")
    yield _toml_line("source_proof_plan", "TonMasterchainShardProof")
    yield _toml_line("finality_model", "TonMasterchain")
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("source_trust_anchor_id", TON_SOURCE_TRUST_ANCHOR_ID)
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", TON_CONSENSUS_VERIFIER_ID)
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        TON_MESSAGE_INCLUSION_VERIFIER_ID,
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line("source_state_verifier_id", TON_SOURCE_STATE_VERIFIER_ID)
    yield _toml_line("source_state_verifier_hash", _hex(args.source_state_verifier_hash))
    yield _toml_line("finality_policy_id", TON_FINALITY_POLICY_ID)
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("placeholder_material", False)


def _deployment_lines(args: argparse.Namespace) -> Iterable[str]:
    yield "[[zk.sccp_source_adapter_engine_deployments]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.source_domain)
    yield _toml_line("target_domain", args.target_domain)
    yield _toml_line("source_chain", "ton")
    yield _toml_line("source_proof_plan", "TonMasterchainShardProof")
    yield _toml_line("finality_model", "TonMasterchain")
    yield _toml_line("adapter_proof_family", SCCP_PROOF_FAMILY_STARK_FRI)
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("adapter_verifier_vk_hash", _hex(args.adapter_verifier_vk_hash))
    yield _toml_line("source_trust_anchor_id", TON_SOURCE_TRUST_ANCHOR_ID)
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", TON_CONSENSUS_VERIFIER_ID)
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        TON_MESSAGE_INCLUSION_VERIFIER_ID,
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line("source_state_verifier_id", TON_SOURCE_STATE_VERIFIER_ID)
    yield _toml_line("source_state_verifier_hash", _hex(args.source_state_verifier_hash))
    yield _toml_line("finality_policy_id", TON_FINALITY_POLICY_ID)
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("deployment_receipt_hash", _hex(args.deployment_receipt_hash))
    supplied = _light_client_evidence_hashes(args)
    gate_hash = ton_full_light_client_gate_hash(args)
    if gate_hash is not None:
        yield _toml_line(
            "ton_masterchain_config_verifier_hash",
            _hex(supplied["masterchain_config_verifier_hash"]),
        )
        yield _toml_line(
            "ton_validator_set_transition_verifier_hash",
            _hex(supplied["validator_set_transition_verifier_hash"]),
        )
        yield _toml_line(
            "ton_shard_accounts_dictionary_verifier_hash",
            _hex(supplied["shard_accounts_dictionary_verifier_hash"]),
        )
        yield _toml_line("ton_full_light_client_gate_hash", _hex(gate_hash))


def _full_light_client_audit_lines(args: argparse.Namespace) -> Iterable[str]:
    supplied = _light_client_evidence_hashes(args)
    gate_hash = ton_full_light_client_gate_hash(args)
    yield "# TON full light-client audit evidence."
    yield "# source_adapter_gate_closed_until_full_light_client = true"
    yield "# Runtime production admission remains fail-closed until this audit"
    yield "# record is consumed by a governed TON light-client readiness predicate."
    if gate_hash is None:
        missing_ids = [
            engine_id
            for field, engine_id in _light_client_evidence_fields()
            if field not in supplied
        ]
        yield "# full_light_client_evidence_ready = false"
        yield "# missing_light_client_verifier_ids = " + json.dumps(missing_ids)
        return

    yield "# full_light_client_evidence_ready = true"
    yield "# ton_full_light_client_gate_hash = " + json.dumps(_hex(gate_hash))
    for field, engine_id in _light_client_evidence_fields():
        yield f"# {field.removesuffix('_hash')}_id = " + json.dumps(engine_id)
        yield f"# {field} = " + json.dumps(_hex(supplied[field]))


def render_toml(args: argparse.Namespace) -> str:
    """Render source material and source adapter deployment TOML records."""

    _validate_ton_source_evidence_args(args)
    material_hash = ton_source_verifier_material_record_hash(args)
    deployment_hash = ton_source_adapter_engine_deployment_record_hash(args)
    _require_expected_record_hashes(args, output="toml")
    _require_full_light_client_evidence_consistency(args, output="toml")
    return "\n".join(
        [
            "# sccp_ton_source_verifier_material_hash = "
            + json.dumps(_hex(material_hash)),
            *_material_lines(args),
            "",
            "# sccp_ton_source_adapter_engine_deployment_hash = "
            + json.dumps(_hex(deployment_hash)),
            *_deployment_lines(args),
            "",
            *_full_light_client_audit_lines(args),
            "",
        ]
    )


def _json_summary(args: argparse.Namespace) -> dict[str, object]:
    _validate_ton_source_evidence_args(args)
    material_hash = ton_source_verifier_material_record_hash(args)
    deployment_hash = ton_source_adapter_engine_deployment_record_hash(args)
    expected_material_matches = (
        getattr(args, "expected_source_verifier_material_hash", None) == material_hash
    )
    expected_deployment_matches = (
        getattr(args, "expected_source_adapter_engine_deployment_hash", None)
        == deployment_hash
    )
    gate_hash = ton_full_light_client_gate_hash(args)
    expected_gate_hash = getattr(args, "expected_full_light_client_gate_hash", None)
    full_light_client_evidence_ready = gate_hash is not None
    expected_gate_matches = gate_hash is not None and expected_gate_hash == gate_hash
    supplied = _light_client_evidence_hashes(args)
    light_client_hashes = {
        field: _hex(value)
        for field, value in supplied.items()
        if len(supplied) == len(_light_client_evidence_fields())
    }
    missing_ids = [
        engine_id
        for field, engine_id in _light_client_evidence_fields()
        if field not in supplied
    ]
    source_adapter_gate_blockers = []
    if not expected_material_matches:
        source_adapter_gate_blockers.append(
            "source verifier material hash is not pinned or mismatched"
        )
    if not expected_deployment_matches:
        source_adapter_gate_blockers.append(
            "source adapter deployment hash is not pinned or mismatched"
        )
    if not full_light_client_evidence_ready:
        source_adapter_gate_blockers.append(
            "full light-client verifier hashes are incomplete"
        )
    elif not expected_gate_matches:
        source_adapter_gate_blockers.append(
            "full light-client gate hash is not pinned or mismatched"
        )
    source_adapter_gate_ready = not source_adapter_gate_blockers
    return {
        "source_domain": args.source_domain,
        "target_domain": args.target_domain,
        "source_chain": "ton",
        "source_proof_plan": "TonMasterchainShardProof",
        "finality_model": "TonMasterchain",
        "source_state_verifier_id": TON_SOURCE_STATE_VERIFIER_ID,
        "source_state_verifier_hash": _hex(args.source_state_verifier_hash),
        "adapter_verifier_vk_hash": _hex(args.adapter_verifier_vk_hash),
        "deployment_receipt_hash": _hex(args.deployment_receipt_hash),
        "source_verifier_material_hash": _hex(material_hash),
        "source_adapter_engine_deployment_hash": _hex(deployment_hash),
        "expected_source_verifier_material_hash_matches": expected_material_matches,
        "expected_source_adapter_engine_deployment_hash_matches": (
            expected_deployment_matches
        ),
        "source_verifier_material_ready": expected_material_matches,
        "source_adapter_engine_deployment_ready": expected_deployment_matches,
        "source_adapter_gate_closed_until_full_light_client": True,
        "source_adapter_gate_ready_with_full_light_client_evidence": (
            source_adapter_gate_ready
        ),
        "source_adapter_gate_blockers": source_adapter_gate_blockers,
        "full_light_client_evidence_ready": full_light_client_evidence_ready,
        "full_light_client_gate_hash": _hex(gate_hash) if gate_hash is not None else None,
        "expected_full_light_client_gate_hash_matches": expected_gate_matches,
        "full_light_client_verifier_ids": [
            engine_id for _field, engine_id in _light_client_evidence_fields()
        ],
        "full_light_client_verifier_hashes": light_client_hashes,
        "missing_full_light_client_verifier_ids": missing_ids,
        "full_toml_ready": source_adapter_gate_ready,
        "toml_ready": source_adapter_gate_ready,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Render SCCP TON source-state deployment evidence.",
    )
    parser.add_argument(
        "--source-domain",
        default=SCCP_DOMAIN_TON,
        type=lambda value: parse_u32(value, label="source domain"),
        help="SCCP source domain. Defaults to TON (4).",
    )
    parser.add_argument(
        "--target-domain",
        default=SCCP_DOMAIN_SORA,
        type=lambda value: parse_u32(value, label="target domain"),
        help="SCCP target domain. Defaults to SORA (0).",
    )
    for name in _component_hash_args():
        parser.add_argument(
            "--" + name.replace("_", "-"),
            required=True,
            type=lambda value, field=name: parse_hex_bytes(
                value,
                label=field.replace("_", " "),
                byte_length=32,
            ),
            help="Non-zero bytes32 TON deployment evidence.",
        )
    parser.add_argument(
        "--expected-source-verifier-material-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected source verifier material hash",
            byte_length=32,
        ),
        help=(
            "Optional governed TON source verifier material record hash. "
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
            "Optional governed TON source-adapter deployment record hash. "
            "Mismatches fail instead of rendering evidence."
        ),
    )
    for field, engine_id in _light_client_evidence_fields():
        parser.add_argument(
            "--" + field.replace("_", "-"),
            type=lambda value, label=field: parse_hex_bytes(
                value,
                label=label.replace("_", " "),
                byte_length=32,
            ),
            help=(
                "Optional non-zero bytes32 audit evidence for "
                f"{engine_id}. All TON full light-client verifier hashes "
                "must be supplied together."
            ),
        )
    parser.add_argument(
        "--expected-full-light-client-gate-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected full light client gate hash",
            byte_length=32,
        ),
        help=(
            "Optional expected audit hash for the governed TON masterchain "
            "config, validator-set transition, and shard-accounts dictionary "
            "verifier evidence."
        ),
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
        _validate_ton_source_evidence_args(args)
        if args.toml:
            print(render_toml(args), end="")
        else:
            print(json.dumps(_json_summary(args), sort_keys=True, indent=2))
    except SystemExit as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
