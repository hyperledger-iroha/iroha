---
lang: am
direction: ltr
source: docs/source/kaigi_privacy_design.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b7ffca7e960376a2959357cd865d8dab5afa1dfcb959adbc688b6db60977c8f
source_last_modified: "2026-01-05T09:28:12.022066+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የካይጂ ግላዊነት እና ቅብብል ንድፍ

ይህ ሰነድ ዜሮ-እውቀትን የሚያስተዋውቅ በግላዊነት ላይ ያተኮረ የዝግመተ ለውጥን ይይዛል
የተሳትፎ ማረጋገጫዎች እና የሽንኩርት አይነት ቅብብሎሽ ቆራጥነት ሳይቆርጥ ወይም
የሂሳብ መዝገብ ኦዲትነት.

# አጠቃላይ እይታ

ዲዛይኑ ሶስት እርከኖችን ያቀፈ ነው-

- ** የስም ዝርዝር ግላዊነት *** - የአስተናጋጅ ፈቃዶችን እና የሂሳብ አከፋፈል ወጥነት ባለው መልኩ የተሳታፊ ማንነቶችን በሰንሰለት ላይ ደብቅ።
- ** የአጠቃቀም ግልጽነት *** - የክፍል ዝርዝሮችን በይፋ ሳይገልጹ አስተናጋጆች የመለኪያ አጠቃቀምን እንዲመዘገቡ ፍቀድ።
- **ተደራቢ ቅብብሎሽ** - የኔትወርክ ታዛቢዎች የትኞቹ ተሳታፊዎች እንደሚግባቡ ለማወቅ እንዳይችሉ በባለብዙ ሆፕ አቻዎች በኩል የመጓጓዣ እሽጎች።

ሁሉም ተጨማሪዎች Norito-በመጀመሪያ ይቀራሉ፣ በABI ስሪት 1 ይሰራሉ፣ እና በልዩ ልዩ ሃርድዌር ላይ በቆራጥነት መፈፀም አለባቸው።

# ግቦች

1. የዜሮ እውቀት ማረጋገጫዎችን ተጠቅመው ተሳታፊዎችን ይቀበሉ/ማስወጣት ስለዚህ የሂሳብ ደብተር ጥሬ የመለያ መታወቂያዎችን በጭራሽ እንዳያጋልጥ።
2. ጠንካራ የሂሳብ ዋስትናዎችን ይያዙ፡ እያንዳንዱ መቀላቀል፣ መተው እና የአጠቃቀም ክስተት አሁንም በቆራጥነት መታረቅ አለበት።
3. ለቁጥጥር/ዳታ ቻናሎች የሽንኩርት መንገዶችን የሚገልጹ እና በሰንሰለት ሊመረመሩ የሚችሉ የአማራጭ ቅብብሎሽ መግለጫዎችን ያቅርቡ።
4. ግላዊነትን ለማይፈልጉ ማሰማራቶች የኋላ ኋላ (ሙሉ ግልጽነት ያለው ዝርዝር) እንዲሰራ ያድርጉ።

# የዛቻ ሞዴል ማጠቃለያ

- ** ተቃዋሚዎች፡** የአውታረ መረብ ታዛቢዎች (አይኤስፒዎች)፣ የማወቅ ጉጉት ሰጪዎች፣ ተንኮል አዘል ኦፕሬተሮች እና ከፊል ሐቀኛ አስተናጋጆች።
- **የተጠበቁ ንብረቶች፡** የተሳታፊ ማንነት፣ የተሳትፎ ጊዜ፣ የክፍል አጠቃቀም/የክፍያ ዝርዝሮች እና የአውታረ መረብ ማዘዋወር ዲበ ውሂብ።
- ** ግምቶች: *** አስተናጋጆች አሁንም እውነተኛውን ተሳታፊ ከሰንሰለት ማጥፋት ይማራሉ; የመመዝገቢያ እኩዮች በቆራጥነት ማረጋገጫዎችን ያረጋግጣሉ; ተደራቢ ቅብብሎሽ የማይታመን ነገር ግን ተመን-የተገደበ ነው; የHPKE እና SNARK ቀዳሚዎች ቀድሞውኑ በኮድ ቤዝ ውስጥ አሉ።

# የውሂብ ሞዴል ለውጦች

ሁሉም ዓይነቶች በ `iroha_data_model::kaigi` ውስጥ ይኖራሉ።

```rust
/// Commitment to a participant identity (Poseidon hash of account + domain salt).
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>,
}

/// Nullifier unique to each join action, prevents double-use of proofs.
pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64,
}

/// Relay path description used by clients to set up onion routing.
pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    pub hpke_public_key: FixedBinary<32>,
    pub weight: u8,
}
```

`KaigiRecord` የሚከተሉትን መስኮች አግኝቷል።

- `roster_commitments: Vec<KaigiParticipantCommitment>` - የግላዊነት ሁነታ ከነቃ በኋላ የተጋለጠውን `participants` ዝርዝር ይተካል። ክላሲክ ማሰማራቶች በስደት ጊዜ ሁለቱንም ሰዎች እንዲሞሉ ማድረግ ይችላሉ።
- `nullifier_log: Vec<KaigiParticipantNullifier>` - ጥብቅ አባሪ-ብቻ፣ የሜታዳታ ውሱን ሆኖ ለማቆየት በሚሽከረከረው መስኮት ተሸፍኗል።
- `room_policy: KaigiRoomPolicy` - ለክፍለ-ጊዜው የተመልካቹን የማረጋገጫ አቋም ይመርጣል (`Public` ክፍሎች መስተዋት ተነባቢ-ብቻ ቅብብሎሽ፤ `Authenticated` ክፍሎች ከመውጣት ፓኬቶች በፊት የተመልካች ትኬቶችን ይፈልጋሉ)።
- `relay_manifest: Option<KaigiRelayManifest>` – የተዋቀረ አንጸባራቂ በNorito ኮድ የተቀመጠ ስለዚህ ሆፕስ፣ HPKE ቁልፎች እና ክብደቶች ያለ JSON ሺምስ ቀኖናዊ ሆነው ይቆያሉ።
- `privacy_mode: KaigiPrivacyMode` enum (ከዚህ በታች ይመልከቱ)።

```rust
pub enum KaigiPrivacyMode {
    Transparent,
    ZkRosterV1,
}
```

`NewKaigi` አስተናጋጆች በፍጥረት ጊዜ ወደ ግላዊነት መርጠው እንዲገቡ ተዛማጅ አማራጭ መስኮችን ይቀበላል።


- መስኮች ቀኖናዊ ኢንኮዲንግ ለማስፈጸም `#[norito(with = "...")]` ረዳቶችን ይጠቀማሉ (ትንሽ-ኢንዲያን ኢንቲጀር፣ በቦታ የተደረደሩ ሆፕስ)።
- `KaigiRecord::from_new` አዲሶቹን ቬክተሮች ባዶ ያደርጋሉ እና ማንኛውንም የቀረበውን የማሰራጫ መግለጫ ይገለበጣሉ።

# መመሪያ የገጽታ ለውጦች

## የማሳያ ፈጣን ጅምር አጋዥ

ለአድ-ሆክ ማሳያዎች እና ለተግባራዊነት ሙከራዎች CLI አሁን ያጋልጣል
`iroha kaigi quickstart`. እሱ፡-- በ`--domain`/`--host` ካልተሻረ በስተቀር የCLI ውቅር (ጎራ `wonderland` + መለያ) እንደገና ይጠቀማል።
- `--call-name` ሲቀር በጊዜ ማህተም ላይ የተመሰረተ የጥሪ ስም ያመነጫል እና `CreateKaigi` ከገባሪው Torii የመጨረሻ ነጥብ ጋር ያቀርባል።
- እንደ አማራጭ አስተናጋጁን (`--auto-join-host`) በመቀላቀል ተመልካቾች ወዲያውኑ መገናኘት ይችላሉ።
- Torii URL፣ የጥሪ ለዪዎች፣ የግላዊነት/የክፍል ፖሊሲ፣ ለመቅዳት ዝግጁ የሆነ የመቀላቀል ትእዛዝን የያዘ የJSON ማጠቃለያ አውጥቷል (ለምሳሌ፣ `storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/*.norito`)። ብልጭታውን ለመቀጠል `--summary-out path/to/file.json` ይጠቀሙ።

ይህ አጋዥ **የማሄድ `irohad --sora` መስቀለኛ መንገድ አስፈላጊነትን አይተካውም፡የግላዊነት መንገዶች፣የማስወጫ ፋይሎች እና የማስተላለፊያ መግለጫዎች በደብዳቤ የተደገፉ ሆነው ይቆያሉ። ጊዜያዊ ክፍሎችን ለውጭ አካላት በሚሽከረከርበት ጊዜ በቀላሉ ቦይለር ይቆርጣል።

### አንድ-ትእዛዝ ማሳያ ስክሪፕት።

ለፈጣን መንገድ ተጓዳኝ ስክሪፕት አለ፡ `scripts/kaigi_demo.sh`።
ለእርስዎ የሚከተሉትን ያከናውናል፡

1. የተጠቀለለውን `defaults/nexus/genesis.json` ወደ `target/kaigi-demo/genesis.nrt` ይፈርማል።
2. `irohad --sora` በተፈረመ ብሎክ ያስጀምራል (ምዝግብ ማስታወሻዎች በ `target/kaigi-demo/irohad.log`) እና Torii ን ለማጋለጥ `http://127.0.0.1:8080/status` ይጠብቃል።
3. `iroha kaigi quickstart --auto-join-host --summary-out target/kaigi-demo/kaigi_summary.json` ይሰራል።
4. ከውጫዊ ሞካሪዎች ጋር መጋራት እንዲችሉ ወደ JSON ማጠቃለያ የሚወስደውን መንገድ እና የስፑል ማውጫውን (`storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/`) ያትማል።

የአካባቢ ተለዋዋጮች፡-

- `TORII_URL` - የ Torii የመጨረሻ ነጥብ ወደ ምርጫ (ነባሪ `http://127.0.0.1:8080`) ይሽሩት።
- `RUN_DIR` - የስራ ማውጫውን ይሽሩ (ነባሪው `target/kaigi-demo`)።

`Ctrl+C` በመጫን ማሳያውን ያቁሙ; በስክሪፕቱ ውስጥ ያለው ወጥመድ `irohad` በራስ-ሰር ያበቃል። ሂደቱ ከወጣ በኋላ ቅርሶችን መስጠት እንዲችሉ የስፑል ፋይሎች እና ማጠቃለያ በዲስክ ላይ ይቀራሉ።

## `CreateKaigi`

- ከአስተናጋጅ ፈቃዶች አንጻር `privacy_mode` ያረጋግጣል።
- `relay_manifest` ከቀረበ፣ ≥3 ሆፕስ፣ ዜሮ ያልሆኑ ክብደቶች፣ የHPKE ቁልፍ መገኘት እና ልዩነት በሰንሰለት ላይ የሚታዩ ምልክቶችን ያስፈጽሙ።
- የ`room_policy` ግቤትን ከኤስዲኬ/CLI (`public` vs `authenticated`) አረጋግጥ እና ወደ SoraNet አቅርቦት በማሰራጨት መሸጎጫዎች ትክክለኛዎቹን የ GAR ምድቦች (`stream.kaigi.public` vs I00001) ያጋልጣሉ። አስተናጋጆች ይህንን በ`iroha kaigi create --room-policy …`፣ በJS SDK's `roomPolicy` መስክ ወይም `room_policy` በማቀናበር የስዊፍት ደንበኞች ከማቅረቡ በፊት የNorito ክፍያን ሲሰበስቡ ነው።
- ባዶ የቁርጠኝነት/የማስወገድ ምዝግብ ማስታወሻዎችን ያከማቻል።

## `JoinKaigi`

መለኪያዎች፡-

- `proof: ZkProof` (Norito ባይት መጠቅለያ) - Groth16 የሚያረጋግጠው መረጃ ጠሪው የሚያውቀው `(account_id, domain_salt)` የ Poseidon hash ከሚቀርበው `commitment` ጋር እኩል ነው።
- `commitment: FixedBinary<32>`
- `nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>` - ለቀጣዩ ሆፕ አማራጭ በአንድ ተሳታፊ መሻር።

የማስፈጸሚያ ደረጃዎች፡-

1. `record.privacy_mode == Transparent` ከሆነ፣ ወደ የአሁኑ ባህሪ ይመለሱ።
2. የ Groth16 ማስረጃን በወረዳው መዝገብ `KAIGI_ROSTER_V1` ላይ ያረጋግጡ።
3. `nullifier` በ `record.nullifier_log` አለመታየቱን ያረጋግጡ።
4. አባሪ ቁርጠኝነት / nullifier ግቤቶች; `relay_hint` ከተሰጠ፣ ለዚህ ​​ተሳታፊ የማስተላለፊያ አንጸባራቂ እይታን ያስተካክሉት (በማስታወሻ ክፍለ ጊዜ ሁኔታ ብቻ እንጂ በሰንሰለት ላይ አይደለም)።## `LeaveKaigi`

ግልጽ ሁነታ ከአሁኑ አመክንዮ ጋር ይዛመዳል።

የግል ሁነታ ያስፈልገዋል፡-

1. ደዋዩ በ`record.roster_commitments` ውስጥ ያለውን ቁርጠኝነት እንደሚያውቅ የሚያሳይ ማረጋገጫ።
2. የነጠላ አጠቃቀም ፈቃድን የሚያረጋግጥ የኑልፋየር ዝማኔ።
3. የቁርጠኝነት/የማጥፋት ግቤቶችን ያስወግዱ። ኦዲቲንግ መዋቅራዊ ፍሳሾችን ለማስቀረት ቋሚ የማቆያ መስኮቶች የመቃብር ድንጋዮችን ይጠብቃል።

## `RecordKaigiUsage`

ክፍያን በሚከተሉት ያራዝማል፡-

- `usage_commitment: FixedBinary<32>` - ለጥሬ አጠቃቀም tuple (የቆይታ ጊዜ ፣ የጋዝ ፣ የክፍል መታወቂያ) ቁርጠኝነት።
- ከደብዳቤ ውጪ የቀረቡትን የተመሰጠሩ ምዝግብ ማስታወሻዎችን የሚያረጋግጥ አማራጭ ZK ማረጋገጫ።

አስተናጋጆች አሁንም ግልጽ ድምርን ማስገባት ይችላሉ; የግላዊነት ሁነታ የግዴታ መስኩን ብቻ ያደርገዋል።

# ማረጋገጫ እና ወረዳዎች

- `iroha_core::smartcontracts::isi::kaigi::privacy` አሁን ሙሉ ዝርዝር ስራዎችን ይሰራል
  በነባሪነት ማረጋገጥ. `zk.kaigi_roster_join_vk` (መቀላቀል) እና ይፈታል።
  `zk.kaigi_roster_leave_vk` (ቅጠሎች) ከውቅረት፣
  ተዛማጅ የሆነውን `VerifyingKeyRef` በ WSV ውስጥ ይመለከታል (መዝገቡ መሆኑን ያረጋግጣል)
  `Active`፣ ከኋላ/የወረዳ መለያዎች ይዛመዳሉ፣ እና ቃል ኪዳኖች አሰላለፍ)፣ ክፍያዎች
  ባይት አካውንቲንግ፣ እና ወደ የተዋቀረው የZK የኋላ ክፍል ይልካል።
- የ `kaigi_privacy_mocks` ባህሪው የመወሰን አረጋጋጭን ይይዛል.
  የዩኒት/የመዋሃድ ሙከራዎች እና የተገደቡ የ CI ስራዎች ያለ Halo2 ጀርባ ሊሰሩ ይችላሉ።
  እውነተኛ ማረጋገጫዎችን ለማስፈጸም የምርት ግንባታዎች ባህሪው እንዲሰናከል ማድረግ አለበት።
- ሣጥኑ `kaigi_privacy_mocks` የነቃ ከሆነ የማጠናቀር ጊዜ ስህተት ያወጣል።
  ያልተሞከረ፣ `debug_assertions` ግንባታ ያልሆነ፣ በአጋጣሚ የሚለቀቁ ሁለትዮሾችን ይከላከላል።
  ከግንዱ ጋር ከማጓጓዝ.
- ኦፕሬተሮች (1) በአስተዳደር በኩል የተቀመጠውን የስም ዝርዝር አረጋጋጭ መመዝገብ አለባቸው
  (2) `zk.kaigi_roster_join_vk`፣ `zk.kaigi_roster_leave_vk` አዘጋጅ፣ እና
  `zk.kaigi_usage_vk` በ`iroha_config` ስለዚህ አስተናጋጆች በሂደት እንዲፈቱዋቸው።
  ቁልፎቹ እስኪገኙ ድረስ የግላዊነት መጋጠሚያዎች፣ ቅጠሎች እና የአጠቃቀም ጥሪዎች አይሳኩም
  በቆራጥነት።
- `crates/kaigi_zk` አሁን Halo2 ወረዳዎችን ለሮስተር መጋጠሚያዎች/ቅጠሎች እና አጠቃቀም ይልካል።
  በድጋሚ ጥቅም ላይ ሊውሉ ከሚችሉት መጭመቂያዎች (`commitment`፣ `nullifier`፣
  `usage`)። የሮስተር ዑደቶች የመርክልን ሥር (አራት ትንሹ-ኤንዲያን) ያጋልጣሉ
  64-ቢት እጅና እግር) እንደ ተጨማሪ የህዝብ ግብአቶች አስተናጋጁ ማረጋገጫውን መሻገር ይችላል።
  ከማረጋገጡ በፊት በተከማቸ የሮስተር ሥር. የአጠቃቀም ግዴታዎች ናቸው።
  በ`KaigiUsageCommitmentCircuit` የተተገበረ፣ እሱም '(የቆይታ ጊዜ፣ ጋዝ፣
  ክፍል)` ወደ ላይ-መሪ ሃሽ።
- `Join` የወረዳ ግብዓቶች: `(commitment, nullifier, domain_salt)` እና የግል
  `(account_id)`. የህዝብ ግብዓቶች `commitment`፣ `nullifier` እና
  የመርክል ሥር አራት እግሮች ለሮስተር ቁርጠኝነት ዛፍ (ስም ዝርዝር
  ከሰንሰለት ውጭ ሆኖ ይቀራል፣ ነገር ግን ሥሩ ወደ ግልባጩ ተያይዟል።
- ቆራጥነት: በ ውስጥ የፖሲዶን መለኪያዎችን ፣ የወረዳ ስሪቶችን እና ኢንዴክሶችን እናስተካክላለን
  መዝገብ ቤት. ማንኛውም ለውጥ `KaigiPrivacyMode` ወደ `ZkRosterV2` ከግጥሚያ ጋር ያጋጫል።
  ሙከራዎች / ወርቃማ ፋይሎች.

# የሽንኩርት መስመር ተደራቢ

## የዝውውር ምዝገባ- የ HPKE ቁልፍ ቁሳቁስ እና የመተላለፊያ ይዘትን ጨምሮ `kaigi_relay::<relay_id>` እንደ ጎራ ሜታዳታ ግቤቶችን ያስተላልፋል።
- የ`RegisterKaigiRelay` መመሪያ ገላጭውን በጎራ ሜታዳታ ውስጥ ይቀጥላል፣ የ`KaigiRelayRegistered` ማጠቃለያ ያወጣል (ከHPKE የጣት አሻራ እና የመተላለፊያ ክፍል ጋር) እና ቁልፎቹን በቆራጥነት ለማሽከርከር እንደገና ሊጠራ ይችላል።
- አስተዳደር የፈቃድ ዝርዝሮችን በጎራ ሜታዳታ (`kaigi_relay_allowlist`) ያዘጋጃል፣ እና አዲስ ዱካዎችን ከመቀበላችን በፊት ምዝገባ/ማሳያ ዝማኔዎችን ያስተላልፋል።

## ፍጥረትን ያሳያል

- አስተናጋጆች ባለብዙ-ሆፕ መንገዶችን (ቢያንስ ርዝመቱ 3) ከሚገኙ ቅብብሎች ይገነባሉ። አንጸባራቂው የተነባበረውን ኤንቨሎፕ ለማመስጠር የAccountIds ቅደም ተከተሎችን እና የHPKE ይፋዊ ቁልፎችን ያሳያል።
- በሰንሰለት ላይ የተከማቸ `relay_manifest` የሆፕ ገላጭ እና የአገልግሎት ጊዜ ማብቂያ (Norito-encoded `KaigiRelayManifest`) ይይዛል። ትክክለኛ ጊዜያዊ ቁልፎች እና በየክፍለ-ጊዜ ማካካሻዎች HPKE ን በመጠቀም ከደብዳቤ ውጪ ይለዋወጣሉ።

## ምልክት እና ሚዲያ

- የኤስዲፒ/ICE ልውውጥ በካይጊ ሜታዳታ በኩል ይቀጥላል ነገር ግን በሆፕ የተመሰጠረ ነው። አረጋጋጮች የHPKE ምስጥር ጽሑፍ እና የርዕስ ማውጫዎችን ብቻ ነው የሚያዩት።
- የሚዲያ እሽጎች QUICን ተጠቅመው በታሸገ ጭነት ይጓዛሉ። እያንዳንዱ ሆፕ ቀጣዩን የሆፕ አድራሻ ለመማር አንድ ንብርብር ዲክሪፕት ያደርጋል; የመጨረሻው ተቀባይ ሁሉንም ንብርብሮች ካጸዳ በኋላ የሚዲያ ዥረቱን ያገኛል።

##ያልተሳካለት

- ደንበኞች የማስተላለፊያ ጤናን በ`ReportKaigiRelayHealth` መመሪያ ይቆጣጠራሉ፣ ይህም በጎራ ዲበዳታ (`kaigi_relay_feedback::<relay_id>`) ላይ የተፈረመ ግብረ መልስ በቀጠለው፣ `KaigiRelayHealthUpdated` ያሰራጫል፣ እና አስተዳደር/አስተናጋጆች ስለአሁኑ ተገኝነት እንዲያስቡ ያስችላቸዋል። ሪሌይ ሳይሳካ ሲቀር አስተናጋጁ የዘመነ አንጸባራቂ ያወጣና የ`KaigiRelayManifestUpdated` ክስተት ይመዘግባል (ከዚህ በታች ይመልከቱ)።
- አስተናጋጆች በ `SetKaigiRelayManifest` መመሪያ በኩል የሰነድ ለውጦችን ይተገብራሉ፣ ይህም የተቀመጠውን መንገድ የሚተካ ወይም ሙሉ በሙሉ ያጸዳዋል። ማጽዳት ከ`hop_count = 0` ጋር ማጠቃለያ ያወጣል ስለዚህ ኦፕሬተሮች ወደ ቀጥታ ማዘዋወር የሚደረገውን ሽግግር መመልከት ይችላሉ።
- Prometheus ሜትሪክስ (`kaigi_relay_registered_total`፣ `kaigi_relay_registration_bandwidth_class`፣ `kaigi_relay_manifest_updates_total`፣ `kaigi_relay_manifest_hop_count`፣ `kaigi_relay_health_reports_total`፣ Norito `kaigi_relay_failover_hop_count`) አሁን የገጽታ ቅብብሎሽ ቸነፈር፣ የጤና ሁኔታ እና ያልተሳካ ውጤት ለኦፕሬተር ዳሽቦርዶች።

# ክስተቶች

የ`DomainEvent` ተለዋጮችን ዘርጋ፡

- `KaigiRosterSummary` - ማንነታቸው ባልታወቁ ቁጥሮች እና አሁን ባለው ዝርዝር የተለቀቀው
  ሮስተር በተቀየረ ቁጥር root (ሥሩ `None` ግልጽ በሆነ ሁነታ) ነው።
- `KaigiRelayRegistered` - የዝውውር ምዝገባ ሲፈጠር ወይም ሲዘመን የሚለቀቅ።
- `KaigiRelayManifestUpdated` - የማስተላለፊያ አንጸባራቂው ሲቀየር የሚለቀቅ።
- `KaigiRelayHealthUpdated` - አስተናጋጆች በ `ReportKaigiRelayHealth` በኩል የቅብብሎሽ የጤና ሪፖርት ሲያቀርቡ የሚወጣው።
- `KaigiUsageSummary` - ከእያንዳንዱ የአጠቃቀም ክፍል በኋላ የሚለቀቀው አጠቃላይ ድምርን ብቻ ያጋልጣል።

ክስተቶች በNorito ተከታታይ ይሆናሉ፣ ይህም ቁርጠኝነትን እና ቆጠራዎችን ብቻ በማጋለጥ ነው።CLI tooling (`iroha kaigi …`) ኦፕሬተሮች ክፍለ ጊዜዎችን መመዝገብ እንዲችሉ እያንዳንዱን አይኤስአይ ይጠቀልላል፣
የስም ዝርዝር ዝመናዎችን ያቅርቡ ፣ የዝውውር ጤናን ሪፖርት ያድርጉ እና ያለ የእጅ ሥራ ግብይቶች አጠቃቀምን ይመዝግቡ።
የማስተላለፊያ መግለጫዎች እና የግላዊነት ማረጋገጫዎች የሚጫኑት ከ JSON/hex ፋይሎች ካለፉ ነው።
የ CLI መደበኛ የማስረከቢያ መንገድ፣ ወደ ስክሪፕት ውል ቀጥተኛ ያደርገዋል
በዝግጅት አከባቢዎች ውስጥ መግባት ።

# ጋዝ የሂሳብ አያያዝ

- አዲስ ቋሚዎች በ `crates/iroha_core/src/gas.rs`:
  - `BASE_KAIGI_JOIN_ZK`፣ `BASE_KAIGI_LEAVE_ZK`፣ እና `BASE_KAIGI_USAGE_ZK`
    ከHalo2 የማረጋገጫ ጊዜዎች ጋር ተስተካክሏል (≈1.6ms ለሮስተር
    መቀላቀል/ቅጠሎች፣ ≈1.2ms በአፕል M2 Ultra ላይ ለመጠቀም)። ተጨማሪ ክፍያዎች ቀጥለዋል።
    በ`PER_KAIGI_PROOF_BYTE` በኩል ከማስረጃ ባይት መጠን ጋር ልኬት።
- `RecordKaigiUsage` በቁርጠኝነት መጠን እና በማረጋገጫ ማረጋገጫ ላይ በመመስረት ተጨማሪ ክፍያ ፈፅሟል።
- የካሊብሬሽን ታጥቆ ሚስጥራዊውን የንብረት መሠረተ ልማት በቋሚ ዘሮች እንደገና ይጠቀማል።

# የሙከራ ስትራቴጂ

- ለ`KaigiParticipantCommitment`፣ `KaigiRelayManifest` Norito ኢንኮድ/መግለጫ ያረጋግጣሉ።
- ቀኖናዊ ቅደም ተከተልን የሚያረጋግጡ ለ JSON እይታ ወርቃማ ሙከራዎች።
- አነስተኛ አውታረ መረብን የሚሽከረከር የውህደት ሙከራዎች (ተመልከት
  `crates/iroha_core/tests/kaigi_privacy.rs` ለአሁኑ ሽፋን፡-
  - የማሾፍ ማረጋገጫዎችን በመጠቀም የግል መቀላቀል/መውጣት ዑደቶችን (የባህሪ ባንዲራ `kaigi_privacy_mocks`)።
  - በሜታዳታ ክስተቶች የሚባዙ አንጸባራቂ ዝመናዎችን ያሰራጩ።
- የአስተናጋጅ የተሳሳተ ውቅረትን የሚሸፍን Trybuild UI ሙከራዎች (ለምሳሌ፣ በግላዊነት ሁነታ ላይ የጠፋ ቅብብሎሽ አንጸባራቂ)።
- ውስን በሆኑ አካባቢዎች (ለምሳሌ ፣ Codex
  ማጠሪያ)፣ የNorito ማሰሪያውን ለማለፍ `NORITO_SKIP_BINDINGS_SYNC=1` ወደ ውጪ ላክ
  የማመሳሰል ፍተሻ በ`crates/norito/build.rs` ተፈጻሚ ነው።

# የስደት እቅድ

1. ✅ ከ`KaigiPrivacyMode::Transparent` ነባሪዎች ጀርባ የመርከብ መረጃ ሞዴል መጨመር።
2. ✅ የሽቦ ባለሁለት መንገድ ማረጋገጫ፡ ምርት `kaigi_privacy_mocks`ን ያሰናክላል፣
   `zk.kaigi_roster_vk`ን ይፈታል፣ እና እውነተኛ ኤንቨሎፕ ማረጋገጫን ያካሂዳል። ፈተናዎች ይችላሉ
   አሁንም ባህሪውን ለ deterministic stubs አንቃ።
3. ✅ የተወሰነውን `kaigi_zk` Halo2 crate፣ የተስተካከለ ጋዝ እና ባለገመድ አስተዋውቋል
   የውህደት ሽፋን እውነተኛ ማረጋገጫዎችን ከጫፍ እስከ ጫፍ ለማስኬድ (ማሾፍ አሁን ለሙከራ-ብቻ ነው)።
4. ⬜ ሁሉም ሸማቾች ቃል ኪዳኖችን ሲረዱ ግልፅ የሆነውን `participants` ቬክተርን ያስወግዱ።

# ጥያቄዎችን ይክፈቱ

- የመርክልን ዛፍ የፅናት ስትራቴጂ ይግለጹ፡ በሰንሰለት ላይ እና በሰንሰለት ላይ (የአሁኑ ዘንበል፡-ከሰንሰለት-ውጭ ዛፍ ላይ በሰንሰለት ላይ ቃል ኪዳን ያለው)። (በKPG-201 ተከታትሏል)*
- የዝውውር መግለጫዎች ባለብዙ ዱካ (በተመሳሳይ ጊዜ የሚደጋገሙ መንገዶች) መደገፍ አለባቸው ወይ የሚለውን ይወስኑ። (በKPG-202 ተከታትሏል)*
- ለቅብብሎሽ ዝናዎች አስተዳደርን ግልጽ ያድርጉ - መጨፍጨፍ ወይም ለስላሳ እገዳዎች እንፈልጋለን? (በKPG-203 ተከታትሏል)*

`KaigiPrivacyMode::ZkRosterV1` በምርት ውስጥ ከማንቃት በፊት እነዚህ ነገሮች መፈታት አለባቸው።