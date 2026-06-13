#!/usr/bin/env python3
"""Render SCCP Solana source-state verifier deployment evidence.

This helper is offline by design. Operators pass the governed Solana
mainnet-beta source trust anchor, verifier component hashes, AccountsDB
source-state verifier hash, adapter verifier key hash, and deployment receipt
hash collected from governance or deployment records. The script validates that
production evidence hashes are non-zero and not the built-in template hashes,
then renders the matching `zk.sccp_source_verifier_materials` and
`zk.sccp_source_adapter_engine_deployments` TOML records for the Solana -> SORA
source lane. Optional Tower replay, full AccountsDB lattice, and bank/fork-choice
verifier hashes are emitted as an audit bundle and, when complete, are bound
into the canonical source-adapter deployment hash. Rendering evidence does not
open the Solana production gate by itself; the runtime still requires the
governed full Solana light-client stack.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from typing import Iterable


SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_SOL = 3
SCCP_PROOF_FAMILY_STARK_FRI = "stark-fri-v1"
SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID = "sccp-source-adapter-v1"
SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET = "fastpq-lane-balanced"
FASTPQ_BALANCED_TRACE_ROOT = 0x002A_247F_81C6_F850
FASTPQ_BALANCED_LDE_ROOT = 0x6026_3388_DBBF_9B2A
FASTPQ_BALANCED_OMEGA_COSET = 0x6AF3_25E8_25AD_5C18

SOLANA_SOURCE_TRUST_ANCHOR_ID = (
    "sccp:sol:source-trust-anchor:solana-mainnet-beta-genesis:v1"
)
SOLANA_CONSENSUS_VERIFIER_ID = (
    "sccp:sol:consensus-verifier:finalized-slot-bankhash-mainnet-beta:v1"
)
SOLANA_MESSAGE_INCLUSION_VERIFIER_ID = (
    "sccp:sol:message-inclusion-verifier:transaction-status-root-branch:v1"
)
SOLANA_SOURCE_STATE_VERIFIER_ID = (
    "sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1"
)
SOLANA_FINALITY_POLICY_ID = "sccp:sol:finality-policy:finalized-slot-mainnet-beta:v1"
SOLANA_TOWER_REPLAY_VERIFIER_ID = (
    "sccp:sol:light-client:tower-replay-mainnet-beta:v1"
)
SOLANA_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID = (
    "sccp:sol:light-client:full-accountsdb-lattice-mainnet-beta:v1"
)
SOLANA_BANK_FORK_CHOICE_VERIFIER_ID = (
    "sccp:sol:light-client:bank-fork-choice-mainnet-beta:v1"
)

SOLANA_MAINNET_GENESIS_HASH = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"
SOLANA_MAINNET_SLOTS_PER_EPOCH = 432_000
SOLANA_TOWER_LOCKOUT_CONFIRMATION_DEPTH = 32
SOLANA_TOWER_WARMUP_COOLDOWN_RATE_BPS = 900
SOLANA_BASIS_POINTS_PER_UNIT = 10_000
SOLANA_TRANSACTION_SIGNATURE_BYTES = 64
SOLANA_PROGRAM_ID_BYTES = 32

SOLANA_VOTE_PROGRAM_ID = bytes.fromhex(
    "0761481d357474bb7c4d7624ebd3bdb3d8355e73d11043fc0da3538000000000"
)
SOLANA_STAKE_PROGRAM_ID = bytes.fromhex(
    "06a1d8179137542a983437bdfe2a7ab2557f535c8a78722b68a49dc000000000"
)
SOLANA_SYSVAR_PROGRAM_ID = bytes.fromhex(
    "06a7d5171875f729c73d93408f216120067ed88c76e08c287fc1946000000000"
)
SOLANA_STAKE_HISTORY_SYSVAR_ID = bytes.fromhex(
    "06a7d517193584d0feed9bb3431d13206be544281b57b8566cc5375ff4000000"
)

SOURCE_PROOF_PLAN_SOLANA_FINALIZED_TRANSACTION = 3
FINALITY_MODEL_SOLANA_FINALIZED_SLOT = 3

SOLANA_COMPONENT_HASH_PREFIX = b"sccp:solana:source-verifier-material:v1"
SOURCE_EVENT_LEAF_PREFIX = b"sccp:source:event-leaf:v1"
SOURCE_NODE_PREFIX = b"sccp:source:node:v1"
SOLANA_MESSAGE_PROOF_PREFIX = b"sccp:solana:message-proof:v1"
SOLANA_TRANSACTION_STATUS_LEAF_PREFIX = b"sccp:solana:transaction-status-leaf:v1"
SOLANA_FINALITY_CONTEXT_PREFIX = b"sccp:solana:finality-context:v1"
SOLANA_EPOCH_STAKE_ROOT_PREFIX = b"sccp:solana:epoch-stake-root:v1"
SOLANA_STAKE_ACTIVATION_PREFIX = b"sccp:solana:stake-activation:v1"
SOLANA_STAKE_ACCOUNT_STATE_PREFIX = b"sccp:solana:stake-account-state:v1"
SOLANA_ACCOUNT_OPENING_PREFIX = b"sccp:solana:account-opening:v1"
SOLANA_ACCOUNT_RAW_DATA_PREFIX = b"sccp:solana:account-raw-data:v1"
SOLANA_ACCOUNT_INCLUSION_LEAF_PREFIX = b"sccp:solana:account-inclusion-leaf:v1"
SOLANA_ACCOUNT_INCLUSION_NODE_PREFIX = b"sccp:solana:account-inclusion-node:v1"
SOLANA_VOTE_ACCOUNT_DATA_PREFIX = b"sccp:solana:vote-account-data:v1"
SOLANA_STAKE_ACCOUNT_DATA_PREFIX = b"sccp:solana:stake-account-data:v1"
SOLANA_STAKE_HISTORY_SYSVAR_DATA_PREFIX = (
    b"sccp:solana:stake-history-sysvar-data:v1"
)
SOLANA_STAKE_HISTORY_PREFIX = b"sccp:solana:stake-history:v1"
SOLANA_ACCOUNTS_LT_HASH_PROOF_PUBLIC_INPUTS_PREFIX = (
    b"sccp:solana:accounts-lt-proof-public-inputs:v1"
)
SOLANA_ACCOUNTS_LT_HASH_OPENED_CONTRIBUTIONS_PREFIX = (
    b"sccp:solana:accounts-lt-opened-contributions:v1"
)
SOLANA_ACCOUNTS_LT_HASH_FASTPQ_DSID_PREFIX = (
    b"sccp:solana:accounts-lt:fastpq:dsid:v1"
)
SOLANA_ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET = "fastpq-lane-balanced"
SOLANA_ACCOUNTS_LT_HASH_FASTPQ_STATEMENT_KEY = b"sccp:solana:accounts-lt:v1:statement"
SOLANA_ACCOUNTS_LT_HASH_FASTPQ_ACCOUNTS_KEY = b"sccp:solana:accounts-lt:v1:accounts"
SOLANA_ACCOUNTS_LT_HASH_FASTPQ_OPENED_CONTRIBUTIONS_KEY = (
    b"sccp:solana:accounts-lt:v1:opened-contributions"
)
SOLANA_ACCOUNTS_LT_HASH_FASTPQ_RESIDUAL_KEY = b"sccp:solana:accounts-lt:v1:residual"
SOLANA_ACCOUNTS_LT_HASH_FASTPQ_CONTEXT_KEY = b"sccp:solana:accounts-lt:v1:context"
SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID = "sccp-solana-accounts-lt-hash-v1"
SOLANA_TOWER_LOCKOUT_PREFIX = b"sccp:solana:tower-lockout:v1"
SOLANA_TOWER_REPLAY_PREFIX = b"sccp:solana:tower-replay:v1"
SOLANA_BANK_FORK_PREFIX = b"sccp:solana:bank-fork:v1"
SOLANA_VOTE_ROSTER_PREFIX = b"sccp:solana:vote-roster:v1"
SOLANA_FINALIZED_VOTE_PREFIX = b"sccp:solana:finalized-vote:v1"
SOLANA_FULL_LIGHT_CLIENT_GATE_PREFIX = b"sccp:solana:full-light-client-gate:v1"


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
    except (TypeError, ValueError):
        raise argparse.ArgumentTypeError(f"{label} must be hex") from None
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


def _require_nonzero_fixed_bytes(value: bytes, *, label: str, byte_length: int) -> bytes:
    raw = _require_fixed_bytes(value, label=label, byte_length=byte_length)
    if not any(raw):
        raise ValueError(f"{label} must not be zero")
    return raw


def solana_template_component_hash(component_id: str, component_kind: str) -> bytes:
    """Return the built-in Solana mainnet source-material template hash."""

    out = bytearray()
    _push_u8(out, 1)
    _push_u32(out, SCCP_DOMAIN_SOL)
    _push_vec(out, b"sol")
    _push_u8(out, SOURCE_PROOF_PLAN_SOLANA_FINALIZED_TRANSACTION)
    _push_u8(out, FINALITY_MODEL_SOLANA_FINALIZED_SLOT)
    _push_vec(out, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode())
    _push_vec(out, b"sccp-solana-recursive-mainnet-v1")
    _push_vec(out, SOLANA_MAINNET_GENESIS_HASH.encode())
    _push_u64(out, SOLANA_MAINNET_SLOTS_PER_EPOCH)
    _push_u64(out, SOLANA_TOWER_LOCKOUT_CONFIRMATION_DEPTH)
    _push_u64(out, SOLANA_TOWER_WARMUP_COOLDOWN_RATE_BPS)
    _push_u64(out, SOLANA_BASIS_POINTS_PER_UNIT)
    _push_vec(out, SOLANA_MESSAGE_PROOF_PREFIX)
    _push_vec(out, SOLANA_TRANSACTION_STATUS_LEAF_PREFIX)
    _push_u64(out, SOLANA_TRANSACTION_SIGNATURE_BYTES)
    _push_u64(out, SOLANA_PROGRAM_ID_BYTES)
    _push_vec(out, SOURCE_EVENT_LEAF_PREFIX)
    _push_vec(out, SOURCE_NODE_PREFIX)
    _push_vec(out, SOLANA_FINALITY_CONTEXT_PREFIX)
    _push_vec(out, SOLANA_EPOCH_STAKE_ROOT_PREFIX)
    _push_vec(out, SOLANA_STAKE_ACTIVATION_PREFIX)
    _push_vec(out, SOLANA_STAKE_ACCOUNT_STATE_PREFIX)
    _push_vec(out, SOLANA_ACCOUNT_OPENING_PREFIX)
    _push_vec(out, SOLANA_ACCOUNT_RAW_DATA_PREFIX)
    _push_vec(out, SOLANA_ACCOUNT_INCLUSION_LEAF_PREFIX)
    _push_vec(out, SOLANA_ACCOUNT_INCLUSION_NODE_PREFIX)
    _push_vec(out, SOLANA_VOTE_ACCOUNT_DATA_PREFIX)
    _push_vec(out, SOLANA_STAKE_ACCOUNT_DATA_PREFIX)
    out.extend(SOLANA_VOTE_PROGRAM_ID)
    out.extend(SOLANA_STAKE_PROGRAM_ID)
    out.extend(SOLANA_SYSVAR_PROGRAM_ID)
    out.extend(SOLANA_STAKE_HISTORY_SYSVAR_ID)
    _push_vec(out, SOLANA_STAKE_HISTORY_SYSVAR_DATA_PREFIX)
    _push_vec(out, SOLANA_STAKE_HISTORY_PREFIX)
    _push_vec(out, SOLANA_ACCOUNTS_LT_HASH_PROOF_PUBLIC_INPUTS_PREFIX)
    _push_vec(out, SOLANA_ACCOUNTS_LT_HASH_OPENED_CONTRIBUTIONS_PREFIX)
    _push_vec(out, SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID.encode())
    _push_vec(out, SOLANA_ACCOUNTS_LT_HASH_FASTPQ_PARAMETER_SET.encode())
    _push_vec(out, SOLANA_ACCOUNTS_LT_HASH_FASTPQ_DSID_PREFIX)
    _push_vec(out, SOLANA_ACCOUNTS_LT_HASH_FASTPQ_STATEMENT_KEY)
    _push_vec(out, SOLANA_ACCOUNTS_LT_HASH_FASTPQ_ACCOUNTS_KEY)
    _push_vec(out, SOLANA_ACCOUNTS_LT_HASH_FASTPQ_OPENED_CONTRIBUTIONS_KEY)
    _push_vec(out, SOLANA_ACCOUNTS_LT_HASH_FASTPQ_RESIDUAL_KEY)
    _push_vec(out, SOLANA_ACCOUNTS_LT_HASH_FASTPQ_CONTEXT_KEY)
    _push_vec(out, SOLANA_TOWER_LOCKOUT_PREFIX)
    _push_vec(out, SOLANA_TOWER_REPLAY_PREFIX)
    _push_vec(out, SOLANA_BANK_FORK_PREFIX)
    _push_vec(out, SOLANA_VOTE_ROSTER_PREFIX)
    _push_vec(out, SOLANA_FINALIZED_VOTE_PREFIX)
    _push_vec(out, component_kind.encode())
    _push_vec(out, component_id.encode())
    return _prefixed_blake2b(SOLANA_COMPONENT_HASH_PREFIX, bytes(out))


def solana_source_adapter_verifier_vk_hash(
    *,
    source_domain: int = SCCP_DOMAIN_SOL,
    target_domain: int = SCCP_DOMAIN_SORA,
) -> bytes:
    """Compute Rust's canonical OpenVerify vk hash for Solana -> SORA."""

    if source_domain != SCCP_DOMAIN_SOL:
        raise ValueError("source_domain must be Solana")
    if target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")

    verifier = bytearray()
    _push_u8(verifier, 1)
    _push_vec(verifier, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(verifier, b"sol")
    _push_u32(verifier, source_domain)
    _push_u32(verifier, target_domain)
    _push_u8(verifier, SOURCE_PROOF_PLAN_SOLANA_FINALIZED_TRANSACTION)
    _push_u8(verifier, FINALITY_MODEL_SOLANA_FINALIZED_SLOT)
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


def solana_source_verifier_material_record_hash(args: argparse.Namespace) -> bytes:
    """Compute Rust's canonical Solana source verifier material record hash."""

    if args.source_domain != SCCP_DOMAIN_SOL:
        raise ValueError("source_domain must be Solana")
    _reject_template_hashes(args)
    _require_source_role_hash_separation(args)
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, args.source_domain)
    _push_vec(payload, b"sol")
    _push_u8(payload, SOURCE_PROOF_PLAN_SOLANA_FINALIZED_TRANSACTION)
    _push_u8(payload, FINALITY_MODEL_SOLANA_FINALIZED_SLOT)
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    _push_vec(payload, SOLANA_SOURCE_TRUST_ANCHOR_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_trust_anchor_hash,
            label="source_trust_anchor_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, SOLANA_CONSENSUS_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.consensus_verifier_hash,
            label="consensus_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, SOLANA_MESSAGE_INCLUSION_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.message_inclusion_verifier_hash,
            label="message_inclusion_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, SOLANA_FINALITY_POLICY_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.finality_policy_hash,
            label="finality_policy_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, SOLANA_SOURCE_STATE_VERIFIER_ID.encode("utf-8"))
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


def solana_source_adapter_engine_deployment_record_hash(
    args: argparse.Namespace,
) -> bytes:
    """Compute Rust's canonical Solana source-adapter deployment record hash."""

    if args.source_domain != SCCP_DOMAIN_SOL:
        raise ValueError("source_domain must be Solana")
    if args.target_domain != SCCP_DOMAIN_SORA:
        raise ValueError("target_domain must be SORA")
    _reject_template_hashes(args)
    _require_source_role_hash_separation(args)
    adapter_verifier_vk_hash = _require_nonzero_fixed_bytes(
        args.adapter_verifier_vk_hash,
        label="adapter_verifier_vk_hash",
        byte_length=32,
    )
    expected_adapter_verifier_vk_hash = solana_source_adapter_verifier_vk_hash(
        source_domain=args.source_domain,
        target_domain=args.target_domain,
    )
    if adapter_verifier_vk_hash != expected_adapter_verifier_vk_hash:
        raise ValueError(
            "adapter_verifier_vk_hash must match the canonical "
            "Solana source-adapter verifier profile"
        )

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, args.source_domain)
    _push_u32(payload, args.target_domain)
    _push_vec(payload, b"sol")
    _push_u8(payload, SOURCE_PROOF_PLAN_SOLANA_FINALIZED_TRANSACTION)
    _push_u8(payload, FINALITY_MODEL_SOLANA_FINALIZED_SLOT)
    _push_vec(payload, SCCP_PROOF_FAMILY_STARK_FRI.encode("utf-8"))
    _push_vec(payload, SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID.encode("utf-8"))
    payload.extend(adapter_verifier_vk_hash)
    _push_vec(payload, SOLANA_SOURCE_TRUST_ANCHOR_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.source_trust_anchor_hash,
            label="source_trust_anchor_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, SOLANA_CONSENSUS_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.consensus_verifier_hash,
            label="consensus_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, SOLANA_MESSAGE_INCLUSION_VERIFIER_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.message_inclusion_verifier_hash,
            label="message_inclusion_verifier_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, SOLANA_FINALITY_POLICY_ID.encode("utf-8"))
    payload.extend(
        _require_nonzero_fixed_bytes(
            args.finality_policy_hash,
            label="finality_policy_hash",
            byte_length=32,
        )
    )
    _push_vec(payload, SOLANA_SOURCE_STATE_VERIFIER_ID.encode("utf-8"))
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
        _push_u8(payload, 1)
        for field, engine_id in _light_client_evidence_fields():
            _push_vec(payload, engine_id.encode("utf-8"))
            payload.extend(supplied_light_client_hashes[field])
    return _prefixed_blake2b(
        b"sccp:source-adapter-engine-deployment:v1",
        bytes(payload),
    )


def _light_client_evidence_fields() -> tuple[tuple[str, str], ...]:
    return (
        ("tower_replay_verifier_hash", SOLANA_TOWER_REPLAY_VERIFIER_ID),
        (
            "full_accountsdb_lattice_verifier_hash",
            SOLANA_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID,
        ),
        ("bank_fork_choice_verifier_hash", SOLANA_BANK_FORK_CHOICE_VERIFIER_ID),
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
            "Solana full light-client evidence must include all verifier hashes: "
            + ", ".join(missing)
        )
    if hashes:
        _require_light_client_evidence_role_separation(args, hashes)
    return hashes


def solana_full_light_client_gate_hash(args: argparse.Namespace) -> bytes | None:
    """Compute the audit hash for the remaining full Solana light-client stack."""

    hashes = _require_complete_light_client_evidence_hashes(args)
    if len(hashes) != len(_light_client_evidence_fields()):
        return None

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, args.source_domain)
    _push_u32(payload, args.target_domain)
    _push_vec(payload, b"sol")
    _push_u8(payload, SOURCE_PROOF_PLAN_SOLANA_FINALIZED_TRANSACTION)
    _push_u8(payload, FINALITY_MODEL_SOLANA_FINALIZED_SLOT)
    _push_vec(payload, SOLANA_MAINNET_GENESIS_HASH.encode("utf-8"))
    payload.extend(solana_source_verifier_material_record_hash(args))
    payload.extend(solana_source_adapter_engine_deployment_record_hash(args))
    for field, engine_id in _light_client_evidence_fields():
        _push_vec(payload, engine_id.encode("utf-8"))
        payload.extend(hashes[field])
    return _prefixed_blake2b(SOLANA_FULL_LIGHT_CLIENT_GATE_PREFIX, bytes(payload))


def _hex(value: bytes) -> str:
    return "0x" + value.hex()


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


def _require_exact_u32(value: object, label: str) -> int:
    if type(value) is not int or value < 0 or value > 0xFFFFFFFF:
        raise SystemExit(f"{label} must be an exact u32")
    return value


def _require_solana_sora_lane(args: argparse.Namespace) -> None:
    source_domain = _require_exact_u32(args.source_domain, "source_domain")
    target_domain = _require_exact_u32(args.target_domain, "target_domain")
    if source_domain != SCCP_DOMAIN_SOL:
        raise SystemExit("Solana production source evidence requires source_domain = 3")
    if target_domain != SCCP_DOMAIN_SORA:
        raise SystemExit("Solana production source evidence requires target_domain = 0")


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
                "Solana source-adapter role hashes must be distinct: "
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
                "Solana full-light-client verifier hashes must be role-separated: "
                f"{field} matches {previous_field}"
            )
        seen[value] = field

    for field, value in hashes.items():
        for role_field in _component_hash_args():
            role_value = getattr(args, role_field, None)
            if role_value is not None and value == role_value:
                raise SystemExit(
                    "Solana full-light-client verifier hashes must not reuse "
                    f"existing source-adapter material: {field} matches {role_field}"
                )
        for template_field, template_hash in _template_component_hashes().items():
            if value == template_hash:
                raise SystemExit(
                    "Solana full-light-client verifier hashes must not reuse "
                    f"built-in template material: {field} matches {template_field}"
                )


def _template_hash_fields() -> tuple[tuple[str, str, str], ...]:
    return (
        ("source_trust_anchor_hash", SOLANA_SOURCE_TRUST_ANCHOR_ID, "source-trust-anchor"),
        ("consensus_verifier_hash", SOLANA_CONSENSUS_VERIFIER_ID, "consensus-verifier"),
        (
            "message_inclusion_verifier_hash",
            SOLANA_MESSAGE_INCLUSION_VERIFIER_ID,
            "message-inclusion-verifier",
        ),
        (
            "source_state_verifier_hash",
            SOLANA_SOURCE_STATE_VERIFIER_ID,
            "source-state-verifier",
        ),
        ("finality_policy_hash", SOLANA_FINALITY_POLICY_ID, "finality-policy"),
    )


def _template_component_hashes() -> dict[str, bytes]:
    return {
        field: solana_template_component_hash(component_id, component_kind)
        for field, component_id, component_kind in _template_hash_fields()
    }


def _reject_template_hashes(args: argparse.Namespace) -> None:
    for field, template_hash in _template_component_hashes().items():
        if getattr(args, field) == template_hash:
            raise SystemExit(f"{field} must be deployed evidence, not the Solana template hash")


def _require_canonical_adapter_verifier_vk_hash(args: argparse.Namespace) -> None:
    expected_hash = solana_source_adapter_verifier_vk_hash(
        source_domain=args.source_domain,
        target_domain=args.target_domain,
    )
    if args.adapter_verifier_vk_hash != expected_hash:
        raise SystemExit(
            "--adapter-verifier-vk-hash does not match the canonical "
            "Solana source-adapter verifier profile: "
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
        material_hash = solana_source_verifier_material_record_hash(args)
        if expected_material_hash != material_hash:
            raise SystemExit(
                "--expected-source-verifier-material-hash does not match the "
                "canonical Solana source verifier material record: "
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
        deployment_hash = solana_source_adapter_engine_deployment_record_hash(args)
        if expected_deployment_hash != deployment_hash:
            raise SystemExit(
                "--expected-source-adapter-engine-deployment-hash does not match "
                "the canonical Solana source-adapter deployment record: "
                f"expected {_hex(expected_deployment_hash)}, got {_hex(deployment_hash)}"
            )


def _require_full_light_client_evidence_consistency(
    args: argparse.Namespace,
    *,
    output: str | None = None,
) -> None:
    supplied = _require_complete_light_client_evidence_hashes(args)
    expected_gate_hash = getattr(args, "expected_full_light_client_gate_hash", None)
    gate_hash = solana_full_light_client_gate_hash(args)
    if output is not None and gate_hash is None:
        raise SystemExit(
            f"--{output} requires the Tower replay, full AccountsDB lattice, "
            "bank/fork-choice verifier hashes, and "
            "--expected-full-light-client-gate-hash"
        )
    if expected_gate_hash is not None and gate_hash is None:
        raise SystemExit(
            "--expected-full-light-client-gate-hash requires the Tower replay, "
            "full AccountsDB lattice, and bank/fork-choice verifier hashes"
        )
    if output is not None and supplied and expected_gate_hash is None:
        raise SystemExit(
            f"--{output} with Solana full light-client audit evidence requires "
            "--expected-full-light-client-gate-hash"
        )
    if expected_gate_hash is not None and expected_gate_hash != gate_hash:
        raise SystemExit(
            "--expected-full-light-client-gate-hash does not match the Solana "
            "full light-client audit record: "
            f"expected {_hex(expected_gate_hash)}, got {_hex(gate_hash)}"
        )


def _validate_solana_evidence(args: argparse.Namespace) -> None:
    _require_solana_sora_lane(args)
    _reject_template_hashes(args)
    _require_canonical_adapter_verifier_vk_hash(args)
    _require_source_role_hash_separation(args)
    _require_expected_record_hashes(args)
    _require_full_light_client_evidence_consistency(args)


def _material_lines(args: argparse.Namespace) -> Iterable[str]:
    yield "[[zk.sccp_source_verifier_materials]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.source_domain)
    yield _toml_line("source_chain", "sol")
    yield _toml_line("source_proof_plan", "SolanaFinalizedTransactionProof")
    yield _toml_line("finality_model", "SolanaFinalizedSlot")
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("source_trust_anchor_id", SOLANA_SOURCE_TRUST_ANCHOR_ID)
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", SOLANA_CONSENSUS_VERIFIER_ID)
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        SOLANA_MESSAGE_INCLUSION_VERIFIER_ID,
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line("source_state_verifier_id", SOLANA_SOURCE_STATE_VERIFIER_ID)
    yield _toml_line("source_state_verifier_hash", _hex(args.source_state_verifier_hash))
    yield _toml_line("finality_policy_id", SOLANA_FINALITY_POLICY_ID)
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("placeholder_material", False)


def _deployment_lines(args: argparse.Namespace) -> Iterable[str]:
    yield "[[zk.sccp_source_adapter_engine_deployments]]"
    yield _toml_line("version", 1)
    yield _toml_line("source_domain", args.source_domain)
    yield _toml_line("target_domain", args.target_domain)
    yield _toml_line("source_chain", "sol")
    yield _toml_line("source_proof_plan", "SolanaFinalizedTransactionProof")
    yield _toml_line("finality_model", "SolanaFinalizedSlot")
    yield _toml_line("adapter_proof_family", SCCP_PROOF_FAMILY_STARK_FRI)
    yield _toml_line("adapter_circuit_id", "sccp-source-adapter-v1")
    yield _toml_line("adapter_verifier_vk_hash", _hex(args.adapter_verifier_vk_hash))
    yield _toml_line("source_trust_anchor_id", SOLANA_SOURCE_TRUST_ANCHOR_ID)
    yield _toml_line("source_trust_anchor_hash", _hex(args.source_trust_anchor_hash))
    yield _toml_line("consensus_verifier_id", SOLANA_CONSENSUS_VERIFIER_ID)
    yield _toml_line("consensus_verifier_hash", _hex(args.consensus_verifier_hash))
    yield _toml_line(
        "message_inclusion_verifier_id",
        SOLANA_MESSAGE_INCLUSION_VERIFIER_ID,
    )
    yield _toml_line(
        "message_inclusion_verifier_hash",
        _hex(args.message_inclusion_verifier_hash),
    )
    yield _toml_line("source_state_verifier_id", SOLANA_SOURCE_STATE_VERIFIER_ID)
    yield _toml_line("source_state_verifier_hash", _hex(args.source_state_verifier_hash))
    yield _toml_line("finality_policy_id", SOLANA_FINALITY_POLICY_ID)
    yield _toml_line("finality_policy_hash", _hex(args.finality_policy_hash))
    yield _toml_line("deployment_receipt_hash", _hex(args.deployment_receipt_hash))
    supplied = _light_client_evidence_hashes(args)
    gate_hash = solana_full_light_client_gate_hash(args)
    if gate_hash is not None:
        yield _toml_line(
            "solana_tower_replay_verifier_hash",
            _hex(supplied["tower_replay_verifier_hash"]),
        )
        yield _toml_line(
            "solana_full_accountsdb_lattice_verifier_hash",
            _hex(supplied["full_accountsdb_lattice_verifier_hash"]),
        )
        yield _toml_line(
            "solana_bank_fork_choice_verifier_hash",
            _hex(supplied["bank_fork_choice_verifier_hash"]),
        )
        yield _toml_line("solana_full_light_client_gate_hash", _hex(gate_hash))


def _full_light_client_audit_lines(args: argparse.Namespace) -> Iterable[str]:
    supplied = _light_client_evidence_hashes(args)
    gate_hash = solana_full_light_client_gate_hash(args)
    yield "# Solana full light-client audit evidence."
    yield "# source_adapter_gate_closed_until_full_light_client = true"
    yield "# Runtime production admission remains fail-closed until this audit"
    yield "# record is consumed by a governed Solana light-client readiness predicate."
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
    yield "# solana_full_light_client_gate_hash = " + json.dumps(_hex(gate_hash))
    for field, engine_id in _light_client_evidence_fields():
        yield f"# {field.removesuffix('_hash')}_id = " + json.dumps(engine_id)
        yield f"# {field} = " + json.dumps(_hex(supplied[field]))


def render_toml(args: argparse.Namespace) -> str:
    """Render source material and source adapter deployment TOML records."""

    _validate_solana_evidence(args)
    material_hash = solana_source_verifier_material_record_hash(args)
    deployment_hash = solana_source_adapter_engine_deployment_record_hash(args)
    _require_expected_record_hashes(args, output="toml")
    _require_full_light_client_evidence_consistency(args, output="toml")
    return "\n".join(
        [
            "# sccp_solana_source_verifier_material_hash = "
            + json.dumps(_hex(material_hash)),
            *_material_lines(args),
            "",
            "# sccp_solana_source_adapter_engine_deployment_hash = "
            + json.dumps(_hex(deployment_hash)),
            *_deployment_lines(args),
            "",
            *_full_light_client_audit_lines(args),
            "",
        ]
    )


def _json_summary(args: argparse.Namespace) -> dict[str, object]:
    _validate_solana_evidence(args)
    material_hash = solana_source_verifier_material_record_hash(args)
    deployment_hash = solana_source_adapter_engine_deployment_record_hash(args)
    expected_material_matches = (
        getattr(args, "expected_source_verifier_material_hash", None) == material_hash
    )
    expected_deployment_matches = (
        getattr(args, "expected_source_adapter_engine_deployment_hash", None)
        == deployment_hash
    )
    supplied = _light_client_evidence_hashes(args)
    gate_hash = solana_full_light_client_gate_hash(args)
    expected_gate_hash = getattr(args, "expected_full_light_client_gate_hash", None)
    full_light_client_evidence_ready = gate_hash is not None
    expected_gate_matches = gate_hash is not None and expected_gate_hash == gate_hash
    light_client_hashes = {
        field: _hex(supplied[field])
        for field, _engine_id in _light_client_evidence_fields()
        if field in supplied
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
        "source_chain": "sol",
        "source_proof_plan": "SolanaFinalizedTransactionProof",
        "finality_model": "SolanaFinalizedSlot",
        "source_state_verifier_id": SOLANA_SOURCE_STATE_VERIFIER_ID,
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
        description="Render SCCP Solana source-state deployment evidence.",
    )
    parser.add_argument(
        "--source-domain",
        default=SCCP_DOMAIN_SOL,
        type=lambda value: parse_u32(value, label="source domain"),
        help="SCCP source domain. Defaults to Solana (3).",
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
            help="Non-zero bytes32 Solana deployment evidence.",
        )
    parser.add_argument(
        "--expected-source-verifier-material-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected source verifier material hash",
            byte_length=32,
        ),
        help=(
            "Optional governed Solana source verifier material record hash. "
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
            "Optional governed Solana source-adapter deployment record hash. "
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
                f"{engine_id}. All Solana full light-client verifier hashes "
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
            "Optional expected audit hash for the remaining Solana Tower replay, "
            "full AccountsDB lattice, and bank/fork-choice verifier evidence."
        ),
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
        _validate_solana_evidence(args)
        if args.toml:
            print(render_toml(args), end="")
        else:
            print(json.dumps(_json_summary(args), sort_keys=True, indent=2))
    except (OSError, SystemExit, RuntimeError, TypeError, ValueError) as exc:
        detail = _cli_error_detail(
            exc,
            fallback="SCCP Solana source-state evidence rendering failed",
        )
        parser.exit(2, f"{parser.prog}: error: {detail}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
