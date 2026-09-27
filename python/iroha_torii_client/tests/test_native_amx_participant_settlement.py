"""Exact nonrecursive Native participant settlement identity and wire boundaries."""

from copy import deepcopy

import pytest

from iroha_torii_client.native_amx import (
    _crc16_ccitt_false,
    compute_native_amx_participant_settlement_hash,
    parse_native_amx_participant_settlement,
)


def settlement():
    body = "01" * 32
    tagged = f"hash:{body}"
    return {
        "lane_id": 0,
        "dataspace_id": 0,
        "lane_incarnation": f"{tagged}#{_crc16_ccitt_false(tagged.encode('ascii')):04X}",
        "participant_lane_block_height": 1,
        "authority_context_height": 2,
        "previous_native_settlement_hash": None,
        "source_ids": ["F0" * 32, "10" * 32],
    }


def test_native_participant_settlement_retains_fifo_and_zero_route_coordinates():
    value = settlement()
    assert parse_native_amx_participant_settlement(value) == value
    original = compute_native_amx_participant_settlement_hash(value)
    reversed_value = {**value, "source_ids": list(reversed(value["source_ids"]))}
    assert original != compute_native_amx_participant_settlement_hash(reversed_value)
    for field in ("participant_lane_block_height", "authority_context_height"):
        assert original != compute_native_amx_participant_settlement_hash({**value, field: 3})


@pytest.mark.parametrize("field", [
    "block_height", "tx_count", "receipts", "total_local_amount", "total_xor_due",
    "total_xor_after_haircut", "total_xor_variance", "swap_metadata", "nexus_fee_receipts",
    "native_amx_receipts",
])
def test_native_participant_settlement_rejects_every_retired_field(field):
    with pytest.raises(ValueError, match="exactly its seven fields"):
        compute_native_amx_participant_settlement_hash({**settlement(), field: None})


@pytest.mark.parametrize("update", [
    {"source_ids": []}, {"source_ids": ["00" * 32]},
    {"source_ids": ["F0" * 32, "F0" * 32]}, {"source_ids": ["01" * 32] * 4097},
    {"source_ids": ["f0" * 32]}, {"source_ids": [[1] * 32]},
    {"participant_lane_block_height": 0}, {"authority_context_height": 0},
    {"lane_id": 1 << 32}, {"dataspace_id": 1 << 64}, {"lane_id": -1},
    {"authority_context_height": True},
])
def test_native_participant_settlement_rejects_invalid_identity(update):
    with pytest.raises((ValueError, TypeError)):
        compute_native_amx_participant_settlement_hash({**settlement(), **update})


def test_native_participant_settlement_exact_boundaries_and_required_fields():
    value = settlement()
    zero_tagged = "hash:" + "00" * 31 + "01"
    marked_zero = f"{zero_tagged}#{_crc16_ccitt_false(zero_tagged.encode('ascii')):04X}"
    with pytest.raises(ValueError, match="nonzero"):
        compute_native_amx_participant_settlement_hash({**value, "lane_incarnation": marked_zero})
    value.update(source_ids=[f"{index + 1:064X}" for index in range(4096)],
                 dataspace_id=(1 << 64) - 1, participant_lane_block_height=(1 << 64) - 1,
                 authority_context_height=(1 << 64) - 1)
    assert len(parse_native_amx_participant_settlement(value)["source_ids"]) == 4096
    for field in settlement():
        missing = deepcopy(value)
        del missing[field]
        with pytest.raises(ValueError, match="exactly its seven fields"):
            compute_native_amx_participant_settlement_hash(missing)


def test_native_participant_settlement_requires_and_binds_native_history_link():
    value = settlement()
    assert compute_native_amx_participant_settlement_hash(value) == (
        "hash:350CB3C0D8728E39820775AC522B345C84631FA81BA164F72FB70043657012CF#EB51")
    later = {**value, "participant_lane_block_height": 2}
    assert compute_native_amx_participant_settlement_hash(later) == (
        "hash:C3196EEEB6B5795424F82CDCCA551495E9F459E75EE274EE957FEC51EEE69393#EC1E")
    linked = {**later, "previous_native_settlement_hash": value["lane_incarnation"]}
    assert compute_native_amx_participant_settlement_hash(linked) == (
        "hash:1F71F0A536D50BB281A9C0FC3A9BF7AA5070F8860BF24173671FFB00D500DED5#1CF3")
    assert parse_native_amx_participant_settlement(linked)["previous_native_settlement_hash"] == value["lane_incarnation"]
    with pytest.raises(ValueError, match="null at participant height one"):
        compute_native_amx_participant_settlement_hash({**value,
            "previous_native_settlement_hash": value["lane_incarnation"]})
    old_six_field = dict(later)
    del old_six_field["previous_native_settlement_hash"]
    with pytest.raises(ValueError, match="seven fields"):
        compute_native_amx_participant_settlement_hash(old_six_field)
    zero_tagged = "hash:" + "00" * 31 + "01"
    marked_zero = f"{zero_tagged}#{_crc16_ccitt_false(zero_tagged.encode('ascii')):04X}"
    for previous in (marked_zero, "", True, {}, value["lane_incarnation"][:-1] + "0"):
        with pytest.raises((ValueError, TypeError)):
            compute_native_amx_participant_settlement_hash({**later,
                "previous_native_settlement_hash": previous})
