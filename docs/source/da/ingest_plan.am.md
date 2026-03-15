---
lang: am
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T14:35:37.693070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus የውሂብ ተገኝነት ማስገቢያ እቅድ

የተረቀቀው፡ 2026-02-20 - ባለቤት፡ ኮር ፕሮቶኮል WG/የማከማቻ ቡድን/DA WG_

የDA-2 የስራ ዥረት Toriiን Norito በሚያወጣው የብሎብ ኢንጀስት ኤፒአይ ያራዝመዋል።
ሜታዳታ እና ዘሮች SoraFS ማባዛት። ይህ ሰነድ የታቀደውን ይይዛል
እቅድ፣ ኤፒአይ ወለል እና የማረጋገጫ ፍሰት ስለዚህ ትግበራ ያለሱ መቀጠል ይችላል።
በአስደናቂ ማስመሰያዎች ላይ ማገድ (DA-1 ክትትል)። ሁሉም የመጫኛ ቅርጸቶች የግድ
Norito ኮዴኮችን ይጠቀሙ; ምንም serde/JSON መውደቅ አይፈቀድም።

# ግቦች

- ትላልቅ ነጠብጣቦችን ይቀበሉ (የታይካይ ክፍሎች ፣ የሌይን የጎን መኪናዎች ፣ የአስተዳደር ቅርሶች)
  ከ Torii በላይ መወሰን።
- የብሎብ ፣ የኮዴክ መለኪያዎችን የሚገልጽ ቀኖናዊ Norito ተገለጠ ።
  መገለጫን መደምሰስ እና የማቆየት ፖሊሲ።
- በSoraFS ትኩስ ማከማቻ ውስጥ ቸንክ ሜታዳታ እና የማባዛት ስራዎችን ቀጥል።
- ፒን ኢንቴንስ + የፖሊሲ መለያዎችን ወደ SoraFS መዝገብ እና አስተዳደር ያትሙ
  ታዛቢዎች.
- የመግቢያ ደረሰኞችን በማጋለጥ ደንበኞቻቸው የህትመት ቆራጥነት ማረጋገጫ መልሰው እንዲያገኙ።

## API Surface (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

ክፍያ Norito-የተመሰጠረ `DaIngestRequest` ነው። ምላሾች ይጠቀማሉ
`application/norito+v1` እና `DaIngestReceipt` ይመለሱ።

| ምላሽ | ትርጉም |
| --- | --- |
| 202 ተቀባይነት | ብሎብ ለመክተፍ/ለመድገም ተሰልፏል; ደረሰኝ ተመልሷል. |
| 400 መጥፎ ጥያቄ | የመርሃግብር/መጠን ጥሰት (የማረጋገጫ ቼኮችን ይመልከቱ)። |
| 401 ያልተፈቀደ | የጠፋ/ልክ ያልሆነ የኤፒአይ ማስመሰያ። |
| 409 ግጭት | `client_blob_id` ከተዛመደ ሜታዳታ ጋር ያባዛ። |
| 413 ጭነት በጣም ትልቅ | ከተዋቀረ የብሎብ ርዝመት ገደብ አልፏል። |
| 429 በጣም ብዙ ጥያቄዎች | የዋጋ ገደብ ተመቷል። |
| 500 የውስጥ ስህተት | ያልተጠበቀ ውድቀት (የተመዘገበ + ማንቂያ)። |

```
GET /v1/da/proof_policies
Accept: application/json | application/x-norito
```

አሁን ካለው የሌይን ካታሎግ የተወሰደ `DaProofPolicyBundle` ያወጣል።
ጥቅሉ `version` (በአሁኑ ጊዜ `1`)፣ `policy_hash` (ሃሽ ኦፍ ዘ
የታዘዘ የፖሊሲ ዝርዝር)፣ እና `policies` ግቤቶች `lane_id`፣ `dataspace_id`፣
`alias`፣ እና የተተገበረው `proof_scheme` (`merkle_sha256` ዛሬ፤ KZG መስመሮች ናቸው
የKZG ቁርጠኝነት እስካልተገኘ ድረስ በመመገብ ውድቅ ተደርጓል)። የማገጃው ራስጌ አሁን
በ`da_proof_policies_hash` በኩል ወደ ቅርቅቡ ቃል ገብቷል፣ ስለዚህ ደንበኞቻችን ፒን ማድረግ ይችላሉ።
የDA ቃል ኪዳኖችን ወይም ማረጋገጫዎችን ሲያረጋግጥ ንቁ ፖሊሲ ተቀናብሯል። ይህን የመጨረሻ ነጥብ አምጡ
ከሌይኑ ፖሊሲ እና ከአሁኑ ጋር የሚዛመዱ መሆናቸውን ለማረጋገጥ ማረጋገጫዎችን ከመገንባቱ በፊት
ጥቅል ሃሽ. የቁርጠኝነት ዝርዝር/የማጠቃለያ ነጥቦች ኤስዲኬዎች አንድ አይነት ጥቅል ይይዛሉ
ማረጋገጫን ከገባሪ ፖሊሲ ስብስብ ጋር ለማያያዝ ተጨማሪ የዙር ጉዞ አያስፈልግዎትም።

```
GET /v1/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

የታዘዘውን የፖሊሲ ዝርዝር እና ሀ የያዘ `DaProofPolicyBundle` ይመልሳል
`policy_hash` ስለዚህ ኤስዲኬዎች እገዳ ሲፈጠር ጥቅም ላይ የዋለውን ስሪት ፒን ማድረግ ይችላሉ። የ
hash በNorito ኢንኮድ በተቀመጠው የፖሊሲ ድርድር ላይ ይሰላል እና በማንኛውም ጊዜ ይለወጣል
የሌይን `proof_scheme` ተዘምኗል፣ ይህም ደንበኞች በመካከላቸው መንሸራተትን እንዲያውቁ ያስችላቸዋል።
የተሸጎጡ ማረጋገጫዎች እና የሰንሰለት ውቅር።

## የታቀደው Norito እቅድ

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
```> የማስፈጸሚያ ማስታወሻ፡ ለነዚህ የክፍያ ጭነቶች ቀኖናዊው የዝገት ውክልናዎች አሁን ይኖራሉ
> `iroha_data_model::da::types`፣ በ`iroha_data_model::da::ingest` ከጥያቄ/ደረሰኝ መጠቅለያዎች ጋር
> እና በ `iroha_data_model::da::manifest` ውስጥ ያለው አንጸባራቂ መዋቅር።

የ`compression` መስኩ ደዋዮች ክፍያውን እንዴት እንዳዘጋጁ ያስተዋውቃል። Torii ይቀበላል
`identity`፣ `gzip`፣ `deflate`፣ እና `zstd`፣ ከዚህ በፊት ባይት በግልፅ እየቀነሱ
ማሽኮርመም፣ መቆራረጥ እና የአማራጭ መገለጫዎችን ማረጋገጥ።

### የማረጋገጫ ዝርዝር

1. ጥያቄን ያረጋግጡ Norito ራስጌ ግጥሚያዎች `DaIngestRequest`።
2. `total_size` ከቀኖናዊው (የተጨመቀ) የመጫኛ ርዝመት ቢለያይ ወይም ከተዋቀረው ከፍተኛው በላይ ከሆነ አይሳካም።
3. የ`chunk_size` አሰላለፍ ያስፈጽሙ (የሁለት ኃይል፣ = 2 ያረጋግጡ።
5. `retention_policy.required_replica_count` የአስተዳደርን መነሻ ማክበር አለበት።
6. ከቀኖናዊ ሃሽ (የፊርማ መስክ በስተቀር) የፊርማ ማረጋገጫ።
7. የተባዛ `client_blob_id` ክፍያ ሃሽ + ሜታዳታ ተመሳሳይ ካልሆነ በስተቀር ውድቅ ያድርጉ።
8. `norito_manifest` ሲቀርብ፣ ሼማ + የሃሽ ግጥሚያዎች እንደገና እንዲሰሉ ያረጋግጡ
   ከተቆረጠ በኋላ ይገለጣል; አለበለዚያ መስቀለኛ መንገድ አንጸባራቂ ያመነጫል እና ያከማቻል.
9. የተዋቀረውን የማባዛት ፖሊሲን ተግባራዊ ማድረግ፡- Torii የገባውን በድጋሚ ይጽፋል
   `RetentionPolicy` ከ `torii.da_ingest.replication_policy` ጋር (ተመልከት)
   `replication_policy.md`) እና ቀድሞ የተሰሩ የማሳያ መግለጫዎችን አይቀበልም
   ሜታዳታ ከተተገበረው መገለጫ ጋር አይዛመድም።

### የመቁረጥ እና የማባዛት ፍሰት1. Chunk payload ወደ `chunk_size`፣ BLAKE3 per chunk + Merkle root ያሰሉት።
2. Norito `DaManifestV1` (አዲስ መዋቅር) የግንቡ ቁርጠኝነት (ሚና/ቡድን_id)፣
   መደምሰስ አቀማመጥ (የረድፍ እና የአምድ እኩልነት ቆጠራዎች እና `ipa_commitment`) ፣ የማቆያ ፖሊሲ ፣
   እና ሜታዳታ።
3. ቀኖናዊ አንጸባራቂ ባይት በ`config.da_ingest.manifest_store_dir` ስር ወረፋ
   (Torii `manifest.encoded` ፋይሎችን በሌይን/ኢፖክ/ተከታታይ/ትኬት/አሻራ/የጣት አሻራ ይጽፋል) ስለዚህ SoraFS
   ኦርኬስትራ እነሱን ወደ ውስጥ ማስገባት እና የማከማቻ ትኬቱን ከቋሚ ውሂብ ጋር ማገናኘት ይችላል።
4. በአስተዳደር መለያ + ፖሊሲ በ`sorafs_car::PinIntent` በኩል ፒን ኢንቴንቶችን ያትሙ።
5. ኢሚት Norito ክስተት `DaIngestPublished` ታዛቢዎችን ለማሳወቅ (ቀላል ደንበኞች፣
   አስተዳደር, ትንታኔ).
6. `DaIngestReceipt` ይመለሱ (በTorii DA አገልግሎት ቁልፍ የተፈረመ) እና ይጨምሩ
   ቤዝ64 Norito ኢንኮዲንግ የያዘ `Sora-PDP-Commitment` ምላሽ ራስጌ
   የተገኘው ቁርጠኝነት ኤስዲኬዎች የናሙናውን ዘር ወዲያውኑ መደበቅ ይችላሉ።
   ደረሰኙ አሁን `rent_quote` (a `DaRentQuote`) እና `stripe_layout` ያካትታል
   ስለዚህ አስገቢዎች የXOR ግዴታዎችን፣ የአክሲዮን ድርሻን፣ የ PDP/PoTR ጉርሻ የሚጠበቁ ነገሮችን ማሟላት ይችላሉ።
   እና የ2D መጥፋት ማትሪክስ ልኬቶች ከማከማቻ-ትኬት ሜታዳታ ጎን ለጎን ገንዘብ ከማድረግዎ በፊት።
7. አማራጭ የመዝገብ ዲበ ውሂብ፡
   - `da.registry.alias` - ይፋዊ፣ ያልመሰጠረ UTF-8 ተለዋጭ ስም የፒን መመዝገቢያ ግቤትን ለመዝራት።
   - `da.registry.owner` — ይፋዊ፣ ያልተመሰጠረ `AccountId` ሕብረቁምፊ የመመዝገቢያ ባለቤትነትን ለመመዝገብ።
   Torii እነዚህን ወደ የመነጨው `DaPinIntent` ይገለብጣቸዋል ስለዚህ የታችኛው ፒን ማቀነባበር ተለዋጭ ስሞችን ማሰር ይችላል
   እና ባለቤቶች ጥሬ ሜታዳታ ካርታውን እንደገና ሳይተነተኑ; የተበላሹ ወይም ባዶ እሴቶች በዚህ ጊዜ ውድቅ ይደረጋሉ።
   የማስገባት ማረጋገጫ.

## ማከማቻ / መዝገብ ቤት ዝመናዎች

- `sorafs_manifest`ን በ`DaManifestV1` ያራዝሙ፣ የሚወስን ትንታኔን በማንቃት።
- አዲስ የመመዝገቢያ ዥረት አክል `da.pin_intent` ከተሰራ የክፍያ ጭነት ማጣቀሻ ጋር
  አንጸባራቂ hash + የቲኬት መታወቂያ።
- የመግቢያ መዘግየትን ለመከታተል ታዛቢነት ያላቸውን ቧንቧዎች አዘምን ፣
  ማባዛት የኋላ ታሪክ፣ እና ውድቀት ይቆጥራል።
- Torii `/status` ምላሾች አሁን የ `taikai_ingest` ድርድር ያካትታሉ
  ኢንኮደር-ወደ-መግባት መዘግየት፣ የቀጥታ ጠርዝ ተንሸራታች እና የስህተት ቆጣሪዎች በ(ክላስተር፣ ዥረት)፣ DA-9ን ማንቃት
  ዳሽቦርዶች Prometheus ሳትቧጭ የጤንነት ቅጽበታዊ ገጽ እይታዎችን በቀጥታ ከአንጓዎች ለማስገባት።

## የሙከራ ስልት- የንድፍ ማረጋገጫ፣ የፊርማ ቼኮች፣ የተባዛ ፈልጎ ለማግኘት የክፍል ሙከራዎች።
- ወርቃማ ሙከራዎች Norito የ`DaIngestRequest` ኢንኮዲንግ ፣መገለጫ እና ደረሰኝ የሚያረጋግጡ።
- የውህደት ማሰሪያ የሚሽከረከር እስከ መሳለቂያ SoraFS + መዝገብ ቤት፣ ቸንክ + ፒን ፍሰቶችን ያረጋግጣል።
- የዘፈቀደ የመደምሰስ መገለጫዎችን እና የማቆየት ውህዶችን የሚሸፍኑ የንብረት ሙከራዎች።
- ከተበላሸ ሜታዳታ ለመጠበቅ የNorito ጭነቶች ማደብዘዝ።
- ለእያንዳንዱ የብሎብ ክፍል ወርቃማ ዕቃዎች በስር ይኖራሉ
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` ከአጃቢ ቁራጭ ጋር
  በ `fixtures/da/ingest/sample_chunk_records.txt` ውስጥ መዘርዘር. ችላ የተባለው ፈተና
  `regenerate_da_ingest_fixtures` መጫዎቻዎቹን ያድሳል፣ እያለ
  `manifest_fixtures_cover_all_blob_classes` አዲስ የ`BlobClass` ልዩነት እንደታከለ አይሳካም
  የ Norito/JSON ቅርቅብ ሳያዘምኑ። ይህ Toriiን፣ ኤስዲኬዎችን እና ሰነዶችን በማንኛውም ጊዜ DA-2 ያቆያል።
  አዲስ የብሎብ ገጽን ይቀበላል።【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tess.rs:2902】

## CLI እና SDK Tooling (DA-8)- `iroha app da submit` (አዲስ የ CLI መግቢያ ነጥብ) አሁን የተጋራውን ኢንጀስት ገንቢ/አሳታሚ ይጠቀለላል ስለዚህ ኦፕሬተሮች
  ከታይካይ ጥቅል ፍሰት ውጭ የዘፈቀደ ነጠብጣቦችን መውሰድ ይችላል። ትዕዛዙ ውስጥ ይኖራል
  `crates/iroha_cli/src/commands/da.rs:1` እና የሚከፈል ጭነት፣ መደምሰስ/ማቆያ መገለጫ እና
  ቀኖናዊውን `DaIngestRequest` በCLI ከመፈረምዎ በፊት አማራጭ ሜታዳታ/ፋይሎችን አሳይ
  የማዋቀር ቁልፍ. ስኬታማ ሩጫዎች በ `da_request.{norito,json}` እና `da_receipt.{norito,json}` ስር ቀጥለዋል
  `artifacts/da/submission_<timestamp>/` (በ`--artifact-dir` መሻር) ስለዚህ ቅርሶችን መልቀቅ ይቻላል
  በሚመገቡበት ጊዜ ጥቅም ላይ የዋለውን ትክክለኛ Norito ባይት ይመዝግቡ።
- ትዕዛዙ ወደ `client_blob_id = blake3(payload)` ነባሪ ነው ነገር ግን መሻሮችን ይቀበላል
  `--client-blob-id`፣ ሜታዳታ JSON ካርታዎችን (`--metadata-json`) እና አስቀድሞ የመነጨ መገለጫዎችን ያከብራል።
  (`--manifest`)፣ እና `--no-submit` ከመስመር ውጭ ዝግጅት እና `--endpoint` ለብጁ ይደግፋል።
  Torii አስተናጋጆች። ደረሰኝ JSON ወደ ዲስክ ከመፃፍ በተጨማሪ ወደ stdout ታትሟል፣ መዝጋት
  DA-8 "submit_blob" የመሳሪያ መስፈርት እና የኤስዲኬ እኩልነት ስራን ማንሳት።
- `iroha app da get` ባለ ብዙ ምንጭ ኦርኬስትራ በ DA-ተኮር ተለዋጭ ስም ያክላል
  `iroha app sorafs fetch`. ኦፕሬተሮች በማኒፌክት + chunk-plan artefacts (`--manifest`፣
  `--plan`፣ `--manifest-id`) **ወይም** በቀላሉ Torii የማከማቻ ትኬት በ`--storage-ticket` ማለፍ። መቼ
  የቲኬት መንገድ ጥቅም ላይ ይውላል CLI አንጸባራቂውን ከ `/v1/da/manifests/<ticket>` ይጎትታል፣ ጥቅሉን ይቀጥላል
  በ`artifacts/da/fetch_<timestamp>/` (በ`--manifest-cache-dir` መሻር) ** ገላጭ
  hash** ለ`--manifest-id`፣ እና ኦርኬስትራውን በቀረበው `--gateway-provider` ያስኬዳል።
  ዝርዝር. የመክፈያ ማረጋገጫ አሁንም በተከተተው CAR/`blob_hash` መፍጨት ላይ የሚመረኮዝ ሲሆን የመግቢያ መታወቂያው ግን
  አሁን ተገልጋዮች እና አረጋጋጮች አንድ ነጠላ ብሎብ ለዪ እንዲካፈሉ አንጸባራቂው ሃሽ። ሁሉም የላቁ እንቡጦች ከ
  የ SoraFS ፈልሳፊው ገጽ ሳይነካ (የገለጡ ፖስታዎች፣ የደንበኛ መለያዎች፣ የጥበቃ መሸጎጫዎች፣ ማንነታቸው የማይታወቅ መጓጓዣ
  ይሽራል፣ የውጤት ሰሌዳ ወደ ውጪ መላክ እና `--output` መንገዶች)፣ እና አንጸባራቂው የመጨረሻ ነጥብ ሊሻር የሚችለው በ
  `--manifest-endpoint` ለብጁ Torii አስተናጋጆች፣ ስለዚህ ከጫፍ እስከ ጫፍ ያለው ተገኝነት ፍተሻዎች ሙሉ በሙሉ በ
  የኦርኬስትራ ሎጂክን ሳያባዛ `da` የስም ቦታ።
- `iroha app da get-blob` ቀኖናዊ መግለጫዎችን በቀጥታ ከ Torii በ `GET /v1/da/manifests/{storage_ticket}` ይጎትታል።
  ትዕዛዙ አሁን ቅርሶችን በአንጸባራቂ ሃሽ (ብሎብ መታወቂያ) ይጽፋል
  `manifest_{manifest_hash}.norito`፣ `manifest_{manifest_hash}.json`፣ እና `chunk_plan_{manifest_hash}.json`
  በ `artifacts/da/fetch_<timestamp>/` (ወይም በተጠቃሚ የቀረበ `--output-dir`) ትክክለኛውን እያስተጋባ
  ለክትትል ኦርኬስትራ ማምጣት የ`iroha app da get` ጥሪ (`--manifest-id`ን ጨምሮ) ያስፈልጋል።
  ይህ ኦፕሬተሮችን ከማንፀባረቂያው የስፑል ማውጫዎች እንዲወጡ ያደርጋቸዋል እና ፈላጊው ሁልጊዜም እንደሚጠቀም ዋስትና ይሰጣል
  በTorii የተለቀቁ የተፈረሙ ቅርሶች። የጃቫ ስክሪፕት Torii ደንበኛ ይህንን ፍሰት ያንጸባርቃል
  `ToriiClient.getDaManifest(storageTicketHex)` ስዊፍት ኤስዲኬ አሁን ሲያጋልጥ
  `ToriiClient.getDaManifestBundle(...)`. ሁለቱም ዲኮድ የተደረገውን Norito ባይት፣ አንጸባራቂ JSON፣ አንጸባራቂ ሃሽ፣የኤስዲኬ ደዋዮች የኦርኬስትራ ክፍለ-ጊዜዎችን ወደ CLI እና ስዊፍት ሳይዘጉ እርጥበታማ ማድረግ እንዲችሉ ቸንክ ፕላን
  ደንበኞቹ በተጨማሪ ወደ `fetchDaPayloadViaGateway(...)` መደወል ይችላሉ እነዚያን ቅርቅቦች በአገሬው በኩል
  SoraFS ኦርኬስትራ መጠቅለያ።【IrohaSwift/ምንጮች/IrohaSwift/ToriiClient.swift:240】
- የ`/v1/da/manifests` ምላሾች አሁን ላዩን `manifest_hash`፣ እና ሁለቱም የCLI + SDK ረዳቶች (`iroha app da get`፣
  `ToriiClient.fetchDaPayloadViaGateway`፣ እና የስዊፍት/ጄኤስ ጌትዌይ መጠቅለያዎች) ይህንን የምግብ መፍጨት ሂደት እንደ
  ቀኖናዊ አንጸባራቂ ለዪ ክፍያዎችን በተከተተው የCAR/blob hash ማረጋገጥ በሚቀጥልበት ጊዜ።
- `iroha app da rent-quote` ለቀረበው የማከማቻ መጠን የሚወስን የቤት ኪራይ እና የማበረታቻ ክፍተቶችን ያሰላል
  እና የማቆያ መስኮት. ረዳቱ ንቁውን `DaRentPolicyV1` (JSON ወይም Norito ባይት) ወይም
  አብሮ የተሰራው ነባሪ፣ ፖሊሲውን ያጸድቃል እና የJSON ማጠቃለያ ያትማል (`gib`፣ `months`፣ የመመሪያ ሜታዳታ፣
  እና `DaRentQuote` መስኮች) ስለዚህ ኦዲተሮች በትክክል የXOR ክፍያዎችን በአስተዳደር ደቂቃዎች ውስጥ ሊጠቅሱ ይችላሉ
  ማስታወቂያ ሆክ ስክሪፕቶችን መጻፍ። ትዕዛዙ አሁን ደግሞ አንድ መስመር `rent_quote ...` ማጠቃለያ ከJSON በፊት ያወጣል።
  በአደጋ ጊዜ ጥቅሶች በሚፈጠሩበት ጊዜ የኮንሶል ምዝግብ ማስታወሻዎችን እና የሩጫ መጽሃፎችን በቀላሉ ለመቃኘት የሚከፈል ጭነት።
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` (ወይም ሌላ ማንኛውንም መንገድ) ማለፍ
  ቆንጆ-የታተመ ማጠቃለያውን ለመቀጠል እና በ `--policy-label "governance ticket #..."` ይጠቀሙ
  artefact አንድ የተወሰነ የድምጽ / ውቅር ጥቅል መጥቀስ ያስፈልገዋል; CLI ብጁ መለያዎችን ጠርጎ ባዶውን ውድቅ ያደርጋል
  የ `policy_source` እሴቶችን በማስረጃ ቅርቅቦች ውስጥ ትርጉም ያለው እንዲሆን ለማድረግ ሕብረቁምፊዎች። ተመልከት
  `crates/iroha_cli/src/commands/da.rs` ለክፍለ ትዕዛዝ እና `docs/source/da/rent_policy.md`
  ለፖሊሲው እቅድ።【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/ rent_policy.md:1】
- የፒን መዝገብ እኩልነት አሁን ወደ ኤስዲኬዎች ይዘልቃል፡ `ToriiClient.registerSorafsPinManifest(...)` በ
  ጃቫ ስክሪፕት ኤስዲኬ በ `iroha app sorafs pin register` ጥቅም ላይ የዋለውን ትክክለኛ የክፍያ ጭነት ይገነባል ፣
  ወደ ከመለጠፍዎ በፊት chunker ሜታዳታ፣ የፒን ፖሊሲዎች፣ ተለዋጭ ማስረጃዎች እና ተተኪ መፍጨት
  `/v1/sorafs/pin/register`. ይህ CI ቦቶች እና አውቶሜሽን ሲደርሱ ወደ CLI እንዳይደርሱ ያደርጋል
  አንጸባራቂ ምዝገባዎችን መቅዳት እና ረዳቱ ከTyScript/README ሽፋን ጋር ወደ DA-8 ይልካል
  "ማስገባት/ማግኘት/አረጋግጥ" የመሳሪያ አሰራር ከ Rust/Swift ጎን ለጎን በJS ላይ ሙሉ በሙሉ ረክቷል።【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:788】
- `iroha app da prove-availability` ሁሉንም ከላይ ያሉትን ሰንሰለቶች ያሰራል፡ የማከማቻ ትኬት ይወስዳል፣ ያውርዳል
  ቀኖናዊ አንጸባራቂ ጥቅል፣ ባለብዙ ምንጭ ኦርኬስትራውን (`iroha app sorafs fetch`) ከ
  የቀረበው `--gateway-provider` ዝርዝር፣ የወረደውን የክፍያ ጭነት + የውጤት ሰሌዳ ይቀጥላል።
  `artifacts/da/prove_availability_<timestamp>/`፣ እና ወዲያውኑ ያለውን የPoR አጋዥን ይጠራል
  (`iroha app da prove`) የተገኙትን ባይት በመጠቀም። ኦፕሬተሮች የኦርኬስትራ ቁልፎችን ማስተካከል ይችላሉ።
  (`--max-peers`፣ `--scoreboard-out`፣ አንጸባራቂ የመጨረሻ ነጥብ ይሽራል) እና የማረጋገጫ ናሙና
  (`--sample-count`፣ `--leaf-index`፣ `--sample-seed`) አንድ ትዕዛዝ ግን ቅርሶቹን ሲያመርት
  በDA-5/DA-9 ኦዲቶች የሚጠበቀው፡ የመጫኛ ኮፒ፣ የውጤት ሰሌዳ ማስረጃ እና የJSON ማረጋገጫ ማጠቃለያዎች።- `da_reconstruct` (አዲስ በ DA-6) ቀኖናዊ ማኒፌክትን እና በቁጣው የወጣውን የቁርጥ ማውጫ ያነባል
  ማከማቻ (`chunk_{index:05}.bin` አቀማመጥ) እና በማረጋገጥ ጊዜ ክፍያውን እንደገና ይሰበስባል
  እያንዳንዱ Blake3 ቁርጠኝነት. CLI በ `crates/sorafs_car/src/bin/da_reconstruct.rs` ስር ይኖራል እና እንደ መርከቦች ይላካል
  የSoraFS መሣሪያ ጥቅል አካል። የተለመደ ፍሰት
  1. `iroha app da get-blob --storage-ticket <ticket>` ለማውረድ `manifest_<manifest_hash>.norito` እና የ chunk እቅድ.
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (ወይንም `iroha app da prove-availability`፣ይህም ከሥር የተገኙ ቅርሶችን ይጽፋል
     `artifacts/da/prove_availability_<ts>/` እና በ`chunks/` ማውጫ ውስጥ ያሉ በአንድ ቸንክ ፋይሎች ይቀጥላል)።
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  የድጋሚ ማስተካከያ በ`fixtures/da/reconstruct/rs_parity_v1/` ስር ይኖራል እና ሙሉውን አንጸባራቂ ይይዛል
  እና ቸንክ ማትሪክስ (ዳታ + እኩልነት) በ `tests::reconstructs_fixture_with_parity_chunks` ጥቅም ላይ የዋለ። ጋር ያድሱት።

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  መሣሪያው ያመነጫል-

  - `manifest.{norito.hex,json}` — ቀኖናዊ `DaManifestV1` ኢንኮዲንግ።
  - `chunk_matrix.json` - የታዘዘ ኢንዴክስ/ማካካሻ/ርዝመት/መፍጨት/ተመጣጣኝ ረድፎች ለዶክ/የሙከራ ማጣቀሻዎች።
  - `chunks/` — `chunk_{index:05}.bin` የመጫኛ ቁርጥራጮች ለሁለቱም ዳታ እና እኩልነት ሻርዶች።
  - `payload.bin` - የተመጣጣኝ የመታጠቂያ ፈተና ጥቅም ላይ የሚውል የመወሰን ጭነት።
  - `commitment_bundle.{json,norito.hex}` - ናሙና `DaCommitmentBundle` ለሰነዶች/ሙከራዎች ወሳኝ KZG ቁርጠኝነት ያለው።

  ማሰሪያው የጎደሉትን ወይም የተቆራረጡ ቁርጥራጮችን እምቢ ይላል፣ የመጨረሻውን የክፍያ ጭነት Blake3 hash በ `blob_hash` ላይ ይፈትሻል፣
  እና ማጠቃለያ JSON ብሎብ ያወጣል (የተጫኑ ባይት፣ ቁርጥራጭ ብዛት፣ የማከማቻ ትኬት) ስለዚህ CI መልሶ ግንባታን ማረጋገጥ ይችላል
  ማስረጃ. ይህ የDA-6 መስፈርትን የሚዘጋው ኦፕሬተሮች እና QA ለሚወስኑ የመልሶ ግንባታ መሳሪያ ነው።
  ስራዎች የቃል ስክሪፕቶችን ሳያደርጉ መጥራት ይችላሉ።

## TODO የጥራት ማጠቃለያ

ሁሉም ከዚህ ቀደም የታገዱ TODOዎች ተተግብረዋል እና ተረጋግጠዋል፡- ** የመጭመቅ ፍንጮች *** - Torii በጠሪዎች የቀረቡ መለያዎችን ይቀበላል (`identity`፣ `gzip`፣ `deflate`፣
  `zstd`) እና ከመጽደቁ በፊት የሚጫኑ ጭነቶችን መደበኛ ያደርጋል ስለዚህም ቀኖናዊው አንጸባራቂ ሃሽ ከሚከተሉት ጋር ይዛመዳል።
  የተቀነሰ ባይት።【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **የመንግስት-ብቻ ዲበዳታ ምስጠራ** — Torii አሁን የአስተዳደር ሜታዳታን በ
  የChaCha20-Poly1305 ቁልፍ ተዋቅሯል፣ ያልተዛመደ መለያዎችን ውድቅ ያደርጋል እና ሁለት ግልፅ አድርጓል።
  የማዋቀር ቁልፎች (`torii.da_ingest.governance_metadata_key_hex`፣
  `torii.da_ingest.governance_metadata_key_label`) የማሽከርከር ቆራጥነትን ለመጠበቅ።【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- ** ትልቅ የክፍያ ጭነት ዥረት ** - ባለብዙ ክፍል ማስገቢያ በቀጥታ ነው። የደንበኛ ዥረት deterministic
  `DaIngestChunk` በ `client_blob_id` ፣ Torii የተከፈቱ ኤንቨሎፖች እያንዳንዱን ቁራጭ ያረጋግጣሉ ፣ ደረጃቸውንም ያዘጋጃሉ
  በ `manifest_store_dir` ስር፣ እና በአቶሚካል መግለጫውን አንዴ የ`is_last` ባንዲራ መሬት ይገነባል።
  በነጠላ ጥሪ ሰቀላዎች የታዩትን የ RAM ፍንጮችን በማስወገድ ላይ።【crates/iroha_torii/src/da/ingest.rs:392】
- ** ግልጽ ስሪት ** — `DaManifestV1` ግልጽ የሆነ `version` መስክ ይይዛል እና Torii እምቢ አለ።
  ያልታወቁ ስሪቶች፣ አዲስ አንጸባራቂ አቀማመጦች ሲርከብ የሚወስኑ ማሻሻያዎችን ዋስትና ይሰጣል።【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR መንጠቆዎች *** - የ PDP ቁርጠኝነት በቀጥታ ከ chunk መደብር የተገኙ እና ጸንተዋል
  የ DA-5 መርሐግብር አውጪዎች ከቀኖናዊ መረጃዎች የናሙና ፈተናዎችን እንዲጀምሩ ከማስረጃዎች በተጨማሪ። የ
  `Sora-PDP-Commitment` ራስጌ አሁን በሁለቱም `/v1/da/ingest` እና `/v1/da/manifests/{ticket}` ይላካል
  ምላሾች ስለዚህ ኤስዲኬዎች ወዲያውኑ የተፈረመውን ቁርጠኝነት ይማራሉ ይህም የወደፊት መርማሪዎች ይጠቅሳሉ።
- **የሻርድ ጠቋሚ ጆርናል** — ሌይን ሜታዳታ `da_shard_id`ን ሊገልጽ ይችላል (ወደ `lane_id` ነባሪ) እና
  Sumeragi አሁን ከፍተኛውን `(epoch, sequence)` በ `(shard_id, lane_id)` ቀጥሏል
  `da-shard-cursors.norito` ከDA spool ጋር በመሆን እንደገና እንዲጀመር እንደገና የተከፋፈሉ/ያልታወቁ መስመሮችን ይጥሉ እና ይቀጥሉ
  እንደገና አጫውት deterministic. የማህደረ ትውስታ ሻርድ ጠቋሚ መረጃ ጠቋሚ አሁን በገባው ቃል ላይ በፍጥነት አይሳካም።
  ካርታ የሌላቸው መስመሮች ወደ ሌይን መታወቂያ ነባሪ ከመሆን ይልቅ የጠቋሚ እድገትን እና ስህተቶችን እንደገና በማጫወት ላይ
  ግልጽ፣ እና ማረጋገጫን አግድ ከተወሰነ ጋር የሻርድ-ጠቋሚ ለውጦችን ውድቅ ያደርጋል
  `DaShardCursorViolation` ምክንያት + የቴሌሜትሪ መለያዎች ለኦፕሬተሮች። ጅምር/መያዝ አሁን DA ያቆማል
  ኩራ ያልታወቀ መስመር ወይም ወደ ኋላ የሚመለስ ጠቋሚ ከያዘ እና ጥፋቱን ከመዘገበ መረጃ ጠቋሚ
  አግድ ቁመት ስለዚህ ኦፕሬተሮች DA ከማገልገልዎ በፊት ማስተካከል ይችላሉ። state.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/ sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- ** የሻርድ ጠቋሚ ላግ ቴሌሜትሪ *** - የ `da_shard_cursor_lag_blocks{lane,shard}` መለኪያ እንዴት እንደሆነ ሪፖርት ያደርጋልቁመቱ እየተረጋገጠ ሲሄድ አንድ ሸርተቴ ይጓዛል። የጠፉ/ያረጁ/ያልታወቁ መስመሮች ዘግይተው ወደ
  የሚፈለገው ቁመት (ወይም ዴልታ)፣ እና የተሳካ እድገቶች ወደ ዜሮ ዳግም ያስጀምረዋል ስለዚህ የተረጋጋ ሁኔታ ጠፍጣፋ ሆኖ ይቆያል።
  ኦፕሬተሮች ዜሮ ባልሆኑ መዘግየት ላይ ማስጠንቀቅ አለባቸው፣ ለጥፋተኛው መስመር የDA spool/ጆርናልን ይፈትሹ፣
  እና የሌይን ካታሎግን በአጋጣሚ መልሶ ለማጋራት ያረጋግጡ ብሎክውን እንደገና ከመጫወትዎ በፊት
  ክፍተት.
- ** ሚስጥራዊ ስሌት መስመሮች *** - መስመሮች ምልክት የተደረገባቸው
  `metadata.confidential_compute=true` እና `confidential_key_version` ይታከማሉ።
  SMPC/የተመሰጠረ DA ዱካዎች፡- Sumeragi ዜሮ ያልሆነ ክፍያን ያስፈጽማል/የመፍጨት እና የማከማቻ ትኬቶችን ያሳያል፣
  ሙሉ ቅጂ የማከማቻ መገለጫዎችን ውድቅ ያደርጋል፣ እና የSoraFS ትኬት + የመመሪያ ሥሪትን ያለ ምንም መረጃ ጠቋሚ ያሳያል።
  የመጫኛ ባይት ማጋለጥ። በድጋሚ ጨዋታ ወቅት ደረሰኞች ከኩራ ይደርቃሉ ስለዚህ አረጋጋጮች ተመሳሳይ መልሰው ያገኛሉ
  እንደገና ከተጀመረ በኋላ ሚስጥራዊነት ሜታዳታ።【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_core/.

## የትግበራ ማስታወሻዎች- የ Torii's `/v1/da/ingest` የመጨረሻ ነጥብ አሁን የክፍያ ጭነት መጨናነቅን መደበኛ ያደርገዋል፣ የመልሶ ማጫወት መሸጎጫውን ያስፈጽማል፣
  ቀኖናዊ ባይቶችን በቆራጥነት ቆርጦ `DaManifestV1` እንደገና ገንብቶ የተመሰጠረውን ጭነት ይጥላል።
  ደረሰኙን ከመሰጠቱ በፊት ወደ `config.da_ingest.manifest_store_dir` ለ SoraFS ኦርኬስትራ; የ
  ደንበኞቻቸው የተመሰጠረውን ቁርጠኝነት እንዲይዙ ተቆጣጣሪው የ `Sora-PDP-Commitment` አርዕስት አያይዟል።
  ወዲያውኑ።【crates/iroha_torii/src/da/ingest.rs:220】
- ቀኖናዊውን `DaCommitmentRecord` ከቀጠለ በኋላ Torii አሁን
  የ`da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` ፋይል ከማንፀባረቂያው spool አጠገብ።
  እያንዳንዱ ግቤት መዝገቡን በጥሬው Norito `PdpCommitment` ባይት ስለዚህ DA-3 የጥቅል ግንበኞች እና
  የDA-5 መርሐግብር አውጪዎች መግለጫዎችን እንደገና ሳያነቡ ወይም የተቆራረጡ መደብሮች ሳያነቡ ተመሳሳይ ግብዓቶችን ያስገባሉ።【crates/iroha_torii/src/da/ingest.rs:1814】
- የኤስዲኬ ረዳቶች እያንዳንዱ ደንበኛ Norito መተንተንን እንደገና እንዲተገበር ሳያስገድዱ የ PDP ራስጌ ባይት ያጋልጣሉ፡
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` ሽፋን ዝገት ፣ Python `ToriiClient`
  አሁን `decode_pdp_commitment_header` ወደ ውጭ ይላካል፣ እና `IrohaSwift` ተንቀሳቃሽ ረዳት አጋዥዎችን ይልካል።
  ደንበኞች ኢንኮድ የተደረገውን የናሙና መርሃ ግብር ወዲያውኑ መደበቅ ይችላሉ።
- Torii በተጨማሪም `GET /v1/da/manifests/{storage_ticket}` ያጋልጣል ስለዚህ ኤስዲኬዎች እና ኦፕሬተሮች ማኒፌክቶችን ማምጣት ይችላሉ።
  እና የመስቀለኛ መንገዱን የስፑል ማውጫ ሳይነኩ ቸንክ እቅዶች። ምላሹ Norito ባይት ይመልሳል
  (base64)፣ የተሰራው አንጸባራቂ JSON፣ `chunk_plan` JSON ብሎብ ለ`sorafs fetch` ዝግጁ፣ እና ተዛማጅነት ያለው
  ሄክስ ዳይጀስት (`storage_ticket`፣ `client_blob_id`፣ `blob_hash`፣ `chunk_root`) የታችኛው ተፋሰስ መሣሪያ ማድረግ ይችላል።
  ኦርኬስትራውን እንደገና ሳያስሉ ይመግቡ እና ተመሳሳዩን `Sora-PDP-Commitment` አርዕስት ወደ
  መስተዋቶች ማስገቢያ ምላሾች. `block_hash=<hex>`ን እንደ መጠይቅ ልኬት ማለፍ ቆራጥነት ይመልሳል
  `sampling_plan` በ `block_hash || client_blob_id` (በአረጋጋጮች የተጋራ)
  `assignment_hash`፣ የተጠየቀው `sample_window`፣ እና `(index, role, group)` tuples ስፋቶችን ቀርቧል።
  የPoR ናሙናዎች እና አረጋጋጮች ተመሳሳይ ኢንዴክሶችን እንደገና ማጫወት እንዲችሉ አጠቃላይ የ 2D ድርድር አቀማመጥ። ናሙና ሰጪው
  `client_blob_id`፣ `chunk_root`፣ እና `ipa_commitment` ወደ ምደባ ሃሽ ያቀላቅላል፤ ኢሮሀ መተግበሪያ አግኝ
  --block-hash ` now writes `sampling_plan_.json` ከማንፀባረቂያው + ቻንክ እቅድ ቀጥሎ
  ሃሽ ተጠብቆ፣ እና የJS/Swift Torii ደንበኞች ተመሳሳይ `assignment_hash_hex` አረጋጋጮች ያጋልጣሉ።
  እና provers አንድ ነጠላ የመወሰኛ መጠይቅ ስብስብ ይጋራሉ። Torii የናሙና እቅድ ሲመልስ `iroha app da
  prove-availability` now reuses that deterministic probe set (seed derived from `sample_seed`) በምትኩ
  የአድ-ሆክ ናሙና ስለዚህ የPoR ምስክሮች ኦፕሬተሩ ቢተውም ከአረጋጋጭ ስራዎች ጋር ይሰለፋሉ።
  `--block-hash` መሻር።【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】 【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/ምንጮች/IrohaSwift/ToriiClient.swift:170】

### ትልቅ የክፍያ ጭነት ፍሰት ፍሰትከተዋቀረው የነጠላ ጥያቄ ገደብ በላይ የሆኑ ንብረቶችን ማስገባት የሚፈልጉ ደንበኞች ሀ
የዥረት ክፍለ ጊዜ `POST /v1/da/ingest/chunk/start` በመደወል። Torii በ ሀ
`ChunkSessionId` (BLAKE3-ከተጠየቀው የብሎብ ሜታዳታ የተገኘ) እና የተደራደረው ቸንክ መጠን።
እያንዳንዱ ተከታይ የ`DaIngestChunk` ጥያቄ የሚከተሉትን ይይዛል፡-

- `client_blob_id` - ከመጨረሻው `DaIngestRequest` ጋር ተመሳሳይ ነው።
- `chunk_session_id` - ቁርጥራጮቹን ከሩጫው ክፍለ ጊዜ ጋር ያገናኛል።
- `chunk_index` እና `offset` - የሚወስን ቅደም ተከተል ያስፈጽሙ።
- `payload` - እስከ ድርድር ቻንክ መጠን ድረስ።
- `payload_hash` — BLAKE3 የቁርጭምጭሚቱ ሃሽ ስለዚህ Torii ሙሉውን ብሎብ ሳያስቀር ማረጋገጥ ይችላል።
- `is_last` - የተርሚናል ቁራጭን ያመለክታል።

Torii በ `config.da_ingest.manifest_store_dir/chunks/<session>/` እና
ድህነትን ለማክበር በድጋሚ አጫውት መሸጎጫ ውስጥ ያለውን ሂደት ይመዘግባል። የመጨረሻው ቁራጭ ሲያርፍ፣ Torii
ክፍያውን በዲስክ ላይ እንደገና ይሰበስባል (የማህደረ ትውስታ ፍንጮችን ለማስወገድ በ chunk directory በኩል መልቀቅ)
ቀኖናዊውን አንጸባራቂ/ደረሰኝ ልክ በነጠላ ምት ሰቀላ ያሰላል እና በመጨረሻም ምላሽ ይሰጣል
`POST /v1/da/ingest` የተደረደሩትን ቅርሶች በመብላት። ያልተሳኩ ክፍለ ጊዜዎች በግልጽ ሊሰረዙ ይችላሉ ወይም
ከ `config.da_ingest.replay_cache_ttl` በኋላ በቆሻሻ የተሰበሰቡ ናቸው። ይህ ንድፍ የአውታረ መረብ ቅርጸቱን ያስቀምጣል
Norito-ተስማሚ፣ ደንበኛ-ተኮር የሆኑ ድጋሚ ሊደረጉ የሚችሉ ፕሮቶኮሎችን ያስወግዳል፣ እና ያለውን አንጸባራቂ ቧንቧ እንደገና ይጠቀማል።
ያልተለወጠ.

** የትግበራ ሁኔታ።** ቀኖናዊው Norito ዓይነቶች አሁን ይኖራሉ
`crates/iroha_data_model/src/da/`፡

- `ingest.rs` `DaIngestRequest`/`DaIngestReceipt` ይገልጻል
  `ExtraMetadata` መያዣ በTorii ጥቅም ላይ ይውላል።【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` `DaManifestV1` እና `ChunkCommitment` ያስተናግዳል፣ ይህም Torii የሚወጣው
  መቆራረጥ ተጠናቋል።【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` የጋራ ተለዋጭ ስሞችን ያቀርባል (`BlobDigest`፣ `RetentionPolicy`፣
  `ErasureProfile`፣ ወዘተ.) እና ከታች በሰነድ የተቀመጡትን ነባሪ የመመሪያ እሴቶችን ኮድ ያደርጋል።【crates/iroha_data_model/src/da/types.rs:240】
- የማኒፌስት ስፑል ፋይሎች በ`config.da_ingest.manifest_store_dir` ውስጥ አርፈዋል፣ ለSoraFS ኦርኬስትራ ዝግጁ
  ወደ ማከማቻ መግቢያ ለመሳብ ጠባቂ።【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi የDA ቅርቅቦችን ሲያሽጉ ወይም ሲያረጋግጡ አንጸባራቂ ተገኝነትን ያስፈጽማል፡-
  ብሎኮች ማጽደቁ አይሳካም ስፑል አንጸባራቂው ከጠፋ ወይም ሃሽ ከተለየ
  ከቁርጠኝነት።【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

ለጥያቄው፣ አንጸባራቂ እና ደረሰኝ የሚጫኑ ጭነቶች የማዞሪያ ጉዞ ሽፋን ተከታትሏል።
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`፣ የNorito ኮዴክን ማረጋገጥ
በሁሉም ዝመናዎች ላይ የተረጋጋ ነው።【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**የማቆየት ነባሪዎች።** አስተዳደር በዚህ ወቅት የመጀመሪያውን የማቆያ ፖሊሲ አጽድቋል
ኤስኤፍ-6; በ `RetentionPolicy::default()` የተተገበሩ ነባሪዎች፡-- ሙቅ ደረጃ: 7 ቀናት (`604_800` ሰከንዶች)
- ቀዝቃዛ ደረጃ: 90 ቀናት (`7_776_000` ሰከንዶች)
- አስፈላጊ ቅጂዎች: `3`
- የማከማቻ ክፍል: `StorageClass::Hot`
- አስተዳደር መለያ: `"da.default"`

የታች ኦፕሬተሮች ሌይን ሲቀበል እነዚህን እሴቶች በግልፅ መሻር አለባቸው
ጥብቅ መስፈርቶች.

## ዝገት ደንበኛ ማረጋገጫ ቅርሶች

የዝገት ደንበኛን የያዙ ኤስዲኬዎች ከአሁን በኋላ ወደ CLI መውጣት አያስፈልጋቸውም።
ቀኖናዊውን የPoR JSON ጥቅል ያዘጋጁ። `Client` ሁለት ረዳቶችን ያጋልጣል፡

- `build_da_proof_artifact` የተፈጠረውን ትክክለኛ መዋቅር ይመልሳል
  `iroha app da prove --json-out`፣ የቀረቡትን አንጸባራቂ/የክፍያ ማብራሪያዎችን ጨምሮ
  በ [`DaProofArtifactMetadata`]።【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` ግንበኛውን ተጠቅልሎ ቅርጹን በዲስክ ላይ ያስቀምጣል።
  (ቆንጆ JSON + ተከታይ አዲስ መስመር በነባሪ) አውቶማቲክ ፋይሉን ማያያዝ ይችላል።
  ለመልቀቅ ወይም ለአስተዳደራዊ ማስረጃዎች ቅርቅቦች።【crates/iroha/src/client.rs:3653】

## ምሳሌ

```rust
use iroha::{
    da::{DaProofArtifactMetadata, DaProofConfig},
    Client,
};

let client = Client::new(config);
let manifest = client.get_da_manifest_bundle(storage_ticket)?;
let payload = std::fs::read("artifacts/da/payload.car")?;
let metadata = DaProofArtifactMetadata::new(
    "artifacts/da/manifest.norito",
    "artifacts/da/payload.car",
);

// Build the JSON artefact in-memory.
let artifact = client.build_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
)?;

// Persist it next to other DA artefacts.
client.write_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
    "artifacts/da/proof_summary.json",
    true,
)?;
```

ረዳትን የሚተው የJSON ክፍያ ጭነት ከ CLI እስከ የመስክ ስሞች ጋር ይዛመዳል
(`manifest_path`፣ `payload_path`፣ `proofs[*].chunk_digest`፣ ወዘተ)፣ ስለዚህ ያለ
አውቶሜሽን ፋይሉን ያለ ቅርጸት-ተኮር ቅርንጫፎች ሊለያይ ይችላል።

## የማረጋገጫ መለኪያ

ከዚህ በፊት በተወካይ ጭነቶች ላይ አረጋጋጭ በጀቶችን ለማረጋገጥ የDA ማረጋገጫ ቤንችማርክን ይጠቀሙ
የማገጃ ደረጃ መያዣዎችን ማጠንከር;

- `cargo xtask da-proof-bench` የጭራሹን ማከማቻ ከማንፀባረቂያ/የመጫኛ ጥንድ፣ ናሙናዎች PoR እንደገና ይገነባል።
  ቅጠሎች፣ እና የጊዜ ማረጋገጫ ከተዋቀረው በጀት ጋር። የታይካይ ሜታዳታ በራስ-የተሞላ ነው፣ እና የ
  የመገጣጠሚያው ጥንድ ወጥነት ከሌለው መታጠቂያው ወደ ሰው ሠራሽ መገለጫ ይመለሳል። መቼ `--payload-bytes`
  ያለ ግልጽ `--payload` ተቀናብሯል፣ የመነጨው ብሎብ የተፃፈው ለ
  `artifacts/da/proof_bench/payload.bin` ስለዚህ ቋሚዎች ሳይነኩ ይቆያሉ።【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- ነባሪውን ወደ `artifacts/da/proof_bench/benchmark.{json,md}` ሪፖርት ያደርጋል እና ማስረጃዎችን/ሩጫን፣ አጠቃላይ እና
  የማረጋገጫ ጊዜዎች፣ የበጀት ማለፊያ መጠን እና የተመከረ በጀት (ከቀዛው ድግግሞሹ 110%)
  መስመር ከ `zk.halo2.verifier_budget_ms`.【artifacts/da/proof_bench/benchmark.md:1】
- የቅርብ ጊዜ ሩጫ (ሠራሽ 1 ሚቢ ጭነት ፣ 64 ኪቢ ቁርጥራጮች ፣ 32 ማረጋገጫዎች/ሩጫ ፣ 10 ድግግሞሾች ፣ 250 ms በጀት)
  በካፒታል ውስጥ 100% ድግግሞሾች የ3 ሚሴ አረጋጋጭ በጀት ይመከራል።【artifacts/da/proof_bench/benchmark.md:1】
- ምሳሌ (የሚወስን ክፍያ ያመነጫል እና ሁለቱንም ሪፖርቶች ይጽፋል)

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

ስብሰባ አግድ ተመሳሳይ በጀቶችን ያስፈጽማል፡ `sumeragi.da_max_commitments_per_block` እና
`sumeragi.da_max_proof_openings_per_block` የ DA ጥቅል በብሎክ ውስጥ ከመካተቱ በፊት በር እና
እያንዳንዱ ቁርጠኝነት ዜሮ ያልሆነ `proof_digest` መያዝ አለበት። ጠባቂው የጥቅል ርዝመትን እንደ እ.ኤ.አ
ግልጽ የሆኑ የማስረጃ ማጠቃለያዎች በስምምነት ውስጥ እስኪጣበቁ ድረስ የመክፈቻ ቆጠራን በማስቀመጥ
≤128-የመክፈቻ ዒላማ በእገዳው ወሰን ላይ ተፈጻሚ ይሆናል።【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## PoR አለመሳካት አያያዝ እና መቁረጥየማጠራቀሚያ ሰራተኞች አሁን ከእያንዳንዱ ጎን የPoR ውድቀትን እና የተሳሰሩ የጭረት ምክሮችን ያሳያሉ
ብይን ከተዋቀረው የምልክት ገደብ በላይ ያሉ ተከታታይ አለመሳካቶች ምክረ ሃሳብ ይሰጣሉ
አቅራቢውን/አንጸባራቂ ጥንዶችን፣ መቆራረጡን የቀሰቀሰው የጅረት ርዝመት እና የታቀደውን ያካትታል
ቅጣት ከአቅራቢው ማስያዣ እና `penalty_bond_bps`; የቀዘቀዘ መስኮቶችን (ሰከንዶች) ያቆዩ
በተመሳሳዩ ክስተት ላይ ከመተኮስ የተባዙ ቁርጥራጮች።

- በማከማቻ ሰራተኛ ገንቢ በኩል ገደቦችን/ማቀዝቀዝ ያዋቅሩ (ነባሪ አስተዳደሩን ያንፀባርቃሉ)
  የቅጣት ፖሊሲ).
- የአስተዳደር/ኦዲተሮች ማያያዝ እንዲችሉ የ Slash ምክሮች በ JSON ማጠቃለያ ላይ ተመዝግበው ይገኛሉ
  ወደ ማስረጃ ጥቅሎች.
- የጭረት አቀማመጥ + በአንድ ክፍል ውስጥ ሚናዎች አሁን በTorii የማከማቻ ፒን መጨረሻ ነጥብ በኩል ተጣብቀዋል
  (`stripe_layout` + `chunk_roles` መስኮች) እና በማከማቻ ሰራተኛው ውስጥ ጸንቷል ስለዚህ
  ኦዲተሮች/የጥገና መሳርያዎች የረድፍ/አምድ ጥገናዎችን ከላይኛው ተፋሰስ ላይ እንደገና ሳያነሱ ማቀድ ይችላሉ

### አቀማመጥ + የጥገና ማሰሪያ

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` አሁን
በ`(index, role, stripe/column, offsets)` ላይ የምደባ ሃሽ ያሰላል እና በመጀመሪያ ረድፍ ይሰራል።
ክፍያውን እንደገና ከመገንባቱ በፊት የአምድ RS(16) ጥገና፡-

- የቦታ አቀማመጥ በነባሪነት ወደ `total_stripes`/`shards_per_stripe` ሲኖር እና ወደ ፍርፋሪ ይመለሳል።
- የጎደሉ / የተበላሹ ቁርጥራጮች በመጀመሪያ ረድፍ እኩልነት እንደገና ይገነባሉ; ቀሪ ክፍተቶች በ ጋር ተስተካክለዋል
  የጭረት (አምድ) እኩልነት. የተስተካከሉ ቁርጥራጮች ወደ chunk directory እና JSON ተመልሰዋል።
  ማጠቃለያ የምደባ ሀሽ እና የረድፍ/አምድ ጥገና ቆጣሪዎችን ይይዛል።
- የረድፍ+አምድ እኩልነት የጎደለውን ስብስብ ማርካት ካልቻለ፣መታጠቂያው በማይመለስበት ፍጥነት ይሳካል።
  ኢንዴክሶች ስለዚህ ኦዲተሮች የማይጠገኑ መገለጫዎችን ይጠቁሙ።