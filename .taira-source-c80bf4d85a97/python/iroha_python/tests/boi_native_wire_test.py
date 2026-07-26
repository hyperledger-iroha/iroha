"""Exact structural wire projections used by the BOI walkthrough."""

from __future__ import annotations

import json

from iroha_python import Instruction

_SOURCE = "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB"
_DESTINATION = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"
_ASSET_DEFINITION = "6pEP9RjNoZ7beWkT3pLfKoM1dyfi"
_ESCROW_ID = "11" * 32
_ESCROW_HASH = "hash:5CDBDB8963A47C9E1C0058ADB1DB2A5748248A240BE05FC2E006F2550BD462BF#3488"


def test_boi_instruction_wire_projections_are_exact_and_structural() -> None:
    batch = Instruction.transfer_asset_batch(
        _SOURCE,
        _ASSET_DEFINITION,
        json.dumps(
            [
                {"id": "israel", "to": _DESTINATION, "amount": "20"},
                {"id": "roman", "to": _SOURCE, "amount": "60"},
            ],
            separators=(",", ":"),
        ),
        mode="Independent",
    )
    holding_limit = Instruction.set_asset_holding_limit(
        _DESTINATION,
        _ASSET_DEFINITION,
        "100",
    )
    open_lock = Instruction.open_conditional_asset_lock(
        _ESCROW_ID,
        _ASSET_DEFINITION,
        _DESTINATION,
        "100",
        json.dumps(
            [
                {
                    "kind": "oracle",
                    "id": "processor",
                    "attestor": _SOURCE,
                    "sequence": 1,
                    "predicate_kind": "text_equals",
                    "predicate_value": "i7",
                },
                {
                    "kind": "oracle",
                    "id": "delivery_days",
                    "attestor": _DESTINATION,
                    "sequence": 2,
                    "predicate_kind": "quantity_at_most",
                    "predicate_value": "3",
                },
                {
                    "kind": "within",
                    "id": "within_7_days",
                    "duration_ms": 604_800_000,
                },
            ],
            separators=(",", ":"),
        ),
    )
    attestation = Instruction.attest_asset_lock_condition(
        _ESCROW_ID,
        "processor",
        "text",
        "i7",
    )

    assert batch.to_wire_json() == {
        "wire_id": "iroha.transfer_batch",
        "payload": {
            "mode": {"mode": "Independent", "value": None},
            "entries": [
                {
                    "leg_id": "israel",
                    "from": _SOURCE,
                    "to": _DESTINATION,
                    "asset_definition": _ASSET_DEFINITION,
                    "amount": "20",
                },
                {
                    "leg_id": "roman",
                    "from": _SOURCE,
                    "to": _SOURCE,
                    "asset_definition": _ASSET_DEFINITION,
                    "amount": "60",
                },
            ],
        },
    }
    assert holding_limit.to_wire_json() == {
        "wire_id": "iroha.asset.holding_limit.set",
        "payload": {
            "account_id": _DESTINATION,
            "asset_definition_id": _ASSET_DEFINITION,
            "holding_limit": "100",
        },
    }
    assert open_lock.to_wire_json() == {
        "wire_id": "iroha_data_model::isi::escrow::OpenAssetLock",
        "payload": {
            "escrow_id": [_ESCROW_HASH],
            "asset_definition": _ASSET_DEFINITION,
            "destination": _DESTINATION,
            "amount": "100",
            "evidence_hashes": [],
            "conditions": [
                {
                    "kind": "Oracle",
                    "value": {
                        "id": "processor",
                        "attestor": _SOURCE,
                        "predicate": {"kind": "TextEquals", "value": "i7"},
                        "sequence": 1,
                    },
                },
                {
                    "kind": "Oracle",
                    "value": {
                        "id": "delivery_days",
                        "attestor": _DESTINATION,
                        "predicate": {
                            "kind": "QuantityAtMost",
                            "value": "3",
                        },
                        "sequence": 2,
                    },
                },
                {
                    "kind": "Within",
                    "value": {
                        "id": "within_7_days",
                        "duration_ms": 604_800_000,
                    },
                },
            ],
        },
    }
    assert attestation.to_wire_json() == {
        "wire_id": "iroha_data_model::isi::escrow::AttestAssetLockCondition",
        "payload": {
            "escrow_id": [_ESCROW_HASH],
            "condition_id": "processor",
            "value": {"kind": "Text", "value": "i7"},
            "evidence_hash": None,
        },
    }

    # Canonical signing JSON remains the opaque InstructionBox payload.
    assert isinstance(json.loads(batch.to_json()), str)
