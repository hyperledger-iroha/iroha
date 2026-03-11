---
lang: ur
direction: rtl
source: docs/portal/docs/reference/account-address-status.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: اکاؤنٹ ایڈریس اسٹیٹس
عنوان: اکاؤنٹ کے عنوان کی تعمیل
تفصیل: فکسچر ایڈر -2 ورک فلو کا خلاصہ اور ایس ڈی کے ٹیمیں مطابقت پذیری میں کیسے رہتی ہیں۔
---

معیاری پیکیج ایڈ آر 2 (`fixtures/account/address_vectors.json`) I105 فکسچر ، کمپریسڈ (`sora` ، دوسرا بہترین ؛ نصف/مکمل چوڑائی) ، ملٹی سائنچر اور منفی پر قبضہ کرتا ہے۔ ہر SDK + Torii انٹرفیس اسی JSON پر انحصار کرتا ہے تاکہ ہم کوڈیک میں کسی بھی انحراف کی پیداوار تک پہنچنے سے پہلے اس کا پتہ لگاسکیں۔ یہ صفحہ داخلی حیثیت میمو (`docs/source/account_address_status.md` کو مخزن کی جڑ میں) کی عکاسی کرتا ہے تاکہ پورٹل کے قارئین مونو ریپو میں کھودے بغیر ورک فلو میں واپس آسکیں۔

## پیکیج کو دوبارہ تخلیق یا تصدیق کریں

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

جھنڈے:

- `--stdout` - فوری اسکیننگ کے لئے JSON کو stdout پر پرنٹ کرتا ہے۔
- `--out <path>` - ایک مختلف راہ پر لکھتا ہے (جیسے مقامی تبدیلیوں کا موازنہ کرتے وقت)۔
- `--verify` - ورکنگ کاپی کا موازنہ نئے تیار کردہ مواد کے ساتھ کرتا ہے (`--stdout` کے ساتھ مل نہیں سکتا)۔

CI روٹ ** ایڈریس ویکٹر ڈرفٹ ** کمانڈ `cargo xtask address-vectors --verify` چلاتا ہے
جب حقیقت ، جنریٹر ، یا دستاویزات تبدیل ہوجاتی ہیں تو ، آڈیٹرز کو فوری طور پر متنبہ کریں۔

## حقیقت کو کون کھاتا ہے؟

| سطح | توثیق |
| --------- | -------------- |
| مورچا ڈیٹا ماڈل | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (سرور) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| جاوا اسکرپٹ SDK | `javascript/iroha_js/test/address.test.js` |
| سوئفٹ ایس ڈی کے | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| اینڈروئیڈ ایس ڈی کے | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

ہر کنٹرول معیاری بائٹس + I105 + کمپریسڈ انکوڈنگز کا ایک گول ٹرپ انجام دیتا ہے اور اس کی تصدیق کرتا ہے کہ قسم Norito کے غلطی والے کوڈ منفی معاملات کے لئے حقیقت سے ملتے ہیں۔

## کیا آپ کو آٹومیشن کی ضرورت ہے؟

ریلیز ٹولز پلگ ان کے ذریعہ فکسچر اپ ڈیٹس کو پروگرام کرسکتے ہیں
`scripts/account_fixture_helper.py` ، جو کاپی/پیسٹ اقدامات کے بغیر معیاری پیکیج لاتا ہے:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

مددگار `--source` یا `IROHA_ACCOUNT_FIXTURE_URL` ماحول متغیر کے ذریعے اوور رائڈس کو قبول کرتا ہے تاکہ SDK کے CI کام ترجیحی آئینے کی طرف اشارہ کرسکیں۔ جب `--metrics-out` گزر جاتا ہے تو ، مددگار SHA-256 ڈائجسٹ (`account_address_fixture_remote_info`) کے ساتھ `account_address_fixture_check_status{target="…"}` لکھتا ہے تاکہ Prometheus میں ٹیکسٹ فائل جمع کرنے والے اور ```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
``` میں `account_address_fixture_status` میں یہ ثابت ہوسکتا ہے۔ جب کوئی ہدف `0` کی قیمت تک پہنچ جاتا ہے تو انتباہ کو چالو کریں۔ ملٹی سطح کے آٹومیشن کے لئے `ci/account_fixture_metrics.sh` ریپر (ڈپلیکیٹ `--target label=path[::source]` کو قبول کرتا ہے) استعمال کریں تاکہ شفٹ ٹیمیں نوڈس ایکسپورٹر کے ٹیکسٹ فائل کلیکٹر میں ایک یونیفائیڈ `.prom` فائل کو تعینات کرسکیں۔

## مکمل خلاصہ کی ضرورت ہے؟

EDDR-2 کی تعمیل کی حیثیت مکمل کریں (مالکان ، نگرانی کا منصوبہ اور کھلی کام کی اشیاء)
ایڈریس ڈھانچہ آر ایف سی (`docs/account_structure.md`) کے ساتھ ذخیرہ میں `docs/source/account_address_status.md` پر واقع ہے۔ اس صفحے کو فوری آپریشنل یاد دہانی کے طور پر استعمال کریں۔ تفصیلی رہنمائی کے لئے ذخیرہ دستاویزات کا حوالہ دیں۔