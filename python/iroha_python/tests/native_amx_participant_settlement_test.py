"""Public Native settlement model keeps exact nonrecursive immutable identity."""

from dataclasses import asdict

import pytest

from iroha_python import SumeragiNativeAmxParticipantSettlement
from iroha_torii_client.native_amx import _crc16_ccitt_false, compute_native_amx_participant_settlement_hash


def test_native_participant_settlement_model_freezes_fifo_without_economic_fields():
    tagged = "hash:" + "01" * 32
    value = {
        "lane_id": 0, "dataspace_id": 0,
        "lane_incarnation": f"{tagged}#{_crc16_ccitt_false(tagged.encode('ascii')):04X}",
        "participant_lane_block_height": 1, "authority_context_height": 2,
        "previous_native_settlement_hash": None,
        "source_ids": ["F0" * 32, "10" * 32],
    }
    model = SumeragiNativeAmxParticipantSettlement.from_payload(value)
    assert model.source_ids == tuple(value["source_ids"])
    assert compute_native_amx_participant_settlement_hash(asdict(model)) == compute_native_amx_participant_settlement_hash(value)
    for field in ("tx_count", "receipts", "block_height", "native_amx_receipts"):
        with pytest.raises(ValueError, match="exactly its seven fields"):
            SumeragiNativeAmxParticipantSettlement.from_payload({**value, field: []})
    with pytest.raises(ValueError, match="nonzero"):
        SumeragiNativeAmxParticipantSettlement(**{**value, "source_ids": ["00" * 32]})


def test_native_participant_settlement_model_requires_nullable_history_link():
    tagged = "hash:" + "01" * 32
    incarnation = f"{tagged}#{_crc16_ccitt_false(tagged.encode('ascii')):04X}"
    value = {
        "lane_id": 0, "dataspace_id": 0, "lane_incarnation": incarnation,
        "participant_lane_block_height": 2, "authority_context_height": 2,
        "previous_native_settlement_hash": incarnation,
        "source_ids": ["F0" * 32, "10" * 32],
    }
    linked = SumeragiNativeAmxParticipantSettlement.from_payload(value)
    assert linked.previous_native_settlement_hash == incarnation
    assert compute_native_amx_participant_settlement_hash(asdict(linked)) == (
        "hash:1F71F0A536D50BB281A9C0FC3A9BF7AA5070F8860BF24173671FFB00D500DED5#1CF3")
    assert SumeragiNativeAmxParticipantSettlement.from_payload(
        {**value, "previous_native_settlement_hash": None}).previous_native_settlement_hash is None
    with pytest.raises(ValueError, match="null at participant height one"):
        SumeragiNativeAmxParticipantSettlement(**{**value, "participant_lane_block_height": 1})
    del value["previous_native_settlement_hash"]
    with pytest.raises(ValueError, match="seven fields"):
        SumeragiNativeAmxParticipantSettlement.from_payload(value)
    with pytest.raises(TypeError):
        SumeragiNativeAmxParticipantSettlement(**value)
