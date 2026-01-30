---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/node-storage.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 92404cf2a5f7c3ed911666e55902f7922c2592dcc2cc37e9914443010900816a
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
id: node-storage
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/node-storage.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note مستند ماخذ
:::

## SoraFS node storage design (Draft)

یہ نوٹ واضح کرتا ہے کہ Iroha (Torii) node کس طرح SoraFS data availability layer میں opt-in کر سکتا ہے اور local disk کا ایک حصہ chunks ذخیرہ/سرو کرنے کے لیے مختص کر سکتا ہے۔ یہ `sorafs_node_client_protocol.md` discovery spec اور SF-1b fixture کام کی تکمیل کرتا ہے، اور storage-side architecture، resource controls، اور configuration plumbing بیان کرتا ہے جو node اور gateway code paths میں شامل ہونا ضروری ہے۔ عملی آپریشنل drills
[Node Operations Runbook](./node-operations) میں موجود ہیں۔

### Goals

- کسی بھی validator یا auxiliary Iroha process کو spare disk بطور SoraFS provider expose کرنے دینا بغیر core ledger ذمہ داریاں متاثر کیے۔
- storage module کو deterministic اور Norito-driven رکھنا: manifests، chunk plans، Proof-of-Retrievability (PoR) roots، اور provider adverts ہی source of truth ہیں۔
- operator-defined quotas enforce کرنا تاکہ node بہت زیادہ pin/fetch requests قبول کر کے اپنے resources ختم نہ کر لے۔
- health/telemetry (PoR sampling، chunk fetch latency، disk pressure) کو governance اور clients تک واپس پہنچانا۔

### High-level architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Key modules:

- **Gateway**: Norito HTTP endpoints expose کرتا ہے pin proposals، chunk fetch requests، PoR sampling اور telemetry کے لیے۔ Norito payloads validate کرتا ہے اور requests کو chunk store میں marshal کرتا ہے۔ نیا daemon بنانے کے بجائے Torii HTTP stack reuse کرتا ہے۔
- **Pin Registry**: manifest pin state جو `iroha_data_model::sorafs` اور `iroha_core` میں track ہوتی ہے۔ Manifest accept ہونے پر registry manifest digest، chunk plan digest، PoR root اور provider capability flags record کرتا ہے۔
- **Chunk Storage**: disk-backed `ChunkStore` implementation جو signed manifests ingest کرتی ہے، `ChunkProfile::DEFAULT` سے chunk plans materialize کرتی ہے، اور chunks کو deterministic layout میں persist کرتی ہے۔ ہر chunk content fingerprint اور PoR metadata کے ساتھ associate ہوتا ہے تاکہ sampling بغیر پورا فائل دوبارہ پڑھنے کے re-validate ہو سکے۔
- **Quota/Scheduler**: operator-configured limits (max disk bytes، max outstanding pins، max parallel fetches، chunk TTL) enforce کرتا ہے اور IO کو coordinate کرتا ہے تاکہ node کے ledger duties starve نہ ہوں۔ Scheduler PoR proofs اور sampling requests کو bounded CPU کے ساتھ serve بھی کرتا ہے۔

### Configuration

`iroha_config` میں نیا section شامل کریں:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # optional human friendly tag
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: participation toggle۔ false ہو تو gateway storage endpoints پر 503 دیتا ہے اور node discovery میں advertise نہیں کرتا۔
- `data_dir`: chunk data، PoR trees اور fetch telemetry کے لیے root directory۔ Default `<iroha.data_dir>/sorafs` ہے۔
- `max_capacity_bytes`: pinned chunk data کے لیے hard limit۔ background task limit پر پہنچنے پر نئے pins reject کرتی ہے۔
- `max_parallel_fetches`: scheduler کا concurrency cap جو bandwidth/disk IO کو validator workload کے ساتھ balance کرتا ہے۔
- `max_pins`: manifest pins کی maximum تعداد جو node قبول کرتا ہے اس سے پہلے eviction/back pressure apply ہو۔
- `por_sample_interval_secs`: automatic PoR sampling jobs کی cadence۔ ہر job `N` leaves sample کرتا ہے (per manifest configurable) اور telemetry events emit کرتا ہے۔ Governance `profile.sample_multiplier` metadata key (integer `1-4`) سے `N` کو deterministically scale کر سکتی ہے۔ Value ایک single number/string یا per-profile overrides والا object ہو سکتا ہے، مثلاً `{"default":2,"sorafs.sf2@1.0.0":3}`۔
- `adverts`: structure جو provider advert generator `ProviderAdvertV1` fields (stake pointer, QoS hints, topics) بھرنے کے لیے استعمال کرتا ہے۔ اگر omit ہو تو node governance registry defaults استعمال کرتا ہے۔

Config plumbing:

- `[sorafs.storage]` `iroha_config` میں `SorafsStorage` کے طور پر define ہے اور node config file سے load ہوتا ہے۔
- `iroha_core` اور `iroha_torii` storage config کو startup پر gateway builder اور chunk store میں thread کرتے ہیں۔
- Dev/test env overrides موجود ہیں (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`)، مگر production deployments کو config file پر rely کرنا چاہیے۔

### CLI utilities

جب تک Torii کی HTTP surface wire ہو رہی ہے، `sorafs_node` crate ایک thin CLI فراہم کرتا ہے تاکہ operators persistent backend کے خلاف ingestion/export drills script کر سکیں۔【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` Norito-encoded manifest `.to` اور matching payload bytes expect کرتا ہے۔ یہ manifest کے chunking profile سے chunk plan rebuild کرتا ہے، digest parity enforce کرتا ہے، chunk files persist کرتا ہے، اور optionally `chunk_fetch_specs` JSON blob emit کرتا ہے تاکہ downstream tooling layout sanity-check کر سکے۔
- `export` manifest ID قبول کرتا ہے اور stored manifest/payload کو disk پر لکھتا ہے (optional plan JSON کے ساتھ) تاکہ fixtures environments میں reproducible رہیں۔

دونوں commands stdout پر Norito JSON summary پرنٹ کرتے ہیں، جسے scripts میں pipe کرنا آسان ہے۔ CLI integration test سے covered ہے تاکہ manifests/payloads round-trip درست رہیں جب تک Torii APIs نہ آئیں۔【crates/sorafs_node/tests/cli.rs:1】

> HTTP parity
>
> Torii gateway اب read-only helpers expose کرتا ہے جو اسی `NodeHandle` پر based ہیں:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — stored Norito manifest (base64) digest/metadata کے ساتھ واپس کرتا ہے۔【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — deterministic chunk plan JSON (`chunk_fetch_specs`) downstream tooling کے لیے واپس کرتا ہے۔【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> یہ endpoints CLI output کو mirror کرتے ہیں تاکہ pipelines local scripts سے HTTP probes پر بغیر parser بدلے منتقل ہو سکیں۔【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Node lifecycle

1. **Startup**:
   - storage enabled ہو تو node configured directory اور capacity کے ساتھ chunk store initialize کرتا ہے۔ اس میں PoR manifest database verify/create کرنا اور pinned manifests replay کر کے caches warm کرنا شامل ہے۔
   - SoraFS gateway routes register کریں (Norito JSON POST/GET endpoints for pin, fetch, PoR sample, telemetry)۔
   - PoR sampling worker اور quota monitor spawn کریں۔
2. **Discovery / Adverts**:
3. **Pin workflow**:
   - Gateway signed manifest وصول کرتا ہے (chunk plan، PoR root، council signatures شامل)۔ alias list validate کریں (`sorafs.sf1@1.0.0` required) اور chunk plan کو manifest metadata سے match کریں۔
   - Quotas check کریں۔ اگر capacity/pin limits exceed ہوں تو policy error (structured Norito) دیں۔
   - Chunk data کو `ChunkStore` میں stream کریں اور ingest کے دوران digests verify کریں۔ PoR trees update کریں اور manifest metadata registry میں store کریں۔
4. **Fetch workflow**:
   - Disk سے chunk range requests serve کریں۔ Scheduler `max_parallel_fetches` enforce کرتا ہے اور saturated ہونے پر `429` واپس کرتا ہے۔
   - Structured telemetry (Norito JSON) emit کریں جس میں latency، bytes served اور error counts ہوں تاکہ downstream monitoring ہو۔
5. **PoR sampling**:
   - Worker manifests کو weight (مثلاً stored bytes) کے تناسب سے select کرتا ہے اور chunk store کے PoR tree سے deterministic sampling کرتا ہے۔
   - Results کو governance audits کے لیے persist کریں اور provider adverts/telemetry endpoints میں summaries شامل کریں۔
6. **Eviction / quota enforcement**:
   - Capacity پہنچنے پر node default طور پر نئے pins reject کرتا ہے۔ Optional طور پر operators eviction policies (مثلاً TTL، LRU) configure کر سکتے ہیں جب governance model طے ہو جائے؛ فی الحال design strict quotas اور operator-initiated unpin operations فرض کرتا ہے۔

### Capacity declaration & scheduling integration

- Torii اب `/v1/sorafs/capacity/declare` سے `CapacityDeclarationRecord` updates embedded `CapacityManager` تک relay کرتا ہے، تاکہ ہر node اپنی committed chunker/lane allocations کا in-memory view بنائے۔ Manager telemetry کے لیے read-only snapshots (`GET /v1/sorafs/capacity/state`) expose کرتا ہے اور نئے orders قبول کرنے سے پہلے per-profile یا per-lane reservations enforce کرتا ہے۔【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- `/v1/sorafs/capacity/schedule` endpoint governance-issued `ReplicationOrderV1` payloads قبول کرتا ہے۔ جب order local provider کو target کرے تو manager duplicate scheduling چیک کرتا ہے، chunker/lane capacity verify کرتا ہے، slice reserve کرتا ہے، اور `ReplicationPlan` واپس کرتا ہے جو remaining capacity بیان کرے تاکہ orchestration tooling ingestion جاری رکھ سکے۔ دوسرے providers کے orders کو `ignored` response سے acknowledge کیا جاتا ہے تاکہ multi-operator workflows آسان ہوں۔【crates/iroha_torii/src/routing.rs:4845】
- Completion hooks (مثلاً ingestion کامیاب ہونے کے بعد) `POST /v1/sorafs/capacity/complete` کو hit کرتے ہیں تاکہ `CapacityManager::complete_order` کے ذریعے reservations release ہوں۔ Response میں `ReplicationRelease` snapshot (remaining totals, chunker/lane residuals) شامل ہوتا ہے تاکہ orchestration tooling polling کے بغیر اگلا order queue کر سکے۔ یہ بعد میں chunk store pipeline کے ساتھ wire ہوگا جب ingestion logic land ہو جائے۔【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- Embedded `TelemetryAccumulator` کو `NodeHandle::update_telemetry` کے ذریعے mutate کیا جا سکتا ہے، جس سے background workers PoR/uptime samples record کرتے ہیں اور مستقبل میں canonical `CapacityTelemetryV1` payloads derive کر سکتے ہیں بغیر scheduler internals چھیڑے۔【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integrations & future work

- **Governance**: `sorafs_pin_registry_tracker.md` کو storage telemetry (PoR success rate، disk utilization) کے ساتھ extend کریں۔ Admission policies adverts accept کرنے سے پہلے minimum capacity یا minimum PoR success rate require کر سکتی ہیں۔
- **Client SDKs**: نئی storage config (disk limits، alias) expose کریں تاکہ management tooling programmatically nodes bootstrap کر سکے۔
- **Telemetry**: موجودہ metrics stack (Prometheus / OpenTelemetry) کے ساتھ integrate کریں تاکہ storage metrics observability dashboards میں نظر آئیں۔
- **Security**: storage module کو dedicated async task pool میں back-pressure کے ساتھ چلائیں اور chunk reads کو io_uring یا bounded tokio pools کے ذریعے sandbox کرنے پر غور کریں تاکہ malicious clients resources exhaust نہ کر سکیں۔

یہ design storage module کو optional اور deterministic رکھتا ہے جبکہ operators کو SoraFS data availability layer میں حصہ لینے کے لیے ضروری knobs فراہم کرتا ہے۔ اس پر عمل درآمد `iroha_config`, `iroha_core`, `iroha_torii` اور Norito gateway میں تبدیلیاں، ساتھ ہی provider advert tooling میں تبدیلیاں مانگتا ہے۔
