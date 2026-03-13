---
lang: am
direction: ltr
source: docs/portal/docs/da/commitments-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ርዕስ: የውሂብ ተገኝነት ግዴታዎች ዕቅድ
sidebar_label: ግዴታዎች ዕቅድ
መግለጫ፡ አግድ፣ RPC እና የ DA ቁርጠኝነትን በNexus ውስጥ ለመክተት ማረጋገጫ የቧንቧ መስመር።
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# Sora Nexus የውሂብ ተገኝነት ግዴታዎች እቅድ (DA-3)

የተረቀቀው፡ 2026-03-25 — ባለቤቶች፡ ኮር ፕሮቶኮል WG / ስማርት ኮንትራት ቡድን / የማከማቻ ቡድን_

DA-3 የNexus የማገጃ ቅርጸትን ያራዝማል ስለዚህ እያንዳንዱ መስመር ወሳኝ መዝገቦችን ይከተታል
በDA-2 የተቀበሉትን ብሎቦች በመግለጽ። ይህ ማስታወሻ ቀኖናዊውን መረጃ ይይዛል
አወቃቀሮች፣ የቧንቧ መስመር መንጠቆዎች፣ የብርሃን ደንበኛ ማረጋገጫዎች፣ እና I18NT0000014X/RPC ንጣፎች
አረጋጋጮች በዲኤ ቃል ኪዳኖች ላይ ከመመካታቸው በፊት ማረፍ ያለበት በመግቢያ ጊዜ ወይም
የአስተዳደር ቼኮች. ሁሉም የክፍያ ጭነቶች I18NT0000002X-የተመሰጠሩ ናቸው; ምንም SCALE ወይም ad-hoc JSON.

# አላማዎች

- ቃል ኪዳኖችን ያካሂዱ ( ቸንክ ስር + ግልጽ ሃሽ + አማራጭ KZG
  ቁርጠኝነት) በእያንዳንዱ Nexus ብሎኮች ውስጥ እኩዮች ተገኝነትን እንደገና እንዲገነቡ
  ከደብዳቤ ውጭ ማከማቻን ሳያማክሩ ግዛት።
- ቀላል ደንበኞች ማረጋገጥ እንዲችሉ ቆራጥ የአባልነት ማረጋገጫዎችን ያቅርቡ ሀ
  መግለጫ ሃሽ በተሰጠው ብሎክ ተጠናቀቀ።
- የTorii መጠይቆችን (`/v2/da/commitments/*`) እና ሪሌሎችን የሚፈቅዱ ማረጋገጫዎችን ያጋልጡ፣
  ኤስዲኬዎች፣ እና የአስተዳደር አውቶሜሽን ኦዲት መገኘት እያንዳንዱን ሳይደግሙ
  አግድ
- አዲሱን ክር በማድረግ ያለውን I18NI0000028X ኤንቨሎፕ ቀኖናዊ ያድርጉት
  በNorito ሜታዳታ ራስጌ በኩል ያሉ መዋቅሮች እና የሃሽ አመጣጥን አግድ።

## ወሰን አጠቃላይ እይታ

1. ** የውሂብ ሞዴል ተጨማሪዎች ** በ I18NI0000029X ሲደመር ብሎክ
   የርዕስ ለውጦች በ I18NI0000030X.
2. **አስፈፃሚ መንጠቆዎች** ስለዚህ `iroha_core` በ Torii የወጡትን የ DA ደረሰኞች አስገብቷል።
   (`crates/iroha_core/src/queue.rs` እና `crates/iroha_core/src/block.rs`)።
3. ** ጽናት/ኢንዴክሶች** ስለዚህ WSV የቁርጠኝነት ጥያቄዎችን በፍጥነት መመለስ ይችላል።
   (`iroha_core/src/wsv/mod.rs`)።
4. **Torii RPC ተጨማሪዎች** ለዝርዝር/ጥያቄ/በስር ያሉ የመጨረሻ ነጥቦችን ያረጋግጡ
   `/v2/da/commitments`.
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
  `iroha_crypto::kzg`. በሌለበት ጊዜ ወደ መርክል ማስረጃዎች ብቻ እንመለሳለን።
- `proof_scheme` ከሌይን ካታሎግ የተገኘ ነው; የመርክል መስመሮች KZGን አይቀበሉም።
  የሚጫኑ ጭነቶች I18NI0000041X መስመሮች ዜሮ ያልሆኑ KZG ግዴታዎች ይጠይቃሉ። Torii
  በአሁኑ ጊዜ የመርክል ቁርጠኝነትን ብቻ የሚያመርት እና በKZG የተዋቀሩ መስመሮችን ውድቅ ያደርጋል።
- `KzgCommitment` አሁን ያለውን ባለ 48-ባይት ነጥብ ከዚህ በታች ጥቅም ላይ ይውላል
  `iroha_crypto::kzg`. በመርክሌ መንገዶች ላይ በማይገኝበት ጊዜ ወደ መርክል ማረጋገጫዎች እንመለሳለን።
  ብቻ።
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

የማስፈጸሚያ ማስታወሻ፡ I18NI0000046X እና ግልፅ የሆነው `BlockBuilder` አሁን ተጋልጧል
`da_commitments` setters/getters (`BlockBuilder::set_da_commitments` ይመልከቱ እና
`SignedBlock::set_da_commitments`)፣ ስለዚህ አስተናጋጆች አስቀድሞ የተሰራ ጥቅል ማያያዝ ይችላሉ።
እገዳ ከመዘጋቱ በፊት. ሁሉም አጋዥ ገንቢዎች መስኩን ወደ `None` ነባሪ አድርገውታል።
እስከ Torii ክሮች እውነተኛ ቅርቅቦች ውስጥ።

### 1.3 ሽቦ ኢንኮዲንግ

- `SignedBlockWire::canonical_wire()` የ I18NT0000004X ራስጌን ለ
  `DaCommitmentBundle` አሁን ካለው የግብይት ዝርዝር በኋላ። የ
  ባይት ስሪት I18NI0000054X ነው።
- `SignedBlockWire::decode_wire()` `version` የማይታወቅ ጥቅሎችን ውድቅ ያደርጋል፣
  በI18NI0000057X ውስጥ ከተገለጸው የNorito ፖሊሲ ጋር የሚዛመድ።
- የሃሽ መነጫነጭ ዝማኔዎች በ`block::Hasher` ውስጥ ብቻ ይኖራሉ። ብርሃን ደንበኞች መፍታት
  አሁን ያለው የሽቦ ቅርፀት አዲሱን መስክ በራስ-ሰር ያገኛል ምክንያቱም Norito
  ራስጌ መገኘቱን ያስታውቃል።

## 2. የምርት ፍሰትን አግድ

1. Torii DA inges `DaIngestReceipt` አጠናቅቆ በ
   የውስጥ ወረፋ (I18NI0000060X)።
2. `PendingBlocks` ሁሉንም ደረሰኞች ይሰበስባል `lane_id` ከታች ካለው ብሎክ ጋር የሚዛመድ
   ግንባታ, በ `(lane_id, client_blob_id, manifest_hash)` ማባዛት.
3. ልክ ከመታተሙ በፊት ብሎክ ገንቢው ቃል ኪዳኖችን በ `(ሌይን_መታወቂያ፣
   epoch፣ sequence)` ሃሽ ወሳኙን ለማድረግ፣ ጥቅሉን በ
   Norito ኮዴክ፣ እና I18NI0000064X ያዘምናል።
4. ሙሉው ጥቅል በ WSV ውስጥ ተከማችቶ ከውስጥ ካለው እገዳ ጋር አብሮ ይወጣል
   `SignedBlockWire`.

እገዳ መፍጠር ካልተሳካ ደረሰኞች በወረፋው ውስጥ ይቀራሉ ስለዚህ ቀጣዩ እገዳ
ሙከራ እነሱን ማንሳት ይችላል; ግንበኛ `sequence` በ የተካተተ የመጨረሻውን ይመዘግባል
ተደጋጋሚ ጥቃቶችን ለማስወገድ መስመር።

## 3. RPC & Query Surface

Torii ሶስት የመጨረሻ ነጥቦችን ያጋልጣል፡

| መስመር | ዘዴ | ጭነት | ማስታወሻ |
|-------|--------|-----|------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (የክልል ማጣሪያ በሌይን/ኢፖክ/ተከታታይ፣ pagination) | `DaCommitmentPage` በጠቅላላ ቆጠራ፣ ቃል ኪዳኖች እና ሃሽ አግድ ይመልሳል። |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (ሌይን + ግልጽ ሃሽ ወይም `(epoch, sequence)` tuple)። | በ`DaCommitmentProof` (መዝገብ + Merkle መንገድ + አግድ ሃሽ) ምላሽ ይሰጣል። |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | የብሎክ ሃሽ ስሌትን የሚደግም እና ማካተትን የሚያረጋግጥ ሀገር አልባ ረዳት ፤ በቀጥታ ወደ `iroha_crypto` ማገናኘት በማይችሉ ኤስዲኬዎች ጥቅም ላይ ይውላል። |

ሁሉም የክፍያ ጭነቶች በ I18NI0000080X ስር ይኖራሉ። Torii ራውተሮች ተራራ
ቶከን/ኤምቲኤልኤስን እንደገና ለመጠቀም ከነባሩ DA ቀጥሎ ያሉት ተቆጣጣሪዎች የመጨረሻ ነጥቦችን ያስገባሉ።
ፖሊሲዎች.

## 4. የማካተት ማረጋገጫዎች እና ቀላል ደንበኞች

- የማገጃ አምራቹ በተከታታዩ ላይ ሁለትዮሽ የመርክል ዛፍ ይገነባል።
  `DaCommitmentRecord` ዝርዝር. ሥሩ `da_commitments_hash` ይመገባል።
- `DaCommitmentProof` የዒላማውን ሪከርድ እና የ`(ወንድም_እህት_ሃሽ፣
  አቀማመጥ)` ግቤቶች ስለዚህ አረጋጋጮች ሥሩን እንደገና መገንባት ይችላሉ። ማስረጃዎችም ያካትታሉ
  ቀላል ደንበኞች የመጨረሻነቱን ማረጋገጥ እንዲችሉ ሃሽ እና የተፈረመ ራስጌ።
- የ CLI ረዳቶች (`iroha_cli app da prove-commitment`) የማስረጃ ጥያቄውን ያጠቃልላሉ/ ያረጋግጡ
  ዑደት እና ወለል Norito/ሄክስ ውፅዓቶች ለኦፕሬተሮች።

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
   ጥቅል ባይት እና የመርክሌ ማረጋገጫዎች።
3. ** የውህደት ሙከራዎች *** ሁለት አረጋጋጮችን ማስነሳት ፣ የናሙና ነጠብጣቦችን ወደ ውስጥ ማስገባት እና
   ሁለቱም አንጓዎች በጥቅሉ ይዘቶች እና መጠይቅ/ማስረጃ ላይ እንደሚስማሙ በማረጋገጥ
   ምላሾች.
4. **የብርሃን-ደንበኛ ሙከራዎች** በI18NI0000093X
   (ዝገት) ወደ I18NI0000094X የሚደውል እና ከTorii ጋር ሳያወራ ማስረጃውን የሚያረጋግጥ።
5. ** CLI ጭስ ** ስክሪፕት I18NI0000095X ኦፕሬተርን ለማቆየት
   መሣርያ ሊባዛ የሚችል.

## 8. የልቀት እቅድ

| ደረጃ | መግለጫ | መውጫ መስፈርት |
|-------|-------------|-----------|
| P0 - የውሂብ ሞዴል ውህደት | መሬት `DaCommitmentRecord`፣ የአርእስት ማሻሻያዎችን እና Norito ኮዴኮችን አግድ። | `cargo test -p iroha_data_model` አረንጓዴ ከአዳዲስ መገልገያዎች ጋር። |
| P1 - ኮር / WSV ሽቦ | የክር ወረፋ + ገንቢ አመክንዮ አግድ፣ ኢንዴክሶችን ቀጥል እና RPC ተቆጣጣሪዎችን ያጋልጣል። | `cargo test -p iroha_core`፣ `integration_tests/tests/da/commitments.rs` ማለፊያ ከጥቅል ማረጋገጫ ማረጋገጫዎች ጋር። |
| P2 - ኦፕሬተር መሳሪያ | CLI አጋዥዎችን፣ Grafana ዳሽቦርድ እና የማረጋገጫ ማረጋገጫ ሰነድ ማሻሻያዎችን ይላኩ። | `iroha_cli app da prove-commitment` devnet ላይ ይሰራል; ዳሽቦርድ የቀጥታ ውሂብ ያሳያል. |
| P3 - የአስተዳደር በር | በI18NI0000101X በተጠቆሙት መስመሮች ላይ የDA ቁርጠኝነትን የሚፈልግ ብሎክ አረጋጋጭን ያንቁ። | የሁኔታ ግቤት + የመንገድ ካርታ ማሻሻያ DA-3 እንደ 🈴 ምልክት ያድርጉ። |

## ክፍት ጥያቄዎች

1. **KZG vs Merkle ነባሪዎች** - ትናንሽ ነጠብጣቦች ሁል ጊዜ የKZG ቃል ኪዳኖችን መዝለል አለባቸው
   የማገጃውን መጠን ይቀንሱ? ፕሮፖዛል፡ `kzg_commitment` እንደ አማራጭ እና በበር በኩል ያቆዩት።
   `iroha_config::da.enable_kzg`.
2. **የቅደም ተከተል ክፍተቶች** - ከትዕዛዝ ውጪ መስመሮችን እንፈቅዳለን? አሁን ያለው እቅድ ክፍተቶችን ውድቅ ያደርጋል
   የአደጋ ጊዜ ድጋሚ ለማጫወት `allow_sequence_skips` አስተዳደር ካልተቀየረ።
3. **የብርሃን ደንበኛ መሸጎጫ** — የኤስዲኬ ቡድን ቀላል ክብደት ያለው SQLite መሸጎጫ ጠየቀ።
   ማስረጃዎች; በDA-8 ስር በመጠባበቅ ላይ ያለ ክትትል.

እነዚህን በአፈፃፀም PRs ውስጥ መመለስ DA-3ን ከ 🈸 (ይህ ሰነድ) ወደ 🈺 ያንቀሳቅሳል
አንዴ የኮድ ሥራ ከጀመረ.