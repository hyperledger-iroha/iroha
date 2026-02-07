---
lang: ur
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-04T10:50:53.604570+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS chunking → مینی فیسٹ پائپ لائن

کوئک اسٹارٹ کے اس ساتھی نے آخر سے آخر تک پائپ لائن کا سراغ لگایا جو خام ہوجاتا ہے
Norito میں بائٹس SoraFS پن رجسٹری کے لئے موزوں ہیں۔ مواد ہے
[`docs/source/sorafs/manifest_pipeline.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md) سے موافقت پذیر ؛
کیننیکل تفصیلات اور تبدیلی کے ل that اس دستاویز سے مشورہ کریں۔

## 1. جزوی طور پر حصہ

SoraFS SF-1 (`sorafs.sf1@1.0.0`) پروفائل استعمال کرتا ہے: ایک فاسٹ سی ڈی سی سے متاثرہ رولنگ
64kib کم سے کم حصہ سائز ، 256kib ہدف ، 512kib زیادہ سے زیادہ ، اور a کے ساتھ ہیش
`0x0000ffff` بریک ماسک۔ پروفائل میں رجسٹرڈ ہے
`sorafs_manifest::chunker_registry`۔

### مورچا مددگار

- `sorafs_car::CarBuildPlan::single_file` - کٹ آفسیٹس ، لمبائی ، اور خارج کرتا ہے
  بلیک 3 ہضم ہوتا ہے جب کار میٹا ڈیٹا تیار کرتے ہیں۔
- `sorafs_car::ChunkStore` - ندیوں کے پے لوڈز ، برقرار میٹا ڈیٹا ، اور
  64kib / 4kib پروف-آف-ریفریٹیبلٹی (POR) نمونے لینے کے درخت کو اخذ کرتا ہے۔
- `sorafs_chunker::chunk_bytes_with_digests` - دونوں CLIs کے پیچھے لائبریری مددگار۔

### CLI ٹولنگ

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON میں آرڈر شدہ آفسیٹس ، لمبائی اور حصہ ڈائجسٹس پر مشتمل ہے۔ برقرار رکھیں
جب منشور یا آرکسٹریٹر کی تشکیل کی وضاحتیں تعمیر کرتے ہو تو منصوبہ بنائیں۔

### پور گواہ

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` کو بے نقاب کرتا ہے اور
`--por-sample=<count>` تاکہ آڈیٹر عین مطابق گواہ سیٹ کی درخواست کرسکیں۔ جوڑی
JSON کو ریکارڈ کرنے کے لئے `--por-proof-out` یا `--por-sample-out` کے ساتھ وہ جھنڈے۔

## 2. ایک منشور لپیٹیں

`ManifestBuilder` گورننس منسلکات کے ساتھ حصہ میٹا ڈیٹا کو جوڑتا ہے:

- روٹ سی آئی ڈی (ڈی اے جی سی بی آر) اور کار کے وعدے۔
- عرف ثبوت اور فراہم کنندہ کی اہلیت کے دعوے۔
- کونسل کے دستخط اور اختیاری میٹا ڈیٹا (جیسے ، آئی ڈی بلڈ آئی ڈی)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

اہم نتائج:

- `payload.manifest`- Norito-encoded مینی فیسٹ بائٹس۔
- `payload.report.json` - انسانی/آٹومیشن پڑھنے کے قابل خلاصہ ، بشمول
  `chunk_fetch_specs` ، `payload_digest_hex` ، کار ڈائجسٹس ، اور عرف میٹا ڈیٹا۔
- `payload.manifest_signatures.json` - منشور بلیک 3 پر مشتمل لفافہ
  ڈائجسٹ ، چنک پلان SHA3 ڈائجسٹ ، اور ترتیب شدہ ED25519 دستخط۔

بیرونی کے ذریعہ فراہم کردہ لفافوں کی توثیق کرنے کے لئے `--manifest-signatures-in` کا استعمال کریں
ان کو واپس لکھنے سے پہلے دستخطی ، اور `--chunker-profile-id` یا
`--chunker-profile=<handle>` رجسٹری کے انتخاب کو لاک کرنے کے لئے۔

## 3. شائع اور پن

1. ** گورننس جمع کرانا ** - ظاہر ہضم اور دستخط فراہم کریں
   کونسل کا لفافہ تاکہ پن کو داخل کیا جاسکے۔ بیرونی آڈیٹرز کو چاہئے
   منشور ڈائجسٹ کے ساتھ ساتھ چنک پلان SHA3 ڈائجسٹ اسٹور کریں۔
2. ** پن پے لوڈز ** - کار آرکائیو (اور اختیاری کار انڈیکس) کا حوالہ اپ لوڈ کریں
   پن رجسٹری کے ظاہر میں۔ یقینی بنائیں کہ ظاہر اور کار کا اشتراک کریں
   ایک ہی جڑ سیڈ۔
3.
   ریلیز نمونے میں میٹرکس۔ یہ ریکارڈ آپریٹر ڈیش بورڈز کو فیڈ کرتے ہیں اور
   بڑے پے لوڈ کو ڈاؤن لوڈ کیے بغیر مسائل کو دوبارہ پیش کرنے میں مدد کریں۔

## 4. ملٹی فراہم کرنے والا بازیافت تخروپن

`کارگو رن -پی sorafs_car -bin sorafs_fetch ---plan = payload.report.json \
  -پرووئڈر = الفا = فراہم کنندہ/الفا.بن-پروویڈر= بی ٹی اے= پروویڈرز/بی ٹی اے ۔بن)
  -آؤٹ پٹ = پے لوڈ.بن-JSON-OUT = fetch_report.json`- `#<concurrency>` فی فراہم کنندہ متوازی (`#4` اوپر) میں اضافہ کرتا ہے۔
- `@<weight>` ٹونز شیڈولنگ تعصب ؛ پہلے سے طے شدہ 1.
- `--max-peers=<n>` کیپس ایک رن کے لئے شیڈول فراہم کرنے والوں کی تعداد جب
  دریافت سے مطلوبہ سے زیادہ امیدوار حاصل ہوتے ہیں۔
- `--expect-payload-digest` اور `--expect-payload-len` گارڈ خاموش کے خلاف
  بدعنوانی
- `--provider-advert=name=advert.to` فراہم کنندہ کی صلاحیتوں کی تصدیق کرتا ہے
  ان کو تخروپن میں استعمال کرنا۔
- `--retry-budget=<n>` فی چنک دوبارہ کوشش کرنے والی گنتی (پہلے سے طے شدہ: 3) تو CI
  ناکامی کے منظرناموں کی جانچ کرتے وقت سطح کے رجعتیں تیزی سے کرسکتے ہیں۔

`fetch_report.json` سطحوں کی مجموعی میٹرکس (`chunk_retry_total` ،
`provider_failure_rate` ، وغیرہ) CI کے دعووں اور مشاہدے کے لئے موزوں ہے۔

## 5. رجسٹری کی تازہ کاری اور گورننس

جب نئے چنکر پروفائلز کی تجویز پیش کرتے ہیں:

1۔ مصنف `sorafs_manifest::chunker_registry_data` میں ڈسکرپٹر۔
2. `docs/source/sorafs/chunker_registry.md` اور متعلقہ چارٹر کو اپ ڈیٹ کریں۔
3. دوبارہ تخلیق فکسچر (`export_vectors`) اور گرفتاری کے منشور پر قبضہ کریں۔
4. گورننس کے دستخطوں کے ساتھ چارٹر تعمیل رپورٹ جمع کروائیں۔

آٹومیشن کو کیننیکل ہینڈلز (`namespace.name@semver`) اور زوال کو ترجیح دینی چاہئے