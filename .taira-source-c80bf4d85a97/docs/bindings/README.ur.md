---
lang: ur
direction: rtl
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb91ce03aee552c65d15ed1c019da4b3b3db9d48d299b3374ca78b4a8c6c1781
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# SDK بائنڈنگز اور فکسچرز کی گورننس

<div dir="rtl">

روڈمیپ میں WP1-E “docs/bindings” کو مختلف زبانوں کے بائنڈنگ اسٹیٹ کے لیے واحد
معتبر جگہ قرار دیتا ہے۔ یہ دستاویز بائنڈنگ انوینٹری، ری جنریشن کمانڈز، ڈرفٹ
گارڈز اور شواہد کے مقامات ریکارڈ کرتی ہے تاکہ GPU پیریٹی گیٹس (WP1-E/F/G) اور
کراس-SDK کیڈنس کونسل کے پاس ایک ہی حوالہ ہو۔

## مشترکہ گارڈ ریلز
- **کینانیکل پلے بک:** `docs/source/norito_binding_regen_playbook.md` میں Android،
  Swift، Python اور مستقبل کے بائنڈنگز کے لیے روٹیشن پالیسی، متوقع شواہد اور
  اسکیلیشن ورک فلو درج ہے۔
- **Norito اسکیما پیریٹی:** `scripts/check_norito_bindings_sync.py` (جسے
  `scripts/check_norito_bindings_sync.sh` کے ذریعے چلایا جاتا ہے اور CI میں
  `ci/check_norito_bindings_sync.sh` بطور گیٹ استعمال ہوتا ہے) Rust، Java یا
  Python اسکیما آرٹیفیکٹس کے ڈرفٹ پر builds روک دیتا ہے۔
- **کیڈنس واچ ڈاگ:** `scripts/check_fixture_cadence.py` فائلوں
  `artifacts/*_fixture_regen_state.json` کو پڑھتا ہے اور منگل/جمعہ (Android,
  Python) اور بدھ (Swift) کے ونڈوز نافذ کرتا ہے تاکہ روڈمیپ گیٹس کے پاس آڈٹ کے
  قابل ٹائم اسٹیمپس موجود ہوں۔

## بائنڈنگ میٹرکس

| بائنڈنگ | انٹری پوائنٹس | فکسچر/ری جنریشن کمانڈ | ڈرفٹ گارڈز | شواہد |
|---------|----------------|------------------------|-----------|-------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (اختیاری `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## بائنڈنگ تفصیلات

### Android (Java)
Android SDK `java/iroha_android/` میں ہے اور `scripts/android_fixture_regen.sh`
سے تیار ہونے والی کینانیکل Norito فکسچرز استعمال کرتا ہے۔ یہ ہیلپر Rust ٹول چین
سے تازہ `.norito` بلاپس ایکسپورٹ کرتا ہے، `artifacts/android_fixture_regen_state.json`
اپ ڈیٹ کرتا ہے اور کیڈنس میٹا ڈیٹا ریکارڈ کرتا ہے جسے
`scripts/check_fixture_cadence.py` اور گورننس ڈیش بورڈز استعمال کرتے ہیں۔ ڈرفٹ
کی نشاندہی `scripts/check_android_fixtures.py` (جو `ci/check_android_fixtures.sh`
سے جڑا ہے) اور `java/iroha_android/run_tests.sh` سے ہوتی ہے، جو JNI بائنڈنگز،
WorkManager قطار ری پلے اور StrongBox فال بیکس چیک کرتا ہے۔ روٹیشن شواہد، ناکامی
نوٹس اور rerun ٹرانسکرپٹس `artifacts/android/fixture_runs/` میں رہتے ہیں۔

### Swift (macOS/iOS)
`IrohaSwift/` وہی Norito payloads `scripts/swift_fixture_regen.sh` کے ذریعے
ریفلیکٹ کرتا ہے۔ یہ اسکرپٹ روٹیشن مالک، کیڈنس لیبل اور سورس (`live` بمقابلہ
`archive`) کو `artifacts/swift_fixture_regen_state.json` میں ریکارڈ کرتا ہے اور
کیڈنس چیکر کو فیڈ دیتا ہے۔ `scripts/swift_fixture_archive.py` مینٹینرز کو Rust
سے بنے آرکائیوز شامل کرنے دیتا ہے؛ `scripts/check_swift_fixtures.py` اور
`ci/check_swift_fixtures.sh` بائٹ لیول پیریٹی اور SLA عمر کی حدود نافذ کرتے ہیں،
جبکہ `scripts/swift_fixture_regen.sh` دستی روٹیشنز کے لیے `SWIFT_FIXTURE_EVENT_TRIGGER`
کو سپورٹ کرتا ہے۔ اسکیلیشن فلو، KPI اور ڈیش بورڈز `docs/source/swift_parity_triage.md`
اور `docs/source/sdk/swift/` میں کیڈنس بریفز کے تحت درج ہیں۔

### Python
Python کلائنٹ (`python/iroha_python/`) Android فکسچرز شیئر کرتا ہے۔
`scripts/python_fixture_regen.sh` چلانے سے تازہ `.norito` payloads آتے ہیں،
`python/iroha_python/tests/fixtures/` ریفریش ہوتا ہے، اور روڈمیپ کے بعد پہلی
روٹیشن کے ساتھ `artifacts/python_fixture_regen_state.json` میں کیڈنس میٹا ڈیٹا
اخراج ہوگا۔ `scripts/check_python_fixtures.py` اور
`python/iroha_python/scripts/run_checks.sh` pytest، mypy، ruff اور فکسچر پیریٹی
کو مقامی طور پر اور CI میں گیٹ کرتے ہیں۔ end-to-end ڈاکس
(`docs/source/sdk/python/…`) اور ری جن پلے بک بتاتے ہیں کہ Android اوونرز کے ساتھ
روٹیشن کیسے کوآرڈینیٹ کی جائے۔

### JavaScript
`javascript/iroha_js/` مقامی `.norito` فائلوں پر منحصر نہیں ہے، لیکن WP1-E اس کی
ریلیز شواہد کو ٹریک کرتا ہے تاکہ GPU CI لینز مکمل provenance وراثت میں لے سکیں۔
ہر ریلیز `npm run release:provenance` کے ذریعے provenance محفوظ کرتی ہے (جسے
`javascript/iroha_js/scripts/record-release-provenance.mjs` طاقت دیتا ہے)، SBOM
بنڈلز `scripts/js_sbom_provenance.sh` سے بناتا اور سائن کرتا ہے،
`scripts/js_signed_staging.sh` کے ذریعے سائنڈ اسٹیجنگ چلاتا ہے، اور رجسٹری
آرٹیفیکٹ کو `javascript/iroha_js/scripts/verify-release-tarball.mjs` سے ویریفائی
کرتا ہے۔ نتیجے کے میٹا ڈیٹا `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`,
`artifacts/js/sbom/` اور `artifacts/js/verification/` میں جاتے ہیں، جو روڈمیپ JS5/JS6
اور WP1-F رنز کے لیے متعین شواہد فراہم کرتے ہیں۔ `docs/source/sdk/js/` میں پبلشنگ
پلے بک پوری آٹومیشن کو جوڑتی ہے۔

</div>
