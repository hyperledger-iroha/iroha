---
lang: am
direction: ltr
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T14:35:37.492932+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! በ`roadmap.md:M3` የተጠቀሰው ሚስጥራዊ ንብረት ማዞሪያ የመጫወቻ መጽሐፍ።

# ሚስጥራዊ ንብረት ማዞሪያ Runbook

ይህ የመጫወቻ መጽሐፍ ኦፕሬተሮች ሚስጥራዊ ንብረትን እንዴት እንደሚያቅዱ እና እንደሚያስፈጽሙ ያብራራል።
ሽክርክሪቶች (የመለኪያ ስብስቦች፣ የማረጋገጫ ቁልፎች እና የፖሊሲ ሽግግሮች) ሲሆኑ
የኪስ ቦርሳዎች፣ Torii ደንበኞች እና የሜምፑል ጠባቂዎች ቆራጥነት መቆየታቸውን ማረጋገጥ።

## የህይወት ዑደት እና ሁኔታዎች

ሚስጥራዊ መለኪያ ስብስቦች (`PoseidonParams`፣ `PedersenParams`፣ የማረጋገጫ ቁልፎች)
በተወሰነ ከፍታ ላይ ውጤታማውን ሁኔታ ለማግኘት የሚያገለግሉ ጥልፍልፍ እና ረዳት በቀጥታ ውስጥ
`crates/iroha_core/src/state.rs:7540`–`7561`. የአሂድ ጊዜ ረዳቶች በመጠባበቅ ላይ ናቸው።
የታለመው ቁመት እንደደረሰ ሽግግሮች እና ለበኋላ ምዝግብ አለመሳካቶች
ድጋሚ ስርጭት (`crates/iroha_core/src/state.rs:6725`–`6765`)።

የንብረት ፖሊሲዎች ተካትተዋል።
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
ስለዚህ አስተዳደር በኩል ማሻሻያዎችን ቀጠሮ ይችላል
`ScheduleConfidentialPolicyTransition` እና ካስፈለገ ይሰርዟቸው። ተመልከት
`crates/iroha_data_model/src/asset/definition.rs:320` እና Torii DTO መስተዋቶች
(`crates/iroha_torii/src/routing.rs:1539`–`1580`)።

## የማሽከርከር የስራ ፍሰት

1. ** አዲስ የመለኪያ ቅርቅቦችን ያትሙ።** ኦፕሬተሮች ያስገባሉ።
   `PublishPedersenParams`/`PublishPoseidonParams` መመሪያዎች (CLI)
   `iroha app zk params publish ...`) አዲስ የጄነሬተር ስብስቦችን በሜታዳታ ለማዘጋጀት፣
   የማግበር/የማጥፋት መስኮቶች፣ እና የሁኔታ አመልካቾች። ፈፃሚው አይቀበለውም።
   የተባዙ መታወቂያዎች፣ የማይጨመሩ ስሪቶች ወይም የመጥፎ ሁኔታ ሽግግሮች በ
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499`–`2635`፣ እና እ.ኤ.አ.
   የመመዝገቢያ ሙከራዎች የውድቀት ሁነታዎችን ይሸፍናሉ (`crates/iroha_core/tests/confidential_params_registry.rs:93`-`226`)።
2. ** ቁልፍ ዝመናዎችን መመዝገብ/ማረጋገጥ።
   ቁርጠኝነት እና አንድ ቁልፍ ወደ ውስጥ ከመግባቱ በፊት የወረዳ/ስሪት ገደቦች
   መዝገብ ቤት (`crates/iroha_core/src/smartcontracts/isi/world.rs:2067`–`2137`)።
   ቁልፍን ማዘመን የድሮውን ግቤት በራስ ሰር ያቋርጣል እና የመስመር ላይ ባይት ያብሳል፣
   በ `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1` እንደተለማመደው.
3. ** የንብረት ፖሊሲ ሽግግሮችን መርሐግብር ያውጡ።** አንዴ አዲሶቹ የመለኪያ መታወቂያዎች በቀጥታ ከወጡ
   አስተዳደር ከተፈለገ `ScheduleConfidentialPolicyTransition` ይደውላል
   ሁነታ፣ የሽግግር መስኮት እና የኦዲት ሃሽ። ፈጻሚው ግጭትን አይቀበልም።
   ሽግግሮች ወይም ንብረቶች የላቀ ግልጽ አቅርቦት። ፈተናዎች እንደ
   `crates/iroha_core/tests/confidential_policy_gates.rs:300`–`384` ያረጋግጡ
   የተሰረዙ ሽግግሮች ግልጽ `pending_transition`፣ እያለ
   `confidential_policy_transition_reaches_shielded_only_on_schedule` በ
   Line385-433 የታቀዱ ማሻሻያዎችን በትክክል ወደ `ShieldedOnly` መገልበጥ ያረጋግጣል።
   ውጤታማ ቁመት.
4. **የመመሪያ አፕሊኬሽን እና የሜምፑል ጠባቂ።** የማገጃው አስፈፃሚ ሁሉንም በመጠባበቅ ላይ ያጠፋል።
   በእያንዳንዱ እገዳ (`apply_policy_if_due`) መጀመሪያ ላይ ሽግግሮች እና ይለቃሉ
   ቴሌሜትሪ ሽግግር ካልተሳካ ኦፕሬተሮች እንደገና ቀጠሮ ማስያዝ ይችላሉ። በመግቢያው ወቅት
   ሜምፑል ውጤታማ ፖሊሲቸው በመካከለኛው እገዳ የሚቀይር ግብይቶችን ውድቅ ያደርጋል፣
   በሽግግር መስኮቱ ውስጥ ቆራጥ ማካተት ማረጋገጥ
   (`docs/source/confidential_assets.md:60`)።

## የኪስ ቦርሳ እና የኤስዲኬ መስፈርቶች- ስዊፍት እና ሌሎች የሞባይል ኤስዲኬዎች ንቁውን ፖሊሲ ለማምጣት የTorii አጋሮችን ያጋልጣሉ
  እንዲሁም ማንኛውም በመጠባበቅ ላይ ያለ ሽግግር፣ ስለዚህ የኪስ ቦርሳዎች ከመፈረምዎ በፊት ተጠቃሚዎችን ሊያስጠነቅቁ ይችላሉ። ተመልከት
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) እና ተዛማጅ
  ሙከራዎች በ `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591`.
- CLI ተመሳሳይ ሜታዳታ በ `iroha ledger assets data-policy get` በኩል ያንጸባርቃል (ረዳት በ ውስጥ
  `crates/iroha_cli/src/main.rs:1497`–`1670`) ኦፕሬተሮች ኦዲት እንዲያደርጉ ያስችላቸዋል።
  የፖሊሲ/ፓራሜትር መታወቂያዎች ወደ የንብረት ፍቺ ሳይገለጽ
  የማገጃ መደብር.

## የሙከራ እና የቴሌሜትሪ ሽፋን

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288`–`345` ያንን ፖሊሲ ያረጋግጣል
  ሽግግሮች ወደ ሜታዳታ ቅጽበተ-ፎቶዎች ይሰራጫሉ እና አንዴ ከተተገበሩ ይጸዳሉ።
- `crates/iroha_core/tests/zk_dedup.rs:1` የ `Preverify` መሸጎጫ መሆኑን ያረጋግጣል
  ባለ ሁለት ወጭዎችን/ድርብ-ማስረጃዎችን ውድቅ ያደርጋል፣ የት መዞሪያ ሁኔታዎችን ጨምሮ
  ቃል ኪዳኖች ይለያያሉ።
- `crates/iroha_core/tests/zk_confidential_events.rs` እና
  `zk_shield_transfer_audit.rs` ሽፋን ከጫፍ እስከ ጫፍ ጋሻ → ማስተላለፍ → ያለጋሻ
  ይፈስሳል፣ ይህም የኦዲት ዱካ በመለኪያ ሽክርክሮች ውስጥ መትረፍን ያረጋግጣል።
- `dashboards/grafana/confidential_assets.json` እና
  `docs/source/confidential_assets.md:401` የCommitmentTree ሰነድ እና
  አረጋጋጭ-መሸጎጫ መለኪያዎች ከእያንዳንዱ የካሊብሬሽን/የማሽከርከር ሩጫ ጋር።

## Runbook ባለቤትነት

- ** ዴቭሬል / የኪስ ቦርሳ ኤስዲኬ ይመራል፡** የኤስዲኬ ቅንጣቢዎችን አቆይ + ፈጣን ጅምር የሚያሳዩ
  በመጠባበቅ ላይ ያሉ ሽግግሮችን እንዴት እንደሚታዩ እና ሚንት → ማስተላለፍ → መገለጥ እንዴት እንደሚጫወቱ
  ሙከራዎችን በአገር ውስጥ (በ `docs/source/project_tracker/confidential_assets_phase_c.md:M3.2` ስር ይከታተላል)።
- ** የፕሮግራም Mgmt / ሚስጥራዊ ንብረቶች TL:** የሽግግር ጥያቄዎችን ያጽድቁ ፣ ያቆዩ
  `status.md` በመጪ ሽክርክሮች ተዘምኗል፣ እና መልቀቂያዎችን (ካለ) ያረጋግጡ
  ከካሊብሬሽን ደብተር ጎን የተመዘገበ።