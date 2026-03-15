---
lang: ar
direction: rtl
source: docs/source/sumeragi_evidence_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 575fdc2adfa8ad461ed44529bc0b129fede932d41c3492373e0135457fb538f4
source_last_modified: "2025-12-22T17:40:03.625971+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/sumeragi_evidence_api.md -->

# ادلة Sumeragi (واجهة تدقيق)

نقاط نهاية تدقيق مؤقتة لادلة Sumeragi.

- GET `/v1/sumeragi/evidence/count`
  - يعيد عدد سجلات Evidence الفريدة التي رصدتها هذه العقدة.
  - الاستجابة (Norito payload): `count: u64`.
  - اضبط `Accept: application/json` لاستلام `{ "count": <u64> }`.
  - ملاحظات:
    - مدعومة بمخزن WSV لكل عقدة (`world.consensus_evidence`) والمحفوظ باستخدام Norito codecs.
    - تستمر بعد اعادة التشغيل وتغذي `/v1/sumeragi/evidence`; يتم ازالة التكرار حسب hash الدليل.
    - ما زالت محلية لكل مدقق (غير مكررة بالاجماع)؛ ستاتي لاحقا عملية governance ingestion.

- GET `/v1/sumeragi/evidence`
  - تسرد سجلات الادلة الحديثة المحفوظة في لقطة تدقيق WSV.
  - معاملات الاستعلام: `limit` (default 50, max 1000)، `offset` (default 0)، `kind` (اختياري؛ احد `DoublePrepare|DoubleCommit|InvalidQc|InvalidProposal|Censorship`).
  - الاستجابة (Norito payload): `(total, Vec<EvidenceRecord>)`.
  - EvidenceRecord entries include `penalty_applied`, `penalty_cancelled`, `penalty_cancelled_at_height`, and `penalty_applied_at_height` so operators can see delayed slashing status and governance cancellations.
  - اضبط `Accept: application/json` لاستلام `{ "total": <u64>, "items": [ ... ] }`.
- الدليل الذي يكون subject height فيه اقدم من `sumeragi.npos.reconfig.evidence_horizon_blocks`
  (default 7200) يتم اسقاطه عند الادخال؛ يسجل الممثل الرفض لمساعدة المشغلين على
  التحقيق في الارسالات القديمة.
- POST `/v1/sumeragi/evidence`
  - يرسل evidence من Norito مشفرة hex الى ممثل Sumeragi (`ControlFlow::Evidence`).
  - جسم الطلب (JSON): `{ "evidence_hex": "<hex string>" }`؛ سلسلة hex تمثل بايتات `ConsensusEvidence` المؤطرة بنوريتو ويتم تجاهل الفراغات.
  - الاستجابة (JSON): `{ "status": "accepted", "kind": "<variant>" }` عند النجاح.
  - التحقق يغطي تطابق signer/height/view/epoch لحمولات double-vote، ويتطلب حمولة غير فارغة لموقع واحد، ويفرض quorums لادلة `Censorship` (حمولات موقعة `TransactionSubmissionReceipt`)، ويرفض سجلات `InvalidProposal` التي لا تتقدم في height او التي يختلف hash الاب فيها عن commit certificate المضمن.
  - مساعد CLI: `iroha ops sumeragi evidence submit --evidence-hex <hex>` او `--evidence-hex-file <path>`.

حالة اجماع اضافية وادلة تنفيذ

Additional consensus status and commit QC proofs

- GET `/v1/sumeragi/status` — returns a Norito-encoded `SumeragiStatusWire` payload by default. Set `Accept: application/json` to receive `{ leader_index, view_change_index, effective_min_finality_ms, effective_block_time_ms, effective_commit_time_ms, effective_pacing_factor_bps, effective_commit_quorum_timeout_ms, effective_availability_timeout_ms, effective_pacemaker_interval_ms, effective_npos_timeouts{propose_ms,prevote_ms,precommit_ms,commit_ms,da_ms,aggregator_ms,exec_ms,witness_ms}, effective_collectors_k, effective_redundant_send_r, highest_qc{height,view,subject_block_hash}, locked_qc{height,view,subject_block_hash}, commit_qc{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}, commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}, view_change_causes{commit_failure_total,quorum_timeout_total,da_gate_total,censorship_evidence_total,missing_payload_total,missing_qc_total,validation_reject_total,last_cause,last_cause_timestamp_ms}, tx_queue{depth,capacity,saturated}, epoch{length_blocks,commit_deadline_offset,reveal_deadline_offset}, membership{height,view,epoch,view_hash}, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, consensus_message_handling{entries:[{kind,outcome,reason,total}]}, validation_reject_total, validation_reject_reason, block_sync{roster{commit_qc_hint_total,checkpoint_hint_total,commit_qc_history_total,checkpoint_history_total,roster_sidecar_total,commit_roster_journal_total,drop_missing_total,drop_unsolicited_share_blocks_total}}, pacemaker_backpressure_deferrals_total, commit_pipeline_tick_total, da_reschedule_total, rbc_store{sessions,bytes,pressure_level,backpressure_deferrals_total,persist_drops_total,evictions_total,recent_evictions[...]}, prf{height,view,epoch_seed}, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_current, collectors_targeted_last_per_block, redundant_sends_total, pipeline_conflict_rate_bps, access_set_sources{manifest_hints,entrypoint_hints,prepass_merge,conservative_fallback}, worker_loop{stage,stage_started_ms,last_iteration_ms,queue_depths{vote_rx,block_payload_rx,rbc_chunk_rx,block_rx,consensus_rx,lane_relay_rx,background_rx}} }`. Note: `highest_qc`/`locked_qc` report highest/locked QC snapshots (highest may be Prepare or Commit) and `view_change_causes.missing_qc_total` counts missing commit certificates. `view_change_causes.da_gate_total` is reserved for compatibility and should remain zero; DA availability warnings are surfaced via `status.da_gate` and `sumeragi_da_gate_block_total{reason="missing_local_data"}`. `da_reschedule_total` is legacy.
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
