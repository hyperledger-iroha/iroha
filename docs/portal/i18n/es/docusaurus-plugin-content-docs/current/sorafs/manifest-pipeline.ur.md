---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS fragmentación → canalización de manifiesto

Inicio rápido de la tubería de inicio rápido de la tubería de inicio rápido de la tubería Norito
manifiesta میں بدلتا ہے جو SoraFS کے Pin Registry کے لیے موزوں ہیں۔ مواد
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
سے ماخوذ ہے؛ مستند وضاحت اور تبدیلی لاگ کے لیے اسی دستاویز سے رجوع کریں۔

## 1. fragmentación

SoraFS Programador SF-1 (`sorafs.sf1@1.0.0`) Programación: FastCDC Programación de programas informáticos
کم از کم trozo سائز 64 KiB، ہدف 256 KiB، زیادہ سے زیادہ 512 KiB اور بریک ماسک `0x0000ffff`
ہے۔ یہ پروفائل `sorafs_manifest::chunker_registry` میں رجسٹر ہے۔

### Óxido مددگار

- `sorafs_car::CarBuildPlan::single_file` – CAR میٹا ڈیٹا تیار کرتے ہوئے trozos کے compensaciones,
  لمبائیاں اور BLAKE3 digiere جاری کرتا ہے۔
- `sorafs_car::ChunkStore` – cargas útiles کو اسٹریم کرتا ہے، fragmentos کا میٹا ڈیٹا محفوظ کرتا ہے اور
  64 KiB / 4 KiB کی Prueba de recuperación (PoR) سیمپلنگ ٹری اخذ کرتا ہے۔
- `sorafs_chunker::chunk_bytes_with_digests` – CLIs دونوں کے پیچھے لائبریری ayudante۔

### CLI ٹولنگ

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON میں ترتیب وار offsets, لمبائیاں اور fragment digests ہوتے ہیں۔ manifiesto یا آرکسٹریٹر buscar
اسپیسفیکیشن بناتے وقت اس پلان کو محفوظ رکھیں۔

### PoR گواہیاں

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` اور `--por-sample=<count>` فراہم کرتا ہے
تاکہ آڈیٹرز حتمی گواہی سیٹ مانگ سکیں۔ ان banderas کو `--por-proof-out` یا `--por-sample-out`
کے ساتھ استعمال کریں تاکہ JSON ریکارڈ ہو جائے۔

## 2. manifiesto کو لپیٹنا`ManifestBuilder` fragmentos کے میٹا ڈیٹا کو گورننس اٹیچمنٹس کے ساتھ جوڑتا ہے:

- روٹ CID (dag-cbor) اور compromisos CAR۔
- alias ثبوت اور فراہم کنندہ صلاحیت کے دعوے۔
- کونسل کی دستخطیں اور اختیاری میٹا ڈیٹا (مثلاً ID de compilación) ۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

اہم آؤٹ پٹس:

- `payload.manifest` – Norito Manifiesto انکوڈڈ بائٹس۔
- `payload.report.json` – انسان/آٹومیشن کیلئے قابلِ فہم خلاصہ، جس میں `chunk_fetch_specs`,
  `payload_digest_hex`, CAR digiere اور alias میٹا ڈیٹا شامل ہیں۔
- `payload.manifest_signatures.json` – لفافہ جس میں manifiesto کا BLAKE3 digest, trozo پلان کا
  Resumen SHA3 اور ترتیب شدہ Ed25519 دستخط شامل ہیں۔

`--manifest-signatures-in` استعمال کریں تاکہ بیرونی دستخط کنندگان کے لفافوں کو دوبارہ لکھنے
سے پہلے تصدیق کیا جا سکے، اور `--chunker-profile-id` یا `--chunker-profile=<handle>` کے ذریعے
رجسٹری انتخاب کو لاک کریں۔

## 3. اشاعت اور پننگ1. **گورننس میں جمع کرانا** – resumen manifiesto اور دستخطی لفافہ کونسل کو فراہم کریں تاکہ pin
   منظور ہو سکے۔ بیرونی آڈیٹرز کو trozo پلان کا Resumen SHA3 resumen manifiesto کے ساتھ محفوظ رکھنا چاہیے۔
2. **cargas útiles کو پن کرنا** – manifiesto میں حوالہ دی گئی CAR آرکائیو (اور اختیاری CAR انڈیکس)
   کو Registro de PIN میں اپ لوڈ کریں۔ یقینی بنائیں کہ manifiesto اور CAR ایک ہی روٹ CID شیئر کرتے ہیں۔
3. **ٹیلی میٹری ریکارڈ کرنا** – JSON رپورٹ، PoR گواہیاں اور کسی بھی buscar میٹرکس کو ریلیز
   آرٹیفیکٹس میں محفوظ کریں۔ یہ ریکارڈز آپریٹر ڈیش بورڈز کو فیڈ کرتے ہیں اور بڑے cargas útiles
   ڈاؤن لوڈ کیے بغیر مسائل دوبارہ پیدا کرنے میں مدد دیتے ہیں۔

## 4. ملٹی پرووائیڈر buscar سمیولیشن

`ejecución de carga -p sorafs_car --bin sorafs_fetch --plan=payload.report.json \
  --provider=alpha=proveedores/alpha.bin --provider=beta=proveedores/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` ہر پرووائیڈر کے لیے paralelismo بڑھاتا ہے (`#4` اوپر)۔
- `@<weight>` شیڈولنگ sesgo کو ایڈجسٹ کرتا ہے؛ ڈیفالٹ 1 ہے۔
- `--max-peers=<n>` دریافت میں بہت سے امیدوار آنے پر رن کے لیے منتخب پرووائیڈرز کی تعداد محدود کرتا ہے۔
- `--expect-payload-digest` اور `--expect-payload-len` خاموش کرپشن سے بچاتے ہیں۔
- `--provider-advert=name=advert.to` سمیولیشن سے پہلے پرووائیڈر کی صلاحیتوں کی توثیق کرتا ہے۔
- `--retry-budget=<n>` ہر fragmento کی ری ٹرائی تعداد (ڈیفالٹ: 3) بدلتا ہے تاکہ CI ناکامی کے
  منظرناموں میں رگریشنز جلد ظاہر کرے۔`fetch_report.json` مجموعی میٹرکس (`chunk_retry_total`, `provider_failure_rate` وغیرہ) دکھاتا ہے
جو afirmaciones de CI اور آبزرویبلٹی کے لیے موزوں ہیں۔

## 5. رجسٹری اپڈیٹس اور گورننس

نئے chunker پروفائل تجویز کرتے وقت:

1. `sorafs_manifest::chunker_registry_data` Descriptor میں لکھیں۔
2. `docs/source/sorafs/chunker_registry.md` اور متعلقہ cartas کو اپڈیٹ کریں۔
3. accesorios (`export_vectors`) دوبارہ جنریٹ کریں اور manifiestos firmados حاصل کریں۔
4. گورننس دستخطوں کے ساتھ cumplimiento de los estatutos رپورٹ جمع کریں۔

آٹومیشن کو manijas canónicas (`namespace.name@semver`) کو ترجیح دینی چاہیے اور صرف
بیک ورڈ کمپٹیبیلیٹی کی ضرورت پڑنے پر ہی عددی ID پر واپس جانا چاہیے۔