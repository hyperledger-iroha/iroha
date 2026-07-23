"""Consume the Rust-owned grouped Native AMX v2 golden/negative corpus."""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
from typing import Any

import pytest

from iroha_python import SumeragiLaneSettlementCommitment, SumeragiNativeAmxPhase


FIXTURE_PATH = (
    Path(__file__).resolve().parents[3]
    / "fixtures"
    / "sumeragi_v2"
    / "native_amx_v2_grouped.json"
)


def _fixture() -> dict[str, Any]:
    return json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


def _tokens(pointer: str) -> list[str]:
    if not pointer.startswith("/"):
        raise AssertionError(f"fixture mutation is not an absolute JSON pointer: {pointer}")
    return [
        token.replace("~1", "/").replace("~0", "~")
        for token in pointer[1:].split("/")
    ]


def _resolve(document: Any, pointer: str) -> Any:
    target = document
    for token in _tokens(pointer):
        target = target[int(token)] if isinstance(target, list) else target[token]
    return target


def _assign(document: Any, pointer: str, value: Any) -> None:
    tokens = _tokens(pointer)
    target = document
    for token in tokens[:-1]:
        target = target[int(token)] if isinstance(target, list) else target[token]
    leaf = tokens[-1]
    if isinstance(target, list):
        target[int(leaf)] = value
    else:
        target[leaf] = value


def _remove(document: Any, pointer: str) -> None:
    tokens = _tokens(pointer)
    target = document
    for token in tokens[:-1]:
        target = target[int(token)] if isinstance(target, list) else target[token]
    leaf = tokens[-1]
    if isinstance(target, list):
        target.pop(int(leaf))
    else:
        del target[leaf]


def _apply_mutation(document: dict[str, Any], mutation: dict[str, Any]) -> None:
    operation = mutation["op"]
    pointer = mutation["path"]
    value = mutation.get("value")
    if operation == "replace":
        _assign(document, pointer, deepcopy(value))
    elif operation == "remove":
        _remove(document, pointer)
    elif operation == "copy":
        _assign(document, pointer, deepcopy(_resolve(document, value["from"])))
    elif operation == "swap":
        target = _resolve(document, pointer)
        target[value["left"]], target[value["right"]] = (
            target[value["right"]],
            target[value["left"]],
        )
    elif operation == "repeat":
        target = _resolve(document, pointer)
        repeated = [deepcopy(target[value["source_index"]]) for _ in range(value["count"])]
        _assign(document, pointer, repeated)
    else:
        raise AssertionError(f"unsupported fixture mutation operation: {operation}")


def test_grouped_native_amx_v2_golden_fixture() -> None:
    fixture = _fixture()
    assert fixture["format"] == "iroha-native-amx-v2-grouped"
    assert fixture["fixture_version"] == 1
    assert fixture["rust_owner"] == "iroha_data_model::block::consensus"

    payload = fixture["golden"]["receipt_group"]
    parsed = SumeragiLaneSettlementCommitment.from_payload(payload)

    assert len(parsed.native_amx_receipts) == 2
    assert [receipt.source_id for receipt in parsed.native_amx_receipts] == fixture[
        "golden"
    ]["ordered_source_ids"]
    assert 1 <= len(parsed.native_amx_receipts) <= 4096
    for receipt in parsed.native_amx_receipts:
        assert len(receipt.legs) == 2
        assert receipt.lane_block_view == 9
        same_route = next(
            leg
            for leg in receipt.legs
            if (leg.lane_id, leg.dataspace_id)
            == (receipt.lane_id, receipt.dataspace_id)
        )
        same_route_descriptor = same_route.participant_proposal.descriptor
        assert same_route_descriptor.lane_incarnation == receipt.lane_incarnation
        assert same_route_descriptor.lane_block_height == receipt.lane_block_height
        assert same_route_descriptor.lane_block_view == receipt.lane_block_view
        assert (
            same_route.participant_proposal.proposal_hash
            == receipt.coordinator_proposal_hash
        )
        for leg in receipt.legs:
            assert not leg.requires_mixed_role_anchor_validation
            assert leg.prepare_qc.body.phase is SumeragiNativeAmxPhase.PREPARE
            assert leg.commit_qc.body.phase is SumeragiNativeAmxPhase.COMMIT
            assert leg.prepare_qc.body.round.view == 6
            assert leg.prepare_qc.body.coordinator_lane_block_view == 9
            assert len(leg.prepare_qc.validator_set) == 4
            assert all(len(pop) == 96 for pop in leg.prepare_qc.validator_set_pops)
            assert len(leg.prepare_qc.bls_aggregate_signature) == 96
            assert [
                grouped.source_id for grouped in leg.participant_settlement.receipts
            ] == fixture["golden"]["ordered_source_ids"]

    projection = fixture["golden"]["expected_diagnostics"]
    assert projection["lane_settlement_commitments"] == [payload]
    assert projection["native_amx_participant_applications"][0]["source_count"] == 2


@pytest.mark.parametrize(
    "control",
    _fixture()["negative_controls"],
    ids=lambda control: control["id"],
)
def test_grouped_native_amx_v2_negative_corpus(control: dict[str, Any]) -> None:
    fixture = _fixture()
    assert control["expectation"] == "reject"
    for mutation in control["mutations"]:
        _apply_mutation(fixture, mutation)

    with pytest.raises((TypeError, ValueError)):
        SumeragiLaneSettlementCommitment.from_payload(
            fixture["golden"]["receipt_group"]
        )
