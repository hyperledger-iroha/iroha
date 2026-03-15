---
lang: he
direction: rtl
source: docs/portal/docs/da/ingest-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
ہونے تک دونوں ورژنز کو sync رکھیں۔
:::

# Sora Nexus תוכנית הטמעת זמינות נתונים

_مسودہ: 2026-02-20 -- مالک: Core Protocol WG / Storage Team / DA WG_

DA-2 ورک اسٹریم Torii میں ایک blob ingest API شامل کرتا ہے جو Norito میٹاڈیٹا
جاری کرتی ہے اور SoraFS ریپلیکیشن کو seed کرتی ہے۔ یہ دستاویز مجوزہ schema،
API surface، اور validation flow کو بیان کرتی ہے تاکہ implementation باقی
simulations (DA-1 follow-ups) پر رکے بغیر آگے بڑھے۔ פורמטים של מטען שימוש
Norito codecs استعمال کرنا لازمی ہے؛ serde/JSON fallback.

## اہداف

- بڑے blobs (Taikai segments، lane sidecars، governance artefacts) کو Torii کے
  ذریعے deterministically قبول کرنا۔
- כתם, פרמטרי קודקים, פרופיל מחיקה, מדיניות שימור או מדיניות שמירה
  والے canonical Norito manifests تیار کرنا۔
- chunk metadata کو SoraFS hot storage میں محفوظ کرنا اور replication jobs کو
  enqueue کرنا۔
- pin intents + policy tags کو SoraFS registry اور governance observers تک
  publish کرنا۔
- admission receipts فراہم کرنا تاکہ clients کو deterministic proof of
  publication مل سکے۔

## משטח API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Payload ایک Norito-encoded `DaIngestRequest` ہے۔ תגובות `application/norito+v1`
استعمال کرتی ہیں اور `DaIngestReceipt` واپس کرتی ہیں۔

| תגובה | مطلب |
| --- | --- |
| 202 מקובל | Blob کو chunking/replication کیلئے queue کیا گیا؛ receipt واپس۔ |
| 400 בקשה רעה | Schema/size violation (validation checks دیکھیں)۔ |
| 401 לא מורשה | API token موجود نہیں/غلط۔ |
| 409 קונפליקט | `client_blob_id` ‏ |
| 413 מטען גדול מדי | Configured blob length limit سے تجاوز۔ |
| 429 יותר מדי בקשות | פגע במגבלת השיעור. |
| 500 שגיאה פנימית | غیر متوقع failure (log + alert)۔ |

## סכימת Norito מוצעת

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
> `iroha_data_model::da::ingest` מבנה מניפסט
> `iroha_data_model::da::manifest` میں ہے۔

`compression` field بتاتا ہے کہ callers نے payload کیسے تیار کیا۔ Torii
`identity`, `gzip`, `deflate`, اور `zstd` قبول کرتا ہے اور hashing, chunking اور
optional manifests verify کرنے سے پہلے bytes کو decompress کرتا ہے۔

### רשימת אימות1. Verify کریں کہ request کا Norito header `DaIngestRequest` سے match کرتا ہے۔
2. اگر `total_size` canonical (decompressed) payload length سے مختلف ہو یا
   configured max سے زیادہ ہو تو fail کریں۔
3. `chunk_size` alignment enforce کریں (power-of-two, <= 2 MiB)۔
4. `data_shards + parity_shards` <= global maximum اور parity >= 2 یقینی بنائیں۔
5. `retention_policy.required_replica_count` کو governance baseline respect کرنا ہوگا۔
6. Canonical hash کے خلاف signature verification (signature field کے بغیر)۔
7. `client_blob_id` כפילויות דחייה
8. اگر `norito_manifest` دیا گیا ہو تو schema + hash کو chunking کے بعد
   recalculated manifest سے match کریں؛ ورنہ node manifest generate کر کے store کرے۔
9. מדיניות שכפול מוגדרת לאכוף תכנים: Torii `RetentionPolicy`
   `torii.da_ingest.replication_policy` سے rewrite کرتا ہے ( `replication-policy.md`
   دیکھیں ) اور pre-built manifests کو reject کرتا ہے اگر retention metadata
   enforced profile سے match نہ کرے۔

### זרימת נתחים ושכפול

1. Payload کو `chunk_size` میں تقسیم کریں، ہر chunk پر BLAKE3 اور Merkle root حساب کریں۔
2. Norito `DaManifestV1` (מבנה חדש) עבור התחייבויות נתח (role/group_id),
   פריסת מחיקה (ספירת שוויון שורה/עמודה + `ipa_commitment`), מדיניות שמירה,
   اور metadata کو capture کرے۔
3. Canonical manifest bytes کو `config.da_ingest.manifest_store_dir` کے تحت queue کریں
   (Torii `manifest.encoded` files lane/epoch/sequence/ticket/fingerprint کے لحاظ سے لکھتا ہے)
   تاکہ SoraFS orchestration انہیں ingest کر کے storage ticket کو persisted data سے link کرے۔
4. `sorafs_car::PinIntent` کے ذریعے governance tag + policy کے ساتھ pin intents publish کریں۔
5. אירוע Norito `DaIngestPublished` פולט משקיפים (לקוחות קלים, ממשל, ניתוח)
   کو اطلاع ملے۔
6. `DaIngestReceipt` caller کو واپس دیں (Torii DA service key سے signed) اور
   `Sora-PDP-Commitment` header emit کریں تاکہ SDKs فوری طور پر commitment capture کر سکیں۔
   Receipt میں اب `rent_quote` (Norito `DaRentQuote`) اور `stripe_layout` شامل ہیں،
   שכר בסיס של שולחים, נתח מילואים, ציפיות בונוס PDP/PoTR או 2D
   erasure layout کو storage ticket کے ساتھ funds commit کرنے سے پہلے دکھا سکتے ہیں۔

## עדכוני אחסון / רישום

- `sorafs_manifest` کو `DaManifestV1` کے ساتھ extend کریں تاکہ deterministic parsing ہو سکے۔
- זרם רישום חדש `da.pin_intent` מטען מטעון גרסתי
  manifest hash + ticket id کو reference کرتا ہے۔
- צינורות של צפיות, חביון ספיגה, תפוקה מופחתת, צבר שכפול
  اور failure counts ٹریک کرنے کیلئے اپ ڈیٹ کریں۔

## אסטרטגיית בדיקה- Schema validation، signature checks، duplicate detection کیلئے unit tests۔
- Norito encoding کے golden tests (`DaIngestRequest`, manifest, receipt)۔
- Integration harness جو mock SoraFS + registry چلاتا ہے اور chunk + pin flows verify کرتا ہے۔
- Property tests جو random erasure profiles اور retention combinations cover کرتے ہیں۔
- Norito payload fuzzing تاکہ malformed metadata سے بچاؤ ہو۔

## CLI & SDK Tooling (DA-8)- `iroha app da submit` (נקודת כניסה של CLI) בונה/מוציא לאור משותף להטמעת תוכן.
  operators Taikai bundle flow کے باہر arbitrary blobs ingest کر سکیں۔ یہ کمانڈ
  `crates/iroha_cli/src/commands/da.rs:1` میں ہے اور payload، erasure/retention profile اور optional
  metadata/manifest files لے کر CLI config key کے ساتھ canonical `DaIngestRequest` sign کرتی ہے۔
  کامیاب runs `da_request.{norito,json}` اور `da_receipt.{norito,json}` کو
  `artifacts/da/submission_<timestamp>/` کے تحت محفوظ کرتے ہیں (override via `--artifact-dir`) تاکہ
  release artefacts ingest میں استعمال ہونے والے exact Norito bytes record کریں۔
- הגדרות ברירת המחדל של `client_blob_id = blake3(payload)` מעקפות את `--client-blob-id`,
  metadata JSON maps (`--metadata-json`) اور pre-generated manifests (`--manifest`) کو accept کرتی ہے،
  اور `--no-submit` (offline preparation) اور `--endpoint` (custom Torii hosts) سپورٹ کرتی ہے۔ קבלה JSON
  stdout پر print ہوتا ہے اور disk پر بھی لکھا جاتا ہے، جس سے DA-8 کا "submit_blob" requirement پورا ہوتا ہے
  اور SDK parity work unblock ہوتا ہے۔
- `iroha app da get` כינוי ממוקד DA.
  `iroha app sorafs fetch` ‏ מניפסט מפעילים + חפצי אמנות בתוכנית נתח (`--manifest`, `--plan`, `--manifest-id`)
  **یا** Torii storage ticket via `--storage-ticket` دے سکتے ہیں۔ Ticket path پر CLI `/v1/da/manifests/<ticket>` سے
  manifest download کرتی ہے، bundle کو `artifacts/da/fetch_<timestamp>/` میں محفوظ کرتی ہے (override via
  `--manifest-cache-dir`)، `--manifest-id` کیلئے blob hash derive کرتی ہے، اور فراہم کردہ `--gateway-provider` list
  کے ساتھ orchestrator run کرتی ہے۔ SoraFS מחזיר כפתורים מתקדמים
  תוויות לקוח, מטמוני שמירה, עקיפת העברה אנונימית, ייצוא לוח תוצאות, או נתיבים `--output`) או
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
  manifests download کرنے، multi-source fetches چلانے اور proofs capture کرنے دیتا ہے بغیر CLI کے۔[IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` دیے گئے storage size اور retention window کیلئے deterministic rent اور incentive breakdown
  compute کرتا ہے۔ یہ helper active `DaRentPolicyV1` (JSON یا Norito bytes) یا built-in default استعمال کرتا ہے،
  policy validate کرتا ہے، اور JSON summary (`gib`, `months`, policy metadata, `DaRentQuote` fields) print کرتا ہے
  تاکہ auditors governance minutes میں exact XOR charges cite کر سکیں بغیر ad hoc scripts کے۔ מטען JSON
  سے پہلے ایک لائن `rent_quote ...` summary بھی دیتی ہے تاکہ console logs readable رہیں۔
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` کو `--policy-label "governance ticket #..."` کے ساتھ
  جوڑیں تاکہ prettified artefacts محفوظ ہوں جو درست policy vote یا config bundle cite کریں؛ תווית מותאמת אישית של CLI לקצץ
  کرتی ہے اور خالی strings reject کرتی ہے تاکہ `policy_source` اقدار dashboard میں actionable رہیں۔ دیکھیں
  `crates/iroha_cli/src/commands/da.rs` (תת-פקודה) או `docs/source/da/rent_policy.md` (סכימת מדיניות).
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` אופי של רשת שרשרת: 20000000127: 20000000127X: 20000000127X 2000000127X, חבילת מניפסט קנונית
  download کرتا ہے، multi-source orchestrator (`iroha app sorafs fetch`) کو فراہم کردہ `--gateway-provider` list کے خلاف
  چلاتا ہے، downloaded payload + scoreboard کو `artifacts/da/prove_availability_<timestamp>/` میں محفوظ کرتا ہے، اور
  فوری طور پر موجودہ PoR helper (`iroha app da prove`) کو fetched bytes کے ساتھ invoke کرتا ہے۔ ידיות מתזמר של מפעילים
  (`--max-peers`, `--scoreboard-out`, עקיפות נקודות קצה מניפסט) אור דוגמית הוכחה (`--sample-count`, `--leaf-index`,
  `--sample-seed`) adjust کر سکتے ہیں جبکہ ایک ہی command DA-5/DA-9 audits کیلئے متوقع artefacts پیدا کرتی ہے:
  עותק מטען, עדויות לוח תוצאות, סיכומי הוכחה של JSON.