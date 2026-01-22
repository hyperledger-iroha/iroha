---
lang: he
direction: rtl
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-11-25T16:21:41.333147+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/compute_lane.md -->

# נתיב מחשוב (SSC-1)

נתיב המחשוב מקבל קריאות דטרמיניסטיות בסגנון HTTP, ממפה אותן לנקודות כניסה של
Kotodama, ומתעד מדידה/קבלות לצורכי חיוב ובקרת ממשל. RFC זה מקבע את סכמת
המניפסט, מעטפות הקריאה/קבלה, מגבלות ה‑sandbox וברירות המחדל של התצורה עבור
הגרסה הראשונה.

## מניפסט

- סכמה: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` מקובע ל‑`1`; מניפסטים עם גרסה אחרת נדחים בזמן אימות.
- כל נתיב מצהיר על:
  - `id` (`service`, `method`)
  - `entrypoint` (שם נקודת הכניסה של Kotodama)
  - רשימת codec מותרות (`codecs`)
  - מגבלות TTL/gas/בקשה/תגובה (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - דטרמיניזם/מחלקת הרצה (`determinism`, `execution_class`)
  - תיאורי כניסת SoraFS/מודל (`input_limits`, `model` אופציונלי)
  - משפחת תמחור (`price_family`) + פרופיל משאבים (`resource_profile`)
  - מדיניות אימות (`auth`)
- מגבלות ה‑sandbox נמצאות בבלוק `sandbox` של המניפסט ומשותפות לכל הנתיבים
  (מצב/אקראיות/אחסון ודחיית syscalls לא דטרמיניסטיים).

דוגמה: `fixtures/compute/manifest_compute_payments.json`.

## קריאות, בקשות וקבלות

- סכמה: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` בתוך
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` מייצר את ה‑request hash הקנוני (הכותרות נשמרות ב‑`BTreeMap`
  דטרמיניסטי וה‑payload נשמר כ‑`payload_hash`).
- `ComputeCall` מתעד namespace/route, codec, מגבלות TTL/gas/תגובה, פרופיל משאבים
  + משפחת תמחור, אימות (`Public` או `ComputeAuthn` קשור‑UAID), דטרמיניזם
  (`Strict` מול `BestEffort`), רמזי מחלקת הרצה (CPU/GPU/TEE), הצהרות על קלט SoraFS
  בבתים/חתיכות, תקציב מממן אופציונלי, ואת מעטפת הבקשה הקנונית. ה‑request hash
  משמש להגנת replay ולניתוב.
- נתיבים יכולים לשלב הפניות אופציונליות למודל SoraFS ומגבלות קלט (caps inline/chunk);
  כללי ה‑sandbox במניפסט גודרים רמזי GPU/TEE.
- `ComputePriceWeights::charge_units` ממיר נתוני מדידה ליחידות מחשוב מחויבות באמצעות
  חלוקת תקרה במחזורי CPU ובבתים של יציאה.
- `ComputeOutcome` מדווח `Success`, `Timeout`, `OutOfMemory`, `BudgetExhausted`, או
  `InternalError` ויכול לכלול hashes/גדלים/codec של תגובה לצורכי ביקורת.

דוגמאות:
- קריאה: `fixtures/compute/call_compute_payments.json`
- קבלה: `fixtures/compute/receipt_compute_payments.json`

## Sandbox ופרופילי משאבים

- `ComputeSandboxRules` נועל את מצב ההרצה ל‑`IvmOnly` כברירת מחדל, מזריע אקראיות
  דטרמיניסטית מה‑request hash, מאפשר גישת SoraFS לקריאה בלבד, ודוחה syscalls לא
  דטרמיניסטיים. רמזי GPU/TEE מגודרים על ידי `allow_gpu_hints`/`allow_tee_hints`
  כדי לשמור על דטרמיניזם.
- `ComputeResourceBudget` קובע מגבלות לכל פרופיל על cycles, זיכרון לינארי, גודל stack,
  תקציב IO, ו‑egress, לצד מתגים לרמזי GPU ולעזרי WASI-lite.
- ברירות המחדל שולחות שני פרופילים (`cpu-small`, `cpu-balanced`) תחת
  `defaults::compute::resource_profiles` עם נפילות דטרמיניסטיות.

## תמחור ויחידות חיוב

- משפחות מחיר (`ComputePriceWeights`) ממפות cycles ובתי egress ליחידות מחשוב; ברירת
  המחדל מחייבת `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` עם
  `unit_label = "cu"`. המשפחות ממופות לפי `price_family` במניפסטים ונאכפות בזמן
  admission.
- רשומות מדידה נושאות `charged_units` וכן סכומים גולמיים של cycles/ingress/egress/duration
  עבור התאמה. החיובים מוכפלים לפי מכפילי מחלקת הרצה ודטרמיניזם (`ComputePriceAmplifiers`)
  ומוגבלים ע"י `compute.economics.max_cu_per_call`; egress נחתך לפי
  `compute.economics.max_amplification_ratio` כדי להגביל הגברת תגובה.
- תקציבי מממן (`ComputeCall::sponsor_budget_cu`) נאכפים מול תקרות לכל‑קריאה/יומיות;
  יחידות מחויבות לא יכולות לחרוג מתקציב המממן המוצהר.
- עדכוני מחירים של הממשל משתמשים בגבולות מחלקת‑סיכון שב‑`compute.economics.price_bounds`
  ובמשפחות הבסיס שב‑`compute.economics.price_family_baseline`; השתמשו ב‑
  `ComputeEconomics::apply_price_update` כדי לאמת דלתאות לפני עדכון מפת המשפחות הפעילה.
  עדכוני תצורה של Torii משתמשים ב‑`ConfigUpdate::ComputePricing`, ו‑kiso מיישם זאת
  עם אותם גבולות כדי לשמור על דטרמיניזם של עריכות ממשל.

## תצורה

תצורת compute חדשה נמצאת ב‑`crates/iroha_config/src/parameters`:

- תצוגת משתמש: `Compute` (`user.rs`) עם דריסות env:
  - `COMPUTE_ENABLED` (ברירת מחדל `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- תמחור/כלכלה: `compute.economics` מתעד
  `max_cu_per_call`/`max_amplification_ratio`, חלוקת עמלה, תקרות מממן
  (לכל‑קריאה וליומי), בסיסי משפחות מחיר + מחלקות/גבולות סיכון עבור עדכוני ממשל,
  ומכפילי מחלקת הרצה (GPU/TEE/best‑effort).
- actual/defaults: `actual.rs` / `defaults.rs::compute` חושפים הגדרות `Compute`
  מפורשות (namespaces, פרופילים, משפחות מחיר, sandbox).
- תצורות לא תקינות (namespaces ריקים, פרופיל/משפחת ברירת מחדל חסרים, היפוכי גבולות TTL)
  מדווחות כ‑`InvalidComputeConfig` בזמן הפריסה.

## בדיקות וקבצי fixtures

- עזרים דטרמיניסטיים (`request_hash`, תמחור) וסבבי fixtures נמצאים בתוך
  `crates/iroha_data_model/src/compute/mod.rs` (ראו `fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units`).
- קבצי JSON fixtures נמצאים ב‑`fixtures/compute/` ונבדקים ע"י בדיקות data‑model
  לכיסוי רגרסיה.

## Harness של SLO ותקציבים

- תצורת `compute.slo.*` חושפת את כפתורי ה‑SLO של השער (עומק תור in‑flight,
  תקרת RPS, ויעדי latency) בתוך
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. ברירות מחדל:
  32 in‑flight, 512 בתור לכל נתיב, 200 RPS, p50 25 ms, p95 75 ms, p99 120 ms.
- הריצו את harness הבנצ'מרק הקל כדי ללכוד סיכומי SLO ותמונת בקשה/egress:
  `cargo run -p xtask --bin compute_gateway -- bench [manifest_path]
  [iterations] [concurrency] [out_dir]` (ברירות מחדל:
  `fixtures/compute/manifest_compute_payments.json`, 128 איטרציות, concurrency 16,
  פלט תחת `artifacts/compute_gateway/bench_summary.{json,md}`). הבנצ'מרק משתמש
  ב‑payloads דטרמיניסטיים (`fixtures/compute/payload_compute_payments.json`) ובכותרות
  לכל בקשה כדי להימנע מהתנגשויות replay תוך הפעלת נקודות הכניסה
  `echo`/`uppercase`/`sha3`.

## Fixtures לתאימות SDK/CLI

- ה‑fixtures הקנוניים נמצאים תחת `fixtures/compute/`: מניפסט, קריאה, payload, ופריסת
  תגובה/קבלה בסגנון gateway. ה‑payload hashes חייבים להתאים ל‑`request.payload_hash`
  של הקריאה; payload העזר נמצא ב‑`fixtures/compute/payload_compute_payments.json`.
- ה‑CLI מספק `iroha compute simulate` ו‑`iroha compute invoke`:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` נמצאים ב‑
  `javascript/iroha_js/src/compute.js` עם בדיקות רגרסיה תחת
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` טוען את אותם fixtures, מאמת hashes של payload,
  ומדמה את נקודות הכניסה עם בדיקות ב‑
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- עזרי CLI/JS/Swift חולקים את אותם fixtures של Norito כך ש‑SDKs יכולים לאמת בניית
  בקשות וטיפול ב‑hash באופן לא מקוון ללא שער רץ.

</div>
