from __future__ import annotations

from iroha_python import (
    SumeragiCommitQcRecord,
    SumeragiPhasesSnapshot,
    ToriiClient,
)


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
