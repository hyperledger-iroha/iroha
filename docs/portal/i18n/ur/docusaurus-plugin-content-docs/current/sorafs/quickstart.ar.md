---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS میں فوری آغاز کریں

یہ عملی گائیڈ آپ کو SF-1 ڈٹرمینسٹک چنکر پروفائل کے ذریعے چلتا ہے ،
منشور پر دستخط کرنے ، ملٹی فراہم کرنے والے بازیافت کا راستہ جو SoraFS اسٹوریج پائپ لائن کی حمایت کرتا ہے۔
اس کو [منشور پائپ لائن میں ڈوبکی] کے ساتھ متوازن کریں (manifest-pipeline.md)
ڈیزائن نوٹ اور کمانڈ لائن جھنڈوں کے حوالہ کے لئے۔

## بنیادی ضروریات

- مقامی طور پر کاپی شدہ ورک اسپیس کے ساتھ زنگ ٹول (`rustup update`)۔
- اختیاری: [اوپن ایس ایس ایل مطابقت پذیر ED25519 کلیدی جوڑی] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  منشور پر دستخط کرنے کے لئے۔
- اختیاری: اگر آپ Docusaurus گیٹ وے کا پیش نظارہ کرنے کا ارادہ رکھتے ہیں تو نوڈ ڈاٹ جے ایس ≥ 18۔

مفید CLI پیغامات کو ظاہر کرنے کے لئے تجربے کے دوران `export RUST_LOG=info` سیٹ کریں۔

## 1. ضروری فکسچر کو اپ ڈیٹ کریں

معیاری SF-1 chunking ویکٹرز کو دوبارہ تخلیق کریں۔ آرڈر لفافے بھی جاری کرتا ہے
جب `--signing-key` فراہم کیا گیا تو منشور پر دستخط ہوئے۔ ترقی کے دوران `--allow-unsigned` استعمال کریں
صرف مقامی۔

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

نتائج:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (اگر دستخط شدہ)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2۔ پے لوڈ کو تقسیم کریں اور منصوبہ چیک کریں

بے ترتیب فائل یا محفوظ شدہ دستاویزات کو تقسیم کرنے کے لئے `sorafs_chunker` استعمال کریں:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

بنیادی فیلڈز:

- `profile` / `break_mask` - `sorafs.sf1@1.0.0` ٹرانزیکشنز کی تصدیق کرتا ہے۔
- `chunks[]` - آفسیٹس ، آرڈرڈ لمبائی اور بلیک 3 فنگر پرنٹ۔

بڑے فکسچر کے ل ، ، یہ یقینی بنانے کے لئے کہ تقسیم کی بنیاد پر مبنی رجعت ٹیسٹ چلائیں
بہاؤ اور بیچوں کے ذریعہ:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. ایک منشور کی تعمیر اور دستخط کریں

ٹکڑوں ، عرفی ناموں اور گورننس کے دستخطوں کو ایک منشور میں استعمال کرتے ہوئے ...
`sorafs-manifest-stub`۔ نیچے دیئے گئے کمانڈ میں ایک فائل کے لئے پے لوڈ کو ظاہر کیا گیا ہے۔ پیکیجوں تک ڈائریکٹری کا راستہ پاس کریں
درخت (سی ایل آئی لغت میں چلتا ہے)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` دیکھیں:

- `chunking.chunk_digest_sha3_256` - SHA3 آفسیٹس/لمبائی کا فنگر پرنٹ ، مخصوص فکسچر سے ملاپ کرنا
  چنکر کے ساتھ
- `manifest.manifest_blake3` - بلیک 3 کے فنگر پرنٹ نے منشور لفافے میں دستخط کیے۔
- `chunk_fetch_specs[]` - آرکیسٹریٹرز کے لئے لینے کی ہدایات کا آرڈر دیں۔

جب آپ حقیقی دستخط فراہم کرنے کے لئے تیار ہوں تو ، دلائل `--signing-key` اور `--signer` شامل کریں۔
کمانڈ لفافے کو لکھنے سے پہلے ہر دستخط ED25519 کی تصدیق کرتا ہے۔

## 4. ملٹی سرور لوپ بیک سمیلیٹر

ایک یا زیادہ فراہم کنندگان کے خلاف حصوں کے منصوبے کو دوبارہ چلانے کے لئے ڈویلپمنٹ بازیافت سی ایل آئی کا استعمال کریں۔
یہ CI میں اور آرکیسٹرل ماڈلنگ میں دھواں کے ٹیسٹوں کے لئے مثالی ہے۔

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

تصدیق:

- `payload_digest_hex` کو مینی فیسٹ رپورٹ سے ملنا چاہئے۔
- `provider_reports[]` ہر فراہم کنندہ کے لئے پاس/فیل نمبر دکھاتا ہے۔
-`chunk_retry_total` کی ایک غیر صفر قیمت بیک پریشر ایڈجسٹمنٹ کو نمایاں کرتی ہے۔
- `--max-peers=<n>` پاس کریں تاکہ فراہم کرنے والوں کی تعداد کو محدود کیا جاسکے اور CI تخروپن کو مرکوز رکھنے کے لئے شیڈول کیا جائے
  پرائمری امیدواروں پر۔
- `--retry-budget=<n>` فی حصہ دوبارہ کوششوں کی پہلے سے طے شدہ تعداد سے تجاوز کر گیا ہے (3)
  جب غلطیوں کو انجیکشن لگاتے ہو تو آرکسٹریٹر ڈپس کی کھوج کو تیز کرنے کے ل .۔

فوری طور پر ناکام ہونے کے لئے `--expect-payload-digest=<hex>` اور `--expect-payload-len=<bytes>` شامل کریں
جب تعمیر نو پے لوڈ منشور سے انحراف کرتا ہے۔

## 5. اگلے اقدامات- ** گورننس انضمام ** - مینی فیسٹ فنگر پرنٹ اور `manifest_signatures.json` بورڈ ورک فلو کو منتقل کریں
  تاکہ پن رجسٹری دستیابی کا اعلان کرسکے۔
- ** رجسٹری کے ساتھ بات چیت کرنا ** - دیکھیں [`sorafs/chunker_registry.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  نئے پروفائلز رجسٹر کرنے سے پہلے۔ آٹومیشن کو معیاری پروسیسرز کے حق میں ہونا چاہئے
  (`namespace.name@semver`) ڈیجیٹل IDs پر۔
- ** سی آئی آٹومیشن ** - دستاویزات اور فکسچر شائع کرنے کے لئے تعیناتی کی رہائی پائپ لائنوں میں مذکورہ بالا احکامات شامل کریں
  دستخط شدہ میٹا ڈیٹا کے ساتھ نمونے کا مظہر لازمی ہے۔