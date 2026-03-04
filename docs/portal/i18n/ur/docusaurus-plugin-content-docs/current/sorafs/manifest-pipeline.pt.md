---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS → منشور پائپ لائن کی chunking

یہ کوئک اسٹارٹ ایڈ آن آن اختتام تک پائپ لائن کو ٹریک کرتا ہے جو بائٹس کو تبدیل کرتا ہے
Norito میں خام فائلیں SoraFS کی پن رجسٹری کے لئے موزوں ہیں۔ مواد سے ڈھال لیا گیا تھا
[`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md) ؛
کیننیکل تفصیلات اور تبدیلی کے ل that اس دستاویز کو دیکھیں۔

## 1. چنکنگ کا تعی .ن

SoraFS SF-1 پروفائل (`sorafs.sf1@1.0.0`) استعمال کرتا ہے: ایک فاسٹ سی ڈی سی سے متاثرہ رولنگ ہیش کے ساتھ
کم سے کم حصہ سائز 64 کیب ، ہدف 256 KIB ، زیادہ سے زیادہ 512 KIB اور لپیٹ ماسک
`0x0000ffff`۔ پروفائل `sorafs_manifest::chunker_registry` میں رجسٹرڈ ہے۔

### زنگ آلود میں مددگار

- `sorafs_car::CarBuildPlan::single_file` - حصہ آفسیٹس ، لمبائی اور ہضموں کو خارج کرتا ہے
  بلیک 3 کار میٹا ڈیٹا تیار کرتے وقت۔
- `sorafs_car::ChunkStore` - اسٹریمز پے لوڈ ، برقرار ہے میٹا ڈیٹا اور بڑھے ہوئے
  64 KIB / 4 KIB پروف-ret-retrivebility (POR) نمونے لینے کا درخت۔
- `sorafs_chunker::chunk_bytes_with_digests` - دونوں CLIs کے پیچھے لائبریری مددگار۔

### سی ایل آئی ٹولز

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON میں آرڈر شدہ آفسیٹ ، لمبائی اور حصہ ڈائجسٹز شامل ہیں۔ منصوبے کو محفوظ رکھیں
آرکیسٹریٹر کو ظاہر کرتا ہے یا وضاحتیں بازیافت کریں۔

### پور گواہ

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` اور `--por-sample=<count>` کو بے نقاب کرتا ہے تاکہ ایسا کریں
آڈیٹر گواہوں کے عین مطابق سیٹوں کی درخواست کرسکتے ہیں۔ ان جھنڈوں کے ساتھ جوڑیں
`--por-proof-out` یا `--por-sample-out` JSON کو ریکارڈ کرنے کے لئے۔

## 2. پیکیج ایک منشور

`ManifestBuilder` گورننس منسلکات کے ساتھ حصہ میٹا ڈیٹا کو جوڑتا ہے:

- روٹ سی آئی ڈی (ڈی اے جی سی بی آر) اور کار کے وعدے۔
- عرف کے ثبوت اور فراہم کنندہ کی صلاحیت کے دعوے۔
- بورڈ کے دستخط اور اختیاری میٹا ڈیٹا (جیسے IDS تعمیر کریں)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

اہم نتائج:

- `payload.manifest` - Norito میں انکوڈ شدہ مینی فیسٹ بائٹس۔
- `payload.report.json` - انسانی/آٹومیشن پڑھنے کے قابل خلاصہ ، بشمول `chunk_fetch_specs` ،
  `payload_digest_hex` ، کار ڈائجسٹس اور عرف میٹا ڈیٹا۔
- `payload.manifest_signatures.json` - منشور کے بلیک 3 ڈائجسٹ پر مشتمل لفافہ ،
  SHA3 ​​ڈائجسٹ آف کنٹ پلان اور ED25519 کے دستخطوں کا حکم دیا گیا۔

بیرونی دستخط کنندگان کے ذریعہ فراہم کردہ لفافوں کی تصدیق کے لئے `--manifest-signatures-in` استعمال کریں
ان کو دوبارہ ریکارڈ کرنے سے پہلے اور `--chunker-profile-id` یا `--chunker-profile=<handle>` to
ریکارڈ کا انتخاب درست کریں۔

## 3. شائع اور پن

1.
   مشورہ تاکہ پن کو داخل کیا جاسکے۔ بیرونی آڈیٹرز کو لازمی طور پر SHA3 ڈائجسٹ رکھنا چاہئے
   منشور ڈائجسٹ کے ساتھ ساتھ حصہ کا منصوبہ۔
2. ** پنیر پے لوڈز ** - کار فائل (اور اختیاری کار انڈیکس) کو اپ لوڈ کریں
   پن رجسٹری کے لئے منشور۔ اس بات کو یقینی بنائیں کہ ظاہر اور کار ایک ہی جڑ سی آئی ڈی کا اشتراک کریں۔
3. ** ریکارڈ ٹیلی میٹری ** - JSON رپورٹ ، پور گواہوں اور کسی کو بھی محفوظ رکھیں
   ریلیز نمونے پر میٹرکس کی بازیافت کریں۔ یہ ریکارڈ ڈیش بورڈز کو فیڈ کرتے ہیں
   آپریٹرز اور بڑے پے لوڈ کو ڈاؤن لوڈ کیے بغیر مسائل کو دوبارہ پیش کرنے میں مدد کریں۔## 4. ملٹی فراہم کرنے والا بازیافت تخروپن

`کارگو رن -پی sorafs_car -bin sorafs_fetch ---plan = payload.report.json \
  -پرووئڈر = الفا = فراہم کنندہ/الفا.بن-پروویڈر= بی ٹی اے= پروویڈرز/بی ٹی اے ۔بن)
  -آؤٹ پٹ = پے لوڈ.بن-JSON-OUT = fetch_report.json`

- `#<concurrency>` فی فراہم کنندہ متوازی (`#4` اوپر) میں اضافہ کرتا ہے۔
- `@<weight>` شیڈولنگ تعصب کو ایڈجسٹ کرتا ہے۔ پہلے سے طے شدہ 1 ہے۔
- `--max-peers=<n>` جب رن کے لئے شیڈول فراہم کرنے والوں کی تعداد کو محدود کرتا ہے تو
  دریافت مطلوبہ سے زیادہ امیدواروں کو لوٹاتا ہے۔
- `--expect-payload-digest` اور `--expect-payload-len` خاموش بدعنوانی سے بچاؤ۔
- `--provider-advert=name=advert.to` اس میں استعمال کرنے سے پہلے فراہم کنندہ کی صلاحیتوں کی جانچ پڑتال کرتا ہے
  تخروپن
- `--retry-budget=<n>` فی حصہ دوبارہ کوشش کرنے والی گنتی (پہلے سے طے شدہ: 3) کو اوور رائڈ کرتی ہے تاکہ CI
  ناکامی کے منظرناموں کی جانچ کرتے وقت دباؤ کو تیزی سے بے نقاب کریں۔

`fetch_report.json` نے مجموعی میٹرکس (`chunk_retry_total` ، `provider_failure_rate` ، کو بے نقاب کیا ،
وغیرہ) سی آئی اور مشاہدہ کرنے کے دعووں کے لئے موزوں ہے۔

## 5. رجسٹری اور گورننس اپڈیٹس

جب نئے چنکر پروفائلز کی تجویز پیش کرتے ہیں:

1. `sorafs_manifest::chunker_registry_data` میں ڈسکرپٹر لکھیں۔
2. `docs/source/sorafs/chunker_registry.md` اور متعلقہ چارٹر کو اپ ڈیٹ کریں۔
3. دوبارہ تخلیق فکسچر (`export_vectors`) اور گرفتاری کے منشور پر قبضہ کریں۔
4. گورننس کے دستخطوں کے ساتھ چارٹر تعمیل رپورٹ جمع کروائیں۔

آٹومیشن کو کیننیکل ہینڈلز (`namespace.name@semver`) کو ترجیح دینی چاہئے اور IDs کا سہارا لینا چاہئے
عددی صرف اس وقت جب رجسٹری کے ذریعہ ضرورت ہو۔