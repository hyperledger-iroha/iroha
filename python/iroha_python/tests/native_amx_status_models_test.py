"""Strict Native AMX and Nexus fee receipt parsing tests."""

from __future__ import annotations

from copy import deepcopy
from typing import Any, Callable

import pytest

from iroha_python import (
    SumeragiLaneRelayEnvelope,
    SumeragiLaneSettlementCommitment,
    SumeragiNativeAmxPhase,
)


def _crc16(value: bytes) -> int:
    crc = 0xFFFF
    for byte in value:
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc


def _hash(seed: int) -> str:
    body = (bytes([seed]) * 31 + bytes([seed | 1])).hex().upper()
    return f"hash:{body}#{_crc16(f'hash:{body}'.encode('ascii')):04X}"


def _qc(
    phase: str,
    *,
    participant_lane_id: int,
    participant_dataspace_id: int,
    participant_lane_incarnation: str,
    entrypoint_hash: str,
) -> dict[str, Any]:
    return {
        "body": {
            "chain_id_hash": _hash(0x11),
            "source_id": "ab" * 32,
            "tx_entrypoint_hash": entrypoint_hash,
            "plan_digest": _hash(0x23),
            "phase": phase,
            "coordinator_lane_id": 7,
            "coordinator_dataspace_id": 11,
            "coordinator_lane_incarnation": _hash(0x89),
            "participant_lane_id": participant_lane_id,
            "participant_dataspace_id": participant_dataspace_id,
            "participant_lane_incarnation": participant_lane_incarnation,
            "authority_context_height": 40,
            "coordinator_lane_block_height": 42,
            "coordinator_lane_block_view": 6,
            "coordinator_proposal_hash": _hash(0x55),
        },
        "validator_set_hash_version": 1,
        "validator_set_hash": _hash(0x45),
        "validator_set": ["validator-0", "validator-1", "validator-2", "validator-3"],
        "signers_bitmap": [0b0000_0111],
        "bls_aggregate_signature": "9a" * 96,
    }


def _leg(lane_id: int, dataspace_id: int, entrypoint_hash: str) -> dict[str, Any]:
    lane_incarnation = _hash(0x89 if lane_id == 7 else 0xB1)
    return {
        "lane_id": lane_id,
        "dataspace_id": dataspace_id,
        "lane_incarnation": lane_incarnation,
        "prepare_qc": _qc(
            "prepare",
            participant_lane_id=lane_id,
            participant_dataspace_id=dataspace_id,
            participant_lane_incarnation=lane_incarnation,
            entrypoint_hash=entrypoint_hash,
        ),
        "commit_qc": _qc(
            "commit",
            participant_lane_id=lane_id,
            participant_dataspace_id=dataspace_id,
            participant_lane_incarnation=lane_incarnation,
            entrypoint_hash=entrypoint_hash,
        ),
    }


def _commitment() -> dict[str, Any]:
    entrypoint_hash = _hash(0x67)
    huge_total = str((1 << 127) + 123)
    return {
        "block_height": 42,
        "lane_id": 7,
        "lane_incarnation": _hash(0x89),
        "dataspace_id": 11,
        "tx_count": 2,
        "total_local_amount": huge_total,
        "total_xor_due": "100000000000000000000000000000000000001",
        "total_xor_after_haircut": "99999999999999999999999999999999999999",
        "total_xor_variance": "2",
        "swap_metadata": None,
        "receipts": [],
        "nexus_fee_receipts": [
            {
                "version": 1,
                "source_id": "cd" * 32,
                "dataspace_id": 11,
                "lane_id": 7,
                "block_height": 42,
                "payer_account_id": "ed0120payer",
                "fee_asset_id": "xor#universal",
                "fee_amount": "12345678901234567890.012300",
                "schedule": {
                    "tx_bytes_len": 1 << 63,
                    "instruction_count": 2,
                    "gas_used": 987654321,
                    "base_fee": "1.2500",
                    "per_byte_fee": "0.0001",
                    "per_instruction_fee": "2",
                    "per_gas_unit_fee": "0.125",
                },
            }
        ],
        "native_amx_receipts": [
            {
                "version": 1,
                "source_id": "ab" * 32,
                "chain_id_hash": _hash(0x11),
                "plan_digest": _hash(0x23),
                "lane_id": 7,
                "dataspace_id": 11,
                "lane_incarnation": _hash(0x89),
                "authority_context_height": 40,
                "lane_block_height": 42,
                "lane_block_view": 6,
                "coordinator_proposal_hash": _hash(0x55),
                "legs": [
                    _leg(7, 11, entrypoint_hash),
                    _leg(8, 12, entrypoint_hash),
                ],
            }
        ],
    }


def _relay() -> dict[str, Any]:
    return {
        "lane_id": 7,
        "lane_incarnation": _hash(0x89),
        "dataspace_id": 11,
        "block_height": 42,
        "block_hash": _hash(0x91),
        "da_commitment_hash": _hash(0x93),
        "commit_qc": None,
        "settlement_commitment": _commitment(),
        "settlement_hash": _hash(0x95),
        "rbc_bytes_total": 1234,
    }


def _set(path: tuple[Any, ...], value: Any) -> Callable[[dict[str, Any]], None]:
    def mutate(payload: dict[str, Any]) -> None:
        target: Any = payload
        for key in path[:-1]:
            target = target[key]
        target[path[-1]] = value

    return mutate


def _delete(path: tuple[Any, ...]) -> Callable[[dict[str, Any]], None]:
    def mutate(payload: dict[str, Any]) -> None:
        target: Any = payload
        for key in path[:-1]:
            target = target[key]
        del target[path[-1]]

    return mutate


def test_lane_commitment_preserves_exact_native_amx_and_fee_evidence() -> None:
    payload = _commitment()
    parsed = SumeragiLaneSettlementCommitment.from_payload(payload)

    assert parsed.total_local_amount == str((1 << 127) + 123)
    assert parsed.lane_incarnation == payload["lane_incarnation"]
    assert parsed.nexus_fee_receipts[0].fee_amount == "12345678901234567890.012300"
    assert parsed.nexus_fee_receipts[0].schedule.tx_bytes_len == 1 << 63
    receipt = parsed.native_amx_receipts[0]
    assert receipt.plan_digest == payload["native_amx_receipts"][0]["plan_digest"]
    assert receipt.lane_incarnation == payload["lane_incarnation"]
    assert receipt.authority_context_height == 40
    assert receipt.lane_block_height == 42
    assert receipt.lane_block_view == 6
    assert receipt.legs[0].prepare_qc.body.phase is SumeragiNativeAmxPhase.PREPARE
    assert receipt.legs[0].commit_qc.body.phase is SumeragiNativeAmxPhase.COMMIT
    assert receipt.legs[0].prepare_qc.signers_bitmap == (0b0000_0111,)
    assert receipt.legs[0].prepare_qc.bls_aggregate_signature == "9a" * 96


def test_lane_settlement_quantities_preserve_canonical_fractional_values() -> None:
    payload = _commitment()
    payload["total_local_amount"] = "1.25"
    payload["total_xor_due"] = "0.5"
    payload["total_xor_after_haircut"] = "0.4"
    payload["total_xor_variance"] = "0.1"
    payload["receipts"] = [
        {
            "source_id": "ab" * 32,
            "local_amount": "1.25",
            "xor_due": "0.5",
            "xor_after_haircut": "0.4",
            "xor_variance": "0.1",
            "timestamp_ms": 1,
        }
    ]

    parsed = SumeragiLaneSettlementCommitment.from_payload(payload)
    assert parsed.total_local_amount == "1.25"
    assert parsed.receipts[0].xor_variance == "0.1"


@pytest.mark.parametrize(
    "value", [1, "1.0", "0.00000000000000000000000000001", str(1 << 511)]
)
def test_lane_settlement_rejects_lossy_or_noncanonical_quantities(value: Any) -> None:
    payload = _commitment()
    payload["total_local_amount"] = value

    with pytest.raises((TypeError, ValueError)):
        SumeragiLaneSettlementCommitment.from_payload(payload)


def test_lane_relay_preserves_the_exact_embedded_native_amx_receipt() -> None:
    payload = _relay()
    parsed = SumeragiLaneRelayEnvelope.from_payload(payload)

    assert parsed.lane_incarnation == parsed.settlement_commitment.lane_incarnation
    assert parsed.settlement_commitment.native_amx_receipts == (
        SumeragiLaneSettlementCommitment.from_payload(
            payload["settlement_commitment"]
        ).native_amx_receipts
    )


@pytest.mark.parametrize(
    "mutate",
    [
        _delete(("native_amx_receipts", 0, "version")),
        _delete(("native_amx_receipts", 0, "chain_id_hash")),
        _set(("native_amx_receipts", 0, "version"), 2),
        _set(("native_amx_receipts", 0, "source_id"), "ab" * 31),
        _set(("native_amx_receipts", 0, "plan_digest"), "hash:BAD#0000"),
        _set(("native_amx_receipts", 0, "lane_id"), 9),
        _set(("native_amx_receipts", 0, "dataspace_id"), 13),
        _set(("native_amx_receipts", 0, "lane_incarnation"), _hash(0x91)),
        _set(("native_amx_receipts", 0, "authority_context_height"), 0),
        _set(("native_amx_receipts", 0, "lane_block_height"), 43),
        _set(("native_amx_receipts", 0, "lane_block_view"), 7),
        _set(("native_amx_receipts", 0, "coordinator_proposal_hash"), _hash(0x57)),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "phase"), "commit"),
        _set(("native_amx_receipts", 0, "legs", 0, "commit_qc", "body", "phase"), "abort"),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "source_id"), "ef" * 32),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "chain_id_hash"), _hash(0x13)),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "plan_digest"), _hash(0x31)),
        _set(("native_amx_receipts", 0, "legs", 1, "commit_qc", "body", "tx_entrypoint_hash"), _hash(0x33)),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "coordinator_lane_id"), 99),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "coordinator_lane_incarnation"), _hash(0x93)),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "participant_dataspace_id"), 99),
        _set(("native_amx_receipts", 0, "legs", 0, "lane_incarnation"), _hash(0xC1)),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "participant_lane_incarnation"), _hash(0xC3)),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "authority_context_height"), 41),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "coordinator_lane_block_height"), 41),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "coordinator_lane_block_view"), 7),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "coordinator_proposal_hash"), _hash(0x59)),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "validator_set_hash_version"), 2),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "validator_set_hash"), _hash(0x37)),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "validator_set"), ["v", "v", "x", "y"]),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "signers_bitmap"), []),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "signers_bitmap"), [0b1000_0111]),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "signers_bitmap"), [0b0000_0011]),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "bls_aggregate_signature"), "zz" * 96),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "bls_aggregate_signature"), "00" * 96),
        _set(("native_amx_receipts", 0, "legs"), []),
    ],
    ids=lambda mutate: mutate.__name__,
)
def test_native_amx_parser_rejects_malformed_or_mismatched_evidence(
    mutate: Callable[[dict[str, Any]], None],
) -> None:
    payload = _commitment()
    mutate(payload)

    with pytest.raises((TypeError, ValueError)):
        SumeragiLaneSettlementCommitment.from_payload(payload)


def test_native_amx_parser_rejects_duplicate_participant_legs() -> None:
    payload = _commitment()
    payload["native_amx_receipts"][0]["legs"][1] = deepcopy(
        payload["native_amx_receipts"][0]["legs"][0]
    )

    with pytest.raises(ValueError, match="duplicate participant"):
        SumeragiLaneSettlementCommitment.from_payload(payload)


@pytest.mark.parametrize(
    "mutate",
    [
        _delete(("nexus_fee_receipts", 0, "schedule")),
        _set(("nexus_fee_receipts", 0, "fee_amount"), 1.25),
        _set(("nexus_fee_receipts", 0, "fee_amount"), "01.25"),
        _set(("nexus_fee_receipts", 0, "schedule", "gas_used"), "123"),
        _set(("nexus_fee_receipts", 0, "schedule", "base_fee"), "-1"),
        _set(("nexus_fee_receipts", 0, "lane_id"), 8),
    ],
)
def test_nexus_fee_parser_rejects_lossy_or_inconsistent_values(
    mutate: Callable[[dict[str, Any]], None],
) -> None:
    payload = _commitment()
    mutate(payload)

    with pytest.raises((TypeError, ValueError)):
        SumeragiLaneSettlementCommitment.from_payload(payload)


@pytest.mark.parametrize(
    "field,value",
    [
        ("lane_id", 8),
        ("lane_incarnation", _hash(0x99)),
        ("dataspace_id", 12),
        ("block_height", 43),
        ("settlement_hash", "hash:" + "AA" * 32 + "#0000"),
    ],
)
def test_lane_relay_parser_rejects_coordinate_or_hash_tampering(
    field: str, value: Any
) -> None:
    payload = _relay()
    payload[field] = value

    with pytest.raises((TypeError, ValueError)):
        SumeragiLaneRelayEnvelope.from_payload(payload)
