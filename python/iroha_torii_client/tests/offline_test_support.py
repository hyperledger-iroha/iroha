"""Canonical fixtures shared by the low-level Torii offline client tests."""

from __future__ import annotations

import copy
from typing import Any, Dict, List, Mapping, Optional

from iroha_torii_client import KagemushaRedeemRequestV4, KagemushaTopUpRequestV4
from iroha_torii_client.norito_frame import _crc64_xz, schema_hash_for_type_name

CANONICAL_OWNER = "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"
CANONICAL_ASSET_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
_NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT = (
    "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F"
)


def _canonical_hash(seed: int) -> str:
    body_bytes = bytearray([seed & 0xFF] * 32)
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


def offline_capability_payload(**overrides: Any) -> Dict[str, Any]:
    """Build one closed universal offline-readiness response fixture."""

    payload = {
        "mandatory": False,
        "cash_handoff_capability": "cash_handoff_v1",
        "required_bridge_abi_version": 22,
        "max_hops": 8,
        "ready": False,
        "assets": [],
        "blockers": [
            {
                "code": "offline_cash_authenticated_release_unavailable",
                "message": "No authenticated Offline Cash V1 release is selected by this asset-neutral response.",
            },
            {
                "code": "offline_cash_eligible_asset_unavailable",
                "message": "No eligible Offline Cash V1 asset is selected by this asset-neutral response.",
            },
            {
                "code": "offline_cash_proof_backend_unavailable",
                "message": "No reviewed production Offline Cash V1 proof and secure-device backend is authenticated by this response.",
            },
        ],
    }
    payload.update(overrides)
    return payload


OFFLINE_OPERATION_BYTES = [0x11] * 32
OFFLINE_OPERATION_ID = "11" * 32
OFFLINE_TRANSACTION_HASH = "23" * 32
OFFLINE_STATUS_URI = f"/v1/offline/operations/{OFFLINE_OPERATION_ID}"
OFFLINE_NETWORK_ID = _canonical_hash(0x91)
OFFLINE_OTHER_NETWORK_ID = _canonical_hash(0x93)
OFFLINE_SUBMITTED_AT_MS = 1_725_000_000_123


def _compact_length(value: int) -> bytes:
    encoded = bytearray()
    while value >= 0x80:
        encoded.append((value & 0x7F) | 0x80)
        value >>= 7
    encoded.append(value)
    return bytes(encoded)


def _field(value: bytes) -> bytes:
    return _compact_length(len(value)) + value


def _struct(*fields: bytes) -> bytes:
    return b"".join(_field(field) for field in fields)


def _network_id_bytes(network_id: str) -> bytes:
    prefix, body_and_checksum = network_id.split(":", 1)
    body, _checksum = body_and_checksum.split("#", 1)
    if prefix != "hash":
        raise ValueError("network_id must be a canonical Norito hash literal")
    return bytes.fromhex(body)


def offline_norito_frame(kind: str, payload: bytes) -> bytes:
    """Frame an explicitly supplied compact payload for adversarial tests."""

    schema = {
        "top_up": "iroha.torii.v1.offline.top_up.request",
        "redeem": "iroha.torii.v1.offline.redeem.request",
    }[kind]
    header = bytearray(40)
    header[:4] = b"NRT0"
    header[6:22] = schema_hash_for_type_name(schema)
    header[23:31] = len(payload).to_bytes(8, "little")
    header[31:39] = _crc64_xz(payload).to_bytes(8, "little")
    header[39] = 0x02
    return bytes(header) + bytes(8) + payload


def offline_norito_request_frame(
    kind: str,
    *,
    operation_id: str = OFFLINE_OPERATION_ID,
    authorization_operation_id: Optional[str] = None,
    issued_at_ms: int = OFFLINE_SUBMITTED_AT_MS,
    network_id: str = OFFLINE_NETWORK_ID,
    version: int = 4,
) -> bytes:
    """Build one structurally canonical compact signed-request test frame."""

    operation_id_bytes = bytes.fromhex(operation_id)
    authorization_id_bytes = bytes.fromhex(
        authorization_operation_id or operation_id
    )
    authorization = _struct(
        *(
            authorization_id_bytes
            if index == 3
            else issued_at_ms.to_bytes(8, "little")
            if index == 4
            else b"\x00"
            for index in range(10)
        )
    )
    network_id_bytes = _network_id_bytes(network_id)
    current_note = _struct(
        *(network_id_bytes if index == 0 else b"\x00" for index in range(5))
    )
    statement = _struct(
        *(network_id_bytes if index == 0 else b"\x00" for index in range(13))
    )
    bundle = _struct(statement, b"\x00", b"\x00")
    field_count = 8 if kind == "top_up" else 10
    operation_id_field_index = 6 if kind == "top_up" else 8
    payload = _struct(
        *(
            version.to_bytes(2, "little")
            if index == 0
            else operation_id_bytes
            if index == operation_id_field_index
            else current_note
            if kind == "top_up" and index == 3
            else bundle
            if kind == "redeem" and index == 1
            else authorization
            if index == field_count - 1
            else b"\x00"
            for index in range(field_count)
        )
    )
    return offline_norito_frame(kind, payload)


OFFLINE_TOP_UP_REQUEST_FRAME = offline_norito_request_frame("top_up")
OFFLINE_REDEEM_REQUEST_FRAME = offline_norito_request_frame("redeem")


def offline_top_up_request(
    *,
    norito: bytes = OFFLINE_TOP_UP_REQUEST_FRAME,
    operation_id: str = OFFLINE_OPERATION_ID,
) -> KagemushaTopUpRequestV4:
    """Build one canonical Kagemusha top-up request fixture."""

    return KagemushaTopUpRequestV4(norito=norito, operation_id=operation_id)


def offline_redeem_request(
    *,
    norito: bytes = OFFLINE_REDEEM_REQUEST_FRAME,
    operation_id: str = OFFLINE_OPERATION_ID,
) -> KagemushaRedeemRequestV4:
    """Build one canonical Kagemusha redemption request fixture."""

    return KagemushaRedeemRequestV4(norito=norito, operation_id=operation_id)


def offline_operation_reference(**overrides: Any) -> Dict[str, Any]:
    """Build one canonical offline operation reference fixture."""

    reference = {
        "operation_id": OFFLINE_OPERATION_ID,
        "kind": {"kind": "top_up", "value": None},
        "state": {"state": "pending", "value": None},
        "transaction_hash": OFFLINE_TRANSACTION_HASH,
        "status_uri": OFFLINE_STATUS_URI,
        "submitted_at_ms": OFFLINE_SUBMITTED_AT_MS,
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
        "asset": CANONICAL_ASSET_ID,
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
        commitment = {
            "parent_state_root": _canonical_hash(seed),
            "post_state_root": _canonical_hash(seed + 1),
            "ordinary_writes_root": _canonical_hash(seed + 2),
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
            commitment["topup_anchor_root"] = _canonical_hash(seed + 4)
            commitment["topup_anchor_count"] = 1
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
        "server_time_ms": 13,
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
