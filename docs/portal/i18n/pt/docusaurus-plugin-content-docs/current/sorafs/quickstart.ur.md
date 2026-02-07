---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS کا فوری آغاز

یہ عملی رہنمائی SoraFS اسٹوریج پائپ لائن کی بنیاد بننے والے
SF-1 determinístico.
buscar فلو پر لے جاتی ہے۔ ڈیزائن نوٹس اور CLI فلیگ ریفرنس کے لیے اسے
[pipeline manifesto کی تفصیلی وضاحت](manifest-pipeline.md) کے ساتھ استعمال کریں۔

## ضروریات

- Ferrugem na área de trabalho (`rustup update`), no espaço de trabalho que está na área de trabalho
- Par de chaves: [par de chaves OpenSSL کے مطابق Ed25519](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  مینی فیسٹ سائن کرنے کے لیے۔
- Configuração: Node.js ≥ 18 اگر آپ Docusaurus پورٹل کا پیش نظارہ کرنا چاہتے ہیں۔

`export RUST_LOG=info` سیٹ کریں تاکہ تجربات کے دوران مفید CLI پیغامات سامنے آئیں۔

## 1. luminárias determinísticas

SF-1 é um vetor de chunking canônico جب `--signing-key` فراہم کیا
جائے تو یہ کمانڈ envelopes de manifesto assinados بھی بناتی ہے؛ `--allow-unsigned` صرف
مقامی ڈیولپمنٹ میں استعمال کریں۔

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Então:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (`fixtures/sorafs_chunker/manifest_signatures.json`)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. carga útil کو chunk کریں اور پلان دیکھیں

`sorafs_chunker` سے کسی بھی فائل یا آرکائیو کو chunk کریں:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

O que fazer:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` کے پیرامیٹرز کی تصدیق۔
- `chunks[]` – ترتیب وار offsets, comprimentos, اور chunk BLAKE3 digests۔

بڑے fixtures کے لیے، proptest پر مبنی regressão چلائیں تاکہ streaming اور batch
chunking ہم آہنگ رہے:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. manifesto بنائیں اور سائن کریں

plano de bloco, aliases, assinaturas de governança e `sorafs-manifest-stub` کے ذریعے
manifesto نیچے دی گئی کمانڈ carga útil de arquivo único دکھاتی ہے؛ درخت پیک کرنے
کے لیے caminho do diretório دیں (CLI اسے lexicográfico ترتیب میں چلتا ہے)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` Nota:

- `chunking.chunk_digest_sha3_256` – deslocamentos/comprimentos کا SHA3 digest, chunker fixtures کے مطابق۔
- `manifest.manifest_blake3` – envelope de manifesto میں سائن کیا گیا BLAKE3 digest۔
- `chunk_fetch_specs[]` – orquestradores کے لیے ترتیب وار fetch ہدایات۔

جب حقیقی assinaturas دینے کے لیے تیار ہوں تو `--signing-key` e `--signer` argumentos شامل
کریں۔ Envelope کمانڈ لکھنے سے پہلے ہر Assinatura Ed25519 کی توثیق کرتی ہے۔

## 4. recuperação de vários provedores

desenvolvedor buscar CLI سے plano de bloco کو ایک یا زیادہ provedores کے خلاف replay کریں۔ یہ CI
testes de fumaça e prototipagem de orquestrador کے لیے بہترین ہے۔

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Descrição:

- `payload_digest_hex` کو manifesto رپورٹ سے ملنا چاہیے۔
- `provider_reports[]` ہر provedor کے لیے contagens de sucesso/falha دکھاتا ہے۔
- غیر صفر `chunk_retry_total` contrapressão ایڈجسٹمنٹ دکھاتا ہے۔
- `--max-peers=<n>` کے ذریعے run میں شیڈول ہونے والے provedores کی تعداد محدود کریں اور CI
  simulações کو اہم امیدواروں پر مرکوز رکھیں۔
- `--retry-budget=<n>` contagem de novas tentativas por bloco (3) کو substituir کرتا ہے تاکہ falhas injetadas
  کرنے پر regressões do orquestrador جلد ظاہر ہوں۔

`--expect-payload-digest=<hex>` e `--expect-payload-len=<bytes>` de construção reconstruída
payload اگر manifest سے ہٹے تو فوراً fail ہو جائے۔

## 5. اگلے اقدامات- **Integração de governança** – resumo do manifesto em `manifest_signatures.json` e conselho
  fluxo de trabalho میں بھیجیں تاکہ Disponibilidade do registro de pinos کی تشہیر کر سکے۔
- **Negociação de registro** – Perfis نئے رجسٹر کرنے سے پہلے
  [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  دیکھیں۔ Existem IDs numéricos e identificadores canônicos (`namespace.name@semver`) e
  ترجیح دینی چاہیے۔
- **Automação de CI** – اوپر دی گئی کمانڈز کو pipelines de liberação میں شامل کریں تاکہ docs،
  fixtures, اور artefatos metadados assinados کے ساتھ manifestos determinísticos شائع کریں۔