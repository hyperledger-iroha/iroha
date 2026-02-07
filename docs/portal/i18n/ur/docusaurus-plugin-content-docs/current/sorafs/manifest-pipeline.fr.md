---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# chunking SoraFS → منشور پائپ لائن

یہ کوئک اسٹارٹ کے اختتام سے آخر تک پائپ لائن کا پتہ لگاتا ہے جو خام بائٹس کو تبدیل کرتا ہے
Norito میں SoraFS کی پن رجسٹری میں موافقت پذیر ہے۔ مواد سے ڈھال لیا گیا ہے
[`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md) ؛
اس دستاویز کو کیننیکل تفصیلات اور تبدیلی کے ل. دیکھیں۔

## 1. چنکر عزم کے ساتھ

SoraFS SF-1 پروفائل (`sorafs.sf1@1.0.0`) کا استعمال کرتا ہے: فاسٹ سی ڈی سی سے متاثر ایک رولنگ ہیش
کم از کم 64 KIB ، 256 KIB کا ہدف ، زیادہ سے زیادہ 512 KIB اور ایک ماسک
ٹوٹنا `0x0000ffff`۔ پروفائل `sorafs_manifest::chunker_registry` میں محفوظ ہے۔

### مددگار زنگ

- `sorafs_car::CarBuildPlan::single_file` - بلیک 3 آفسیٹس ، لمبائی اور پیروں کے نشانات خارج کرتا ہے
  کار میٹا ڈیٹا کی تیاری کے دوران ٹکڑے۔
- `sorafs_car::ChunkStore` - اسٹریم پے لوڈز ، برقرار رہنے والے میٹا ڈیٹا اور اخذ کریں
  64 KIB / 4 KIB پروف-ret-retrivebility (POR) نمونے لینے کا درخت۔
- `sorafs_chunker::chunk_bytes_with_digests` - دونوں CLIs کے پیچھے لائبریری مددگار۔

### سی ایل آئی ٹولز

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON میں ٹکڑوں کی آرڈر شدہ آفسیٹ ، لمبائی اور فنگر پرنٹس شامل ہیں۔ اسے رکھو
جب آرکسٹریٹر کے لئے ظاہر ہوتا ہے یا وضاحتیں لاتے ہو تو منصوبہ بنائیں۔

### پور کوکیز

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` اور `--por-sample=<count>` کو بے نقاب کرتا ہے تاکہ
سامعین کوکیز کے سیٹوں کے سیٹ کی درخواست کرسکتے ہیں۔ ان جھنڈوں کو اس کے ساتھ منسلک کریں
`--por-proof-out` یا `--por-sample-out` JSON کو بچانے کے لئے۔

## 2۔ ایک منشور لپیٹیں

`ManifestBuilder` گورننس کے ٹکڑوں کے ساتھ حصہ میٹا ڈیٹا کو جوڑتا ہے:

- روٹ سی آئی ڈی (ڈی اے جی سی بی آر) اور کار کے وعدے۔
- عرف ثبوت اور فروش کی اہلیت کے بیانات۔
- بورڈ کے دستخط اور اختیاری میٹا ڈیٹا (جیسے ، آئی ڈی بلڈ آئی ڈی)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

اہم ریلیز:

- `payload.manifest` - ظاہر کردہ بائٹس کو Norito کے طور پر انکوڈ کیا گیا۔
- `payload.report.json` - انسانی/آٹومیشن پڑھنے کے قابل خلاصہ ، بشمول
  `chunk_fetch_specs` ، `payload_digest_hex` ، کار فنگر پرنٹ اور عرف میٹا ڈیٹا۔
- `payload.manifest_signatures.json` - منشور کے بلیک 3 فنگر پرنٹ پر مشتمل لفافہ ،
  حصہ پلان کا SHA3 فنگر پرنٹ اور ترتیب شدہ ED25519 دستخطوں۔

دستخط کنندگان کے ذریعہ فراہم کردہ لفافوں کی تصدیق کے لئے `--manifest-signatures-in` استعمال کریں
ان کو دوبارہ لکھنے سے پہلے بیرونی ، اور `--chunker-profile-id` یا `--chunker-profile=<handle>` to
لاک رجسٹر سلیکشن۔

## 3. شائع اور پن1. ** گورننس میں جمع کرانا ** - ظاہر امپرنٹ اور لفافہ فراہم کریں
   کونسل پر دستخطوں کو تاکہ پائن کو داخل کیا جاسکے۔ بیرونی آڈیٹرز کو لازمی ہے
   منشور فنگر پرنٹ کے ساتھ حصہ منصوبے کے SHA3 فنگر پرنٹ رکھیں۔
2. ** پنر پے لوڈز ** - کار آرکائیو (اور اختیاری کار انڈیکس) کو لوڈ کریں
   اسے پن رجسٹری میں ظاہر کریں۔ اس بات کو یقینی بنائیں کہ منشور اور کار کا اشتراک کریں
   ایک ہی جڑ سیڈ۔
3.
   ریلیز نمونے میں بازیافت کی۔ یہ ریکارڈنگ ڈیش بورڈز کو کھانا کھلاتی ہیں
   آپریٹرز اور بڑے پے لوڈ کو ڈاؤن لوڈ کیے بغیر واقعات کو دوبارہ پیش کرنے میں مدد کریں۔

## 4. ملٹی فراہم کرنے والا بازیافت تخروپن

`کارگو رن -پی sorafs_car -bin sorafs_fetch ---plan = payload.report.json \
  -پرووئڈر = الفا = فراہم کنندہ/الفا.بن-پروویڈر= بی ٹی اے= پروویڈرز/بی ٹی اے ۔بن)
  -آؤٹ پٹ = پے لوڈ.بن-JSON-OUT = fetch_report.json`

- `#<concurrency>` ہر فراہم کنندہ (`#4` اوپر) متوازی اضافہ کرتا ہے۔
- `@<weight>` شیڈولنگ تعصب کو ایڈجسٹ کرتا ہے۔ پہلے سے طے شدہ 1 ہے۔
- `--max-peers=<n>` جب عملدرآمد کے لئے شیڈول فراہم کرنے والوں کی تعداد کو محدود کرتا ہے جب
  دریافت مطلوبہ سے زیادہ امیدواروں کو لوٹاتا ہے۔
- `--expect-payload-digest` اور `--expect-payload-len` خاموش بدعنوانی سے بچاؤ۔
- `--provider-advert=name=advert.to` فراہم کنندہ کی صلاحیتوں کو استعمال کرنے سے پہلے چیک کرتا ہے
  تخروپن میں
- `--retry-budget=<n>` فی حصہ (پہلے سے طے شدہ: 3) کی کوششوں کی تعداد کو تبدیل کرتا ہے تاکہ
  سی آئی نے بندش کے منظرناموں کے دوران رجعت پسندی کو زیادہ تیزی سے ظاہر کیا۔

`fetch_report.json` نے مجموعی میٹرکس (`chunk_retry_total` ، `provider_failure_rate` ، کو بے نقاب کیا ،
وغیرہ) سی آئی کے دعووں اور مشاہدے کے ل suitable موزوں ہے۔

## 5. رجسٹری کی تازہ کاری اور گورننس

جب نئے چنکر پروفائلز کی تجویز پیش کرتے ہیں:

1. `sorafs_manifest::chunker_registry_data` میں ڈسکرپٹر لکھیں۔
2. `docs/source/sorafs/chunker_registry.md` اور اس سے وابستہ چارٹ کو اپ ڈیٹ کریں۔
3. دوبارہ تخلیق فکسچر (`export_vectors`) اور گرفتاری کے منشور پر قبضہ کریں۔
4. گورننس کے دستخطوں کے ساتھ چارٹر تعمیل رپورٹ جمع کروائیں۔

آٹومیشن کو کیننیکل ہینڈلز (`namespace.name@semver`) کے حق میں ہونا چاہئے اور نہیں ہونا چاہئے
جب رجسٹری کے لئے ضروری ہو تو عددی آئی ڈی میں واپس جائیں۔