from __future__ import annotations

from iroha_python import (
    SumeragiCommitQcRecord,
    SumeragiPhasesSnapshot,
    SumeragiStatusSnapshot,
    ToriiClient,
)
from iroha_python.client import SumeragiLaneRelayEnvelope


def test_sumeragi_phases_snapshot_parses_without_exec_witness() -> None:
    payload = {
        "propose_ms": 1,
        "collect_da_ms": 2,
        "collect_prevote_ms": 3,
        "collect_precommit_ms": 4,
        "collect_aggregator_ms": 5,
        "commit_ms": 6,
        "pipeline_total_ms": 7,
        "collect_aggregator_gossip_total": 8,
        "block_created_dropped_by_lock_total": 9,
        "block_created_hint_mismatch_total": 10,
        "block_created_proposal_mismatch_total": 11,
        "ema_ms": {
            "propose_ms": 12,
            "collect_da_ms": 13,
            "collect_prevote_ms": 14,
            "collect_precommit_ms": 15,
            "collect_aggregator_ms": 16,
            "commit_ms": 17,
            "pipeline_total_ms": 18,
        },
    }

    snapshot = SumeragiPhasesSnapshot.from_payload(payload)

    assert snapshot.collect_aggregator_ms == 5
    assert snapshot.ema_ms.pipeline_total_ms == 18


def test_sumeragi_lane_relay_envelope_parses_qc_payload() -> None:
    payload = {
        "lane_id": 1,
        "dataspace_id": 2,
        "block_height": 3,
        "block_header": {"height": 3},
        "qc": {"subject_block_hash": "feedface"},
        "da_commitment_hash": "0xcafe",
        "settlement_commitment": {
            "block_height": 3,
            "lane_id": 1,
            "dataspace_id": 2,
            "tx_count": 1,
            "total_local_micro": 10,
            "total_xor_due_micro": 5,
            "total_xor_after_haircut_micro": 4,
            "total_xor_variance_micro": 1,
            "receipts": [
                {
                    "source_id": "0xabc",
                    "local_amount_micro": 10,
                    "xor_due_micro": 5,
                    "xor_after_haircut_micro": 4,
                    "xor_variance_micro": 1,
                    "timestamp_ms": 1700,
                }
            ],
        },
        "settlement_hash": "0xdead",
        "rbc_bytes_total": 128,
    }

    envelope = SumeragiLaneRelayEnvelope.from_payload(payload)

    assert envelope.qc == {"subject_block_hash": "feedface"}
    assert envelope.settlement_hash == "0xdead"


def test_sumeragi_status_parses_commit_qc_and_quorum() -> None:
    payload = {
        "leader_index": 1,
        "view_change_index": 2,
        "highest_qc": {"height": 1, "view": 1, "subject_block_hash": "0xaaa"},
        "locked_qc": {"height": 1, "view": 1, "subject_block_hash": "0xbbb"},
        "commit_qc": {
            "height": 3,
            "view": 2,
            "epoch": 1,
            "block_hash": "0xccc",
            "validator_set_hash": "0xddd",
            "validator_set_len": 4,
            "signatures_total": 3,
        },
        "commit_quorum": {
            "height": 3,
            "view": 2,
            "block_hash": "0xccc",
            "signatures_present": 3,
            "signatures_counted": 3,
            "signatures_set_b": 2,
            "signatures_required": 3,
            "last_updated_ms": 1700,
        },
        "tx_queue": {"depth": 0, "saturated": False},
        "epoch": {"length_blocks": 1, "commit_deadline_offset": 0, "reveal_deadline_offset": 0},
        "rbc_store": {
            "sessions": 0,
            "bytes": 0,
            "pressure_level": 0,
            "backpressure_deferrals_total": 0,
            "persist_drops_total": 2,
            "evictions_total": 0,
            "recent_evictions": [],
        },
        "prf": {"height": 1, "view": 1, "epoch_seed": None},
        "membership": {"height": 1, "view": 1, "epoch": 0, "view_hash": None},
    }

    snapshot = SumeragiStatusSnapshot.from_payload(payload)

    assert snapshot.commit_qc.block_hash == "0xccc"
    assert snapshot.commit_qc.validator_set_hash == "0xddd"
    assert snapshot.commit_qc.signatures_total == 3
    assert snapshot.commit_quorum.signatures_set_b == 2
    assert snapshot.commit_quorum.last_updated_ms == 1700
    assert snapshot.rbc_store.persist_drops_total == 2


def test_sumeragi_commit_qc_record_parses() -> None:
    payload = {
        "subject_block_hash": "aa" * 32,
        "commit_qc": {
            "phase": "Commit",
            "parent_state_root": "bb" * 32,
            "post_state_root": "cc" * 32,
            "height": 12,
            "view": 3,
            "epoch": 4,
            "mode_tag": "iroha2-consensus::permissioned-sumeragi@v1",
            "validator_set_hash": "dd" * 32,
            "validator_set_hash_version": 1,
            "validator_set": ["alice@test", "bob@test"],
            "signers_bitmap": "0a",
            "bls_aggregate_signature": "ff",
        },
    }

    record = SumeragiCommitQcRecord.from_payload(payload)

    assert record.subject_block_hash == "aa" * 32
    assert record.commit_qc is not None
    assert record.commit_qc.parent_state_root == "bb" * 32
    assert record.commit_qc.validator_set_hash == "dd" * 32


def test_get_sumeragi_commit_qc_typed_normalizes_hash() -> None:
    payload = {
        "subject_block_hash": "aa" * 32,
        "commit_qc": None,
    }
    client = ToriiClient("http://127.0.0.1:8080")
    captured = {}

    def fake_request_json(method: str, path: str, expected_status=()) -> object:
        captured["method"] = method
        captured["path"] = path
        return payload

    client.request_json = fake_request_json  # type: ignore[assignment]
    record = client.get_sumeragi_commit_qc_typed("0x" + "aa" * 32)

    assert captured["method"] == "GET"
    assert captured["path"] == "/v1/sumeragi/commit_qc/" + "aa" * 32
    assert record.commit_qc is None
