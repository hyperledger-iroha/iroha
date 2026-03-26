---
lang: he
direction: rtl
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c1214f4d0ad86449c0ef4b8f8cbaa38fe265bab4afcc2930cd30a57c089e6d7
source_last_modified: "2025-11-15T05:05:33.914289+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל- docs/source/account_address_status.md -->

## סטטוס תאימות כתובת חשבון (ADDR-2)

סטטוס: אושר 2026-03-30  
בעלים: צוות מודל הנתונים / גילדת QA  
הפניה לרודמפ: ADDR-2 — Dual-Format Compliance Suite

### 1. סקירה

- Fixture: `fixtures/account/address_vectors.json` (I105 + multisig מקרי חיובי/שלילי).
- היקף: payloads דטרמיניסטיים ב-V1 המכסים implicit-default, Local-12, Global registry, ו-controllers
  של multisig עם מיפוי שגיאות מלא.
- הפצה: משותף בין Rust data-model, Torii, SDKs של JS/TS, Swift ו-Android; CI נכשל אם צרכן כלשהו סוטה.
- מקור אמת: הגנרטור נמצא ב-`crates/iroha_data_model/src/account/address/compliance_vectors.rs` ונחשף
  דרך `cargo xtask address-vectors`.

### 2. יצירה מחדש ואימות

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

דגלים:

- `--out <path>` — עקיפה אופציונלית בעת יצירת bundles אד-הוק (ברירת מחדל:
  `fixtures/account/address_vectors.json`).
- `--stdout` — מפיק JSON ל-stdout במקום לכתוב לדיסק.
- `--verify` — משווה את הקובץ הנוכחי לתוכן שנוצר מחדש (נכשל מהר על סטיה; לא תואם ל-`--stdout`).

### 3. מטריצת ארטיפקטים

| שכבה | אכיפה | הערות |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | מפרש את ה-JSON, משחזר payloads קנוניים, ובודק המרות I105 (מועדף)/i105 (`sora`, אפשרות שנייה)/canonical + שגיאות מובנות. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | מאמת codecs בצד השרת כך ש-Torii דוחה payloads I105 (מועדף)/i105 (`sora`, אפשרות שנייה) פגומים באופן דטרמיניסטי. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | משקף fixtures V1 (I105 מועדף/I105 אפשרות שנייה/fullwidth) ומאמת קודי שגיאה בסגנון Norito לכל מקרה שלילי. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | בודק decoding של I105 (מועדף)/i105 (`sora`, אפשרות שנייה), payloads של multisig והצפת שגיאות בפלטפורמות Apple. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | מבטיח שביינדינגים Kotlin/Java נשארים מיושרים עם ה-fixture הקנוני. |

### 4. ניטור ועבודה פתוחה

- דיווח סטטוס: מסמך זה מקושר מ-`status.md` ומה-roadmap כדי שביקורות שבועיות יוודאו את תקינות
  ה-fixture.
- תקציר פורטל מפתחים: ראו **Reference -> Account address compliance** בדוקס הפורטל
  (`docs/portal/docs/reference/account-address-status.md`) לסיכום חיצוני.
- Prometheus ודשבורדים: בכל אימות של עותק SDK, הריצו את ה-helper עם `--metrics-out` (ואופציונלית
  `--metrics-label`) כדי ש-textfile collector של Prometheus יקלוט
  `account_address_fixture_check_status{target=...}`. דשבורד Grafana **Account Address Fixture Status**
  (`dashboards/grafana/account_address_fixture_status.json`) מציג ספירות pass/fail לכל שכבה ומציג
  את digest ה-SHA-256 הקנוני כראיית audit. התריעו כאשר יעד כלשהו מדווח `0`.
- Torii metrics: `torii_address_domain_total{endpoint,domain_kind}` נמדד כעת עבור כל account literal
  שפורש בהצלחה, ומשקף את `torii_address_invalid_total`/`torii_address_local8_total`. התריעו על כל
  תעבורה עם `domain_kind="local12"` בפרודקשן ושכפלו את המונים לדשבורד SRE `address_ingest` כדי ששער
  ה-retirement של Local-12 יקבל ראיות audit.
- Fixture helper: `scripts/account_fixture_helper.py` מוריד או מאמת את ה-JSON הקנוני כך שאוטומציית
  release של ה-SDK יכולה למשוך/לבדוק את ה-bundle בלי העתקה ידנית, תוך כתיבת metrics של Prometheus
  לפי הצורך. דוגמה:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \
    --target path/to/sdk/address_vectors.json \
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
    --metrics-label android
  ```

  ה-helper כותב `account_address_fixture_check_status{target="android"} 1` כאשר היעד תואם, ועוד
  gauges של `account_address_fixture_remote_info` / `account_address_fixture_local_info` שמציגים
  digests SHA-256. קבצים חסרים מדווחים כ-`account_address_fixture_local_missing`.
  Automation wrapper: קראו ל-`ci/account_fixture_metrics.sh` מ-cron/CI כדי להפיק textfile מרוכז
  (ברירת מחדל `artifacts/account_fixture/address_fixture.prom`). העבירו entries חוזרים של
  `--target label=path` (אפשר לצרף `::https://mirror/...` לכל target כדי להחליף מקור) כדי ש-Prometheus
  תגרד קובץ אחד שמכסה כל SDK/CLI copy. GitHub workflow `address-vectors-verify.yml` כבר מריץ את
  ה-helper נגד ה-fixture הקנוני ומעלה את ה-artifact `account-address-fixture-metrics` ל-SRE ingestion.

</div>
