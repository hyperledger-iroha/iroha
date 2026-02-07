---
lang: ur
direction: rtl
source: docs/portal/docs/reference/account-address-status.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: اکاؤنٹ ایڈریس اسٹیٹس
عنوان: اکاؤنٹ ایڈریس کی تعمیل
تفصیل: EDDR-2 فکسچر ورک فلو کا خلاصہ اور SDK ٹیمیں کس طرح مطابقت پذیری میں رہتی ہیں۔
---

کیننیکل ایڈی آر -2 پیکیج (`fixtures/account/address_vectors.json`) IH58 (ترجیحی) ، کمپریسڈ (`sora` ، دوسرا بہترین ؛ نصف/مکمل چوڑائی) ، ملٹی سائنچر اور منفی فکسچر پر قبضہ کرتا ہے۔ ہر سطح SDK + Torii کسی بھی کوڈیک بڑھنے کی پیداوار تک پہنچنے سے پہلے اس کا پتہ لگانے کے لئے اسی JSON پر انحصار کرتا ہے۔ یہ صفحہ داخلی حیثیت مختصر (جڑ کے ذخیرے میں `docs/source/account_address_status.md`) کی عکاسی کرتا ہے تاکہ پورٹل کے قارئین مونو ریپو کو تلاش کیے بغیر بہاؤ سے مشورہ کرسکیں۔

## پیکیج کو دوبارہ تخلیق یا تصدیق کریں

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

جھنڈے:

- `--stdout` - JSON کو ایڈہاک معائنہ کے لئے stdout پر آؤٹ پٹ کرتا ہے۔
- `--out <path>` - ایک مختلف راستے پر لکھتا ہے (جیسے مقامی طور پر تبدیلیوں کا موازنہ کرتے وقت)۔
- `--verify` - ورکنگ کاپی کو نئے تیار کردہ مواد کے خلاف موازنہ کرتا ہے (`--stdout` کے ساتھ مل نہیں سکتا)۔

CI ورک فلو ** ایڈریس ویکٹر ڈرفٹ ** چلتا ہے `cargo xtask address-vectors --verify`
ہر بار جب حقیقت ، جنریٹر یا دستاویزات جائزہ لینے والوں کو فوری طور پر آگاہ کرنے کے ل change تبدیل ہوجاتی ہیں۔

## حقیقت کون کھاتا ہے؟

| سطح | توثیق |
| --------- | -------------- |
| مورچا ڈیٹا ماڈل | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (سرور) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| جاوا اسکرپٹ SDK | `javascript/iroha_js/test/address.test.js` |
| سوئفٹ ایس ڈی کے | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| اینڈروئیڈ ایس ڈی کے | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

ہر استعمال کیننیکل بائٹس + IH58 + کمپریسڈ انکوڈنگز کا ایک گول سفر کرتا ہے اور اس بات کی تصدیق کرتا ہے کہ Norito اسٹائل کی غلطی والے کوڈ منفی معاملات کے لئے حقیقت سے ملتے ہیں۔

## کیا آپ کو آٹومیشن کی ضرورت ہے؟

ریلیز ٹولز ہیلپر کے ساتھ فکسچر ریفریشس کو خودکار کرسکتے ہیں
`scripts/account_fixture_helper.py` ، جو کاپی/پیسٹ اقدامات کے بغیر کیننیکل پیکیج کی تصدیق یا تصدیق کرتا ہے:

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

مددگار `--source` یا `IROHA_ACCOUNT_FIXTURE_URL` ماحول متغیر کے اوور رائڈس کو قبول کرتا ہے تاکہ SDK CI ملازمتیں ان کے ترجیحی آئینے کی طرف اشارہ کریں۔ جب `--metrics-out` فراہم کیا جاتا ہے تو ، مددگار `account_address_fixture_check_status{target="..."}` لکھتا ہے اور ساتھ ہی کیننیکل SHA-256 ڈائجسٹ (`account_address_fixture_remote_info`) کے ساتھ ساتھ Prometheus ٹیکسٹ فائل کلیکٹرز اور I18800000000000000000027X DASHDX DASHS DASHS DASHS DASHENT میں بھی ہے۔ انتباہ جب ایک ہدف `0` کی اطلاع دیتا ہے۔ ملٹی سطح کے آٹومیشن کے لئے `ci/account_fixture_metrics.sh` ریپر (بار بار `--target label=path[::source]` کو قبول کرتا ہے) استعمال کریں تاکہ آن کال ٹیمیں نوڈ ایکسپلٹر ٹیکسٹ فائل کلکٹر کو ایک واحد کنسولیڈیٹڈ `.prom` فائل شائع کریں۔

## کیا آپ کو مکمل مختصر کی ضرورت ہے؟

ایڈریس ڈھانچہ آر ایف سی (`docs/account_structure.md`) کے ساتھ ساتھ `docs/source/account_address_status.md` میں مکمل EDDR-2 تعمیل کی حیثیت (مالکان ، مانیٹرنگ پلان ، کھلی کارروائیوں) میں رہتی ہے۔ اس صفحے کو فوری آپریشنل یاد دہانی کے طور پر استعمال کریں۔ گہرائی میں رہنماؤں کے لئے ، ریپو دستاویزات دیکھیں۔