---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# فوری آغاز SoraFS

یہ عملی گائیڈ ڈٹرمینسٹک SF-1 چنکر پروفائل کا جائزہ لیتا ہے ،
منشور پر دستخط اور ملٹی وینڈر کی بازیابی کا بہاؤ جو
اسٹوریج پائپ لائن SoraFS کو دبائیں۔ اس کے ساتھ مکمل کریں
[منشور پائپ لائن گہری تجزیہ] (manifest-pipeline.md)
ڈیزائن نوٹ اور سی ایل آئی جھنڈوں کے حوالہ کے ل .۔

## شرائط

- ٹولچین زنگ (`rustup update`) ، مقامی طور پر کلونڈ ورک اسپیس۔
- اختیاری: [ED25519 کلیدی جوڑی اوپن ایس ایل کے ذریعہ تیار کردہ] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  منشور پر دستخط کرنے کے لئے۔
- اختیاری: اگر آپ پورٹل Docusaurus کا پیش نظارہ کرنے کا ارادہ رکھتے ہیں تو نوڈ ڈاٹ جے ایس ≥ 18۔

مفید CLI پیغامات کو ظاہر کرنے کے لئے جانچ کے دوران `export RUST_LOG=info` سیٹ کریں۔

## 1. ریفریش ڈٹرمینسٹک فکسچر

کیننیکل SF-1 کلپنگ ویکٹرز کو دوبارہ تخلیق کریں۔ کمانڈ بھی تیار کرتا ہے
جب `--signing-key` فراہم کیا جاتا ہے تو دستخط شدہ منشور لفافے ؛ استعمال کریں
`--allow-unsigned` صرف مقامی ترقی میں۔

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

نتائج:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (اگر دستخط شدہ)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2۔ ایک پے لوڈ کاٹ کر منصوبے کا معائنہ کریں

کسی صوابدیدی فائل یا محفوظ شدہ دستاویزات کے لئے `sorafs_chunker` استعمال کریں:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

کلیدی فیلڈز:

- `profile` / `break_mask` - `sorafs.sf1@1.0.0` کی ترتیبات کی تصدیق کرتا ہے۔
- `chunks[]` - ٹکڑوں کے آفسیٹس ، لمبائی اور بلیک 3 فنگر پرنٹ کا آرڈر دیا گیا۔

بڑے فکسچر کے لئے ، پریپٹیسٹ پر مبنی رجعت چلائیں
اس بات کو یقینی بنائیں کہ ہم آہنگی میں اسٹریمنگ اور بیچ سلائسنگ قیام کریں:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. ایک منشور کی تعمیر اور دستخط کریں

ایک میں حصہ ، عرفیت اور گورننس کے دستخطوں کو لپیٹیں
`sorafs-manifest-stub` کے ذریعے منشور۔ نیچے دیئے گئے کمانڈ میں پے لوڈ کی وضاحت کی گئی ہے
ایک فائل ؛ کسی درخت کو پیک کرنے کے لئے ڈائریکٹری کا راستہ پاس کریں (سی ایل آئی یہ کرتا ہے
لغت میں براؤز)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` کے لئے جانچ پڑتال کریں:

- `chunking.chunk_digest_sha3_256` - SHA3 آفسیٹس/لمبائی کا فنگر پرنٹ ، مساوی ہے
  چنکر فکسچر۔
- `manifest.manifest_blake3` - بلیک 3 امپرنٹ نے مینی فیسٹ لفافے میں دستخط کیے۔
- `chunk_fetch_specs[]` - آرکیسٹریٹرز کے لئے بحالی کی ہدایات کا آرڈر دیا گیا۔

جب آپ حقیقی دستخط فراہم کرنے کے لئے تیار ہوں تو ، دلائل شامل کریں
`--signing-key` اور `--signer`۔ کمانڈ اس سے پہلے ہر ED25519 دستخط کی جانچ پڑتال کرتا ہے
لفافہ لکھنے کے لئے۔

## 4. ملٹی وینڈر کی بازیابی کا نقالی کریں

ایک کے خلاف حصہ کے منصوبے کو دوبارہ چلانے کے لئے ڈویلپمنٹ بازیافت سی ایل آئی کا استعمال کریں
کئی سپلائرز۔ یہ CI دھواں ٹیسٹنگ اور پروٹو ٹائپنگ کے لئے مثالی ہے
آرکیسٹریٹر۔

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

تصدیق:- `payload_digest_hex` کو مینی فیسٹ رپورٹ سے ملنا چاہئے۔
- `provider_reports[]` فراہم کنندہ کے ذریعہ پاس/فیل گنتی کو بے نقاب کرتا ہے۔
-ایک غیر صفر `chunk_retry_total` بیک دباؤ ایڈجسٹمنٹ کو نمایاں کرتا ہے۔
- `--max-peers=<n>` کو پاس کریں تاکہ سپلائی کرنے والوں کی تعداد کو محدود کیا جاسکے
  پھانسی اور CI کے نقالی کو اہم امیدواروں پر مرکوز رکھیں۔
- `--retry-budget=<n>` فی حصہ (3) کی کوششوں کی پہلے سے طے شدہ تعداد کو تبدیل کرتا ہے تاکہ
  انجیکشن کے دوران آرکیسٹریٹر ریگریشنز کو زیادہ تیزی سے اجاگر کریں
  شطرنج

ناکام ہونے کے لئے `--expect-payload-digest=<hex>` اور `--expect-payload-len=<bytes>` شامل کریں
جلدی سے جب تعمیر نو پے لوڈ منشور سے ہٹ جاتا ہے۔

## 5. اگلے اقدامات

- ** گورننس انضمام ** - منشور کے نقش کو پہنچائیں اور
  `manifest_signatures.json` ٹپ کے بہاؤ میں تاکہ پن کی رجسٹری کر سکے
  دستیابی کا اعلان کریں۔
- ** مذاکرات کو رجسٹر کریں ** - دیکھیں [`sorafs/chunker_registry.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  نئے پروفائلز کو بچانے سے پہلے۔ آٹومیشن کو ہینڈلز کو ترجیح دینی چاہئے
  عددی IDs کے بجائے کیننیکل (`namespace.name@semver`)۔
- ** سی آئی آٹومیشن ** - ریلیز پائپ لائنوں میں مذکورہ بالا احکامات شامل کریں تاکہ
  دستاویزات ، فکسچر اور نوادرات اختیاری مظہر کو شائع کرتے ہیں
  دستخط شدہ میٹا ڈیٹا کے ساتھ۔