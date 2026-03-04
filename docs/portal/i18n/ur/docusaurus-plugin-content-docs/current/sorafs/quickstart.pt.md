---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS فوری آغاز

یہ عملی گائیڈ SF-1 ڈٹرمینسٹک چنکر پروفائل کے ذریعے چلتا ہے ،
منشور پر دستخط اور ملٹی فراہم کرنے والے تلاش کا بہاؤ جو اس کی مدد کرتا ہے
SoraFS اسٹوریج پائپ لائن۔ اس کے ساتھ جوڑیں
[واضح پائپ لائن میں گہری غوطہ خوری] (manifest-pipeline.md)
ڈیزائن نوٹ اور سی ایل آئی جھنڈوں کے حوالہ کے ل .۔

## شرائط

- مورچا ٹولچین (`rustup update`) ، ورک اسپیس مقامی طور پر کلون کیا گیا۔
- اختیاری: [اوپن ایس ایس ایل مطابقت پذیر ED25519 کلیدی جوڑی] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  منشور پر دستخط کرنے کے لئے۔
- اختیاری: نوڈ. جے ایس ≥ 18 اگر آپ Docusaurus پورٹل کا پیش نظارہ کرنا چاہتے ہیں۔

مفید CLI پیغامات کو بے نقاب کرنے کے لئے جانچ کے دوران `export RUST_LOG=info` سیٹ کریں۔

## 1. تازہ ترین فکسچر کو اپ ڈیٹ کریں

کیننیکل SF-1 chunking ویکٹرز کو دوبارہ تخلیق کریں۔ کمانڈ بھی جاری کرتا ہے
جب `--signing-key` فراہم کیا جاتا ہے تو دستخط شدہ منشور لفافے ؛ استعمال کریں
صرف مقامی ترقی میں `--allow-unsigned`۔

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

نتائج:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (اگر دستخط شدہ)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2۔ ایک پے لوڈ کو ٹکڑے ٹکڑے کریں اور منصوبے کا معائنہ کریں

کسی صوابدیدی فائل یا کمپریسڈ فائل کو توڑنے کے لئے `sorafs_chunker` استعمال کریں:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

کلیدی فیلڈز:

- `profile` / `break_mask` - `sorafs.sf1@1.0.0` کے پیرامیٹرز کی تصدیق کرتا ہے۔
- `chunks[]` - آرڈرڈ آفسیٹس ، لمبائی اور بلیک 3 ہضم۔

بڑے فکسچر کے ل ، ، کو یقینی بنانے کے لئے پروپیسٹ کے ساتھ رجعت چلائیں
اسٹریمنگ اور بیچ چنکنگ ہم آہنگی میں رہتا ہے:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. ایک منشور کی تعمیر اور دستخط کریں

ایک منشور میں حصہ ، عرفیت ، اور گورننس کے دستخطوں کو پیکج کریں
`sorafs-manifest-stub` کا استعمال کرتے ہوئے۔ نیچے دیئے گئے کمانڈ میں ایک فائل پے لوڈ کو ظاہر کیا گیا ہے۔ پاس
ایک درخت کو پیکیج کرنے کا ایک ڈائرکٹری راستہ (CLI سائیکلوں کے ذریعے لغت کے مطابق)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` کا جائزہ لیں:

- `chunking.chunk_digest_sha3_256` - SHA3 آفسیٹس/لمبائی کا ڈائجسٹ ، کے مطابق ہے
  چنکر فکسچر۔
- `manifest.manifest_blake3` - مینی فیسٹ لفافے پر ڈائجسٹ بلیک 3 پر دستخط ہوئے۔
- `chunk_fetch_specs[]` - آرکیسٹریٹرز کے لئے بازیافت ہدایات کا آرڈر دیا گیا۔

جب آپ اصل دستخط فراہم کرنے کے لئے تیار ہوں تو ، دلائل شامل کریں
`--signing-key` اور `--signer`۔ کمانڈ لکھنے سے پہلے ہر ED25519 دستخط کی جانچ پڑتال کرتا ہے
لفافہ

## 4. ملٹی فراہم کرنے والے بازیافت کی تقلید کریں

ایک کے خلاف حصہ کے منصوبے کو دوبارہ پیش کرنے کے لئے ترقی کی بازیافت سی ایل آئی کا استعمال کریں
مزید فراہم کرنے والے۔ یہ CI دھواں ٹیسٹنگ اور آرکسٹریٹر پروٹو ٹائپنگ کے لئے مثالی ہے۔

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

چیک:- `payload_digest_hex` کو مینی فیسٹ رپورٹ سے ملنا چاہئے۔
- `provider_reports[]` فراہم کنندہ کے ذریعہ کامیابی/ناکامی کی گنتی کو ظاہر کرتا ہے۔
-نان صفر `chunk_retry_total` بیک پریشر ایڈجسٹمنٹ کو نمایاں کرتا ہے۔
- رن کے لئے شیڈول فراہم کرنے والوں کی تعداد کو محدود کرنے کے لئے `--max-peers=<n>` پاس کریں
  اور سی آئی کے نقالی کو اعلی امیدواروں پر مرکوز رکھیں۔
- `--retry-budget=<n>` کو بے نقاب کرنے کے لئے فی حصہ (3) کی کوششوں کی پہلے سے طے شدہ گنتی
  جب غلطیوں کو انجیکشن لگاتے ہو تو تیز تر آرکیسٹریٹر رجعتیں۔

ناکام ہونے کے لئے `--expect-payload-digest=<hex>` اور `--expect-payload-len=<bytes>` شامل کریں
جلدی سے جب تعمیر نو پے لوڈ منشور سے ہٹ جاتا ہے۔

## 5. اگلے اقدامات

- ** گورننس انضمام ** - مینی فیسٹ ڈائجسٹ اور `manifest_signatures.json` بھیجیں
  بورڈ کے سلسلے میں تاکہ پن رجسٹری دستیابی کا اعلان کرسکے۔
- ** رجسٹریشن مذاکرات ** - دیکھیں [`sorafs/chunker_registry.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  نئے پروفائلز رجسٹر کرنے سے پہلے۔ آٹومیشن کو کیننیکل شناخت کاروں کو ترجیح دینی چاہئے
  (`namespace.name@semver`) عددی IDs کے بجائے۔
- ** سی آئی آٹومیشن ** - پائپ لائنوں کو جاری کرنے کے لئے مذکورہ بالا احکامات شامل کریں تاکہ
  دستاویزات ، فکسچر اور نمونے اس کے ساتھ ساتھ اختیاری ظاہر بھی شائع کرتے ہیں
  دستخط شدہ میٹا ڈیٹا۔