---
lang: ur
direction: rtl
source: docs/portal/docs/reference/account-address-status.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: اکاؤنٹ ایڈریس اسٹیٹس
عنوان: اکاؤنٹ ایڈریس کی تعمیل
تفصیل: EDDR-2 حقیقت کے بہاؤ کا خلاصہ اور SDK ٹیمیں مطابقت پذیری میں کیسے رہتی ہیں۔
---

کیننیکل ایڈی آر -2 پیکیج (`fixtures/account/address_vectors.json`) I105 (ترجیحی) ، کمپریسڈ (`sora` ، دوسرا بہترین ؛ نصف/مکمل چوڑائی) ، ملٹی سائنچر اور منفی فکسچر پر قبضہ کرتا ہے۔ ہر SDK + Torii سطح کی پیداوار تک پہنچنے سے پہلے کسی بھی کوڈیک بڑھے جانے کا پتہ لگانے کے لئے وہی JSON استعمال کرتا ہے۔ یہ صفحہ داخلی حیثیت کے مختصر (`docs/source/account_address_status.md` میں جڑ کے ذخیرے میں) کی آئینہ دار ہے تاکہ پورٹل کے قارئین مونو ریپو کے ذریعے کھودے بغیر بہاؤ سے مشورہ کرسکیں۔

## پیکیج کو دوبارہ تخلیق کریں یا تصدیق کریں

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

جھنڈے:

- `--stdout` - ایڈہاک معائنہ کے لئے آؤٹ پٹ JSON STDOUT.
- `--out <path>` - ایک مختلف راستے پر لکھتا ہے (جیسے مقامی طور پر تبدیلیوں کا موازنہ کرتے وقت)۔
- `--verify` - ورکنگ کاپی کو نئے تیار کردہ مواد کے ساتھ موازنہ کرتا ہے (`--stdout` کے ساتھ مل نہیں سکتا)۔

CI ورک فلو ** ایڈریس ویکٹر ڈرفٹ ** چلتا ہے `cargo xtask address-vectors --verify`
جب بھی حقیقت ، جنریٹر یا دستاویزات جائزہ لینے والوں کو فوری طور پر آگاہ کرنے کے لئے تبدیل ہوجاتے ہیں۔

## حقیقت کون کھاتا ہے؟

| سطح | توثیق |
| --------- | -------------- |
| مورچا ڈیٹا ماڈل | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (سرور) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| جاوا اسکرپٹ SDK | `javascript/iroha_js/test/address.test.js` |
| سوئفٹ ایس ڈی کے | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| اینڈروئیڈ ایس ڈی کے | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

ہر ہارنس راؤنڈ ٹرپس کیننیکل بائٹس + I105 + کمپریسڈ انکوڈنگز اور جانچ پڑتال کرتا ہے کہ آیا Norito طرز کے غلطی والے کوڈ منفی معاملات کے لئے حقیقت سے ملتے ہیں۔

## کیا آپ کو آٹومیشن کی ضرورت ہے؟

ریلیز ٹولز ہیلپر کے ساتھ فکسچر ریفریشس کو خودکار کرسکتے ہیں
`scripts/account_fixture_helper.py` ، جو کاپی/پیسٹ کے بغیر کیننیکل پیکیج کو حاصل کرتا ہے یا چیک کرتا ہے:

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

مددگار `--source` یا `IROHA_ACCOUNT_FIXTURE_URL` ماحول متغیر کے اوور رائڈس کو قبول کرتا ہے تاکہ SDK CI ملازمتیں ان کے ترجیحی آئینے کی طرف اشارہ کریں۔ جب `--metrics-out` فراہم کیا جاتا ہے تو ، مددگار `account_address_fixture_check_status{target=\"...\"}` کے ساتھ ساتھ کیننیکل SHA-256 ڈائجسٹ (`account_address_fixture_remote_info`) لکھتا ہے تاکہ Prometheus کے ٹیکسٹ فائل جمع کرنے والے اور ڈیش بورڈ Grafana ```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
``` ایکس کے ساتھ ملتے ہیں۔ جب کوئی ہدف `0` کی اطلاع دیتا ہے تو الرٹ تیار کریں۔ ملٹی سطح کے آٹومیشن کے لئے `ci/account_fixture_metrics.sh` ریپر (بار بار `--target label=path[::source]` کو قبول کرتا ہے) استعمال کریں تاکہ کال ٹیمیں نوڈ ایکسپلیٹر کے ٹیکسٹ فائل کلکٹر کو ایک واحد کنسولیڈیٹڈ `.prom` فائل شائع کریں۔

## کیا آپ کو مکمل بریف کی ضرورت ہے؟

EDDR-2 کی تعمیل کی حیثیت مکمل کریں (مالکان ، نگرانی کا منصوبہ ، کھلی ایکشن آئٹمز)
یہ ایڈریس ڈھانچہ آر ایف سی (`docs/account_structure.md`) کے ساتھ ساتھ `docs/source/account_address_status.md` پر ذخیرے کے اندر واقع ہے۔ اس صفحے کو فوری آپریشنل یاد دہانی کے طور پر استعمال کریں۔ تفصیلی رہنمائی کے لئے ، ذخیرہ دستاویزات دیکھیں۔