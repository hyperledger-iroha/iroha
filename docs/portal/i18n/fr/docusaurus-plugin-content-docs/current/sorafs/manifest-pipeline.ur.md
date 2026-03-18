---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS chunking → pipeline manifeste

Un guide de démarrage rapide pour le pipeline et le pipeline Norito
manifeste le nom du registre SoraFS pour le registre des broches et le nom du registre. مواد
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
سے ماخوذ ہے؛ مستند وضاحت اور تبدیلی لاگ کے لیے اسی دستاویز سے رجوع کریں۔

## 1. حتمی chunking

SoraFS pour SF-1 (`sorafs.sf1@1.0.0`) Nom du produit : FastCDC est disponible en ligne.
Un morceau de morceau pèse 64 KiB, ou 256 KiB, ou 512 KiB pour une mémoire `0x0000ffff`
ہے۔ یہ پروفائل `sorafs_manifest::chunker_registry` میں رجسٹر ہے۔

### Rust مددگار

- `sorafs_car::CarBuildPlan::single_file` – CAR contient des morceaux et des décalages
  لمبائیاں اور BLAKE3 digests جاری کرتا ہے۔
- `sorafs_car::ChunkStore` – charges utiles pour les morceaux et les morceaux
  64 KiB / 4 KiB pour la preuve de récupérabilité (PoR)
- `sorafs_chunker::chunk_bytes_with_digests` – Les CLI sont une aide précieuse pour les CLI

### CLI ٹولنگ

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON est un outil de compensation et de compensation pour les résumés de fragments manifeste یا آرکسٹریٹر récupérer
اسپیسفیکیشن بناتے وقت اس پلان کو محفوظ رکھیں۔

### PoR گواہیاں

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` et `--por-sample=<count>` فراہم کرتا ہے
تاکہ آڈیٹرز حتمی گواہی سیٹ مانگ سکیں۔ Les drapeaux sont `--por-proof-out` et `--por-sample-out`
Le code JSON est utilisé pour créer des fichiers JSON.

## 2. manifeste کو لپیٹنا`ManifestBuilder` chunks کے میٹا ڈیٹا کو گورننس اٹیچمنٹس کے ساتھ جوڑتا ہے :

- روٹ CID (dag-cbor) اور Engagements RCA۔
- alias ثبوت اور فراہم کنندہ صلاحیت کے دعوے۔
- Les identifiants de build et les identifiants de build suivants

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

اہم آؤٹ پٹس:

- `payload.manifest` – Norito manifeste en ligne
- `payload.report.json` – Prise en charge/désactivation de l'application `chunk_fetch_specs`,
  `payload_digest_hex`, CAR digère et alias میٹا ڈیٹا شامل ہیں۔
- `payload.manifest_signatures.json` – il s'agit d'un manifeste du résumé BLAKE3, d'un morceau de fichier
  SHA3 digest اور ترتیب شدہ Ed25519 دستخط شامل ہیں۔

`--manifest-signatures-in` استعمال کریں تاکہ بیرونی دستخط کنندگان کے لفافوں کو دوبارہ لکھنے
Il s'agit d'une solution pour `--chunker-profile-id` et `--chunker-profile=<handle>`.
رجسٹری انتخاب کو لاک کریں۔

## 3. اشاعت اور پننگ1. ** گورننس میں جمع کرانا** – manifeste digest اور دستخطی لفافہ کونسل کو فراہم کریں تاکہ pin
   منظور ہو سکے۔ Il s'agit d'un morceau de résumé du résumé du manifeste SHA3 digest qui contient un fragment de fichier.
2. **charges utiles pour les véhicules** – manifeste pour les véhicules CAR انڈیکس (اور اختیاری CAR انڈیکس)
   کو Pin Registry میں اپ لوڈ کریں۔ یقینی بنائیں کہ manifest اور CAR ایک ہی روٹ CID شیئر کرتے ہیں۔
3. **ٹیلی ریٹری ریکارڈ کرنا** – JSON رپورٹ، PoR گواہیاں اور کسی بھی fetch میٹرکس کو ریلیز
   آرٹیفیکٹس میں محفوظ کریں۔ Il s'agit d'un ensemble de charges utiles
   ڈاؤن لوڈ کیے بغیر مسائل دوبارہ پیدا کرنے میں مدد دیتے ہیں۔

## 4. ملٹی پرووائیڈر récupérer سمیولیشن

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` pour le parallélisme (`#4`)
- `@<weight>` biais de polarisation en ligne ڈیفالٹ 1 ہے۔
- `--max-peers=<n>` L'appareil est prêt à être utilisé pour obtenir un appareil photo کرتا ہے۔
- `--expect-payload-digest` et `--expect-payload-len` خاموش کرپشن سے بچاتے ہیں۔
- `--provider-advert=name=advert.to` سمیولیشن سے پہلے پرووائیڈر کی صلاحیتوں کی توثیق کرتا ہے۔
- `--retry-budget=<n>` ہر chunk کی ری ٹرائی تعداد (ڈیفالٹ: 3) بدلتا ہے تاکہ CI ناکامی کے
  منظرناموں میں رگریشنز جلد ظاہر کرے۔`fetch_report.json` مجموعی میٹرکس (`chunk_retry_total`, `provider_failure_rate` وغیرہ) دکھاتا ہے
جو Les affirmations de CI اور آبزرویبلٹی کے لیے موزوں ہیں۔

## 5. رجسٹری اپڈیٹس اور گورننس

La version chunker est la suivante :

1. `sorafs_manifest::chunker_registry_data` descripteur لکھیں۔
2. `docs/source/sorafs/chunker_registry.md` pour les charters et les charters
3. luminaires (`export_vectors`) دوبارہ جنریٹ کریں اور manifestes signés حاصل کریں۔
4. Conformité à la charte en matière de respect de la charte

Poignées canoniques (`namespace.name@semver`) pour les poignées canoniques (`namespace.name@semver`)
Vous avez besoin d'une carte d'identité pour les identifiants de votre compte