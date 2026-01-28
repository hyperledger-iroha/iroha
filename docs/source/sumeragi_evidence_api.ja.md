<!-- Japanese translation of docs/source/sumeragi_evidence_api.md -->

---
lang: ja
direction: ltr
source: docs/source/sumeragi_evidence_api.md
status: complete
translator: manual
---

# Sumeragi エビデンス（監査 API）

Sumeragi エビデンス用の一時的な監査エンドポイント。

- GET `/v1/sumeragi/evidence/count`
  - ノードが観測したユニークな Evidence エントリ数を返します。
  - レスポンス（Norito ペイロード）: `count: u64`
  - `Accept: application/json` を指定すると `{ "count": <u64> }` を取得。
  - 備考:
    - ノードごとの WSV ストア（`world.consensus_evidence`）に保存され、Norito コーデックで永続化。
    - 再起動後も保持され `/v1/sumeragi/evidence` のデータソースになります。エビデンスハッシュで重複排除。
    - 現時点では各バリデータにローカル（コンセンサス複製なし）。ガバナンス取り込みは今後対応。

- GET `/v1/sumeragi/evidence`
  - WSV の監査スナップショットに保存された最近のエビデンスを一覧表示。
  - クエリパラメータ: `limit`（既定 50、最大 1000）、`offset`（既定 0）、`kind`（任意、`DoublePrepare|DoubleCommit|InvalidQc|InvalidProposal` のいずれか）。
  - レスポンス（Norito ペイロード）: `(total, Vec<EvidenceRecord>)`
  - EvidenceRecord entries include `penalty_applied`, `penalty_cancelled`, `penalty_cancelled_at_height`, and `penalty_applied_at_height` so operators can see delayed slashing status and governance cancellations.
  - `Accept: application/json` で `{ "total": <u64>, "items": [ ... ] }` を取得。
  - `sumeragi.npos.reconfig.evidence_horizon_blocks`（既定 7200）より古い高さのエビデンスは取り込み時に破棄され、オペレーターが古い投稿を調査できるようアクターがログを出力。

- POST `/v1/sumeragi/evidence`
  - 16 進 Norito エンコードのエビデンスを Sumeragi アクター（`ControlFlow::Evidence`）に送信。
  - リクエストボディ（JSON）: `{ "evidence_hex": "<hex string>" }`（hex は Norito-framed `ConsensusEvidence` バイト列、空白は無視）。
  - レスポンス（JSON）: `{ "status": "accepted", "kind": "<variant>" }`
  - バリデーションは二重投票ペイロードの署名者／高さ／ビュー／エポック一致を確認し、単一署名ペイロードは空でないことを要求。`Censorship` は署名済み `TransactionSubmissionReceipt` のレシート・クォラムを要求し、`InvalidProposal` は高さを前進させないものや埋め込み commit certificate と親ハッシュが一致しないものを拒否。
  - CLI ヘルパー: `iroha ops sumeragi evidence submit --evidence-hex <hex>` または `--evidence-hex-file <path>`

追加のコンセンサスステータスと実行証跡

Additional consensus status and commit QC proofs

- GET `/v1/sumeragi/status` — returns a Norito-encoded `SumeragiStatusWire` payload by default. Set `Accept: application/json` to receive `{ leader_index, view_change_index, effective_min_finality_ms, effective_block_time_ms, effective_commit_time_ms, effective_commit_quorum_timeout_ms, effective_availability_timeout_ms, effective_pacemaker_interval_ms, effective_npos_timeouts{propose_ms,prevote_ms,precommit_ms,commit_ms,da_ms,aggregator_ms,exec_ms,witness_ms}, effective_collectors_k, effective_redundant_send_r, highest_qc{height,view,subject_block_hash}, locked_qc{height,view,subject_block_hash}, commit_qc{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}, commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}, view_change_causes{commit_failure_total,quorum_timeout_total,da_gate_total,censorship_evidence_total,missing_payload_total,missing_qc_total,validation_reject_total,last_cause,last_cause_timestamp_ms}, tx_queue{depth,capacity,saturated}, epoch{length_blocks,commit_deadline_offset,reveal_deadline_offset}, membership{height,view,epoch,view_hash}, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, consensus_message_handling{entries:[{kind,outcome,reason,total}]}, validation_reject_total, validation_reject_reason, block_sync{roster{commit_qc_hint_total,checkpoint_hint_total,commit_qc_history_total,checkpoint_history_total,roster_sidecar_total,commit_roster_journal_total,drop_missing_total,drop_unsolicited_share_blocks_total}}, pacemaker_backpressure_deferrals_total, commit_pipeline_tick_total, da_reschedule_total, rbc_store{sessions,bytes,pressure_level,backpressure_deferrals_total,persist_drops_total,evictions_total,recent_evictions[...]}, prf{height,view,epoch_seed}, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_current, collectors_targeted_last_per_block, redundant_sends_total, pipeline_conflict_rate_bps, access_set_sources{manifest_hints,entrypoint_hints,prepass_merge,conservative_fallback}, worker_loop{stage,stage_started_ms,last_iteration_ms,queue_depths{vote_rx,block_payload_rx,rbc_chunk_rx,block_rx,consensus_rx,lane_relay_rx,background_rx}} }`. Note: `highest_qc`/`locked_qc` report highest/locked QC snapshots (highest may be Prepare or Commit) and `view_change_causes.missing_qc_total` counts missing commit certificates. `view_change_causes.da_gate_total` is reserved for compatibility and should remain zero; DA availability warnings are surfaced via `status.da_gate` and `sumeragi_da_gate_block_total{reason="missing_local_data"}`. `da_reschedule_total` is legacy.
- GET `/v1/sumeragi/qc` — returns a Norito-encoded highest/locked QC snapshot (`SumeragiQcSnapshot`) by default. Set `Accept: application/json` to receive `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }`.
- GET `/v1/sumeragi/status/sse` — SSE stream of the same payload (≈1s cadence).
- GET `/v1/sumeragi/commit_qc/:hash` — returns a Norito-encoded `Option<Qc>` for `:hash` (block hash) by default. With `Accept: application/json` the response expands to:
  - If present, `{ subject_block_hash, commit_qc: { phase, parent_state_root, post_state_root, height, view, epoch, mode_tag, validator_set_hash, validator_set_hash_version, validator_set, signers_bitmap, bls_aggregate_signature } }`.
  - If missing, returns `{ subject_block_hash, commit_qc: null }`.

Example (curl)

```bash
# Replace HASH with a real block hash (hex, 32 bytes)
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2

curl -s   http://127.0.0.1:8080/v1/sumeragi/commit_qc/$HASH | jq .

# Example response (when present):
# {
#   "subject_block_hash": "BA6733…F5B2",
#   "commit_qc": {
#     "phase": "Commit",
#     "parent_state_root": "1f9a7d…2c0e",
#     "post_state_root": "9b2f11…a12c",
#     "height": 42,
#     "view": 3,
#     "epoch": 0,
#     "mode_tag": "iroha2-consensus::permissioned-sumeragi@v1",
#     "validator_set_hash": "…",
#     "validator_set_hash_version": 1,
#     "validator_set": ["…"],
#     "signers_bitmap": "0700",
#     "bls_aggregate_signature": ""
#   }
# }
```

Note
- Commit QCs always bind the parent/post state roots; there is no separate execution-root endpoint.
