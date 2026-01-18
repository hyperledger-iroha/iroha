<!-- Hebrew translation of docs/source/sumeragi_evidence_api.md -->

---
lang: he
direction: rtl
source: docs/source/sumeragi_evidence_api.md
status: complete
translator: manual
---

<div dir="rtl">

# ראיות Sumeragi (ממשק ביקורת)

נקודות קצה זמניות לביקורת ראיות ב-Sumeragi.

- GET `/v1/sumeragi/evidence/count`
  - מחזיר את מספר רשומות ה-Evidence הייחודיות שנצפו על-ידי הצומת.
  - תגובה (Norito): `count: u64`.
  - `Accept: application/json` יניב `{ "count": <u64> }`.
  - הערות:
    - נשען על חנות ה-WSV המקומית (`world.consensus_evidence`) ומתPersist באמצעות Norito.
    - שורד אתחולים ומשרת את `/v1/sumeragi/evidence`; כפילויות מוסרות לפי hash.
    - מקומי לכל מאמת (לא משוכפל בקונצנזוס); קליטה דרך ממשל תתווסף בעתיד.

- GET `/v1/sumeragi/evidence`
  - מציג רשומות ראיות אחרונות שנשמרו בסנאפשוט הביקורת של ה-WSV.
  - פרמטרים: `limit` (ברירת מחדל 50, מקסימום 1000), ‏`offset` (ברירת מחדל 0), ‏`kind` (אופציונלי: `DoublePrepare|DoubleCommit|InvalidQc|InvalidProposal`).
  - תגובה (Norito): `(total, Vec<EvidenceRecord>)`.
  - `Accept: application/json` יחזיר `{ "total": <u64>, "items": [ ... ] }`.
  - ראיות עם subject height ישן מ-`sumeragi.npos.reconfig.evidence_horizon_blocks` (ברירת מחדל ‎7200) נזרקות בעת קבלה; השחקן מדווח זאת בלוג לצורך חקירת הגשות ישנות.

- POST `/v1/sumeragi/evidence`
  - מגיש ראיה מקודדת Norito (hex) אל שחקן Sumeragi (`ControlFlow::Evidence`).
  - גוף הבקשה (JSON): `{ "evidence_hex": "<hex string>" }`; מחרוזת ה-hex מייצגת בתים של `ConsensusEvidence` עם framing של Norito ורווחים מותרים.
  - תגובה (JSON): `{ "status": "accepted", "kind": "<variant>" }`.
  - ולידציה בוחנת התאמת חותמים/גובה/תצוגה/אפוק עבור כפלי הצבעה, דורשת payload לא ריק לחתימה בודדת, דורשת קוורום קבלות חתומות (`TransactionSubmissionReceipt`) עבור `Censorship`, ודוחה `InvalidProposal` שאינן מקדמות height או שה-hash של ההורה אינו תואם את ה-commit certificate.
  - כלי CLI: `iroha sumeragi evidence submit --evidence-hex <hex>` או `--evidence-hex-file <path>`.

עוד סטטוס קונצנזוס והוכחות ריצה

Additional consensus status and commit QC proofs

- GET `/v1/sumeragi/status` — returns a Norito-encoded `SumeragiStatusWire` payload by default. Set `Accept: application/json` to receive `{ leader_index, view_change_index, highest_qc{height,view,subject_block_hash}, locked_qc{height,view,subject_block_hash}, commit_qc{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}, commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}, view_change_causes{commit_failure_total,quorum_timeout_total,da_gate_total,censorship_evidence_total,missing_payload_total,missing_qc_total,validation_reject_total,last_cause,last_cause_timestamp_ms}, tx_queue{depth,capacity,saturated}, epoch{length_blocks,commit_deadline_offset,reveal_deadline_offset}, membership{height,view,epoch,view_hash}, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, consensus_message_handling{entries:[{kind,outcome,reason,total}]}, validation_reject_total, validation_reject_reason, block_sync{roster{commit_qc_hint_total,checkpoint_hint_total,commit_qc_history_total,checkpoint_history_total,roster_sidecar_total,commit_roster_journal_total,drop_missing_total,drop_unsolicited_share_blocks_total}}, pacemaker_backpressure_deferrals_total, commit_pipeline_tick_total, da_reschedule_total, rbc_store{sessions,bytes,pressure_level,backpressure_deferrals_total,evictions_total,recent_evictions[...]}, prf{height,view,epoch_seed}, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_current, collectors_targeted_last_per_block, redundant_sends_total, pipeline_conflict_rate_bps, access_set_sources{manifest_hints,entrypoint_hints,prepass_merge,conservative_fallback}, worker_loop{stage,stage_started_ms,last_iteration_ms,queue_depths{vote_rx,block_payload_rx,rbc_chunk_rx,block_rx,consensus_rx,lane_relay_rx,background_rx}} }`. Note: `highest_qc`/`locked_qc` report commit certificate snapshots and `view_change_causes.missing_qc_total` counts missing commit certificates. `view_change_causes.da_gate_total` is reserved for compatibility and should remain zero; DA availability warnings are surfaced via `status.da_gate` and `sumeragi_da_gate_block_total{reason="missing_local_data"}`. `da_reschedule_total` is legacy.
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
