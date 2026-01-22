---
lang: ar
direction: rtl
source: docs/source/references/operator_aids.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf412f2645cea9d5468f4541ff48d4ace67bc6f3d60a97e68561dde4949ff9be
source_last_modified: "2025-12-13T10:25:50.323533+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

# نقاط نهاية Torii — أدوات مساعدة للمشغّل (مرجع سريع)

تسرد هذه الصفحة نقاط نهاية غير توافقية موجهة للمشغّلين تساعد في الرؤية واستكشاف الأعطال. الاستجابات بصيغة JSON ما لم يُذكر خلاف ذلك.

التوافق (Sumeragi)
- GET `/v1/sumeragi/new_view`
  - لقطة لعدّادات استلام NEW_VIEW لكل `(height, view)`.
  - الشكل: `{ "ts_ms": <u64>, "items": [{ "height": <u64>, "view": <u64>, "count": <u64> }, ...] }`
  - مثال:
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/new_view | jq .`
- GET `/v1/sumeragi/new_view/sse` (SSE)
  - تدفق دوري (≈1 ث) لنفس الحمولة لأجل لوحات المتابعة.
  - مثال:
    - `curl -Ns http://127.0.0.1:8080/v1/sumeragi/new_view/sse`
- المقاييس: عدادات `sumeragi_new_view_receipts_by_hv{height,view}` تعكس الأعداد.
- GET `/v1/sumeragi/status`
  - لقطة لمؤشر القائد، Highest/Locked QCs (`highest_qc`/`locked_qc` مع الارتفاعات والمشاهدات وتجزئات الموضوع)، عدادات المجمّعين/VRF، تأجيلات pacemaker، عمق طابور المعاملات، وصحة مخزن RBC (`rbc_store.{sessions,bytes,pressure_level,persist_drops_total,evictions_total,recent_evictions[...]}`).
- GET `/v1/sumeragi/status/sse`
  - تدفق SSE (≈1 ث) لنفس الحمولة مثل `/v1/sumeragi/status` للمتابعة الحية.
- GET `/v1/sumeragi/qc`
  - لقطة لـ highest/locked QCs؛ تتضمن `subject_block_hash` لـ highest QC عند توفره.
- GET `/v1/sumeragi/pacemaker`
  - مؤقتات/إعدادات pacemaker: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- GET `/v1/sumeragi/leader`
  - لقطة لمؤشر القائد. في وضع NPoS تتضمن سياق PRF: `{ height, view, epoch_seed }`.
- GET `/v1/sumeragi/collectors`
  - خطة مجمّعين حتمية مشتقة من الطوبولوجيا المُلتزم بها والمعلمات على السلسلة: تصدر `mode` وخطة `(height, view)` (مع `height` مساويًا لارتفاع السلسلة الحالي)، و`collectors_k` و`redundant_send_r` و`proxy_tail_index` و`min_votes_for_commit` والقائمة المرتبة للمجمّعين، و`epoch_seed` (hex) عند تفعيل NPoS.
- GET `/v1/sumeragi/params`
  - لقطة لمعلمات Sumeragi على السلسلة `{ block_time_ms, commit_time_ms, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
  - عندما تكون `da_enabled` true، يتم تتبع دليل التوفر (`availability evidence` أو RBC `READY`) لكن الالتزام لا ينتظرها؛ كما أن `DELIVER` المحلي لـ RBC ليس شرطًا. يمكن للمشغّلين تأكيد سلامة نقل الحمولة عبر نقاط نهاية RBC أدناه.
- GET `/v1/sumeragi/rbc`
  - عدادات Reliable Broadcast الإجمالية: `{ sessions_active, sessions_pruned_total, ready_broadcasts_total, ready_rebroadcasts_skipped_total, deliver_broadcasts_total, payload_bytes_delivered_total, payload_rebroadcasts_skipped_total }`.
- GET `/v1/sumeragi/rbc/sessions`
  - لقطة لحالة كل جلسة (تجزئة الكتلة، height/view، عدادات القطع، علامة delivered، مؤشر `invalid`، تجزئة الحمولة، وقيمة recovered) لتشخيص تعطل تسليم RBC وإبراز الجلسات المستعادة بعد إعادة التشغيل.
  - اختصار CLI: `iroha --output-format text ops sumeragi rbc sessions` يطبع `hash` و`height/view` وتقدّم القطع وعدد ready وعلامات invalid/delivered.

الأدلة (تدقيق؛ خارج التوافق)
- GET `/v1/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v1/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - تتضمن حقولًا أساسية (مثل DoublePrepare/DoubleCommit، InvalidQc، InvalidProposal) للفحص.
  - أمثلة:
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence | jq .`
- POST `/v1/sumeragi/evidence` → `{ "status": "accepted", "kind": "<variant>" }`
  - أدوات CLI:
    - `iroha --output-format text ops sumeragi evidence list`
    - `iroha --output-format text ops sumeragi evidence count`
    - `iroha ops sumeragi evidence submit --evidence-hex <hex>` (أو `--evidence-hex-file <path>`)

مصادقة المشغّل (WebAuthn/mTLS)
- POST `/v1/operator/auth/registration/options`
  - تعيد خيارات تسجيل WebAuthn (`publicKey`) لإدراج بيانات الاعتماد الأولى.
- POST `/v1/operator/auth/registration/verify`
  - تتحقق من حمولة attestation الخاصة بـ WebAuthn وتُثبّت بيانات اعتماد المشغّل.
- POST `/v1/operator/auth/login/options`
  - تعيد خيارات مصادقة WebAuthn (`publicKey`) لتسجيل دخول المشغّل.
- POST `/v1/operator/auth/login/verify`
  - تتحقق من حمولة assertion الخاصة بـ WebAuthn وتعيد رمز جلسة للمشغّل.
- الرؤوس:
  - `x-iroha-operator-session`: رمز جلسة لنقاط نهاية المشغّل (يُصدره login verify).
  - `x-iroha-operator-token`: رمز bootstrap (مسموح عندما يسمح `torii.operator_auth.token_fallback`).
  - `x-api-token`: مطلوب عندما `torii.require_api_token = true` أو `torii.operator_auth.token_source = "api"`.
  - `x-forwarded-client-cert`: مطلوب عندما `torii.operator_auth.require_mtls = true` (يُضبط بواسطة وكيل الدخول).
- تدفق التسجيل:
  1. استدعِ registration options مع رمز bootstrap (يسمح به فقط قبل تسجيل أول بيانات اعتماد عندما `token_fallback = "bootstrap"`).
  2. نفّذ `navigator.credentials.create` في واجهة المشغّل وأرسل attestation إلى registration verify.
  3. استدعِ login options وlogin verify للحصول على `x-iroha-operator-session`.
  4. أرسل `x-iroha-operator-session` مع نقاط نهاية المشغّل.

ملاحظات
- هذه النقاط هي مشاهد محلية للعقدة (في الذاكرة حيثما ذُكر) ولا تؤثر على التوافق أو التخزين.
- قد تُحمى الوصولات برموز API ومصادقة المشغّل (WebAuthn/mTLS) وحدود المعدل حسب إعدادات Torii.

مقتطفات CLI للمراقبة (bash)

- استعلام عن لقطة JSON كل 2 ث (يطبع آخر 10 إدخالات):

```bash
#!/usr/bin/env bash
set -euo pipefail
TORII="${TORII:-http://127.0.0.1:8080}"
INTERVAL="${INTERVAL:-2}"
TOKEN="${TOKEN:-}"
HDR=()
if [[ -n "$TOKEN" ]]; then HDR=(-H "x-api-token: $TOKEN"); fi
while true; do
  curl -s "${HDR[@]}" "$TORII/v1/sumeragi/new_view" \
    | jq -c '{ts_ms, items:(.items|sort_by([.height,.view])|reverse|.[:10])}'
  sleep "$INTERVAL"
done
```

- متابعة تدفق SSE مع تنسيق (آخر 10 إدخالات):

```bash
#!/usr/bin/env bash
set -euo pipefail
TORII="${TORII:-http://127.0.0.1:8080}"
TOKEN="${TOKEN:-}"
HDR=()
if [[ -n "$TOKEN" ]]; then HDR=(-H "x-api-token: $TOKEN"); fi
curl -Ns "${HDR[@]}" "$TORII/v1/sumeragi/new_view/sse" \
  | awk '/^data:/{sub(/^data: /,""); print}' \
  | jq -c '{ts_ms, items:(.items|sort_by([.height,.view])|reverse|.[:10])}'
```

</div>
