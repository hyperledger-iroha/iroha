"""Authoritative Sumeragi v2 status parsing and adversarial rejection tests."""

from __future__ import annotations

from copy import deepcopy
from typing import Any

import pytest

from iroha_python import SumeragiStatusSnapshot, SumeragiV2Status, ToriiClient


def _crc16(value: bytes) -> int:
    crc = 0xFFFF
    for byte in value:
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return crc


def _hash(seed: int) -> str:
    body = (bytes([seed]) * 31 + bytes([seed | 1])).hex().upper()
    checksum = _crc16(f"hash:{body}".encode("ascii"))
    return f"hash:{body}#{checksum:04X}"


def _payload() -> dict[str, Any]:
    subject = {
        "parent_block_hash": _hash(0x31),
        "block_hash": _hash(0x32),
        "payload_hash": _hash(0x33),
    }
    return {
        "protocol_version": 2,
        "node_fingerprint": _hash(0x11),
        "build_fingerprint": _hash(0x12),
        "config_fingerprint": _hash(0x13),
        "height_context_id": [_hash(0x14)],
        "height": 10,
        "view": 2,
        "phase": {"phase": "prepare", "details": None},
        "leader": 1,
        "locked_prepare_qc": None,
        "highest_prepare_qc": None,
        "last_timeout_certificate": None,
        "body_state": {"state": "validated", "details": None},
        "pending_persistence_id": None,
        "last_committed_height": 9,
        "last_committed_subject": subject,
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
                    "context_id": [_hash(0x41)],
                    "height": 9,
                    "view": 1,
                },
                "phase": {"phase": "commit", "details": None},
                "subject": dict(subject),
            },
            "validator_count": 4,
            "signer_count": 3,
            "min_signers": 3,
            "signed_power": 3,
            "total_power": 4,
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


def test_status_snapshot_is_the_shared_strict_v2_model() -> None:
    status = SumeragiStatusSnapshot.from_payload(_payload())

    assert isinstance(status, SumeragiV2Status)
    assert status.protocol_version == 2
    assert status.height_context.mode == "permissioned"
    assert status.last_commit_qc is not None
    assert status.last_commit_qc.certificate.subject == status.last_committed_subject
    assert status.operator.tx_queue.queued_transactions == 3


def test_typed_client_uses_the_authoritative_v2_decoder(monkeypatch: Any) -> None:
    payload = _payload()
    client = object.__new__(ToriiClient)
    monkeypatch.setattr(
        ToriiClient,
        "request_json",
        lambda _self, *_args, **_kwargs: payload,
    )

    status = client.get_sumeragi_status_typed()

    assert status.protocol_version == 2
    assert status.height == 10


@pytest.mark.parametrize(
    "mutate",
    [
        lambda payload: payload.__setitem__("protocol_version", 1),
        lambda payload: payload.__setitem__("leader_index", 1),
        lambda payload: payload["height_context"]["quorum"].__setitem__(
            "min_signers", 2
        ),
        lambda payload: payload["height_context"].__setitem__(
            "epoch_end_height", 9
        ),
        lambda payload: payload.__setitem__("leader", 4),
        lambda payload: payload["last_commit_qc"].__setitem__("signed_power", 2),
        lambda payload: payload["last_commit_qc"]["certificate"].__setitem__(
            "subject",
            {
                **payload["last_committed_subject"],
                "block_hash": _hash(0x77),
            },
        ),
        lambda payload: payload["operator"]["adapter_queues"].__setitem__(
            "ingress_keys", 17
        ),
        lambda payload: payload["operator"]["tx_queue"].__setitem__(
            "queued_transactions", 6
        ),
        lambda payload: payload.pop("lane_block_sessions"),
    ],
)
def test_status_snapshot_rejects_legacy_or_inconsistent_payloads(mutate: Any) -> None:
    payload = deepcopy(_payload())
    mutate(payload)

    with pytest.raises(RuntimeError):
        SumeragiStatusSnapshot.from_payload(payload)
