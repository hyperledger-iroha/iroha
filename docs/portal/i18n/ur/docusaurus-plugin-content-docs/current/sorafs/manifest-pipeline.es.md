---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS → منشور پائپ لائن کی chunking

یہ کوئک اسٹارٹ پلگ ان اختتام سے آخر میں پائپ لائن پر چلتا ہے جو بدلتا ہے
Norito میں خام بائٹس SoraFS کے رجسٹری پن کے لئے موزوں ہیں۔ مواد ہے
[`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md) سے موافقت پذیر ؛
کیننیکل تفصیلات اور تبدیلی کے ل that اس دستاویز کو دیکھیں۔

## 1. ٹکڑے کا تعی .ن

SoraFS SF-1 پروفائل (`sorafs.sf1@1.0.0`) استعمال کرتا ہے: A کے ساتھ ایک فاسٹ سی ڈی سی سے متاثرہ رولنگ ہیش
کم سے کم 64 کیب کا کم سے کم حصہ ، 256 KIB کا ہدف ، زیادہ سے زیادہ 512 KIB اور ایک ماسک
`0x0000ffff` کاٹنے۔ پروفائل `sorafs_manifest::chunker_registry` پر رجسٹرڈ ہے۔

### مورچا مددگار

- `sorafs_car::CarBuildPlan::single_file` - جاری کردہ آفسیٹس ، لمبائی اور ہضم جاری کرتے ہیں
  بلیک 3 کار میٹا ڈیٹا تیار کرتے وقت۔
- `sorafs_car::ChunkStore` - اسٹریم پے لوڈز ، برقرار میٹا ڈیٹا کو برقرار رکھیں اور درخت کو اخذ کریں
  پروف-ریٹری کیبلٹی (POR) نمونے لینے کی شرح 64 KIB / 4 KIB۔
- `sorafs_chunker::chunk_bytes_with_digests` - دونوں CLIs کے پیچھے لائبریری مددگار۔

### سی ایل آئی ٹولز

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON میں ٹکڑوں کی آرڈر شدہ آفسیٹس ، لمبائی اور ہضم ہوتا ہے۔ بچت کریں
جب منصوبہ بندی کی جاتی ہے یا آرکیسٹریٹر کی تعمیر کی وضاحت کی گئی ہے تو منصوبہ۔

### پور گواہ

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` اور `--por-sample=<count>` کو بے نقاب کرتا ہے تاکہ ایسا کریں
آڈیٹر عین مطابق گواہوں کے سیٹ کی درخواست کرسکتے ہیں۔ ان جھنڈوں کے ساتھ جوڑیں
`--por-proof-out` یا `--por-sample-out` JSON کو ریکارڈ کرنے کے لئے۔

## 2. ایک مینی فیسٹ کو لپیٹنا

`ManifestBuilder` گورننس منسلکات کے ساتھ حصہ میٹا ڈیٹا کو جوڑتا ہے:

- روٹ سی آئی ڈی (ڈی اے جی سی بی آر) اور کار کے وعدے۔
- عرفی اور سپلائر کی گنجائش کے دعوے کی جانچ کرنا۔
- کونسل کے دستخط اور اختیاری میٹا ڈیٹا (جیسے IDS تعمیر کریں)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

اہم روانگی:

- `payload.manifest` - Norito میں انکوڈ شدہ مینی فیسٹ بائٹس۔
- `payload.report.json` - انسانی/آٹومیشن پڑھنے کے قابل خلاصہ ، شامل ہے
  `chunk_fetch_specs` ، `payload_digest_hex` ، کار ڈائجسٹس اور عرف میٹا ڈیٹا۔
- `payload.manifest_signatures.json` - منشور کے بلیک 3 ڈائجسٹ پر مشتمل لفافہ ،
  آرڈرڈ منڈ پلان اور ED25519 کے دستخطوں کا ہضم SHA3۔

بیرونی دستخط کنندگان کے ذریعہ فراہم کردہ لفافوں کی تصدیق کے لئے `--manifest-signatures-in` استعمال کریں
ان کو دوبارہ لکھنے سے پہلے ، اور `--chunker-profile-id` یا `--chunker-profile=<handle>` کے لئے
ریکارڈ کا انتخاب مرتب کریں۔

## 3. پوسٹ اور پن1. ** گورننس کو بھیجیں ** - مینی فیسٹ اور دستخطی لفافے کو ہضم فراہم کریں
   مشورہ تاکہ پن کو داخل کیا جاسکے۔ بیرونی آڈیٹرز کو لازمی طور پر اسٹور کرنا چاہئے
   SHA3 ​​منشور کے ہضم کے ساتھ ساتھ حصہ کے منصوبے کا ڈائجسٹ۔
2. ** پن پے لوڈز ** - کار فائل (اور اختیاری کار انڈیکس) کو اپ لوڈ کریں
   پن رجسٹری میں ظاہر ہے۔ اس بات کو یقینی بناتا ہے کہ منشور اور کار ایک ہی جڑ سی آئی ڈی کا اشتراک کریں۔
3. ** لاگ ٹیلی میٹری ** - JSON رپورٹ ، پور ٹوکنز اور کسی بھی میٹرکس کو برقرار رکھتا ہے
   ریلیز نمونے میں بازیافت کریں۔ یہ ریکارڈ ڈیش بورڈز کو کھانا کھاتے ہیں
   آپریٹرز اور بڑے پے لوڈ کو ڈاؤن لوڈ کیے بغیر واقعات کو دوبارہ پیش کرنے میں مدد کریں۔

## 4. ملٹی وینڈر بازیافت تخروپن

`کارگو رن -پی sorafs_car -bin sorafs_fetch ---plan = payload.report.json \
  -پرووئڈر = الفا = فراہم کنندہ/الفا.بن-پروویڈر= بی ٹی اے= پروویڈرز/بی ٹی اے ۔بن)
  -آؤٹ پٹ = پے لوڈ.بن-JSON-OUT = fetch_report.json`

- `#<concurrency>` ہر فراہم کنندہ (`#4` اوپر) متوازی اضافہ کرتا ہے۔
- `@<weight>` منصوبہ بندی کے تعصب کو ایڈجسٹ کرتا ہے۔ پہلے سے طے شدہ طور پر یہ 1 ہے۔
- `--max-peers=<n>` جب رن کے لئے شیڈول کی تعداد کو محدود کرتا ہے جب
  دریافت مطلوبہ سے زیادہ امیدوار تیار کرتی ہے۔
- `--expect-payload-digest` اور `--expect-payload-len` خاموش بدعنوانی سے بچاؤ۔
- `--provider-advert=name=advert.to` فراہم کنندہ کی صلاحیتوں کو استعمال کرنے سے پہلے چیک کرتا ہے
  تخروپن میں
- `--retry-budget=<n>` فی حصہ دوبارہ شروع کرنے والی گنتی (پہلے سے طے شدہ: 3) کو اوور رائڈ کرتی ہے
  ناکامی کے منظرناموں کی جانچ کرتے وقت سی آئی تیزی سے دباؤ کو بے نقاب کرسکتا ہے۔

`fetch_report.json` مجموعی میٹرکس (`chunk_retry_total` ، `provider_failure_rate` ،
وغیرہ) سی آئی کے دعووں اور مشاہدے کے ل suitable موزوں ہے۔

## 5. رجسٹری اور گورننس اپڈیٹس

جب نئے چنکر پروفائلز کی تجویز پیش کرتے ہیں:

1. `sorafs_manifest::chunker_registry_data` میں ڈسکرپٹر لکھیں۔
2. `docs/source/sorafs/chunker_registry.md` اور متعلقہ چارٹر کو اپ ڈیٹ کریں۔
3. دوبارہ تخلیق فکسچر (`export_vectors`) اور گرفتاری کے منشور پر قبضہ کریں۔
4. گورننس کے دستخطوں کے ساتھ چارٹر تعمیل رپورٹ جمع کروائیں۔

آٹومیشن کو کیننیکل ہینڈلز (`namespace.name@semver`) کو ترجیح دینی چاہئے اور IDs کا سہارا لینا چاہئے
عددی صرف جب رجسٹری کے لئے ضروری ہو۔