---
lang: ur
direction: rtl
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
ہونے تک دونوں ورژنز کو sync رکھیں۔
:::

# Sora Nexus Data Availability Ingest Plan

_مسودہ: 2026-02-20 -- مالک: Core Protocol WG / Storage Team / DA WG_

DA-2 ورک اسٹریم Torii میں ایک blob ingest API شامل کرتا ہے جو Norito میٹاڈیٹا
جاری کرتی ہے اور SoraFS ریپلیکیشن کو seed کرتی ہے۔ یہ دستاویز مجوزہ schema،
API surface، اور validation flow کو بیان کرتی ہے تاکہ implementation باقی
simulations (DA-1 follow-ups) پر رکے بغیر آگے بڑھے۔ تمام payload formats کو
Norito codecs استعمال کرنا لازمی ہے؛ serde/JSON fallback کی اجازت نہیں۔

## اہداف

- بڑے blobs (Taikai segments، lane sidecars، governance artefacts) کو Torii کے
  ذریعے deterministically قبول کرنا۔
- blob، codec parameters، erasure profile، اور retention policy کو بیان کرنے
  والے canonical Norito manifests تیار کرنا۔
- chunk metadata کو SoraFS hot storage میں محفوظ کرنا اور replication jobs کو
  enqueue کرنا۔
- pin intents + policy tags کو SoraFS registry اور governance observers تک
  publish کرنا۔
- admission receipts فراہم کرنا تاکہ clients کو deterministic proof of
  publication مل سکے۔

## API Surface (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Payload ایک Norito-encoded `DaIngestRequest` ہے۔ Responses `application/norito+v1`
استعمال کرتی ہیں اور `DaIngestReceipt` واپس کرتی ہیں۔

| Response | مطلب |
| --- | --- |
| 202 Accepted | Blob کو chunking/replication کیلئے queue کیا گیا؛ receipt واپس۔ |
| 400 Bad Request | Schema/size violation (validation checks دیکھیں)۔ |
| 401 Unauthorized | API token موجود نہیں/غلط۔ |
| 409 Conflict | `client_blob_id` ڈپلیکیٹ ہے اور metadata مختلف ہے۔ |
| 413 Payload Too Large | Configured blob length limit سے تجاوز۔ |
| 429 Too Many Requests | Rate limit hit۔ |
| 500 Internal Error | غیر متوقع failure (log + alert)۔ |

## Proposed Norito Schema

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> Implementation note: ان payloads کی canonical Rust representations اب
> `iroha_data_model::da::types` کے تحت ہیں، request/receipt wrappers
> `iroha_data_model::da::ingest` میں، اور manifest structure
> `iroha_data_model::da::manifest` میں ہے۔

`compression` field بتاتا ہے کہ callers نے payload کیسے تیار کیا۔ Torii
`identity`, `gzip`, `deflate`, اور `zstd` قبول کرتا ہے اور hashing, chunking اور
optional manifests verify کرنے سے پہلے bytes کو decompress کرتا ہے۔

### Validation Checklist

1. Verify کریں کہ request کا Norito header `DaIngestRequest` سے match کرتا ہے۔
2. اگر `total_size` canonical (decompressed) payload length سے مختلف ہو یا
   configured max سے زیادہ ہو تو fail کریں۔
3. `chunk_size` alignment enforce کریں (power-of-two, <= 2 MiB)۔
4. `data_shards + parity_shards` <= global maximum اور parity >= 2 یقینی بنائیں۔
5. `retention_policy.required_replica_count` کو governance baseline respect کرنا ہوگا۔
6. Canonical hash کے خلاف signature verification (signature field کے بغیر)۔
7. `client_blob_id` duplicates reject کریں جب تک payload hash + metadata identical نہ ہوں۔
8. اگر `norito_manifest` دیا گیا ہو تو schema + hash کو chunking کے بعد
   recalculated manifest سے match کریں؛ ورنہ node manifest generate کر کے store کرے۔
9. Configured replication policy enforce کریں: Torii `RetentionPolicy` کو
   `torii.da_ingest.replication_policy` سے rewrite کرتا ہے ( `replication-policy.md`
   دیکھیں ) اور pre-built manifests کو reject کرتا ہے اگر retention metadata
   enforced profile سے match نہ کرے۔

### Chunking & Replication Flow

1. Payload کو `chunk_size` میں تقسیم کریں، ہر chunk پر BLAKE3 اور Merkle root حساب کریں۔
2. Norito `DaManifestV1` (نیا struct) بنائیں جو chunk commitments (role/group_id)،
   erasure layout (row/column parity counts + `ipa_commitment`)، retention policy،
   اور metadata کو capture کرے۔
3. Canonical manifest bytes کو `config.da_ingest.manifest_store_dir` کے تحت queue کریں
   (Torii `manifest.encoded` files lane/epoch/sequence/ticket/fingerprint کے لحاظ سے لکھتا ہے)
   تاکہ SoraFS orchestration انہیں ingest کر کے storage ticket کو persisted data سے link کرے۔
4. `sorafs_car::PinIntent` کے ذریعے governance tag + policy کے ساتھ pin intents publish کریں۔
5. Norito event `DaIngestPublished` emit کریں تاکہ observers (light clients, governance, analytics)
   کو اطلاع ملے۔
6. `DaIngestReceipt` caller کو واپس دیں (Torii DA service key سے signed) اور
   `Sora-PDP-Commitment` header emit کریں تاکہ SDKs فوری طور پر commitment capture کر سکیں۔
   Receipt میں اب `rent_quote` (Norito `DaRentQuote`) اور `stripe_layout` شامل ہیں،
   جس سے submitters base rent، reserve share، PDP/PoTR bonus expectations اور 2D
   erasure layout کو storage ticket کے ساتھ funds commit کرنے سے پہلے دکھا سکتے ہیں۔

## Storage / Registry Updates

- `sorafs_manifest` کو `DaManifestV1` کے ساتھ extend کریں تاکہ deterministic parsing ہو سکے۔
- نیا registry stream `da.pin_intent` شامل کریں جس کا versioned payload
  manifest hash + ticket id کو reference کرتا ہے۔
- Observability pipelines کو ingest latency، chunking throughput، replication backlog
  اور failure counts ٹریک کرنے کیلئے اپ ڈیٹ کریں۔

## Testing Strategy

- Schema validation، signature checks، duplicate detection کیلئے unit tests۔
- Norito encoding کے golden tests (`DaIngestRequest`, manifest, receipt)۔
- Integration harness جو mock SoraFS + registry چلاتا ہے اور chunk + pin flows verify کرتا ہے۔
- Property tests جو random erasure profiles اور retention combinations cover کرتے ہیں۔
- Norito payload fuzzing تاکہ malformed metadata سے بچاؤ ہو۔

## CLI & SDK Tooling (DA-8)

- `iroha app da submit` (نیا CLI entrypoint) اب shared ingest builder/publisher کو wrap کرتا ہے تاکہ
  operators Taikai bundle flow کے باہر arbitrary blobs ingest کر سکیں۔ یہ کمانڈ
  `crates/iroha_cli/src/commands/da.rs:1` میں ہے اور payload، erasure/retention profile اور optional
  metadata/manifest files لے کر CLI config key کے ساتھ canonical `DaIngestRequest` sign کرتی ہے۔
  کامیاب runs `da_request.{norito,json}` اور `da_receipt.{norito,json}` کو
  `artifacts/da/submission_<timestamp>/` کے تحت محفوظ کرتے ہیں (override via `--artifact-dir`) تاکہ
  release artefacts ingest میں استعمال ہونے والے exact Norito bytes record کریں۔
- کمانڈ default میں `client_blob_id = blake3(payload)` استعمال کرتی ہے مگر `--client-blob-id` overrides،
  metadata JSON maps (`--metadata-json`) اور pre-generated manifests (`--manifest`) کو accept کرتی ہے،
  اور `--no-submit` (offline preparation) اور `--endpoint` (custom Torii hosts) سپورٹ کرتی ہے۔ Receipt JSON
  stdout پر print ہوتا ہے اور disk پر بھی لکھا جاتا ہے، جس سے DA-8 کا "submit_blob" requirement پورا ہوتا ہے
  اور SDK parity work unblock ہوتا ہے۔
- `iroha app da get` DA-focused alias فراہم کرتا ہے جو multi-source orchestrator کو use کرتا ہے جو پہلے ہی
  `iroha app sorafs fetch` کو چلاتا ہے۔ Operators manifest + chunk-plan artefacts (`--manifest`, `--plan`, `--manifest-id`)
  **یا** Torii storage ticket via `--storage-ticket` دے سکتے ہیں۔ Ticket path پر CLI `/v1/da/manifests/<ticket>` سے
  manifest download کرتی ہے، bundle کو `artifacts/da/fetch_<timestamp>/` میں محفوظ کرتی ہے (override via
  `--manifest-cache-dir`)، `--manifest-id` کیلئے blob hash derive کرتی ہے، اور فراہم کردہ `--gateway-provider` list
  کے ساتھ orchestrator run کرتی ہے۔ SoraFS fetcher کے advanced knobs برقرار رہتے ہیں (manifest envelopes,
  client labels, guard caches, anonymity transport overrides, scoreboard export، اور `--output` paths) اور
  `--manifest-endpoint` کے ذریعے manifest endpoint override کیا جا سکتا ہے، لہذا end-to-end availability checks
  مکمل طور پر `da` namespace میں رہتے ہیں بغیر orchestrator logic duplicate کئے۔
- `iroha app da get-blob` Torii سے `GET /v1/da/manifests/{storage_ticket}` کے ذریعے canonical manifests کھینچتا ہے۔
  کمانڈ `manifest_{ticket}.norito`, `manifest_{ticket}.json`, اور `chunk_plan_{ticket}.json` کو
  `artifacts/da/fetch_<timestamp>/` میں لکھتی ہے (یا user-supplied `--output-dir`) اور عین `iroha app da get`
  invocation (بشمول `--manifest-id`) echo کرتی ہے جو follow-up orchestrator fetch کیلئے درکار ہے۔ اس سے operators
  manifest spool directories سے دور رہتے ہیں اور fetcher ہمیشہ Torii کے signed artefacts استعمال کرتا ہے۔
  JavaScript Torii client یہی flow `ToriiClient.getDaManifest(storageTicketHex)` کے ذریعے دیتا ہے، اور decoded
  Norito bytes، manifest JSON اور chunk plan واپس کرتا ہے تاکہ SDK callers CLI کے بغیر orchestrator sessions hydrate
  کر سکیں۔ Swift SDK بھی وہی surfaces expose کرتا ہے (`ToriiClient.getDaManifestBundle(...)` اور
  `fetchDaPayloadViaGateway(...)`)، bundles کو native SoraFS orchestrator wrapper میں pipe کر کے iOS clients کو
  manifests download کرنے، multi-source fetches چلانے اور proofs capture کرنے دیتا ہے بغیر CLI کے۔
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` دیے گئے storage size اور retention window کیلئے deterministic rent اور incentive breakdown
  compute کرتا ہے۔ یہ helper active `DaRentPolicyV1` (JSON یا Norito bytes) یا built-in default استعمال کرتا ہے،
  policy validate کرتا ہے، اور JSON summary (`gib`, `months`, policy metadata, `DaRentQuote` fields) print کرتا ہے
  تاکہ auditors governance minutes میں exact XOR charges cite کر سکیں بغیر ad hoc scripts کے۔ کمانڈ JSON payload
  سے پہلے ایک لائن `rent_quote ...` summary بھی دیتی ہے تاکہ console logs readable رہیں۔
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` کو `--policy-label "governance ticket #..."` کے ساتھ
  جوڑیں تاکہ prettified artefacts محفوظ ہوں جو درست policy vote یا config bundle cite کریں؛ CLI custom label کو trim
  کرتی ہے اور خالی strings reject کرتی ہے تاکہ `policy_source` اقدار dashboard میں actionable رہیں۔ دیکھیں
  `crates/iroha_cli/src/commands/da.rs` (subcommand) اور `docs/source/da/rent_policy.md` (policy schema)۔
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` اوپر کی سب چیزیں chain کرتا ہے: یہ storage ticket لیتا ہے، canonical manifest bundle
  download کرتا ہے، multi-source orchestrator (`iroha app sorafs fetch`) کو فراہم کردہ `--gateway-provider` list کے خلاف
  چلاتا ہے، downloaded payload + scoreboard کو `artifacts/da/prove_availability_<timestamp>/` میں محفوظ کرتا ہے، اور
  فوری طور پر موجودہ PoR helper (`iroha app da prove`) کو fetched bytes کے ساتھ invoke کرتا ہے۔ Operators orchestrator knobs
  (`--max-peers`, `--scoreboard-out`, manifest endpoint overrides) اور proof sampler (`--sample-count`, `--leaf-index`,
  `--sample-seed`) adjust کر سکتے ہیں جبکہ ایک ہی command DA-5/DA-9 audits کیلئے متوقع artefacts پیدا کرتی ہے:
  payload copy، scoreboard evidence، اور JSON proof summaries۔
