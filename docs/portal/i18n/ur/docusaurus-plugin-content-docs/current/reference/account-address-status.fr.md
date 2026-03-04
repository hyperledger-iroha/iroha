---
lang: ur
direction: rtl
source: docs/portal/docs/reference/account-address-status.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: اکاؤنٹ ایڈریس اسٹیٹس
عنوان: اکاؤنٹ ایڈریس کی تعمیل
تفصیل: EDDR-2 فکسچر ورک فلو اور SDK ٹیم کی ہم آہنگی کا خلاصہ۔
---

کیننیکل ایڈر -2 بنڈل (`fixtures/account/address_vectors.json`) IH58 (ترجیحی) ، کمپریسڈ (`sora` ، دوسری بہترین ؛ نصف/مکمل چوڑائی) ، ملٹی سائنچر اور منفی فکسچر پر قبضہ کرتا ہے۔ ہر SDK سطح + Torii پیداوار سے پہلے کسی بھی کوڈیک بڑھے جانے کا پتہ لگانے کے لئے اسی JSON پر انحصار کرتا ہے۔ یہ صفحہ داخلی حیثیت کے مختصر (`docs/source/account_address_status.md` میں جڑ کے ذخیرے میں) کی عکاسی کرتا ہے تاکہ پورٹل کے قارئین مونو ریپو کی تلاش کیے بغیر ورک فلو سے مشورہ کرسکیں۔

## بنڈل کو دوبارہ تخلیق کریں یا چیک کریں

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

جھنڈے:

- `--stdout` - ایڈہاک معائنہ کے لئے JSON کو stdout سے خارج کرتا ہے۔
- `--out <path>` - کسی اور راستے پر لکھا گیا (جیسے مقامی موازنہ کے دوران)۔
- `--verify` - ورکنگ کاپی کو تازہ تیار کردہ مواد کے ساتھ موازنہ کرتا ہے (`--stdout` کے ساتھ مل نہیں سکتا)۔

CI ورک فلو ** ایڈریس ویکٹر ڈرفٹ ** `cargo xtask address-vectors --verify` پر عملدرآمد کرتا ہے
جب بھی حقیقت میں ، جنریٹر یا دستاویزات تبدیل ہوجاتے ہیں تاکہ جائزہ لینے والوں کو فوری طور پر الرٹ کیا جاسکے۔

## حقیقت کون کھاتا ہے؟

| علاقہ | توثیق |
| -------- | -------------- |
| مورچا ڈیٹا ماڈل | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (سرور) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| جاوا اسکرپٹ SDK | `javascript/iroha_js/test/address.test.js` |
| سوئفٹ ایس ڈی کے | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| اینڈروئیڈ ایس ڈی کے | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

ہر استعمال کیننیکل بائٹس + IH58 + کمپریسڈ انکوڈنگز اور چیکوں کا ایک گول سفر کرتا ہے جو Norito اسٹائل کی غلطی والے کوڈ منفی معاملات کے لئے حقیقت سے ملتے ہیں۔

## آٹومیشن کی ضرورت ہے؟

ریلیز ٹولز ہیلپر کے ساتھ فکسچر ریفریشس کو خودکار کرسکتے ہیں
`scripts/account_fixture_helper.py` ، جو کاپی/چسپاں کے بغیر کیننیکل بنڈل کو بازیافت یا تصدیق کرتا ہے:

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

مددگار `--source` یا ماحولیاتی متغیر `IROHA_ACCOUNT_FIXTURE_URL` کو اوور رائڈس قبول کرتا ہے تاکہ SDK CI ملازمتیں ان کے ترجیحی آئینے کی طرف اشارہ کریں۔ جب `--metrics-out` فراہم کیا جاتا ہے تو ، مددگار `account_address_fixture_check_status{target="..."}` کے ساتھ ساتھ کیننیکل SHA-256 ڈائجسٹ (`account_address_fixture_remote_info`) لکھتا ہے تاکہ ٹیکسٹ فائل جمع کرنے والے Prometheus اور ڈیش بورڈ Grafana Prometheus انتباہ جب ایک ہدف `0` کی اطلاع دیتا ہے۔ ملٹی سطح کے آٹومیشن کے لئے ، `ci/account_fixture_metrics.sh` ریپر (بار بار `--target label=path[::source]` کو قبول کرتا ہے) استعمال کریں تاکہ آن کال ٹیمیں نوڈ ایکسپلیٹر ٹیکسٹ فائل کلکٹر کو ایک واحد کنسولیڈیٹڈ `.prom` فائل شائع کریں۔

## مکمل بریف کی ضرورت ہے؟

مکمل EDDR-2 تعمیل کی حیثیت (مالکان ، نگرانی کا منصوبہ ، کھلی کارروائیوں)
`docs/source/account_address_status.md` میں ذخیرہ کے اندر واقع ہے ، نیز RFC ایڈریس ڈھانچہ (`docs/account_structure.md`)۔ اس صفحے کو فوری آپریشنل یاد دہانی کے طور پر استعمال کریں۔ گہرائی میں گائیڈ کے لئے ریپو دستاویزات کا حوالہ دیں۔