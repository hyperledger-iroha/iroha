"""Protocol-v2-only parsing tests for the Torii Sumeragi status model."""

from __future__ import annotations

import copy

import iroha_python
import iroha_python.client as client_module
import pytest

from iroha_python.client import (
    OperatorSigningContext,
    SumeragiDiagnosticsSnapshot,
    SumeragiStatusSnapshot,
    SumeragiV2BodyState,
    SumeragiV2ExecutionCommitment,
    SumeragiV2GlobalPhase,
    SumeragiV2LaneFinalityManifestCommitment,
    SumeragiV2MergeCarrierCommitment,
    SumeragiV2StatusPhase,
    ToriiClient,
)
from iroha_python.crypto import Ed25519KeyPair, NetworkId


_OPERATOR_CONTEXT = OperatorSigningContext(
    NetworkId.from_bytes(bytes([0xA5]) * 32),
    Ed25519KeyPair.from_private_key(bytes([0x0B]) * 32),
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


_NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT = (
    "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F"
)


def _subject(seed: int = 0x31) -> dict[str, str]:
    return {
        "parent_block_hash": _canonical_hash(seed),
        "block_hash": _canonical_hash(seed + 1),
        "payload_hash": _canonical_hash(seed + 2),
    }


def _execution_commitment(seed: int = 0x51) -> dict[str, object]:
    return {
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
        "executed_block_wire_len": 512,
        "executed_block_wire_hash": _canonical_hash(seed + 3),
    }


def _prepare_qc(view: int = 3) -> dict[str, object]:
    return {
        "round": {
            "context_id": [_canonical_hash(0x14)],
            "height": 15,
            "view": view,
        },
        "proposal_round": {
            "context_id": [_canonical_hash(0x14)],
            "height": 15,
            "view": view,
        },
        "phase": {"phase": "prepare", "details": None},
        "subject": _subject(),
        "execution_commitment": _execution_commitment(),
    }


def _healthy_status() -> dict[str, object]:
    prepare_qc = _prepare_qc()
    committed_subject = _subject(0x41)
    return {
        "protocol_version": 4,
        "node_fingerprint": _canonical_hash(0x11),
        "build_fingerprint": _canonical_hash(0x12),
        "config_fingerprint": _canonical_hash(0x13),
        "restart_required": False,
        "height_context_id": [_canonical_hash(0x14)],
        "height": 15,
        "view": 4,
        "phase": {"phase": "prepare", "details": None},
        "leader": 1,
        "locked_prepare_qc": copy.deepcopy(prepare_qc),
        "highest_prepare_qc": copy.deepcopy(prepare_qc),
        "last_timeout_certificate": {
            "round": {
                "context_id": [_canonical_hash(0x14)],
                "height": 15,
                "view": 3,
            },
            "highest_prepare_qc": copy.deepcopy(prepare_qc),
            "certificate_hash": _canonical_hash(0x21),
        },
        "body_state": {"state": "validated", "details": None},
        "pending_persistence_id": 17,
        "last_committed_height": 14,
        "last_committed_subject": committed_subject,
        "height_context": {
            "epoch": 1,
            "epoch_end_height": 20,
            "mode": {"mode": "permissioned", "details": None},
            "epoch_seed": bytes(range(32)).hex().upper(),
            "validator_count": 4,
            "quorum": {"min_signers": 3, "total_power": 4},
        },
        "last_commit_qc": {
            "certificate": {
                "round": {
                    "context_id": [_canonical_hash(0x22)],
                    "height": 14,
                    "view": 1,
                },
                "proposal_round": {
                    "context_id": [_canonical_hash(0x22)],
                    "height": 14,
                    "view": 1,
                },
                "phase": {"phase": "commit", "details": None},
                "subject": copy.deepcopy(committed_subject),
                "execution_commitment": _execution_commitment(0x61),
            },
            "validator_count": 4,
            "signer_count": 3,
            "min_signers": 3,
            "signed_power": 3,
            "total_power": 4,
        },
        "liveness": {
            "generation": 4,
            "prepare_quorums": [],
            "commit_quorums": [],
            "timeout_quorums": [],
            "outbound_intents": [
                {
                    "kind": {"kind": "commit_vote", "details": None},
                    "round": {
                        "context_id": [_canonical_hash(0x14)],
                        "height": 15,
                        "view": 4,
                    },
                    "proposal_round": {
                        "context_id": [_canonical_hash(0x14)],
                        "height": 15,
                        "view": 4,
                    },
                    "subject": _subject(),
                    "execution_commitment": _execution_commitment(),
                    "stage": {"stage": "sent", "details": None},
                }
            ],
            "work": {
                "candidate": {"stage": "idle", "details": None},
                "body_recovery": {"stage": "idle", "details": None},
                "body_store": {"stage": "idle", "details": None},
                "validation": {"stage": "complete", "details": None},
                "application": {"stage": "idle", "details": None},
                "successor_height": {"stage": "idle", "details": None},
            },
            "queues": [],
            "last_progress": None,
            "no_progress_age_ms": 0,
            "blocker": None,
            "ignore_counts": [],
        },
    }


def _healthy_diagnostics() -> dict[str, object]:
    return {
        "pipeline_execution": {
            "tx_vertices_total": 1,
            "tx_edges_total": 0,
            "overlay_count_total": 1,
            "overlay_instr_total": 2,
            "overlay_bytes_total": 128,
            "rbc_chunks_total": 1,
            "rbc_bytes_total": 256,
            "detached_prepared_total": 1,
            "detached_merged_total": 1,
            "detached_fallback_total": 0,
            "detached_fallback_fee_postprocessing_total": 0,
            "detached_fallback_user_executor_total": 0,
            "detached_fallback_durable_state_total": 0,
            "detached_fallback_unsupported_instruction_total": 0,
            "detached_fallback_rejected_eval_total": 0,
            "detached_fallback_overlay_error_total": 0,
            "quarantine_executed_total": 0,
        },
        "tx_queue_depth": 3,
        "tx_queue_capacity": 32,
        "tx_queue_retained_bytes": 4096,
        "tx_queue_max_retained_bytes": 65536,
        "tx_queue_saturated": False,
        "tx_queue_saturated_by_count": False,
        "tx_queue_saturated_by_bytes": False,
        "tx_queue_saturated_by_age": False,
        "tx_queue_oldest_queued_age_ms": 25,
        "npos": None,
        "lane_commitments": [],
        "dataspace_commitments": [],
        "lane_settlement_commitments": [],
        "lane_relay_envelopes": [],
        "lane_payload_ownerships": [],
        "committed_lane_blocks": [],
        "lane_block_sessions": [],
        "lane_governance_sealed_total": 0,
        "lane_governance_sealed_aliases": [],
        "lane_governance": [],
        "native_amx_participant_applications": [
            {
                "lane_id": 3,
                "dataspace_id": 8,
                "lane_incarnation": _canonical_hash(0x65),
                "participant_height": 8,
                "participant_view": 1,
                "predecessor_height": 7,
                "predecessor_descriptor_hash": _canonical_hash(0x68),
                "descriptor_hash": _canonical_hash(0x73),
                "proposal_hash": _canonical_hash(0x69),
                "settlement_hash": _canonical_hash(0x6B),
                "source_count": 2,
                "application_block_height": 15,
                "application_block_hash": _canonical_hash(0x79),
                "state": "durably_applied",
            }
        ],
        "autonomous_lane_executions": [
            {
                "lane_id": 3,
                "dataspace_id": 8,
                "lane_incarnation": _canonical_hash(0x65),
                "lane_block_height": 8,
                "lane_block_view": 1,
                "proposal_height": 10,
                "reservation_owner_hash": _canonical_hash(0x81),
                "proposal_identity_hash": _canonical_hash(0x82),
                "reservation_group_hash": _canonical_hash(0x83),
                "reservation_count": 2,
                "transaction_count": 2,
                "highest_durable_stage": "reservations_durable",
                "stuck_reason": "awaiting_executable_payload",
            }
        ],
    }


def test_status_parses_authoritative_reducer_state() -> None:
    status = SumeragiStatusSnapshot.from_payload(_healthy_status())

    assert status.protocol_version == 4
    assert status.restart_required is False
    assert status.height_context_id.hash == _canonical_hash(0x14)
    assert status.height == 15
    assert status.view == 4
    assert status.phase is SumeragiV2StatusPhase.PREPARE
    assert status.body_state is SumeragiV2BodyState.VALIDATED
    assert status.locked_prepare_qc is not None
    assert status.locked_prepare_qc.phase is SumeragiV2GlobalPhase.PREPARE
    assert status.locked_prepare_qc.round.view == 3
    assert status.locked_prepare_qc.proposal_round.view == 3
    expected_execution_commitment = SumeragiV2ExecutionCommitment.from_payload(
        _execution_commitment(), "expected_execution_commitment"
    )
    assert (
        status.locked_prepare_qc.execution_commitment
        == expected_execution_commitment
    )
    assert status.last_timeout_certificate is not None
    assert status.last_timeout_certificate.certificate_hash == _canonical_hash(0x21)
    assert status.last_timeout_certificate.highest_prepare_qc is not None
    assert (
        status.last_timeout_certificate.highest_prepare_qc.execution_commitment
        == expected_execution_commitment
    )
    assert status.pending_persistence_id == 17
    assert status.last_committed_subject is not None
    assert status.last_committed_subject.block_hash == _canonical_hash(0x42)
    assert status.height_context.validator_count == 4
    assert status.last_commit_qc is not None
    assert status.last_commit_qc.signed_power == 3
    assert (
        status.last_commit_qc.certificate.execution_commitment.parent_state_root
        == _canonical_hash(0x61)
    )
    assert len(status.liveness.outbound_intents) == 1
    outbound_intent = status.liveness.outbound_intents[0]
    assert outbound_intent.round.view == 4
    assert outbound_intent.proposal_round is not None
    assert outbound_intent.proposal_round.view == 4
    assert outbound_intent.execution_commitment is not None
    assert (
        outbound_intent.execution_commitment.executed_block_wire_hash
        == _canonical_hash(0x54)
    )
    assert not hasattr(status, "lane_payload_ownerships")
    assert not hasattr(status, "operator")


def test_diagnostics_parse_separately_from_authoritative_status() -> None:
    diagnostics = SumeragiDiagnosticsSnapshot.from_payload(_healthy_diagnostics())

    assert SumeragiDiagnosticsSnapshot is not SumeragiStatusSnapshot
    assert diagnostics.tx_queue_depth == 3
    assert diagnostics.pipeline_execution.tx_vertices_total == 1
    assert diagnostics.native_amx_participant_applications[0].state == "durably_applied"
    autonomous = diagnostics.autonomous_lane_executions[0]
    assert autonomous.proposal_identity_hash == _canonical_hash(0x82)
    assert autonomous.proposal_view is None
    assert autonomous.proposal_hash is None
    assert autonomous.stuck_reason == "awaiting_executable_payload"

    status = _healthy_status()
    status["lane_settlement_commitments"] = []
    with pytest.raises(RuntimeError, match="unknown field lane_settlement_commitments"):
        SumeragiStatusSnapshot.from_payload(status)

    missing_autonomous = _healthy_diagnostics()
    del missing_autonomous["autonomous_lane_executions"]
    with pytest.raises(
        RuntimeError,
        match="missing required field autonomous_lane_executions",
    ):
        SumeragiDiagnosticsSnapshot.from_payload(missing_autonomous)


def test_typed_endpoint_methods_reject_swapped_sumeragi_payloads(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[tuple[str, str, int]] = []
    payloads = {
        "/v1/sumeragi/status": _healthy_diagnostics(),
        "/v1/sumeragi/diagnostics": _healthy_status(),
    }
    client = ToriiClient(
        "http://node.test",
        max_retries=0,
        operator_signing_context=_OPERATOR_CONTEXT,
    )

    def get_sccp_json_object(
        path: str,
        *,
        context: str,
        maximum_body_bytes: int,
        parser: object,
    ) -> object:
        calls.append((path, context, maximum_body_bytes))
        return payloads[path]

    monkeypatch.setattr(
        client,
        "_get_sumeragi_operator_json_object",
        get_sccp_json_object,
    )

    with pytest.raises(RuntimeError, match="sumeragi status contains unknown field"):
        client.get_sumeragi_status_typed()
    with pytest.raises(
        RuntimeError, match="sumeragi diagnostics contains unknown field"
    ):
        client.get_sumeragi_diagnostics_typed()

    assert calls == [
        ("/v1/sumeragi/status", "sumeragi status", 1 * 1024 * 1024),
        (
            "/v1/sumeragi/diagnostics",
            "sumeragi diagnostics",
            16 * 1024 * 1024,
        ),
    ]

    response = client_module.requests.Response()
    response.status_code = 200
    response.headers["Content-Type"] = "application/json"
    response._content = b'{"receipt":{"version":1,"version":2}}'
    response._content_consumed = True
    strict_client = ToriiClient(
        "http://node.test",
        max_retries=0,
        operator_signing_context=_OPERATOR_CONTEXT,
    )
    monkeypatch.setattr(strict_client, "_request", lambda *args, **kwargs: response)
    with pytest.raises(ValueError, match="duplicate field `version`"):
        strict_client.get_sumeragi_diagnostics_typed()


def test_qc_reference_preserves_execution_commitment() -> None:
    payload = _prepare_qc()
    execution_commitment = payload["execution_commitment"]
    assert isinstance(execution_commitment, dict)
    execution_commitment["topup_anchor_root"] = _canonical_hash(0x55)
    execution_commitment["topup_anchor_count"] = 2

    qc = client_module.SumeragiV2QuorumCertificateRef.from_payload(
        payload, "test_qc"
    )

    assert qc.execution_commitment == SumeragiV2ExecutionCommitment(
        parent_state_root=_canonical_hash(0x51),
        post_state_root=_canonical_hash(0x52),
        ordinary_writes_root=_canonical_hash(0x53),
        topup_anchor_root=_canonical_hash(0x55),
        topup_anchor_count=2,
        native_amx_application_manifest_version=1,
        native_amx_application_manifest_root=(
            _NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT
        ),
        native_amx_application_manifest_count=0,
        lane_finality_manifest=None,
        merge_carrier=None,
        executed_block_wire_len=512,
        executed_block_wire_hash=_canonical_hash(0x54),
    )


def test_execution_commitment_accepts_nonempty_native_manifest() -> None:
    payload = _execution_commitment()
    payload["native_amx_application_manifest_root"] = _canonical_hash(0x55)
    payload["native_amx_application_manifest_count"] = 1

    commitment = SumeragiV2ExecutionCommitment.from_payload(
        payload, "test_commitment"
    )

    assert commitment.native_amx_application_manifest_root == _canonical_hash(0x55)
    assert commitment.native_amx_application_manifest_count == 1


def test_execution_commitment_requires_exact_lane_finality_manifest() -> None:
    payload = _execution_commitment()
    assert (
        SumeragiV2ExecutionCommitment.from_payload(payload, "test_commitment")
        .lane_finality_manifest
        is None
    )
    payload["lane_finality_manifest"] = {
        "root": _canonical_hash(0x56),
        "leaf_count": 1024,
    }
    commitment = SumeragiV2ExecutionCommitment.from_payload(
        payload, "test_commitment"
    )
    assert commitment.lane_finality_manifest == SumeragiV2LaneFinalityManifestCommitment(
        root=_canonical_hash(0x56), leaf_count=1024
    )
    invalid_manifests: tuple[object, ...] = (
        [],
        {},
        {"root": _canonical_hash(0x56), "leaf_count": 0},
        {"root": _canonical_hash(0x56), "leaf_count": 1025},
        {"root": "not-a-hash", "leaf_count": 1},
        {"root": _canonical_hash(0x56), "leaf_count": 1, "future": True},
    )
    invalid_payloads = []
    missing = _execution_commitment()
    del missing["lane_finality_manifest"]
    invalid_payloads.append(missing)
    for manifest in invalid_manifests:
        invalid = _execution_commitment()
        invalid["lane_finality_manifest"] = manifest
        invalid_payloads.append(invalid)
    for invalid in invalid_payloads:
        with pytest.raises((TypeError, ValueError)):
            SumeragiV2ExecutionCommitment.from_payload(invalid, "test_commitment")


def test_execution_commitment_requires_exact_merge_carrier_projection() -> None:
    payload = _execution_commitment()
    assert (
        SumeragiV2ExecutionCommitment.from_payload(payload, "test_commitment").merge_carrier
        is None
    )

    payload["merge_carrier"] = {
        "version": 1,
        "entry_hash": _canonical_hash(0x56),
    }
    commitment = SumeragiV2ExecutionCommitment.from_payload(
        payload, "test_commitment"
    )
    assert commitment.merge_carrier == SumeragiV2MergeCarrierCommitment(
        version=1,
        entry_hash=_canonical_hash(0x56),
    )

    invalid_payloads = []
    missing = _execution_commitment()
    del missing["merge_carrier"]
    invalid_payloads.append(missing)
    malformed = _execution_commitment()
    malformed["merge_carrier"] = "carrier"
    invalid_payloads.append(malformed)
    wrong_version = _execution_commitment()
    wrong_version["merge_carrier"] = {
        "version": 2,
        "entry_hash": _canonical_hash(0x56),
    }
    invalid_payloads.append(wrong_version)
    missing_version = _execution_commitment()
    missing_version["merge_carrier"] = {
        "entry_hash": _canonical_hash(0x56),
    }
    invalid_payloads.append(missing_version)
    missing_entry_hash = _execution_commitment()
    missing_entry_hash["merge_carrier"] = {"version": 1}
    invalid_payloads.append(missing_entry_hash)
    bad_hash = _execution_commitment()
    bad_hash["merge_carrier"] = {"version": 1, "entry_hash": "not-a-hash"}
    invalid_payloads.append(bad_hash)
    unknown = _execution_commitment()
    unknown["merge_carrier"] = {
        "version": 1,
        "entry_hash": _canonical_hash(0x56),
        "future": True,
    }
    invalid_payloads.append(unknown)

    for invalid in invalid_payloads:
        with pytest.raises((TypeError, ValueError)):
            SumeragiV2ExecutionCommitment.from_payload(invalid, "test_commitment")


@pytest.mark.parametrize("invalid", [None, True, 0, -1, 1 << 64, "512"])
def test_execution_commitment_requires_exact_executed_wire_len(invalid: object) -> None:
    payload = _execution_commitment()
    commitment = SumeragiV2ExecutionCommitment.from_payload(payload, "test_commitment")
    assert commitment.executed_block_wire_len == 512

    payload["executed_block_wire_len"] = invalid
    with pytest.raises((TypeError, ValueError), match="executed_block_wire_len"):
        SumeragiV2ExecutionCommitment.from_payload(payload, "test_commitment")

    del payload["executed_block_wire_len"]
    with pytest.raises((TypeError, ValueError), match="executed_block_wire_len"):
        SumeragiV2ExecutionCommitment.from_payload(payload, "test_commitment")


@pytest.mark.parametrize(
    ("mutate", "error"),
    [
        (
            lambda payload: payload.update(
                native_amx_application_manifest_version=2
            ),
            "native_amx_application_manifest_version must equal 1",
        ),
        (
            lambda payload: payload.update(
                native_amx_application_manifest_count=1025
            ),
            "native_amx_application_manifest_count",
        ),
        (
            lambda payload: payload.update(
                native_amx_application_manifest_root=_canonical_hash(0x55)
            ),
            "must be zero exactly for the canonical empty root",
        ),
        (
            lambda payload: payload.update(
                native_amx_application_manifest_count=1
            ),
            "must be zero exactly for the canonical empty root",
        ),
    ],
)
def test_execution_commitment_rejects_invalid_native_manifest(
    mutate, error: str
) -> None:
    payload = _execution_commitment()
    mutate(payload)

    with pytest.raises((TypeError, ValueError), match=error):
        SumeragiV2ExecutionCommitment.from_payload(payload, "test_commitment")


def test_status_allows_genesis_without_optional_certificates() -> None:
    payload = _healthy_status()
    liveness = payload["liveness"]
    assert isinstance(liveness, dict)
    liveness["outbound_intents"] = []
    payload.update(
        {
            "height": 0,
            "view": 0,
            "phase": {"phase": "awaiting_proposal", "details": None},
            "body_state": {"state": "missing", "details": None},
            "last_committed_height": 0,
            "last_committed_subject": None,
            "last_commit_qc": None,
            "pending_persistence_id": None,
            "locked_prepare_qc": None,
            "highest_prepare_qc": None,
            "last_timeout_certificate": None,
        }
    )

    status = SumeragiStatusSnapshot.from_payload(payload)

    assert status.phase is SumeragiV2StatusPhase.AWAITING_PROPOSAL
    assert status.body_state is SumeragiV2BodyState.MISSING
    assert status.last_committed_subject is None


def test_status_allows_authenticated_bootstrap_without_commit_details() -> None:
    payload = _healthy_status()
    payload["last_committed_subject"] = None
    payload["last_commit_qc"] = None

    status = SumeragiStatusSnapshot.from_payload(payload)

    assert status.last_committed_height == 14
    assert status.last_committed_subject is None
    assert status.last_commit_qc is None


def test_status_allows_subject_without_parent_hash() -> None:
    payload = _healthy_status()
    subject = payload["last_committed_subject"]
    assert isinstance(subject, dict)
    subject["parent_block_hash"] = None
    commit_qc = payload["last_commit_qc"]
    assert isinstance(commit_qc, dict)
    certificate = commit_qc["certificate"]
    assert isinstance(certificate, dict)
    certified_subject = certificate["subject"]
    assert isinstance(certified_subject, dict)
    certified_subject["parent_block_hash"] = None

    status = SumeragiStatusSnapshot.from_payload(payload)

    assert status.last_committed_subject is not None
    assert status.last_committed_subject.parent_block_hash is None


def test_retired_global_sumeragi_rbc_and_collectors_surfaces_are_absent() -> None:
    retired_methods = (
        "get_sumeragi_rbc",
        "get_sumeragi_rbc_typed",
        "get_sumeragi_rbc_sessions",
        "get_sumeragi_rbc_sessions_typed",
        "find_sumeragi_rbc_sampling_candidate",
        "find_sumeragi_rbc_sampling_candidate_typed",
        "get_sumeragi_rbc_delivered",
        "get_sumeragi_rbc_delivered_typed",
        "request_sumeragi_rbc_sample",
        "request_sumeragi_rbc_sample_typed",
        "get_sumeragi_collectors",
        "get_sumeragi_collectors_typed",
    )
    for name in retired_methods:
        assert not hasattr(ToriiClient, name), name

    retired_models = (
        "SumeragiRbcSnapshot",
        "SumeragiRbcSession",
        "SumeragiRbcSessionsSnapshot",
        "SumeragiRbcDeliveryStatus",
        "SumeragiCollectorEntry",
        "SumeragiCollectorPlan",
        "RbcSample",
        "RbcChunkProof",
        "RbcMerkleProof",
    )
    for name in retired_models:
        assert not hasattr(client_module, name), name
        assert name not in client_module.__all__, name
        assert not hasattr(iroha_python, name), name
        assert name not in iroha_python.__all__, name

    retained_telemetry_models = (
        "SumeragiAvailabilityCollector",
        "SumeragiRbcBacklog",
        "SumeragiRbcEviction",
        "SumeragiRbcStoreStatus",
    )
    for name in retained_telemetry_models:
        assert hasattr(client_module, name), name
        assert hasattr(iroha_python, name), name


def test_retained_rbc_store_telemetry_models_parse_snapshot() -> None:
    status = client_module.SumeragiRbcStoreStatus.from_payload(
        {
            "sessions": 3,
            "bytes": 4096,
            "pressure_level": 1,
            "backpressure_deferrals_total": 2,
            "persist_drops_total": 4,
            "evictions_total": 5,
            "recent_evictions": [
                {
                    "block_hash": "hash:EVICTED#0001",
                    "height": 14,
                    "view": 3,
                }
            ],
        }
    )

    assert status.sessions == 3
    assert status.bytes == 4096
    assert status.recent_evictions == [
        client_module.SumeragiRbcEviction(
            block_hash="hash:EVICTED#0001",
            height=14,
            view=3,
        )
    ]


@pytest.mark.parametrize(
    ("mutate", "error"),
    [
        (lambda payload: payload.update(protocol_version=3), "must equal 4"),
        (
            lambda payload: payload.pop("restart_required"),
            "restart_required must be a boolean",
        ),
        (
            lambda payload: payload.update(restart_required=0),
            "restart_required must be a boolean",
        ),
        (
            lambda payload: payload.update(pending_rbc={"sessions": 0}),
            "contains unknown field pending_rbc",
        ),
        (
            lambda payload: payload.update(
                phase={"phase": "prepare", "details": {}}
            ),
            "details must be explicitly null",
        ),
        (
            lambda payload: payload.update(last_committed_height=16),
            "must not exceed height",
        ),
        (
            lambda payload: payload.update(
                phase={"phase": "Prepare", "details": None}
            ),
            "not a supported v2 variant",
        ),
        (
            lambda payload: payload["height_context"]["quorum"].update(
                min_signers=2
            ),
            "quorum is not canonical",
        ),
        (
            lambda payload: payload["last_commit_qc"].update(signed_power=2),
            "exact frozen certificate quorum",
        ),
        (
            lambda payload: payload["last_commit_qc"].update(
                signer_count=4, signed_power=4
            ),
            "exact frozen certificate quorum",
        ),
        (
            lambda payload: payload["locked_prepare_qc"].pop("proposal_round"),
            "proposal_round",
        ),
        (
            lambda payload: payload["locked_prepare_qc"].pop(
                "execution_commitment"
            ),
            "execution_commitment",
        ),
        (
            lambda payload: payload.update(last_committed_subject=None),
            "committed subject and QC are required",
        ),
    ],
)
def test_status_rejects_malformed_or_legacy_state(mutate, error: str) -> None:
    payload = _healthy_status()
    mutate(payload)

    with pytest.raises(RuntimeError, match=error):
        SumeragiStatusSnapshot.from_payload(payload)
