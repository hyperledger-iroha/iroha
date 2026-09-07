"""Consume the Rust-owned grouped Native AMX v2 golden/negative corpus."""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from typing import Any, get_type_hints

import pytest
from iroha_torii_client.client import (
    SumeragiDiagnosticsStatus as CanonicalSumeragiDiagnosticsStatus,
)
from iroha_torii_client.native_amx import (
    compute_native_amx_application_manifest_singleton_root,
    compute_native_amx_participant_settlement_hash,
    parse_native_amx_participant_settlement,
)

import iroha_python.client as iroha_python_client
from iroha_python import (
    SumeragiDiagnosticsSnapshot,
    SumeragiLaneSettlementCommitment,
    SumeragiNativeAmxAttestationBody,
    SumeragiNativeAmxPhase,
    SumeragiNativeAmxParticipantSettlement,
    SumeragiNativeAmxSourceId,
    SumeragiNativeAmxTransactionEntrypointHash,
    SumeragiV2ExecutionCommitment,
)

FIXTURE_PATH = (
    Path(__file__).resolve().parents[3]
    / "fixtures"
    / "sumeragi_v2"
    / "native_amx_v2_grouped.json"
)


def _fixture() -> dict[str, Any]:
    return json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


def _validate_participant_constructor_boundaries(original: dict[str, Any]) -> None:
    first = {**deepcopy(original), "lane_id": 0, "dataspace_id": 0,
             "participant_lane_block_height": 1, "previous_native_settlement_hash": None}
    later = {**first, "participant_lane_block_height": 2}
    linked = {**later, "previous_native_settlement_hash": first["lane_incarnation"]}
    maximum_sources = [f"{index + 1:064X}" for index in range(4096)]
    maximum = {**first, "source_ids": maximum_sources}
    for value in (first, later, linked, maximum):
        assert parse_native_amx_participant_settlement(value) == value
        model = SumeragiNativeAmxParticipantSettlement.from_payload(value)
        assert model.source_ids == tuple(value["source_ids"])
        assert model.previous_native_settlement_hash == value["previous_native_settlement_hash"]
    assert compute_native_amx_participant_settlement_hash(later) != (
        compute_native_amx_participant_settlement_hash(linked))
    assert compute_native_amx_participant_settlement_hash(first) != (
        compute_native_amx_participant_settlement_hash(
            {**first, "source_ids": list(reversed(first["source_ids"]))}))
    invalid = [{key: value for key, value in first.items() if key != missing}
               for missing in first]
    invalid.extend({**first, retired: None} for retired in (
        "block_height", "tx_count", "receipts", "total_local_amount", "total_xor_due",
        "total_xor_after_haircut", "total_xor_variance", "swap_metadata",
        "nexus_fee_receipts", "native_amx_receipts"))
    invalid.extend({**first, **update} for update in (
        {"previous_native_settlement_hash": first["lane_incarnation"]},
        {"source_ids": []}, {"source_ids": ["00" * 32]},
        {"source_ids": [first["source_ids"][0]] * 2},
        {"source_ids": maximum_sources + ["FF" * 32]},
        {"source_ids": [first["source_ids"][0].lower()]},
        {"participant_lane_block_height": 0}, {"authority_context_height": 0},
        {"lane_id": 1 << 32}, {"dataspace_id": 1 << 64},
        {"participant_lane_block_height": 2,
         "previous_native_settlement_hash": "hash:" + "00" * 31 + "01#C50E"},
        {"lane_incarnation": "hash:" + "00" * 31 + "01#C50E"},
    ))
    for value in invalid:
        for parse in (parse_native_amx_participant_settlement,
                      SumeragiNativeAmxParticipantSettlement.from_payload):
            with pytest.raises((TypeError, ValueError)):
                parse(value)


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


def _validate_application_evidence(document: dict[str, Any]) -> None:
    golden = document["golden"]
    group = golden["receipt_group"]
    evidence = golden["application_evidence"]
    execution = evidence["execution_commitment"]
    artifacts = evidence["manifest_artifacts"]

    def require(condition: bool, message: str) -> None:
        if not condition:
            raise ValueError(message)

    require(execution["native_amx_application_manifest_version"] == 1, "manifest version")
    try:
        parsed_execution = SumeragiV2ExecutionCommitment.from_payload(
            execution,
            "golden.application_evidence.execution_commitment",
        )
    except (TypeError, ValueError) as error:
        raise ValueError("execution commitment") from error
    require(parsed_execution.merge_carrier is not None, "merge carrier")
    assert parsed_execution.merge_carrier is not None
    require(parsed_execution.merge_carrier.version == 1, "merge carrier version")
    require(
        parsed_execution.merge_carrier.entry_hash
        == execution["merge_carrier"]["entry_hash"],
        "merge carrier entry hash",
    )
    require(
        execution["native_amx_application_manifest_count"] == len(artifacts) == 1,
        "manifest count",
    )
    artifact = artifacts[0]
    leaf = artifact["leaf"]
    proof = artifact["proof"]
    require(artifact["version"] == 1 and leaf["version"] == 1, "artifact version")
    require(artifact["leaf_index"] == proof["leaf_index"] == 0, "proof position")
    require(proof["audit_path"] == [], "singleton proof path")
    expected_manifest_root = compute_native_amx_application_manifest_singleton_root(
        artifact["leaf_hash"]
    )
    require(
        artifact["manifest_leaf_count"]
        == execution["native_amx_application_manifest_count"]
        and artifact["manifest_root"]
        == execution["native_amx_application_manifest_root"]
        == expected_manifest_root,
        "manifest root",
    )
    require(
        leaf["executed_block_wire_hash"] == execution["executed_block_wire_hash"],
        "executed wire",
    )
    require(
        isinstance(execution.get("executed_block_wire_len"), int)
        and not isinstance(execution["executed_block_wire_len"], bool)
        and execution["executed_block_wire_len"] == 49,
        "executed wire length",
    )
    require(
        leaf["predecessor_height"] + 1 == leaf["participant_height"],
        "participant predecessor",
    )
    active = evidence["active_lane_incarnations"]
    require(
        active
        == [
            {
                "lane_id": leaf["lane_id"],
                "dataspace_id": leaf["dataspace_id"],
                "lane_incarnation": leaf["lane_incarnation"],
            }
        ],
        "active incarnation",
    )
    coordinator_route = (group["lane_id"], group["dataspace_id"])
    require(
        (leaf["lane_id"], leaf["dataspace_id"]) != coordinator_route,
        "same-route coordinator must not have separate application evidence",
    )

    members = leaf["members"]
    source_ids = [member["source_id"] for member in members]
    require(
        source_ids == [receipt["source_id"] for receipt in group["native_amx_receipts"]],
        "manifest source membership",
    )
    require(
        1 <= len(members) <= 4096
        and len(source_ids) == len(set(source_ids))
        and all(
            left["entrypoint_index"] < right["entrypoint_index"]
            for left, right in zip(members, members[1:], strict=False)
        ),
        "manifest member geometry",
    )
    carrier_entrypoints = set(evidence["carrier_entrypoint_hashes"])
    for receipt, member in zip(group["native_amx_receipts"], members, strict=True):
        leg = next(
            (
                candidate
                for candidate in receipt["legs"]
                if (
                    candidate["lane_id"],
                    candidate["dataspace_id"],
                )
                == (leaf["lane_id"], leaf["dataspace_id"])
            ),
            None,
        )
        require(leg is not None, "manifest route is absent from receipt")
        assert leg is not None
        descriptor = leg["participant_proposal"]["descriptor"]
        require(
            descriptor["lane_incarnation"] == leaf["lane_incarnation"]
            and descriptor["lane_block_height"] == leaf["participant_height"]
            and descriptor["lane_block_view"] == leaf["participant_view"]
            and descriptor["previous_lane_block_height"] == leaf["predecessor_height"]
            and descriptor.get("previous_lane_block_descriptor_hash")
            == leaf.get("predecessor_descriptor_hash")
            and descriptor["descriptor_hash"] == leaf["descriptor_hash"]
            and leg["participant_proposal"]["proposal_hash"] == leaf["proposal_hash"]
            and leg["participant_settlement_hash"] == leaf["settlement_hash"],
            "manifest participant identity",
        )
        require("previous_native_settlement_hash" in leaf, "manifest Native history link is required")
        settlement = parse_native_amx_participant_settlement(leg["participant_settlement"])
        require(leaf["previous_native_settlement_hash"] == settlement["previous_native_settlement_hash"],
                "manifest Native history link differs from participant settlement")
        body = leg["prepare_qc"]["body"]
        require(
            body["source_id"] == member["source_id"]
            and body["tx_entrypoint_hash"] == member["entrypoint_hash"]
            and member["entrypoint_index"]
            in descriptor["accepted_candidate_indices"]
            and set(descriptor["accepted_transaction_hashes"]) <= carrier_entrypoints,
            "mixed-role carrier anchor",
        )

    row = golden["expected_diagnostics"]["native_amx_participant_applications"][0]
    require(
        row["lane_id"] == leaf["lane_id"]
        and row["dataspace_id"] == leaf["dataspace_id"]
        and row["lane_incarnation"] == leaf["lane_incarnation"]
        and row["participant_height"] == leaf["participant_height"]
        and row["participant_view"] == leaf["participant_view"]
        and row["predecessor_height"] == leaf["predecessor_height"]
        and row.get("predecessor_descriptor_hash")
        == leaf.get("predecessor_descriptor_hash")
        and row["descriptor_hash"] == leaf["descriptor_hash"]
        and row["proposal_hash"] == leaf["proposal_hash"]
        and row["settlement_hash"] == leaf["settlement_hash"]
        and row["source_count"] == len(members)
        and row["application_block_height"] == leaf["application_block_height"]
        and row["application_block_hash"] == leaf["application_block_hash"],
        "diagnostic application identity",
    )


def test_grouped_native_amx_v2_golden_fixture() -> None:
    fixture = _fixture()
    assert fixture["format"] == "iroha-native-amx-v2-grouped"
    assert fixture["fixture_version"] == 1
    assert fixture["rust_owner"] == "iroha_data_model::block::consensus"
    assert {
        "coherent_duplicate_validator_set",
        "coherent_over_quorum_requirement",
        "execution_commitment_merge_carrier_wrong_version",
        "execution_commitment_missing_merge_carrier_field",
    } <= {control["id"] for control in fixture["negative_controls"]}

    payload = fixture["golden"]["receipt_group"]
    _validate_participant_constructor_boundaries(
        payload["native_amx_receipts"][0]["legs"][0]["participant_settlement"])
    # Coherent commitments using raw Hash encoding for the Rust [u8; 32]
    # source elements must not be accepted as the canonical settlement hash.
    raw_group = deepcopy(payload)
    raw_leg = raw_group["native_amx_receipts"][0]["legs"][0]
    raw_hash = "hash:F4FBF033695C8BE66DAD0D4296C8C20207C2558175D7FC7E30284A1685F007E3#ED3B"
    assert raw_hash != raw_leg["participant_settlement_hash"]
    raw_leg["participant_settlement_hash"] = raw_hash
    for qc in ("prepare_qc", "commit_qc"):
        raw_leg[qc]["body"]["participant_settlement_commitment"] = raw_hash
    with pytest.raises(ValueError, match="participant settlement hash"):
        SumeragiLaneSettlementCommitment.from_payload(raw_group)
    raw_diagnostics = deepcopy(fixture["golden"]["expected_diagnostics"])
    raw_diagnostics["lane_settlement_commitments"] = [raw_group]
    with pytest.raises(RuntimeError, match="participant_settlement_hash"):
        CanonicalSumeragiDiagnosticsStatus.from_payload(raw_diagnostics)
    parsed = SumeragiLaneSettlementCommitment.from_payload(payload)

    assert len(parsed.native_amx_receipts) == 2
    assert [receipt.source_id for receipt in parsed.native_amx_receipts] == fixture[
        "golden"
    ]["ordered_source_ids"]
    assert 1 <= len(parsed.native_amx_receipts) <= 4096
    expected_settlement_hashes = {
        (7, 11): "hash:32950D237EC6ACA2B345D3EFFBD0FE7E30C6E9AF9BD90EE18F8FBFDBDE2A8699#E813",
        (8, 12): "hash:954C813DA9EC5BE63036F21582293E718CF706A2275B25DA96E061FED76492CB#3240",
    }
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
            assert leg.participant_proposal.payload_block_hint is None
            assert leg.participant_settlement_hash == expected_settlement_hashes[
                (leg.lane_id, leg.dataspace_id)
            ]
            assert not leg.requires_mixed_role_anchor_validation
            assert leg.prepare_qc.body.phase is SumeragiNativeAmxPhase.PREPARE
            assert leg.commit_qc.body.phase is SumeragiNativeAmxPhase.COMMIT
            assert leg.prepare_qc.body.round.view == 6
            assert leg.prepare_qc.body.coordinator_lane_block_view == 9
            assert len(leg.prepare_qc.validator_set) == 4
            assert all(len(pop) == 96 for pop in leg.prepare_qc.validator_set_pops)
            assert len(leg.prepare_qc.bls_aggregate_signature) == 96
            assert list(leg.participant_settlement.source_ids) == fixture["golden"]["ordered_source_ids"]

    projection = fixture["golden"]["expected_diagnostics"]
    assert projection["lane_settlement_commitments"] == [payload]
    assert projection["native_amx_participant_applications"][0]["source_count"] == 2
    canonical_diagnostics = CanonicalSumeragiDiagnosticsStatus.from_payload(projection)
    high_level_diagnostics = SumeragiDiagnosticsSnapshot.from_payload(projection)
    assert (
        canonical_diagnostics.native_amx_participant_applications[0].source_count
        == 2
    )
    assert (
        high_level_diagnostics.native_amx_participant_applications[0].source_count
        == 2
    )
    _validate_application_evidence(fixture)


@pytest.mark.parametrize(
    "validator",
    [
        " not-a-canonical-bls-peer-id",
        "ed0120" + "AA" * 32,
        "ea0130" + "80" + "00" * 47,
        "EA0130" + "AA" * 48,
    ],
    ids=[
        "surrounding-whitespace",
        "non-bls-peer-id",
        "non-subgroup-bls-point",
        "non-canonical-multihash-case",
    ],
)
def test_grouped_native_amx_v2_rejects_noncanonical_validator_peer_ids(
    validator: str,
) -> None:
    fixture = _fixture()
    group = fixture["golden"]["receipt_group"]
    group["native_amx_receipts"][0]["legs"][0]["prepare_qc"][
        "validator_set"
    ][0] = validator

    with pytest.raises((TypeError, ValueError)):
        SumeragiLaneSettlementCommitment.from_payload(group)
    diagnostics = deepcopy(fixture["golden"]["expected_diagnostics"])
    diagnostics["lane_settlement_commitments"] = [group]
    with pytest.raises(RuntimeError):
        CanonicalSumeragiDiagnosticsStatus.from_payload(diagnostics)


def test_native_amx_source_and_entrypoint_domains_are_distinct_public_types() -> None:
    hints = get_type_hints(SumeragiNativeAmxAttestationBody)
    assert SumeragiNativeAmxSourceId is not SumeragiNativeAmxTransactionEntrypointHash
    assert hints["source_id"] is SumeragiNativeAmxSourceId
    assert (
        hints["tx_entrypoint_hash"]
        is SumeragiNativeAmxTransactionEntrypointHash
    )


def test_grouped_native_amx_v2_corpus_includes_required_controls() -> None:
    controls = _fixture()["negative_controls"]
    assert len(controls) == 58
    identifiers = {control["id"] for control in controls}
    assert {
        "missing_previous_native_settlement_hash",
        "manifest_missing_previous_native_settlement_hash",
        "coherent_forged_validator_set_hash",
        "coherent_stale_descriptor_hash",
        "coherent_stale_proposal_hash",
        "coherent_stale_settlement_hash",
        "manifest_leaf_hash_tampering",
        "non_canonical_validator_peer_id",
    }.issubset(identifiers)


@pytest.mark.parametrize(
    "control",
    _fixture()["negative_controls"],
    ids=lambda control: control["id"],
)
def test_grouped_native_amx_v2_negative_corpus(
    control: dict[str, Any], monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture = _fixture()
    assert control["expectation"] == "reject"
    for mutation in control["mutations"]:
        _apply_mutation(fixture, mutation)

    if control["validator"] == "application_evidence":
        with pytest.raises(ValueError):
            _validate_application_evidence(fixture)
        return

    assert control["validator"] == "receipt_group"
    mutated_group = fixture["golden"]["receipt_group"]
    with pytest.raises((TypeError, ValueError)):
        SumeragiLaneSettlementCommitment.from_payload(mutated_group)
    diagnostics = deepcopy(fixture["golden"]["expected_diagnostics"])
    diagnostics["lane_settlement_commitments"] = [mutated_group]
    with pytest.raises(RuntimeError):
        CanonicalSumeragiDiagnosticsStatus.from_payload(diagnostics)
    with pytest.raises((RuntimeError, TypeError, ValueError)):
        SumeragiDiagnosticsSnapshot.from_payload(diagnostics)

    if control["id"] == "short_aggregate_signature":
        # ML-MUT-API-03 deliberately weakens one real SDK check. The Rust-owned
        # control must then cross the Python accept boundary, proving that the
        # corpus detects this exact 96-byte signature-length regression.
        strict_byte_vector = iroha_python_client._strict_byte_vector

        def weakened_ml_mut_api_03_byte_vector(
            value: Any, length: int, context: str
        ) -> tuple[int, ...]:
            if (
                context == "native AMX v2 attestation QC bls_aggregate_signature"
                and length == 96
                and isinstance(value, list)
                and len(value) == 95
            ):
                return strict_byte_vector(value, 95, context)
            return strict_byte_vector(value, length, context)

        monkeypatch.setattr(
            iroha_python_client,
            "_strict_byte_vector",
            weakened_ml_mut_api_03_byte_vector,
        )
        weakened = SumeragiLaneSettlementCommitment.from_payload(mutated_group)
        assert (
            len(
                weakened.native_amx_receipts[0]
                .legs[0]
                .prepare_qc.bls_aggregate_signature
            )
            == 95
        )
