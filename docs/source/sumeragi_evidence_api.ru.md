---
lang: ru
direction: ltr
source: docs/source/sumeragi_evidence_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 575fdc2adfa8ad461ed44529bc0b129fede932d41c3492373e0135457fb538f4
source_last_modified: "2025-12-22T17:40:03.625971+00:00"
translation_last_reviewed: 2026-01-01
---

# Доказательства Sumeragi (API аудита)

Временные audit endpoints для evidence Sumeragi.

- GET `/v1/sumeragi/evidence/count`
  - Возвращает количество уникальных записей Evidence, наблюденных этим узлом.
  - Ответ (Norito payload): `count: u64`.
  - Установите `Accept: application/json`, чтобы получить `{ "count": <u64> }`.
  - Примечания:
    - Опирается на локальное WSV-хранилище (`world.consensus_evidence`), сохраненное с Norito кодеками.
    - Переживает рестарты и питает `/v1/sumeragi/evidence`; записи дедуплицируются по hash evidence.
    - Все еще локально для каждого валидатора (не реплицируется консенсусом); ingestion через governance будет позже.

- GET `/v1/sumeragi/evidence`
  - Показывает свежие evidence записи, сохраненные в WSV audit snapshot.
  - Query params: `limit` (default 50, max 1000), `offset` (default 0), `kind` (optional; one of `DoublePrepare|DoubleCommit|InvalidQc|InvalidProposal|Censorship`).
  - Ответ (Norito payload): `(total, Vec<EvidenceRecord>)`.
  - Установите `Accept: application/json`, чтобы получить `{ "total": <u64>, "items": [ ... ] }`.
- Evidence с subject height старше `sumeragi.npos.reconfig.evidence_horizon_blocks`
  (default 7200) отбрасывается при ingest; actor логирует отклонение, чтобы операторы могли
  расследовать устаревшие отправки.
- POST `/v1/sumeragi/evidence`
  - Отправляет hex-кодированное Norito evidence в actor Sumeragi (`ControlFlow::Evidence`).
  - Тело запроса (JSON): `{ "evidence_hex": "<hex string>" }`; hex-строка представляет Norito-фреймированные байты `ConsensusEvidence`, пробелы игнорируются.
  - Ответ (JSON): `{ "status": "accepted", "kind": "<variant>" }` при успехе.
  - Валидация проверяет равенство signer/height/view/epoch для double-vote payloads, требует непустые payloads с одним подписантом, применяет receipt кворум для evidence `Censorship` (подписанные payloads `TransactionSubmissionReceipt`), и отклоняет записи `InvalidProposal`, которые не продвигают height или чьи parent hash расходятся с вложенным commit certificate.
  - CLI helper: `iroha sumeragi evidence submit --evidence-hex <hex>` или `--evidence-hex-file <path>`.

Дополнительный статус консенсуса и execution proofs

Additional consensus status and commit QC proofs

- GET `/v1/sumeragi/status` — returns a Norito-encoded `SumeragiStatusWire` payload by default. Set `Accept: application/json` to receive `{ leader_index, view_change_index, highest_qc{height,view,subject_block_hash}, locked_qc{height,view,subject_block_hash}, commit_certificate{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}, commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}, view_change_causes{commit_failure_total,quorum_timeout_total,da_gate_total,censorship_evidence_total,missing_payload_total,missing_qc_total,validation_reject_total,last_cause,last_cause_timestamp_ms}, tx_queue{depth,capacity,saturated}, epoch{length_blocks,commit_deadline_offset,reveal_deadline_offset}, membership{height,view,epoch,view_hash}, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, consensus_message_handling{entries:[{kind,outcome,reason,total}]}, validation_reject_total, validation_reject_reason, block_sync{roster{commit_certificate_hint_total,checkpoint_hint_total,commit_certificate_history_total,checkpoint_history_total,roster_sidecar_total,commit_roster_journal_total,drop_missing_total,drop_unsolicited_share_blocks_total}}, pacemaker_backpressure_deferrals_total, commit_pipeline_tick_total, da_reschedule_total, rbc_store{sessions,bytes,pressure_level,backpressure_deferrals_total,evictions_total,recent_evictions[...]}, prf{height,view,epoch_seed}, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_current, collectors_targeted_last_per_block, redundant_sends_total, pipeline_conflict_rate_bps, access_set_sources{manifest_hints,entrypoint_hints,prepass_merge,conservative_fallback}, worker_loop{stage,stage_started_ms,last_iteration_ms,queue_depths{vote_rx,block_payload_rx,rbc_chunk_rx,block_rx,consensus_rx,lane_relay_rx,background_rx}} }`. Note: `highest_qc`/`locked_qc` report commit certificate snapshots and `view_change_causes.missing_qc_total` counts missing commit certificates. `view_change_causes.da_gate_total` is reserved for compatibility and should remain zero; DA availability warnings are surfaced via `status.da_gate` and `sumeragi_da_gate_block_total{reason="missing_local_data"}`. `da_reschedule_total` is legacy.
- GET `/v1/sumeragi/qc` — returns a Norito-encoded commit certificate snapshot (`SumeragiQcSnapshot`) by default. Set `Accept: application/json` to receive `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }`.
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
