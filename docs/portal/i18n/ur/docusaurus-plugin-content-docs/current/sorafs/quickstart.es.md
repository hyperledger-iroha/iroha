---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#SoraFS فوری آغاز

یہ عملی گائیڈ چنکر SF-1 کے تعصب پسند پروفائل کے ذریعے چلتا ہے ،
منشور پر دستخط اور ملٹی سپلائیئر ریکوری فلو جو اس کی حمایت کرتے ہیں
SoraFS اسٹوریج پائپ لائن۔ اس کے ساتھ مکمل کریں
[منشور پائپ لائن گہری غوطہ خور] (manifest-pipeline.md)
ڈیزائن نوٹ اور سی ایل آئی پرچم حوالہ کے لئے۔

## شرائط

- مورچا ٹولچین (`rustup update`) ، مقامی طور پر کلونڈ ورک اسپیس۔
- اختیاری: [ED25519 کلیدی جوڑی اوپن ایس ایل کے ساتھ تیار کی گئی] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  منشور پر دستخط کرنے کے لئے۔
- اختیاری: اگر آپ Docusaurus پورٹل کا پیش نظارہ کرنے کا ارادہ رکھتے ہیں تو نوڈ ڈاٹ جے ایس ≥ 18۔

مفید CLI پیغامات کو ظاہر کرنے کے لئے تجربہ کرتے ہوئے `export RUST_LOG=info` کی وضاحت کریں۔

## 1. تازہ ترین فکسچر کو اپ ڈیٹ کریں

کیننیکل SF-1 chunking ویکٹروں کو دوبارہ تخلیق کرتا ہے۔ کمانڈ بھی جاری کرتا ہے
جب `--signing-key` فراہم کیا جاتا ہے تو دستخط شدہ منشور لفافے ؛ استعمال کریں
`--allow-unsigned` صرف مقامی ترقی کے دوران۔

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

روانگی:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (اگر دستخط شدہ)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2۔ ایک پے لوڈ کو ٹکڑے ٹکڑے کریں اور منصوبے کا معائنہ کریں

فائل یا صوابدیدی آرکائو کو ٹکڑے کرنے کے لئے `sorafs_chunker` استعمال کریں:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

کلیدی فیلڈز:

- `profile` / `break_mask` - `sorafs.sf1@1.0.0` کے پیرامیٹرز کی تصدیق کرتا ہے۔
- `chunks[]` - حصوں کی آفسیٹس ، لمبائی اور بلیک 3 ڈائجسٹس۔

بڑے فکسچر کے لئے ، پریپٹیسٹ بیک بیک ریگریشن کو چلائیں
اس بات کو یقینی بنائیں کہ ہم آہنگی میں اسٹریمنگ اور بیچ چنکنگ قیام کریں:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. ایک منشور کی تعمیر اور دستخط کریں

منشور استعمال کرتے ہوئے منشور میں حصہ ، عرفی اور گورننس کے دستخطوں کو لپیٹیں
`sorafs-manifest-stub`۔ ذیل میں کمانڈ ایک فائل پے لوڈ کو دکھاتا ہے۔ پاس
ایک درخت کو پیکیج کرنے کا ایک ڈائرکٹری راستہ (سی ایل آئی اسے لغت کے مطابق ترتیب دیتا ہے)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` کے لئے چیک کریں:

- `chunking.chunk_digest_sha3_256` - SHA3 آفسیٹس/لمبائی کا ڈائجسٹ ، میچ کرتا ہے
  چنکر فکسچر۔
- `manifest.manifest_blake3` - مینی فیسٹ لفافے پر ڈائجسٹ بلیک 3 پر دستخط ہوئے۔
- `chunk_fetch_specs[]` - آرکیسٹریٹرز کے لئے بحالی کی ہدایات کا آرڈر دیا گیا۔

جب آپ حقیقی دستخط فراہم کرنے کے لئے تیار ہوں تو ، دلائل `--signing-key` اور شامل کریں
`--signer`۔ کمانڈ لفافے کو لکھنے سے پہلے ہر ED25519 دستخط کی تصدیق کرتا ہے۔

## 4. ملٹی وینڈر کی بازیابی کا نقالی کریں

ایک یا زیادہ سے زیادہ کے خلاف حصہ پلان کو دوبارہ چلانے کے لئے ڈویلپمنٹ بازیافت سی ایل آئی کا استعمال کریں
سپلائرز۔ یہ CI دھواں ٹیسٹ اور آرکسٹریٹر پروٹو ٹائپ کے لئے مثالی ہے۔

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

چیک:- `payload_digest_hex` کو مینی فیسٹ رپورٹ سے ملنا چاہئے۔
- `provider_reports[]` فراہم کنندہ کے ذریعہ کامیابی/ناکامی کی گنتی کو ظاہر کرتا ہے۔
-ایک غیر صفر `chunk_retry_total` بیک پریشر کی ترتیبات کو نمایاں کرتا ہے۔
- رن کے لئے شیڈول فراہم کرنے والوں کی تعداد کو محدود کرنے کے لئے `--max-peers=<n>` پاس کریں
  اور سی آئی کے نقالی کو اعلی امیدواروں پر مرکوز رکھیں۔
- `--retry-budget=<n>` فی حصہ (3) کی بحالی کی پہلے سے طے شدہ گنتی کے لئے اوور رائڈس
  غلطیوں کو انجیکشن لگاتے وقت آرکسٹریٹر ریگریشنز کا تیزی سے پتہ لگائیں۔

تیز رفتار ناکام ہونے کے لئے `--expect-payload-digest=<hex>` اور `--expect-payload-len=<bytes>` شامل کریں
جب تعمیر نو کارگو منشور سے انحراف کرتا ہے۔

## 5. اگلے اقدامات

- ** گورننس انضمام ** - چینلز منشور ڈائجسٹ اور
  `manifest_signatures.json` مشورے کے بہاؤ میں تاکہ رجسٹری پن کی جاسکے
  دستیابی کا اعلان کریں۔
- ** رجسٹری مذاکرات ** - استفسار [`sorafs/chunker_registry.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  نئے پروفائلز رجسٹر کرنے سے پہلے۔ آٹومیشن کو کیننیکل ہینڈلرز کو ترجیح دینی چاہئے
  (`namespace.name@semver`) عددی IDs پر۔
- ** سی آئی آٹومیشن ** - ریلیز پائپ لائنوں میں مذکورہ بالا احکامات شامل کرتا ہے تاکہ
  دستاویزات ، فکسچر اور نمونے اس کے ساتھ ساتھ اختیاری ظاہر بھی شائع کرتے ہیں
  دستخط شدہ میٹا ڈیٹا۔