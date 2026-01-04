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
  - Query params: `limit` (default 50, max 1000), `offset` (default 0), `kind` (optional; one of `DoublePrevote|DoublePrecommit|InvalidQC|InvalidProposal|Censorship`).
  - Response (Norito payload): `(total, Vec<EvidenceRecord>)`.
  - `{ "total": <u64>, "items": [ ... ] }` حاصل کرنے کیلئے `Accept: application/json` سیٹ کریں۔
- جس evidence کا subject height `sumeragi.npos.reconfig.evidence_horizon_blocks`
  (default 7200) سے پرانا ہو، اسے ingress پر drop کیا جاتا ہے؛ actor rejection لاگ کرتا ہے تاکہ operators
  پرانی submissions کی تفتیش کر سکیں۔
- POST `/v1/sumeragi/evidence`
  - Hex-encoded Norito evidence کو Sumeragi actor (`ControlFlow::Evidence`) میں جمع کراتا ہے۔
  - Request body (JSON): `{ "evidence_hex": "<hex string>" }`; hex string Norito-framed `ConsensusEvidence` bytes کی نمائندگی کرتی ہے اور whitespace نظرانداز ہوتا ہے۔
  - Response (JSON): `{ "status": "accepted", "kind": "<variant>" }` کامیابی پر۔
  - Validation signer/height/view/epoch برابری کو double-vote payloads کیلئے چیک کرتا ہے، single-signer payloads کو non-empty ہونے کی شرط دیتا ہے، `Censorship` evidence کیلئے receipt quorums نافذ کرتا ہے (signed `TransactionSubmissionReceipt` payloads)، اور `InvalidProposal` records کو رد کرتا ہے جو height/view کو advance نہیں کرتے یا جن کا parent hash embedded QC سے مختلف ہو۔
  - CLI helper: `iroha sumeragi evidence submit --evidence-hex <hex>` یا `--evidence-hex-file <path>`.

اضافی consensus status اور execution proofs

- GET `/v1/sumeragi/qc` - ڈیفالٹ Norito `SumeragiQcSnapshot` واپس کرتا ہے۔ `Accept: application/json` کے ساتھ `{ highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash } }` ملتا ہے۔
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
