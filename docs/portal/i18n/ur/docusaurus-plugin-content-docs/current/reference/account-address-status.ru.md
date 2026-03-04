---
lang: ur
direction: rtl
source: docs/portal/docs/reference/account-address-status.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: اکاؤنٹ ایڈریس اسٹیٹس
عنوان: اکاؤنٹ ایڈریس مماثل
تفصیل: EDDR-2 فکسچر ورک فلو اور SDK کمانڈ کی ہم آہنگی کا خلاصہ۔
---

کیننیکل ایڈی آر -2 پیکیج (`fixtures/account/address_vectors.json`) میں فکسچر IH58 (ترجیحی) ، کمپریسڈ (`sora` ، دوسرا بہترین ؛ نصف/مکمل چوڑائی) ، ملٹی سائنچر اور منفی شامل ہیں۔ ہر SDK+Torii سطح اسی JSON پر انحصار کرتی ہے تاکہ پیداوار میں جانے سے پہلے کوڈیک ڈرفٹ کا پتہ لگ سکے۔ یہ صفحہ داخلی حیثیت کے مختصر (`docs/source/account_address_status.md` کو مخزن کی جڑ میں) کی عکاسی کرتا ہے تاکہ پورٹل کے قارئین مونو ریپو میں کھودنے کے بغیر ورک فلو تک رسائی حاصل کرسکیں۔

## پیکیج کو دوبارہ تخلیق کریں یا چیک کریں

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

جھنڈے:

- `--stdout` - ایڈہاک تصدیق کے لئے JSON کو STDOUT میں آؤٹ کرتا ہے۔
- `--out <path>` - کسی اور راستے پر لکھتا ہے (مثال کے طور پر ، جب مقامی طور پر تبدیلیوں کا موازنہ کرتے ہو)۔
- `--verify` - ورکنگ کاپی کو نئے تیار کردہ مواد کے ساتھ موازنہ کرتا ہے (`--stdout` کے ساتھ مل نہیں سکتا)۔

CI ورک فلو ** ایڈریس ویکٹر ڈرفٹ ** شروع ہوتا ہے `cargo xtask address-vectors --verify`
ہر بار جب کوئی حقیقت ، جنریٹر یا دستاویزات جائزہ لینے والوں کو فوری طور پر آگاہ کرنے میں تبدیل ہوجاتی ہیں۔

## حقیقت کون استعمال کرتا ہے؟

| سطح | توثیق |
| --------- | ----------- |
| مورچا ڈیٹا ماڈل | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (سرور) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| جاوا اسکرپٹ SDK | `javascript/iroha_js/test/address.test.js` |
| سوئفٹ ایس ڈی کے | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| اینڈروئیڈ ایس ڈی کے | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

ہر کنٹرول کمپریسڈ انکوڈنگز کے کیننیکل بائٹس + IH58 + کا ایک گول سفر انجام دیتا ہے اور چیک کرتا ہے کہ Norito اسٹائل کی غلطی والے کوڈ منفی معاملات کے لئے حقیقت سے ملتے ہیں۔

## آٹومیشن کی ضرورت ہے؟

ریلیز ٹولنگ ہیلپر کے ذریعہ فکسچر اپڈیٹس کو خود کار بنا سکتی ہے
`scripts/account_fixture_helper.py` ، جو کاپی/پیسٹ کے بغیر کیننیکل پیکٹ حاصل کرتا ہے یا چیک کرتا ہے:

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

ہیلپر `--source` یا ماحولیاتی متغیر `IROHA_ACCOUNT_FIXTURE_URL` کے ذریعے اوور رائڈس کو قبول کرتا ہے تاکہ SDK CI ملازمتیں ترجیحی آئینے کی وضاحت کرسکیں۔ جب `--metrics-out` کو پاس کرتے ہو تو ، مددگار `account_address_fixture_check_status{target="…"}` لکھتا ہے اور ساتھ ہی کیننیکل SHA-256 ڈائجسٹ (`account_address_fixture_remote_info`) کے ساتھ ساتھ Prometheus ٹیکسٹ فائل جمع کرنے والے اور ```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
``` DASHBORDION کی تصدیق کر سکتے ہیں۔ جب ہدف `0` کی اطلاع دیتا ہے تو ایک انتباہ مرتب کریں۔ ملٹی سطح کے آٹومیشن کے لئے ، ریپر `ci/account_fixture_metrics.sh` (Prometheus کو دہرانے کو قبول کرتا ہے) استعمال کریں تاکہ آن کال کمانڈز ٹیکسٹ فائل کلیکٹر نوڈ-ایکسپلٹر کو ایک واحد `.prom` فائل شائع کریں۔

## ایک مکمل مختصر کی ضرورت ہے؟

مکمل EDDR-2 تعمیل کی حیثیت (مالکان ، نگرانی کا منصوبہ ، کھلے کام)
`docs/source/account_address_status.md` میں ذخیرہ کے اندر ایڈریس ڈھانچہ آر ایف سی (`docs/account_structure.md`) کے ساتھ ذخیرہ کیا گیا ہے۔ براہ کرم اس صفحے کو فوری یاد دہانی کے طور پر استعمال کریں۔ گہرائی میں مدد کے لئے براہ کرم ذخیرہ دستاویزات کا حوالہ دیں۔