---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/developer-cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 608725e4b562d3c014d5582b4e50a02abf80e656da246b3c85500cf26a3d59f5
source_last_modified: "2026-01-22T06:58:48+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: developer-cli
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note مستند ماخذ
:::

Consolidated `sorafs_cli` surface (جو `sorafs_car` crate کے ذریعے `cli` feature کے ساتھ فراہم ہوتا ہے) SoraFS artifacts تیار کرنے کے لیے درکار ہر قدم expose کرتا ہے۔ اس cookbook کو عام workflows پر سیدھا جانے کے لیے استعمال کریں؛ operational context کے لیے اسے manifest pipeline اور orchestrator runbooks کے ساتھ pair کریں۔

## Package payloads

Deterministic CAR archives اور chunk plans بنانے کے لیے `car pack` استعمال کریں۔ اگر handle فراہم نہ ہو تو command خودکار طور پر SF-1 chunker منتخب کرتا ہے۔

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Default chunker handle: `sorafs.sf1@1.0.0`.
- Directory inputs lexicographic order میں walk ہوتے ہیں تاکہ checksums مختلف پلیٹ فارمز پر بھی stable رہیں۔
- JSON summary میں payload digests، فی chunk metadata، اور registry/orchestrator کے ذریعے پہچانا گیا root CID شامل ہوتا ہے۔

## Construct manifests

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` options براہ راست `sorafs_manifest::ManifestBuilder` میں `PinPolicy` fields سے map ہوتے ہیں۔
- `--chunk-plan` تب دیں جب آپ چاہتے ہوں کہ CLI submission سے پہلے SHA3 chunk digest دوبارہ compute کرے؛ ورنہ وہ summary میں embed شدہ digest reuse کرتا ہے۔
- JSON output Norito payload کی عکاسی کرتا ہے تاکہ reviews کے دوران diffs سیدھے ہوں۔

## Sign manifests without long-lived keys

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Inline tokens، environment variables یا file-based sources قبول کرتا ہے۔
- Provenance metadata (`token_source`, `token_hash_hex`, chunk digest) شامل کرتا ہے اور raw JWT کو محفوظ نہیں کرتا، جب تک `--include-token=true` نہ ہو۔
- CI میں بہتر کام کرتا ہے: GitHub Actions OIDC کے ساتھ `--identity-token-provider=github-actions` استعمال کریں۔

## Submit manifests to Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority soraカタカナ... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Alias proofs کے لیے Norito decoding کرتا ہے اور Torii کو POST کرنے سے پہلے انہیں manifest digest سے match کرتا ہے۔
- Plan سے chunk SHA3 digest دوبارہ compute کرتا ہے تاکہ mismatch attacks روکے جا سکیں۔
- Response summaries بعد کی auditing کے لیے HTTP status، headers، اور registry payloads محفوظ کرتی ہیں۔

## Verify CAR contents and proofs

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- PoR tree دوبارہ بناتا ہے اور payload digests کو manifest summary کے ساتھ compare کرتا ہے۔
- Replication proofs کو governance میں submit کرتے وقت مطلوبہ counts اور identifiers capture کرتا ہے۔

## Stream proof telemetry

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- ہر streamed proof کے لیے NDJSON items emit کرتا ہے (`--emit-events=false` سے replay بند کریں)۔
- Success/failure counts، latency histograms، اور sampled failures کو summary JSON میں aggregate کرتا ہے تاکہ dashboards logs scrape کیے بغیر نتائج دکھا سکیں۔
- جب gateway failures report کرے یا local PoR verification (`--por-root-hex` کے ذریعے) proofs reject کرے تو non-zero exit دیتا ہے۔ rehearsal runs کے لیے `--max-failures` اور `--max-verification-failures` سے thresholds adjust کریں۔
- آج PoR کو support کرتا ہے؛ PDP اور PoTR SF-13/SF-14 آنے پر اسی envelope کو reuse کریں گے۔
- `--governance-evidence-dir` rendered summary، metadata (timestamp, CLI version, gateway URL, manifest digest)، اور manifest کی ایک copy فراہم کردہ directory میں لکھتا ہے تاکہ governance packets proof-stream evidence کو run دوبارہ کیے بغیر archive کر سکیں۔

## Additional references

- `docs/source/sorafs_cli.md` — تمام flags کی جامع دستاویزات۔
- `docs/source/sorafs_proof_streaming.md` — proof telemetry schema اور Grafana dashboard template۔
- `docs/source/sorafs/manifest_pipeline.md` — chunking، manifest composition، اور CAR handling پر تفصیلی جائزہ۔
