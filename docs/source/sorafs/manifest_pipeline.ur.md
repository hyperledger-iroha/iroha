---
lang: ur
direction: rtl
source: docs/source/sorafs/manifest_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2572648c9c5aa1d4c346e66440fd14bff98afd55232ba1a7ba1c5fcd505559c6
source_last_modified: "2025-11-02T17:57:27.798590+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS chunking → manifest pipeline

یہ نوٹ کم از کم مراحل کو بیان کرتا ہے جو بائٹس والے payload کو Norito-encoded manifest میں
تبدیل کرنے کے لیے ضروری ہیں تاکہ اسے SoraFS رجسٹری میں pin کیا جا سکے۔

1. **payload کو حتمی طور پر chunk کریں**
   - `sorafs_car::CarBuildPlan::single_file` استعمال کریں (یہ اندرونی طور پر SF-1 chunker
     استعمال کرتا ہے) تاکہ chunk offsets، لمبائیاں اور BLAKE3 digests اخذ کیے جا سکیں۔
   - یہ پلان payload digest اور chunk metadata فراہم کرتا ہے جسے downstream tooling CAR
     assembly اور Proof-of-Replication scheduling کے لیے دوبارہ استعمال کر سکتا ہے۔
   - متبادل طور پر، پروٹوٹائپ `sorafs_car::ChunkStore` بائٹس ingest کرتا ہے اور بعد میں CAR
     بنانے کے لیے حتمی chunk metadata ریکارڈ کرتا ہے۔ یہ store اب 64 KiB / 4 KiB PoR
     sampling tree (domain-tagged، chunk-aligned) اخذ کرتا ہے تاکہ schedulers Merkle proofs
     بغیر payload دوبارہ پڑھے مانگ سکیں۔
    `--por-proof=<chunk>:<segment>:<leaf>` استعمال کریں تاکہ sampled leaf کے لیے JSON witness
    نکلے، اور `--por-json-out` کے ذریعے root digest snapshot لکھا جائے۔ `--por-proof` کو
    `--por-proof-out=path` کے ساتھ جوڑیں تاکہ witness محفوظ ہو، اور `--por-proof-verify=path`
    سے تصدیق کریں کہ موجودہ proof موجودہ payload کے لیے حساب شدہ `por_root_hex` سے میل کھاتا ہے۔
    متعدد leaves کے لیے، `--por-sample=<count>` (اختیاری `--por-sample-seed` اور
    `--por-sample-out` کے ساتھ) حتمی samples بناتا ہے اور جب درخواست دستیاب leaves سے بڑھ جائے تو
    `por_samples_truncated=true` سیٹ کرتا ہے۔
   - اگر آپ bundle proofs (CAR manifests، PoR schedules) بنانا چاہتے ہیں تو chunk offsets/lengths/
     digests محفوظ رکھیں۔
   - مستند registry entries اور مذاکراتی رہنمائی کے لیے [`sorafs/chunker_registry.md`](chunker_registry.md)
     دیکھیں۔

2. **manifest لپیٹیں**
   - chunking metadata، root CID، CAR commitments، pin policy، alias claims اور governance signatures
     کو `sorafs_manifest::ManifestBuilder` میں فیڈ کریں۔
   - Norito bytes حاصل کرنے کے لیے `ManifestV1::encode` اور Pin Registry میں ریکارڈ ہونے والا
     canonical digest حاصل کرنے کے لیے `ManifestV1::digest` کال کریں۔

3. **اشاعت**
   - governance (council signature، alias proofs) کے ذریعے manifest digest جمع کریں اور
     deterministic pipeline کے ساتھ manifest bytes کو SoraFS میں pin کریں۔
   - یہ یقینی بنائیں کہ manifest میں حوالہ دی گئی CAR فائل (اور اختیاری CAR index) اسی SoraFS
     pin set میں محفوظ ہے۔

### CLI quickstart

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub   ./docs.tar   --root-cid=0155aa   --car-cid=017112...   --alias-file=docs:sora:alias_proof.bin   --council-signature-file=0123...cafe:council.sig   --metadata=build:ci-123   --manifest-out=docs.manifest   --manifest-signatures-out=docs.manifest.signatures.json   --car-out=docs.car   --json-out=docs.report.json
```

یہ کمانڈ chunk digests اور manifest کی تفصیلات پرنٹ کرتی ہے؛ جب `--manifest-out` اور/یا
`--car-out` فراہم ہوں تو یہ Norito payload اور spec-compliant CARv2 archive (pragma + header +
CARv1 blocks + Multihash index) ڈسک پر لکھتی ہے۔ اگر آپ directory path دیں تو ٹول اسے
recursive طور پر (lexicographic order میں) چلتا ہے، ہر فائل کو chunk کرتا ہے اور directory-rooted
dag-cbor tree بناتا ہے جس کا CID manifest اور CAR دونوں کا root بنتا ہے۔ JSON رپورٹ میں
CAR payload digest، مکمل archive digest، size، raw CID اور root (dag-cbor codec کو الگ کر کے)
شامل ہوتے ہیں، ساتھ ہی manifest کی alias/metadata entries بھی ہوتی ہیں۔ CI رنز میں root یا codec
کی *تصدیق* کے لیے `--root-cid`/`--dag-codec` استعمال کریں، payload hash نافذ کرنے کے لیے
`--car-digest`، پہلے سے حساب شدہ raw CAR identifier (CIDv1، `raw` codec، BLAKE3 multihash)
نافذ کرنے کے لیے `--car-cid`، اور پرنٹ کیا گیا JSON manifest/CAR artifacts کے ساتھ محفوظ کرنے
کے لیے `--json-out` استعمال کریں تاکہ downstream automation کے لیے دستیاب رہے۔

جب `--manifest-signatures-out` فراہم ہو (اور کم از کم ایک `--council-signature*` flag موجود ہو)،
ٹول `manifest_signatures.json` envelope بھی لکھتا ہے جس میں manifest BLAKE3 digest، aggregated
chunk plan SHA3-256 digest (offsets، lengths اور chunk BLAKE3 digests) اور council signatures شامل
ہوتے ہیں۔ یہ envelope اب chunker profile کو canonical `namespace.name@semver` فارم میں ریکارڈ کرتا ہے؛
اس envelope کو governance logs میں شائع کر سکتی ہے یا manifest/CAR artifacts کے ساتھ distribute کر سکتی ہے۔
جب آپ کو کسی بیرونی signer سے envelope ملے تو `--manifest-signatures-in=<path>` شامل کریں تاکہ CLI
digests کی تصدیق کرے اور ہر Ed25519 signature کو تازہ حساب شدہ manifest digest کے خلاف verify کرے۔

جب متعدد chunker profiles رجسٹر ہوں تو آپ `--chunker-profile-id=<id>` کے ذریعے کسی ایک کو واضح طور پر
منتخب کر سکتے ہیں۔ یہ flag [`chunker_registry`](chunker_registry.md) کے عددی identifiers سے میپ ہوتا ہے
اور یقینی بناتا ہے کہ chunking pass اور جاری شدہ manifest دونوں اسی `(namespace, name, semver)` tuple کو
refer کریں۔ Automation میں canonical handle فارم (`--chunker-profile=sorafs.sf1@1.0.0`) کو ترجیح دیں تاکہ
عددی IDs hard-code نہ ہوں۔ موجودہ registry entries دیکھنے کے لیے `sorafs_manifest_chunk_store --list-profiles`
چلائیں (آؤٹ پٹ `sorafs_manifest_chunk_store` کے فراہم کردہ listing سے میل کھاتا ہے)، یا registry اپڈیٹ کی تیاری
کے لیے `--promote-profile=<handle>` سے canonical handle اور alias metadata ایکسپورٹ کریں۔

آڈیٹرز `--por-json-out=path` کے ذریعے مکمل Proof-of-Retrievability tree حاصل کر سکتے ہیں، جو sampling
verification کے لیے chunk/segment/leaf digests کو serialize کرتا ہے۔ انفرادی witnesses کو
`--por-proof=<chunk>:<segment>:<leaf>` سے نکالا جا سکتا ہے (اور `--por-proof-verify=path` سے verify کیا جا سکتا
ہے)، جبکہ `--por-sample=<count>` deterministic اور de-duplicated samples تیار کرتا ہے۔

کوئی بھی flag جو JSON لکھتا ہے (`--json-out`, `--chunk-fetch-plan-out`, `--por-json-out`, وغیرہ) `-` کو path کے
طور پر قبول کرتا ہے، جس سے آپ temporary files بنائے بغیر payload کو براہِ راست stdout پر stream کر سکتے ہیں۔

`--chunk-fetch-plan-out=path` استعمال کریں تاکہ ordered chunk fetch specification (chunk index، payload offset، length،
BLAKE3 digest) محفوظ ہو جو manifest plan کے ساتھ آتی ہے۔ Multi-source clients نتیجے میں آنے والے JSON کو براہ راست
SoraFS fetch orchestrator میں feed کر سکتے ہیں بغیر source payload دوبارہ پڑھے۔ CLI کے پرنٹ کردہ JSON رپورٹ میں یہ
array `chunk_fetch_specs` کے تحت بھی شامل ہوتا ہے۔ `chunking` سیکشن اور `manifest` object دونوں canonical `profile`
برقرار رکھ سکیں۔

stub دوبارہ چلانے پر (مثلاً CI یا release pipeline میں) آپ `--plan=chunk_fetch_specs.json` یا `--plan=-` پاس کر سکتے ہیں
تاکہ پہلے سے بنائی گئی specification دوبارہ امپورٹ ہو سکے۔ CLI اس بات کی تصدیق کرتی ہے کہ ہر chunk کا index، offset، length اور
BLAKE3 digest نئے نکالے گئے CAR plan سے اب بھی میل کھاتا ہے، پھر ingestion جاری رکھتی ہے، جس سے stale یا tampered plans سے بچاؤ ہوتا ہے۔

### مقامی orchestration smoke-test

`sorafs_car` crate اب `sorafs-fetch` فراہم کرتا ہے، جو developer CLI ہے اور `chunk_fetch_specs` array کو استعمال کرتے ہوئے
local files سے multi-provider retrieval سمیولیٹ کرتا ہے۔ اسے `--chunk-fetch-plan-out` سے نکلے JSON پر پوائنٹ کریں، ایک یا
زیادہ provider payload paths دیں (اختیاری طور پر `#N` کے ساتھ تاکہ concurrency بڑھے)، اور یہ chunks verify کرے گا، payload دوبارہ
assemble کرے گا، اور JSON رپورٹ چھاپے گا جو provider success/failure counts اور ہر chunk کے receipts کو خلاصہ کرتا ہے:

```
cargo run -p sorafs_car --bin sorafs_fetch --   --plan=chunk_fetch_specs.json   --provider=alpha=./providers/alpha.bin   --provider=beta=./providers/beta.bin#4@3   --output=assembled.bin   --json-out=fetch_report.json   --provider-metrics-out=providers.json   --scoreboard-out=scoreboard.json
```

اس فلو کو orchestrator کے رویّے کی توثیق یا provider payloads کا موازنہ کرنے کے لیے استعمال کریں،
اس سے پہلے کہ SoraFS node میں حقیقی network transports جوڑے جائیں۔

جب آپ کو local files کے بجائے live Torii gateway ہٹ کرنا ہو تو `--provider=/path` flags کو نئے
HTTP-oriented options سے بدل دیں:

```
sorafs-fetch   --plan=chunk_fetch_specs.json   --gateway-provider=name=gw-a,provider-id=<hex>,base-url=https://gw-a.example/,stream-token=<base64>   --gateway-manifest-id=<manifest_id_hex>   --gateway-chunker-handle=sorafs.sf1@1.0.0   --gateway-client-id=ci-orchestrator   --json-out=gateway_fetch_report.json
```

CLI stream token کی تصدیق کرتی ہے، chunker/profile alignment نافذ کرتی ہے، اور gateway metadata کو معمول کے provider receipts کے
ساتھ ریکارڈ کرتی ہے تاکہ آپریٹرز رپورٹ کو rollout evidence کے طور پر محفوظ کر سکیں (مکمل blue/green فلو کے لیے deployment handbook دیکھیں)۔

اگر آپ `--provider-advert=name=/path/to/advert.to` پاس کریں تو CLI اب Norito envelope decode کرتی ہے، Ed25519 signature verify کرتی ہے
اور یہ نافذ کرتی ہے کہ provider `chunk_range_fetch` capability advertise کرے۔ اس سے multi-source fetch simulation governance admission

`#N` suffix provider کی concurrency حد بڑھاتا ہے، جبکہ `@W` scheduling weight سیٹ کرتا ہے (ڈیفالٹ 1 جب omit ہو)۔ جب adverts یا gateway
descriptors فراہم ہوں تو CLI fetch شروع کرنے سے پہلے orchestrator scoreboard evaluate کرتی ہے: اہل providers telemetry-aware weights لیتے ہیں
اور JSON snapshot `--scoreboard-out=<path>` کے ذریعے محفوظ ہو جاتا ہے۔ جو providers capability checks یا governance deadlines میں fail ہوں انہیں
خودکار طور پر warning کے ساتھ drop کر دیا جاتا ہے تاکہ رنز admission policy کے مطابق رہیں۔ مثال کے لیے
`docs/examples/sorafs_ci_sample/{telemetry.sample.json,scoreboard.json}` دیکھیں۔

`--expect-payload-digest=<hex>` اور/یا `--expect-payload-len=<bytes>` پاس کریں تاکہ assembled payload لکھنے سے پہلے manifest expectations سے
مطابقت کی تصدیق ہو جائے — یہ CI smoke-tests کے لیے مفید ہے جو یہ یقینی بنانا چاہتے ہیں کہ orchestrator نے خاموشی سے chunks drop یا reorder نہیں کیے۔

اگر آپ کے پاس پہلے سے `sorafs-manifest-stub` سے بنا ہوا JSON رپورٹ موجود ہو تو اسے `--manifest-report=docs.report.json` کے ذریعے براہ راست پاس کریں۔
fetch CLI embedded `chunk_fetch_specs`, `payload_digest_hex`, اور `payload_len` فیلڈز کو reuse کرتی ہے، اس لیے الگ plan یا validation فائلیں
سنبھالنے کی ضرورت نہیں رہتی۔

fetch رپورٹ aggregated telemetry بھی دکھاتی ہے تاکہ monitoring آسان ہو:
`chunk_retry_total`, `chunk_retry_rate`, `chunk_attempt_total`,
`chunk_attempt_average`, `provider_success_total`, `provider_failure_total`,
`provider_failure_rate`, اور `provider_disabled_total` fetch سیشن کی مجموعی صحت کو ظاہر کرتے ہیں اور Grafana/Loki dashboards یا CI assertions کے
لیے موزوں ہیں۔ `--provider-metrics-out` استعمال کریں تاکہ اگر downstream tooling کو صرف provider-level stats چاہییں تو صرف `provider_reports` array لکھا جائے۔

### اگلے اقدامات

- CAR metadata کو manifest digests کے ساتھ governance logs میں محفوظ کریں تاکہ observers payload دوبارہ ڈاؤن لوڈ کیے بغیر CAR contents کی توثیق کر سکیں۔
- manifest اور CAR publishing فلو کو CI میں شامل کریں تاکہ ہر docs/artefacts build خودکار طور پر manifest بنائے، signatures حاصل کرے، اور نتیجے میں آنے والے payloads pin کرے۔
