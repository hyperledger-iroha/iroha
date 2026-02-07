---
lang: am
direction: ltr
source: docs/source/da/commitments_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ea1b16b73a55e3e47dfe9d5bfc77dedce2e8fa9ff964d244856767f14931733
source_last_modified: "2026-01-22T14:45:02.095688+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus የውሂብ ተገኝነት ግዴታዎች እቅድ (DA-3)

የተረቀቀው፡ 2026-03-25 — ባለቤቶች፡ ኮር ፕሮቶኮል WG / ስማርት ኮንትራት ቡድን / የማከማቻ ቡድን_

DA-3 የNexus የማገጃ ቅርጸትን ያራዝማል ስለዚህ እያንዳንዱ መስመር ወሳኝ መዝገቦችን ይከተታል
በDA-2 የተቀበሉትን ብሎቦች በመግለጽ። ይህ ማስታወሻ ቀኖናዊውን መረጃ ይይዛል
አወቃቀሮች፣ የቧንቧ መስመር መንጠቆዎች፣ የብርሃን ደንበኛ ማረጋገጫዎች፣ እና Torii/RPC ንጣፎች
አረጋጋጮች በዲኤ ቃል ኪዳኖች ላይ ከመመካታቸው በፊት ማረፍ ያለበት በመግቢያ ጊዜ ወይም
የአስተዳደር ቼኮች. ሁሉም የክፍያ ጭነቶች Norito-የተመሰጠሩ ናቸው; ምንም SCALE ወይም ad-hoc JSON.

# አላማዎች

- ቃል ኪዳኖችን ያካሂዱ ( ቸንክ ስር + ግልጽ ሃሽ + አማራጭ KZG
  ቁርጠኝነት) በእያንዳንዱ Nexus ብሎኮች ውስጥ እኩዮች ተገኝነትን እንደገና እንዲገነቡ
  ከደብዳቤ ውጭ ማከማቻን ሳያማክሩ ግዛት።
- ቀላል ደንበኞች ማረጋገጥ እንዲችሉ ቆራጥ የአባልነት ማረጋገጫዎችን ያቅርቡ ሀ
  መግለጫ ሃሽ በተሰጠው ብሎክ ተጠናቀቀ።
- የTorii መጠይቆችን (`/v1/da/commitments/*`) እና ሪሌሎችን የሚፈቅዱ ማረጋገጫዎችን ያጋልጡ።
  ኤስዲኬዎች፣ እና የአስተዳደር አውቶሜሽን ኦዲት መገኘት እያንዳንዱን ሳይደግሙ
  አግድ
- አዲሱን በክር በማድረግ ያለውን `SignedBlockWire` ፖስታ ቀኖናዊ ያድርጉት
  በNorito ሜታዳታ ራስጌ በኩል ያሉ መዋቅሮች እና የሃሽ አመጣጥን አግድ።

## ወሰን አጠቃላይ እይታ

1. ** የውሂብ ሞዴል ተጨማሪዎች ** በ `iroha_data_model::da::commitment` plus block
   የርዕስ ለውጦች በ `iroha_data_model::block`.
2. **አስፈፃሚ መንጠቆዎች** ስለዚህ `iroha_core` በTorii የወጡትን DA ደረሰኞች አስገብቷል።
   (`crates/iroha_core/src/queue.rs` እና `crates/iroha_core/src/block.rs`)።
3. ** ጽናት/ኢንዴክሶች** ስለዚህ WSV የቁርጠኝነት ጥያቄዎችን በፍጥነት መመለስ ይችላል።
   (`iroha_core/src/wsv/mod.rs`)።
4. **Torii RPC ተጨማሪዎች** ለዝርዝር/ጥያቄ/በስር ያሉ የመጨረሻ ነጥቦችን ያረጋግጡ
   `/v1/da/commitments`.
5. ** የውህደት ሙከራዎች + ቋሚዎች ** የሽቦውን አቀማመጥ የሚያረጋግጡ እና የማረጋገጫ ፍሰት ወደ ውስጥ
   `integration_tests/tests/da/commitments.rs`.

## 1. የውሂብ ሞዴል ተጨማሪዎች

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` አሁን ያለውን ባለ 48-ባይት ነጥብ ከዚህ በታች ጥቅም ላይ ይውላል
  `iroha_crypto::kzg`. Merkle መስመሮች ባዶ መተው; `kzg_bls12_381` መስመሮች አሁን
  ቆራጥ የሆነ BLAKE3-XOF ቁርጠኝነት ከ chunk root የተገኘ እና ያግኙ
  የማጠራቀሚያ ትኬት ስለዚህ hashes ያለ ውጫዊ prover ተረጋግተው እንዲቆዩ።
- `proof_scheme` ከሌይን ካታሎግ የተገኘ ነው; የመርክል መስመሮች የባዘነውን KZG ውድቅ ያደርጋሉ
  የሚጫኑ ጭነቶች `kzg_bls12_381` መስመሮች ዜሮ ያልሆኑ KZG ግዴታዎችን ይፈልጋሉ።
- `proof_digest` የ DA-5 PDP/PoTR ውህደትን ይጠብቃል ስለዚህም ተመሳሳይ መዝገብ
  ነጠብጣቦችን በቀጥታ ለማቆየት ጥቅም ላይ የዋለውን የናሙና መርሃ ግብር ይዘረዝራል።

### 1.2 የራስጌ ቅጥያ አግድ

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

የጥቅል ሃሽ በሁለቱም የብሎክ hash እና `SignedBlockWire` ሜታዳታ ይመገባል።
በላይ።

የማስፈጸሚያ ማስታወሻ፡ `BlockPayload` እና ግልጽ የሆነው `BlockBuilder` አሁን ተጋልጧል
`da_commitments` ሴተሮች/ጌተርስ (`BlockBuilder::set_da_commitments` ይመልከቱ እና
`SignedBlock::set_da_commitments`)፣ ስለዚህ አስተናጋጆች አስቀድሞ የተሰራ ጥቅል ማያያዝ ይችላሉ።
እገዳ ከመዘጋቱ በፊት. ሁሉም አጋዥ ገንቢዎች መስኩን ወደ `None` ነባሪ አድርገውታል።
እስከ Torii ክሮች እውነተኛ ቅርቅቦች ውስጥ።

### 1.3 ሽቦ ኢንኮዲንግ- `SignedBlockWire::canonical_wire()` የ Norito ራስጌን ለ
  `DaCommitmentBundle` አሁን ካለው የግብይት ዝርዝር በኋላ። የ
  ባይት ስሪት `0x01` ነው።
- `SignedBlockWire::decode_wire()` `version` የማይታወቅ ጥቅሎችን ውድቅ ያደርጋል፣
  በ`norito.md` ውስጥ ከተገለጸው የNorito ፖሊሲ ጋር የሚዛመድ።
- የሃሽ መነጫነጭ ዝማኔዎች በ`block::Hasher` ውስጥ ብቻ ይኖራሉ። ብርሃን ደንበኞች መፍታት
  አሁን ያለው የሽቦ ቅርፀት አዲሱን መስክ በራስ-ሰር ያገኛል ምክንያቱም Norito
  ራስጌ መገኘቱን ያስታውቃል።

## 2. የምርት ፍሰትን አግድ

1. Torii DA መግባቱ የተፈረመ ደረሰኞች እና የቁርጠኝነት መዝገቦች በ
   DA spool (`da-receipt-*.norito` / `da-commitment-*.norito`)። ዘላቂው
   ደረሰኝ የምዝግብ ማስታወሻ ዘሮች ጠቋሚዎች እንደገና ሲጀመሩ እንደገና የተጫወቱ ደረሰኞች አሁንም ታዝዘዋል
   በቆራጥነት።
2. የስብሰባ ደረሰኞችን ከስፖሉ ላይ ይጭናል፣ ያረጀ/ አስቀድሞ የታሸገ ይጥላል
   የገባውን የጠቋሚ ቅጽበታዊ ገጽ እይታ በመጠቀም ግቤቶችን፣ እና contiguity per
   `(lane, epoch)`. ሊደረስበት የሚችል ደረሰኝ ተዛማጅ ቁርጠኝነት ከሌለው ወይም የ
   አንጸባራቂ hash በጸጥታ ከማስቀረት ይልቅ ፕሮፖዛሉን ያስወርዳል።
3. ልክ ከመታተሙ በፊት ገንቢው የቃል ኪዳኑን ጥቅል ለ
   በደረሰኝ የሚመራ ስብስብ፣ በ`(lane_id, epoch, sequence)` ደርድር፣
   ከ Norito ኮዴክ ጋር ያዋህዱ እና `da_commitments_hash` ያዘምናል።
4. ሙሉው ጥቅል በ WSV ውስጥ ተከማችቶ ከውስጥ ካለው እገዳ ጋር አብሮ ይወጣል
   `SignedBlockWire`; የተፈፀሙ ጥቅሎች ደረሰኝ ጠቋሚዎችን ያራምዳሉ (hydrated
   ከኩራ እንደገና ሲጀመር) እና ከዲስክ እድገት ጋር የታሰሩ የቆዩ ስፖሎች ግቤቶችን ይከርክሙ።

ስብሰባን አግድ እና `BlockCreated` መግባቱ እያንዳንዱን ቃል ኪዳን በድጋሚ ያረጋግጣል
የሌይን ካታሎግ፡ የመርክል መስመሮች የ KZG ቃል ኪዳኖችን አይቀበሉም፣ KZG መስመሮች ያስፈልጋቸዋል
ዜሮ ያልሆኑ KZG ቁርጠኝነት እና ዜሮ ያልሆኑ `chunk_root`፣ እና ያልታወቁ መስመሮች
ወረደ። የTorii's `/v1/da/commitments/verify` የመጨረሻ ነጥብ አንድ አይነት ጠባቂ ያንጸባርቃል፣
እና አሁን ወሳኙን የKZG ቁርጠኝነት ወደ እያንዳንዱ አስገባ
`kzg_bls12_381` መዝገብ ስለዚህ ፖሊሲን የሚያከብሩ ጥቅሎች የማገጃ ስብሰባ ላይ ደርሰዋል።

በDA-2 የመግቢያ እቅድ ውስጥ የተገለጹት የማሳያ መሳሪያዎች እንደ ምንጭ በእጥፍ ይጨምራሉ
እውነት ለቁርጠኝነት ጥቅል። የ Torii ፈተና
`manifest_fixtures_cover_all_blob_classes` ለእያንዳንዱ ሰው ይገለጣል
`BlobClass` ተለዋጭ እና አዳዲስ ክፍሎች መጫዎቻዎች እስኪያገኙ ድረስ ለመሰብሰብ ፈቃደኛ አልሆነም።
በእያንዳንዱ `DaCommitmentRecord` ውስጥ የተመሰከረው አንጸባራቂ ሃሽ ከሚከተሉት ጋር እንደሚዛመድ ማረጋገጥ
golden Norito/JSON ጥንድ።【crates/iroha_torii/src/da/tess.rs:2902】

እገዳ መፍጠር ካልተሳካ ደረሰኞች በወረፋው ውስጥ ይቀራሉ ስለዚህ ቀጣዩ እገዳ
ሙከራ እነሱን ማንሳት ይችላል; ግንበኛ `sequence` በአንድ የተካተተውን የመጨረሻውን ይመዘግባል
ተደጋጋሚ ጥቃቶችን ለማስወገድ መስመር።

## 3. RPC & Query Surface

Torii ሶስት የመጨረሻ ነጥቦችን ያጋልጣል፡| መስመር | ዘዴ | ጭነት | ማስታወሻ |
|-------|--------|-----|------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (የክልል ማጣሪያ በሌይን/ኢፖክ/ተከታታይ፣ pagination) | `DaCommitmentPage` በጠቅላላ ቆጠራ፣ ቃል ኪዳኖች እና ሃሽ አግድ ይመልሳል። |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (ሌይን + ግልጽ ሃሽ ወይም `(epoch, sequence)` tuple)። | በ`DaCommitmentProof` (መዝገብ + Merkle መንገድ + እገዳ ሃሽ) ምላሽ ይሰጣል። |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | የብሎክ ሃሽ ስሌትን የሚደግም እና ማካተትን የሚያረጋግጥ ሀገር አልባ ረዳት ፤ በቀጥታ ወደ `iroha_crypto` ማገናኘት በማይችሉ ኤስዲኬዎች ጥቅም ላይ ይውላል። |

ሁሉም የክፍያ ጭነቶች በ `iroha_data_model::da::commitment` ስር ይኖራሉ። Torii ራውተሮች ተራራ
ቶከን/ኤምቲኤልኤስን እንደገና ለመጠቀም ከነባሩ DA ቀጥሎ ያሉት ተቆጣጣሪዎች የመጨረሻ ነጥቦችን ያስገባሉ።
ፖሊሲዎች.

## 4. የማካተት ማረጋገጫዎች እና ቀላል ደንበኞች

- የማገጃ አምራቹ በተከታታዩ ላይ ሁለትዮሽ የመርክል ዛፍ ይገነባል።
  `DaCommitmentRecord` ዝርዝር. ሥሩ `da_commitments_hash` ይመገባል።
- `DaCommitmentProof` የዒላማውን ሪከርድ እና የ`(ወንድም_እህት_ሃሽ፣
  አቀማመጥ)` ግቤቶች ስለዚህ አረጋጋጮች ሥሩን እንደገና መገንባት ይችላሉ። ማስረጃዎችም ያካትታሉ
  ቀላል ደንበኞች የመጨረሻነቱን ማረጋገጥ እንዲችሉ ሃሽ እና የተፈረመ ራስጌ።
- የ CLI ረዳቶች (`iroha_cli app da prove-commitment`) የማስረጃ ጥያቄውን ያጠቃልላሉ/ ያረጋግጣሉ
  ዑደት እና ወለል Norito/ሄክስ ውጤቶች ኦፕሬተሮች.

## 5. ማከማቻ እና መረጃ ጠቋሚ

WSV በ`manifest_hash` ቁልፍ በተሰየመ አምድ ቤተሰብ ውስጥ ቃል ኪዳኖችን ያከማቻል።
የሁለተኛ ደረጃ ኢንዴክሶች `(lane_id, epoch)` እና `(lane_id, sequence)` ይሸፍናሉ ስለዚህ መጠይቆች
ሙሉ ጥቅሎችን መቃኘትን ያስወግዱ። እያንዳንዱ መዝገብ የታሸገውን የማገጃ ቁመት ይከታተላል ፣
የተያዙ አንጓዎች መረጃ ጠቋሚውን ከብሎክ ሎግ በፍጥነት እንዲገነቡ ማድረግ።

## 6. ቴሌሜትሪ እና ታዛቢነት

- `torii_da_commitments_total` ጭማሪዎች ቢያንስ አንድ ብሎክ ባዘጋ ቁጥር
  መዝገብ.
- `torii_da_commitment_queue_depth` ለመጠቅለል የሚጠብቁ ደረሰኞችን ይከታተላል (በየ
  መስመር)።
- Grafana ዳሽቦርድ `dashboards/grafana/da_commitments.json` ምስላዊ አግድ
  DA-3 የመልቀቂያ በሮች ኦዲት ማድረግ እንዲችሉ ማካተት፣ የወረፋ ጥልቀት እና የማስረጃ ፍሰት
  ባህሪ.

## 7. የሙከራ ስልት

1. **የዩኒት ሙከራዎች** ለ`DaCommitmentBundle` ኢንኮዲንግ/መግለጽ እና ሃሽ ማገድ
   የመነሻ ዝማኔዎች.
2. **ወርቃማ እቃዎች** በ`fixtures/da/commitments/` ስር ቀኖናዊ እየያዙ
   ጥቅል ባይት እና የመርክሌ ማረጋገጫዎች። እያንዳንዱ ጥቅል አንጸባራቂ ባይት ይጠቅሳል
   ከ `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}`, ስለዚህ
   `cargo test -p iroha_torii regenerate_da_ingest_fixtures -- --ignored --nocapture` በማደስ ላይ
   `ci/check_da_commitments.sh` ቁርጠኝነትን ከማደስዎ በፊት የNorito ታሪክን ወጥነት ያለው ያደርገዋል።
   ማረጋገጫዎች።【ቋሚዎች/da/ingest/README.md:1】
3. ** የውህደት ሙከራዎች *** ሁለት አረጋጋጮችን ማስነሳት ፣ የናሙና ነጠብጣቦችን ወደ ውስጥ ማስገባት እና
   ሁለቱም አንጓዎች በጥቅሉ ይዘቶች እና መጠይቅ/ማስረጃ ላይ እንደሚስማሙ በማረጋገጥ
   ምላሾች.
4. ** የብርሃን-ደንበኛ ሙከራዎች *** በ `integration_tests/tests/da/commitments.rs`
   (ዝገት) ወደ `/prove` የሚደውል እና ከTorii ጋር ሳያወራ ማስረጃውን የሚያረጋግጥ።
5. ** CLI ጭስ ** ስክሪፕት `scripts/da/check_commitments.sh` ኦፕሬተርን ለማቆየት
   መሣርያ ሊባዛ የሚችል.

## 8. የልቀት እቅድ| ደረጃ | መግለጫ | መውጫ መስፈርት |
|-------|-------------|-----------|
| P0 - የውሂብ ሞዴል ውህደት | መሬት `DaCommitmentRecord`፣ የአርእስት ማሻሻያዎችን እና Norito ኮዴኮችን አግድ። | `cargo test -p iroha_data_model` አረንጓዴ ከአዳዲስ መገልገያዎች ጋር። |
| P1 - ኮር / WSV ሽቦ | የክር ወረፋ + ገንቢ አመክንዮ አግድ፣ ኢንዴክሶችን ቀጥል እና RPC ተቆጣጣሪዎችን ያጋልጣል። | `cargo test -p iroha_core`፣ `integration_tests/tests/da/commitments.rs` ማለፊያ ከጥቅል ማረጋገጫ ማረጋገጫዎች ጋር። |
| P2 - ኦፕሬተር መሳሪያ | CLI አጋዥዎችን፣ Grafana ዳሽቦርድ እና የማረጋገጫ ማረጋገጫ ሰነድ ማሻሻያዎችን ይላኩ። | `iroha_cli app da prove-commitment` በዴቭኔት ላይ ይሰራል; ዳሽቦርድ የቀጥታ ውሂብ ያሳያል. |
| P3 - የአስተዳደር በር | በ`iroha_config::nexus` በተጠቆሙት መስመሮች ላይ የDA ቁርጠኝነትን የሚፈልግ ብሎክ አረጋጋጭን ያንቁ። | የሁኔታ ግቤት + የመንገድ ካርታ ማሻሻያ DA-3 እንደ 🈴 ምልክት ያድርጉ። |

## ክፍት ጥያቄዎች

1. **KZG vs Merkle ነባሪዎች** - ትናንሽ ነጠብጣቦች ሁል ጊዜ የKZG ቃል ኪዳኖችን መዝለል አለባቸው
   የማገጃውን መጠን ይቀንሱ? ፕሮፖዛል፡ `kzg_commitment` እንደ አማራጭ ያቆዩ እና በር በኩል
   `iroha_config::da.enable_kzg`.
2. **የቅደም ተከተል ክፍተቶች** - ከትዕዛዝ ውጪ መስመሮችን እንፈቅዳለን? አሁን ያለው እቅድ ክፍተቶችን ውድቅ ያደርጋል
   የአደጋ ጊዜ መልሶ ማጫወት `allow_sequence_skips` አስተዳደር ካልተቀየረ በስተቀር።
3. **የብርሃን ደንበኛ መሸጎጫ** — የኤስዲኬ ቡድን ቀላል ክብደት ያለው SQLite መሸጎጫ ጠየቀ።
   ማስረጃዎች; በDA-8 ስር በመጠባበቅ ላይ ያለ ክትትል.

እነዚህን በአፈፃፀም PRs ውስጥ መመለስ DA-3ን ከ 🈸 (ይህ ሰነድ) ወደ 🈺 ያንቀሳቅሳል
አንዴ የኮድ ሥራ ከጀመረ.