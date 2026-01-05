---
lang: ur
direction: rtl
source: docs/source/sumeragi_evidence_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 575fdc2adfa8ad461ed44529bc0b129fede932d41c3492373e0135457fb538f4
source_last_modified: "2025-12-22T17:40:03.625971+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/sumeragi_evidence_api.md -->

# Sumeragi Evidence (Audit API)

Sumeragi evidence کیلئے عارضی audit endpoints۔

- GET `/v1/sumeragi/evidence/count`
  - اس نوڈ کے ذریعے دیکھی گئی منفرد Evidence entries کی تعداد واپس کرتا ہے۔
  - Response (Norito payload): `count: u64`.
  - `{ "count": <u64> }` حاصل کرنے کیلئے `Accept: application/json` سیٹ کریں۔
  - نوٹس:
    - فی نوڈ WSV store (`world.consensus_evidence`) پر مبنی ہے جو Norito codecs کے ذریعے persisted ہے۔
    - ری اسٹارٹس کے بعد بھی برقرار رہتا ہے اور `/v1/sumeragi/evidence` کو feed کرتا ہے؛ entries evidence hash سے deduplicate ہوتی ہیں۔
    - ابھی بھی ہر validator کیلئے لوکل ہے (consensus-replicated نہیں)؛ governance ingestion بعد میں آئے گا۔

- GET `/v1/sumeragi/evidence`
  - WSV audit snapshot میں محفوظ حالیہ evidence entries کی فہرست دیتا ہے۔
  - Query params: `limit` (default 50, max 1000), `offset` (default 0), `kind` (optional; one of `DoublePrepare|DoubleCommit|InvalidCommitCertificate|InvalidProposal|Censorship`).
  - Response (Norito payload): `(total, Vec<EvidenceRecord>)`.
  - `{ "total": <u64>, "items": [ ... ] }` حاصل کرنے کیلئے `Accept: application/json` سیٹ کریں۔
- جس evidence کا subject height `sumeragi.npos.reconfig.evidence_horizon_blocks`
  (default 7200) سے پرانا ہو، اسے ingress پر drop کیا جاتا ہے؛ actor rejection لاگ کرتا ہے تاکہ operators
  پرانی submissions کی تفتیش کر سکیں۔
- POST `/v1/sumeragi/evidence`
  - Hex-encoded Norito evidence کو Sumeragi actor (`ControlFlow::Evidence`) میں جمع کراتا ہے۔
  - Request body (JSON): `{ "evidence_hex": "<hex string>" }`; hex string میں whitespace نظرانداز ہوتا ہے۔
  - Response (JSON): `{ "status": "accepted", "kind": "<variant>" }` کامیابی پر۔
  - Validation signer/height/view/epoch برابری کو double-vote payloads کیلئے چیک کرتا ہے، single-signer payloads کو non-empty ہونے کی شرط دیتا ہے، `Censorship` evidence کیلئے receipt quorums نافذ کرتا ہے (signed `TransactionSubmissionReceipt` payloads)، اور `InvalidProposal` records کو رد کرتا ہے جو height/view کو advance نہیں کرتے یا جن کا parent hash embedded commit certificate سے مختلف ہو۔
  - CLI helper: `iroha sumeragi evidence submit --evidence-hex <hex>` یا `--evidence-hex-file <path>`.

اضافی consensus status اور execution proofs

- GET `/v1/sumeragi/status` - ڈیفالٹ طور پر Norito-encoded `SumeragiStatusWire` payload واپس کرتا ہے۔ `{ leader_index, view_change_index, highest_qc{height,view,subject_block_hash}, locked_qc{height,view,subject_block_hash}, commit_certificate{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}, commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}, view_change_causes{commit_failure_total,quorum_timeout_total,da_gate_total,censorship_evidence_total,missing_payload_total,missing_qc_total,validation_reject_total,last_cause,last_cause_timestamp_ms}, tx_queue{depth,capacity,saturated}, epoch{length_blocks,commit_deadline_offset,reveal_deadline_offset}, membership{height,view,epoch,view_hash}, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, validation_reject_total, validation_reject_reason, block_sync_roster{commit_certificate_hint_total,checkpoint_hint_total,commit_certificate_history_total,checkpoint_history_total,roster_sidecar_total,commit_roster_journal_total,drop_missing_total}, pacemaker_backpressure_deferrals_total, commit_pipeline_tick_total, da_reschedule_total, rbc_store{sessions,bytes,pressure_level,backpressure_deferrals_total,evictions_total,recent_evictions[...]}, prf{height,view,epoch_seed}, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_current, collectors_targeted_last_per_block, redundant_sends_total, pipeline_conflict_rate_bps, access_set_sources{manifest_hints,entrypoint_hints,prepass_merge,conservative_fallback}, worker_loop{stage,stage_started_ms,last_iteration_ms,queue_depths{vote_rx,block_payload_rx,rbc_chunk_rx,block_rx,consensus_rx,lane_relay_rx,background_rx}} }` حاصل کرنے کیلئے `Accept: application/json` سیٹ کریں۔ نوٹ: `view_change_causes.da_gate_total` compatibility کیلئے reserved ہے اور zero رہنا چاہیے؛ DA availability warnings `status.da_gate` اور `sumeragi_da_gate_block_total{reason="missing_local_data"}` کے ذریعے ظاہر ہوتی ہیں۔ `da_reschedule_total` legacy ہے۔
- GET `/v1/sumeragi/qc` - ڈیفالٹ Norito commit certificate snapshot (`SumeragiQcSnapshot`) واپس کرتا ہے۔ `Accept: application/json` کے ساتھ `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }` ملتا ہے۔
- GET `/v1/sumeragi/status/sse` - اسی payload کا SSE stream (cadence ~1s).
- GET `/v1/sumeragi/exec_root/:hash` - ڈیفالٹ Norito payload `(block_hash, Option<ExecRoot>)` واپس کرتا ہے۔ `Accept: application/json` کے ساتھ `{ block_hash, exec_root }` (hex) یا `exec_root = null` ملتا ہے اگر غائب ہو۔
- GET `/v1/sumeragi/exec_qc/:hash` - ڈیفالٹ طور پر `:hash` (parent block hash) کیلئے Norito-encoded `ExecutionQcRecord` واپس کرتا ہے۔ `Accept: application/json` کے ساتھ جواب میں:
  - `post_state_root` (hex), `height`, `view`, `epoch`, `signers_bitmap` (hex), `bls_aggregate_signature` (hex).
  - اگر غائب ہو تو `{ subject_block_hash, exec_qc: null }` واپس کرتا ہے۔

مثال (curl)

```bash
# Replace HASH with a real parent block hash (hex, 32 bytes)
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2

curl -s \
  http://127.0.0.1:8080/v1/sumeragi/exec_qc/$HASH | jq .

# Example response (when present):
# {
#   "subject_block_hash": "BA6733...F5B2",
#   "post_state_root": "1f9a7d...2c0e",
#   "height": 42,
#   "view": 3,
#   "epoch": 0,
#   "signers_bitmap": "0700",
#   "bls_aggregate_signature": ""
# }
```

Exec root مثال (curl)

```bash
# Replace HASH with a real parent block hash (hex, 32 bytes)
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2

curl -s \
  http://127.0.0.1:8080/v1/sumeragi/exec_root/$HASH | jq .

# Example response (when present):
# {
#   "block_hash": "BA6733...F5B2",
#   "exec_root": "1f9a7d...2c0e"
# }

# When missing:
# {
#   "block_hash": "BA6733...F5B2",
#   "exec_root": null
# }
```

Quick jq one-liners (consistency + wrappers)

- Parent consistency check (ExecQC vs exec_root):

```bash
P=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2
QC=$(curl -s http://127.0.0.1:8080/v1/sumeragi/exec_qc/$P | jq -r '.post_state_root // empty')
RT=$(curl -s http://127.0.0.1:8080/v1/sumeragi/exec_root/$P | jq -r '.exec_root // empty')
if [ -z "$QC" ]; then echo "no ExecQC record in WSV for $P"; exit 1; fi
if [ -z "$RT" ]; then echo "no exec_root in WSV for $P"; exit 1; fi
if [ "$QC" = "$RT" ]; then echo "OK: parent ExecQC equals exec_root ($QC)"; else echo "MISMATCH: QC=$QC root=$RT"; fi
```

- Shell helpers you can paste into your terminal:

```bash
get_exec_qc_root() { curl -s "http://127.0.0.1:8080/v1/sumeragi/exec_qc/$1" | jq -r '.post_state_root // empty'; }
get_exec_root()    { curl -s "http://127.0.0.1:8080/v1/sumeragi/exec_root/$1" | jq -r '.exec_root // empty'; }
check_parent_consistency() {
  local h="$1"; local qc=$(get_exec_qc_root "$h"); local rt=$(get_exec_root "$h");
  if [ -z "$qc" ]; then echo "no ExecQC record for $h"; return 2; fi
  if [ -z "$rt" ]; then echo "no exec_root for $h"; return 3; fi
  if [ "$qc" = "$rt" ]; then echo "OK: $h QC==root ($qc)"; else echo "MISMATCH: QC=$qc root=$rt"; return 1; fi
}

# usage:
#   H=...
#   check_parent_consistency "$H"
```

Note
- Cross-height equality (child vs parent) Sumeragi میں strict mode میں اندرونی طور پر نافذ ہے
  (`require_wsv_exec_qc=true`) اور ابھی query کے ذریعے براہ راست دستیاب نہیں۔ اوپر دی گئی
  operator-visible consistency checks یہ تصدیق کرتی ہیں کہ parent کا ExecutionQC record
  اس کے persisted exec_root سے match کرتا ہے۔ مستقبل کا endpoint proposal/header roots
  ظاہر کرے گا تاکہ external parent-vs-child comparison ممکن ہو۔

</div>
