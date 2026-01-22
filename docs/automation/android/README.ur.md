---
lang: ur
direction: rtl
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 27b5ac3c7adb19a87f0b3d076f3c9618b188602898ed3954808ac9f7a52b3a62
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Android دستاویزات آٹومیشن بیس لائن (AND5)

<div dir="rtl">

روڈمیپ آئٹم AND5 تقاضا کرتا ہے کہ دستاویزات، لوکلائزیشن اور پبلشنگ کی آٹومیشن
AND6 (CI & Compliance) شروع ہونے سے پہلے آڈٹ کے قابل ہو۔ یہ فولڈر اُن کمانڈز،
آؤٹ پٹس اور شواہد کے لے آؤٹ کو ریکارڈ کرتا ہے جن کا حوالہ AND5/AND6 دیتے ہیں،
اور یہ منصوبوں `docs/source/sdk/android/developer_experience_plan.md` اور
`docs/source/sdk/android/parity_dashboard_plan.md` میں درج منصوبہ بندی کی عکاسی
کرتا ہے۔

## پائپ لائنز اور کمانڈز

| کام | کمانڈز | متوقع آؤٹ پٹس | نوٹس |
|-----|--------|--------------|------|
| لوکلائزیشن stubs سنک | `python3 scripts/sync_docs_i18n.py` (اختیاری طور پر ہر رن میں `--lang <code>` دیں) | `docs/automation/android/i18n/<timestamp>-sync.log` میں لاگ فائل، ساتھ ہی ترجمہ شدہ stubs کے کمٹس | `docs/i18n/manifest.json` کو ترجمہ شدہ stubs کے ساتھ ہم آہنگ رکھتا ہے؛ لاگ میں استعمال شدہ زبانوں کے کوڈ اور بیس لائن میں شامل کمٹ درج ہوتے ہیں۔ |
| Norito فکسچر + پیریٹی ویریفیکیشن | `ci/check_android_fixtures.sh` (جس میں `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json` چلتا ہے) | تیار کردہ JSON خلاصہ `docs/automation/android/parity/<stamp>-summary.json` میں کاپی کریں | `java/iroha_android/src/test/resources` کے payloads، manifest hashes اور دستخط شدہ فکسچر لمبائیاں چیک کرتا ہے۔ خلاصہ `artifacts/android/fixture_runs/` میں کیڈنس ثبوت کے ساتھ منسلک کریں۔ |
| نمونہ manifest اور پبلشنگ ثبوت | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (ٹیسٹ + SBOM + provenance چلاتا ہے) | provenance بنڈل میٹا ڈیٹا اور `docs/source/sdk/android/samples/` سے بننے والا `sample_manifest.json` جسے `docs/automation/android/samples/<version>/` میں رکھا جائے | AND5 نمونہ ایپس کو ریلیز آٹومیشن سے جوڑتا ہے: تیار شدہ manifest، SBOM hash اور provenance لاگ بیٹا ریویو کے لیے محفوظ کریں۔ |
| پیریٹی ڈیش بورڈ فیڈ | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` کے بعد `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | `metrics.prom` کا اسنیپ شاٹ یا Grafana JSON ایکسپورٹ `docs/automation/android/parity/<stamp>-metrics.prom` میں کاپی کریں | ڈیش بورڈ پلان کو فیڈ کرتا ہے تاکہ AND5/AND7 غلط سبمشن کاؤنٹرز اور ٹیلی میٹری اپنانے کی تصدیق کر سکیں۔ |

## شواہد کی محفوظ سازی

1. **ہر چیز کو ٹائم اسٹیمپ دیں۔** فائلوں کو UTC ٹائم اسٹیمپ (`YYYYMMDDTHHMMSSZ`)
   کے ساتھ نام دیں تاکہ پیریٹی ڈیش بورڈز، گورننس منٹس اور شائع شدہ ڈاکس ایک ہی
   رن کو ریفرنس کر سکیں۔
2. **کمٹ ریفرنس کریں۔** ہر لاگ میں رن کا کمٹ ہیش اور متعلقہ کنفیگریشن (مثلاً
   `ANDROID_PARITY_PIPELINE_METADATA`) شامل ہونا چاہیے۔ اگر پرائیویسی کے باعث
   ردوبدل درکار ہو تو نوٹ اور محفوظ والٹ کا لنک شامل کریں۔
3. **کم سے کم سیاق و سباق محفوظ کریں۔** صرف ساختہ خلاصے (JSON، `.prom`, `.log`)
   کو ورژن کیا جاتا ہے۔ بھاری آؤٹ پٹس (APK بنڈلز، اسکرین شاٹس) `artifacts/` یا
   آبجیکٹ اسٹوریج میں رہیں، اور دستخط شدہ ہیش لاگ میں درج کریں۔
4. **اسٹیٹس اینٹریز اپ ڈیٹ کریں۔** جب AND5 کے سنگِ میل `status.md` میں آگے
   بڑھیں تو متعلقہ فائل (مثلاً
   `docs/automation/android/parity/20260324T010203Z-summary.json`) کا حوالہ دیں
   تاکہ آڈیٹرز CI لاگز کھنگالے بغیر بیس لائن تک پہنچ سکیں۔

یہ لے آؤٹ AND6 کی "docs/automation بیس لائنز آڈٹ کے لیے دستیاب" شرط پوری کرتا
ہے اور Android دستاویزات پروگرام کو شائع شدہ منصوبوں کے ساتھ ہم آہنگ رکھتا ہے۔

</div>
