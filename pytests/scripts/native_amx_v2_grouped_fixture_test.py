"""OpenAPI parity for the Rust-owned grouped Native AMX v2 fixture."""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
import re
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_PATH = (
    REPO_ROOT / "fixtures" / "sumeragi_v2" / "native_amx_v2_grouped.json"
)
OPENAPI_PATHS = (
    REPO_ROOT / "docs" / "portal" / "static" / "openapi" / "torii.json",
    REPO_ROOT
    / "docs"
    / "portal"
    / "static"
    / "openapi"
    / "versions"
    / "current"
    / "torii.json",
)

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


def _validate_application_evidence(document: dict[str, Any]) -> None:
    golden = document["golden"]
    group = golden["receipt_group"]
    evidence = golden["application_evidence"]
    execution = evidence["execution_commitment"]
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
        pattern = schema.get("pattern")
        assert pattern is None or re.fullmatch(pattern, value), (
            f"{path}: does not match {pattern}"
        )
    elif expected_type == "null":
        assert value is None, f"{path}: expected null"

    if "enum" in schema:
        assert value in schema["enum"], f"{path}: value is not in enum"


def test_grouped_native_amx_v2_fixture_matches_current_openapi() -> None:
    fixture = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    diagnostics = fixture["golden"]["expected_diagnostics"]
    group = fixture["golden"]["receipt_group"]
    assert diagnostics["lane_settlement_commitments"] == [group]
    assert len(group["native_amx_receipts"]) == 2
    assert fixture["golden"]["ordered_source_ids"] == [
        receipt["source_id"] for receipt in group["native_amx_receipts"]
    ]
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
                    "NativeAmxReceipt",
                    "SumeragiNativeAmxParticipantApplication",
                )
            }
        )

    assert schemas_by_path[0] == schemas_by_path[1]


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
    assert all(1 <= len(control["mutations"]) <= 8 for control in controls)
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
        "zero_pop",
        "long_pop",
        "zero_aggregate_signature",
        "long_aggregate_signature",
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
