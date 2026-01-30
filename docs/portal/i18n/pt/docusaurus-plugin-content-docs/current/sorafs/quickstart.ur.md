---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SoraFS کا فوری آغاز

یہ عملی رہنمائی SoraFS اسٹوریج پائپ لائن کی بنیاد بننے والے
deterministic SF-1 چنکر پروفائل، مینی فیسٹ سائننگ، اور متعدد فراہم کنندگان سے
fetch فلو پر لے جاتی ہے۔ ڈیزائن نوٹس اور CLI فلیگ ریفرنس کے لیے اسے
[manifest pipeline کی تفصیلی وضاحت](manifest-pipeline.md) کے ساتھ استعمال کریں۔

## ضروریات

- Rust ٹول چین (`rustup update`)، اور workspace لوکل طور پر کلون ہو۔
- اختیاری: [OpenSSL کے مطابق Ed25519 keypair](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  مینی فیسٹ سائن کرنے کے لیے۔
- اختیاری: Node.js ≥ 18 اگر آپ Docusaurus پورٹل کا پیش نظارہ کرنا چاہتے ہیں۔

`export RUST_LOG=info` سیٹ کریں تاکہ تجربات کے دوران مفید CLI پیغامات سامنے آئیں۔

## 1. deterministic fixtures تازہ کریں

SF-1 کے canonical chunking vectors دوبارہ تیار کریں۔ جب `--signing-key` فراہم کیا
جائے تو یہ کمانڈ signed manifest envelopes بھی بناتی ہے؛ `--allow-unsigned` صرف
مقامی ڈیولپمنٹ میں استعمال کریں۔

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

آؤٹ پٹ:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (اگر سائن ہوا ہو)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. payload کو chunk کریں اور پلان دیکھیں

`sorafs_chunker` سے کسی بھی فائل یا آرکائیو کو chunk کریں:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

اہم فیلڈز:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` کے پیرامیٹرز کی تصدیق۔
- `chunks[]` – ترتیب وار offsets، lengths، اور chunk BLAKE3 digests۔

بڑے fixtures کے لیے، proptest پر مبنی regression چلائیں تاکہ streaming اور batch
chunking ہم آہنگ رہے:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. manifest بنائیں اور سائن کریں

chunk plan، aliases، اور governance signatures کو `sorafs-manifest-stub` کے ذریعے
manifest میں لپیٹیں۔ نیچے دی گئی کمانڈ single-file payload دکھاتی ہے؛ درخت پیک کرنے
کے لیے directory path دیں (CLI اسے lexicographic ترتیب میں چلتا ہے)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` میں دیکھیں:

- `chunking.chunk_digest_sha3_256` – offsets/lengths کا SHA3 digest، chunker fixtures کے مطابق۔
- `manifest.manifest_blake3` – manifest envelope میں سائن کیا گیا BLAKE3 digest۔
- `chunk_fetch_specs[]` – orchestrators کے لیے ترتیب وار fetch ہدایات۔

جب حقیقی signatures دینے کے لیے تیار ہوں تو `--signing-key` اور `--signer` arguments شامل
کریں۔ کمانڈ envelope لکھنے سے پہلے ہر Ed25519 signature کی توثیق کرتی ہے۔

## 4. multi-provider retrieval کی سمیولیشن

developer fetch CLI سے chunk plan کو ایک یا زیادہ providers کے خلاف replay کریں۔ یہ CI
smoke tests اور orchestrator prototyping کے لیے بہترین ہے۔

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

توثیقات:

- `payload_digest_hex` کو manifest رپورٹ سے ملنا چاہیے۔
- `provider_reports[]` ہر provider کے لیے success/failure counts دکھاتا ہے۔
- غیر صفر `chunk_retry_total` back-pressure ایڈجسٹمنٹ دکھاتا ہے۔
- `--max-peers=<n>` کے ذریعے run میں شیڈول ہونے والے providers کی تعداد محدود کریں اور CI
  simulations کو اہم امیدواروں پر مرکوز رکھیں۔
- `--retry-budget=<n>` per-chunk retry count (3) کو override کرتا ہے تاکہ failures inject
  کرنے پر orchestrator regressions جلد ظاہر ہوں۔

`--expect-payload-digest=<hex>` اور `--expect-payload-len=<bytes>` شامل کریں تاکہ reconstructed
payload اگر manifest سے ہٹے تو فوراً fail ہو جائے۔

## 5. اگلے اقدامات

- **Governance integration** – manifest digest اور `manifest_signatures.json` کو council
  workflow میں بھیجیں تاکہ Pin Registry availability کی تشہیر کر سکے۔
- **Registry negotiation** – نئے profiles رجسٹر کرنے سے پہلے
  [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  دیکھیں۔ آٹومیشن کو numeric IDs کے بجائے canonical handles (`namespace.name@semver`) کو
  ترجیح دینی چاہیے۔
- **CI automation** – اوپر دی گئی کمانڈز کو release pipelines میں شامل کریں تاکہ docs،
  fixtures، اور artifacts signed metadata کے ساتھ deterministic manifests شائع کریں۔
