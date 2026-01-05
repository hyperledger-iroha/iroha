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
  - פרמטרים: `limit` (ברירת מחדל 50, מקסימום 1000), ‏`offset` (ברירת מחדל 0), ‏`kind` (אופציונלי: `DoublePrepare|DoubleCommit|InvalidCommitCertificate|InvalidProposal`).
  - תגובה (Norito): `(total, Vec<EvidenceRecord>)`.
  - `Accept: application/json` יחזיר `{ "total": <u64>, "items": [ ... ] }`.
  - ראיות עם subject height ישן מ-`sumeragi.npos.reconfig.evidence_horizon_blocks` (ברירת מחדל ‎7200) נזרקות בעת קבלה; השחקן מדווח זאת בלוג לצורך חקירת הגשות ישנות.

- POST `/v1/sumeragi/evidence`
  - מגיש ראיה מקודדת Norito (hex) אל שחקן Sumeragi (`ControlFlow::Evidence`).
  - גוף הבקשה (JSON): `{ "evidence_hex": "<hex string>" }`; רווחים מותרים.
  - תגובה (JSON): `{ "status": "accepted", "kind": "<variant>" }`.
  - ולידציה בוחנת התאמת חותמים/גובה/תצוגה/אפוק עבור כפלי הצבעה, דורשת payload לא ריק לחתימה בודדת, דורשת קוורום קבלות חתומות (`TransactionSubmissionReceipt`) עבור `Censorship`, ודוחה `InvalidProposal` שאינן מקדמות height/view או שה-hash של ההורה אינו תואם את ה-commit certificate.
  - כלי CLI: `iroha sumeragi evidence submit --evidence-hex <hex>` או `--evidence-hex-file <path>`.

עוד סטטוס קונצנזוס והוכחות ריצה

- GET `/v1/sumeragi/status` — כברירת מחדל מחזיר `SumeragiStatusWire` בקידוד Norito. ‏`Accept: application/json` מספק אובייקט עם נתוני מנהיג, commit certificate, `commit_certificate{height,view,epoch,block_hash,validator_set_hash,validator_set_len,signatures_total}` ו-`commit_quorum{height,view,block_hash,signatures_present,signatures_counted,signatures_set_b,signatures_required,last_updated_ms}`, תורי טרנזקציות, מקטע `view_change_causes { commit_failure_total, quorum_timeout_total, da_gate_total, censorship_evidence_total, missing_payload_total, missing_qc_total, validation_reject_total, last_cause, last_cause_timestamp_ms }`, מדדי gossip fallback, חתכי epoch (`epoch { length_blocks, commit_deadline_offset, reveal_deadline_offset }`), אובייקט `membership { height, view, epoch, view_hash }`, מדדי VRF/PRF, מוני validation_reject (total/reason) והמידע של block_sync_roster (commit/ checkpoint hints, history, sidecar, journal, topology, trusted, drops), בנוסף למונים של pacemaker_backpressure_deferrals_total ו-commit_pipeline_tick_total יחד עם pipeline_conflict_rate_bps ו-`access_set_sources { manifest_hints, entrypoint_hints, prepass_merge, conservative_fallback }`, ונתוני RBC/DA המשויכים, worker_loop { stage, stage_started_ms, last_iteration_ms, queue_depths { vote_rx, block_payload_rx, rbc_chunk_rx, block_rx, consensus_rx, lane_relay_rx, background_rx } }.
- GET `/v1/sumeragi/qc` — ברירת מחדל Norito `SumeragiQcSnapshot` עבור commit certificate; JSON מספק `highest_qc` ו-`locked_qc`.
- GET `/v1/sumeragi/status/sse` — זרם SSE של אותו מטען (≈שנייה).
- GET `/v1/sumeragi/exec_root/:hash` — Norito `(block_hash, Option<ExecRoot>)`; JSON מחזיר `{ block_hash, exec_root }` (hex) או `null`.
- GET `/v1/sumeragi/exec_qc/:hash` — Norito `ExecutionQcRecord`; JSON כולל `post_state_root`, ‏`height`, ‏`view`, ‏`epoch`, ‏`signers_bitmap`, ‏`bls_aggregate_signature` (hex). אם אין נתון, מוחזר `{ subject_block_hash, exec_qc: null }`.

דוגמאות curl

```bash
HASH=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2
curl -s http://127.0.0.1:8080/v1/sumeragi/exec_qc/$HASH | jq .
curl -s http://127.0.0.1:8080/v1/sumeragi/exec_root/$HASH | jq .
```

## jq מהירים (בדיקות עקביות)

- התאמת ExecQC להוראת exec_root:

```bash
P=BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2
QC=$(curl -s http://127.0.0.1:8080/v1/sumeragi/exec_qc/$P | jq -r '.post_state_root // empty')
RT=$(curl -s http://127.0.0.1:8080/v1/sumeragi/exec_root/$P | jq -r '.exec_root // empty')
if [ -z "$QC" ]; then echo "no ExecQC record in WSV for $P"; exit 1; fi
if [ -z "$RT" ]; then echo "no exec_root in WSV for $P"; exit 1; fi
if [ "$QC" = "$RT" ]; then echo "OK: parent ExecQC equals exec_root ($QC)"; else echo "MISMATCH: QC=$QC root=$RT"; fi
```

- פונקציות shell:

```bash
get_exec_qc_root() { curl -s "http://127.0.0.1:8080/v1/sumeragi/exec_qc/$1" | jq -r '.post_state_root // empty'; }
get_exec_root()    { curl -s "http://127.0.0.1:8080/v1/sumeragi/exec_root/$1" | jq -r '.exec_root // empty'; }
check_parent_consistency() {
  local h="$1"; local qc=$(get_exec_qc_root "$h"); local rt=$(get_exec_root "$h");
  if [ -z "$qc" ]; then echo "no ExecQC record for $h"; return 2; fi
  if [ -z "$rt" ]; then echo "no exec_root for $h"; return 3; fi
  if [ "$qc" = "$rt" ]; then echo "OK: $h QC==root ($qc)"; else echo "MISMATCH: QC=$qc root=$rt"; return 1; fi
}
```

הערה
- השוואת ילד-הורה enforced בתוך Sumeragi במצב קפדני (`require_wsv_exec_qc=true`) ועוד לא זמינה בקשה חיצונית. הבדיקות לעיל מאמתות כי ExecQC של ההורה תואם את exec_root המתPersist. בעתיד יתווסף קצה שמציג שורשי הצעה/כותרת להשוואה מלאה.

</div>
