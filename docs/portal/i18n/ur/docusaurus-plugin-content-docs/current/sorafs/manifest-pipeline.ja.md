---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1564d4cb31bceeeca07ce611f386a53e77b1f2aa8e488249e5dc46946b7c2496
source_last_modified: "2025-11-14T04:43:21.768474+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS chunking → manifest pipeline

یہ quickstart کا ساتھی دستاویز اُس مکمل pipeline کو بیان کرتا ہے جو خام بائٹس کو Norito
manifests میں بدلتا ہے جو SoraFS کے Pin Registry کے لیے موزوں ہیں۔ مواد
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
سے ماخوذ ہے؛ مستند وضاحت اور تبدیلی لاگ کے لیے اسی دستاویز سے رجوع کریں۔

## 1. حتمی chunking

SoraFS پروفائل SF-1 (`sorafs.sf1@1.0.0`) استعمال کرتا ہے: FastCDC سے متاثر رولنگ ہیش جس میں
کم از کم chunk سائز 64 KiB، ہدف 256 KiB، زیادہ سے زیادہ 512 KiB اور بریک ماسک `0x0000ffff`
ہے۔ یہ پروفائل `sorafs_manifest::chunker_registry` میں رجسٹر ہے۔

### Rust مددگار

- `sorafs_car::CarBuildPlan::single_file` – CAR میٹا ڈیٹا تیار کرتے ہوئے chunks کے offsets،
  لمبائیاں اور BLAKE3 digests جاری کرتا ہے۔
- `sorafs_car::ChunkStore` – payloads کو اسٹریم کرتا ہے، chunks کا میٹا ڈیٹا محفوظ کرتا ہے اور
  64 KiB / 4 KiB کی Proof-of-Retrievability (PoR) سیمپلنگ ٹری اخذ کرتا ہے۔
- `sorafs_chunker::chunk_bytes_with_digests` – دونوں CLIs کے پیچھے لائبریری helper۔

### CLI ٹولنگ

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON میں ترتیب وار offsets، لمبائیاں اور chunk digests ہوتے ہیں۔ manifest یا آرکسٹریٹر fetch
اسپیسفیکیشن بناتے وقت اس پلان کو محفوظ رکھیں۔

### PoR گواہیاں

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` اور `--por-sample=<count>` فراہم کرتا ہے
تاکہ آڈیٹرز حتمی گواہی سیٹ مانگ سکیں۔ ان flags کو `--por-proof-out` یا `--por-sample-out`
کے ساتھ استعمال کریں تاکہ JSON ریکارڈ ہو جائے۔

## 2. manifest کو لپیٹنا

`ManifestBuilder` chunks کے میٹا ڈیٹا کو گورننس اٹیچمنٹس کے ساتھ جوڑتا ہے:

- روٹ CID (dag-cbor) اور CAR commitments۔
- alias ثبوت اور فراہم کنندہ صلاحیت کے دعوے۔
- کونسل کی دستخطیں اور اختیاری میٹا ڈیٹا (مثلاً build IDs)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

اہم آؤٹ پٹس:

- `payload.manifest` – Norito انکوڈڈ manifest بائٹس۔
- `payload.report.json` – انسان/آٹومیشن کیلئے قابلِ فہم خلاصہ، جس میں `chunk_fetch_specs`,
  `payload_digest_hex`, CAR digests اور alias میٹا ڈیٹا شامل ہیں۔
- `payload.manifest_signatures.json` – لفافہ جس میں manifest کا BLAKE3 digest، chunk پلان کا
  SHA3 digest اور ترتیب شدہ Ed25519 دستخط شامل ہیں۔

`--manifest-signatures-in` استعمال کریں تاکہ بیرونی دستخط کنندگان کے لفافوں کو دوبارہ لکھنے
سے پہلے تصدیق کیا جا سکے، اور `--chunker-profile-id` یا `--chunker-profile=<handle>` کے ذریعے
رجسٹری انتخاب کو لاک کریں۔

## 3. اشاعت اور پننگ

1. **گورننس میں جمع کرانا** – manifest digest اور دستخطی لفافہ کونسل کو فراہم کریں تاکہ pin
   منظور ہو سکے۔ بیرونی آڈیٹرز کو chunk پلان کا SHA3 digest manifest digest کے ساتھ محفوظ رکھنا چاہیے۔
2. **payloads کو پن کرنا** – manifest میں حوالہ دی گئی CAR آرکائیو (اور اختیاری CAR انڈیکس)
   کو Pin Registry میں اپ لوڈ کریں۔ یقینی بنائیں کہ manifest اور CAR ایک ہی روٹ CID شیئر کرتے ہیں۔
3. **ٹیلی میٹری ریکارڈ کرنا** – JSON رپورٹ، PoR گواہیاں اور کسی بھی fetch میٹرکس کو ریلیز
   آرٹیفیکٹس میں محفوظ کریں۔ یہ ریکارڈز آپریٹر ڈیش بورڈز کو فیڈ کرتے ہیں اور بڑے payloads
   ڈاؤن لوڈ کیے بغیر مسائل دوبارہ پیدا کرنے میں مدد دیتے ہیں۔

## 4. ملٹی پرووائیڈر fetch سمیولیشن

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` ہر پرووائیڈر کے لیے parallelism بڑھاتا ہے (`#4` اوپر)۔
- `@<weight>` شیڈولنگ bias کو ایڈجسٹ کرتا ہے؛ ڈیفالٹ 1 ہے۔
- `--max-peers=<n>` دریافت میں بہت سے امیدوار آنے پر رن کے لیے منتخب پرووائیڈرز کی تعداد محدود کرتا ہے۔
- `--expect-payload-digest` اور `--expect-payload-len` خاموش کرپشن سے بچاتے ہیں۔
- `--provider-advert=name=advert.to` سمیولیشن سے پہلے پرووائیڈر کی صلاحیتوں کی توثیق کرتا ہے۔
- `--retry-budget=<n>` ہر chunk کی ری ٹرائی تعداد (ڈیفالٹ: 3) بدلتا ہے تاکہ CI ناکامی کے
  منظرناموں میں رگریشنز جلد ظاہر کرے۔

`fetch_report.json` مجموعی میٹرکس (`chunk_retry_total`, `provider_failure_rate` وغیرہ) دکھاتا ہے
جو CI assertions اور آبزرویبلٹی کے لیے موزوں ہیں۔

## 5. رجسٹری اپڈیٹس اور گورننس

نئے chunker پروفائل تجویز کرتے وقت:

1. `sorafs_manifest::chunker_registry_data` میں descriptor لکھیں۔
2. `docs/source/sorafs/chunker_registry.md` اور متعلقہ charters کو اپڈیٹ کریں۔
3. fixtures (`export_vectors`) دوبارہ جنریٹ کریں اور signed manifests حاصل کریں۔
4. گورننس دستخطوں کے ساتھ charter compliance رپورٹ جمع کریں۔

آٹومیشن کو canonical handles (`namespace.name@semver`) کو ترجیح دینی چاہیے اور صرف
بیک ورڈ کمپٹیبیلیٹی کی ضرورت پڑنے پر ہی عددی IDs پر واپس جانا چاہیے۔
