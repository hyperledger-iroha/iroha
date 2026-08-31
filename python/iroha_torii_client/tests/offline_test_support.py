"""Canonical fixtures shared by the low-level Torii offline client tests."""

from __future__ import annotations

import copy
import hashlib
from typing import Any, Dict, List, Mapping, Optional

from iroha_torii_client import KagemushaRedeemRequestV4, KagemushaTopUpRequestV4
from iroha_torii_client.norito_frame import _crc64_xz, schema_hash_for_type_name

CANONICAL_OWNER = "sorauﾛ1PｺfMﾇﾘｾﾄoﾂﾊﾔH7ZdﾘhﾚmAｸdnｳu1ｱﾄ1ｺﾋuSﾑﾀﾇﾐuHEB5DP"
CANONICAL_ASSET_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
CANONICAL_BALANCE_ASSET_ID = f"{CANONICAL_ASSET_ID}#{CANONICAL_OWNER}"
_NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT = (
    "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F"
)


def _hash_literal(raw: bytes) -> str:
    body_bytes = bytearray(raw)
    body_bytes[-1] |= 1
    body = body_bytes.hex().upper()
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return f"hash:{body}#{crc:04X}"


def _canonical_hash(seed: int) -> str:
    body_bytes = bytearray([seed & 0xFF] * 32)
    return _hash_literal(body_bytes)


def _iroha_hash(*chunks: bytes) -> bytes:
    digest = bytearray(hashlib.blake2b(b"".join(chunks), digest_size=32).digest())
    digest[-1] |= 1
    return bytes(digest)


def offline_capability_payload(**overrides: Any) -> Dict[str, Any]:
    """Build one closed universal offline-capability response fixture."""

    payload = {
        "cash_handoff_capability": "cash_handoff_v1",
        "required_bridge_abi_version": 23,
        "max_hops": 8,
        "ready": True,
    }
    payload.update(overrides)
    return payload


OFFLINE_OPERATION_BYTES = [0x11] * 32
OFFLINE_OPERATION_ID = "11" * 32
OFFLINE_TRANSACTION_HASH = "23" * 32
OFFLINE_STATUS_URI = f"/v1/offline/operations/{OFFLINE_OPERATION_ID}"
OFFLINE_NETWORK_ID = _canonical_hash(0x91)
OFFLINE_OTHER_NETWORK_ID = _canonical_hash(0x93)
OFFLINE_TOP_UP_REQUEST_SCHEMA_NAME = "iroha.torii.v1.offline.top_up.request"
OFFLINE_REDEEM_REQUEST_SCHEMA_NAME = "iroha.torii.v1.offline.redeem.request"
OFFLINE_ISSUED_AT_MS = 1_725_000_000_123


def offline_norito_frame(type_name: str, payload: bytes = b"\x01") -> bytes:
    """Build one canonical uncompressed compact-length Norito frame fixture."""

    return b"".join(
        (
            b"NRT0\x00\x00",
            schema_hash_for_type_name(type_name),
            b"\x00",
            len(payload).to_bytes(8, "little"),
            _crc64_xz(payload).to_bytes(8, "little"),
            b"\x02",
            b"\x00" * 8,
            payload,
        )
    )


def _compact_length(value: int) -> bytes:
    encoded = bytearray()
    while value >= 0x80:
        encoded.append((value & 0x7F) | 0x80)
        value >>= 7
    encoded.append(value)
    return bytes(encoded)


def offline_kagemusha_request_frame(
    type_name: str,
    *,
    field_count: int,
    operation_id_field_index: int,
    operation_id: bytes = bytes(OFFLINE_OPERATION_BYTES),
    authorization: Optional[bytes] = None,
    version: int = 4,
    trailing_payload: bytes = b"",
) -> bytes:
    """Build the exact top-level compact struct needed for SDK request binding."""

    fields = [b"\x01" for _ in range(field_count)]
    fields[0] = version.to_bytes(2, "little")
    fields[operation_id_field_index] = operation_id
    fields[-1] = (
        offline_kagemusha_authorization_archive(operation_id=operation_id)
        if authorization is None
        else authorization
    )
    payload = b"".join(_compact_length(len(field)) + field for field in fields)
    return offline_norito_frame(type_name, payload + trailing_payload)


def offline_kagemusha_authorization_archive(
    *,
    operation_id: bytes = bytes(OFFLINE_OPERATION_BYTES),
    issued_at_ms: int = OFFLINE_ISSUED_AT_MS,
    issued_at_ms_bytes: Optional[bytes] = None,
    field_count: int = 10,
    trailing_payload: bytes = b"",
) -> bytes:
    """Build the exact compact Kagemusha request-authorization archive."""

    fields = [b"\x01" for _ in range(field_count)]
    if field_count > 3:
        fields[3] = operation_id
    if field_count > 4:
        fields[4] = (
            issued_at_ms.to_bytes(8, "little")
            if issued_at_ms_bytes is None
            else issued_at_ms_bytes
        )
    return (
        b"".join(_compact_length(len(field)) + field for field in fields)
        + trailing_payload
    )


def offline_top_up_request(
    *,
    norito: Optional[bytes] = None,
) -> KagemushaTopUpRequestV4:
    """Build one canonical Kagemusha top-up request fixture."""

    archive = (
        offline_kagemusha_request_frame(
            OFFLINE_TOP_UP_REQUEST_SCHEMA_NAME,
            field_count=8,
            operation_id_field_index=6,
        )
        if norito is None
        else norito
    )
    return KagemushaTopUpRequestV4(norito=archive)


def offline_redeem_request(
    *,
    norito: Optional[bytes] = None,
) -> KagemushaRedeemRequestV4:
    """Build one canonical Kagemusha redemption request fixture."""

    archive = (
        offline_kagemusha_request_frame(
            OFFLINE_REDEEM_REQUEST_SCHEMA_NAME,
            field_count=10,
            operation_id_field_index=8,
        )
        if norito is None
        else norito
    )
    return KagemushaRedeemRequestV4(norito=archive)


def offline_operation_reference(**overrides: Any) -> Dict[str, Any]:
    """Build one canonical offline operation reference fixture."""

    reference = {
        "operation_id": OFFLINE_OPERATION_ID,
        "kind": {"kind": "top_up", "value": None},
        "state": {"state": "pending", "value": None},
        "transaction_hash": OFFLINE_TRANSACTION_HASH,
        "status_uri": OFFLINE_STATUS_URI,
        "submitted_at_ms": OFFLINE_ISSUED_AT_MS,
    }
    reference.update(overrides)
    return reference


def offline_fixed_bytes(byte: int) -> List[int]:
    """Return one fixed 32-byte fixture value."""

    return [byte] * 32


def offline_top_up_anchor(**overrides: Any) -> Dict[str, Any]:
    """Build one exact-NetworkId top-up anchor fixture."""

    amount = overrides.get("amount", {"atomic_units": 17, "scale": 4})
    current_note = overrides.get(
        "current_note",
        {
            "network_id": OFFLINE_NETWORK_ID,
            "asset": CANONICAL_ASSET_ID,
            "note_commitment": offline_fixed_bytes(0x41),
            "spend_nullifier": offline_fixed_bytes(0x51),
            "amount": dict(amount),
        },
    )
    anchor = {
        "version": 4,
        "network_id": OFFLINE_NETWORK_ID,
        "payer": CANONICAL_OWNER,
        "asset": CANONICAL_BALANCE_ASSET_ID,
        "asset_scale": amount["scale"],
        "amount": amount,
        "initial_root": offline_fixed_bytes(0x10),
        "finalized_root": offline_fixed_bytes(0x20),
        "shield_leaf_index": 7,
        "current_note": current_note,
        "topup_operation_id": list(OFFLINE_OPERATION_BYTES),
        "shield_verifier_id": {
            "backend": "halo2/ipa",
            "name": "asset-topup-shield-v2",
        },
        "shield_verifier_commitment": offline_fixed_bytes(0x61),
        "artifact_binding": {
            "version": 4,
            "generation": "generation-1",
            "manifest_sha256": offline_fixed_bytes(0x81),
        },
        "finalized_height": 12,
        "finalized_tx_hash": offline_fixed_bytes(0x23),
        "anchor_digest": offline_fixed_bytes(0x71),
    }
    anchor.update(overrides)
    return anchor


def offline_top_up_finality_proof(
    anchor: Optional[Mapping[str, Any]] = None,
    *,
    finalized_height: int = 12,
    **overrides: Any,
) -> Dict[str, Any]:
    """Build one exact-NetworkId top-up finality proof fixture."""

    bound_anchor = anchor if anchor is not None else offline_top_up_anchor()
    context_id = _canonical_hash(0xA0)

    def execution_commitment(*, includes_top_up: bool, seed: int) -> Dict[str, Any]:
        ordinary_root_bytes = bytearray([(seed + 2) & 0xFF] * 32)
        ordinary_root_bytes[-1] |= 1
        ordinary_root = bytes(ordinary_root_bytes)
        commitment = {
            "parent_state_root": _canonical_hash(seed),
            "post_state_root": _canonical_hash(seed + 1),
            "ordinary_writes_root": _hash_literal(ordinary_root),
            "topup_anchor_root": None,
            "topup_anchor_count": 0,
            "native_amx_application_manifest_version": 1,
            "native_amx_application_manifest_root": (
                _NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT
            ),
            "native_amx_application_manifest_count": 0,
            "lane_finality_manifest": None,
            "merge_carrier": None,
            "executed_block_wire_len": 123,
            "executed_block_wire_hash": _canonical_hash(seed + 3),
        }
        if includes_top_up:
            operation_id = bytes(
                bound_anchor.get("topup_operation_id", OFFLINE_OPERATION_BYTES)
            )
            anchor_digest = bytes(
                bound_anchor.get("anchor_digest", offline_fixed_bytes(0x71))
            )
            key_hash = _iroha_hash(b"\xd2" + operation_id)
            value_hash = _iroha_hash(anchor_digest)
            topup_root = _iroha_hash(b"\x00", key_hash, value_hash)
            commitment["topup_anchor_root"] = _hash_literal(topup_root)
            commitment["topup_anchor_count"] = 1
            commitment["post_state_root"] = _hash_literal(
                _iroha_hash(
                    b"iroha:kagemusha:v2:post-state-root\x00",
                    (1).to_bytes(4, "little"),
                    ordinary_root,
                    topup_root,
                )
            )
        return commitment

    def certificate(
        *,
        height: int,
        certificate_context_id: str,
        includes_top_up: bool,
        seed: int,
    ) -> Dict[str, Any]:
        round_ = {
            "context_id": [certificate_context_id],
            "height": height,
            "view": 0,
        }
        return {
            "round": round_,
            "proposal_round": copy.deepcopy(round_),
            "phase": {"phase": "commit", "details": None},
            "subject": {
                "parent_block_hash": (
                    None if height == 1 else _canonical_hash(seed + 5)
                ),
                "block_hash": _canonical_hash(seed + 6),
                "payload_hash": _canonical_hash(seed + 7),
            },
            "execution_commitment": execution_commitment(
                includes_top_up=includes_top_up,
                seed=seed + 8,
            ),
            "signers": [0],
            "aggregate_signature": [seed] * 96,
        }

    parent_commit_qc = None
    if finalized_height > 1:
        parent_commit_qc = certificate(
            height=finalized_height - 1,
            certificate_context_id=_canonical_hash(0xA1),
            includes_top_up=False,
            seed=0x21,
        )

    proof = {
        "version": 1,
        "anchor": {
            "topup_operation_id": list(
                bound_anchor.get("topup_operation_id", OFFLINE_OPERATION_BYTES)
            ),
            "anchor_digest": list(
                bound_anchor.get("anchor_digest", offline_fixed_bytes(0x71))
            ),
        },
        "commit_qc": {
            "height_context": {
                "context_id": [context_id],
                "network_id": OFFLINE_NETWORK_ID,
                "protocol_version": 4,
                "height": finalized_height,
                "epoch": 0,
                "epoch_end_height": max(100, finalized_height),
                "next_epoch_snapshot": None,
                "mode": {"mode": "permissioned", "details": None},
                "parent_commit_qc": parent_commit_qc,
                "snapshot_bootstrap": None,
                "nexus_amx_context_hash": _canonical_hash(0xA2),
                "execution_policy_hash": _canonical_hash(0xA3),
                "da_layout": {
                    "encoding": {
                        "encoding": "reed_solomon16",
                        "details": None,
                    },
                    "chunk_size_bytes": 4,
                    "data_shards": 1,
                    "parity_shards": 1,
                    "max_payload_size_bytes": 4,
                    "max_chunk_count": 2,
                },
                "leader_seed": offline_fixed_bytes(0xA4),
            },
            "certificate": certificate(
                height=finalized_height,
                certificate_context_id=context_id,
                includes_top_up=True,
                seed=0x31,
            ),
        },
        "anchor_path": {"leaf_index": 0, "leaf_count": 1, "siblings": []},
    }
    proof.update(overrides)
    return proof


def offline_applied_top_up_status(
    anchor: Optional[Mapping[str, Any]] = None,
    **result_overrides: Any,
) -> Dict[str, Any]:
    """Build one applied top-up status fixture."""

    finalized_height = result_overrides.get("finalized_block_height", 12)
    bound_anchor = dict(anchor if anchor is not None else offline_top_up_anchor())
    result = {
        "transaction_hash": OFFLINE_TRANSACTION_HASH,
        "finalized_block_height": finalized_height,
        "anchor": bound_anchor,
        "finality_proof": offline_top_up_finality_proof(
            bound_anchor,
            finalized_height=finalized_height,
        ),
    }
    result.update(result_overrides)
    return {
        "state": "applied",
        "value": {
            "operation_id": OFFLINE_OPERATION_ID,
            "result": {"kind": "top_up", "result": result},
        },
    }


def offline_rejected_status(error: Mapping[str, Any]) -> Dict[str, Any]:
    """Build one rejected offline status fixture."""

    return {
        "state": "rejected",
        "value": {
            "operation_id": OFFLINE_OPERATION_ID,
            "kind": {"kind": "redeem", "value": None},
            "transaction_hash": OFFLINE_TRANSACTION_HASH,
            "error": dict(error),
        },
    }
