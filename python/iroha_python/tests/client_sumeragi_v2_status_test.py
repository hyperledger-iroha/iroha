"""Protocol-v2-only parsing tests for the Torii Sumeragi status model."""

from __future__ import annotations

import copy

import iroha_python
import iroha_python.client as client_module
import pytest

from iroha_python.client import (
    SumeragiStatusSnapshot,
    SumeragiV2BodyState,
    SumeragiV2GlobalPhase,
    SumeragiV2StatusPhase,
    ToriiClient,
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


def _subject(seed: int = 0x31) -> dict[str, str]:
    return {
        "parent_block_hash": _canonical_hash(seed),
        "block_hash": _canonical_hash(seed + 1),
        "payload_hash": _canonical_hash(seed + 2),
    }


def _prepare_qc(view: int = 3) -> dict[str, object]:
    return {
        "round": {
            "context_id": [_canonical_hash(0x14)],
            "height": 15,
            "view": view,
        },
        "phase": {"phase": "prepare", "details": None},
        "subject": _subject(),
    }


def _healthy_status() -> dict[str, object]:
    prepare_qc = _prepare_qc()
    committed_subject = _subject(0x41)
    return {
        "protocol_version": 3,
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
                "phase": {"phase": "commit", "details": None},
                "subject": copy.deepcopy(committed_subject),
            },
            "validator_count": 4,
            "signer_count": 3,
            "min_signers": 3,
            "signed_power": 3,
            "total_power": 4,
        },
        "safety_halt": {
            "active": False,
            "reason": None,
            "height": 0,
            "epoch": 0,
            "first_block_hash": None,
            "conflicting_block_hash": None,
            "first_parent_state_root": None,
            "first_post_state_root": None,
            "conflicting_parent_state_root": None,
            "conflicting_post_state_root": None,
        },
        "lane_settlement_commitments": [],
        "lane_relay_envelopes": [],
        "lane_payload_ownerships": [],
        "committed_lane_blocks": [],
        "lane_block_sessions": [],
        "local_peer_removed": False,
        "operator": {
            "view_change_install_total": 7,
            "busy_deferral_total": 3,
            "adapter_queues": {
                "ingress_keys": 2,
                "ingress_capacity": 16,
                "deferred_completion": 1,
                "deferred_progress": 2,
                "deferred_progress_capacity": 4,
                "deferred_normal": 3,
                "deferred_normal_capacity": 8,
            },
            "tx_queue": {
                "tracked_transactions": 5,
                "queued_transactions": 3,
                "capacity": 32,
                "retained_bytes": 4096,
                "max_retained_bytes": 65536,
                "oldest_queued_age_ms": 25,
                "saturated_by_count": False,
                "saturated_by_bytes": False,
                "saturated_by_age": False,
            },
        },
    }


def test_status_parses_authoritative_reducer_state() -> None:
    status = SumeragiStatusSnapshot.from_payload(_healthy_status())

    assert status.protocol_version == 3
    assert status.restart_required is False
    assert status.height_context_id.hash == _canonical_hash(0x14)
    assert status.height == 15
    assert status.view == 4
    assert status.phase is SumeragiV2StatusPhase.PREPARE
    assert status.body_state is SumeragiV2BodyState.VALIDATED
    assert status.locked_prepare_qc is not None
    assert status.locked_prepare_qc.phase is SumeragiV2GlobalPhase.PREPARE
    assert status.locked_prepare_qc.round.view == 3
    assert status.last_timeout_certificate is not None
    assert status.last_timeout_certificate.certificate_hash == _canonical_hash(0x21)
    assert status.pending_persistence_id == 17
    assert status.last_committed_subject is not None
    assert status.last_committed_subject.block_hash == _canonical_hash(0x42)
    assert status.height_context.validator_count == 4
    assert status.last_commit_qc is not None
    assert status.last_commit_qc.signed_power == 3
    assert status.safety_halt.active is False
    assert status.lane_payload_ownerships == []
    assert status.committed_lane_blocks == []
    assert status.lane_block_sessions == []
    assert status.local_peer_removed is False
    assert status.operator.tx_queue.queued_transactions == 3


def test_status_allows_genesis_without_optional_certificates() -> None:
    payload = _healthy_status()
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
        (lambda payload: payload.update(protocol_version=1), "must equal 3"),
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
            "does not satisfy its frozen dual quorum",
        ),
        (
            lambda payload: payload["operator"]["tx_queue"].update(
                queued_transactions=6
            ),
            "tx_queue occupancy exceeds capacity",
        ),
        (
            lambda payload: payload.pop("lane_payload_ownerships"),
            "lane_payload_ownerships must be an array",
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
