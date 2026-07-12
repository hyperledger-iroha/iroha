---
lang: ur
direction: rtl
source: docs/source/references/operator_aids.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf412f2645cea9d5468f4541ff48d4ace67bc6f3d60a97e68561dde4949ff9be
source_last_modified: "2025-12-13T10:25:50.323533+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

# Torii Endpoints — آپریٹر معاونت (مختصر حوالہ)

یہ صفحہ غیر‑کنسینسس، آپریٹر‑فیسنگ endpoints کی فہرست دیتا ہے جو مانیٹرنگ اور خرابی دور کرنے میں مدد دیتے ہیں۔ جب تک واضح نہ ہو، جوابات JSON میں ہوتے ہیں۔

کنسینسس (Sumeragi)
- میٹرکس: `sumeragi_new_view_receipts_by_hv{height,view}` گیجز کاؤنٹس کو ظاہر کرتے ہیں۔
- GET `/v1/sumeragi/status`
  - لیڈر انڈیکس، Highest/Locked QCs (`highest_qc`/`locked_qc`, ہائٹس/ویوز/سبجیکٹ ہیشز)، کلیکٹر/VRF کاؤنٹرز، pacemaker ڈفرلز، ٹرانزیکشن کیو ڈیپتھ، اور RBC اسٹور ہیلتھ (`rbc_store.{sessions,bytes,pressure_level,persist_drops_total,evictions_total,recent_evictions[...]}`) کا سنیپ شاٹ۔
- GET `/v1/sumeragi/status/sse`
  - `/v1/sumeragi/status` والے payload کا SSE اسٹریم (≈1 سیکنڈ) لائیو ڈیش بورڈز کے لیے۔
- GET `/v1/sumeragi/qc`
  - highest/locked QCs کا سنیپ شاٹ؛ معلوم ہونے پر highest QC کے لیے `subject_block_hash` شامل ہوتا ہے۔
- GET `/v1/sumeragi/pacemaker`
  - pacemaker ٹائمر/کنفیگ: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`۔
- GET `/v1/sumeragi/leader`
  - لیڈر انڈیکس کا سنیپ شاٹ۔ NPoS موڈ میں PRF کانٹیکسٹ شامل ہے: `{ height, view, epoch_seed }`۔
- GET `/v1/sumeragi/telemetry`
  - Aggregated consensus telemetry: `availability.collectors` contains observed collector indices, peer IDs, and ingested-vote counts; `rbc_backlog` contains missing-chunk totals; `rbc_pending` contains bounded pre-session queue totals, drops, and limits. This is not a deterministic collector plan or a per-session RBC contract.
- GET `/v1/sumeragi/params`
  - آن‑چین Sumeragi پیرامیٹرز کا سنیپ شاٹ `{ block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`۔
  - When `da_enabled` is true, availability evidence is tracked but does not gate commit; local payload is required and can be satisfied via RBC `DELIVER` or block sync. Use the aggregated telemetry endpoint, Prometheus counters, status snapshots, and logs to diagnose payload transport.

ثبوت (آڈٹ؛ غیر‑کنسینسس)
- GET `/v1/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v1/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - بنیادی فیلڈز (مثلاً DoublePrepare/DoubleCommit، InvalidQc، InvalidProposal) معائنہ کے لیے شامل ہیں۔
  - مثالیں:
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence | jq .`
- POST `/v1/sumeragi/evidence` → `{ "status": "accepted", "kind": "<variant>" }`
  - CLI مددگار:
    - `iroha --output-format text ops sumeragi evidence list`
    - `iroha --output-format text ops sumeragi evidence count`
    - `iroha ops sumeragi evidence submit --evidence-hex <hex>` (یا `--evidence-hex-file <path>`)

آپریٹر توثیق (WebAuthn/mTLS)
- POST `/v1/operator/auth/registration/options`
  - ابتدائی کریڈینشل اندراج کے لیے WebAuthn رجسٹریشن آپشنز (`publicKey`) واپس کرتا ہے۔
- POST `/v1/operator/auth/registration/verify`
  - WebAuthn attestation payload کی توثیق کرتا ہے اور آپریٹر کریڈینشل محفوظ کرتا ہے۔
- POST `/v1/operator/auth/login/options`
  - آپریٹر لاگ اِن کے لیے WebAuthn توثیقی آپشنز (`publicKey`) واپس کرتا ہے۔
- POST `/v1/operator/auth/login/verify`
  - WebAuthn assertion payload کی توثیق کرتا ہے اور آپریٹر سیشن ٹوکن واپس کرتا ہے۔
- ہیڈرز:
  - `x-iroha-operator-session`: آپریٹر endpoints کے لیے سیشن ٹوکن (login verify سے جاری ہوتا ہے)۔
  - `x-iroha-operator-token`: bootstrap ٹوکن (جب `torii.operator_auth.token_fallback` اجازت دے)۔
  - `x-api-token`: جب `torii.require_api_token = true` یا `torii.operator_auth.token_source = "api"` ہو تو لازمی ہے۔
  - `x-forwarded-client-cert`: جب `torii.operator_auth.require_mtls = true` ہو تو لازمی ہے (ingress proxy سیٹ کرتا ہے)۔
- اندراج کا عمل:
  1. bootstrap ٹوکن کے ساتھ registration options کال کریں (صرف پہلے کریڈینشل اندراج سے پہلے جب `token_fallback = "bootstrap"` ہو)۔
  2. آپریٹر UI میں `navigator.credentials.create` چلائیں اور attestation کو registration verify پر بھیجیں۔
  3. login options اور login verify کال کریں تاکہ `x-iroha-operator-session` ملے۔
  4. آپریٹر endpoints پر `x-iroha-operator-session` بھیجیں۔

نوٹس
- یہ endpoints نوڈ‑لوکل ویوز ہیں (جہاں ذکر ہو وہاں میموری میں) اور کنسینسس یا پرسِسٹنس کو متاثر نہیں کرتے۔
- رسائی Torii کنفیگ کے مطابق API ٹوکنز، آپریٹر توثیق (WebAuthn/mTLS) اور ریٹ لِمٹس سے محفوظ ہو سکتی ہے۔

</div>
