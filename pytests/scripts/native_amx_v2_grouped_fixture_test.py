"""OpenAPI parity for the Rust-owned grouped Native AMX v2 fixture."""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
import re
from typing import Any

from python.iroha_torii_client.native_amx import (
    compute_native_amx_descriptor_hash,
    compute_native_amx_participant_settlement_hash,
    compute_native_amx_proposal_hash,
    compute_native_amx_validator_set_hash,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_PATH = (
    REPO_ROOT / "fixtures" / "sumeragi_v2" / "native_amx_v2_grouped.json"
)
OPENAPI_PATHS = (
    REPO_ROOT / "artifacts" / "openapi" / "torii.json",
    REPO_ROOT
    / "artifacts"
    / "openapi"
    / "versions"
    / "current"
    / "torii.json",
)
SOURCE_ID_RE = re.compile(r"^[0-9A-F]{64}$")
HASH_RE = re.compile(r"^hash:[0-9A-F]{64}#[0-9A-F]{4}$")
BLS_VALIDATOR_RE = re.compile(r"^ea0130[0-9A-F]{96}$")
MAX_GROUP_SOURCES = 4_096
MAX_PARTICIPANT_LEGS = 255
MAX_VALIDATORS = 128
BLS_PROOF_BYTES = 96


def _tokens(pointer: str) -> list[str]:
    assert pointer.startswith("/")
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
    if isinstance(target, list):
        target[int(tokens[-1])] = value
    else:
        target[tokens[-1]] = value


def _remove(document: Any, pointer: str) -> None:
    tokens = _tokens(pointer)
    target = document
    for token in tokens[:-1]:
        target = target[int(token)] if isinstance(target, list) else target[token]
    if isinstance(target, list):
        target.pop(int(tokens[-1]))
    else:
        del target[tokens[-1]]


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
        _assign(
            document,
            pointer,
            [
                deepcopy(target[value["source_index"]])
                for _ in range(value["count"])
            ],
        )
    else:
        raise AssertionError(f"unsupported mutation {operation}")


def _expected_quorum(validator_count: int) -> int:
    return validator_count - (validator_count - 1) // 3


def _validate_qc(qc: dict[str, Any], expected_phase: str) -> None:
    body = qc["body"]
    assert body["phase"] == {"phase": expected_phase, "detail": None}
    assert body["round"]["height"] >= 1
    assert body["authority_context_height"] == body["round"]["height"]
    assert body["planned_coordinator_block_height"] >= 1
    assert SOURCE_ID_RE.fullmatch(body["source_id"])
    for field in (
        "chain_id_hash",
        "tx_entrypoint_hash",
        "plan_digest",
        "coordinator_lane_incarnation",
        "participant_lane_incarnation",
        "participant_proposal_hash",
        "participant_settlement_commitment",
        "participant_validator_set_hash",
        "coordinator_proposal_hash",
    ):
        assert HASH_RE.fullmatch(body[field])
    predecessor_height = body["participant_previous_block_height"]
    assert predecessor_height + 1 == body["participant_lane_block_height"]
    predecessor_hash = body["participant_previous_block_descriptor_hash"]
    assert (predecessor_height == 0) == (predecessor_hash is None)
    if predecessor_hash is not None:
        assert HASH_RE.fullmatch(predecessor_hash)

    validator_set = qc["validator_set"]
    assert 1 <= len(validator_set) <= MAX_VALIDATORS
    assert validator_set == sorted(validator_set)
    assert len(set(validator_set)) == len(validator_set)
    assert all(BLS_VALIDATOR_RE.fullmatch(validator) for validator in validator_set)
    assert qc["validator_set_hash_version"] == 1
    assert qc["validator_set_hash"] == compute_native_amx_validator_set_hash(
        validator_set
    )
    assert qc["validator_set_hash"] == body["participant_validator_set_hash"]

    pops = qc["validator_set_pops"]
    assert len(pops) == len(validator_set)
    for pop in pops:
        assert len(pop) == BLS_PROOF_BYTES
        assert all(isinstance(byte, int) and 0 <= byte <= 255 for byte in pop)
        assert any(pop)

    validator_count = len(validator_set)
    quorum = _expected_quorum(validator_count)
    assert body["participant_validator_count"] == validator_count
    assert body["participant_min_quorum"] == quorum
    bitmap = qc["signers_bitmap"]
    assert len(bitmap) == (validator_count + 7) // 8
    assert all(isinstance(byte, int) and 0 <= byte <= 255 for byte in bitmap)
    used_bits = validator_count % 8
    if used_bits:
        padding_mask = 0xFF ^ ((1 << used_bits) - 1)
        assert bitmap[-1] & padding_mask == 0
    assert sum(bin(byte).count("1") for byte in bitmap) >= quorum

    signature = qc["bls_aggregate_signature"]
    assert len(signature) == BLS_PROOF_BYTES
    assert all(isinstance(byte, int) and 0 <= byte <= 255 for byte in signature)
    assert any(signature)


def _validate_participant_descriptor(descriptor: dict[str, Any]) -> None:
    assert descriptor["proposal_height"] >= 1
    assert descriptor["lane_block_height"] >= 1
    predecessor_height = descriptor["previous_lane_block_height"]
    assert predecessor_height + 1 == descriptor["lane_block_height"]
    predecessor_hash = descriptor["previous_lane_block_descriptor_hash"]
    assert (predecessor_height == 0) == (predecessor_hash is None)
    if predecessor_hash is not None:
        assert HASH_RE.fullmatch(predecessor_hash)
    assert descriptor["qc_mode_tag"].strip()

    indices = descriptor["accepted_candidate_indices"]
    hashes = descriptor["accepted_transaction_hashes"]
    assert 1 <= len(indices) <= MAX_GROUP_SOURCES
    assert len(indices) == len(hashes)
    assert len(set(indices)) == len(indices)
    assert len(set(hashes)) == len(hashes)
    assert all(isinstance(index, int) and index >= 0 for index in indices)
    assert all(HASH_RE.fullmatch(hash_value) for hash_value in hashes)

    validator_set = descriptor["validator_set"]
    assert 1 <= len(validator_set) <= MAX_VALIDATORS
    assert validator_set == sorted(validator_set)
    assert len(set(validator_set)) == len(validator_set)
    assert all(BLS_VALIDATOR_RE.fullmatch(validator) for validator in validator_set)
    assert descriptor["validator_set_hash_version"] == 1
    assert descriptor[
        "validator_set_hash"
    ] == compute_native_amx_validator_set_hash(validator_set)
    assert descriptor["validator_count"] == len(validator_set)
    assert descriptor["min_quorum"] == _expected_quorum(len(validator_set))
    for field in (
        "lane_incarnation",
        "subject_hash",
        "payload_ownership_hash",
        "rbc_instance_hash",
        "validator_set_hash",
        "descriptor_hash",
    ):
        assert HASH_RE.fullmatch(descriptor[field])
    assert descriptor["descriptor_hash"] == compute_native_amx_descriptor_hash(
        descriptor
    )


def _validate_receipt_group(document: dict[str, Any]) -> None:
    group = document["golden"]["receipt_group"]
    receipts = group["native_amx_receipts"]
    assert 1 <= len(receipts) <= MAX_GROUP_SOURCES
    source_ids = [receipt["source_id"] for receipt in receipts]
    assert all(SOURCE_ID_RE.fullmatch(source_id) for source_id in source_ids)
    assert source_ids == sorted(source_ids)
    assert len(set(source_ids)) == len(source_ids)
    assert group["tx_count"] == len(receipts)

    for receipt in receipts:
        assert receipt["version"] == 2
        assert receipt["lane_id"] == group["lane_id"]
        assert receipt["dataspace_id"] == group["dataspace_id"]
        assert receipt["lane_incarnation"] == group["lane_incarnation"]
        assert receipt["lane_block_height"] == group["block_height"]
        assert receipt["authority_context_height"] >= 1
        for field in (
            "chain_id_hash",
            "plan_digest",
            "lane_incarnation",
            "coordinator_proposal_hash",
        ):
            assert HASH_RE.fullmatch(receipt[field])

        legs = receipt["legs"]
        assert 1 <= len(legs) <= MAX_PARTICIPANT_LEGS
        routes = [(leg["lane_id"], leg["dataspace_id"]) for leg in legs]
        assert len(set(routes)) == len(routes)
        first_body = legs[0]["prepare_qc"]["body"]
        expected_round = first_body["round"]
        expected_epoch = first_body["epoch"]
        expected_entrypoint = first_body["tx_entrypoint_hash"]

        for leg in legs:
            proposal = leg["participant_proposal"]
            descriptor = proposal["descriptor"]
            _validate_participant_descriptor(descriptor)
            assert proposal["proposal_hash"] == compute_native_amx_proposal_hash(
                descriptor
            )
            prepare = leg["prepare_qc"]
            commit = leg["commit_qc"]
            _validate_qc(prepare, "prepare")
            _validate_qc(commit, "commit")

            expected_commit_body = deepcopy(prepare["body"])
            expected_commit_body["phase"] = {"phase": "commit", "detail": None}
            assert commit["body"] == expected_commit_body
            for field in (
                "validator_set_hash_version",
                "validator_set_hash",
                "validator_set",
                "validator_set_pops",
            ):
                assert prepare[field] == commit[field]

            body = prepare["body"]
            assert body["round"] == expected_round
            assert body["epoch"] == expected_epoch
            assert body["source_id"] == receipt["source_id"]
            assert body["tx_entrypoint_hash"] == expected_entrypoint
            assert body["chain_id_hash"] == receipt["chain_id_hash"]
            assert body["plan_digest"] == receipt["plan_digest"]
            assert body["coordinator_lane_id"] == receipt["lane_id"]
            assert body["coordinator_dataspace_id"] == receipt["dataspace_id"]
            assert body["coordinator_lane_incarnation"] == receipt["lane_incarnation"]
            assert (
                body["authority_context_height"]
                == receipt["authority_context_height"]
            )
            assert (
                body["planned_coordinator_block_height"]
                == receipt["lane_block_height"]
            )
            assert body["coordinator_lane_block_view"] == receipt["lane_block_view"]
            assert (
                body["coordinator_proposal_hash"]
                == receipt["coordinator_proposal_hash"]
            )

            assert body["participant_lane_id"] == leg["lane_id"]
            assert body["participant_dataspace_id"] == leg["dataspace_id"]
            assert descriptor["lane_id"] == leg["lane_id"]
            assert descriptor["dataspace_id"] == leg["dataspace_id"]
            assert (
                descriptor["lane_incarnation"]
                == body["participant_lane_incarnation"]
            )
            assert descriptor["proposal_height"] == body["authority_context_height"]
            assert (
                descriptor["previous_lane_block_height"]
                == body["participant_previous_block_height"]
            )
            assert (
                descriptor["previous_lane_block_descriptor_hash"]
                == body["participant_previous_block_descriptor_hash"]
            )
            assert (
                descriptor["lane_block_height"]
                == body["participant_lane_block_height"]
            )
            assert (
                descriptor["lane_block_view"]
                == body["participant_lane_block_view"]
            )
            assert proposal["proposal_hash"] == body["participant_proposal_hash"]
            assert (
                descriptor["validator_set_hash_version"]
                == prepare["validator_set_hash_version"]
            )
            assert descriptor["validator_set_hash"] == prepare["validator_set_hash"]
            assert descriptor["validator_set"] == prepare["validator_set"]
            assert descriptor["validator_count"] == body["participant_validator_count"]
            assert descriptor["min_quorum"] == body["participant_min_quorum"]

            settlement = leg["participant_settlement"]
            settlement_receipts = settlement["receipts"]
            assert 1 <= len(settlement_receipts) <= MAX_GROUP_SOURCES
            settlement_sources = [
                settlement_receipt["source_id"]
                for settlement_receipt in settlement_receipts
            ]
            assert settlement_sources == source_ids
            assert settlement_sources == sorted(settlement_sources)
            assert len(set(settlement_sources)) == len(settlement_sources)
            assert settlement_sources.count(receipt["source_id"]) == 1
            assert settlement["tx_count"] == len(settlement_receipts)
            assert settlement["block_height"] == body["participant_lane_block_height"]
            assert settlement["lane_id"] == leg["lane_id"]
            assert settlement["dataspace_id"] == leg["dataspace_id"]
            assert (
                settlement["lane_incarnation"]
                == body["participant_lane_incarnation"]
            )
            for field in (
                "total_local_amount",
                "total_xor_due",
                "total_xor_after_haircut",
                "total_xor_variance",
            ):
                assert settlement[field] == "0"
            assert settlement["swap_metadata"] is None
            assert settlement["nexus_fee_receipts"] == []
            assert settlement["native_amx_receipts"] == []
            for settlement_receipt in settlement_receipts:
                assert settlement_receipt["source_id"] in source_ids
                assert settlement_receipt["timestamp_ms"] == body[
                    "authority_context_height"
                ]
                for field in (
                    "local_amount",
                    "xor_due",
                    "xor_after_haircut",
                    "xor_variance",
                ):
                    assert settlement_receipt[field] == "0"
            assert leg[
                "participant_settlement_hash"
            ] == compute_native_amx_participant_settlement_hash(settlement)
            assert (
                leg["participant_settlement_hash"]
                == body["participant_settlement_commitment"]
            )

            accepted = descriptor["accepted_transaction_hashes"]
            entrypoint_position = (
                accepted.index(expected_entrypoint)
                if expected_entrypoint in accepted
                else None
            )
            if entrypoint_position is not None:
                assert len(accepted) == len(settlement_receipts)
                assert len(descriptor["accepted_candidate_indices"]) == len(
                    settlement_receipts
                )
                assert (
                    settlement_receipts[entrypoint_position]["source_id"]
                    == body["source_id"]
                )
            same_route = (
                leg["lane_id"],
                leg["dataspace_id"],
            ) == (receipt["lane_id"], receipt["dataspace_id"])
            if same_route:
                assert entrypoint_position is not None
                assert descriptor["lane_incarnation"] == receipt["lane_incarnation"]
                assert (
                    descriptor["proposal_height"]
                    == receipt["authority_context_height"]
                )
                assert descriptor["lane_block_height"] == receipt["lane_block_height"]
                assert descriptor["lane_block_view"] == receipt["lane_block_view"]
                assert (
                    proposal["proposal_hash"]
                    == receipt["coordinator_proposal_hash"]
                )


def _validate_application_evidence(document: dict[str, Any]) -> None:
    golden = document["golden"]
    group = golden["receipt_group"]
    evidence = golden["application_evidence"]
    execution = evidence["execution_commitment"]
    merge_carrier = execution["merge_carrier"]
    assert set(merge_carrier) == {"entry_hash", "version"}
    assert merge_carrier["version"] == 1
    assert merge_carrier["entry_hash"].startswith("hash:")
    artifacts = evidence["manifest_artifacts"]
    assert execution["native_amx_application_manifest_version"] == 1
    assert execution["native_amx_application_manifest_count"] == len(artifacts) == 1
    artifact = artifacts[0]
    leaf = artifact["leaf"]
    proof = artifact["proof"]
    assert artifact["version"] == leaf["version"] == 1
    assert artifact["leaf_index"] == proof["leaf_index"] == 0
    assert proof["audit_path"] == []
    assert artifact["manifest_leaf_count"] == 1
    assert (
        artifact["manifest_root"]
        == execution["native_amx_application_manifest_root"]
        == artifact["leaf_hash"]
    )
    assert leaf["executed_block_wire_hash"] == execution["executed_block_wire_hash"]
    assert execution["executed_block_wire_len"] == 49
    assert leaf["predecessor_height"] + 1 == leaf["participant_height"]
    assert evidence["active_lane_incarnations"] == [
        {
            "lane_id": leaf["lane_id"],
            "dataspace_id": leaf["dataspace_id"],
            "lane_incarnation": leaf["lane_incarnation"],
        }
    ]
    assert (leaf["lane_id"], leaf["dataspace_id"]) != (
        group["lane_id"],
        group["dataspace_id"],
    )
    members = leaf["members"]
    receipts = group["native_amx_receipts"]
    assert 1 <= len(members) <= 4_096
    assert [member["source_id"] for member in members] == [
        receipt["source_id"] for receipt in receipts
    ]
    carrier = set(evidence["carrier_entrypoint_hashes"])
    for member, receipt in zip(members, receipts):
        leg = next(
            leg
            for leg in receipt["legs"]
            if (leg["lane_id"], leg["dataspace_id"])
            == (leaf["lane_id"], leaf["dataspace_id"])
        )
        descriptor = leg["participant_proposal"]["descriptor"]
        assert descriptor["lane_incarnation"] == leaf["lane_incarnation"]
        assert descriptor["descriptor_hash"] == leaf["descriptor_hash"]
        assert leg["participant_proposal"]["proposal_hash"] == leaf["proposal_hash"]
        assert leg["participant_settlement_hash"] == leaf["settlement_hash"]
        assert leg["prepare_qc"]["body"]["source_id"] == member["source_id"]
        assert (
            leg["prepare_qc"]["body"]["tx_entrypoint_hash"]
            == member["entrypoint_hash"]
        )
        assert set(descriptor["accepted_transaction_hashes"]) <= carrier
    row = golden["expected_diagnostics"]["native_amx_participant_applications"][0]
    for field in (
        "lane_id",
        "dataspace_id",
        "lane_incarnation",
        "participant_height",
        "participant_view",
        "predecessor_height",
        "predecessor_descriptor_hash",
        "descriptor_hash",
        "proposal_hash",
        "settlement_hash",
        "application_block_height",
        "application_block_hash",
    ):
        assert row.get(field) == leaf.get(field)
    assert row["source_count"] == len(members)


def _validate_schema(
    value: Any,
    schema: dict[str, Any],
    components: dict[str, Any],
    path: str,
) -> None:
    reference = schema.get("$ref")
    if reference is not None:
        prefix = "#/components/schemas/"
        assert reference.startswith(prefix), f"{path}: unsupported reference {reference}"
        _validate_schema(value, components[reference[len(prefix) :]], components, path)
        return

    alternatives = schema.get("oneOf") or schema.get("anyOf")
    if alternatives is not None:
        failures: list[str] = []
        for alternative in alternatives:
            try:
                _validate_schema(value, alternative, components, path)
                return
            except AssertionError as error:
                failures.append(str(error))
        raise AssertionError(f"{path}: no schema alternative accepted value: {failures}")

    expected_type = schema.get("type")
    if expected_type == "object":
        assert isinstance(value, dict), f"{path}: expected object"
        properties = schema.get("properties", {})
        missing = set(schema.get("required", ())) - value.keys()
        assert not missing, f"{path}: missing required fields {sorted(missing)}"
        if schema.get("additionalProperties") is False:
            unknown = value.keys() - properties.keys()
            assert not unknown, f"{path}: unknown fields {sorted(unknown)}"
        for key, child in value.items():
            if key in properties:
                _validate_schema(child, properties[key], components, f"{path}.{key}")
    elif expected_type == "array":
        assert isinstance(value, list), f"{path}: expected array"
        assert len(value) >= schema.get("minItems", 0), f"{path}: below minItems"
        maximum = schema.get("maxItems")
        assert maximum is None or len(value) <= maximum, f"{path}: above maxItems"
        if schema.get("uniqueItems"):
            canonical = [json.dumps(item, sort_keys=True) for item in value]
            assert len(canonical) == len(set(canonical)), f"{path}: items are not unique"
        item_schema = schema.get("items")
        if item_schema is not None:
            for index, item in enumerate(value):
                _validate_schema(item, item_schema, components, f"{path}[{index}]")
    elif expected_type == "integer":
        assert isinstance(value, int) and not isinstance(value, bool), (
            f"{path}: expected integer"
        )
        assert value >= schema.get("minimum", value), f"{path}: below minimum"
        assert value <= schema.get("maximum", value), f"{path}: above maximum"
    elif expected_type == "boolean":
        assert isinstance(value, bool), f"{path}: expected boolean"
    elif expected_type == "string":
        assert isinstance(value, str), f"{path}: expected string"
        assert len(value) >= schema.get("minLength", 0), f"{path}: below minLength"
        maximum = schema.get("maxLength")
        assert maximum is None or len(value) <= maximum, f"{path}: above maxLength"
        pattern = schema.get("pattern")
        assert pattern is None or re.fullmatch(pattern, value), (
            f"{path}: does not match {pattern}"
        )
    elif expected_type == "null":
        assert value is None, f"{path}: expected null"

    if "enum" in schema:
        assert value in schema["enum"], f"{path}: value is not in enum"
    if "const" in schema:
        assert value == schema["const"], f"{path}: value does not equal const"


def test_grouped_native_amx_v2_fixture_matches_current_openapi() -> None:
    fixture = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    diagnostics = fixture["golden"]["expected_diagnostics"]
    group = fixture["golden"]["receipt_group"]
    assert diagnostics["lane_settlement_commitments"] == [group]
    assert len(group["native_amx_receipts"]) == 2
    assert fixture["golden"]["ordered_source_ids"] == [
        receipt["source_id"] for receipt in group["native_amx_receipts"]
    ]
    expected_settlement_hashes = {
        (7, 11): "hash:C6B18DBE6BEC468DB021B79604233F3CB9E2D6CDF3384C491CE7A6DA89747825#9D72",
        (8, 12): "hash:40C7FCA7AA143B323B473A9958B96F49896C03C3547B83DD340FAE2FC1A85D29#B452",
    }
    for leg in group["native_amx_receipts"][0]["legs"]:
        expected = expected_settlement_hashes[
            (leg["lane_id"], leg["dataspace_id"])
        ]
        assert leg["participant_settlement_hash"] == expected
        assert (
            compute_native_amx_participant_settlement_hash(
                leg["participant_settlement"]
            )
            == expected
        )
    _validate_receipt_group(fixture)
    _validate_application_evidence(fixture)

    schemas_by_path = []
    for openapi_path in OPENAPI_PATHS:
        openapi = json.loads(openapi_path.read_text(encoding="utf-8"))
        components = openapi["components"]["schemas"]
        _validate_schema(
            diagnostics,
            components["SumeragiDiagnosticsResponse"],
            components,
            "SumeragiDiagnosticsResponse",
        )
        schemas_by_path.append(
            {
                name: components[name]
                for name in (
                    "NativeAmxPhase",
                    "NativeAmxAttestationBody",
                    "NativeAmxAttestationQc",
                    "NativeAmxLegRecord",
                    "NativeAmxParticipantLaneBlockDescriptor",
                    "NativeAmxParticipantLaneBlockProposal",
                    "NativeAmxParticipantSettlementCommitment",
                    "NativeAmxParticipantSettlementReceipt",
                    "NativeAmxReceipt",
                    "SumeragiNativeAmxParticipantApplication",
                )
            }
        )

    assert schemas_by_path[0] == schemas_by_path[1]


def test_native_amx_openapi_expresses_direct_group_bounds() -> None:
    projections = []
    for openapi_path in OPENAPI_PATHS:
        schemas = json.loads(openapi_path.read_text(encoding="utf-8"))[
            "components"
        ]["schemas"]
        descriptor = schemas["NativeAmxParticipantLaneBlockDescriptor"][
            "properties"
        ]
        for field in (
            "accepted_candidate_indices",
            "accepted_transaction_hashes",
        ):
            assert descriptor[field]["minItems"] == 1
            assert descriptor[field]["maxItems"] == MAX_GROUP_SOURCES
            assert descriptor[field]["uniqueItems"] is True

        participant = schemas["NativeAmxParticipantSettlementCommitment"]
        assert participant["additionalProperties"] is False
        participant_properties = participant["properties"]
        assert participant_properties["tx_count"]["minimum"] == 1
        assert participant_properties["tx_count"]["maximum"] == MAX_GROUP_SOURCES
        assert participant_properties["receipts"] == {
            "items": {
                "$ref": (
                    "#/components/schemas/"
                    "NativeAmxParticipantSettlementReceipt"
                )
            },
            "maxItems": MAX_GROUP_SOURCES,
            "minItems": 1,
            "type": "array",
            "uniqueItems": True,
        }
        for field in (
            "total_local_amount",
            "total_xor_due",
            "total_xor_after_haircut",
            "total_xor_variance",
        ):
            assert participant_properties[field] == {"const": "0"}
        assert participant_properties["swap_metadata"] == {"type": "null"}
        for field in ("nexus_fee_receipts", "native_amx_receipts"):
            assert participant_properties[field] == {
                "maxItems": 0,
                "type": "array",
            }

        receipt = schemas["NativeAmxParticipantSettlementReceipt"]
        assert receipt["additionalProperties"] is False
        for field in (
            "local_amount",
            "xor_due",
            "xor_after_haircut",
            "xor_variance",
        ):
            assert receipt["properties"][field] == {"const": "0"}

        leg = schemas["NativeAmxLegRecord"]["properties"]
        assert leg["participant_settlement"] == {
            "$ref": (
                "#/components/schemas/"
                "NativeAmxParticipantSettlementCommitment"
            )
        }
        outer_receipts = schemas["LaneSettlementCommitment"]["properties"][
            "native_amx_receipts"
        ]
        assert outer_receipts["maxItems"] == MAX_GROUP_SOURCES

        qc = schemas["NativeAmxAttestationQc"]["properties"]
        assert qc["validator_set"]["items"] == {
            "$ref": "#/components/schemas/SumeragiV2BlsValidatorId"
        }
        assert qc["validator_set_pops"]["items"] == {
            "$ref": "#/components/schemas/SumeragiV2BlsProof"
        }
        assert qc["bls_aggregate_signature"] == {
            "$ref": "#/components/schemas/SumeragiV2BlsProof"
        }
        projections.append(
            {
                "descriptor": descriptor,
                "participant": participant,
                "participant_receipt": receipt,
                "leg": leg,
                "outer_receipts": outer_receipts,
                "qc": qc,
            }
        )

    assert projections[0] == projections[1]


def test_sumeragi_status_and_diagnostics_openapi_surfaces_are_disjoint() -> None:
    authoritative_fields = {
        "protocol_version",
        "height_context_id",
        "height",
        "view",
        "phase",
        "leader",
        "locked_prepare_qc",
        "highest_prepare_qc",
        "last_timeout_certificate",
        "body_state",
        "pending_persistence_id",
        "last_committed_height",
        "last_committed_subject",
        "height_context",
        "last_commit_qc",
        "liveness",
    }
    diagnostic_fields = {
        "pipeline_execution",
        "tx_queue_depth",
        "lane_commitments",
        "dataspace_commitments",
        "lane_settlement_commitments",
        "lane_relay_envelopes",
        "native_amx_participant_applications",
    }

    projections = []
    for openapi_path in OPENAPI_PATHS:
        openapi = json.loads(openapi_path.read_text(encoding="utf-8"))
        schemas = openapi["components"]["schemas"]
        paths = openapi["paths"]
        status_ref = paths["/v1/sumeragi/status"]["get"]["responses"]["200"][
            "content"
        ]["application/json"]["schema"]["$ref"]
        diagnostics_ref = paths["/v1/sumeragi/diagnostics"]["get"]["responses"][
            "200"
        ]["content"]["application/json"]["schema"]["$ref"]
        assert status_ref == "#/components/schemas/SumeragiStatusResponse"
        assert (
            diagnostics_ref
            == "#/components/schemas/SumeragiDiagnosticsResponse"
        )

        status = schemas["SumeragiStatusResponse"]
        diagnostics = schemas["SumeragiDiagnosticsResponse"]
        assert authoritative_fields <= status["properties"].keys()
        assert "liveness" in status["required"]
        assert status["properties"]["liveness"] == {
            "$ref": "#/components/schemas/SumeragiV2LivenessStatus"
        }
        assert diagnostic_fields.isdisjoint(status["properties"])
        assert diagnostic_fields <= diagnostics["properties"].keys()
        assert authoritative_fields.isdisjoint(diagnostics["properties"])
        native_rows = diagnostics["properties"][
            "native_amx_participant_applications"
        ]
        assert native_rows["maxItems"] == 1_024
        assert native_rows["items"] == {
            "$ref": "#/components/schemas/SumeragiNativeAmxParticipantApplication"
        }
        native_row = schemas["SumeragiNativeAmxParticipantApplication"]
        assert native_row["properties"]["source_count"] == {
            "format": "uint64",
            "maximum": 4_096,
            "minimum": 1,
            "type": "integer",
        }
        assert native_row["properties"]["state"] == {
            "$ref": "#/components/schemas/SumeragiNativeAmxParticipantApplicationState"
        }
        assert schemas["SumeragiNativeAmxParticipantApplicationState"]["enum"] == [
            "certified_pending_carrier",
            "committed_evidence_pending",
            "durably_applied",
            "conflict",
        ]
        projections.append((status, diagnostics, native_row))

    assert projections[0] == projections[1]


def test_grouped_native_amx_v2_negative_control_contract_is_bounded() -> None:
    fixture = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    controls = fixture["negative_controls"]
    assert 12 <= len(controls) <= 64
    assert len({control["id"] for control in controls}) == len(controls)
    assert all(control["expectation"] == "reject" for control in controls)
    assert {
        control["validator"] for control in controls
    } == {"receipt_group", "application_evidence"}
    # Coherent committee and outer-group substitutions intentionally update
    # every mirrored identity field; keep the corpus bounded while allowing
    # those controls to isolate one invariant rather than fail accidentally.
    assert all(
        1
        <= len(control["mutations"])
        <= (9 if control["id"] == "coherent_forged_validator_set_hash" else 8)
        for control in controls
    )
    assert {
        mutation["op"]
        for control in controls
        for mutation in control["mutations"]
    } <= {"replace", "remove", "copy", "swap", "repeat"}
    assert all(
        mutation["path"].startswith(
            (
                "/golden/receipt_group/",
                "/golden/application_evidence/",
            )
        )
        for control in controls
        for mutation in control["mutations"]
    )
    assert {
        "coherent_unordered_validator_set",
        "under_quorum_bitmap",
        "out_of_range_bitmap",
        "zero_pop",
        "long_pop",
        "zero_aggregate_signature",
        "long_aggregate_signature",
        "duplicate_participant_leg",
        "duplicate_group_source",
        "group_source_overflow",
        "outer_group_source_reorder",
        "outer_group_source_substitution",
        "source_id_substituted_for_entrypoint_hash",
        "entrypoint_hash_substituted_for_source_id",
        "wrong_entrypoint_hash_checksum",
        "wrong_entrypoint_hash_marker",
        "stale_same_route_incarnation",
        "same_route_coordinator_view_drift",
        "same_route_mixed_role_deferral",
        "stale_participant_application_incarnation",
        "same_route_participant_application_marker",
        "unanchored_mixed_role_participant",
        "manifest_root_tampering",
        "manifest_proof_path_tampering",
        "manifest_proof_position_tampering",
        "application_block_substitution",
    } <= {control["id"] for control in controls}


def test_receipt_group_negative_controls_fail_closed_semantically() -> None:
    canonical = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    _validate_receipt_group(canonical)
    applied: set[str] = set()
    for control in canonical["negative_controls"]:
        if control["validator"] != "receipt_group":
            continue
        mutated = deepcopy(canonical)
        for mutation in control["mutations"]:
            _apply_mutation(mutated, mutation)
        try:
            _validate_receipt_group(mutated)
        except (AssertionError, KeyError, TypeError, ValueError):
            applied.add(control["id"])
            continue
        raise AssertionError(f"receipt group control passed: {control['id']}")

    expected = {
        control["id"]
        for control in canonical["negative_controls"]
        if control["validator"] == "receipt_group"
    }
    assert applied == expected


def test_receipt_group_dynamic_relationship_checks_are_bounded() -> None:
    canonical = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    descriptor_path = (
        "/golden/receipt_group/native_amx_receipts/0/legs/0/"
        "participant_proposal/descriptor"
    )
    cases = {
        "accepted_work_overflow": [
            {
                "op": "repeat",
                "path": f"{descriptor_path}/accepted_candidate_indices",
                "value": {"count": MAX_GROUP_SOURCES + 1, "source_index": 0},
            },
            {
                "op": "repeat",
                "path": f"{descriptor_path}/accepted_transaction_hashes",
                "value": {"count": MAX_GROUP_SOURCES + 1, "source_index": 0},
            },
        ],
        "duplicate_candidate_index": [
            {
                "op": "copy",
                "path": f"{descriptor_path}/accepted_candidate_indices/1",
                "value": {
                    "from": (
                        f"{descriptor_path}/accepted_candidate_indices/0"
                    )
                },
            }
        ],
        "duplicate_transaction_hash": [
            {
                "op": "copy",
                "path": f"{descriptor_path}/accepted_transaction_hashes/1",
                "value": {
                    "from": (
                        f"{descriptor_path}/accepted_transaction_hashes/0"
                    )
                },
            }
        ],
        "wrong_bitmap_length": [
            {
                "op": "replace",
                "path": (
                    "/golden/receipt_group/native_amx_receipts/0/legs/0/"
                    "prepare_qc/signers_bitmap"
                ),
                "value": [7, 0],
            }
        ],
    }
    assert len(cases) <= 8
    for case_id, mutations in cases.items():
        mutated = deepcopy(canonical)
        for mutation in mutations:
            _apply_mutation(mutated, mutation)
        try:
            _validate_receipt_group(mutated)
        except (AssertionError, KeyError, TypeError, ValueError):
            continue
        raise AssertionError(f"dynamic relationship control passed: {case_id}")


def test_application_evidence_negative_controls_fail_closed() -> None:
    canonical = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    for control in canonical["negative_controls"]:
        if control["validator"] != "application_evidence":
            continue
        mutated = deepcopy(canonical)
        for mutation in control["mutations"]:
            _apply_mutation(mutated, mutation)
        try:
            _validate_application_evidence(mutated)
        except (AssertionError, KeyError, StopIteration, TypeError, ValueError):
            continue
        raise AssertionError(f"application evidence control passed: {control['id']}")
