---
lang: am
direction: ltr
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ርዕስ: የውሂብ ተገኝነት Ingest ዕቅድ
sidebar_label: Ingest ዕቅድ
መግለጫ፡ ለTorii ብሎብ ማስገባት እቅድ፣ ኤፒአይ ወለል እና የማረጋገጫ እቅድ።
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# Sora Nexus የውሂብ ተገኝነት ማስገቢያ እቅድ

የተረቀቀው፡ 2026-02-20 - ባለቤት፡ ኮር ፕሮቶኮል WG/የማከማቻ ቡድን/DA WG_

የ DA-2 የስራ ዥረት Toriiን Norito በሚያወጣው የብሎብ ኢንጀስት ኤፒአይ ያራዝመዋል።
ሜታዳታ እና ዘሮች SoraFS ማባዛት። ይህ ሰነድ የታቀደውን ይይዛል
እቅድ፣ ኤፒአይ ወለል እና የማረጋገጫ ፍሰት ስለዚህ ትግበራ ያለሱ መቀጠል ይችላል።
በአስደናቂ ማስመሰያዎች ላይ ማገድ (DA-1 ክትትል)። ሁሉም የመጫኛ ቅርጸቶች የግድ
Norito ኮዴኮችን ይጠቀሙ; ምንም serde/JSON መውደቅ አይፈቀድም።

# ግቦች

- ትላልቅ ነጠብጣቦችን ይቀበሉ (የታይካይ ክፍሎች ፣ የሌይን የጎን መኪናዎች ፣ የአስተዳደር ቅርሶች)
  ከ Torii በላይ መወሰን።
- የብሎብ ፣ የኮዴክ መለኪያዎችን የሚገልጽ ቀኖናዊ Norito ተገለጠ ።
  መገለጫን መደምሰስ እና የማቆየት ፖሊሲ።
- በSoraFS ሙቅ ማከማቻ ውስጥ ቸንክ ሜታዳታን ቀጥል እና የማባዛት ስራዎችን አስይዝ።
- ፒን ኢንቴንስ + የፖሊሲ መለያዎችን ወደ SoraFS መዝገብ እና አስተዳደር ያትሙ
  ታዛቢዎች.
- የመግቢያ ደረሰኞችን በማጋለጥ ደንበኞቻቸው የህትመት ቆራጥነት ማረጋገጫ መልሰው እንዲያገኙ።

## API Surface (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

ክፍያ Norito-የተመሰጠረ `DaIngestRequest` ነው። ምላሾች ይጠቀማሉ
`application/norito+v1` እና `DaIngestReceipt` ተመለስ።

| ምላሽ | ትርጉም |
| --- | --- |
| 202 ተቀባይነት | ብሎብ ለመክተፍ/ለመድገም ተሰልፏል; ደረሰኝ ተመልሷል. |
| 400 መጥፎ ጥያቄ | የመርሃግብር/መጠን ጥሰት (የማረጋገጫ ቼኮችን ይመልከቱ)። |
| 401 ያልተፈቀደ | የጠፋ/ልክ ያልሆነ የኤፒአይ ማስመሰያ። |
| 409 ግጭት | `client_blob_id` ከተዛመደ ሜታዳታ ጋር ያባዛ። |
| 413 ጭነት በጣም ትልቅ | ከተዋቀረ የብሎብ ርዝመት ገደብ አልፏል። |
| 429 በጣም ብዙ ጥያቄዎች | የዋጋ ገደብ ተመቷል። |
| 500 የውስጥ ስህተት | ያልተጠበቀ ውድቀት (የተመዘገበ + ማንቂያ)። |

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
```

> የማስፈጸሚያ ማስታወሻ፡ ለነዚህ የክፍያ ጭነቶች ቀኖናዊው ዝገት ውክልናዎች አሁን ይኖራሉ
> `iroha_data_model::da::types`፣ በ`iroha_data_model::da::ingest` ከጥያቄ/ደረሰኝ መጠቅለያዎች ጋር
> እና በ `iroha_data_model::da::manifest` ውስጥ ያለው አንጸባራቂ መዋቅር።

የ`compression` መስኩ ደዋዮች ክፍያውን እንዴት እንዳዘጋጁ ያስተዋውቃል። Torii ይቀበላል
`identity`፣ `gzip`፣ `deflate`፣ እና `zstd`፣ ከዚህ በፊት ባይት በግልፅ እየቀነሱ
ማሽኮርመም፣ መቆራረጥ እና የአማራጭ መገለጫዎችን ማረጋገጥ።

### የማረጋገጫ ዝርዝር1. ጥያቄን ያረጋግጡ Norito ራስጌ ግጥሚያዎች `DaIngestRequest`።
2. `total_size` ከቀኖናዊው (የተጨመቀ) የመጫኛ ርዝመት ቢለያይ ወይም ከተዋቀረው ከፍተኛው በላይ ከሆነ አይሳካም።
3. የ`chunk_size` አሰላለፍ ያስፈጽሙ (የሁለት ኃይል፣ <= 2 ሚቢ)።
4. `data_shards + parity_shards` <= አለማቀፋዊ ከፍተኛ እና እኩልነት >= 2 ያረጋግጡ።
5. `retention_policy.required_replica_count` የአስተዳደርን መነሻ ማክበር አለበት።
6. ከቀኖናዊ ሃሽ (የፊርማ መስክ በስተቀር) የፊርማ ማረጋገጫ።
7. የተባዛ `client_blob_id` ክፍያ ሃሽ + ሜታዳታ ተመሳሳይ ካልሆነ በስተቀር ውድቅ ያድርጉ።
8. `norito_manifest` ሲቀርብ፣ ሼማ + የሃሽ ግጥሚያዎች እንደገና እንዲሰሉ ያረጋግጡ
   ከተቆረጠ በኋላ ይገለጣል; አለበለዚያ ኖድ አንጸባራቂን ያመነጫል እና ያከማቻል.
9. የተዋቀረውን የማባዛት ፖሊሲን ተግባራዊ ማድረግ፡ Torii የገባውን በድጋሚ ይጽፋል
   `RetentionPolicy` ከ `torii.da_ingest.replication_policy` ጋር (ይመልከቱ)
   `replication-policy.md`) እና ቀድሞ-የተገነቡ የማሳያ መግለጫዎችን ውድቅ ያደርጋል
   ሜታዳታ ከተተገበረው መገለጫ ጋር አይዛመድም።

### የመቁረጥ እና የማባዛት ፍሰት

1. Chunk payload ወደ `chunk_size`፣ BLAKE3 per chunk + Merkle root ያሰሉት።
2. Norito `DaManifestV1` (አዲስ መዋቅር) ቁርጠኝነትን የሚይዝ (ሚና/ቡድን)
   መደምሰስ አቀማመጥ (የረድፍ እና የአምድ እኩልነት ቆጠራዎች እና `ipa_commitment`) ፣ የማቆያ ፖሊሲ ፣
   እና ሜታዳታ።
3. ቀኖናዊ አንጸባራቂ ባይት በ`config.da_ingest.manifest_store_dir` ስር ወረፋ
   (Torii `manifest.encoded` ፋይሎችን በሌይን/ኢፖክ/ተከታታይ/ትኬት/የጣት አሻራ ይጽፋል) ስለዚህ SoraFS
   ኦርኬስትራ እነሱን ወደ ውስጥ ማስገባት እና የማከማቻ ትኬቱን ከቋሚ ውሂብ ጋር ማገናኘት ይችላል።
4. በአስተዳደር መለያ + ፖሊሲ በ`sorafs_car::PinIntent` በኩል ፒን ኢንቴንቶችን ያትሙ።
5. ኢሚት Norito ክስተት `DaIngestPublished` ታዛቢዎችን ለማሳወቅ (ቀላል ደንበኞች፣
   አስተዳደር, ትንታኔ).
6. `DaIngestReceipt`ን ወደ ደዋይ ይመልሱ (በTorii DA አገልግሎት ቁልፍ የተፈረመ) እና ልቀት
   `Sora-PDP-Commitment` ራስጌ ስለዚህ ኤስዲኬዎች የተመሰጠረውን ቁርጠኝነት ወዲያውኑ እንዲይዙ። ደረሰኙ
   አሁን `rent_quote` (a Norito `DaRentQuote`) እና `stripe_layout`ን ያካትታል፣ ይህም አስገቢዎች እንዲታዩ ያስችላቸዋል።
   የመሠረት ኪራይ፣ የተጠባባቂ ድርሻ፣ የPDP/PoTR ጉርሻ የሚጠበቁ፣ እና የ2D መደምሰስ አቀማመጥ ጎን ለጎን
   ገንዘብ ከመውሰዱ በፊት የማከማቻ ትኬቱ.

## ማከማቻ / መዝገብ ቤት ዝመናዎች

- `sorafs_manifest`ን በ`DaManifestV1` ያራዝሙ፣ የሚወስን ትንታኔን በማንቃት።
- አዲስ የመመዝገቢያ ዥረት አክል `da.pin_intent` ከተሰራ የክፍያ ጭነት ማጣቀሻ ጋር
  አንጸባራቂ hash + የቲኬት መታወቂያ።
- የመግቢያ መዘግየትን ለመከታተል ታዛቢነት ያላቸውን ቧንቧዎች አዘምን ፣
  ማባዛት ወደኋላ, እና ውድቀት ይቆጠራል.

## የሙከራ ስልት

- የንድፍ ማረጋገጫ፣ የፊርማ ቼኮች፣ የተባዛ ፈልጎ ለማግኘት የክፍል ሙከራዎች።
- ወርቃማ ሙከራዎች Norito የ`DaIngestRequest` ኢንኮዲንግ ፣ማሳያ እና ደረሰኝ የሚያረጋግጡ።
- የውህደት ማሰሪያ የሚሽከረከር እስከ መሳለቂያ SoraFS + መዝገብ ቤት፣ ቸንክ + ፒን ፍሰቶችን ያረጋግጣል።
- የዘፈቀደ የመደምሰስ መገለጫዎችን እና የማቆየት ውህዶችን የሚሸፍኑ የንብረት ሙከራዎች።
- ከተበላሸ ሜታዳታ ለመጠበቅ የNorito ጭነቶች ማደብዘዝ።

## CLI እና ኤስዲኬ መሣሪያ (DA-8)- `iroha app da submit` (አዲስ የ CLI መግቢያ ነጥብ) አሁን የተጋራውን ኢንጀስት ግንበኛ/አሳታሚ ይጠቀለላል ስለዚህ ኦፕሬተሮች
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
  `--plan`፣ `--manifest-id`) **ወይም** የTorii የማከማቻ ትኬት በNorito ማለፍ። ቲኬቱ ሲወጣ
  ዱካ ጥቅም ላይ ይውላል CLI አንጸባራቂውን ከ Norito ይጎትታል፣ ጥቅሉ ከስር ይቀጥላል።
  `artifacts/da/fetch_<timestamp>/` (በ`--manifest-cache-dir` መሻር)፣ የብሎብ ሃሽ ለ
  `--manifest-id`፣ እና ኦርኬስትራውን በቀረበው `--gateway-provider` ዝርዝር ያካሂዳል። ሁሉም
  የላቁ ቁልፎች ከ SoraFS ፈልሳፊው ገጽ ሳይነካ (ግልጽ የሆኑ ፖስታዎች፣ የደንበኛ መለያዎች፣ የጥበቃ መሸጎጫዎች፣
  ስም-አልባ ማጓጓዣ ይሽራል፣ የውጤት ሰሌዳ ወደ ውጪ መላክ እና `--output` መንገዶች)፣ እና የገለጻው የመጨረሻ ነጥብ
  ለበጁ Torii አስተናጋጆች በ`--manifest-endpoint` በኩል ይሰረዛሉ፣ ስለዚህ ከጫፍ እስከ ጫፍ ያለው ተገኝነት በቀጥታ ይፈትሻል
  የኦርኬስትራ ሎጂክን ሳያባዙ ሙሉ በሙሉ በ`da` የስም ቦታ ስር።
- `iroha app da get-blob` ቀኖናዊ መግለጫዎችን በቀጥታ ከTorii በ `GET /v1/da/manifests/{storage_ticket}` ይጎትታል።
  ትዕዛዙ `manifest_{ticket}.norito`፣ `manifest_{ticket}.json` እና `chunk_plan_{ticket}.json` ይጽፋል።
  በ `artifacts/da/fetch_<timestamp>/` (ወይም በተጠቃሚ የቀረበ `--output-dir`) ትክክለኛውን እያስተጋባ
  `iroha app da get` ጥሪ (`--manifest-id`ን ጨምሮ) ለተከታዮቹ ኦርኬስትራ ማምጣት ያስፈልጋል።
  ይህ ኦፕሬተሮችን ከማንፀባረቂያው የስፑል ማውጫዎች እንዲወጡ ያደርጋቸዋል እና ፈላጊው ሁልጊዜም እንደሚጠቀም ዋስትና ይሰጣል
  በTorii የተለቀቁ የተፈረሙ ቅርሶች። የጃቫ ስክሪፕት Torii ደንበኛ ይህንን ፍሰት ያንጸባርቃል
  `ToriiClient.getDaManifest(storageTicketHex)`፣ ዲኮድ የተደረገውን Norito ባይት፣ አንጸባራቂ JSON፣
  የኤስዲኬ ደዋዮች የኦርኬስትራ ክፍለ-ጊዜዎችን ወደ CLI ሳይተኩሱ እንዲረጩ ለማድረግ ቸንክ እቅድ።
  ስዊፍት ኤስዲኬ አሁን ተመሳሳይ ንጣፎችን ያጋልጣል (`ToriiClient.getDaManifestBundle(...)` plus)
  `fetchDaPayloadViaGateway(...)`)፣ ቧንቧዎችን ወደ ቤተኛ SoraFS ኦርኬስትራ መጠቅለያ ስለዚህ
  የiOS ደንበኞች ማኒፌክቶችን ማውረድ፣ ባለብዙ ምንጭ ፈልጎዎችን ማከናወን እና ያለሱ ማስረጃዎችን ማንሳት ይችላሉ።CLIን በመጥራት።【IrohaSwift/ምንጮች/IrohaSwift/ToriiClient.swift:240】【IrohaSwift/ምንጮች/IrohaSwift/SorafsOrchestrator ደንበኛ.swift:12】
- `iroha app da rent-quote` ለቀረበው የማከማቻ መጠን የሚወስን የቤት ኪራይ እና የማበረታቻ ክፍተቶችን ያሰላል።
  እና የማቆያ መስኮት. ረዳቱ ንቁውን `DaRentPolicyV1` (JSON ወይም Norito ባይት) ወይም
  አብሮ የተሰራው ነባሪ፣ ፖሊሲውን ያጸድቃል እና የJSON ማጠቃለያ ያትማል (`gib`፣ `months`፣ የመመሪያ ሜታዳታ፣
  እና `DaRentQuote` መስኮች) ስለዚህ ኦዲተሮች በትክክል የXOR ክፍያዎችን በአስተዳደር ደቂቃዎች ውስጥ ሊጠቅሱ ይችላሉ
  ማስታወቂያ ሆክ ስክሪፕቶችን መጻፍ። ትዕዛዙ ከJSON በፊት አንድ መስመር `rent_quote ...` ማጠቃለያ ያወጣል።
  በአደጋ ልምምዶች ወቅት የኮንሶል ምዝግብ ማስታወሻዎች እንዲነበቡ ለማድረግ ጭነት። `--quote-out artifacts/da/rent_quotes/<stamp>.json` ያጣምሩ
  `--policy-label "governance ticket #..."` ትክክለኛ የፖሊሲ ድምጽን የሚጠቅሱ ቆንጆ ቅርሶችን ለመቀጠል
  ወይም ማዋቀር ጥቅል; CLI ብጁ መለያውን ጠርጎ ባዶ ሕብረቁምፊዎችን ውድቅ ያደርጋል ስለዚህ `policy_source` እሴቶች
  በግምጃ ቤት ዳሽቦርዶች ላይ ተግባራዊ ሊሆኑ የሚችሉ እንደሆኑ ይቆያሉ። ለትዕዛዙ `crates/iroha_cli/src/commands/da.rs` ይመልከቱ
  እና `docs/source/da/rent_policy.md` ለፖሊሲው እቅድ።【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/ rent_policy.md:1】
- `iroha app da prove-availability` ሁሉንም ከላይ ያሉትን ሰንሰለቶች ያሰራል፡ የማከማቻ ትኬት ይወስዳል፣ ያውርዳል
  ቀኖናዊ አንጸባራቂ ጥቅል፣ ባለብዙ ምንጭ ኦርኬስትራውን (`iroha app sorafs fetch`) ከ
  የቀረበው `--gateway-provider` ዝርዝር፣ የወረደውን የክፍያ ጭነት + የውጤት ሰሌዳ ይቀጥላል።
  `artifacts/da/prove_availability_<timestamp>/`፣ እና ወዲያውኑ ያለውን የPoR አጋዥን ይጠራል
  (`iroha app da prove`) የተገኙትን ባይት በመጠቀም። ኦፕሬተሮች የኦርኬስትራ ቁልፎችን ማስተካከል ይችላሉ።
  (`--max-peers`፣ `--scoreboard-out`፣ አንጸባራቂ የመጨረሻ ነጥብ ይሽራል) እና የማረጋገጫ ናሙና
  (`--sample-count`፣ `--leaf-index`፣ `--sample-seed`) አንድ ትዕዛዝ ግን ቅርሶቹን ሲያመርት
  በDA-5/DA-9 ኦዲቶች የሚጠበቀው፡ የመጫኛ ኮፒ፣ የውጤት ሰሌዳ ማስረጃ እና የJSON ማረጋገጫ ማጠቃለያዎች።

## TODO የጥራት ማጠቃለያ

ሁሉም ከዚህ ቀደም የታገዱ TODOዎች ተተግብረዋል እና ተረጋግጠዋል፡- ** የመጭመቅ ፍንጮች *** - Torii በጠሪዎች የቀረቡ መለያዎችን ይቀበላል (`identity`፣ `gzip`፣ `deflate`፣
  `zstd`) እና ከመጽደቁ በፊት የሚጫኑ ጭነቶችን መደበኛ ያደርጋል ስለዚህም ቀኖናዊው አንጸባራቂ ሃሽ ከሚከተሉት ጋር ይዛመዳል።
  የተቀነሰ ባይት።【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **የመንግስት-ብቻ ሜታዳታ ምስጠራ** — Torii አሁን የአስተዳደር ዲበ ውሂብን ከ
  የChaCha20-Poly1305 ቁልፍ ተዋቅሯል፣ ያልተዛመደ መለያዎችን ውድቅ ያደርጋል እና ሁለት ግልፅ አድርጓል።
  የማዋቀር ቁልፎች (`torii.da_ingest.governance_metadata_key_hex`፣
  `torii.da_ingest.governance_metadata_key_label`) የማሽከርከር ቆራጥነትን ለመጠበቅ።【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- ** ትልቅ የክፍያ ጭነት ዥረት ** - ባለብዙ ክፍል ማስገቢያ በቀጥታ ነው። የደንበኞች ዥረት መወሰኛ
  `DaIngestChunk` በ `client_blob_id` ፣ Torii የተከፈቱ ኤንቨሎፖች እያንዳንዱን ቁራጭ ያረጋግጣሉ ፣ ደረጃቸውንም ያዘጋጃሉ
  በ `manifest_store_dir` ስር፣ እና በአቶሚካል መግለጫውን አንዴ የ `is_last` ባንዲራ መሬት ይገነባል።
  በነጠላ ጥሪ ሰቀላዎች የታዩትን የ RAM ፍንጮችን በማስወገድ ላይ።【crates/iroha_torii/src/da/ingest.rs:392】
- **አሳያ ሥሪት** — `DaManifestV1` ግልጽ የሆነ የ`version` መስክ ይይዛል እና Torii እምቢ አለ።
  ያልታወቁ ስሪቶች፣ አዲስ አንጸባራቂ አቀማመጦች ሲርከብ የሚወስኑ ማሻሻያዎችን ዋስትና ይሰጣል።【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR መንጠቆዎች *** - የ PDP ቁርጠኝነት በቀጥታ ከ chunk መደብር የተገኙ እና ጸንተዋል
  የDA-5 መርሐግብር አውጪዎች ከቀኖናዊ መረጃዎች የናሙና ፈተናዎችን እንዲጀምሩ ከማስረጃው በተጨማሪ
  `/v1/da/ingest` እና `/v1/da/manifests/{ticket}` አሁን የ `Sora-PDP-Commitment` ራስጌን ያካትታል
  የመሠረት 64 Norito ጭነትን ተሸክሞ ኤስዲኬዎች ትክክለኛውን ቁርጠኝነት DA-5 የመመርመሪያ ኢላማውን ያሸሻሉ ።

## የትግበራ ማስታወሻዎች- የ Torii's `/v1/da/ingest` የመጨረሻ ነጥብ አሁን የደመወዝ ጭነት መጨናነቅን መደበኛ ያደርገዋል፣ የመልሶ ማጫወት መሸጎጫውን ያስፈጽማል፣
  ቀኖናዊ ባይቶችን በቆራጥነት ቆርጧል፣ `DaManifestV1`ን እንደገና ገነባ፣ የተመሰጠረውን ጭነት ይጥላል።
  ወደ `config.da_ingest.manifest_store_dir` ለ SoraFS ኦርኬስትራ፣ እና `Sora-PDP-Commitment` ይጨምራል
  ርዕስ ስለዚህ ኦፕሬተሮች የ PDP መርሐግብር አውጪዎች የሚጠቅሱትን ቁርጠኝነት ይይዛሉ።【crates/iroha_torii/src/da/ingest.rs:220】
- እያንዳንዱ ተቀባይነት ያለው ብሎብ አሁን `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` ያወጣል።
  በ `manifest_store_dir` ስር መግባት ቀኖናዊውን `DaCommitmentRecord` ከጥሬው ጋር በማያያዝ
  `PdpCommitmentV1` ባይት ስለዚህ DA-3 የጥቅል ግንበኞች እና DA-5 መርሐግብር አውጪዎች ተመሳሳይ ግብዓቶችን ያለ ምንም ውሃ ያጠጣሉ።
  አንጸባራቂዎችን ወይም የሱቅ መደብሮችን እንደገና ማንበብ።【crates/iroha_torii/src/da/ingest.rs:1814】
- የኤስዲኬ አጋዥ ኤፒአይዎች ደዋዮች Norito መፍታትን እንደገና እንዲፈጽሙ ሳያስገድዱ የ PDP ራስጌ ክፍያን ያጋልጣሉ፡
  የ Rust crate `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}`, Python ን ወደ ውጭ ይልካል
  `ToriiClient` አሁን `decode_pdp_commitment_header` እና `IrohaSwift` መርከቦችን ያጠቃልላል
  `decodePdpCommitmentHeader` ከመጠን በላይ ጭነቶች ለጥሬ ራስጌ ካርታዎች ወይም `HTTPURLResponse` አጋጣሚዎች.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/ምንጮች/IrohaSwift/ToriiClient.swift:1】
- Torii በተጨማሪም `GET /v1/da/manifests/{storage_ticket}` ያጋልጣል ስለዚህ ኤስዲኬዎች እና ኦፕሬተሮች ማኒፌክቶችን ማምጣት ይችላሉ።
  እና የመስቀለኛ መንገዱን የስፑል ማውጫ ሳይነኩ ቸንክ እቅዶች። ምላሹ Norito ባይት ይመልሳል
  (ቤዝ64)፣ የተሰራው አንጸባራቂ JSON፣ `chunk_plan` JSON blob ለ`sorafs fetch` ዝግጁ ነው፣ ተገቢነቱ
  ሄክስ ዳይጀስት (`storage_ticket`፣ `client_blob_id`፣ `blob_hash`፣ `chunk_root`)፣ እና መስተዋቱን ያሳያል።
  `Sora-PDP-Commitment` ራስጌ ለተመጣጣኝ ምላሾች ከመግባት። በ ውስጥ `block_hash=<hex>` በማቅረብ ላይ
  የመጠይቅ ሕብረቁምፊ የሚወስን `sampling_plan` ይመልሳል (የመመደብ ሃሽ፣ `sample_window`፣ እና ናሙና የተደረገ
  `(index, role, group)` tuples ሙሉውን 2D አቀማመጥ የሚሸፍኑ) ስለዚህ አረጋጋጮች እና የPoR መሳሪያዎች ተመሳሳይ ይሳሉ
  ኢንዴክሶች.

### ትልቅ የክፍያ ጭነት ፍሰት ፍሰት

ከተዋቀረው የነጠላ ጥያቄ ገደብ በላይ የሆኑ ንብረቶችን ማስገባት የሚፈልጉ ደንበኞች ሀ
የዥረት ክፍለ ጊዜ `POST /v1/da/ingest/chunk/start` በመደወል። Torii በ ሀ
`ChunkSessionId` (BLAKE3-ከተጠየቀው የብሎብ ሜታዳታ የተገኘ) እና የተደራደረው ቸንክ መጠን።
እያንዳንዱ ተከታይ የ`DaIngestChunk` ጥያቄ የሚከተሉትን ይይዛል፡-

- `client_blob_id` - ከመጨረሻው `DaIngestRequest` ጋር ተመሳሳይ ነው።
- `chunk_session_id` - ቁርጥራጮቹን ከሩጫው ክፍለ ጊዜ ጋር ያገናኛል።
- `chunk_index` እና `offset` - የሚወስን ቅደም ተከተል ያስፈጽሙ።
- `payload` - እስከ ድርድር ቻንክ መጠን ድረስ።
- `payload_hash` — BLAKE3 የቁርጭምጭሚቱ ሃሽ ስለዚህ Torii ሙሉውን ብሎብ ሳያስቀር ማረጋገጥ ይችላል።
- `is_last` - የተርሚናል ቁራጭን ያመለክታል።Torii በ `config.da_ingest.manifest_store_dir/chunks/<session>/` እና
ድህነትን ለማክበር በድጋሚ አጫውት መሸጎጫ ውስጥ ያለውን ሂደት ይመዘግባል። የመጨረሻው ቁራጭ ሲያርፍ፣ Torii
ክፍያውን በዲስክ ላይ እንደገና ይሰበስባል (የማስታወሻ እጢዎችን ለማስወገድ በ chunk directory በኩል በዥረት መልቀቅ)
ቀኖናዊውን አንጸባራቂ/ደረሰኝ ልክ በነጠላ ምት ሰቀላ ያሰላል እና በመጨረሻም ምላሽ ይሰጣል
`POST /v1/da/ingest` የተደረደሩትን ቅርሶች በመብላት። ያልተሳኩ ክፍለ ጊዜዎች በግልጽ ሊሰረዙ ይችላሉ ወይም
ከ `config.da_ingest.replay_cache_ttl` በኋላ በቆሻሻ የተሰበሰቡ ናቸው። ይህ ንድፍ የአውታረ መረብ ቅርጸቱን ያስቀምጣል
Norito-ተስማሚ፣ ደንበኛ-ተኮር የሆኑ ድጋሚ ሊደረጉ የሚችሉ ፕሮቶኮሎችን ያስወግዳል፣ እና ያለውን አንጸባራቂ ቧንቧ እንደገና ይጠቀማል።
ያልተለወጠ.

** የትግበራ ሁኔታ።** ቀኖናዊው Norito ዓይነቶች አሁን ይኖራሉ
`crates/iroha_data_model/src/da/`፡

- `ingest.rs` `DaIngestRequest`/`DaIngestReceipt` ይገልጻል
  `ExtraMetadata` መያዣ በTorii ጥቅም ላይ የዋለ።【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` `DaManifestV1` እና `ChunkCommitment` ያስተናግዳል፣ይህም Torii የሚወጣው
  መቆራረጥ ተጠናቋል።【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` የጋራ ተለዋጭ ስሞችን ያቀርባል (`BlobDigest`፣ `RetentionPolicy`፣
  `ErasureProfile`፣ ወዘተ.) እና ከታች በሰነድ የተቀመጡትን ነባሪ የመመሪያ እሴቶችን ኮድ ያደርጋል።【crates/iroha_data_model/src/da/types.rs:240】
- የማኒፌስት ስፑል ፋይሎች በ`config.da_ingest.manifest_store_dir` ውስጥ አርፈዋል፣ ለSoraFS ኦርኬስትራ ዝግጁ
  ወደ ማከማቻ መግቢያ ለመሳብ ጠባቂ።【crates/iroha_torii/src/da/ingest.rs:220】

ለጥያቄው፣ አንጸባራቂ እና ደረሰኝ የሚጫኑ ጭነቶች የማዞሪያ ጉዞ ሽፋን ተከታትሏል።
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`፣ የ Norito ኮዴክን ማረጋገጥ
በሁሉም ዝመናዎች ላይ የተረጋጋ ነው።【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**የማቆየት ነባሪዎች።** አስተዳደር በዚህ ወቅት የመጀመሪያውን የማቆያ ፖሊሲ አጽድቋል
ኤስኤፍ-6; በ `RetentionPolicy::default()` የተተገበሩ ነባሪዎች፡-

- ትኩስ ደረጃ፡ 7 ቀናት (`604_800` ሰከንድ)
- ቀዝቃዛ ደረጃ: 90 ቀናት (`7_776_000` ሰከንዶች)
- አስፈላጊ ቅጂዎች: `3`
- የማከማቻ ክፍል: `StorageClass::Hot`
- አስተዳደር መለያ: `"da.default"`

የታች ኦፕሬተሮች ሌይን ሲቀበል እነዚህን እሴቶች በግልፅ መሻር አለባቸው
ጥብቅ መስፈርቶች.