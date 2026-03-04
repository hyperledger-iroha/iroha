---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS کا فوری آغاز

یہ عملی رہنمائی SoraFS اسٹوریج پائپ لائن کی بنیاد بننے والے
determinista SF-1 چنکر پروفائل، مینی فیسٹ سائننگ، اور متعدد فراہم کنندگان سے
buscar فلو پر لے جاتی ہے۔ ڈیزائن نوٹس اور CLI فلیگ ریفرنس کے لیے اسے
[canalización de manifiesto کی تفصیلی وضاحت](manifest-pipeline.md) کے ساتھ استعمال کریں۔

## ضروریات

- Rust ٹول چین (`rustup update`), اور espacio de trabajo لوکل طور پر کلون ہو۔
- Información: [Par de claves OpenSSL کے مطابق Ed25519](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  مینی فیسٹ سائن کرنے کے لیے۔
- Nombre: Node.js ≥ 18 اگر آپ Docusaurus پورٹل کا پیش نظارہ کرنا چاہتے ہیں۔

`export RUST_LOG=info` سیٹ کریں تاکہ تجربات کے دوران مفید CLI پیغامات سامنے آئیں۔

## 1. accesorios deterministas تازہ کریں

SF-1 کے vectores de fragmentación canónicos دوبارہ تیار کریں۔ جب `--signing-key` فراہم کیا
جائے تو یہ کمانڈ sobres de manifiesto firmados بھی بناتی ہے؛ `--allow-unsigned` Negro
مقامی ڈیولپمنٹ میں استعمال کریں۔

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

آؤٹ پٹ:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (اگر سائن ہوا ہو)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. carga útil کو fragmento کریں اور پلان دیکھیں

`sorafs_chunker` سے کسی بھی فائل یا آرکائیو کو trozo کریں:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

اہم فیلڈز:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` کے پیرامیٹرز کی تصدیق۔
- `chunks[]` – Mejora y compensaciones, longitudes, resúmenes de fragmentos BLAKE3

بڑے accesorios کے لیے، proptest پر مبنی regresión چلائیں تاکہ streaming اور lotes
fragmentación ہم آہنگ رہے:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```## 3. manifiesto بنائیں اور سائن کریں

plan de fragmentos, alias, firmas de gobierno کو `sorafs-manifest-stub` کے ذریعے
manifiesto میں لپیٹیں۔ نیچے دی گئی کمانڈ carga útil de un solo archivo دکھاتی ہے؛ درخت پیک کرنے
کے لیے ruta de directorio دیں (CLI اسے lexicográfico ترتیب میں چلتا ہے)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` Número de modelo:

- `chunking.chunk_digest_sha3_256` – compensaciones/longitudes en resumen SHA3, accesorios de fragmentación en color
- `manifest.manifest_blake3` – sobre de manifiesto میں سائن کیا گیا BLAKE3 digest۔
- `chunk_fetch_specs[]` – orquestadores کے لیے ترتیب وار fetch ہدایات۔

جب حقیقی firmas دینے کے لیے تیار ہوں تو `--signing-key` اور `--signer` argumentos شامل
کریں۔ کمانڈ sobre لکھنے سے پہلے ہر Ed25519 firma کی توثیق کرتی ہے۔

## 4. recuperación de múltiples proveedores کی سمیولیشن

desarrollador buscar CLI سے plan de fragmentos کو ایک یا زیادہ proveedores کے خلاف repetición کریں۔ یہ CI
pruebas de humo اور creación de prototipos de orquestador کے لیے بہترین ہے۔

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Funciones:

- `payload_digest_hex` کو manifiesto رپورٹ سے ملنا چاہیے۔
- `provider_reports[]` ہر proveedor کے لیے éxito/fracaso cuenta دکھاتا ہے۔
- غیر صفر `chunk_retry_total` contrapresión ایڈجسٹمنٹ دکھاتا ہے۔
- `--max-peers=<n>` کے ذریعے ejecutar میں شیڈول ہونے والے proveedores کی تعداد محدود کریں اور CI
  simulaciones کو اہم امیدواروں پر مرکوز رکھیں۔
- `--retry-budget=<n>` recuento de reintentos por fragmento (3) کو anular کرتا ہے تاکہ fallas de inyección
  کرنے پر regresiones del orquestador جلد ظاہر ہوں۔`--expect-payload-digest=<hex>` اور `--expect-payload-len=<bytes>` شامل کریں تاکہ reconstruido
carga útil اگر manifiesto سے ہٹے تو فوراً fail ہو جائے۔

## 5. اگلے اقدامات

- **Integración de la gobernanza** – resumen del manifiesto del consejo `manifest_signatures.json`
  flujo de trabajo میں بھیجیں تاکہ Disponibilidad del registro de pines کی تشہیر کر سکے۔
- **Negociación de registro** – نئے perfiles رجسٹر کرنے سے پہلے
  [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  دیکھیں۔ آٹومیشن کو ID numéricos کے بجائے identificadores canónicos (`namespace.name@semver`) کو
  ترجیح دینی چاہیے۔
- **Automatización de CI** – اوپر دی گئی کمانڈز کو canalizaciones de lanzamiento میں شامل کریں تاکہ docs،
  accesorios، اور artefactos metadatos firmados کے ساتھ manifiestos deterministas شائع کریں۔