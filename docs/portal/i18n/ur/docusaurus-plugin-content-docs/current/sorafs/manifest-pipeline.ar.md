---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ہیش SoraFS → ظاہر راستہ

یہ کوئیک اسٹارٹ گائیڈ منسلک ایک اختتام سے آخر میں پائپ لائن کی نمائندگی کرتا ہے جو خام بائٹس کو تبدیل کرتا ہے
Norito میں SoraFS میں پن رجسٹری کے لئے موزوں ہے۔ مواد سے موافقت پذیر
[`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md) ؛
اس دستاویز کو منظور شدہ تفصیلات اور تبدیلیوں کے لاگ کے لئے دیکھیں۔

## 1. تعصب پسند ہیشنگ

SoraFS ایک SF-1 پروفائل (`sorafs.sf1@1.0.0`) استعمال کرتا ہے: ایک فاسٹ سی ڈی سی سے متاثرہ رولنگ ہیش جس کا نچلا حصہ ہے
64 KIB کے ایک حصے کے لئے ، 256 KIB کا ایک ہدف ، زیادہ سے زیادہ 512 KIB ، اور `0x0000ffff` کا بریک ماسک۔ فائل
`sorafs_manifest::chunker_registry` پر رجسٹرڈ۔

### مورچا مددگار

- `sorafs_car::CarBuildPlan::single_file` - اس کے دوران ٹکڑوں کی آفسیٹ ، لمبائی اور بلیک 3 سمری تیار کرتا ہے
  کار میٹا ڈیٹا تیار کرنا۔
- `sorafs_car::ChunkStore` - پے لوڈز ، ٹکڑوں کو میٹا ڈیٹا ، اور اخذ کرتا ہے
  سائز 64 KIB / 4 KIB کے ساتھ نمونے لینے والے درخت کی مناسبت (POR)۔
- `sorafs_chunker::chunk_bytes_with_digests` - دونوں CLIs کے پیچھے ایک لائبریری اسسٹنٹ۔

### سی ایل آئی ٹولز

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON میں آرڈر شدہ آفسیٹ ، لمبائی اور ٹکڑوں کے خلاصے شامل ہیں۔ منشور کی تعمیر کرتے وقت منصوبہ رکھیں
یا آرکسٹریٹر کی بازیافت کی وضاحتیں۔

### پور ثبوت

`ChunkStore` اختیارات `--por-proof=<chunk>:<segment>:<leaf>` اور `--por-sample=<count>` تک قابل بناتا ہے
آڈیٹر ڈٹرمینسٹک شواہد کے سیٹ کی درخواست کرسکتے ہیں۔ ان جھنڈوں کو `--por-proof-out` یا کے ساتھ منسلک کریں
`--por-sample-out` JSON کو رجسٹر کرنے کے لئے۔

## 2. ظاہر پیکیجنگ

`ManifestBuilder` گورننس منسلکات کے ساتھ ٹکڑوں کو میٹا ڈیٹا جمع کرتا ہے:

- سی آئی ڈی روٹ (ڈی اے جی سی بی آر) اور کار وعدے۔
- عرف کے ثبوت اور فراہم کنندہ کی صلاحیتوں کے دعوے۔
- بورڈ کے دستخط اور اختیاری میٹا ڈیٹا (جیسے بلڈ آئی ڈی)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

اہم نتائج:

- `payload.manifest` - Norito کے ساتھ انکوڈ شدہ مینی فیسٹ بائٹس۔
- `payload.report.json` - انسانی/آٹومیشن پڑھنے کے قابل خلاصہ بشمول `chunk_fetch_specs` اور
  `payload_digest_hex` ، کار کے خلاصے اور عرف میٹا ڈیٹا۔
- `payload.manifest_signatures.json` - ایک لفافہ جس میں مینی فیسٹ کا ایک بلیک 3 سمری ہے ، اور اس منصوبے کا ایک SHA3 خلاصہ ہے
  ٹکڑوں ، دستخطوں ED25519 کو ترتیب دیا گیا۔

واپس آنے سے پہلے بیرونی دستخط کرنے والوں سے آنے والے لفافوں کی تصدیق کے لئے `--manifest-signatures-in` استعمال کریں
اسے لکھیں ، اور رجسٹری کا انتخاب انسٹال کرنے کے لئے `--chunker-profile-id` یا `--chunker-profile=<handle>` استعمال کریں۔

## 3. تعیناتی اور تنصیب (پن)

1. ** گورننس سبمیشن ** - بورڈ میں ظاہر ہونے والا خلاصہ اور دستخطی لفافہ جمع کروائیں تاکہ پن کو قبول کیا جاسکے۔
   بیرونی آڈیٹرز کو مینی فیسٹ سمری کے ساتھ ساتھ حصوں کے منصوبے کے SHA3 سمری کو محفوظ کرنا ہوگا۔
2.
   پن رجسٹری۔ اس بات کو یقینی بنائیں کہ منشور اور کار ایک جڑ سی آئی ڈی کا اشتراک کریں۔
3.
   یہ لاگز آپریٹرز کے ڈیش بورڈز کو فیڈ کرتے ہیں اور بغیر کسی ڈاؤن لوڈ کے مسائل کو دوبارہ پیش کرنے میں مدد کرتے ہیں
   بڑے پے لوڈ۔

## 4. ملٹی فراہم کرنے والا بازیافت تخروپن

`کارگو رن -پی sorafs_car -bin sorafs_fetch ---plan = payload.report.json \
  -پرووئڈر = الفا = فراہم کنندہ/الفا.بن-پروویڈر= بی ٹی اے= پروویڈرز/بی ٹی اے ۔بن)
  -آؤٹ پٹ = پے لوڈ.بن-JSON-OUT = fetch_report.json`- `#<concurrency>` ہر فراہم کنندہ (`#4` اوپر) متوازی اضافہ کرتا ہے۔
- `@<weight>` شیڈولنگ تعصب کو ایڈجسٹ کرتا ہے۔ پہلے سے طے شدہ قیمت 1 ہے۔
-`--max-peers=<n>` فراہم کرنے والوں کی تعداد کو محدود کرتا ہے جب دریافت کرنے کے لئے طے شدہ ہے جب دریافت ضرورت سے زیادہ امیدواروں کو لوٹاتا ہے۔
- `--expect-payload-digest` اور `--expect-payload-len` خاموش بدعنوانی سے بچاؤ۔
- `--provider-advert=name=advert.to` تخروپن میں استعمال کرنے سے پہلے فراہم کنندہ کی صلاحیتوں کی جانچ پڑتال کرتا ہے۔
- `--retry-budget=<n>` فی حصہ (پہلے سے طے شدہ: 3) کی کوششوں کی تعداد کو تبدیل کرتا ہے تاکہ CI کا پتہ لگ سکے
  ناکامی کے منظرناموں کی جانچ کرتے وقت رول بیکس تیز تر ہوتے ہیں۔

`fetch_report.json` گروپڈ میٹرکس (`chunk_retry_total` ، `provider_failure_rate` ، وغیرہ) دکھاتا ہے۔
CI اور مشاہدہ کرنے کے دعووں کے لئے موزوں ہے۔

## 5. رجسٹری اور گورننس اپڈیٹس

جب نئے چنکر پروفائلز کی تجویز کرتے ہیں:

1. `sorafs_manifest::chunker_registry_data` میں تفصیل بنائیں۔
2. `docs/source/sorafs/chunker_registry.md` اور متعلقہ کنونشنز کو اپ ڈیٹ کریں۔
3. فکسچر (`export_vectors`) کو دوبارہ تخلیق کریں اور دستخط شدہ منشور منتخب کریں۔
4. گورننس کے دستخطوں کے ساتھ چارٹر کی تعمیل کی رپورٹ پیش کریں۔

آٹومیشن کو معیاری ہینڈلز (`namespace.name@semver`) کو ترجیح دینی چاہئے اور عددی IDs میں واپس نہیں جانا چاہئے
سوائے اس کے کہ جب پسماندہ مطابقت کی ضرورت ہو۔