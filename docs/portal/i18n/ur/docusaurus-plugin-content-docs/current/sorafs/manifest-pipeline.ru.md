---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# chunking SoraFS → پائپ لائن منشور

یہ مواد کوئک اسٹارٹ کی تکمیل کرتا ہے اور ایک مکمل پائپ لائن کی وضاحت کرتا ہے جو خام بن جاتا ہے
Norito پر بائٹس پن رجسٹری SoraFS کے لئے موزوں ہیں۔ متن سے موافقت پذیر
[`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md) ؛
کیننیکل تفصیلات اور چینجلوگ کے ل this اس دستاویز کا حوالہ دیں۔

## 1. تعصب کا chunking

SoraFS SF-1 پروفائل (`sorafs.sf1@1.0.0`) استعمال کرتا ہے: ایک فاسٹ سی ڈی سی کے ساتھ رولنگ ہیش کی حوصلہ افزائی
کم سے کم حصہ سائز 64 کیب ، ہدف 256 KIB ، زیادہ سے زیادہ 512 KIB اور بریک ماسک
`0x0000ffff`۔ پروفائل `sorafs_manifest::chunker_registry` میں رجسٹرڈ ہے۔

### مورچا مددگار

- `sorafs_car::CarBuildPlan::single_file` - آؤٹ سیٹس ، لمبائی اور بلیک 3 ہضموں کو آؤٹ پٹ ، لمبائی اور بلیک 3 ہضم کرتا ہے
  جب کار میٹا ڈیٹا تیار کرتے ہو۔
- `sorafs_car::ChunkStore` - پے لوڈ کو اسٹریم کرتا ہے ، کٹ میٹا ڈیٹا کو بچاتا ہے اور ایک درخت دکھاتا ہے
  پروف-ریٹریویبلٹی (POR) نمونے 64 KIB / 4 KIB۔
- `sorafs_chunker::chunk_bytes_with_digests` - لائبریری ہیلپر ، دونوں CLIS کے تحت واقع ہے۔

### سی ایل آئی ٹولز

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON میں آرڈر شدہ آفسیٹ ، لمبائی اور حصہ ڈائجسٹ شامل ہیں۔ جمع کرتے وقت منصوبے کو محفوظ کریں
آرکسٹریٹر کے لئے ظاہر یا نمونے لینے کی وضاحتیں۔

### پور گواہ

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` اور `--por-sample=<count>` فراہم کرتا ہے ،
تاکہ آڈیٹر گواہوں کے عین مطابق سیٹوں سے استفسار کرسکیں۔ ان جھنڈوں کے ساتھ جوڑیں
`--por-proof-out` یا `--por-sample-out` json لکھنے کے لئے۔

## 2. منشور کو لپیٹیں

`ManifestBuilder` گورننس منسلکات کے ساتھ حصہ میٹا ڈیٹا کو جوڑتا ہے:

- روٹ سی آئی ڈی (ڈی اے جی سی بی آر) اور کار کا ارتکاب۔
- عرف کا ثبوت اور فراہم کنندگان کی دعوے کی صلاحیتوں کا ثبوت۔
- کونسل کے دستخط اور اختیاری میٹا ڈیٹا (جیسے بلڈ آئی ڈی)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

اہم آؤٹ پٹ:

- `payload.manifest`- Norito-encoded مینی فیسٹ بائٹس۔
- `payload.report.json` - لوگوں/آٹومیشن کے لئے خلاصہ ، بشمول `chunk_fetch_specs` ،
  `payload_digest_hex` ، کار ڈائجسٹس اور عرف میٹا ڈیٹا۔
- `payload.manifest_signatures.json` - بلیک 3 منشور ڈائجسٹ پر مشتمل لفافہ ،
  SHA3 ​​چنک پلان ڈائجسٹ اور ترتیب شدہ دستخط ED25519۔

اس سے پہلے بیرونی دستخط کنندگان سے لفافوں کی توثیق کرنے کے لئے `--manifest-signatures-in` استعمال کریں
انتخاب کو ٹھیک کرنے کے لئے اوور رائٹ ، اور `--chunker-profile-id` یا `--chunker-profile=<handle>`
رجسٹری

## 3. اشاعت اور پننگ

1. ** گورننس میں جمع کروائیں ** - منشور ڈائجسٹ اور دستخطوں کا لفافہ کونسل کو جمع کروائیں
   پن قبول کیا جاسکتا ہے۔ بیرونی آڈیٹرز کو اس کے اگلے حصے کے SHA3 ڈائجسٹ کو ذخیرہ کرنا چاہئے
   منشور ڈائجسٹ۔
2.
   منشور ، پن رجسٹری میں۔ اس بات کو یقینی بنائیں کہ ظاہر اور کار ایک ہی جڑ سی آئی ڈی کا استعمال کریں۔
3.
   نمونے جاری کریں۔ یہ ریکارڈنگ آپریٹر ڈیش بورڈز کو فیڈ کرتی ہے اور دوبارہ پیش کرنے میں مدد کرتی ہے
   بڑے پے لوڈ کو لوڈ کیے بغیر مسائل۔

## 4. متعدد فراہم کنندگان سے نمونے لینے کا نقالی`کارگو رن -پی sorafs_car -bin sorafs_fetch ---plan = payload.report.json \
  -پرووئڈر = الفا = فراہم کنندہ/الفا.بن-پروویڈر= بی ٹی اے= پروویڈرز/بی ٹی اے ۔بن)
  -آؤٹ پٹ = پے لوڈ.بن-JSON-OUT = fetch_report.json`

- `#<concurrency>` ہر فراہم کنندہ (`#4` اوپر) میں ہم آہنگی میں اضافہ کرتا ہے۔
- `@<weight>` شیڈولنگ آفسیٹ کو تشکیل دیتا ہے۔ پہلے سے طے شدہ 1.
- `--max-peers=<n>` فراہم کرنے والوں کی تعداد کو محدود کرتا ہے جب شروع ہونے کے لئے شیڈول کیا جاتا ہے
  پتہ لگانے سے ضرورت سے زیادہ امیدواروں کی واپسی ہوتی ہے۔
- `--expect-payload-digest` اور `--expect-payload-len` خاموش ڈیٹا بدعنوانی سے بچاؤ۔
- `--provider-advert=name=advert.to` استعمال سے پہلے فراہم کنندہ کی صلاحیتوں کی جانچ کرتا ہے
  تخروپن میں
- `--retry-budget=<n>` فی حصہ تکرار کی تعداد کو اوور رائڈس (پہلے سے طے شدہ: 3) تاکہ CI
  ناکامیوں کی جانچ کرتے وقت تیزی سے شناخت کی گئی۔

`fetch_report.json` مجموعی میٹرکس (`chunk_retry_total` ، `provider_failure_rate` ، دکھاتا ہے
وغیرہ) سی آئی کے دعووں اور مشاہدے کے ل suitable موزوں ہے۔

## 5. رجسٹری اور گورننس اپڈیٹس

جب نئے چنکر پروفائلز کی تجویز پیش کرتے ہیں:

1. `sorafs_manifest::chunker_registry_data` پر ایک ہینڈل تیار کریں۔
2. `docs/source/sorafs/chunker_registry.md` اور اس سے وابستہ چارٹر کو اپ ڈیٹ کریں۔
3. فکسچر (`export_vectors`) کو دوبارہ تخلیق کریں اور دستخط شدہ ظاہر ہونے کا ارتکاب کریں۔
4. گورننس کے دستخطوں کے ساتھ چارٹر کی تعمیل سے متعلق ایک رپورٹ پیش کریں۔

آٹومیشن کو کیننیکل ہینڈلز (`namespace.name@semver`) اور کو ترجیح دینی چاہئے
عددی شناختی صرف اس وقت واپس جائیں جب پسماندہ مطابقت ضروری ہو۔