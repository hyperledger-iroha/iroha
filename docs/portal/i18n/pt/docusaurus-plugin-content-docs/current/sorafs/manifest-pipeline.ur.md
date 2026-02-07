---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS fragmentação → pipeline de manifesto

یہ início rápido کا ساتھی دستاویز اُس مکمل pipeline کو بیان کرتا ہے جو خام بائٹس کو Norito
manifestos میں بدلتا ہے جو SoraFS کے Pin Registry کے لیے موزوں ہیں۔ Mãe
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
سے ماخوذ ہے؛ مستند وضاحت اور تبدیلی لاگ کے لیے اسی دستاویز سے رجوع کریں۔

## 1. حتمی fragmentação

SoraFS پروفائل SF-1 (`sorafs.sf1@1.0.0`) Cartão de crédito: FastCDC para download gratuito
Um pedaço de 64 KiB, 256 KiB, um pedaço de 512 KiB e 18NI00000010X
ہے۔ یہ پروفائل `sorafs_manifest::chunker_registry` میں رجسٹر ہے۔

### Ferrugem

- `sorafs_car::CarBuildPlan::single_file` – CAR میٹا ڈیٹا تیار کرتے ہوئے pedaços کے compensações,
  لمبائیاں اور BLAKE3 digests جاری کرتا ہے۔
- `sorafs_car::ChunkStore` – cargas úteis کو اسٹریم کرتا ہے, pedaços کا میٹا ڈیٹا محفوظ کرتا ہے اور
  64 KiB / 4 KiB کی Prova de Recuperabilidade (PoR) سیمپلنگ ٹری اخذ کرتا ہے۔
- `sorafs_chunker::chunk_bytes_with_digests` – CLIs کے پیچھے لائبریری helper۔

### CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON میں ترتیب وار offsets, لمبائیاں اور chunk digests ہوتے ہیں۔ manifesto یا آرکسٹریٹر buscar
اسپیسفیکیشن بناتے وقت اس پلان کو محفوظ رکھیں۔

### PoR گواہیاں

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` e `--por-sample=<count>` são de propriedade
تاکہ آڈیٹرز حتمی گواہی سیٹ مانگ سکیں۔ Os sinalizadores são `--por-proof-out` e `--por-sample-out`
Qual é o valor do arquivo JSON ریکارڈ ہو جائے۔

## 2. manifesto

Pedaços `ManifestBuilder` کے میٹا ڈیٹا کو گورننس اٹیچمنٹس کے ساتھ جوڑتا ہے:

- روٹ CID (dag-cbor) اور compromissos CAR۔
- alias ثبوت اور فراہم کنندہ صلاحیت کے دعوے۔
- کونسل کی دستخطیں اور اختیاری میٹا ڈیٹا (مثلاً IDs de compilação)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

O que é isso:

- `payload.manifest` – Norito Manifesto do arquivo بائٹس۔
- `payload.report.json` – انسان/آٹومیشن کیلئے قابلِ فہم خلاصہ, جس میں `chunk_fetch_specs`,
  `payload_digest_hex`, CAR digests اور alias میٹا ڈیٹا شامل ہیں۔
- `payload.manifest_signatures.json` – o arquivo do manifesto کا BLAKE3 digest, chunk پلان کا
  Digest SHA3 اور ترتیب شدہ Ed25519 دستخط شامل ہیں۔

`--manifest-signatures-in` استعمال کریں تاکہ بیرونی دستخط کنندگان کے لفافوں کو دوبارہ لکھنے
Você pode usar o `--chunker-profile-id` ou `--chunker-profile=<handle>` para obter mais informações
رجسٹری انتخاب کو لاک کریں۔

## 3. اشاعت اور پننگ

1. **گورننس میں جمع کرانا** – resumo do manifesto اور دستخطی لفافہ کونسل کو فراہم کریں تاکہ pin
   منظور ہو سکے۔ بیرونی آڈیٹرز کو chunk پلان کا SHA3 digest manifest digest digest کے ساتھ محفوظ رکھنا چاہیے۔
2. **payloads کو پن کرنا** – manifesto میں حوالہ دی گئی CAR آرکائیو (اور اختیاری CAR انڈیکس)
   کو Pin Registry میں اپ لوڈ کریں۔ یقینی بنائیں کہ manifesto اور CAR ایک ہی روٹ CID شیئر کرتے ہیں۔
3. **ٹیلی میٹری ریکارڈ کرنا** – JSON رپورٹ، PoR گواہیاں اور کسی بھی fetch میٹرکس کو ریلیز
   آرٹیفیکٹس میں محفوظ کریں۔ یہ ریکارڈز آپریٹر ڈیش بورڈز کو فیڈ کرتے ہیں اور بڑے cargas úteis
   ڈاؤن لوڈ کیے بغیر مسائل دوبارہ پیدا کرنے میں مدد دیتے ہیں۔

## 4. ملٹی پرووائیڈر buscar سمیولیشن

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=provedores/alpha.bin --provider=beta=provedores/beta.bin#4@3 \
  --output = carga útil.bin --json-out = fetch_report.json`- `#<concurrency>` ہر پروائیڈر کے لیے paralelismo بڑھاتا ہے (`#4` اوپر)۔
- `@<weight>` شیڈولنگ viés کو ایڈجسٹ کرتا ہے؛ ڈیفالٹ 1 ہے۔
- `--max-peers=<n>` دریافت میں بہت سے امیدوار آنے پر کے لیے منتخب پرووائیڈرز کی تعداد محدود کرتا ہے۔
- `--expect-payload-digest` e `--expect-payload-len` خاموش کرپشن سے بچاتے ہیں۔
- `--provider-advert=name=advert.to` سمیولیشن سے پہلے پرووائیڈر کی صلاحیتوں کی توثیق کرتا ہے۔
- `--retry-budget=<n>` ہر chunk کی ری ٹرائی تعداد (ڈیفالٹ: 3) بدلتا ہے تاکہ CI ناکامی کے
  منظرناموں میں رگریشنز جلد ظاہر کرے۔

`fetch_report.json` مجموعی میٹرکس (`chunk_retry_total`, `provider_failure_rate` وغیرہ) دکھاتا ہے
As asserções do CI são importantes para você

## 5. رجسٹری اپڈیٹس اور گورننس

Qual chunker é o que você precisa saber:

1. `sorafs_manifest::chunker_registry_data` میں descritor لکھیں۔
2. `docs/source/sorafs/chunker_registry.md` اور متعلقہ charters کو اپڈیٹ کریں۔
3. fixtures (`export_vectors`) دوبارہ جنریٹ کریں اور manifestos assinados حاصل کریں۔
4. گورننس دستخطوں کے ساتھ conformidade com o regulamento رپورٹ جمع کریں۔

Existem identificadores canônicos (`namespace.name@semver`) que são usados ​​para usar
بیک ورڈ کمپٹیبیلیٹی کی ضرورت پڑنے پر ہی عددی IDs پر واپس جانا چاہیے۔