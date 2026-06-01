#!/usr/bin/env python3
"""Render SCCP Substrate-family source verifier deployment evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
from typing import Iterable


SCCP_DOMAIN_SORA = 0
SCCP_PROOF_FAMILY_STARK_FRI = "stark-fri-v1"
SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID = "sccp-source-adapter-v1"
SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET = "fastpq-lane-balanced"
SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND = "substrate-runtime-v1"
SCCP_SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID = (
    "sccp-substrate-runtime-storage-v1"
)
SCCP_SUBSTRATE_RUNTIME_STORAGE_FASTPQ_PARAMETER_SET = "fastpq-lane-balanced"
SCCP_SUBSTRATE_RUNTIME_STORAGE_GATE_PREFIX = (
    b"sccp:substrate:runtime-storage-gate:v1"
)
SUBSTRATE_SOURCE_PROOF_PLAN_CODE = 6
SUBSTRATE_FINALITY_MODEL_CODE = 6
FASTPQ_BALANCED_TRACE_ROOT = 0x002A_247F_81C6_F850
FASTPQ_BALANCED_LDE_ROOT = 0x6026_3388_DBBF_9B2A
FASTPQ_BALANCED_OMEGA_COSET = 0x6AF3_25E8_25AD_5C18
SUBSTRATE_TEMPLATE_TRANSCRIPT_PREFIXES = (
    b"sccp:substrate:storage-proof:v1",
    b"sccp:substrate:runtime-storage-proof-public-inputs:v1",
    b"sccp:substrate:authority-set:v1",
    b"sccp:substrate:authority-set-payload:v1",
    b"sccp:substrate:grandpa-precommit:v1",
    b"sccp:substrate:grandpa-justification:v1",
    b"sccp:substrate:authority-set-transition-message:v1",
    b"sccp:substrate:authority-set-transition-justification:v1",
)
SUBSTRATE_SOURCE_PROFILES = {
    6: {
        "chain": "sora-kusama",
        "source_trust_anchor_id": (
            "sccp:sora-kusama:source-trust-anchor:grandpa-authority-set:v1"
        ),
        "consensus_verifier_id": (
            "sccp:sora-kusama:consensus-verifier:grandpa-finalized-header:v1"
        ),
        "message_inclusion_verifier_id": (
            "sccp:sora-kusama:message-inclusion-verifier:events-storage-proof:v1"
        ),
        "source_state_verifier_id": (
            "sccp:sora-kusama:source-state-verifier:runtime-storage-proof:v1"
        ),
        "finality_policy_id": "sccp:sora-kusama:finality-policy:grandpa-finality:v1",
    },
    7: {
        "chain": "sora-polkadot",
        "source_trust_anchor_id": (
            "sccp:sora-polkadot:source-trust-anchor:grandpa-authority-set:v1"
        ),
        "consensus_verifier_id": (
            "sccp:sora-polkadot:consensus-verifier:grandpa-finalized-header:v1"
        ),
        "message_inclusion_verifier_id": (
            "sccp:sora-polkadot:message-inclusion-verifier:events-storage-proof:v1"
        ),
        "source_state_verifier_id": (
            "sccp:sora-polkadot:source-state-verifier:runtime-storage-proof:v1"
        ),
        "finality_policy_id": (
            "sccp:sora-polkadot:finality-policy:grandpa-finality:v1"
        ),
    },
    8: {
        "chain": "sora2",
        "source_trust_anchor_id": (
            "sccp:sora2:source-trust-anchor:grandpa-authority-set:v1"
        ),
        "consensus_verifier_id": (
            "sccp:sora2:consensus-verifier:grandpa-finalized-header:v1"
        ),
        "message_inclusion_verifier_id": (
            "sccp:sora2:message-inclusion-verifier:events-storage-proof:v1"
        ),
        "source_state_verifier_id": (
            "sccp:sora2:source-state-verifier:runtime-storage-proof:v1"
        ),
        "finality_policy_id": "sccp:sora2:finality-policy:grandpa-finality:v1",
    },
}
SUBSTRATE_DOMAIN_ALIASES = {
    "6": 6,
    "sora-kusama": 6,
    "kusama": 6,
    "7": 7,
    "sora-polkadot": 7,
    "polkadot": 7,
    "8": 8,
    "sora2": 8,
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


def parse_substrate_domain(value: str) -> int:
    """Parse a supported Substrate-family SCCP source domain."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(
            "domain must be sora-kusama, sora-polkadot, or sora2"
        )
    domain = SUBSTRATE_DOMAIN_ALIASES.get(value.lower())
    if domain is None:
        raise argparse.ArgumentTypeError(
            "domain must be sora-kusama, sora-polkadot, or sora2"
        )
    return domain


def _profile(domain: int) -> dict[str, str]:
    domain = _require_exact_u32(domain, "domain")
    if domain not in SUBSTRATE_SOURCE_PROFILES:
        raise ValueError("domain must be sora-kusama, sora-polkadot, or sora2")
    return SUBSTRATE_SOURCE_PROFILES[domain]


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


def _substrate_template_component_hash(
    domain: int,
    component_id: str,
    component_kind: str,
) -> bytes:
    profile = _profile(domain)
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, domain)
    _push_vec(payload, profile["chain"].encode("utf-8"))
    _push_u8(payload, SUBSTRATE_SOURCE_PROOF_PLAN_CODE)  # SubstrateGrandpaEventProof
    _push_u8(payload, SUBSTRATE_FINALITY_MODEL_CODE)  # SubstrateGrandpa
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(
        payload,
        SCCP_SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"),
    )
    _push_vec(
        payload,
        SCCP_SUBSTRATE_RUNTIME_STORAGE_FASTPQ_PARAMETER_SET.encode("utf-8"),
    )
    _push_vec(payload, SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND.encode("utf-8"))
    for prefix in SUBSTRATE_TEMPLATE_TRANSCRIPT_PREFIXES:
        _push_vec(payload, prefix)
    _push_vec(payload, component_kind.encode("utf-8"))
    _push_vec(payload, component_id.encode("utf-8"))
    return _prefixed_blake2b(
        b"sccp:substrate-family:source-verifier-material:v1",
        payload,
    )


def substrate_source_adapter_verifier_vk_hash(
    domain: int,
    *,
    target_domain: int = SCCP_DOMAIN_SORA,
) -> bytes:
    """Compute Rust's canonical OpenVerify vk hash for a Substrate -> SORA lane."""

    profile = _profile(domain)
    domain = _require_exact_u32(domain, "domain")
    target_domain = _require_exact_u32(target_domain, "target_domain")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")

    verifier = bytearray()
    _push_u8(verifier, 1)
    _push_vec(verifier, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(verifier, profile["chain"].encode("utf-8"))
    _push_u32(verifier, domain)
    _push_u32(verifier, target_domain)
    _push_u8(verifier, SUBSTRATE_SOURCE_PROOF_PLAN_CODE)
    _push_u8(verifier, SUBSTRATE_FINALITY_MODEL_CODE)
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


def substrate_source_verifier_material_record_hash(args: argparse.Namespace) -> bytes:
    """Compute Rust's canonical Substrate source verifier material record hash."""

    profile = _profile(args.domain)
    _reject_template_hashes(args)
    _require_source_role_hash_separation(args)
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, args.domain)
    _push_vec(payload, profile["chain"].encode("utf-8"))
    _push_u8(payload, SUBSTRATE_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, SUBSTRATE_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, profile["source_trust_anchor_id"].encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_trust_anchor_hash,
            label="source_trust_anchor_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, profile["consensus_verifier_id"].encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.consensus_verifier_hash,
            label="consensus_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, profile["message_inclusion_verifier_id"].encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.message_inclusion_verifier_hash,
            label="message_inclusion_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, profile["finality_policy_id"].encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.finality_policy_hash,
            label="finality_policy_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, profile["source_state_verifier_id"].encode("utf-8"))
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


def substrate_source_adapter_engine_deployment_record_hash(
    args: argparse.Namespace,
) -> bytes:
    """Compute Rust's canonical Substrate source-adapter deployment record hash."""

    domain = _require_exact_u32(args.domain, "domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    profile = _profile(domain)
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")
    _reject_template_hashes(args)
    _require_source_role_hash_separation(args)
    adapter_verifier_vk_hash = _require_nonzero_fixed_bytes(
        args.adapter_verifier_vk_hash,
        label="adapter_verifier_vk_hash",
        byte_length=32,
    )
    expected_adapter_verifier_vk_hash = substrate_source_adapter_verifier_vk_hash(
        domain,
        target_domain=target_domain,
    )
    if adapter_verifier_vk_hash != expected_adapter_verifier_vk_hash:
        raise ValueError(
            "adapter_verifier_vk_hash must match the canonical "
            f"{profile['chain']} source-adapter verifier profile"
        )

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, domain)
    _push_u32(payload, target_domain)
    _push_vec(payload, profile["chain"].encode("utf-8"))
    _push_u8(payload, SUBSTRATE_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, SUBSTRATE_FINALITY_MODEL_CODE)
    _push_vec(payload, SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8"))
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    payload.extend(adapter_verifier_vk_hash)
    _push_vec(payload, profile["source_trust_anchor_id"].encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_trust_anchor_hash,
            label="source_trust_anchor_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, profile["consensus_verifier_id"].encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.consensus_verifier_hash,
            label="consensus_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, profile["message_inclusion_verifier_id"].encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.message_inclusion_verifier_hash,
            label="message_inclusion_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, profile["finality_policy_id"].encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.finality_policy_hash,
            label="finality_policy_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, profile["source_state_verifier_id"].encode("utf-8"))
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
    return _prefixed_blake2b(
        b"sccp:source-adapter-engine-deployment:v1",
        bytes(payload),
    )


def substrate_runtime_storage_gate_hash(args: argparse.Namespace) -> bytes:
    """Compute Rust's Substrate runtime-storage source-adapter gate hash."""

    domain = _require_exact_u32(args.domain, "domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    profile = _profile(domain)
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")
    material_hash = substrate_source_verifier_material_record_hash(args)
    deployment_hash = substrate_source_adapter_engine_deployment_record_hash(args)

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, domain)
    _push_u32(payload, target_domain)
    _push_vec(payload, profile["chain"].encode("utf-8"))
    _push_u8(payload, SUBSTRATE_SOURCE_PROOF_PLAN_CODE)
    _push_u8(payload, SUBSTRATE_FINALITY_MODEL_CODE)
    _push_vec(
        payload,
        SCCP_SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"),
    )
    _push_vec(
        payload,
        SCCP_SUBSTRATE_RUNTIME_STORAGE_FASTPQ_PARAMETER_SET.encode("utf-8"),
    )
    _push_vec(payload, profile["source_state_verifier_id"].encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_state_verifier_hash,
            label="source_state_verifier_hash",
            byte_length=32,
        )
    )
    payload.extend(material_hash)
    payload.extend(deployment_hash)
    return _prefixed_blake2b(
        SCCP_SUBSTRATE_RUNTIME_STORAGE_GATE_PREFIX,
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


def _require_substrate_sora_lane(args: argparse.Namespace) -> None:
    target_domain = _require_exact_u32(args.target_domain, "target_domain", SystemExit)
    if target_domain != SCCP_DOMAIN_SORA:
        raise SystemExit("Substrate production source evidence requires target_domain = 0")


def _template_hash_fields() -> tuple[tuple[str, str, str], ...]:
    return (
        ("source_trust_anchor_hash", "source_trust_anchor_id", "source-trust-anchor"),
        ("consensus_verifier_hash", "consensus_verifier_id", "consensus-verifier"),
        (
            "message_inclusion_verifier_hash",
            "message_inclusion_verifier_id",
            "message-inclusion-verifier",
        ),
        (
            "source_state_verifier_hash",
            "source_state_verifier_id",
            "source-state-verifier",
        ),
        ("finality_policy_hash", "finality_policy_id", "finality-policy"),
    )


def _reject_template_hashes(args: argparse.Namespace) -> None:
    profile = _profile(args.domain)
    for field, id_key, component_kind in _template_hash_fields():
        if getattr(args, field) == _substrate_template_component_hash(
            args.domain,
            profile[id_key],
            component_kind,
        ):
            label = field.replace("_", " ")
            raise SystemExit(
                f"Substrate production source evidence requires live {label}; "
                f"template-derived {label} is not deployable"
            )


def _require_canonical_adapter_verifier_vk_hash(args: argparse.Namespace) -> None:
    expected_hash = substrate_source_adapter_verifier_vk_hash(
        args.domain,
        target_domain=args.target_domain,
    )
    if args.adapter_verifier_vk_hash != expected_hash:
        raise SystemExit(
            "--adapter-verifier-vk-hash does not match the canonical "
            f"{_profile(args.domain)['chain']} source-adapter verifier profile: "
            f"expected {_hex(expected_hash)}, got {_hex(args.adapter_verifier_vk_hash)}"
        )


def _require_expected_record_hashes(
    args: argparse.Namespace,
    *,
    output: str | None = None,
) -> None:
    profile = _profile(args.domain)
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
        material_hash = substrate_source_verifier_material_record_hash(args)
        if expected_material_hash != material_hash:
            raise SystemExit(
                "--expected-source-verifier-material-hash does not match the "
                f"canonical {profile['chain']} source verifier material record: "
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
        deployment_hash = substrate_source_adapter_engine_deployment_record_hash(args)
        if expected_deployment_hash != deployment_hash:
            raise SystemExit(
                "--expected-source-adapter-engine-deployment-hash does not match "
                f"the canonical {profile['chain']} source-adapter deployment record: "
                f"expected {_hex(expected_deployment_hash)}, got {_hex(deployment_hash)}"
            )


def _require_expected_runtime_storage_gate_hash(
    args: argparse.Namespace,
    *,
    output: str | None = None,
) -> None:
    expected_gate_hash = getattr(args, "expected_runtime_storage_gate_hash", None)
    if expected_gate_hash is None:
        if output is not None:
            raise SystemExit(
                f"--{output} requires --expected-runtime-storage-gate-hash"
            )
        return
    gate_hash = substrate_runtime_storage_gate_hash(args)
    if expected_gate_hash != gate_hash:
        prefix = f"--{output} " if output is not None else ""
        raise SystemExit(
            f"{prefix}--expected-runtime-storage-gate-hash does not match the "
            f"canonical {_profile(args.domain)['chain']} runtime-storage source gate: "
            f"expected {_hex(expected_gate_hash)}, got {_hex(gate_hash)}"
        )


def _validate_substrate_source_evidence_args(args: argparse.Namespace) -> None:
    _require_substrate_sora_lane(args)
    _reject_template_hashes(args)
    _require_canonical_adapter_verifier_vk_hash(args)
    _require_source_role_hash_separation(args)
    _require_expected_record_hashes(args)
    _require_expected_runtime_storage_gate_hash(args)


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
                "Substrate source-adapter role hashes must be distinct: "
                f"{field} matches {previous_field}"
            )
        seen[value] = field


def _material_lines(args: argparse.Namespace) -> Iterable[str]:
    profile = _profile(args.domain)
    yield "[[zk.sccp_source_verifier_materials]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.domain)
    yield _toml_line("source_chain", profile["chain"])
    yield _toml_line("source_proof_plan", "SubstrateGrandpaEventProof")
    yield _toml_line("finality_model", "SubstrateGrandpa")
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("source_trust_anchor_id", profile["source_trust_anchor_id"])
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", profile["consensus_verifier_id"])
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        profile["message_inclusion_verifier_id"],
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line("source_state_verifier_id", profile["source_state_verifier_id"])
    yield _toml_line(
        "source_state_verifier_hash",
        _hex(args.source_state_verifier_hash),
    )
    yield _toml_line("finality_policy_id", profile["finality_policy_id"])
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("placeholder_material", False)


def _deployment_lines(args: argparse.Namespace) -> Iterable[str]:
    profile = _profile(args.domain)
    yield "[[zk.sccp_source_adapter_engine_deployments]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.domain)
    yield _toml_line("target_domain", args.target_domain)
    yield _toml_line("source_chain", profile["chain"])
    yield _toml_line("source_proof_plan", "SubstrateGrandpaEventProof")
    yield _toml_line("finality_model", "SubstrateGrandpa")
    yield _toml_line("adapter_proof_family", SCCP_PROOF_FAMILY_STARK_FRI)
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("adapter_verifier_vk_hash", _hex(args.adapter_verifier_vk_hash))
    yield _toml_line("source_trust_anchor_id", profile["source_trust_anchor_id"])
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", profile["consensus_verifier_id"])
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        profile["message_inclusion_verifier_id"],
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line("source_state_verifier_id", profile["source_state_verifier_id"])
    yield _toml_line(
        "source_state_verifier_hash",
        _hex(args.source_state_verifier_hash),
    )
    yield _toml_line("finality_policy_id", profile["finality_policy_id"])
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("deployment_receipt_hash", _hex(args.deployment_receipt_hash))


def render_toml(args: argparse.Namespace) -> str:
    """Render source material and source adapter deployment TOML records."""

    _validate_substrate_source_evidence_args(args)
    material_hash = substrate_source_verifier_material_record_hash(args)
    deployment_hash = substrate_source_adapter_engine_deployment_record_hash(args)
    gate_hash = substrate_runtime_storage_gate_hash(args)
    _require_expected_record_hashes(args, output="toml")
    _require_expected_runtime_storage_gate_hash(args, output="toml")
    return "\n".join(
        [
            "# sccp_substrate_source_verifier_material_hash = "
            + json.dumps(_hex(material_hash)),
            *_material_lines(args),
            "",
            "# sccp_substrate_source_adapter_engine_deployment_hash = "
            + json.dumps(_hex(deployment_hash)),
            "# sccp_substrate_runtime_storage_gate_hash = "
            + json.dumps(_hex(gate_hash)),
            *_deployment_lines(args),
            "",
        ]
    )


def _json_summary(args: argparse.Namespace) -> dict[str, object]:
    _validate_substrate_source_evidence_args(args)
    profile = _profile(args.domain)
    material_hash = substrate_source_verifier_material_record_hash(args)
    deployment_hash = substrate_source_adapter_engine_deployment_record_hash(args)
    gate_hash = substrate_runtime_storage_gate_hash(args)
    expected_material_matches = (
        getattr(args, "expected_source_verifier_material_hash", None) == material_hash
    )
    expected_deployment_matches = (
        getattr(args, "expected_source_adapter_engine_deployment_hash", None)
        == deployment_hash
    )
    expected_gate_hash = getattr(args, "expected_runtime_storage_gate_hash", None)
    expected_gate_matches = expected_gate_hash == gate_hash
    return {
        "source_domain": args.domain,
        "target_domain": args.target_domain,
        "source_chain": profile["chain"],
        "source_proof_plan": "SubstrateGrandpaEventProof",
        "finality_model": "SubstrateGrandpa",
        "adapter_verifier_vk_hash": _hex(args.adapter_verifier_vk_hash),
        "source_state_verifier_id": profile["source_state_verifier_id"],
        "source_state_verifier_hash": _hex(args.source_state_verifier_hash),
        "deployment_receipt_hash": _hex(args.deployment_receipt_hash),
        "source_verifier_material_hash": _hex(material_hash),
        "source_adapter_engine_deployment_hash": _hex(deployment_hash),
        "substrate_runtime_storage_gate_hash": _hex(gate_hash),
        "expected_source_verifier_material_hash_matches": expected_material_matches,
        "expected_source_adapter_engine_deployment_hash_matches": (
            expected_deployment_matches
        ),
        "expected_runtime_storage_gate_hash_matches": expected_gate_matches,
        "toml_ready": (
            expected_material_matches
            and expected_deployment_matches
            and expected_gate_matches
        ),
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Render SCCP Substrate-family source deployment evidence.",
    )
    parser.add_argument(
        "--domain",
        required=True,
        type=parse_substrate_domain,
        help="Source domain: sora-kusama, sora-polkadot, or sora2.",
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
            help="Non-zero bytes32 Substrate-family deployment evidence.",
        )
    parser.add_argument(
        "--expected-source-verifier-material-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected source verifier material hash",
            byte_length=32,
        ),
        help=(
            "Optional governed Substrate-family source verifier material record "
            "hash. Mismatches fail instead of rendering evidence."
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
            "Optional governed Substrate-family source-adapter deployment record "
            "hash. Mismatches fail instead of rendering evidence."
        ),
    )
    parser.add_argument(
        "--expected-runtime-storage-gate-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected runtime storage gate hash",
            byte_length=32,
        ),
        help=(
            "Optional governed Substrate-family runtime-storage source-adapter "
            "gate hash. Mismatches fail instead of rendering evidence."
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
        _validate_substrate_source_evidence_args(args)
        if args.toml:
            print(render_toml(args), end="")
        else:
            print(json.dumps(_json_summary(args), sort_keys=True, indent=2))
    except SystemExit as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
