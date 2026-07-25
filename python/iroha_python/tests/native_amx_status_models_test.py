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
from iroha_torii_client.native_amx import (
    compute_native_amx_descriptor_hash,
    compute_native_amx_participant_settlement_hash,
    compute_native_amx_proposal_hash,
    compute_native_amx_validator_set_hash,
)


_NATIVE_AMX_VALIDATOR_SET = [
    "ea013094D37A1FCA72E8734CAAD4163678D82C36FE2CA70B80F5626E6591709E0D44831BE86CBA9BD0471C6D0D73FF9C4B54E0",
    "ea01309988FA1336476987EF7F91C3EA728B7EA0556698AA0F1A294147C8D5CD43BB24C4BCD14FAE23A384D721CBF1F6A16DF7",
    "ea013099BA3FACE165941434D3238C4D5767059EBFFFB4120A9885A4EB2BAC9CD868F690660D2936B03C0214FBDAD36034D578",
    "ea0130B921EAC90D1A99EC9DA3FF8C8A29EBEE19DD1B659A4C6FC21BC8046EA30DE566668EDCCEAE4CB5932F4F860606A1E0E3",
]


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
    participant_lane_block_view: int,
    entrypoint_hash: str,
    previous_descriptor_hash: str,
    participant_proposal_hash: str,
    participant_settlement_hash: str,
) -> dict[str, Any]:
    return {
        "body": {
            "round": {
                "context_id": [_hash(0x0D)],
                "height": 40,
                "view": 6,
            },
            "epoch": 3,
            "chain_id_hash": _hash(0x11),
            "source_id": "AB" * 32,
            "tx_entrypoint_hash": entrypoint_hash,
            "plan_digest": _hash(0x23),
            "phase": {"phase": phase, "detail": None},
            "coordinator_lane_id": 7,
            "coordinator_dataspace_id": 11,
            "coordinator_lane_incarnation": _hash(0x89),
            "participant_lane_id": participant_lane_id,
            "participant_dataspace_id": participant_dataspace_id,
            "participant_lane_incarnation": participant_lane_incarnation,
            "participant_previous_block_height": 41,
            "participant_previous_block_descriptor_hash": previous_descriptor_hash,
            "participant_lane_block_height": 42,
            "participant_lane_block_view": participant_lane_block_view,
            "participant_proposal_hash": participant_proposal_hash,
            "participant_settlement_commitment": participant_settlement_hash,
            "participant_validator_set_hash": _hash(0x45),
            "participant_validator_count": 4,
            "participant_min_quorum": 3,
            "authority_context_height": 40,
            "planned_coordinator_block_height": 42,
            "coordinator_lane_block_view": 6,
            "coordinator_proposal_hash": _hash(0x55),
        },
        "validator_set_hash_version": 1,
        "validator_set_hash": _hash(0x45),
        "validator_set": list(_NATIVE_AMX_VALIDATOR_SET),
        "validator_set_pops": [[0x5A] * 96 for _ in range(4)],
        "signers_bitmap": [0b0000_0111],
        "bls_aggregate_signature": [0x9A] * 96,
    }


def _leg(
    lane_id: int,
    dataspace_id: int,
    entrypoint_hash: str,
    grouped_entrypoint_hashes: list[str],
) -> dict[str, Any]:
    lane_incarnation = _hash(0x89 if lane_id == 7 else 0xB1)
    previous_descriptor_hash = _hash(0xA1 if lane_id == 7 else 0xA3)
    proposal_hash = _hash(0x55 if lane_id == 7 else 0xA7)
    settlement_hash = _hash(0xA9 if lane_id == 7 else 0xAB)
    participant_lane_block_view = 6 if lane_id == 7 else 0
    descriptor = {
        "lane_id": lane_id,
        "dataspace_id": dataspace_id,
        "lane_incarnation": lane_incarnation,
        "proposal_height": 40,
        "previous_lane_block_height": 41,
        "previous_lane_block_descriptor_hash": previous_descriptor_hash,
        "lane_block_height": 42,
        "lane_block_view": participant_lane_block_view,
        "subject_hash": _hash(0xB3),
        "payload_ownership_hash": _hash(0xB5),
        "rbc_instance_hash": _hash(0xB7),
        "accepted_candidate_indices": list(range(len(grouped_entrypoint_hashes))),
        "accepted_transaction_hashes": list(grouped_entrypoint_hashes),
        "validator_set_hash_version": 1,
        "validator_set_hash": _hash(0x45),
        "validator_set": list(_NATIVE_AMX_VALIDATOR_SET),
        "validator_count": 4,
        "min_quorum": 3,
        "qc_mode_tag": "permissioned:native-amx-v2",
        "descriptor_hash": _hash(0xB9 if lane_id == 7 else 0xBB),
    }
    settlement = {
        "block_height": 42,
        "lane_id": lane_id,
        "lane_incarnation": lane_incarnation,
        "dataspace_id": dataspace_id,
        "tx_count": 2,
        "total_local_amount": "0",
        "total_xor_due": "0",
        "total_xor_after_haircut": "0",
        "total_xor_variance": "0",
        "swap_metadata": None,
        "receipts": [
            {
                "source_id": "AB" * 32,
                "local_amount": "0",
                "xor_due": "0",
                "xor_after_haircut": "0",
                "xor_variance": "0",
                "timestamp_ms": 40,
            },
            {
                "source_id": "CD" * 32,
                "local_amount": "0",
                "xor_due": "0",
                "xor_after_haircut": "0",
                "xor_variance": "0",
                "timestamp_ms": 40,
            },
        ],
        "nexus_fee_receipts": [],
        "native_amx_receipts": [],
    }
    return {
        "lane_id": lane_id,
        "dataspace_id": dataspace_id,
        "participant_proposal": {
            "descriptor": descriptor,
            "proposal_hash": proposal_hash,
        },
        "participant_settlement": settlement,
        "participant_settlement_hash": settlement_hash,
        "prepare_qc": _qc(
            "prepare",
            participant_lane_id=lane_id,
            participant_dataspace_id=dataspace_id,
            participant_lane_incarnation=lane_incarnation,
            participant_lane_block_view=participant_lane_block_view,
            entrypoint_hash=entrypoint_hash,
            previous_descriptor_hash=previous_descriptor_hash,
            participant_proposal_hash=proposal_hash,
            participant_settlement_hash=settlement_hash,
        ),
        "commit_qc": _qc(
            "commit",
            participant_lane_id=lane_id,
            participant_dataspace_id=dataspace_id,
            participant_lane_incarnation=lane_incarnation,
            participant_lane_block_view=participant_lane_block_view,
            entrypoint_hash=entrypoint_hash,
            previous_descriptor_hash=previous_descriptor_hash,
            participant_proposal_hash=proposal_hash,
            participant_settlement_hash=settlement_hash,
        ),
    }


def _seal_native_amx_leg(leg: dict[str, Any]) -> None:
    descriptor = leg["participant_proposal"]["descriptor"]
    descriptor["validator_set_hash"] = compute_native_amx_validator_set_hash(
        descriptor["validator_set"]
    )
    descriptor["descriptor_hash"] = compute_native_amx_descriptor_hash(
        descriptor
    )
    leg["participant_proposal"]["proposal_hash"] = (
        compute_native_amx_proposal_hash(descriptor)
    )
    leg["participant_settlement_hash"] = (
        compute_native_amx_participant_settlement_hash(
            leg["participant_settlement"]
        )
    )
    for qc in (leg["prepare_qc"], leg["commit_qc"]):
        qc["validator_set_hash"] = descriptor["validator_set_hash"]
        qc["body"]["participant_validator_set_hash"] = descriptor[
            "validator_set_hash"
        ]
        qc["body"]["participant_proposal_hash"] = leg[
            "participant_proposal"
        ]["proposal_hash"]
        qc["body"]["participant_settlement_commitment"] = leg[
            "participant_settlement_hash"
        ]


def _seal_native_amx_receipt(receipt: dict[str, Any]) -> None:
    for leg in receipt["legs"]:
        _seal_native_amx_leg(leg)
    same_route = next(
        (
            leg
            for leg in receipt["legs"]
            if (leg["lane_id"], leg["dataspace_id"])
            == (receipt["lane_id"], receipt["dataspace_id"])
        ),
        None,
    )
    if same_route is not None:
        receipt["coordinator_proposal_hash"] = same_route[
            "participant_proposal"
        ]["proposal_hash"]
        for leg in receipt["legs"]:
            for qc in (leg["prepare_qc"], leg["commit_qc"]):
                qc["body"]["coordinator_proposal_hash"] = receipt[
                    "coordinator_proposal_hash"
                ]


def _commitment() -> dict[str, Any]:
    entrypoint_hashes = [_hash(0xAD), _hash(0xAF)]
    first_native_receipt = {
        "version": 2,
        "source_id": "AB" * 32,
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
            _leg(7, 11, entrypoint_hashes[0], entrypoint_hashes),
            _leg(8, 12, entrypoint_hashes[0], entrypoint_hashes),
        ],
    }
    _seal_native_amx_receipt(first_native_receipt)
    second_native_receipt = deepcopy(first_native_receipt)
    second_native_receipt["source_id"] = "CD" * 32
    for leg in second_native_receipt["legs"]:
        for qc in (leg["prepare_qc"], leg["commit_qc"]):
            qc["body"]["source_id"] = "CD" * 32
            qc["body"]["tx_entrypoint_hash"] = entrypoint_hashes[1]
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
                "source_id": "CD" * 32,
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
        "native_amx_receipts": [first_native_receipt, second_native_receipt],
    }


def _relay() -> dict[str, Any]:
    return {
        "lane_id": 7,
        "lane_incarnation": _hash(0x89),
        "dataspace_id": 11,
        "block_height": 42,
        "block_header": {"height": 42},
        "da_commitment_hash": _hash(0x93),
        "lane_block_descriptor_hash": _hash(0x91),
        "qc": None,
        "settlement_commitment": _commitment(),
        "settlement_hash": _hash(0x95),
        "rbc_bytes_total": 1234,
        "manifest_root": "A5" * 32,
        "fastpq_proof": {
            "proof_digest": _hash(0x97),
            "verified_at_height": 43,
        },
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
    assert receipt.legs[0].prepare_qc.bls_aggregate_signature == (0x9A,) * 96
    assert (
        receipt.legs[0].participant_proposal.proposal_hash
        == receipt.legs[0].prepare_qc.body.participant_proposal_hash
    )
    assert (
        receipt.legs[0].participant_settlement_hash
        == receipt.legs[0].commit_qc.body.participant_settlement_commitment
    )
    assert receipt.legs[0].participant_settlement.block_height == 42
    assert len(receipt.legs[0].participant_settlement.receipts) == 2
    assert receipt.legs[0].prepare_qc.body.source_id == "AB" * 32
    assert receipt.legs[0].prepare_qc.body.tx_entrypoint_hash == _hash(0xAD)
    assert not receipt.legs[0].requires_mixed_role_anchor_validation
    assert [native.source_id for native in parsed.native_amx_receipts] == [
        "AB" * 32,
        "CD" * 32,
    ]


def test_native_amx_keeps_global_round_and_coordinator_views_independent() -> None:
    payload = _commitment()
    receipt = payload["native_amx_receipts"][0]
    receipt["lane_block_view"] = 9
    for leg in receipt["legs"]:
        same_route = (leg["lane_id"], leg["dataspace_id"]) == (
            receipt["lane_id"],
            receipt["dataspace_id"],
        )
        if same_route:
            leg["participant_proposal"]["descriptor"]["lane_block_view"] = 9
        for qc in (leg["prepare_qc"], leg["commit_qc"]):
            assert qc["body"]["round"]["view"] == 6
            qc["body"]["coordinator_lane_block_view"] = 9
            if same_route:
                qc["body"]["participant_lane_block_view"] = 9
    _seal_native_amx_receipt(receipt)

    parsed = SumeragiLaneSettlementCommitment.from_payload(payload)

    body = parsed.native_amx_receipts[0].legs[0].prepare_qc.body
    assert body.round.view == 6
    assert body.coordinator_lane_block_view == 9


def test_native_amx_rejects_unordered_qc_validator_set() -> None:
    payload = _commitment()
    validator_set = payload["native_amx_receipts"][0]["legs"][0]["prepare_qc"][
        "validator_set"
    ]
    validator_set[0], validator_set[1] = validator_set[1], validator_set[0]

    with pytest.raises(ValueError, match="strictly ordered by canonical validator id"):
        SumeragiLaneSettlementCommitment.from_payload(payload)


def test_native_amx_parser_accepts_first_participant_lane_block_predecessor_shape() -> None:
    payload = _commitment()
    leg = payload["native_amx_receipts"][0]["legs"][1]
    for qc in (leg["prepare_qc"], leg["commit_qc"]):
        qc["body"]["participant_previous_block_height"] = 0
        qc["body"]["participant_previous_block_descriptor_hash"] = None
        qc["body"]["participant_lane_block_height"] = 1
    descriptor = leg["participant_proposal"]["descriptor"]
    descriptor["previous_lane_block_height"] = 0
    del descriptor["previous_lane_block_descriptor_hash"]
    descriptor["lane_block_height"] = 1
    leg["participant_settlement"]["block_height"] = 1
    _seal_native_amx_receipt(payload["native_amx_receipts"][0])

    parsed = SumeragiLaneSettlementCommitment.from_payload(payload)

    parsed_leg = parsed.native_amx_receipts[0].legs[1]
    assert parsed_leg.prepare_qc.body.participant_previous_block_descriptor_hash is None
    assert parsed_leg.participant_proposal.descriptor.previous_lane_block_descriptor_hash is None


def test_native_amx_parser_accepts_mixed_role_proposal_without_current_entrypoint() -> None:
    payload = _commitment()
    leg = payload["native_amx_receipts"][0]["legs"][1]
    leg["participant_proposal"]["descriptor"]["accepted_transaction_hashes"] = [
        _hash(0xC5),
        _hash(0xC7),
    ]
    _seal_native_amx_receipt(payload["native_amx_receipts"][0])

    parsed = SumeragiLaneSettlementCommitment.from_payload(payload)
    parsed_leg = parsed.native_amx_receipts[0].legs[1]

    assert (
        parsed_leg.participant_proposal.descriptor.accepted_transaction_hashes
        == (_hash(0xC5), _hash(0xC7))
    )
    assert parsed_leg.requires_mixed_role_anchor_validation


def test_native_amx_parser_rejects_unordered_participant_source_group() -> None:
    payload = _commitment()
    receipts = payload["native_amx_receipts"][0]["legs"][0][
        "participant_settlement"
    ]["receipts"]
    receipts[0], receipts[1] = receipts[1], receipts[0]
    _seal_native_amx_receipt(payload["native_amx_receipts"][0])

    with pytest.raises(ValueError, match="strictly ordered and unique"):
        SumeragiLaneSettlementCommitment.from_payload(payload)


@pytest.mark.parametrize(
    "accepted_hashes",
    [
        [_hash(0xAD)],
        [_hash(0xAF), _hash(0xAD)],
    ],
    ids=["descriptor-length-drift", "entrypoint-source-position-drift"],
)
def test_native_amx_parser_rejects_present_entrypoint_group_alignment_drift(
    accepted_hashes: list[str],
) -> None:
    payload = _commitment()
    descriptor = payload["native_amx_receipts"][0]["legs"][0][
        "participant_proposal"
    ]["descriptor"]
    descriptor["accepted_candidate_indices"] = list(range(len(accepted_hashes)))
    descriptor["accepted_transaction_hashes"] = accepted_hashes
    _seal_native_amx_receipt(payload["native_amx_receipts"][0])

    with pytest.raises(ValueError, match="grouped settlement are not aligned"):
        SumeragiLaneSettlementCommitment.from_payload(payload)


def _drift_same_route_incarnation(leg: dict[str, Any]) -> None:
    incarnation = _hash(0xD1)
    leg["participant_proposal"]["descriptor"]["lane_incarnation"] = incarnation
    leg["participant_settlement"]["lane_incarnation"] = incarnation
    for qc in (leg["prepare_qc"], leg["commit_qc"]):
        qc["body"]["participant_lane_incarnation"] = incarnation


def _drift_same_route_height(leg: dict[str, Any]) -> None:
    descriptor = leg["participant_proposal"]["descriptor"]
    descriptor["previous_lane_block_height"] = 42
    descriptor["lane_block_height"] = 43
    leg["participant_settlement"]["block_height"] = 43
    for qc in (leg["prepare_qc"], leg["commit_qc"]):
        qc["body"]["participant_previous_block_height"] = 42
        qc["body"]["participant_lane_block_height"] = 43


def _drift_same_route_view(leg: dict[str, Any]) -> None:
    leg["participant_proposal"]["descriptor"]["lane_block_view"] = 7
    for qc in (leg["prepare_qc"], leg["commit_qc"]):
        qc["body"]["participant_lane_block_view"] = 7


def _drift_same_route_proposal(leg: dict[str, Any]) -> None:
    leg["participant_proposal"]["descriptor"]["subject_hash"] = _hash(0xD3)


@pytest.mark.parametrize(
    "mutate",
    [
        _drift_same_route_incarnation,
        _drift_same_route_height,
        _drift_same_route_view,
        _drift_same_route_proposal,
    ],
    ids=lambda mutate: mutate.__name__,
)
def test_native_amx_parser_rejects_same_route_coordinator_identity_drift(
    mutate: Callable[[dict[str, Any]], None],
) -> None:
    payload = _commitment()
    leg = payload["native_amx_receipts"][0]["legs"][0]
    mutate(leg)
    _seal_native_amx_leg(leg)

    with pytest.raises(ValueError, match="same-route proposal"):
        SumeragiLaneSettlementCommitment.from_payload(payload)


def test_native_amx_parser_rejects_unordered_outer_source_group() -> None:
    payload = _commitment()
    payload["native_amx_receipts"].reverse()

    with pytest.raises(ValueError, match="strictly ordered and unique"):
        SumeragiLaneSettlementCommitment.from_payload(payload)


def test_native_amx_parser_rejects_outer_source_group_overflow_before_decode() -> None:
    payload = _commitment()
    payload["native_amx_receipts"] = [{}] * 4097

    with pytest.raises(ValueError, match="grouped source bound"):
        SumeragiLaneSettlementCommitment.from_payload(payload)


def test_native_amx_parser_rejects_participant_group_different_from_outer_group() -> None:
    payload = _commitment()
    payload["native_amx_receipts"][0]["legs"][0]["participant_settlement"][
        "receipts"
    ][1]["source_id"] = "EF" * 32
    _seal_native_amx_receipt(payload["native_amx_receipts"][0])

    with pytest.raises(ValueError, match="exact ordered source group"):
        SumeragiLaneSettlementCommitment.from_payload(payload)


def test_native_amx_parser_rejects_participant_finality_tampering() -> None:
    def add_leg_field(leg: dict[str, Any]) -> None:
        leg["future_leg_field"] = 1

    def delete_settlement_hash(leg: dict[str, Any]) -> None:
        del leg["participant_settlement_hash"]

    def wrong_proposal_type(leg: dict[str, Any]) -> None:
        leg["participant_proposal"] = []

    def wrong_settlement_hash_type(leg: dict[str, Any]) -> None:
        leg["participant_settlement_hash"] = 7

    def set_prepare_phase_to_string(leg: dict[str, Any]) -> None:
        leg["prepare_qc"]["body"]["phase"] = "prepare"

    def delete_participant_height(leg: dict[str, Any]) -> None:
        del leg["prepare_qc"]["body"]["participant_lane_block_height"]

    def add_body_field(leg: dict[str, Any]) -> None:
        leg["prepare_qc"]["body"]["future_participant_field"] = 1

    def wrong_body_type(leg: dict[str, Any]) -> None:
        leg["prepare_qc"]["body"]["participant_lane_block_view"] = "0"

    def mismatch_commit_identity(leg: dict[str, Any]) -> None:
        leg["commit_qc"]["body"]["participant_proposal_hash"] = _hash(0xC1)

    def mismatch_proposal_hash(leg: dict[str, Any]) -> None:
        leg["participant_proposal"]["proposal_hash"] = _hash(0xC3)

    def add_payload_hint(leg: dict[str, Any]) -> None:
        leg["participant_proposal"]["payload_block_hint"] = None

    def delete_descriptor_field(leg: dict[str, Any]) -> None:
        del leg["participant_proposal"]["descriptor"]["subject_hash"]

    def add_descriptor_field(leg: dict[str, Any]) -> None:
        leg["participant_proposal"]["descriptor"]["future_descriptor_field"] = 1

    def delete_required_predecessor(leg: dict[str, Any]) -> None:
        del leg["participant_proposal"]["descriptor"]["previous_lane_block_descriptor_hash"]

    def null_non_genesis_predecessor(leg: dict[str, Any]) -> None:
        for qc in (leg["prepare_qc"], leg["commit_qc"]):
            qc["body"]["participant_previous_block_descriptor_hash"] = None

    def nonnull_genesis_predecessor(leg: dict[str, Any]) -> None:
        for qc in (leg["prepare_qc"], leg["commit_qc"]):
            qc["body"]["participant_previous_block_height"] = 0
            qc["body"]["participant_lane_block_height"] = 1

    def explicit_null_genesis_descriptor(leg: dict[str, Any]) -> None:
        for qc in (leg["prepare_qc"], leg["commit_qc"]):
            qc["body"]["participant_previous_block_height"] = 0
            qc["body"]["participant_previous_block_descriptor_hash"] = None
            qc["body"]["participant_lane_block_height"] = 1
        descriptor = leg["participant_proposal"]["descriptor"]
        descriptor["previous_lane_block_height"] = 0
        descriptor["previous_lane_block_descriptor_hash"] = None
        descriptor["lane_block_height"] = 1
        leg["participant_settlement"]["block_height"] = 1

    def mismatch_proposal_route(leg: dict[str, Any]) -> None:
        leg["participant_proposal"]["descriptor"]["lane_id"] = 99

    def mismatch_proposal_height(leg: dict[str, Any]) -> None:
        leg["participant_proposal"]["descriptor"]["proposal_height"] = 41

    def mismatch_settlement_hash(leg: dict[str, Any]) -> None:
        leg["participant_settlement_hash"] = _hash(0xC7)

    def mismatch_settlement_route(leg: dict[str, Any]) -> None:
        leg["participant_settlement"]["lane_id"] = 99

    def nonzero_participant_effect(leg: dict[str, Any]) -> None:
        leg["participant_settlement"]["total_local_amount"] = "1"

    def mismatch_settlement_source(leg: dict[str, Any]) -> None:
        leg["participant_settlement"]["receipts"][0]["source_id"] = "EF" * 32

    def duplicate_settlement_source(leg: dict[str, Any]) -> None:
        leg["participant_settlement"]["receipts"][1]["source_id"] = "AB" * 32

    def wrong_settlement_tx_count(leg: dict[str, Any]) -> None:
        leg["participant_settlement"]["tx_count"] = 1

    def empty_settlement(leg: dict[str, Any]) -> None:
        leg["participant_settlement"]["tx_count"] = 0
        leg["participant_settlement"]["receipts"] = []

    def oversized_settlement(leg: dict[str, Any]) -> None:
        receipt = deepcopy(leg["participant_settlement"]["receipts"][0])
        leg["participant_settlement"]["tx_count"] = 4097
        leg["participant_settlement"]["receipts"] = [receipt] * 4097

    def recursive_settlement(leg: dict[str, Any]) -> None:
        leg["participant_settlement"]["native_amx_receipts"] = [{}]

    mutations = (
        add_leg_field,
        delete_settlement_hash,
        wrong_proposal_type,
        wrong_settlement_hash_type,
        set_prepare_phase_to_string,
        delete_participant_height,
        add_body_field,
        wrong_body_type,
        mismatch_commit_identity,
        mismatch_proposal_hash,
        add_payload_hint,
        delete_descriptor_field,
        add_descriptor_field,
        delete_required_predecessor,
        null_non_genesis_predecessor,
        nonnull_genesis_predecessor,
        explicit_null_genesis_descriptor,
        mismatch_proposal_route,
        mismatch_proposal_height,
        mismatch_settlement_hash,
        mismatch_settlement_route,
        nonzero_participant_effect,
        mismatch_settlement_source,
        duplicate_settlement_source,
        wrong_settlement_tx_count,
        empty_settlement,
        oversized_settlement,
        recursive_settlement,
    )
    for mutate in mutations:
        payload = _commitment()
        mutate(payload["native_amx_receipts"][0]["legs"][0])
        with pytest.raises((TypeError, ValueError), match="."):
            SumeragiLaneSettlementCommitment.from_payload(payload)


def test_lane_commitment_accepts_canonical_maximum_total_and_tagged_swap_enums() -> None:
    payload = _commitment()
    maximum = str((1 << 511) - 1)
    scale_28_maximum = f"{maximum[:126]}.{maximum[126:]}"
    assert len(scale_28_maximum) == 155
    payload["total_local_amount"] = scale_28_maximum
    payload["swap_metadata"] = {
        "epsilon_bps": 25,
        "twap_window_seconds": 300,
        "liquidity_profile": {"profile": "Tier2", "state": None},
        "twap_local_per_xor": "1.25",
        "volatility_class": {"bucket": "Elevated", "state": None},
    }

    parsed = SumeragiLaneSettlementCommitment.from_payload(payload)

    assert parsed.total_local_amount == scale_28_maximum
    assert parsed.swap_metadata is not None
    assert parsed.swap_metadata.liquidity_profile == "Tier2"
    assert parsed.swap_metadata.volatility_class == "Elevated"


@pytest.mark.parametrize(
    "invalid",
    [
        (1 << 128) - 1,
        True,
        "01",
        "1.0",
        "1.",
        "-1",
        "1e3",
        "not-a-quantity",
        "0.00000000000000000000000000001",
        str(1 << 511),
        "1" * 156,
    ],
)
def test_lane_commitment_rejects_noncanonical_quantity_wire_values(
    invalid: Any,
) -> None:
    payload = _commitment()
    payload["total_local_amount"] = invalid

    with pytest.raises(
        (TypeError, ValueError), match="quantity|canonical|length|512-bit"
    ):
        SumeragiLaneSettlementCommitment.from_payload(payload)


def test_lane_settlement_quantities_preserve_canonical_fractional_values() -> None:
    payload = _commitment()
    payload["total_local_amount"] = "1.25"
    payload["total_xor_due"] = "0.5"
    payload["total_xor_after_haircut"] = "0.4"
    payload["total_xor_variance"] = "0.1"
    payload["receipts"] = [
        {
            "source_id": "AB" * 32,
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


@pytest.mark.parametrize(
    "retired_field",
    [
        "total_local_micro",
        "total_xor_due_micro",
        "total_xor_after_haircut_micro",
        "total_xor_variance_micro",
    ],
)
def test_lane_settlement_rejects_retired_total_fields(retired_field: str) -> None:
    payload = _commitment()
    payload[retired_field] = "0"

    with pytest.raises(ValueError, match=f"unknown field `{retired_field}`"):
        SumeragiLaneSettlementCommitment.from_payload(payload)


@pytest.mark.parametrize(
    "retired_field",
    [
        "local_amount_micro",
        "xor_due_micro",
        "xor_after_haircut_micro",
        "xor_variance_micro",
    ],
)
def test_lane_settlement_rejects_retired_receipt_fields(retired_field: str) -> None:
    payload = _commitment()
    payload["receipts"] = [
        {
            "source_id": "AB" * 32,
            "local_amount": "0",
            "xor_due": "0",
            "xor_after_haircut": "0",
            "xor_variance": "0",
            "timestamp_ms": 1,
            retired_field: "0",
        }
    ]

    with pytest.raises(ValueError, match=f"unknown field `{retired_field}`"):
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
        _set(("native_amx_receipts", 0, "version"), 1),
        _set(("native_amx_receipts", 0, "source_id"), "ab" * 31),
        _set(("native_amx_receipts", 0, "source_id"), "ab" * 32),
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
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "round", "height"), 41),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "round", "view"), 7),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "round", "context_id"), []),
        _set(("native_amx_receipts", 0, "legs", 0, "commit_qc", "body", "epoch"), 4),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "phase", "detail"), "unexpected"),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "participant_validator_count"), 3),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "participant_min_quorum"), 2),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "participant_validator_set_hash"), _hash(0x39)),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "authority_context_height"), 41),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "planned_coordinator_block_height"), 41),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "coordinator_lane_block_view"), 7),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "coordinator_proposal_hash"), _hash(0x59)),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "validator_set_hash_version"), 2),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "validator_set_hash"), _hash(0x37)),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "validator_set"), ["v", "v", "x", "y"]),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "validator_set_pops"), []),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "validator_set_pops", 0), [0] * 96),
        _set(("native_amx_receipts", 0, "legs", 0, "commit_qc", "validator_set_pops", 0), [0x5B] * 96),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "signers_bitmap"), []),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "signers_bitmap"), [0b1000_0111]),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "signers_bitmap"), [0b0000_0011]),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "bls_aggregate_signature"), "zz" * 96),
        _set(("native_amx_receipts", 0, "legs", 0, "prepare_qc", "bls_aggregate_signature"), [0] * 96),
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


def test_native_amx_parser_rejects_participant_leg_overflow_before_decode() -> None:
    payload = _commitment()
    payload["native_amx_receipts"][0]["legs"] = [
        deepcopy(payload["native_amx_receipts"][0]["legs"][0])
        for _ in range(256)
    ]

    with pytest.raises(TypeError, match="bounded non-empty list"):
        SumeragiLaneSettlementCommitment.from_payload(payload)


@pytest.mark.parametrize(
    "mutate",
    [
        _delete(("nexus_fee_receipts", 0, "schedule")),
        _set(("nexus_fee_receipts", 0, "fee_amount"), 1.25),
        _set(("nexus_fee_receipts", 0, "fee_amount"), "01.25"),
        _set(("nexus_fee_receipts", 0, "source_id"), "cd" * 32),
        _set(("nexus_fee_receipts", 0, "schedule", "gas_used"), "123"),
        _set(("nexus_fee_receipts", 0, "schedule", "base_fee"), "-1"),
        _set(("nexus_fee_receipts", 0, "schedule", "legacy_rate"), "1"),
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
    "path",
    [
        ("legacy_total",),
        ("nexus_fee_receipts", 0, "legacy_fee"),
        ("native_amx_receipts", 0, "legs", 0, "prepare_qc", "body", "legacy_round"),
    ],
)
def test_lane_commitment_rejects_unknown_nested_wire_fields(path: tuple[Any, ...]) -> None:
    payload = _commitment()
    _set(path, 1)(payload)

    with pytest.raises(ValueError, match="unknown field"):
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
