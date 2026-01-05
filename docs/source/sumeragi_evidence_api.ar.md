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
  - معاملات الاستعلام: `limit` (default 50, max 1000)، `offset` (default 0)، `kind` (اختياري؛ احد `DoublePrepare|DoubleCommit|InvalidCommitCertificate|InvalidProposal|Censorship`).
  - الاستجابة (Norito payload): `(total, Vec<EvidenceRecord>)`.
  - اضبط `Accept: application/json` لاستلام `{ "total": <u64>, "items": [ ... ] }`.
- الدليل الذي يكون subject height فيه اقدم من `sumeragi.npos.reconfig.evidence_horizon_blocks`
  (default 7200) يتم اسقاطه عند الادخال؛ يسجل الممثل الرفض لمساعدة المشغلين على
  التحقيق في الارسالات القديمة.
- POST `/v1/sumeragi/evidence`
  - يرسل evidence من Norito مشفرة hex الى ممثل Sumeragi (`ControlFlow::Evidence`).
  - جسم الطلب (JSON): `{ "evidence_hex": "<hex string>" }`؛ يتم تجاهل الفراغات في السلسلة hex.
  - الاستجابة (JSON): `{ "status": "accepted", "kind": "<variant>" }` عند النجاح.
  - التحقق يغطي تطابق signer/height/view/epoch لحمولات double-vote، ويتطلب حمولة غير فارغة لموقع واحد، ويفرض quorums لادلة `Censorship` (حمولات موقعة `TransactionSubmissionReceipt`)، ويرفض سجلات `InvalidProposal` التي لا تتقدم في height/view او التي يختلف hash الاب فيها عن commit certificate المضمن.
  - مساعد CLI: `iroha sumeragi evidence submit --evidence-hex <hex>` او `--evidence-hex-file <path>`.

حالة اجماع اضافية وادلة تنفيذ

- GET `/v1/sumeragi/status` - يعيد `SumeragiStatusWire` مشفر Norito افتراضيا. اضبط `Accept: application/json` لاستلام `{ leader_index, view_change_index, highest_qc{height,view,subject_block_hash}, locked_qc{height,view,subject_block_hash}, commit_certificate{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}, commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}, view_change_causes{commit_failure_total,quorum_timeout_total,da_gate_total,censorship_evidence_total,missing_payload_total,missing_qc_total,validation_reject_total,last_cause,last_cause_timestamp_ms}, tx_queue{depth,capacity,saturated}, epoch{length_blocks,commit_deadline_offset,reveal_deadline_offset}, membership{height,view,epoch,view_hash}, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, validation_reject_total, validation_reject_reason, block_sync_roster{commit_certificate_hint_total,checkpoint_hint_total,commit_certificate_history_total,checkpoint_history_total,roster_sidecar_total,commit_roster_journal_total,drop_missing_total}, pacemaker_backpressure_deferrals_total, commit_pipeline_tick_total, da_reschedule_total, rbc_store{sessions,bytes,pressure_level,backpressure_deferrals_total,evictions_total,recent_evictions[...]}, prf{height,view,epoch_seed}, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_current, collectors_targeted_last_per_block, redundant_sends_total, pipeline_conflict_rate_bps, access_set_sources{manifest_hints,entrypoint_hints,prepass_merge,conservative_fallback}, worker_loop{stage,stage_started_ms,last_iteration_ms,queue_depths{vote_rx,block_payload_rx,rbc_chunk_rx,block_rx,consensus_rx,lane_relay_rx,background_rx}} }`. ملاحظة: `view_change_causes.da_gate_total` محجوز للتوافق ويجب ان يبقى صفرا؛ تحذيرات توفر DA تظهر عبر `status.da_gate` و `sumeragi_da_gate_block_total{reason="missing_local_data"}`. `da_reschedule_total` قديم.
- GET `/v1/sumeragi/qc` - يعيد snapshot commit certificate (`SumeragiQcSnapshot`) مشفر Norito افتراضيا. اضبط `Accept: application/json` لاستلام `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }`.
- GET `/v1/sumeragi/status/sse` - بث SSE لنفس الحمولة (cadence ~1s).
- GET `/v1/sumeragi/exec_root/:hash` - يعيد Norito `(block_hash, Option<ExecRoot>)` افتراضيا. مرر `Accept: application/json` لاستلام `{ block_hash, exec_root }` (hex) او `exec_root = null` عند الغياب.
- GET `/v1/sumeragi/exec_qc/:hash` - يعيد `ExecutionQcRecord` مشفر Norito لـ `:hash` (hash بلوك الاب) افتراضيا. مع `Accept: application/json` تتوسع الاستجابة الى:
  - `post_state_root` (hex), `height`, `view`, `epoch`, `signers_bitmap` (hex), `bls_aggregate_signature` (hex).
  - عند الغياب، يعيد `{ subject_block_hash, exec_qc: null }`.

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

مثال exec root (curl)

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

اوامر jq سريعة (consistency + wrappers)

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

ملاحظة
- يتم فرض تطابق cross-height (child vs parent) داخليا في Sumeragi ضمن الوضع الصارم
  (`require_wsv_exec_qc=true`) ولا يتم كشفه مباشرة عبر query بعد. الفحوصات المرئية
  للمشغلين اعلاه تتحقق من ان سجل ExecutionQC للاب يطابق exec_root المحفوظ. ستوفر
  نقطة نهاية مستقبلية roots الخاصة بالproposal/header للسماح بمقارنة خارجية بين الاب والابن.

</div>
