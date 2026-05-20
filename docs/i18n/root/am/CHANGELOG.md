---
lang: am
direction: ltr
source: CHANGELOG.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26f5115a14476de15fbc8f26c5a9807954df6884763a818b2bc98ec6cfe1a4cc
source_last_modified: "2026-01-05T09:28:11.640562+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ለውጥ መዝገብ

[Unreleased]: https://github.com/hyperledger-iroha/iroha/compare/v2.0.0-rc.2.0...HEAD
[2.0.0-rc.2.0]: https://github.com/hyperledger-iroha/iroha/releases/tag/v2.0.0-rc.2.0

በዚህ ፕሮጀክት ላይ ሁሉም ጉልህ ለውጦች በዚህ ፋይል ውስጥ ይመዘገባሉ.

## [ያልተለቀቀ]- የ SCALE ሺም ጣል ያድርጉ; `norito::codec` አሁን በቤተኛ Norito ተከታታይነት ተተግብሯል።
- የ`parity_scale_codec` አጠቃቀሞችን በ `norito::codec` በሳጥኖች ውስጥ ይተኩ።
-የመሳሪያ ስራን ወደ ቤተኛ Norito ተከታታይነት ማዛወር ጀምር።
- የቀረውን `parity-scale-codec` ጥገኝነት ከስራ ቦታ አስወግድ ቤተኛ Norito ተከታታይ።
- የቀረውን የ SCALE የባህርይ መገለጫዎችን በቤተኛ I18NT0000022X ትግበራዎች ይተኩ እና የተሻሻለውን የኮዴክ ሞጁል እንደገና ይሰይሙ።
- `iroha_config_base_derive` እና `iroha_futures_derive` ወደ `iroha_derive` በባህሪ-የተከለሉ ማክሮዎች ያዋህዱ።
- *(መልቲሲግ)* የባለብዙ ሲግ ባለስልጣናት ቀጥተኛ ፊርማዎችን በተረጋጋ የስህተት ኮድ/ምክንያት ውድቅ ያድርጉ፣ ባለብዙ ሲግ ቲቲኤል ካፕዎችን በጎጆ ማሰራጫዎች ላይ ያስፈጽሙ እና ከማቅረቡ በፊት በCLI ውስጥ ላዩን TTL ካፕ (የኤስዲኬ ፓሪቲ በመጠባበቅ ላይ)።
- FFI የሥርዓት ማክሮዎችን ወደ `iroha_ffi` ይውሰዱ እና `iroha_ffi_derive` ሣጥን ያስወግዱ።
- *(schema_gen)* አላስፈላጊ የ`transparent_api` ባህሪን ከ`iroha_data_model` ጥገኝነት ያስወግዱ።
- *(የውሂብ_ሞዴል)* የ ICU NFC መደበኛ መሸጎጫ ለ`Name` ትንተና ተደጋጋሚ ማስጀመሪያን ከአራስ በላይ ለመቀነስ።
- 📚 ለTorii ደንበኛ የ JS ፈጣን ማስጀመሪያን፣ የውቅረት ፈቺን፣ የስራ ፍሰትን እና ውቅረትን የሚያውቅ የምግብ አሰራርን ይመዝግቡ።
- *(IrohaSwift)* ዝቅተኛውን የማሰማራት ኢላማዎች ወደ iOS 15/macOS 12 ያሳድጉ፣ የSwift concurrencyን በTorii የደንበኛ ኤፒአይዎች ይጠቀሙ እና የህዝብ ሞዴሎችን እንደ I18NI0000137X ምልክት ያድርጉ።
- *(IrohaSwift)* `ToriiDaProofSummaryArtifact` እና `DaProofSummaryArtifactEmitter.emit` ታክሏል ስለዚህ ስዊፍት መተግበሪያዎች ከ CLI ጋር ተኳሃኝ የሆኑ የDA ማረጋገጫ ቅርቅቦችን ወደ CLI ሳይለቁ በሰነዶች እና በዲስክ ላይ ሁለቱንም የሚሸፍኑ ሰነዶች እና የድጋሚ ሙከራዎች የስራ ፍሰት s/IrohaSwiftTests/ToriiDaProof ማጠቃለያArtifactTests.swift:1】【F:docs/source/sdk/swift/index.md:260】
- *(የውሂብ_ሞዴል/js_host)* በማህደር የተቀመጠ-እንደገና ጥቅም ላይ የዋለውን ባንዲራ ከ`KaigiParticipantCommitment` በማንሳት የካይጊ አማራጭን ያስተካክሉ፣ ቤተኛ የማዞሪያ ሙከራዎችን ይጨምሩ እና የJS ውድቀትን መፍታት ይጣሉ ስለዚህ Kaigi አሁኑኑ Norito የክብ ጉዞ ጉዞ ያድርጉ። ማስገባት።【F: crates/iroha_data_model/src/kaigi.rs:128】【F:crates/iroha_js_host/src/lib.rs:1379】【F:javascript/iroha_js/test/instructionBuilders.test.js:30】
- *(ጃቫስክሪፕት)* `ToriiClient` ደዋዮች ነባሪ ራስጌዎችን እንዲሰርዙ ይፍቀዱ (`null` በማለፍ) ስለዚህ `getMetrics` በJSON እና Prometheus ጽሑፍ መካከል በንጽህና ይቀያየራል። ራስጌዎች።【F:javascript/iroha_js/src/toriiClient.js:488】【F:javascript/iroha_js/src/toriiClient.js:761】
- *(ጃቫስክሪፕት)* ለኤንኤፍቲዎች ፣በየመለያ የንብረት ቀሪ ሒሳቦች እና የንብረት ፍቺ ያዢዎች (ከታይፕ ስክሪፕት ዴፍስ ፣ ሰነዶች እና ሙከራዎች ጋር) ተጨምረዋል ፣ ስለዚህ Torii ፔጅ አሁን የቀረውን መተግበሪያ ይሸፍናል ። የመጨረሻ ነጥቦች።【F:javascript/iroha_js/src/toriiClient.js:105】【F:javascript/iroha_js/index.d.ts:8 0】【F:javascript/iroha_js/test/toriiClient.test.js:365】【F:javascript/iroha_js/ README.md:470】
- *(ጃቫስክሪፕት)* የአስተዳደር መመሪያ/የግብይት ገንቢዎች እና የአስተዳደር መመሪያ ተጨምሯል ስለዚህ የJS ደንበኞች የውሳኔ ሃሳቦችን፣ የምርጫ ካርዶችን ፣ አዋጁን እና የምክር ቤቱን ጽናት እንዲያቆሙ መጨረሻ።【F:javascript/iroha_js/src/instructionBuilders.js:1012】【F:javascript/iroha_js/src/transaction.js:1082】【F:javascript/iroha_js/recipes/መንግስት.mjs:1
- *(ጃቫስክሪፕት)* ታክሏል ISO 20022 pacs.008 አስረክብ/ሁኔታ አጋዥ እና ተዛማጅ የምግብ አሰራር፣ JS ደዋዮች የ Torii ISO ድልድይ ያለ ኤችቲቲፒ. የቧንቧ ስራ።【F:javascript/iroha_js/src/toriiClient.js:888】【F:javascript/iroha_js/index.d.ts:706】【F:javascript/iroha_js/recipes/iso_bridge.mjs:1】- *(ጃቫስክሪፕት)* ታክሏል pacs.008/pacs.009 ግንበኛ አጋዥ እና በውቅረት የሚመራ የምግብ አዘገጃጀት መመሪያ JS ደዋዮች የ ISO 20022 ክፍያን ከተረጋገጠ BIC/IBAN ሜታዳታ ጋር ማዋሃድ ይችላሉ ድልድይ.【F:javascript/iroha_js/src/isoBridge.js:1】【F:javascript/iroha_js/test/isoBridge.test.js :1】【F:javascript/iroha_js/recipes/iso_bridge_builder.mjs:1】【F:javascript/iroha_js/index.d.ts:1】
- *(ጃቫስክሪፕት)* የDA ingest/fitch/prove loopን ጨርሷል፡ `ToriiClient.fetchDaPayloadViaGateway` አሁን በራስ-ፍሪቭስ ቻንከር እጀታዎችን (በአዲሱ `deriveDaChunkerHandle` ማሰሪያ በኩል)፣ አማራጭ የማሳያ ማጠቃለያዎች የ I18NI000000146X ጥሪዎችን እንደገና መጠቀም እና ኤስዲኤዲኤዲኤክስ ጥሪውን እንደገና መጠቀም ጀመሩ። `iroha da get-blob/prove-availability` ን ያለማሳያ ማንጸባረቅ ይችላል። የቧንቧ ስራ።【F:javascript/iroha_js/src/toriiClient.js:1123】【F:javascript/iroha_js/src/dataAvailability.js:1】【F:javascrip t/iroha_js/test/toriiClient.test.js:1454】【F:javascript/iroha_js/index.d.ts:3275】【F:javascript/iroha_js/README.md:760】
- *(ጃቫስክሪፕት/js_host)* `sorafsGatewayFetch` የውጤት ሰሌዳ ሜታዳታ አሁን የጌትዌይ መታወቂያ/CID አቅራቢዎች ጥቅም ላይ በሚውሉበት ጊዜ ሁሉ ይመዘግባል ስለዚህ የማደጎ ቅርሶች ከ CLI ጋር ይጣጣማሉ ይይዛል።【F:crates/iroha_js_host/src/lib.rs:3017】【F:docs/source/sorafs_orchesterrator_rollout.md:23】
- *(torii/cli)* የ ISO መሻገሪያ መንገዶችን ያስፈጽሙ፡ Torii አሁን `pacs.008` ግቤቶችን ከማይታወቅ ወኪል BICs ጋር ውድቅ ያደርጋል እና የDvP CLI ቅድመ እይታ `--delivery-instrument-id` በ በኩል ያረጋግጣል `--iso-reference-crosswalk`.【F: crates/iroha_torii/src/iso20022_bridge.rs:704】【F: crates/iroha_cli/src/main.rs:3892】
- *(torii)* ከመገንባቱ በፊት `Purp=SECU` እና BIC የማጣቀሻ መረጃ ፍተሻዎችን በማስፈጸም በ`POST /v1/iso20022/pacs009` በኩል የPvP ገንዘብ ማስገባትን ይጨምሩ። ማስተላለፍ
- *(መሳሪያ)* ISIN/CUSIPን፣ BIC↔LEI እና MIC ቅጽበተ-ፎቶዎችን ከማጠራቀሚያው ጋር ለማረጋገጥ `cargo xtask iso-bridge-lint` (ከ`ci/check_iso_reference_data.sh` በተጨማሪ) ታክሏል። fixtures.【F:xtask/src/main.rs:146】【F:ci/Check_iso_ማጣቀሻ_ዳታ.sh:1】
- *(ጃቫስክሪፕት)* የማጠራቀሚያ ሜታዳታን በማወጅ የተጠናከረ npm ህትመት፣ ግልጽ የሆነ የፋይል ፍቃድ ዝርዝር፣ የፕሮቬንቴንስ የነቃ `publishConfig`፣ `prepublishOnly` ለውጥ ሎግ/የሙከራ ጠባቂ እና በመስቀለኛ 18/20 ውስጥ የሚለማመደው GitHub Actions የስራ ፍሰት CI【F:javascript/iroha_js/package.json:1】【F:javascript/iroha_js/scripts/check-changelog.mjs:1】【F:docs/source/sdk/js/publishing.md:1】【F:-gidkvas
- *(ivm/cuda)* BN254 መስክ አክል/ንዑስ/mul አሁን በአዲሱ የCUDA አስኳሎች ላይ በአስተናጋጅ ጎን ባቺንግ በ`bn254_launch_kernel` ያስፈጽማል፣ ይህም የሃርድዌር ማጣደፍን ለPoseidon እና ZK መግብሮች በማንቃት ቆራጥነትን በመጠበቅ ላይ። ውድቀት።【F: crates/ivm/cuda/bn254.cu:1】【F:crates/ivm/src/cuda.rs:66】【F:crates/ivm/src/cuda.rs:1244】

## [2.0.0-rc.2.0] - 2025-05-08

### 🚀 ባህሪዎች

- *(ክሊ)* `iroha transaction get` እና ሌሎች አስፈላጊ ትዕዛዞችን ያክሉ (#5289)
- [** መስበር**] የሚተነፍሱ እና የማይበሰብሱ ንብረቶች (#5308)
- [** መስበር**] ባዶ ያልሆኑ ብሎኮችን ከኋላቸው በመፍቀድ ያጠናቅቁ (#5320)
- የቴሌሜትሪ ዓይነቶችን በሼማ እና በደንበኛ ያጋልጡ (#5387)
- *(iroha_torii)* ለባህሪ-የተከለሉ የመጨረሻ ነጥቦች (#5385)
- የጊዜ መለኪያዎችን ይጨምሩ (#5380)

### 🐛 የሳንካ ጥገናዎች

- ዜሮ ያልሆኑትን ይከልሱ (#5278)
- በሰነድ ሰነዶች ውስጥ ያሉ ጽሑፎች (#5309)
- *(crypto)* `Signature::payload` ጌተርን ያጋልጡ (#5302) (#5310)
- *(ኮር)* ከመስጠትዎ በፊት ለሚና መገኘት ቼኮችን ያክሉ (#5300)
- *(ኮር)* የተቋረጠውን አቻ እንደገና ያገናኙ (#5325)
- ከማከማቻ ንብረቶች እና ከኤንኤፍቲ (#5341) ጋር የተዛመዱ ፒቲስቶችን ያስተካክሉ
- *(CI)* ለግጥም v2 (#5374) የpython static analysis workflow ን አስተካክል።
- ጊዜው ያለፈበት የግብይት ክስተት ከተፈጸመ በኋላ ይታያል (#5396)

### 💼 ሌላ- `rust-toolchain.toml` (#5376) አካትት።
- በ`unused` ላይ አስጠንቅቅ እንጂ `deny` (#5377)

### 🚜 ሪፋክተር

- ጃንጥላ Iroha CLI (#5282)
- *(iroha_test_network)* ለመዝገቦች ቆንጆ ቅርጸት ተጠቀም (#5331)
- [** መስበር**] የ`NumericSpec` ተከታታይነት በ`genesis.json` (#5340) ቀለል ያድርጉት።
- ለተሳነው p2p ግንኙነት (#5379) መግባትን አሻሽል
- `logger.level` አድህር፣ `logger.filter` ጨምር፣ የማዋቀር መንገዶችን አስፋ (#5384)

### 📚 ሰነድ

- `network.public_address` ወደ `peer.template.toml` ያክሉ (#5321)

### ⚡ አፈጻጸም

- *(ኩራ)* ተደጋጋሚ ብሎክ ወደ ዲስክ መፃፍን መከላከል (#5373)
- የተተገበረ ብጁ ማከማቻ ለግብይቶች hashes (#5405)

### ⚙️ የተለያዩ ተግባራት

- የግጥም አጠቃቀምን ያስተካክሉ (#5285)
- ከ`iroha_torii_const` (#5322) ተደጋጋሚ ጉዳቶችን ያስወግዱ
- ጥቅም ላይ ያልዋለ `AssetEvent::Metadata*` ያስወግዱ (#5339)
- Bump Sonarqube Action ስሪት (#5337)
- ጥቅም ላይ ያልዋሉ ፈቃዶችን ያስወግዱ (#5346)
- ጥቅልን ንቀል ወደ ci-image ያክሉ (#5347)
- አንዳንድ አስተያየቶችን ያስተካክሉ (#5397)
- የውህደት ሙከራዎችን ከ`iroha` ሳጥን (#5393) ይውሰዱ
ጉድለት ዶጆ ሥራን አሰናክል (#5406)
- ለጎደሉት ድርጊቶች የDCO ማቋረጥን ያክሉ
- የስራ ፍሰቶችን እንደገና ማደራጀት (ሁለተኛ ሙከራ) (#5399)
- Pull Request CIን ወደ ዋናው በመግፋት አያሂዱ (#5415)

<!-- generated by git-cliff -->

## [2.0.0-rc.1.3] - 2025-03-07

### ታክሏል።

ባዶ ያልሆኑ ብሎኮችን ከነሱ በኋላ በመፍቀድ (#5320) ማጠናቀቅ

## [2.0.0-rc.1.2] - 2025-02-25

### ተስተካክሏል።

- እንደገና የተመዘገቡ እኩዮች አሁን በትክክል በአቻ ዝርዝር ውስጥ ተንጸባርቀዋል (#5327)

## [2.0.0-rc.1.1] - 2025-02-12

### ታክሏል።

- `iroha transaction get` እና ሌሎች አስፈላጊ ትዕዛዞችን ያክሉ (#5289)

## [2.0.0-rc.1.0] - 2024-12-06

### ታክሏል።

- የጥያቄ ትንበያዎችን ይተግብሩ (#5242)
ቀጣይነት ያለው አስፈፃሚ ተጠቀም (#5082)
- በአይሮሃ ክሊ (#5241) ላይ የመስማት ጊዜ ማብቂያዎችን ያክሉ
- add/peers API መጨረሻ ነጥብ ወደ ቶሪ (#5235)
አድራሻ አግኖስቲክ ፒ2p (#5176)
- ባለብዙ ሲግ መገልገያ እና አጠቃቀምን ያሻሽሉ (#5027)
- `BasicAuth::password` እንዳይታተም ይጠብቁ (#5195)
- በ`FindTransactions` መጠይቅ (#5190) ውስጥ የሚወርድ ደርድር
- በእያንዳንዱ ብልጥ የኮንትራት አፈፃፀም አውድ ውስጥ የብሎክ ራስጌን ያስተዋውቁ (#5151)
- በእይታ ለውጥ መረጃ ጠቋሚ (#4957) ላይ የተመሠረተ ተለዋዋጭ የቁርጠኝነት ጊዜ
ነባሪ የፍቃድ ስብስብን ይግለጹ (#5075)
- ለ`Option<Box<R>>` (#5094) የኒቼን ትግበራ ይጨምሩ
- ግብይት እና አግድ ተንታኞች (#5025)
- በመጠይቁ ውስጥ የተቀሩትን እቃዎች መጠን ሪፖርት ያድርጉ (#5016)
- የተወሰነ ጊዜ (#4928)
የጎደሉትን የሂሳብ ስራዎችን ወደ `Numeric` (#4976) ይጨምሩ
የማመሳሰያ መልዕክቶችን አረጋግጥ (#4965)
- የጥያቄ ማጣሪያዎች (#4833)

### ተለውጧል

- የአቻ መታወቂያ መተንተንን ቀላል ማድረግ (#5228)
- የግብይት ስህተትን ከክፍያ ጭነት ውጭ ውሰድ (#5118)
- JsonStringን ወደ Json እንደገና ሰይም (#5154)
- ወደ ዘመናዊ ኮንትራቶች የደንበኛ አካል ይጨምሩ (#5073)
መሪ እንደ ግብይት ማዘዣ አገልግሎት (#4967)
- ኩራ አሮጌ ብሎኮችን ከማስታወሻ እንዲጥል ያድርጉ (#5103)
- በ`Executable` (#5096) ውስጥ ለመመሪያዎች `ConstVec` ይጠቀሙ
- ሐሜት txs ቢበዛ (#5079)
- የ`CommittedTransaction` (#5089) የማህደረ ትውስታ አጠቃቀምን ይቀንሱ
- የጠቋሚ ስህተቶችን የበለጠ ልዩ ያድርጉ (#5086)
- ሳጥኖችን እንደገና ማደራጀት (#4970)
- የ`FindTriggers` መጠይቅን ያስተዋውቁ፣ `FindTriggerById` ያስወግዱ (#5040)
- ለማዘመን በፊርማዎች ላይ አይመሰረቱ (#5039)
- በ genesis.json (#5020) ውስጥ የመለኪያዎችን ቅርጸት ቀይር
- የአሁኑን እና የቀደመውን የእይታ ለውጥ ማረጋገጫ ብቻ ይላኩ (#4929)
- ስራ የሚበዛበትን ዑደት ለመከላከል ዝግጁ በማይሆንበት ጊዜ መልእክት መላክን ያሰናክሉ (#5032)
- አጠቃላይ የንብረት ብዛትን ወደ የንብረት ትርጉም ማንቀሳቀስ (#5029)
- የብሎክን ራስጌ ብቻ ይፈርሙ እንጂ ሙሉውን ክፍያ (#5000)
- `HashOf<BlockHeader>` እንደ የማገጃ ሃሽ አይነት ይጠቀሙ (#4998)
- `/health` እና `/api_version` (#4960) ቀለል ያድርጉት
- `configs` ወደ `defaults` እንደገና ይሰይሙ፣ `swarm` ያስወግዱ (#4862)

### ተስተካክሏል።- ጠፍጣፋ ውስጣዊ ሚና በ json (#5198)
- `cargo audit` ማስጠንቀቂያዎችን ያስተካክሉ (#5183)
- የክልል ፍተሻን ወደ ፊርማ መረጃ ጠቋሚ አክል (#5157)
የሞዴል ማክሮ ምሳሌን በሰነዶች ያስተካክሉ (#5149)
- በብሎኮች/በክስተቶች ዥረት ውስጥ wsን በትክክል ይዝጉ (#5101)
- የተሰበረ የታመኑ አቻዎች ቼክ (#5121)
- ቀጣዩ ብሎክ ቁመት +1 (#5111) እንዳለው ያረጋግጡ
- የዘፍጥረት ብሎክ የጊዜ ማህተም አስተካክል (#5098)
- የ`iroha_genesis` ቅንብርን ያለ `transparent_api` ባህሪ (#5056) ያስተካክሉ
- በትክክል `replace_top_block` (#4870) ይያዙ
የአስፈፃሚውን ክሎኒንግ ማስተካከል (#4955)
- ተጨማሪ የስህተት ዝርዝሮችን አሳይ (#4973)
- ለብሎኮች ዥረት `GET` ይጠቀሙ (#4990)
- የወረፋ ግብይቶችን አያያዝ ማሻሻል (#4947)
- ተደጋጋሚ የማገጃ መልዕክቶችን መከላከል (#4909)
- በአንድ ጊዜ ትልቅ መልእክት መላክ ላይ መዘጋትን መከላከል (#4948)
- ጊዜው ያለፈበት ግብይት ከመሸጎጫ ያስወግዱ (#4922)
- የቶሪ ዩአርኤልን በመንገዱ ያስተካክሉ (#4903)

### ተወግዷል

- በሞጁል ላይ የተመሰረተ ኤፒአይ ከደንበኛው ያስወግዱ (#5184)
- `riffle_iter` ያስወግዱ (#5181)
- ጥቅም ላይ ያልዋሉ ጥገኞችን ያስወግዱ (#5173)
- የ`max` ቅድመ ቅጥያ ከ`blocks_in_memory` ያስወግዱ (#5145)
- የጋራ መግባባት ግምትን ያስወግዱ (#5116)
- `event_recommendations`ን ከብሎክ ያስወግዱ (#4932)

## ደህንነት

## [2.0.0-ቅድመ-rc.22.1] - 2024-07-30

### ተስተካክሏል።

- ወደ ዶከር ምስል `jq` ታክሏል።

## [2.0.0-ቅድመ-rc.22.0] - 2024-07-25

### ታክሏል።

- በሰንሰለት ላይ ያሉትን መለኪያዎች በዘፍጥረት (#4812) በግልጽ ይግለጹ
- ተርቦፊሽ ከበርካታ `Instruction`s (#4805) ጋር ፍቀድ
- ባለብዙ ፊርማ ግብይቶችን እንደገና መፈጸም (#4788)
አብሮ የተሰራ እና ብጁ በሰንሰለት ላይ መለኪያዎችን ይተግብሩ (#4731)
- ብጁ መመሪያ አጠቃቀምን ማሻሻል (#4778)
- JsonString (#4732) በመተግበር ሜታዳታውን ተለዋዋጭ ያድርጉት
- ብዙ እኩዮች የዘፍጥረት እገዳ እንዲያቀርቡ ፍቀድ (#4775)
- ለእኩያ ከ `SignedTransaction` ይልቅ `SignedBlock` ያቅርቡ (#4739)
- በአፈፃፀም ውስጥ ብጁ መመሪያዎች (#4645)
- የ json መጠይቆችን ለመጠየቅ ደንበኛ ክሊን ያራዝሙ (#4684)
 - ለ`norito_decoder` (#4680) የማወቂያ ድጋፍን ይጨምሩ
- የውሂብ ሞዴልን ወደ አስፈፃሚ የፈቃድ ንድፍ አጠቃላይ (#4658)
- በነባሪ አስፈፃሚው ውስጥ የመመዝገቢያ ቀስቃሽ ፈቃዶችን ታክሏል (#4616)
 - JSON በ I18NI0000203X ውስጥ ይደግፉ
- የp2p የስራ ፈት ጊዜ ማብቂያ ጊዜን ያስተዋውቁ

### ተለውጧል

- `lol_alloc` በ `dlmalloc` (#4857) ተካ
- `type_` ወደ `type` እንደገና ይሰይሙ (#4855)
- `Duration`ን በ `u64` ተካ (#4841)
- ለመመዝገቢያ `RUST_LOG`-እንደ EnvFilter ይጠቀሙ (#4837)
- በሚቻልበት ጊዜ የድምጽ መስጫ እገዳን ይቀጥሉ (#4828)
- ከዋርፕ ወደ አክሱም መሰደድ (#4718)
- የተከፈለ አስፈፃሚ ውሂብ ሞዴል (#4791)
- ጥልቀት የሌለው የውሂብ ሞዴል (#4734) (#4792)
- የህዝብ ቁልፍ በፊርማ (#4518) አይላኩ
- `--outfile` ወደ `--out-file` (#4679) እንደገና ሰይም
- የኢሮሃ አገልጋይ እና ደንበኛን እንደገና ይሰይሙ (#4662)
- `PermissionToken` ወደ `Permission` (#4635) እንደገና ሰይም
- በጉጉት `BlockMessages` ውድቅ (#4606)
- `SignedBlock` የማይለወጥ አድርግ (#4620)
- TransactionValue ወደ ኮሚትድ ግብይት (#4610) እንደገና ይሰይሙ
- የግል መለያዎችን በመታወቂያ (#4411) ያረጋግጡ
- ለግል ቁልፎች መልቲሃሽ ቅርጸት ይጠቀሙ (#4541)
 - `parity_scale_decoder` ወደ `norito_cli` እንደገና ሰይም
- ብሎኮችን ወደ Set B validators ላክ
- `Role` ግልፅ አድርግ (#4886)
- ከራስጌ (#4890) የብሎክ ሃሽ ያውጡ

### ተስተካክሏል።- ለማስተላለፍ ባለስልጣኑ ጎራ እንዳለው ያረጋግጡ (#4807)
- የመግቢያ ድርብ ማስጀመሪያን ያስወግዱ (#4800)
- የንብረቶች እና ፈቃዶች ስም አሰጣጥን ማስተካከል (#4741)
በዘፍጥረት ብሎክ (#4757) በተለየ ግብይት ፈፃሚውን ማሻሻል
- ትክክለኛ ነባሪ ዋጋ ለ I18NI0000220X (#4692)
- የስህተት መልእክት ማሻሻል (#4659)
- ያለፈው Ed25519Sha512 የህዝብ ቁልፍ ልክ ያልሆነ ርዝመት (#4650) ከሆነ አትደናገጡ።
በመግቢያው ላይ ትክክለኛውን የእይታ ለውጥ መረጃ ጠቋሚ ይጠቀሙ (#4612)
- ከ `start` የጊዜ ማህተም (#4333) በፊት ጊዜ-ቀስቃሾችን ያለጊዜው አይፈጽሙ።
- ድጋፍ `https` ለ `torii_url` (#4601) (#4617)
- serde (ጠፍጣፋ)ን ከሴትKeyValue/የቁልፍ እሴትን አስወግድ (#4547)
- ቀስቅሴ ስብስብ በትክክል ተከታታይ ነው
- የተወገዱ `PermissionToken`s በ`Upgrade<Executor>` (#4503) ላይ መሻር
- ለአሁኑ ዙር ትክክለኛ የእይታ ለውጥ ኢንዴክስ ሪፖርት ያድርጉ
- በ `Unregister<Domain>` (#4461) ላይ ተጓዳኝ ቀስቅሴዎችን ያስወግዱ
- በዘፍጥረት ዙር የጄኔሲስ መጠጥ ቤት ቁልፍን ያረጋግጡ
- ዘፍጥረት Domain ወይም Account መመዝገብን ይከለክላል
- በህጋዊ አካል ምዝገባ ላይ ካሉ ሚናዎች ፈቃዶችን ያስወግዱ
- ቀስቃሽ ሜታዳታ በዘመናዊ ኮንትራቶች ውስጥ ተደራሽ ነው።
- ወጥነት የሌለውን የግዛት እይታ ለመከላከል rw መቆለፊያን ይጠቀሙ (#4867)
- ለስላሳ ሹካ በቅጽበት ይያዙ (#4868)
- MinSize ለ ChaCha20Poly1305 ያስተካክሉ
- ከፍተኛ የማህደረ ትውስታ አጠቃቀምን ለመከላከል በ LiveQueryStore ላይ ገደቦችን ያክሉ (#4893)

### ተወግዷል

- የህዝብ ቁልፍን ከ ed25519 የግል ቁልፍ ያስወግዱ (#4856)
- kura.lockን ያስወግዱ (#4849)
- በማዋቀር ውስጥ `_ms` እና `_bytes` ቅጥያዎችን አድህር (#4667)
- ከዘፍጥረት መስኮች `_id` እና `_file` ቅጥያ ያስወግዱ (#4724)
- ኢንዴክስ ንብረቶችን በ AssetsMap በ AssetDefinitionId ያስወግዱ (#4701)
- ጎራውን ከቀስቃሽ ማንነት ያስወግዱ (#4640)
- የዘር ፊርማውን ከIroha ያስወግዱ (#4673)
- ከ `Validate` የታሰረውን `Visit` ያስወግዱ (#4642)
- `TriggeringEventFilterBox` ያስወግዱ (#4866)
- በp2p እጅ መጨባበጥ `garbage` ያስወግዱ (#4889)
- `committed_topology`ን ከብሎክ ያስወግዱ (#4880)

## ደህንነት

- ከሚስጢር መፍሰስ ይጠብቁ

## [2.0.0-ቅድመ-rc.21] - 2024-04-19

### ታክሏል።

- ቀስቅሴ መታወቂያ በመግቢያ ነጥብ ውስጥ ያካትቱ (#4391)
- በክስተቱ ውስጥ እንደ ቢትፊልድ የተዘጋጀውን ክስተት አጋልጥ (#4381)
- ከጥራጥሬ መዳረሻ (#2664) ጋር አዲስ `wsv` ያስተዋውቁ
- ለ`PermissionTokenSchemaUpdate`፣ `Configuration` እና `Executor` ክስተቶች የክስተት ማጣሪያዎችን ያክሉ
- ቅጽበታዊ ገጽ እይታን ያስተዋውቁ "ሞድ" (#4365)
- የሚና ፈቃዶችን መስጠት/መሻር ፍቀድ (#4244)
- የዘፈቀደ-ትክክለኛ የቁጥር አይነት ለንብረቶች ያስተዋውቁ (ሌሎች የቁጥር አይነቶችን ያስወግዱ) (#3660)
- ለአስፈጻሚው የተለየ የነዳጅ ገደብ (#3354)
- የ ppprof ፕሮፋይል አዋህድ (#4250)
- በደንበኛ CLI ውስጥ የንብረት ንዑስ ትዕዛዝ ያክሉ (#4200)
- `Register<AssetDefinition>` ፈቃዶች (#4049)
- የመድገም ጥቃቶችን ለመከላከል `chain_id` ይጨምሩ (#4185)
- በደንበኛ CLI ውስጥ የጎራ ዲበ ውሂብን ለማርትዕ ንዑስ ትዕዛዞችን ያክሉ (#4175)
- የመደብር ስብስብን ይተግብሩ ፣ ያስወግዱ ፣ በ Client CLI ውስጥ ሥራዎችን ያግኙ (#4163)
- ለመቀስቀስ ተመሳሳይ ዘመናዊ ኮንትራቶችን ይቁጠሩ (#4133)
- ጎራዎችን ለማስተላለፍ ንዑስ ትዕዛዝ ወደ ደንበኛ CLI ያክሉ (#3974)
- በFFI ውስጥ የሳጥን ቁርጥራጮችን ይደግፉ (#4062)
- git SHA ለደንበኛ CLI (#4042)
- ፕሮክ ማክሮ ለነባሪ አረጋጋጭ ቦይለር (#3856)
- የመጠይቅ ጥያቄ ገንቢን ወደ ደንበኛ ኤፒአይ (#3124) አስተዋወቀ።
- ብልጥ ኮንትራቶች ውስጥ ሰነፍ መጠይቆች (#3929)
- `fetch_size` መጠይቅ መለኪያ (#3900)
- የንብረት ማከማቻ መመሪያ (#4258)
- ከሚስጥር መፍሰስ ይጠብቁ (#3240)
- የተቀነሱ ቀስቅሴዎች በተመሳሳይ የምንጭ ኮድ (#4419)

### ተለውጧል- የዝገት መሣሪያ ሰንሰለት እስከ ማታ - 2024-04-18
- ብሎኮችን ወደ Set B validators ላክ (#4387)
- የቧንቧ መስመር ክስተቶችን ወደ እገዳ እና የግብይት ክስተቶች ከፋፍለው (#4366)
- የ `[telemetry.dev]` ውቅር ክፍልን ወደ `[dev_telemetry]` (#4377) እንደገና ይሰይሙ
- `Action` እና `Filter` አጠቃላይ ያልሆኑ ዓይነቶችን ያድርጉ (#4375)
- የክስተት ማጣሪያ ኤፒአይን ከገንቢ ጥለት ጋር አሻሽል (#3068)
- የተለያዩ የክስተት ማጣሪያ ኤፒአይዎችን አንድ ማድረግ፣ አቀላጥፎ ገንቢ ኤፒአይ ያስተዋውቁ
- `FilterBox` ወደ `EventFilterBox` እንደገና ይሰይሙ
- `TriggeringFilterBox` ወደ `TriggeringEventFilterBox` እንደገና ይሰይሙ
- የማጣሪያ መሰየምን ማሻሻል፣ ለምሳሌ `AccountFilter` -> `AccountEventFilter`
- በአወቃቀሩ RFC (#4239) መሠረት ውቅረትን እንደገና ይፃፉ
- የተሻሻሉ መዋቅሮችን ውስጣዊ መዋቅር ከህዝብ ኤፒአይ ደብቅ (#3887)
ብዙ ያልተሳኩ የእይታ ለውጦች (#4263) በኋላ ሊገመት የሚችል ማዘዣን ለጊዜው ያስተዋውቁ (#4263)
- በ `iroha_crypto` (#4181) ውስጥ የኮንክሪት ቁልፍ ዓይነቶችን ይጠቀሙ
- ከመደበኛ መልዕክቶች የተከፈለ እይታ ለውጦች (#4115)
- `SignedTransaction` የማይለወጥ አድርግ (#4162)
- `iroha_config` ወደ `iroha_client` ወደ ውጪ ላክ (#4147)
- `iroha_crypto` ወደ `iroha_client` ወደ ውጪ ላክ (#4149)
- `data_model` ወደ `iroha_client` ወደ ውጪ ላክ (#4081)
- የ`openssl-sys` ጥገኝነትን ከ`iroha_crypto` ያስወግዱ እና የሚዋቀሩ tls backendsን ወደ `iroha_client` (#3422) ያስተዋውቁ
- ያልተጠበቀ EOF I18NI0000264X በቤት ውስጥ መፍትሄ ይተኩ `iroha_crypto` (#3422)
- የአስፈፃሚውን አፈፃፀም ያሳድጉ (#4013)
- ቶፖሎጂ አቻ ማሻሻያ (#3995)

### ተስተካክሏል።

- በ `Unregister<Domain>` (#4461) ላይ ተጓዳኝ ቀስቅሴዎችን ያስወግዱ
- በህጋዊ አካል ምዝገባ ላይ ካሉ ሚናዎች ፈቃዶችን ያስወግዱ (#4242)
- የዘፍጥረት ግልባጭ የተፈረመው በዘፍጥረት pub ቁልፍ (#4253) መሆኑን አስረግጠው
- ምላሽ ለማይሰጡ እኩዮች የጊዜ ማብቂያ ጊዜን በp2p ያስተዋውቁ (#4267)
- የዘፍጥረት ዶሜይን ወይም መለያ (#4226) መመዝገብን ይከለክላል።
- `MinSize` ለ `ChaCha20Poly1305` (#4395)
- `tokio-console` ሲነቃ ኮንሶል ይጀምሩ (#4377)
- እያንዳንዱን ንጥል በ`\n` ይለዩ እና ለ`dev-telemetry` ፋይል ምዝግብ ማስታወሻዎች የወላጅ ማውጫዎችን ደጋግመው ይፍጠሩ
- ያለ ፊርማ የመለያ ምዝገባን መከላከል (#4212)
- የቁልፍ ጥንድ ትውልድ አሁን የማይሳሳት ነው (#4283)
- የ `X25519` ቁልፎችን እንደ `Ed25519` (#4174) መመስጠር አቁም
- በ `no_std` (#4270) ውስጥ የፊርማ ማረጋገጫን ያድርጉ
- የማገጃ ዘዴዎችን በተዛመደ አውድ ውስጥ መጥራት (#4211)
- በህጋዊ አካል አለመመዝገቢያ (#3962) ላይ ተዛማጅ ምልክቶችን መሻር
- Sumeragi ሲጀምር አsync blocking bug
- ቋሚ `(get|set)_config` 401 HTTP (#4177)
- `musl` የመዝገብ ቤት ስም በI18NT0000039X (#4193)
- ብልጥ የውል ማረም ህትመት (#4178)
- እንደገና ሲጀመር የቶፖሎጂ ዝመና (#4164)
- የአዳዲስ እኩዮች ምዝገባ (#4142)
- በሰንሰለት ላይ ሊገመት የሚችል የድግግሞሽ ቅደም ተከተል (#4130)
- እንደገና አርክቴክት ሎገር እና ተለዋዋጭ ውቅር (#4100)
- ቀስቃሽ አቶሚዝም (#4106)
- የጥያቄ መደብር መልእክት ማዘዝ ችግር (#4057)
- Norito በመጠቀም ምላሽ ለሚሰጡ የመጨረሻ ነጥቦች `Content-Type: application/x-norito` ያዘጋጁ

### ተወግዷል

- `logger.tokio_console_address` የውቅር መለኪያ (#4377)
- `NotificationEvent` (#4377)
- `Value` ቁጥር (#4305)
- MST ድምር ከአይሮሃ (#4229)
- ለ ISI ክሎኒንግ እና በዘመናዊ ኮንትራቶች ውስጥ መጠይቅ አፈፃፀም (#4182)
- `bridge` እና `dex` ባህሪያት (#4152)
- ጠፍጣፋ ክስተቶች (#3068)
- መግለጫዎች (#4089)
- በራስ-የመነጨ ውቅር ማጣቀሻ
- `warp` ጫጫታ በምዝግብ ማስታወሻዎች ውስጥ (#4097)

## ደህንነት

- በ p2p (#4065) ውስጥ የመጠጫ ቤት ቁልፍን ማፈንዳትን ይከላከሉ
- ከOpenSSL የሚመጡ የ`secp256k1` ፊርማዎች መደበኛ መሆናቸውን ያረጋግጡ (#4155)

## [2.0.0-ቅድመ-rc.20] - 2023-10-17

### ታክሏል።- የ `Domain` ባለቤትነትን ያስተላልፉ
- `Domain` የባለቤት ፍቃዶች
- የ `owned_by` መስክ ወደ I18NI0000288X ያክሉ
- ማጣሪያን እንደ JSON5 በ`iroha_client_cli` (#3923) ይተን
- በከፊል መለያ በተሰየሙ በ serde ውስጥ የራስ ዓይነትን ለመጠቀም ድጋፍን ይጨምሩ
- የማገጃ ኤፒአይ መደበኛ አድርግ (#3884)
- `Fast` kura init ሁነታን ተግብር
- የiroha_swarm ማስተባበያ ራስጌ ያክሉ
- ለ WSV ቅጽበተ-ፎቶዎች የመጀመሪያ ድጋፍ

### ተስተካክሏል።

- በ update_configs.sh (#3990) ውስጥ የማስፈጸሚያ ማውረድን ያስተካክሉ
- ትክክለኛ rustc በ devShell ውስጥ
- ማቃጠል `Trigger` rertitions ያስተካክሉ
- ማስተላለፍ `AssetDefinition` ያስተካክሉ
- ለ`Domain` `RemoveKeyValue` አስተካክል
- የ `Span::join` አጠቃቀምን ያስተካክሉ
- የቶፖሎጂ አለመዛመድ ስህተትን ያስተካክሉ (#3903)
- `apply_blocks` እና `validate_blocks` ማመሳከሪያን ያስተካክሉ
- `mkdir -r` ከሱቅ መንገድ ጋር እንጂ የመቆለፊያ መንገድ አይደለም (#3908)
- dir test_env.py ውስጥ ካለ አትወድም።
- የማረጋገጫ/የፈቀዳ ሰነድ ያስተካክሉ (#3876)
- ለጥያቄ ፍለጋ ስህተት የተሻለ የስህተት መልእክት
- ዲቪ ዶከር ለመጻፍ የዘፍጥረት መለያ ይፋዊ ቁልፍ ያክሉ
- የፈቃድ ማስመሰያ ክፍያን እንደ JSON ያወዳድሩ (#3855)
- በ `#[model]` ማክሮ ውስጥ `irrefutable_let_patterns` አስተካክል
- ዘፍጥረት ማንኛውንም አይኤስአይ (#3850) እንዲፈጽም ፍቀድ
- የዘፍጥረት ማረጋገጫን ያስተካክሉ (#3844)
- ለ 3 ወይም ከዚያ በታች ለሆኑ እኩዮች ቶፖሎጂን ያስተካክሉ
- tx_amounts ሂስቶግራም እንዴት እንደሚሰላ አስተካክል።
- `genesis_transactions_are_validated()` ሙከራ flakiness
- ነባሪ አረጋጋጭ ማመንጨት
- የኢሮሃ ግርማ ሞገስ ያለው መዘጋት ያስተካክሉ

### አነቃቂ- ጥቅም ላይ ያልዋሉ ጥገኞችን ያስወግዱ (#3992)
- የተደናቀፈ ጥገኛ (#3981)
- አረጋጋጭን ወደ ፈጻሚው እንደገና ይሰይሙ (#3976)
- `IsAssetDefinitionOwner` አስወግድ (#3979)
- ብልጥ የኮንትራት ኮድን በስራ ቦታ ላይ ያካትቱ (#3944)
- ኤፒአይ እና ቴሌሜትሪ የመጨረሻ ነጥቦችን ወደ አንድ አገልጋይ ያዋህዱ
- አገላለጽ ሌንስን ከሕዝብ ኤፒአይ ወደ ዋና ይውሰዱ (#3949)
- ሚናዎች ፍለጋ ውስጥ cloneን ያስወግዱ
- ለሚናዎች የክልል መጠይቆች
- የመለያ ሚናዎችን ወደ `WSV` ይውሰዱ
- ISIን ከ * ሳጥን ወደ * ኤክስፕር (#3930) እንደገና ይሰይሙ
- 'የተሰራ' ቅድመ ቅጥያ ከተዘጋጁት መያዣዎች ያስወግዱ (#3913)
- `commit_topology` ወደ የማገጃ ጭነት (#3916) ይውሰዱ
- `telemetry_future` ማክሮ ወደ ሲን 2.0 ማዛወር
- በ ISI ወሰኖች ውስጥ በሚለይ የተመዘገበ (#3925)
- መሰረታዊ የጄኔቲክስ ድጋፍን ወደ `derive(HasOrigin)` ያክሉ
- ክሊፕን ደስተኛ ለማድረግ የ Emitter APIs ሰነዶችን ያጽዱ
- ለዲሪቭ(HasOrigin) ማክሮ ሙከራዎችን ይጨምሩ ፣ በderive (IdEqOrdHash) ውስጥ መደጋገምን ይቀንሱ ፣ በተረጋጋ ላይ የስህተት ሪፖርት ያስተካክሉ
- ስም አወጣጥ አሻሽል፣ ተደጋጋሚ .filter_maps አቅልለው እና ከማስረጃ (ማጣሪያ) በስተቀር አላስፈላጊ የሆኑትን አስወግድ።
- በከፊል መለያ የተደረገ ተከታታይ አድርግ/ተጠቀም ውዴ
- ዳሪቭ (IdEqOrdHash) ውዴ ይጠቀሙ፣ ሙከራዎችን ይጨምሩ
- መውደድን (ማጣሪያ) ይጠቀሙ
- ሲን 2.0 ለመጠቀም የiroha_data_model_derive ያዘምኑ
- የፊርማ ማረጋገጫ ሁኔታ ክፍል ሙከራዎችን ያክሉ
- የተወሰነ የፊርማ ማረጋገጫ ሁኔታዎችን ብቻ ፍቀድ
- ConstBytesን ወደ ConstVec ያጠቃልሉ ማንኛውንም ተከታታይ ቅደም ተከተል ይይዛል
- ላልተቀየሩ ባይት እሴቶች የበለጠ ቀልጣፋ ውክልና ተጠቀም
- የተጠናቀቀውን wsv በቅጽበት ያከማቹ
- `SnapshotMaker` ተዋናይ አክል
- በፕሮc ማክሮዎች ውስጥ የመተንተን ሰነድ ውስንነት
- አስተያየቶችን አጽዳ
- ለlib.rs ባህሪያትን ለመተንተን የተለመደ የሙከራ መገልገያ ማውጣት
- parse_display ተጠቀም እና Attr አዘምን -> Attrs መሰየም
- በ ffi function args ውስጥ የስርዓተ-ጥለት ማዛመድን ይፍቀዱ
- በ getset atters መተንተን ውስጥ መደጋገምን ይቀንሱ
- Emitter :: ወደ ቶከን_ዥረት ወደ Emitter :: ጨርስ_ቶከን_ዥረት እንደገና ይሰይሙ
- የጌሴት ቶከኖችን ለመተንተን parse_display ይጠቀሙ
- የፊደል ስህተቶችን ያስተካክሉ እና የስህተት መልዕክቶችን ያሻሽሉ።
- iroha_ffi_derive: ባህሪያትን ለመተንተን እና syn 2.0 ን ለመጠቀም ዳርሊን ይጠቀሙ
- iroha_ffi_derive፡ ፕሮክ-ማክሮ-ስህተትን በብዙ መንገድ ይተኩ
- የኩራ መቆለፊያ ፋይል ኮድን ቀለል ያድርጉት
- ሁሉንም የቁጥር እሴቶች እንደ የሕብረቁምፊ ቃል በቃል ተከታታይ እንዲሆኑ ያድርጉ
- የተከፈለ Kagami (#3841)
- `scripts/test-env.sh` እንደገና ይፃፉ
- ብልጥ ውል እና ቀስቅሴ መግቢያ ነጥቦች መካከል ያለውን ልዩነት
- Elide `.cloned()` በ `data_model/src/block.rs`
- ሲን 2.0 ለመጠቀም `iroha_schema_derive` ያዘምኑ

## [2.0.0-ቅድመ-rc.19] - 2023-08-14

### ታክሏል።- hyperledger#3309 Bump IVM የሩጫ ጊዜ ለተሻሻለ
- hyperledger#3383 የሶኬት አድራሻዎችን በማጠናቀር ጊዜ ለመተንተን ማክሮን ይተግብሩ
- hyperledger#2398 ለጥያቄ ማጣሪያዎች የውህደት ሙከራዎችን ያክሉ
- ትክክለኛውን የስህተት መልእክት በ `InternalError` ውስጥ ያካትቱ
- የ `nightly-2023-06-25` እንደ ነባሪ የመሳሪያ ሰንሰለት አጠቃቀም
- hyperledger # 3692 አረጋጋጭ ፍልሰት
- [DSL internship] hyperledger#3688፡ መሰረታዊ ሂሳብን እንደ ፕሮክ ማክሮ ይተግብሩ።
- hyperledger#3371 ስፕሊት አረጋጋጭ `entrypoint` አረጋጋጮች ከአሁን በኋላ እንደ ብልጥ ኮንትራቶች እንደማይታዩ ለማረጋገጥ
- hyperledger#3651 WSV ቅጽበተ-ፎቶዎች፣ ይህም ከብልሽት በኋላ Iroha መስቀለኛ መንገድን በፍጥነት ለማምጣት ያስችላል።
- hyperledger#3752 `MockValidator` ሁሉንም ግብይቶች በሚቀበል `Initial` አረጋጋጭ ይተኩ
- hyperledger#3276 የተወሰነ ሕብረቁምፊ ወደ ዋናው የ Iroha መስቀለኛ መንገድ የሚመዘግብ `Log` የሚባል ጊዜያዊ መመሪያ ያክሉ
- hyperledger # 3641 የፍቃድ ማስመሰያ ጭነት በሰው ሊነበብ የሚችል ያድርጉት
- hyperledger#3324 `iroha_client_cli` ተዛማጅ `burn` ቼኮችን እና ማደስን ይጨምሩ
- hyperledger # 3781 የዘፍጥረት ግብይቶችን ያረጋግጡ
- hyperledger # 2885 ለመቀስቀስ ጥቅም ላይ ሊውሉ በሚችሉ እና በማይችሉ ክስተቶች መካከል ያለውን ልዩነት ይለዩ
- hyperledger#2245 I18NI0000320X ላይ የተመሠረተ የኢሮሃ ኖድ ሁለትዮሽ ግንባታ እንደ `AppImage`

### ተስተካክሏል።

- hyperledger#3613 ሪግሬሽን ይህም በስህተት የተፈረሙ ግብይቶችን ለመቀበል ያስችላል
- የተሳሳተ የውቅር ቶፖሎጂን ቀደም ብለው ውድቅ ያድርጉ
- hyperledger#3445 ሪግሬሽን አስተካክል እና `POST` በ `/configuration` የመጨረሻ ነጥብ ላይ እንደገና እንዲሰራ አድርግ
- hyperledger#3654 መጠገን `iroha2` `glibc` ላይ የተመሠረተ `Dockerfiles` እንዲሰማራ
- hyperledger#3451 አስተካክል `docker` ግንባታ በአፕል ሲሊኮን ማክስ ላይ
- hyperledger#3741 `tempfile` ስህተትን በ`kagami validator` አስተካክል።
- hyperledger#3758 የግለሰቦች ሳጥኖች ሊገነቡ የማይችሉትን ነገር ግን እንደ የስራ ቦታ አካል ሊገነቡ የሚችሉበትን ሪግሬሽን ያስተካክሉ።
- hyperledger# 3777 የፔች ቀዳዳ በሚና ምዝገባ ላይ እየተረጋገጠ አይደለም።
- hyperledger#3805 አስተካክል Iroha `SIGTERM` ከተቀበለ በኋላ አይዘጋም

### ሌላ

- hyperledger#3648 በ CI ሂደቶች ውስጥ `docker-compose.*.yml` ቼክን ያካትቱ
- መመሪያን I18NI0000332X ከ `iroha_data_model` ወደ `iroha_core` ይውሰዱ
- hyperledger#3672 `HashMap` በ `FxHashMap` በመነጩ ማክሮዎች ይተኩ
- hyperledger#3374 የስህተት ሰነዶችን አስተያየቶችን እና የ`fmt::Display` ትግበራን አንድ አድርግ
- hyperledger#3289 ዝገት 1.70 የመስሪያ ቦታ ውርስ በፕሮጀክት በሙሉ ይጠቀሙ
- hyperledger#3654 `Dockerfiles` ይጨምሩ `GNU libc <https://www.gnu.org/software/libc/>`_
- `syn` 2.0፣ `manyhow` እና `darling`ን ለፕሮክ-ማክሮዎች ያስተዋውቁ
- hyperledger # 3802 ዩኒኮድ `kagami crypto` ዘር

## [2.0.0-ቅድመ-rc.18]

### ታክሏል።

- hyperledger#3468፡ የአገልጋይ ጎን ጠቋሚ፣ ይህም ለጥያቄ መዘግየት ትልቅ አወንታዊ አንድምታ ያለው በሰነፍ የተገመገመ ድጋሚ የገባ pagination ያስችላል።
- hyperledger # 3624: አጠቃላይ ዓላማ ፈቃድ ማስመሰያዎች; በተለይ
  - የፍቃድ ማስመሰያዎች ማንኛውም መዋቅር ሊኖራቸው ይችላል
  - የማስመሰያ መዋቅር በራሱ በ`iroha_schema` ውስጥ ተገልጿል እና እንደ JSON ሕብረቁምፊ ተከታታይ ነው.
  - የማስመሰያ ዋጋ `Norito`-የተመሰጠረ ነው።
  - በዚህ ለውጥ ምክንያት የፍቃድ ማስመሰያ ስምምነቶች ከ `snake_case` ወደ `UpeerCamelCase` ተንቀሳቅሷል
- hyperledger # 3615 ከተረጋገጠ በኋላ wsv ን ይቆጥቡ

### ተስተካክሏል።- hyperledger#3627 የግብይት atomicity አሁን በ `WorlStateView` ክሎኒንግ ተፈጻሚ ሆኗል
- hyperledger#3195 ውድቅ የተደረገ የዘፍጥረት ግብይት ሲቀበሉ የፍርሃት ባህሪን ያራዝሙ
- hyperledger#3042 መጥፎ የጥያቄ መልእክት አስተካክል።
- hyperledger#3352 የቁጥጥር ፍሰት እና የውሂብ መልእክት ወደ ተለያዩ ቻናሎች ይከፋፍሉ።
- hyperledger#3543 የመለኪያዎችን ትክክለኛነት ያሻሽሉ።

## 2.0.0-ቅድመ-rc.17

### ታክሏል።

- hyperledger#3330 `NumericValue` ማድረቂያን ዘርጋ
- hyperledger#2622 I18NI0000350X/`i128` ድጋፍ በFFI
- hyperledger#3088 ዶኤስን ለመከላከል ወረፋ ስሮትልንግን አስተዋውቁ
- hyperledger#2373 I18NI0000352X እና `kagami swarm dir` የ`docker-compose` ፋይሎችን ለመፍጠር የትዕዛዝ ልዩነቶች
- hyperledger#3597 የፍቃድ ማስመሰያ ትንተና (Iroha ጎን)
- hyperledger#3353 የስህተት ሁኔታዎችን በመዘርዘር እና በጥብቅ የተተየቡ ስህተቶችን በመጠቀም `eyre`ን ከ `block.rs` ያስወግዱ
- hyperledger#3318 ኢንተርሊቭ የግብይት ሂደትን ለማስቀጠል በብሎኮች የተደረጉ ግብይቶችን ውድቅ አድርጎ ተቀብሏል።

### ተስተካክሏል።

- hyperledger#3075 በ `genesis.json` ውስጥ ልክ ባልሆነ ግብይት ላይ መሸበር ልክ ያልሆኑ ግብይቶች እንዳይካሄዱ ለመከላከል
- hyperledger # 3461 በነባሪ ውቅረት ውስጥ የነባሪ እሴቶችን በትክክል ማስተናገድ
- hyperledger#3548 `IntoSchema` ግልፅ ባህሪን ያስተካክሉ
- hyperledger # 3552 አረጋጋጭ መንገድ ንድፍ ውክልና ያስተካክሉ
- hyperledger # 3546 ተጣብቆ እንዲቆይ ጊዜ ቀስቅሴዎችን ያስተካክሉ
- hyperledger#3162 0 ቁመት ከልክል የዥረት ጥያቄዎች
- ውቅር ማክሮ የመጀመሪያ ሙከራ
- hyperledger#3592 የማዋቀር ፋይሎችን በ`release` ላይ ማሻሻያ
- hyperledger#3246 `Set B validators <https://github.com/hyperledger-iroha/iroha/blob/main/docs/source/iroha_2_whitepaper.md#2-system-architecture>`_ ያለ `fault <https://en.wikipedia.org/wiki/Byzantine_fault>`_ አያካትቱ
- hyperledger#3570 የደንበኛ-ጎን ሕብረቁምፊ መጠይቅ ስህተቶችን በትክክል አሳይ
- hyperledger#3596 I18NI0000362X ብሎኮች/ክስተቶችን ያሳያል
- hyperledger#3473 `kagami validator` ከአይሮሃ ማከማቻ ስርወ ማውጫ ውጭ እንዲሰራ አድርግ

### ሌላ

- hyperledger#3063 የካርታ ግብይት `hash` ቁመትን በ`wsv` ለማገድ
- በ`Value` ውስጥ በጥብቅ የተተየበው `HashOf<T>`

## [2.0.0-pre-rc.16]

### ታክሏል።

- hyperledger#2373 I18NI0000368X ንዑስ ትዕዛዝ `docker-compose.yml` ለማመንጨት
- hyperledger # 3525 የግብይት ኤ.ፒ.አይ
- hyperledger#3376 Iroha ደንበኛ CLI `pytest <https://docs.pytest.org/en/7.4.x/>`_ አውቶሜሽን ማዕቀፍ ያክሉ
- hyperledger#3516 ኦሪጅናል ብሎብ ሃሽን በ`LoadedExecutable` ያቆዩ

### ተስተካክሏል።

- hyperledger#3462 `burn` የንብረት ትዕዛዝ ወደ `client_cli` ያክሉ
- hyperledger # 3233 Refactor ስህተት አይነቶች
- hyperledger#3330 ተሃድሶን አስተካክል፣ `serde::de::Deserialize` ለ `partially-tagged <https://serde.rs/enum-representations.html>`_ `enums` በእጅ በመተግበር
- hyperledger# 3487 የጎደሉ ዓይነቶችን ወደ እቅዱ ይመልሱ
- hyperledger#3444 አድልዎ ወደ ሼማ ይመለሱ
- hyperledger # 3496 አስተካክል `SocketAddr` መስክ ትንተና
- hyperledger # 3498 ለስላሳ-ሹካ ማወቂያን ያስተካክሉ
- hyperledger#3396 ብሎክ የተደረገ ክስተት ከማውጣቱ በፊት በ`kura` ውስጥ የመደብር ብሎክ

### ሌላ

- hyperledger#2817 የውስጥ ለውጥን ከ`WorldStateView` ያስወግዱ
- hyperledger # 3363 ዘፍጥረት API refactor
- ነባር እና ለቶፖሎጂ አዳዲስ ሙከራዎችን ይጨምሩ
- ለሙከራ ሽፋን ከ`Codecov <https://about.codecov.io/>`_ ወደ `Coveralls <https://coveralls.io/>`_ ይቀይሩ
- hyperledger#3533 `Bool` ወደ `bool` እንደገና ይሰይሙ በእቅድ

## [2.0.0-ቅድመ-rc.15]

### ታክሏል።- hyperledger # 3231 ሞኖሊቲክ አረጋጋጭ
- hyperledger#3015 በኤፍኤፍአይ ውስጥ ለቦታ ማመቻቸት ድጋፍ
- hyperledger#2547 አርማ ወደ `AssetDefinition` ያክሉ
- hyperledger#3274 ወደ `kagami` ጨምር ምሳሌዎችን የሚያመነጭ ንዑስ ትዕዛዝ (ወደ LTS ተመልሷል)
- hyperledger # 3415 `Nix <https://nixos.wiki/wiki/Flakes>`_ flake
- hyperledger # 3412 የግብይት ወሬዎችን ወደ ተለየ ተዋናይ ያንቀሳቅሱ
- hyperledger # 3435 `Expression` ጎብኝን ያስተዋውቁ
- hyperledger#3168 የጄኔሲስ አረጋጋጭን እንደ የተለየ ፋይል ያቅርቡ
- hyperledger#3454 LTS ለአብዛኛዎቹ Docker ኦፕሬሽኖች እና ሰነዶች ነባሪ ያድርጉት
- hyperledger#3090 በሰንሰለት ላይ መለኪያዎችን ከብሎክቼይን ወደ I18NI00000388

### ተስተካክሏል።

- hyperledger#3330 መለያ ያልተደረገበት enum de-serialization በ`u128` ቅጠሎች ያስተካክሉ (ወደ RC14 የተመለሰ)
- hyperledger # 2581 የምዝግብ ማስታወሻዎች ውስጥ ጫጫታ ቀንሷል
- hyperledger # 3360 አስተካክል `tx/s` መለኪያ
- hyperledger#3393 በ `actors` ውስጥ የግንኙነት መዘጋትን ማቋረጥ
- hyperledger # 3402 አስተካክል `nightly` ግንባታ
- hyperledger # 3411 የእኩዮችን ግንኙነት በአንድ ጊዜ በትክክል ይያዙ
- hyperledger#3440 በዝውውር ወቅት የንብረት ልወጣዎችን ያስወግዱ፣ ይልቁንም በስማርት ኮንትራቶች የሚስተናገደው
- hyperledger # 3408: አስተካክል `public_keys_cannot_be_burned_to_nothing` ፈተና

### ሌላ

- hyperledger#3362 ወደ `tokio` ተዋናዮች ስደዱ
- hyperledger#3349 `EvaluateOnHost`ን ከስማርት ኮንትራቶች ያስወግዱ
- hyperledger#1786 ለሶኬት አድራሻዎች `iroha`-ቤተኛ አይነቶችን ያክሉ
- IVM መሸጎጫ አሰናክል
- IVM መሸጎጫውን እንደገና አንቃ
- የፈቃድ አረጋጋጭን ወደ አረጋጋጭ እንደገና ይሰይሙ
- hyperledger#3388 `model!` የሞጁል ደረጃ አይነታ ማክሮ አድርግ
- hyperledger#3370 `hash` እንደ ሄክሳዴሲማል ሕብረቁምፊ
- `maximum_transactions_in_block` ከ `queue` ወደ `sumeragi` ውቅር ውሰድ
- የ `AssetDefinitionEntry` አይነትን ያስወግዱ እና ያስወግዱ
- `configs/client_cli` ወደ `configs/client` እንደገና ይሰይሙ
- `MAINTAINERS.md` ያዘምኑ

## [2.0.0-ቅድመ-rc.14]

### ታክሏል።

- hyperledger#3127 የውሂብ ሞዴል `structs` ግልጽ ያልሆነ በነባሪ
- hyperledger#3122 የምግብ መፈጨት ተግባርን (የማህበረሰብ አስተዋፅዖ አበርካች) ለማከማቸት `Algorithm` ይጠቀሙ።
- hyperledger#3153 I18NI0000408X ውፅዓት ማሽን ሊነበብ የሚችል ነው።
- hyperledger#3105 `Transfer` ለ`AssetDefinition` መተግበር
- hyperledger#3010 I18NI0000411X የአገልግሎት ጊዜው ያለፈበት የቧንቧ መስመር ክስተት ታክሏል

### ተስተካክሏል።

- hyperledger#3113 ያልተረጋጋ የአውታረ መረብ ሙከራዎች ክለሳ
- hyperledger#3129 አስተካክል `Parameter` ደ/ተከታታይ
- hyperledger#3141 `IntoSchema` ለ`Hash` በእጅ ይተግብሩ
- hyperledger # 3155 የፍርሃት መንጠቆን በፈተናዎች ውስጥ ያስተካክሉ ፣ መዘጋትን ይከላከላል
- hyperledger#3166 ስራ ፈት ላይ ለውጥን አትመልከት፣ አፈፃፀሙን ያሻሽላል
- hyperledger#2123 ከብዙሃሽ ወደ PublicKey ተመለስ/ተከታታይ ማድረግ
- hyperledger # 3132 አዲስ ፓራሜትር አረጋጋጭ ያክሉ
- hyperledger#3249 ሃሾችን ማገድን ወደ ከፊል እና ሙሉ ስሪቶች ክፈል።
- hyperledger#3031 የጎደሉትን የውቅረት መለኪያዎች UI/UX ያስተካክሉ
- hyperledger#3247 የተወገደ የስህተት መርፌ ከ `sumeragi`።

### ሌላ

- የተሳሳቱ ውድቀቶችን ለማስተካከል የጎደለውን `#[cfg(debug_assertions)]` ይጨምሩ
- hyperledger#2133 ወደ ነጭ ወረቀት ለመቅረብ ቶፖሎጂን እንደገና ይፃፉ
- የ`iroha_client` ጥገኝነትን በ`iroha_core` ያስወግዱ
- hyperledger # 2943 Derive `HasOrigin`
- hyperledger#3232 የስራ ቦታ ሜታዳታ አጋራ
- hyperledger#3254 Refactor `commit_block()` እና `replace_top_block()`
- የተረጋጋ ነባሪ አመዳደብ ተቆጣጣሪን ይጠቀሙ
- hyperledger#3183 `docker-compose.yml` ፋይሎችን እንደገና ይሰይሙ
- የ `Multihash` የማሳያ ቅርጸት አሻሽሏል።
- hyperledger # 3268 በዓለም አቀፍ ደረጃ ልዩ የሆኑ የንጥል መለያዎች
- አዲስ PR አብነት

## [2.0.0-ቅድመ-rc.13]

### ታክሏል።- hyperledger#2399 መለኪያዎችን እንደ ISI ያዋቅሩ።
- hyperledger # 3119 `dropped_messages` ሜትሪክ ይጨምሩ።
- hyperledger#3094 ከ `n` እኩዮች ጋር አውታረ መረብ ይፍጠሩ።
- hyperledger#3082 በ `Created` ክስተት ውስጥ ሙሉ መረጃ ያቅርቡ።
- hyperledger#3021 ግልጽ ያልሆነ ጠቋሚ ማስመጣት።
- hyperledger#2794 በFFI ውስጥ ግልጽ የሆነ አድሎአዊ ድርጊት ያላቸውን የመስክ አልባ ዝርዝሮችን ውድቅ ያድርጉ።
- hyperledger#2922 `Grant<Role>` ወደ ነባሪ ዘፍጥረት ይጨምሩ።
- hyperledger # 2922 `inner` መስክ በ `NewRole` json deserialization ውስጥ መተው።
- hyperledger#2922 Omit `object(_id)` በ json deserialization ውስጥ።
- hyperledger#2922 Omit `Id` በ json deserialisation ውስጥ።
- hyperledger#2922 Omit I18NI0000432X በ json deserialization ውስጥ።
- hyperledger#2963 `queue_size` ወደ መለኪያዎች ያክሉ።
- hyperledger#3027 የመቆለፊያ ፋይል ለኩራ ይተግብሩ።
- hyperledger#2813 Kagami ነባሪ የአቻ ውቅር ያመነጫል።
- hyperledger#3019 ድጋፍ JSON5።
- hyperledger#2231 FFI መጠቅለያ ኤፒአይ ይፍጠሩ።
- hyperledger#2999 የማገጃ ፊርማዎችን ያከማቹ።
- hyperledger # 2995 ለስላሳ ሹካ ማወቂያ።
- hyperledger#2905 `NumericValue` ለመደገፍ የሂሳብ ስራዎችን ያራዝሙ
- hyperledger#2868 ኢሚት ኢሮሃ እትም እና ሎግ ውስጥ ሃሽ ያድርጉ።
- hyperledger#2096 ለጠቅላላ የንብረት መጠን መጠይቅ።
- hyperledger#2899 ባለብዙ መመሪያ ንዑስ ትዕዛዝን ወደ 'client_cli' ያክሉ
- hyperledger # 2247 የዌብሶኬት የመገናኛ ድምጽን ያስወግዱ.
- hyperledger#2889 የማገድ ድጋፍን ወደ `iroha_client` ያክሉ
- hyperledger#2280 ሚና ሲሰጥ/ሲሻር የፈቃድ ክንውኖችን አዘጋጅ።
- hyperledger # 2797 አበልጽጉ ክስተቶች.
- hyperledger#2725 የእረፍት ጊዜን ወደ `submit_transaction_blocking` እንደገና ማስተዋወቅ
- hyperledger # 2712 Config proptests.
- hyperledger # 2491 Enum ድጋፍ በ FF.
- hyperledger#2775 በሰው ሰራሽ ዘር ውስጥ የተለያዩ ቁልፎችን መፍጠር።
- hyperledger # 2627 ማዋቀር ማጠናቀቂያ ፣ የተኪ መግቢያ ነጥብ ፣ ካጋሚ ዶክገን።
- hyperledger#2765 ሰው ሰራሽ ዘረመል በ`kagami` ይፍጠሩ
- hyperledger#2698 ግልጽ ያልሆነ የስህተት መልእክት በ `iroha_client` ያስተካክሉ
- hyperledger#2689 የፍቃድ ማስመሰያ ፍቺ መለኪያዎችን ያክሉ።
- hyperledger#2502 ማከማቻ GIT hash of build.
- hyperledger#2672 `ipv4Addr`፣ `ipv6Addr` ተለዋጭ እና ተሳቢዎችን ያክሉ።
- hyperledger # 2626 `Combine` መውጣቱን መተግበር ፣ የተከፈለ `config` ማክሮዎች።
- hyperledger#2586 I18NI0000443X እና `LoadFromEnv` ለፕሮክሲ structs።
- hyperledger#2611 Derive `TryFromReprC` እና `IntoFfi` ለአጠቃላይ ግልጽ ያልሆኑ መዋቅሮች።
- hyperledger#2587 `Configurable` ወደ ሁለት ባህሪያት ይከፈል። # 2587: `Configurable` ወደ ሁለት ባህሪያት ተከፍሏል
- hyperledger#2488 በ `ffi_export` ውስጥ ለባህሪ ምልክቶች ድጋፍን ይጨምሩ
- hyperledger#2553 ወደ የንብረት መጠይቆች መደርደርን ይጨምሩ።
- hyperledger # 2407 Parametrise ቀስቅሴዎች.
- hyperledger#2536 `ffi_import` ለኤፍኤፍአይ ደንበኞች አስተዋውቋል።
- hyperledger # 2338 `cargo-all-features` መሣሪያ አክል.
- hyperledger#2564 Kagami መሣሪያ አልጎሪዝም አማራጮች።
- hyperledger#2490 ffi_exportን ነፃ ለሆኑ ተግባራት ይተግብሩ።
- hyperledger # 1891 ቀስቅሴን ማስፈጸምን ያረጋግጡ።
- hyperledger#1988 ማክሮዎችን ለመለየት ፣ Eq ፣ Hash ፣ Ord።
- hyperledger # 2434 FFI bindgen ላይብረሪ.
- hyperledger#2073 በብሎክቼይን ላሉት አይነቶች ConstStringን በ String ላይ እመርጣለሁ።
- hyperledger#1889 በጎራ የተቀመጡ ቀስቅሴዎችን ይጨምሩ።
- hyperledger#2098 የራስጌ መጠይቆችን አግድ። # 2098: የአግድ ራስጌ መጠይቆችን ያክሉ
- hyperledger#2467 የመለያ ስጦታ ንዑስ ትዕዛዝን በiroha_client_cli ውስጥ ይጨምሩ።
- hyperledger#2301 ሲጠይቁት የግብይት ብሎክ ሃሽ ይጨምሩ።
 - hyperledger#2454 የግንባታ ስክሪፕት ወደ I18NT0000026X ዲኮደር መሣሪያ።
- hyperledger # 2061 ለማጣሪያዎች ማክሮ።- hyperledger#2228 ያልተፈቀደ ልዩነት ወደ ስማርት ኮንትራቶች መጠይቅ ስህተት ያክሉ።
- hyperledger#2395 ዘፍጥረት መተግበር ካልተቻለ ድንጋጤ ይጨምሩ።
- hyperledger#2000 ባዶ ስሞችን አትፍቀድ። # 2000: ባዶ ስሞችን አትፍቀድ
 - hyperledger#2127 በ Norito ኮዴክ ዲኮድ የተደረገው ሁሉም ዳታ መበላቱን ለማረጋገጥ የጤንነት ማረጋገጫን ይጨምሩ።
- hyperledger # 2360 እንደገና `genesis.json` አማራጭ አድርግ.
- hyperledger#2053 ለቀሩት ጥያቄዎች በግል blockchain ውስጥ ፈተናዎችን ያክሉ።
- hyperledger # 2381 `Role` ምዝገባን አንድ አድርግ።
- hyperledger#2053 ሙከራዎችን ከንብረት ጋር በተያያዙ ጥያቄዎች ላይ በግል blockchain ውስጥ ይጨምሩ።
- hyperledger#2053 ሙከራዎችን ወደ 'private_blockchain' ያክሉ
- hyperledger#2302 'FindTriggersByDomainId' stub-query ያክሉ።
- hyperledger#1998 ማጣሪያዎችን ወደ መጠይቆች ያክሉ።
- hyperledger#2276 የአሁኑን የብሎክ ሃሽ ወደ BlockHeaderValue ያካትቱ።
- hyperledger#2161 የመያዣ መታወቂያ እና የተጋራ FFI fns።
- የመያዣ መታወቂያ ያክሉ እና የFFI አቻ የጋራ ባህሪዎችን ይተግብሩ (Clone ፣ Eq ፣ Ord)
- hyperledger#1638 I18NI0000454X መመለሻ ሰነድ ንዑስ-ዛፍ።
- hyperledger # 2132 `endpointN` proc ማክሮ ይጨምሩ።
- hyperledger#2257 መሻር<Role> ሮል የተሻረ ክስተትን ያወጣል።
- hyperledger#2125 FindAssetDefinitionById መጠይቅን ያክሉ።
- hyperledger#1926 የምልክት አያያዝ እና ግርማ ሞገስ ያለው መዘጋት ይጨምሩ።
- hyperledger#2161 FFI ተግባራትን ለ`data_model` ያመነጫል።
- hyperledger#1149 የማገጃ ፋይል ብዛት በአንድ ማውጫ ከ1000000 አይበልጥም።
- hyperledger # 1413 የኤፒአይ ስሪት መጨረሻ ነጥብ ያክሉ።
- hyperledger#2103 ለብሎኮች እና ግብይቶች መጠይቅን ይደግፋል። የ`FindAllTransactions` መጠይቅ ያክሉ
- hyperledger#2186 ለ`BigQuantity` እና `Fixed` ማስተላለፍ ISI ይጨምሩ።
- hyperledger#2056 ለ `AssetValueType` `enum` አንድ deriv proc ማክሮ crate ያክሉ.
- hyperledger#2100 ሁሉንም ሂሳቦች በንብረት ለማግኘት መጠይቅ ይጨምሩ።
- hyperledger # 2179 ቀስቅሴ አፈፃፀምን ያመቻቹ።
- hyperledger#1883 የተከተቱ የውቅር ፋይሎችን ያስወግዱ።
- hyperledger#2105 በደንበኛው ውስጥ የጥያቄ ስህተቶችን ያስተናግዳል።
- hyperledger#2050 ከ ሚና ጋር የተያያዙ ጥያቄዎችን ያክሉ።
- hyperledger # 1572 ልዩ የፍቃድ ማስመሰያዎች።
- hyperledger#2121 ኪይፓይር ሲገነባ የሚሰራ መሆኑን ያረጋግጡ።
 - hyperledger#2003 Norito ዲኮደር መሣሪያን አስተዋውቅ።
- hyperledger#1952 የ TPS ቤንችማርክን ለማመቻቸት እንደ መስፈርት ያክሉ።
- hyperledger#2040 የውህደት ሙከራን ከግብይት አፈፃፀም ገደብ ጋር ይጨምሩ።
- hyperledger#1890 በኦሪሊዮን አጠቃቀም-ጉዳዮች ላይ በመመስረት የውህደት ሙከራዎችን ያስተዋውቁ።
- hyperledger#2048 የመሳሪያ ሰንሰለት ፋይል ያክሉ።
- hyperledger#2100 ሁሉንም ሂሳቦች በንብረት ለማግኘት መጠይቅ ይጨምሩ።
- hyperledger # 2179 ቀስቅሴ አፈፃፀምን ያመቻቹ።
- hyperledger#1883 የተከተቱ የውቅር ፋይሎችን ያስወግዱ።
- hyperledger#2004 `isize` እና `usize` `IntoSchema` እንዳይሆኑ ይከልክሉ።
- hyperledger#2105 በደንበኛው ውስጥ የጥያቄ ስህተቶችን ያስተናግዳል።
- hyperledger#2050 ከ ሚና ጋር የተያያዙ ጥያቄዎችን ያክሉ።
- hyperledger # 1572 ልዩ የፍቃድ ማስመሰያዎች።
- hyperledger#2121 ኪይፓይር ሲገነባ የሚሰራ መሆኑን ያረጋግጡ።
 - hyperledger#2003 Norito ዲኮደር መሣሪያን አስተዋውቅ።
- hyperledger#1952 የ TPS ቤንችማርክን ለማመቻቸት እንደ መስፈርት ያክሉ።
- hyperledger#2040 የውህደት ሙከራን ከግብይት አፈፃፀም ገደብ ጋር ይጨምሩ።
- hyperledger#1890 በኦሪሊዮን አጠቃቀም-ጉዳዮች ላይ በመመርኮዝ የውህደት ሙከራዎችን ያስተዋውቁ።
- hyperledger#2048 የመሳሪያ ሰንሰለት ፋይል ያክሉ።
- hyperledger # 2037 ቅድመ-ተግባር ቀስቅሴዎችን አስተዋውቅ።
- hyperledger#1621 በጥሪ ቀስቅሴዎች ያስተዋውቁ።
- hyperledger#1970 አማራጭ schema መጨረሻ ነጥብ ያክሉ.
- hyperledger#1620 በጊዜ ላይ የተመሰረቱ ቀስቅሴዎችን ያስተዋውቁ።
- hyperledger#1918 ለ`client` መሰረታዊ ማረጋገጫን ይተግብሩ
- hyperledger#1726 የመልቀቂያ PR የስራ ፍሰትን ይተግብሩ።
- hyperledger#1815 የጥያቄ ምላሾችን በአይነት የተዋቀሩ ያድርጉ።- hyperledger#1928 `gitchangelog` በመጠቀም የለውጥ ሎግ ማመንጨትን ይተግብሩ
- hyperledger # 1902 ባዶ ብረት 4-አቻ ማዋቀር ስክሪፕት.

  ዶከር ማቀናበር የማይፈልግ እና የIroha ማረም የሚጠቀም የ setup_test_env.sh ስሪት ታክሏል።
- hyperledger#1619 ክስተት ላይ የተመሰረቱ ቀስቅሴዎችን አስተዋውቅ።
- hyperledger#1195 የዌብሶኬት ግንኙነትን በንጽህና ዝጋ።
- hyperledger#1606 የጎራ መዋቅር ውስጥ ipfs አገናኝ ወደ ጎራ አርማ ያክሉ።
- hyperledger # 1754 የኩራ ኢንስፔክተር CLI ን ይጨምሩ።
- hyperledger#1790 ቁልል ላይ የተመሰረቱ ቬክተሮችን በመጠቀም አፈፃፀሙን ያሻሽሉ።
- hyperledger#1805 ለድንጋጤ ስህተቶች አማራጭ ተርሚናል ቀለሞች።
- hyperledger#1749 I18NI0000467X በ `data_model`
- hyperledger#1179 የመሻር-ፈቃድ-ወይም-ሚና መመሪያን ይጨምሩ።
- hyperledger#1782 ከአይሮሀ_ክሪቶ ኖ_ኤስትዲ ጋር የሚስማማ ያደርገዋል።
- hyperledger#1172 የመመሪያ ዝግጅቶችን ተግባራዊ ያድርጉ።
- hyperledger#1734 ነጭ ቦታዎችን ለማስቀረት `Name`ን ያረጋግጡ።
- hyperledger#1144 ሜታዳታ መክተቻ ጨምር።
- # 1210 አግድ ዥረት (የአገልጋይ ጎን)።
- hyperledger#1331 ተጨማሪ `Prometheus` መለኪያዎችን ይተግብሩ።
- hyperledger # 1689 የባህሪ ጥገኛዎችን ያስተካክሉ። # 1261: የጭነት እብጠትን ይጨምሩ.
- hyperledger # 1675 ለታሸጉ ዕቃዎች ከመጠቅለያ ይልቅ ዓይነት ይጠቀሙ።
- hyperledger#1643 እኩዮች በፈተና ውስጥ ዘፍጥረትን እስኪፈጽሙ ድረስ ይጠብቁ።
- hyperledger # 1678 `try_allocate`
- hyperledger # 1216 Prometheus የመጨረሻ ነጥብ ይጨምሩ። #1216፡ የመለኪያዎች የመጨረሻ ነጥብ የመጀመሪያ ትግበራ።
- hyperledger#1238 የሩጫ ጊዜ ምዝግብ ማስታወሻ ደረጃ ማሻሻያ። መሰረታዊ `connection` የመግቢያ ነጥብ ላይ የተመሰረተ ዳግም መጫን ተፈጠረ።
- hyperledger#1652 PR ርዕስ ቅርጸት።
- የተገናኙትን እኩዮች ቁጥር ወደ `Status` ይጨምሩ

  - "ከተገናኙት እኩዮች ብዛት ጋር የተያያዙ ነገሮችን ሰርዝ" አድህር

  ይህ b228b41dab3c035ce9973b6aa3b35d443c082544ን ያድህራል።
  - Clarify I18NI0000474X እውነተኛ የህዝብ ቁልፍ ያለው ከእጅ መጨባበጥ በኋላ ነው።
  - `DisconnectPeer` ያለ ሙከራዎች
  - የእኩዮችን አፈፃፀም ያለማስመዝገብ ይተግብሩ
  - የአቻ ንዑስ ትዕዛዝን ወደ `client_cli` አክል
  - ካልተመዘገበ እኩያ በአድራሻው እንደገና መገናኘትን እምቢ ማለት

  እኩዮችዎ ከሌላ እኩያ ተመዝግበው ካቋረጡ በኋላ፣
  አውታረ መረብዎ ከእኩያ የሚቀርቡትን የመልሶ ማገናኘት ጥያቄዎችን ይሰማል።
  መጀመሪያ ላይ ማወቅ የሚችሉት የወደብ ቁጥሩ የዘፈቀደ የሆነ አድራሻ ነው።
  ስለዚህ ያልተመዘገበውን አቻ ከወደብ ቁጥር ሌላ ክፍል አስታውሱ
  እና ከዚያ እንደገና መገናኘትን እምቢ ይበሉ
- `/status` የመጨረሻ ነጥብ ወደ አንድ የተወሰነ ወደብ ያክሉ።

### ማስተካከያዎች- hyperledger # 3129 አስተካክል `Parameter` ደ / ተከታታይ.
- hyperledger#3109 `sumeragi` ከእንቅልፍ አግኖስቲክ መልእክት በኋላ መከላከል።
- hyperledger#3046 Iroha በሚያምር ሁኔታ በባዶ መጀመሩን ያረጋግጡ
  `./storage`
- hyperledger#2599 የችግኝ መጫዎቻዎችን ያስወግዱ።
- hyperledger#3087 ከእይታ ለውጥ በኋላ ድምጾችን ከ Set B validators ይሰብስቡ።
- hyperledger # 3056 አስተካክል `tps-dev` ቤንችማርክ ማንጠልጠያ.
- hyperledger#1170 ክሎኒንግ-wsv-style ለስላሳ-ፎርክ አያያዝን ተግባራዊ ያድርጉ።
- hyperledger#2456 ዘፍጥረት ብሎክ ያልተገደበ አድርግ።
- hyperledger # 3038 መልቲሲግስን እንደገና አንቃ።
- hyperledger#2894 አስተካክል `LOG_FILE_PATH` env ተለዋዋጭ deserialization.
- hyperledger#2803 ለፊርማ ስህተቶች ትክክለኛውን የሁኔታ ኮድ ይመልሱ።
- hyperledger#2963 I18NI0000483X ግብይቶችን በትክክል ያስወግዳል።
- hyperledger#0000 Vergen ሰበር CI.
- hyperledger # 2165 የመሳሪያ ሰንሰለትን አስወግድ.
- hyperledger#2506 የማገጃ ማረጋገጫውን ያስተካክሉ።
- hyperledger # 3013 በትክክል ሰንሰለት ማቃጠል validators.
- hyperledger#2998 ጥቅም ላይ ያልዋለ የሰንሰለት ኮድ ሰርዝ።
- hyperledger#2816 ወደ ኩራ ብሎኮች የመድረስ ሃላፊነትን ያንቀሳቅሱ።
- hyperledger#2384 ዲኮድ በዲኮድ_ሁሉንም ይተኩ።
- hyperledger#1967 የእሴት ስም በስም ተካ።
- hyperledger#2980 የማገጃ እሴት ffi ዓይነትን ያስተካክሉ።
- hyperledger # 2858 የመኪና ማቆሚያ ቦታን ያስተዋውቁ :: ከ std ይልቅ Mutex.
- hyperledger#2850 የ `Fixed` ዲሴሪያላይዜሽን/መግለጥን ያስተካክሉ
- hyperledger#2923 `FindError` መመለስ `AssetDefinition`
  አለ ።
- hyperledger # 0000 አስተካክል `panic_on_invalid_genesis.sh`
- hyperledger#2880 የዌብሶኬት ግንኙነትን በትክክል ዝጋ።
- hyperledger # 2880 የማገጃ ዥረት ያስተካክሉ።
- hyperledger#2804 I18NI0000488X የግብይት እገዳን አስገባ።
- hyperledger # 2819 አስፈላጊ ያልሆኑ አባላትን ከ WSV ማውጣት።
- አገላለጽ ተከታታይነት ያለው ድግግሞሽ ስህተትን ያስተካክሉ።
- hyperledger # 2834 የአጭር ጊዜ አገባብ አሻሽል።
- hyperledger#2379 አዳዲስ የኩራ ብሎኮችን ወደ blocks.txt የመጣል ችሎታን ይጨምሩ።
- hyperledger#2758 የመደርደር መዋቅርን ወደ እቅዱ ያክሉ።
- ሲ.አይ.
- hyperledger#2548 በትልቅ የዘፍጥረት ፋይል ላይ አስጠንቅቅ።
- hyperledger#2638 `whitepaper` አዘምን እና ለውጦችን ያሰራጫል።
- hyperledger # 2678 በመድረክ ቅርንጫፍ ላይ ሙከራዎችን ያስተካክሉ።
- hyperledger#2678 የኩራ ሃይል መዘጋት ላይ ሙከራዎችን አስተካክል።
- hyperledger#2607 የ sumeragi ኮድ ለበለጠ ቀላልነት Refactor እና
  የጥንካሬ ጥገናዎች.
- hyperledger#2561 የአመለካከት ለውጦችን ወደ መግባባት ማስተዋወቅ።
- hyperledger#2560 በብሎክ_ስንክርክ እና አቻ ማቋረጥ ላይ መልሰው ይጨምሩ።
- hyperledger#2559 የሱመራጊ ክር መዝጋትን ይጨምሩ።
- hyperledger#2558 wsv ከ kura ከማዘመንዎ በፊት ዘፍጥረትን ያረጋግጡ።
- hyperledger#2465 sumeragi መስቀለኛ መንገድ እንደ ነጠላ ክር ሁኔታ እንደገና ይድገሙት
  ማሽን.
- hyperledger#2449 የ Sumeragi መልሶ ማዋቀር የመጀመሪያ ትግበራ።
- hyperledger # 2802 አስተካክል env መጫን ለማዋቀር.
- hyperledger#2787 ሁሉም አድማጭ በፍርሃት እንዲዘጋ ያሳውቁ።
- hyperledger#2764 ከፍተኛ የመልእክት መጠን ላይ ያለውን ገደብ አስወግድ።
- # 2571: የተሻለ የኩራ ኢንስፔክተር UX.
- hyperledger # 2703 አስተካክል Orillion dev env ስህተቶች.
- በሰነድ አስተያየት በ schema/src ውስጥ የትየባ ያስተካክሉ።
- hyperledger#2716 የሚቆይበትን ጊዜ በአፕሊኬሽን ይፋዊ ያድርጉ።
- hyperledger#2700 `KURA_BLOCK_STORE_PATH` ወደ ውጭ ላክ በዶከር ምስሎች።
- hyperledger#0 `/iroha/rust-toolchain.toml` ከግንበኛ ያስወግዱ
  ምስል.
- hyperledger # 0 አስተካክል `docker-compose-single.yml`
- hyperledger#2554 ስህተትን ያሳድጉ `secp256k1` ዘር ከ32 ያነሰ ከሆነ
  ባይት.
- hyperledger#0 አሻሽል `test_env.sh` ለእያንዳንዱ አቻ ማከማቻ ለመመደብ።
- hyperledger#2457 በፈተናዎች ኩራን በግዳጅ ዘጋው።
- hyperledger # 2623 Fix doctest for VariantCount.
- በ ui_fail ሙከራዎች ውስጥ የሚጠበቀውን ስህተት ያዘምኑ።
- በፈቃድ አረጋጋጮች ውስጥ የተሳሳተ የሰነድ አስተያየት ያስተካክሉ።- hyperledger # 2422 በማዋቀር የመጨረሻ ነጥብ ምላሽ ውስጥ የግል ቁልፎችን ደብቅ።
- hyperledger # 2492: ሁሉም የሚፈጸሙ ቀስቅሴዎች ከክስተት ጋር የሚዛመዱ አይደሉም።
- hyperledger#2504 ያልተሳካ tps ቤንችማርክን ያስተካክሉ።
- hyperledger#2477 የሚናዎች ፈቃዶች በማይቆጠሩበት ጊዜ ስህተትን ያስተካክሉ።
- hyperledger # 2416 ሊንቶችን በማክኦኤስ ክንድ ላይ ያስተካክሉ።
- hyperledger#2457 በድንጋጤ ላይ ከመዝጋት ጋር የተዛመደ የፍተሻ ሙከራዎችን ያስተካክሉ።
  # 2457: በድንጋጤ ውቅረት ላይ ዝጋን ይጨምሩ
- hyperledger#2473 parse rustc - ስሪት ከRUSTUP_TOOLCHAIN ይልቅ።
- hyperledger#1480 በድንጋጤ ዝጋ። #1480፡ በድንጋጤ ላይ ለመውጣት የሽብር መንጠቆን ይጨምሩ
- hyperledger # 2376 ቀለል ያለ ኩራ ፣ አልተመሳሰልም ፣ ሁለት ፋይሎች።
- hyperledger#0000 Docker የግንባታ ውድቀት።
- hyperledger#1649 `spawn`ን ከ`do_send` ያስወግዱ
- hyperledger # 2128 አስተካክል `MerkleTree` ግንባታ እና ድግግሞሽ.
- hyperledger#2137 ለብዙ ሂደት አውድ ፈተናዎችን ያዘጋጁ።
- hyperledger#2227 ይመዝገቡ እና ለንብረት ይውጡ።
- hyperledger#2081 ሚና የሚሰጥ ስህተትን ያስተካክሉ።
- hyperledger#2358 መለቀቅን ከማረም መገለጫ ጋር ይጨምሩ።
- hyperledger#2294 የፍላሜግራፍ ትውልድን ወደ onehot.rs ያክሉ።
- hyperledger#2202 አጠቃላይ መስክ በጥያቄ ምላሽ ያስተካክሉ።
- hyperledger#2081 ሚናውን ለመስጠት የፈተናውን ጉዳይ አስተካክል።
- hyperledger # 2017 ሚና አለመመዝገብን ያስተካክሉ።
- hyperledger#2303 አስተካክል docker-compose እኩዮችን በጸጋ አይዘጋም።
- hyperledger#2295 ያልተመዘገበ ቀስቃሽ ስህተትን ያስተካክሉ።
- hyperledger#2282 FFI የሚያሻሽለው ከጌሴት ትግበራ ነው።
- hyperledger#1149 የ nocheckin ኮድን ያስወግዱ።
- hyperledger#2232 ዘፍጥረት በጣም ብዙ isi ሲኖረው Iroha ትርጉም ያለው መልእክት እንዲታተም ያድርጉ።
- hyperledger#2170 ግንባታን በዶክተር ኮንቴይነር M1 ማሽኖች ላይ አስተካክል።
- hyperledger#2215 በምሽት-2022-04-20 አማራጭ አድርግ `cargo build`
- hyperledger#1990 config.json በማይኖርበት ጊዜ የአቻ ጅምርን በ env vars ያንቁ።
- hyperledger # 2081 ሚና ምዝገባን ያስተካክሉ።
- hyperledger # 1640 ማመንጨት config.json እና genesis.json.
- hyperledger#1716 የጋራ ስምምነት ውድቀትን በf=0 ጉዳዮች ያስተካክሉ።
- hyperledger#1845 የማይታሰብ ንብረቶችን አንድ ጊዜ ብቻ ማውጣት ይቻላል።
- hyperledger#2005 `Client::listen_for_events()` የዌብሶኬት ዥረት የማይዘጋውን አስተካክል።
- hyperledger#1623 RawGenesisBlockBuilder ይፍጠሩ።
- hyperledger#1917 ቀላል_from_str_impl ማክሮ ያክሉ።
- hyperledger#1990 config.json በማይኖርበት ጊዜ የአቻ ጅምርን በ env vars ያንቁ።
- hyperledger # 2081 ሚና ምዝገባን ያስተካክሉ።
- hyperledger # 1640 ማመንጨት config.json እና genesis.json.
- hyperledger#1716 የጋራ ስምምነት ውድቀትን በf=0 ጉዳዮች ያስተካክሉ።
- hyperledger#1845 የማይታሰብ ንብረቶችን አንድ ጊዜ ብቻ ማውጣት ይቻላል።
- hyperledger#2005 `Client::listen_for_events()` የዌብሶኬት ዥረት የማይዘጋውን አስተካክል።
- hyperledger#1623 RawGenesisBlockBuilder ይፍጠሩ።
- hyperledger#1917 ቀላል_from_str_impl ማክሮ ያክሉ።
- hyperledger#1922 crypto_cliን ወደ መሳሪያዎች ይውሰዱ።
- hyperledger#1969 የ `roles` ባህሪ የነባሪ ባህሪ ስብስብ አካል ያድርጉት።
- hyperledger # 2013 Hotfix CLI args.
- hyperledger#1897 አጠቃቀምን/መጠንን ከተከታታይነት ያስወግዱ።
- hyperledger#1955 `:` በ`web_login` ውስጥ ማለፍ የሚቻልበትን ሁኔታ ያስተካክሉ
- hyperledger#1943 የጥያቄ ስህተቶችን ወደ እቅዱ ያክሉ።
- hyperledger#1939 ትክክለኛ ባህሪያት ለ `iroha_config_derive`.
- hyperledger#1908 ለቴሌሜትሪ ትንተና ስክሪፕት የዜሮ እሴት አያያዝን ያስተካክሉ።
- hyperledger#0000 በተዘዋዋሪ ችላ የተባለውን የዶክመንተ-ሙከራ በግልፅ ችላ እንዲሉ ያድርጉ።
- hyperledger#1848 የህዝብ ቁልፎችን ወደ ምናምን እንዳይቃጠል መከላከል።
- hyperledger#1811 የታመኑ የአቻ ቁልፎችን ለማጥፋት ሙከራዎችን እና ቼኮችን ጨምሯል።
- hyperledger#1821 IntoSchema ለ MerkleTree እና VersionedValidBlock ያክሉ፣ HashOf እና SignatureOf schemas ያስተካክሉ።- hyperledger#1819 በማረጋገጫው ላይ ከስህተት ዘገባ ዱካውን ያስወግዱ።
- hyperledger#1774 የማረጋገጫ ውድቀቶች ትክክለኛ ምክንያት።
- hyperledger#1714 PeerIdን በቁልፍ ብቻ ያወዳድሩ።
- hyperledger#1788 የ`Value` የማስታወሻ አሻራን ይቀንሱ።
- hyperledger#1804 አስተካክል schema ትውልድ HashOf, SignatureOf, ምንም schema መቅረት ለማረጋገጥ ሙከራ ያክሉ.
- hyperledger#1802 የመግቢያ ተነባቢነት ማሻሻያዎች።
  - የክስተቶች ምዝግብ ማስታወሻ ወደ መከታተያ ደረጃ ተንቀሳቅሷል
  - ctx ከምዝግብ ማስታወሻ ተወግዷል
  - የተርሚናል ቀለሞች እንደ አማራጭ ተደርገዋል (ለተሻለ የፋይል ውፅዓት)
- hyperledger#1783 ቋሚ የቶሪ ቤንችማርክ።
- hyperledger # 1772 ከ # 1764 በኋላ አስተካክል.
- hyperledger # 1755 ጥቃቅን ጥገናዎች ለ # 1743, # 1725.
  - JSONsን በ#1743 `Domain` መዋቅር ለውጥ ያስተካክሉ
- hyperledger # 1751 የጋራ ስምምነት ተስተካክሏል. #1715፡ ከፍተኛ ጭነትን ለማስተናገድ የጋራ መግባባት ተስተካክሏል (#1746)
  - የለውጥ አያያዝ ጥገናዎችን ይመልከቱ
  - ከተወሰኑ የግብይት ሃሽ ነፃ የተደረጉ የለውጥ ማረጋገጫዎችን ይመልከቱ
  - የተቀነሰ መልእክት ማስተላለፍ
  - ወዲያውኑ መልዕክቶችን ከመላክ ይልቅ የእይታ ለውጥ ድምጾችን ይሰብስቡ (የአውታረ መረብ መቋቋምን ያሻሽላል)
  - በSumeragi ውስጥ የተዋናይ ማዕቀፍን ሙሉ በሙሉ ተጠቀም (በተግባር ምትክ መልእክቶችን ለራስ ያዝ)
  - በSumeragi ለሙከራዎች የስህተት መርፌን ያሻሽላል
  - የሙከራ ኮድን ወደ ምርት ኮድ ያቀርባል
  - ከመጠን በላይ የተወሳሰቡ መጠቅለያዎችን ያስወግዳል
  - በሙከራ ኮድ ውስጥ Sumeragi የተዋናይ አውድ እንዲጠቀም ይፈቅዳል
- hyperledger#1734 ከአዲሱ የጎራ ማረጋገጫ ጋር እንዲመጣጠን ዘፍጥረትን ያዘምኑ።
- hyperledger#1742 የኮንክሪት ስህተቶች በ `core` መመሪያዎች ውስጥ ተመልሰዋል።
- hyperledger # 1404 አረጋግጥ ቋሚ.
- hyperledger#1636 `trusted_peers.json` እና `structopt` አስወግድ
  # 1636: `trusted_peers.json` አስወግድ.
- hyperledger#1706 `max_faults` ከቶፖሎጂ ዝመና ጋር ያዘምኑ።
- hyperledger#1698 ቋሚ የህዝብ ቁልፎች፣ ሰነዶች እና የስህተት መልዕክቶች።
የማዕድን ጉዳዮች (1593 እና 1405) ቁጥር 1405

### አነቃቂ- ተግባራትን ከ sumeragi main loop ያውጡ።
- Refactor I18NI0000512X ወደ newtype.
- `Mutex`ን ከ `Metrics` ያስወግዱ
- adt_const_generics የምሽት ባህሪን ያስወግዱ።
- hyperledger#3039 የመልቲሲግ መጠበቂያ ቋት ያስተዋውቁ።
- ሱመራጊን ቀለል ያድርጉት።
- hyperledger # 3053 ቅንጥብ lints ያስተካክሉ።
- hyperledger#2506 በብሎክ ማረጋገጫ ላይ ተጨማሪ ሙከራዎችን ይጨምሩ።
- `BlockStoreTrait` ን በኩራ አስወግድ።
- ለ `nightly-2022-12-22` lints ያዘምኑ
- hyperledger#3022 `Option` በ `transaction_cache` አስወግድ
- hyperledger#3008 የኒሽ እሴትን ወደ `Hash` ይጨምሩ
- ሊንቶችን ወደ 1.65 ያዘምኑ።
- ሽፋንን ለመጨመር ትናንሽ ሙከራዎችን ያክሉ።
- የሞተ ኮድን ከ `FaultInjection` ያስወግዱ
- ከ sumeragi ብዙ ጊዜ ወደ p2p ይደውሉ።
- hyperledger#2675 ቬክ ሳይመድቡ የንጥል ስሞችን/መታወቂያዎችን ያረጋግጡ።
- hyperledger#2974 ያለ ሙሉ ማሻሻያ ማገድን መከላከል።
- የበለጠ ቀልጣፋ I18NI0000521X በማጣመር።
- hyperledger#2955 ብሎክን ከተከለከለው መልእክት ያስወግዱ።
- hyperledger#1868 የተረጋገጡ ግብይቶችን ከመላኩ ይከለክላል
  በእኩዮች መካከል.
- hyperledger # 2458 አጠቃላይ አጣማሪ ኤፒአይን ይተግብሩ።
- የማከማቻ አቃፊን ወደ gitignore ያክሉ።
- hyperledger # 2909 ሃርድ ኮድ ወደቦች ለሚቀጥለው።
- hyperledger # 2747 ለውጥ I18NI0000522X API.
- በማዋቀር ውድቀት ላይ የስህተት መልዕክቶችን ያሻሽሉ።
- ተጨማሪ ምሳሌዎችን ወደ `genesis.json` ያክሉ
- `rc9` ከመለቀቁ በፊት ጥቅም ላይ ያልዋሉ ጥገኞችን ያስወግዱ።
- በአዲሱ Sumeragi ላይ መጨረስ።
- በዋናው ዑደት ውስጥ ንዑስ ሂደቶችን ያውጡ።
- hyperledger#2774 `kagami` የዘፍጥረት ማመንጨት ሁነታን ከባንዲራ ወደ ቀይር
  ንዑስ ትዕዛዝ.
- hyperledger # 2478 `SignedTransaction` ያክሉ
- hyperledger#2649 `byteorder` ክሬትን ከ `Kura` ያስወግዱ
- `DEFAULT_BLOCK_STORE_PATH` ከ `./blocks` ወደ `./storage` እንደገና ይሰይሙ
- hyperledger#2650 የኢሮሃ ንዑስ ሞጁሎችን ለመዝጋት `ThreadHandler` ይጨምሩ።
- hyperledger#2482 የማከማቻ `Account` የፍቃድ ማስመሰያዎች በ`Wsv`
- አዲስ ሽፋኖችን ወደ 1.62 ያክሉ።
- `p2p` የስህተት መልዕክቶችን አሻሽል።
- hyperledger#2001 `EvaluatesTo` የማይንቀሳቀስ አይነት መፈተሽ።
- hyperledger#2052 የፈቃድ ቶከኖች ከትርጉም ጋር እንዲመዘገቡ ያድርጉ።
  #2052፡ የፍቃድ ቶከን ትርጉምን ተግብር
- ሁሉም የባህሪ ጥምረት መስራቱን ያረጋግጡ።
- hyperledger#2468 ከፈቃድ አረጋጋጮች የማረሚያ ልዕለ ባህሪን ያስወግዱ።
- hyperledger#2419 ግልጽ `drop`s ያስወግዱ።
- hyperledger#2253 `Registrable` ባህሪ ወደ `data_model` ያክሉ
- ለዳታ ክስተቶቹ ከ `Identifiable` ይልቅ `Origin`ን ይተግብሩ።
- hyperledger # 2369 Refactor ፈቃድ አረጋጋጮች.
- hyperledger#2307 `events_sender` በ `WorldStateView` አማራጭ ያልሆነ አድርግ።
- hyperledger#1985 የ`Name` መዋቅር መጠን ይቀንሱ።
- ተጨማሪ `const fn` ይጨምሩ።
- የውህደት ሙከራዎችን `default_permissions()` ይጠቀሙ
- የፍቃድ ማስመሰያ መጠቅለያዎችን በግል_blockchain ውስጥ ይጨምሩ።
- hyperledger#2292 `WorldTrait` ን ያስወግዱ፣ አጠቃላይ መረጃዎችን ከ`IsAllowedBoxed` ያስወግዱ።
- hyperledger#2204 ከንብረት ጋር የተያያዙ ስራዎችን አጠቃላይ ያድርጉ።
- hyperledger#2233 `impl` በ `derive` ለ `Display` እና `Debug` ተካ።
- ሊታወቅ የሚችል መዋቅር ማሻሻያ.
- hyperledger#2323 የ kura init የስህተት መልእክት ያሻሽሉ።
- hyperledger#2238 ለፈተናዎች አቻ ገንቢ ይጨምሩ።
- hyperledger # 2011 ተጨማሪ ገላጭ ውቅረት ፓራሞች።
- hyperledger # 1896 ቀላል `produce_event` ትግበራ.
- Refactor `QueryError` ዙሪያ.
- `TriggerSet` ወደ I18NI0000556X ውሰድ።
- hyperledger#2145 refactor client's `WebSocket` ጎን፣ ንጹህ ዳታ አመክንዮ ማውጣት።
- የ `ValueMarker` ባህሪን ያስወግዱ።
- hyperledger#2149 `Mintable` እና `MintabilityError` በ `prelude` ያጋልጡ
- hyperledger#2144 የደንበኛውን http የስራ ፍሰት እንደገና ይንደፋ፣ የውስጥ ኤፒአይን ያጋልጣል።- ወደ `clap` ውሰድ።
- `iroha_gen` ሁለትዮሽ ፣ የሚያጠናክሩ ሰነዶች ፣ schema_bin ይፍጠሩ።
- hyperledger # 2109 የ `integration::events::pipeline` ሙከራ የተረጋጋ ያድርጉት።
- hyperledger#1982 የ `iroha_crypto` መዋቅሮች መዳረሻን ያጠቃልላል።
- `AssetDefinition` ገንቢ ያክሉ።
- አላስፈላጊውን I18NI0000567X ከኤፒአይ ያስወግዱ።
- የውሂብ ሞዴል መዋቅሮች መዳረሻን ያጠቃልላል.
- hyperledger#2144 የደንበኛውን http የስራ ፍሰት እንደገና ይንደፋ፣ የውስጥ ኤፒአይን ያጋልጣል።
- ወደ `clap` ውሰድ።
- `iroha_gen` ሁለትዮሽ ፣ የሚያጠናክሩ ሰነዶች ፣ schema_bin ይፍጠሩ።
- hyperledger # 2109 የ `integration::events::pipeline` ሙከራ የተረጋጋ ያድርጉት።
- hyperledger#1982 የ `iroha_crypto` መዋቅሮች መዳረሻን ያጠቃልላል።
- `AssetDefinition` ገንቢ ያክሉ።
- አላስፈላጊውን I18NI0000573X ከኤፒአይ ያስወግዱ።
- የውሂብ ሞዴል መዋቅሮች መዳረሻን ያጠቃልላል.
- ኮር፣ `sumeragi`፣ ምሳሌ ተግባራት፣ `torii`
- hyperledger#1903 የክስተት ልቀት ወደ `modify_*` ዘዴዎች ያንቀሳቅሳል።
- የተከፈለ `data_model` lib.rs ፋይል።
- ወረፋ ላይ wsv ማጣቀሻ ያክሉ።
- hyperledger # 1210 የተከፈለ ክስተት ዥረት.
  - ከግብይት ጋር የተያያዙ ተግባራትን ወደ ዳታ_ሞዴል/የግብይት ሞጁል ውሰድ
- hyperledger#1725 በ Torii ውስጥ ዓለም አቀፍ ሁኔታን ያስወግዱ።
  - `add_state macro_rules` ተግባራዊ ያድርጉ እና `ToriiState` ያስወግዱ
- የሊንተርን ስህተት ያስተካክሉ.
- hyperledger # 1661 `Cargo.toml` ማጽዳት.
  - የጭነት ጥገኛዎችን ደርድር
- hyperledger # 1650 የጸዳ `data_model`
  - ዓለምን ወደ wsv ይውሰዱ ፣ ሚናዎችን ያስተካክሉ ፣ ወደ ቁርጠኝነት ብሎክ ወደ Schema ያውጡ
- የ `json` ፋይሎች እና የንባብ አደረጃጀት። ከአብነት ጋር ለመስማማት Readmeን ያዘምኑ።
- 1529: የተዋቀረ ምዝግብ ማስታወሻ.
  - Refactor የምዝግብ ማስታወሻ መልዕክቶች
- `iroha_p2p`
  - p2p ፕራይቬታይዜሽን ጨምር።

### ሰነድ

- Iroha የደንበኛ CLI ንባብ ያዘምኑ።
- የመማሪያ ክፍሎችን ያዘምኑ።
- 'ድብደባ_በ_ሜታዳታ_ቁልፍ'ን ወደ API spec ያክሉ።
- ወደ ሰነዶች አገናኞችን ያዘምኑ።
- ትምህርትን ከንብረት ጋር በተያያዙ ሰነዶች ያራዝሙ።
- ጊዜ ያለፈባቸው ሰነዶችን ያስወግዱ።
- ሥርዓተ-ነጥብ ይገምግሙ።
- አንዳንድ ሰነዶችን ወደ የማጠናከሪያ ትምህርት ማከማቻ ውሰድ።
- ቅርንጫፉን ለማዘጋጀት የተበላሸ ሪፖርት።
- ለቅድመ-rc.7 changelog ይፍጠሩ.
- ለጁላይ 30 የተበላሸ ሪፖርት።
- የጎማ ስሪቶች.
- የሙከራ ብልሹነትን ያዘምኑ።
- hyperledger#2499 የደንበኛ_ክሊ ስህተት መልዕክቶችን ያስተካክሉ።
- hyperledger # 2344 CHANGELOG ፍጠር ለ 2.0.0-ቅድመ-rc.5-lts.
- ወደ አጋዥ ስልጠናው አገናኞችን ያክሉ።
- በ git መንጠቆዎች ላይ መረጃን ያዘምኑ።
- ብልሹነት ሙከራ ጽሑፍ።
- hyperledger # 2193 አዘምን Iroha ደንበኛ ሰነድ.
- hyperledger # 2193 አዘምን Iroha CLI ሰነድ.
- hyperledger # 2193 README ለ ማክሮ ክሬትን ያዘምኑ።
 - hyperledger # 2193 አዘምን Norito ዲኮደር መሣሪያ ሰነድ.
- hyperledger # 2193 አዘምን Kagami ሰነድ.
- hyperledger#2193 የማመሳከሪያ ሰነዶችን ያዘምኑ።
- hyperledger#2192 አስተዋጽዖ መመሪያዎችን ይገምግሙ።
- የተበላሹ የውስጠ-ኮድ ማጣቀሻዎችን ያስተካክሉ።
- hyperledger#1280 ሰነድ Iroha ሜትሪክስ።
- hyperledger#2119 Iroha በ Docker ዕቃ ውስጥ እንዴት እንደገና መጫን እንደሚቻል ላይ መመሪያን ያክሉ።
- hyperledger # 2181 ግምገማ README.
- hyperledger # 2113 በካርጎ.toml ፋይሎች ውስጥ የሰነድ ባህሪዎች።
- hyperledger#2177 የጊትቻንሎግ ውፅዓትን ያፅዱ።
- hyperledger#1991 readme ወደ ኩራ ኢንስፔክተር ያክሉ።
- hyperledger#2119 Iroha በ I18NT0000043X ዕቃ ውስጥ እንዴት እንደገና መጫን እንደሚቻል ላይ መመሪያን ያክሉ።
- hyperledger # 2181 ግምገማ README.
- hyperledger # 2113 በካርጎ.toml ፋይሎች ውስጥ የሰነድ ባህሪዎች።
- hyperledger#2177 የጊትቻንሎግ ውፅዓትን ያፅዱ።
- hyperledger#1991 readme ወደ ኩራ ኢንስፔክተር ያክሉ።
- የቅርብ ለውጥ መዝገብ መፍጠር.
- የለውጥ መዝገብ ይፍጠሩ.
- ጊዜ ያለፈባቸው README ፋይሎችን ያዘምኑ።
- የጎደሉ ሰነዶች ወደ `api_spec.md` ታክለዋል።

### CI/ሲዲ ይቀየራል።- አምስት ተጨማሪ በራስ የሚስተናገዱ ሯጮችን ይጨምሩ።
- ለሶራሚትሱ መዝገብ መደበኛ የምስል መለያ ያክሉ።
- ለlibgit2-sys 0.5.0 የስራ ቦታ። ወደ 0.4.4 ተመለስ።
- ቅስት ላይ የተመሰረተ ምስል ለመጠቀም ሞክር።
- በአዲስ የምሽት-ብቻ-ኮንቴይነር ላይ ለመስራት የስራ ሂደቶችን ያዘምኑ።
- ሁለትዮሽ የመግቢያ ነጥቦችን ከሽፋን ያስወግዱ።
- የዴቭ ሙከራዎችን ወደ Equinix በራስ የሚስተናገዱ ሯጮች ይቀይሩ።
- hyperledger#2865 የ tmp ፋይል አጠቃቀምን ከ`scripts/check.sh` ያስወግዱ።
- hyperledger#2781 የሽፋን ማካካሻዎችን ይጨምሩ።
- ቀርፋፋ ውህደት ሙከራዎችን አሰናክል።
- የመሠረት ምስልን በዶክተር መሸጎጫ ይተኩ።
- hyperledger # 2781 ኮድኮቭ የወላጅ ባህሪን ያክሉ።
- ስራዎችን ወደ github ሯጮች ይውሰዱ።
- hyperledger # 2778 የደንበኛ ውቅር ያረጋግጡ።
- hyperledger # 2732 የiroha2-base ምስሎችን ለማዘመን እና ለመጨመር ሁኔታዎችን ያክሉ
  የ PR መለያዎች።
- የምሽት ምስል ግንባታን ያስተካክሉ።
- የ`buildx` ስህተትን በI18NI0000587X ያስተካክሉ
- የማይሰራ `tj-actions/changed-files` የመጀመሪያ እርዳታዎች
- ተከታታይ ምስሎችን ማተምን ከ#2662 በኋላ አንቃ።
- ወደብ መዝገብ ያክሉ።
- በራስ ሰር መለያ I18NI0000589X እና `config-changes`
- በምስሉ ውስጥ hash ያከናውኑ ፣ የመሳሪያ ሰንሰለት ፋይል እንደገና ፣ የዩአይኤን ማግለል ፣
  schema መከታተል.
- የህትመት የስራ ፍሰቶችን በቅደም ተከተል እና በ#2427 ማሟያ ያድርጉ።
- hyperledger # 2309: በ CI ውስጥ የዶክ ሙከራዎችን እንደገና አንቃ።
- hyperledger#2165 codecov install አስወግድ።
- ከአሁኑ ተጠቃሚዎች ጋር ግጭቶችን ለመከላከል ወደ አዲስ መያዣ ይውሰዱ።
 - hyperledger # 2158 አሻሽል `parity_scale_codec` እና ሌሎች ጥገኞች። (Norito ኮድ)
- ግንባታን ያስተካክሉ።
- hyperledger # 2461 አሻሽል iroha2 CI.
- `syn` ያዘምኑ።
- ሽፋንን ወደ አዲስ የስራ ሂደት ያንቀሳቅሱ።
- የተገላቢጦሽ docker መግቢያ ver.
- የ `archlinux:base-devel` ስሪት መግለጫን ያስወግዱ
- Dockerfiles እና Codecov ሪፖርቶችን እንደገና ጥቅም ላይ ማዋል እና ኮንኩሬሽን ያዘምኑ።
- የለውጥ መዝገብ ይፍጠሩ.
- `cargo deny` ፋይል አክል.
- ከ`iroha2` የተቀዳ የስራ ፍሰት ያለው የ`iroha2-lts` ቅርንጫፍ ያክሉ
- hyperledger#2393 የ Docker የመሠረት ምስል ሥሪት ይዝለሉ።
- hyperledger # 1658 የሰነድ ማረጋገጫ ያክሉ።
- የሣጥኖች መጨናነቅ እና ጥቅም ላይ ያልዋሉ ጥገኛዎችን ያስወግዱ።
- አላስፈላጊ የሽፋን ዘገባን ያስወግዱ።
- hyperledger#2222 መሸፈኛን ያካትታል ወይም አይጨምርም በሚል የተከፋፈሉ ሙከራዎች።
- hyperledger # 2153 መጠገን # 2154.
- ሥሪት ሁሉንም ሳጥኖች ያደናቅፋል።
- የተዘረጋውን የቧንቧ መስመር ያስተካክሉ።
- hyperledger # 2153 ሽፋንን አስተካክል.
- የዘር ፍተሻን ይጨምሩ እና ሰነዶችን ያዘምኑ።
- ዝገት, ሻጋታ እና ማታ ወደ 1.60, 1.2.0 እና 1.62 በቅደም ተከተል.
- ሎድ-rs ቀስቅሴዎች.
- hyperledger # 2153 መጠገን # 2154.
- ሥሪት ሁሉንም ሳጥኖች ያደናቅፋል።
- የተዘረጋውን የቧንቧ መስመር ያስተካክሉ።
- hyperledger # 2153 ሽፋንን አስተካክል.
- የዘር ፍተሻን ይጨምሩ እና ሰነዶችን ያዘምኑ።
- ዝገት, ሻጋታ እና ማታ ወደ 1.60, 1.2.0 እና 1.62 በቅደም ተከተል.
- ሎድ-rs ቀስቅሴዎች.
- load-rs: የስራ ፍሰት ቀስቅሴዎችን መልቀቅ.
- የግፋ የስራ ፍሰት ያስተካክሉ።
- ቴሌሜትሪ ወደ ነባሪ ባህሪያት ያክሉ።
- በዋናው ላይ የስራ ፍሰት ለመግፋት ተገቢውን መለያ ያክሉ።
- ያልተሳኩ ሙከራዎችን ያስተካክሉ.
- hyperledger # 1657 ምስሉን ወደ ዝገት አዘምን 1.57. #1630፡ ወደ ራስ-ተስተናጋጅ ሯጮች ተመለስ።
- CI ማሻሻያዎች.
- `lld` ለመጠቀም ሽፋን ተቀይሯል።
- CI ጥገኛ መጠገን.
- የ CI ክፍል ማሻሻያዎች.
- በ CI ውስጥ ቋሚ የዝገት ስሪት ይጠቀማል.
- Docker ማተም እና iroha2-dev ግፊት CI አስተካክል. ሽፋን እና አግዳሚ ወንበር ወደ PR ይውሰዱ
- በ CI docker ሙከራ ውስጥ አላስፈላጊውን ሙሉ የI18NT0000068X ግንባታን ያስወግዱ።

  የIroha ግንባታ አሁን በራሱ በዶክተር ምስል ላይ እንደሚደረገው ከንቱ ሆነ። ስለዚህ CI በፈተናዎች ውስጥ ጥቅም ላይ የሚውለውን ደንበኛ ክሊን ብቻ ይገነባል።
- በ CI ቧንቧ ውስጥ ለአይሮሀ2 ቅርንጫፍ ድጋፍን ይጨምሩ።
  - ረጅም ሙከራዎች በ PR ወደ iroha2 ብቻ ነበር የሚሰሩት።
  - ዶከር ምስሎችን ከiroha2 ብቻ ያትሙ
- ተጨማሪ CI መሸጎጫዎች.

### ድር-ጉባኤ


### ሥሪት ጎድቷል።- ስሪት ወደ ቅድመ-rc.13.
- ስሪት ወደ ቅድመ-rc.11.
- ስሪት ወደ RC.9.
- ስሪት ወደ RC.8.
- ስሪቶችን ወደ RC7 ያዘምኑ።
- ቅድመ-ልቀት ዝግጅቶች.
- ሻጋታ 1.0 ያዘምኑ.
- ጥገኞች ጥገኝነት.
- api_spec.md አዘምን፡ መጠየቂያ/ምላሽ አካላትን ያስተካክሉ።
- የዝገት ሥሪትን ወደ 1.56.0 ያዘምኑ።
- የአስተዋጽኦ መመሪያን ያዘምኑ።
- ከአዲስ ኤፒአይ እና ዩአርኤል ቅርጸት ጋር ለማዛመድ README.md እና `iroha/config.json` ያዘምኑ።
- ዶከር ዒላማውን ወደ hyperledger/iroha2 #1453 ያዘምኑ።
- የስራ ፍሰት ከዋናው ጋር እንዲዛመድ ያዘምናል።
- api specን ያዘምኑ እና የጤና የመጨረሻ ነጥብን ያስተካክሉ።
- ዝገት ዝማኔ ወደ 1.54.
- ሰነዶች(iroha_crypto): `Signature` ሰነዶችን ያዘምኑ እና የ`verify` አርጎችን አሰልፍ
- የኡርሳ ስሪት ከ 0.3.5 ወደ 0.3.6 ጎድቷል.
- የስራ ፍሰቶችን ወደ አዲስ ሯጮች ያዘምኑ።
- ለመሸጎጫ እና ፈጣን ci ግንባታዎች dockerfile ያዘምኑ።
- የlibssl ስሪት ያዘምኑ።
- dockerfiles እና async-std ያዘምኑ።
- የዘመነ ቅንጥብ ያስተካክሉ።
- የንብረት መዋቅርን ያዘምናል.
  - በንብረት ውስጥ ለቁልፍ-እሴት መመሪያዎች ድጋፍ
  - የንብረት ዓይነቶች እንደ ዝርዝር
  - በንብረት ISI መጠገን ውስጥ ከመጠን ያለፈ ተጋላጭነት
- የአስተዋጽኦ መመሪያን ያሻሽላል።
- ጊዜው ያለፈበት lib ያዘምኑ።
- ነጭ ወረቀትን ያዘምኑ እና የሽፋን ችግሮችን ያስተካክሉ።
- cucumber_rust lib ያዘምኑ።
- ለቁልፍ ትውልድ README ዝመናዎች።
- Github Actions የስራ ፍሰቶችን ያዘምኑ።
- Github Actions የስራ ፍሰቶችን ያዘምኑ።
- መስፈርቶችን ያዘምኑ.txt.
- common.yaml አዘምን.
- የሰነዶች ዝመናዎች ከሳራ።
- የመመሪያ አመክንዮ ያዘምኑ።
- ነጭ ወረቀት ያዘምኑ።
- የአውታረ መረብ ተግባራት መግለጫን ያሻሽላል።
- በአስተያየቶች ላይ በመመስረት ነጭ ወረቀት ያዘምኑ።
- የ WSV ዝመናን መለያየት እና ወደ ሚዛን ማዛወር።
- gitignore ያዘምኑ።
- በ WP ውስጥ የኩራ ትንሽ መግለጫ ያዘምኑ።
- በነጭ ወረቀት ስለ ኩራ መግለጫ አዘምን።

### እቅድ

- hyperledger#2114 የተደረደሩ ስብስቦች ድጋፍ በሼማዎች ውስጥ።
- hyperledger # 2108 ገጽ ጨምር።
- hyperledger#2114 የተደረደሩ ስብስቦች ድጋፍ በሼማዎች ውስጥ።
- hyperledger # 2108 ገጽ ጨምር።
- ሼማ፣ ሥሪት እና ማክሮ ኖ_ስትድ ተኳሃኝ ያድርጉ።
- በእቅድ ውስጥ ፊርማዎችን ያስተካክሉ።
- የ `FixedPoint` ውክልና በእቅድ ውስጥ ተቀይሯል።
- `RawGenesisBlock` ወደ ንድፍ ውስጣዊ እይታ ታክሏል።
ንድፍ IR-115 ለመፍጠር የነገር-ሞዴሎች ተለውጠዋል።

### ሙከራዎች

- hyperledger # 2544 አጋዥ ትምህርቶች።
- hyperledger#2272 ለ 'FindAssetDefinitionById' መጠይቅ ሙከራዎችን ያክሉ።
- የ `roles` ውህደት ሙከራዎችን ያክሉ።
- የዩአይ ፈተናዎችን ቅርፀት መደበኛ አድርግ፣ ሣጥኖችን ለማግኘት የዩአይ ፈተናዎችን አንቀሳቅስ።
- የማስመሰል ሙከራዎችን ያስተካክሉ (የወደፊቱ ያልታዘዘ ሳንካ)።
- የ DSL ሣጥን ተወግዷል እና ሙከራዎችን ወደ `data_model` ተንቀሳቅሷል
- ያልተረጋጋ የአውታረ መረብ ሙከራዎች ለትክክለኛ ኮድ ማለፋቸውን ያረጋግጡ።
- ሙከራዎች ወደ iroha_p2p ታክለዋል።
- ፈተና ካልተሳካ በስተቀር የፈተና ምዝግብ ማስታወሻዎችን ይይዛል።
- ለፈተናዎች ምርጫን ይጨምሩ እና አልፎ አልፎ የሚበላሹ ፈተናዎችን ያስተካክሉ።
- ትይዩ ቅንብርን ይፈትሻል።
- ከአይሮሃ ኢንት እና ከአይሮሀ_ደንበኛ ሙከራዎች ስር ያስወግዱ።
- የፈተናዎች ቅንጥብ ማስጠንቀቂያዎችን ያስተካክሉ እና ቼኮችን ወደ ci ይጨምሩ።
- በቤንችማርክ ሙከራዎች ወቅት የ`tx` የማረጋገጫ ስህተቶችን ያስተካክሉ።
- hyperledger # 860: Iroha መጠይቆች እና ሙከራዎች.
- Iroha ብጁ ISI መመሪያ እና የኩሽ ሙከራዎች።
- ለ no-std ደንበኛ ሙከራዎችን ያክሉ።
- ድልድይ ምዝገባ ለውጦች እና ሙከራዎች.
- ከአውታረ መረብ ማሾፍ ጋር የጋራ ስምምነት ሙከራዎች።
- ለሙከራዎች አፈፃፀም የሙቀት ዲር አጠቃቀም።
- አግዳሚ ወንበሮች አወንታዊ ጉዳዮችን ይፈትሻል።
- የመጀመርያው የመርክል ዛፍ ተግባር ከሙከራዎች ጋር።
- ቋሚ ሙከራዎች እና የዓለም ግዛት እይታ ጅምር።

### ሌላ- ፓራሜትሪሽን ወደ ባህሪያት ይውሰዱ እና የ FFI IR ዓይነቶችን ያስወግዱ።
- ለሠራተኛ ማህበራት ድጋፍን ይጨምሩ ፣ `non_robust_ref_mut` ያስተዋውቁ * conststring FFI ልወጣን ይተግብሩ።
- IdOrdEqHashን አሻሽል።
- FilterOpt:: BySomeን ከ(de-) ተከታታይነት ያስወግዱ።
- ግልጽ ያልሆነ አድርግ.
- ContextValue ግልጽ አድርግ።
- አገላለጽ ያድርጉ :: ጥሬ መለያ አማራጭ።
- ለአንዳንድ መመሪያዎች ግልጽነት ይጨምሩ.
- የRoleID ተከታታይነትን አሻሽል።
- አሻሽል (de-) ተከታታይ ማረጋገጫ :: መታወቂያ.
- የ PermissionTokenId ተከታታይነት አሻሽል።
- የTriggerId ተከታታይነትን አሻሽል።
- አሻሽል (de-) የንብረት መለያ (-ፍቺ) መታወቂያዎች።
- የአካውንትአይድን ተከታታይነት ማሻሻል (መሰረዝ)።
- የIpfs እና DomainId ተከታታይነት (de-) ማሻሻል።
- የሎገር ውቅረትን ከደንበኛ ውቅር ያስወግዱ።
- በFFI ውስጥ ግልጽ ለሆኑ መዋቅሮች ድጋፍን ይጨምሩ።
- Refactor &አማራጭ<T> ወደ አማራጭ<&T>
- ቅንጥብ ማስጠንቀቂያዎችን ያስተካክሉ።
- በ `Find` የስህተት መግለጫ ውስጥ ተጨማሪ ዝርዝሮችን ያክሉ።
- `PartialOrd` እና `Ord` አተገባበርን ያስተካክሉ።
- ከ `cargo fmt` ይልቅ `rustfmt` ይጠቀሙ
- የ `roles` ባህሪን ያስወግዱ።
- ከ `cargo fmt` ይልቅ `rustfmt` ይጠቀሙ
- ከዴቭ ዶከር ምሳሌዎች ጋር workdir እንደ ጥራዝ ያጋሩ።
- በ Execute ውስጥ Diff ተዛማጅ አይነት ያስወግዱ.
- ከባለብዙቫል መመለስ ይልቅ ብጁ ኢንኮዲንግ ይጠቀሙ።
- serde_jsonን እንደ iroha_crypto ጥገኝነት ያስወግዱ።
- በስሪት ባህሪ ውስጥ የሚታወቁ መስኮችን ብቻ ፍቀድ።
- ለመጨረሻ ነጥቦች የተለያዩ ወደቦችን ያፅዱ።
- `Io` መገኛን ያስወግዱ።
- የቁልፍ_ጥንዶች የመጀመሪያ ሰነዶች።
- ወደ ራስ-ተስተናጋጅ ሯጮች ተመለስ።
- በኮዱ ውስጥ አዲስ ክሊፕ ፒን ያስተካክሉ።
- i1i1ን ከጠባቂዎች ያስወግዱ.
- የተዋናይ ሰነድ እና ጥቃቅን ጥገናዎችን ያክሉ።
- የቅርብ ብሎኮችን ከመግፋት ይልቅ የሕዝብ አስተያየት ይስጡ።
- የግብይት ሁኔታ ክስተቶች ለእያንዳንዱ 7 እኩዮች ተፈትነዋል።
- ከ `join_all` ይልቅ `FuturesUnordered`
- ወደ GitHub Runners ቀይር።
- ለ/መጠይቁ የመጨረሻ ነጥብ VersionedQueryResult vs QueryResult ይጠቀሙ።
- ቴሌሜትሪ እንደገና ያገናኙ.
- dependabot ውቅር አስተካክል.
- ማጥፋትን ለማካተት መፈጸም-msg git hook ያክሉ።
- የግፋውን ቧንቧ ያስተካክሉ.
- ጥገኛ አሻሽል.
- በወረፋ ግፊት ላይ የወደፊት የጊዜ ማህተምን ያግኙ።
- hyperledger # 1197: ኩራ ስህተቶችን ያስተናግዳል.
- የአቻዎችን መመሪያ አስወግድ።
- ግብይቶችን ለመለየት አማራጭ ኖኖ ይጨምሩ። ዝጋ #1493
- አላስፈላጊ `sudo` ተወግዷል።
- ለጎራዎች ዲበ ውሂብ።
- በ `create-docker` የስራ ፍሰት ውስጥ የዘፈቀደ ማገገሚያዎችን ያስተካክሉ።
- በመጥፋቱ የቧንቧ መስመር እንደተጠቆመው `buildx` ታክሏል።
- hyperledger # 1454: የጥያቄ ስህተት ምላሽ ከተወሰነ የሁኔታ ኮድ እና ፍንጮች ጋር ያስተካክሉ።
- hyperledger # 1533: ግብይት በሃሽ ይፈልጉ።
- `configure` የመጨረሻ ነጥብ አስተካክል.
- በቦሊያን ላይ የተመሰረተ የንብረት አነስተኛነት ማረጋገጫ ያክሉ።
- የተተየቡ ክሪፕቶ ፕሪሚቲቭስ መጨመር እና ወደ አይነት-አስተማማኝ ምስጠራ ፍልሰት።
- የምዝግብ ማስታወሻ ማሻሻያዎች.
- hyperledger # 1458: እንደ `mailbox` ለማዋቀር የተዋናይ ቻናል መጠን ይጨምሩ።
- hyperledger#1451: `faulty_peers = 0` እና `trusted peers count > 1` ከሆነ ስለ የተሳሳተ ውቅር ማስጠንቀቂያ ያክሉ
- የተወሰነ የማገጃ ሃሽ ለማግኘት ተቆጣጣሪ ያክሉ።
- FindTransactionByHash አዲስ መጠይቅ ታክሏል።
- hyperledger # 1185: የሳጥን ስም እና መንገድ ይቀይሩ.
- መዝገቦችን እና አጠቃላይ ማሻሻያዎችን ያስተካክሉ።
- hyperledger # 1150: ቡድን 1000 በእያንዳንዱ ፋይል ውስጥ ብሎኮች
- ወረፋ ውጥረት ፈተና.
- የምዝግብ ማስታወሻ ደረጃ ማስተካከል.
- ለደንበኛ ቤተ-መጽሐፍት የራስጌ መግለጫ ያክሉ።
- የወረፋ መደናገጥ አለመሳካት ማስተካከል።
- ወረፋ አስተካክል።
- የዶክ ፋይል መልቀቂያ ግንባታን አስተካክል።
- የኤችቲቲፒ ደንበኛ መጠገን።
- ማፋጠን ci.
- 1. ከአይሮሃ_ክሪቶ በስተቀር ሁሉንም የኡርሳ ጥገኞች ተወግዷል።
- ቆይታዎችን ሲቀንሱ የትርፍ ፍሰትን ያስተካክሉ።
- መስኮችን በደንበኛው ውስጥ ይፋ ያድርጉ።
- እንደ ምሽት Iroha2ን ወደ Dockerhub ግፋ።
- የ http ሁኔታ ኮዶችን ያስተካክሉ።
- አይሮሀ_ስህተትን በዚህ ስህተት፣ አይር እና ባለቀለም አይር ይተኩ።
- ወረፋ በመስቀልበም አንድ ይተኩ።- አንዳንድ የማይጠቅሙ የሊንት አበል ያስወግዱ።
- ለንብረት ፍቺዎች ሜታዳታን ያስተዋውቃል።
- ክርክሮችን ከ test_network crate ማስወገድ።
- አላስፈላጊ ጥገኛዎችን ያስወግዱ.
- iroha_client_cli ያስተካክሉ :: ክስተቶች.
- hyperledger # 1382: የድሮውን የአውታረ መረብ ትግበራ ያስወግዱ.
- hyperledger # 1169: ለንብረቶች ትክክለኛነት ታክሏል.
- በእኩዮች ጅምር ላይ መሻሻል;
  - የዘፍጥረት ህዝባዊ ቁልፍን ከኤንቪ ብቻ መጫን ይፈቅዳል
  - config, genesis እና trusted_peers ዱካ አሁን በ cli params ውስጥ ሊገለጹ ይችላሉ
- hyperledger#1134፡ የIroha P2P ውህደት።
- ከGET ይልቅ የመጠይቁን የመጨረሻ ነጥብ ወደ POST ቀይር።
- በተዋናይነት በተጀመረበት ጊዜ ያስፈጽሙ።
- ወደ ሽግሽግ.
- ከደላላ ስህተት ጥገናዎች ጋር እንደገና መሥራት።
- "የብዙ ደላላ ጥገናዎችን ያስተዋውቃል" ቁርጠኝነትን አድህር(9c148c33826067585b5868d297dcdd17c0efe246)
- በርካታ የደላላ ጥገናዎችን ያስተዋውቃል፡-
  - በተዋናይ ማቆሚያ ላይ ከደላላ ደንበኝነት ምዝገባ ይውጡ
  - ከተመሳሳይ የተዋናይ አይነት ብዙ ምዝገባዎችን ይደግፉ (ከዚህ ቀደም TODO)
  - ደላላ ሁል ጊዜ እራሱን እንደ ተዋናይ መታወቂያ የሚያስቀምጥበትን ስህተት ያስተካክሉ።
- ደላላ ስህተት (የሙከራ ማሳያ)።
- ለውሂብ ሞዴል አመጣጥ ያክሉ።
- rwlockን ከቶሪ ያስወግዱ።
- የ OOB መጠይቅ ፈቃድ ፍተሻዎች።
- hyperledger # 1272: የእኩዮች ብዛት መተግበር;
- በመመሪያው ውስጥ የጥያቄ ፈቃዶችን ተደጋጋሚ ፍተሻ።
- የማቆሚያ ተዋናዮችን መርሐግብር ያስይዙ.
- hyperledger # 1165: የእኩዮች ቆጠራን መተግበር.
- የጥያቄ ፈቃዶችን በቶሪ መጨረሻ ነጥብ ውስጥ በመለያ ይፈትሹ።
- ሲፒዩን የሚያጋልጥ እና የማህደረ ትውስታ አጠቃቀም በስርዓት መለኪያዎች ተወግዷል።
 - ለ WS መልእክቶች JSON በI18NT0000032X ይተኩ።
- የማከማቻ ማረጋገጫ የእይታ ለውጦች.
- hyperledger # 1168: ግብይቱ የፊርማ ማረጋገጫ ሁኔታን ካላለፈ መግባቱ ተጨምሯል።
- የተስተካከሉ ትናንሽ ጉዳዮች ፣ የተጨመረ የግንኙነት ማዳመጥ ኮድ።
- የአውታረ መረብ ቶፖሎጂ ገንቢን ያስተዋውቁ።
- ለIroha የP2P አውታረ መረብን ይተግብሩ።
- የማገጃ መጠን መለኪያን ይጨምራል.
- የፈቃድ አረጋጋጭ ባህሪ ወደ አይፈቀድም ተቀይሯል። እና ተጓዳኝ ሌሎች ስም ለውጦች
- API spec የድር ሶኬት እርማቶች።
- አላስፈላጊ ጥገኞችን ከዶክተር ምስል ያስወግዳል።
- Fmt Crate import_granularity ይጠቀማል።
- አጠቃላይ የፍቃድ ማረጋገጫን ያስተዋውቃል።
- ወደ ተዋንያን ማዕቀፍ ስደዱ።
- የደላሎች ንድፍ ይቀይሩ እና ለተዋናዮች አንዳንድ ተግባራትን ያክሉ።
- የኮድኮቭ ሁኔታ ፍተሻዎችን ያዋቅራል።
- ምንጭ ላይ የተመሠረተ ሽፋን ከ grcov ጋር ይጠቀማል።
- በርካታ የግንባታ-args ቅርጸት እና ለመካከለኛ የግንባታ ኮንቴይነሮች ARG እንደገና ታወጀ።
- የደንበኝነት ምዝገባ ተቀባይነት ያለው መልእክት ያስተዋውቃል።
- ከተጠቀሙ በኋላ የዜሮ እሴት ንብረቶችን ከመለያዎች ያስወግዱ።
- ቋሚ ዶከር ግንባታ ነጋሪ እሴቶች ቅርጸት።
- የልጆች እገዳ ካልተገኘ ቋሚ የስህተት መልእክት።
- ለመገንባት ክፍት ኤስኤስኤል ታክሏል፣ pkg-config ጥገኝነትን ያስተካክላል።
- ለ dockerhub እና የሽፋን ልዩነት የማከማቻ ስም ያስተካክሉ።
የታመኑ አቻዎች ሊጫኑ ካልቻሉ ግልጽ የሆነ የስህተት ጽሑፍ እና የፋይል ስም ታክሏል።
- የጽሑፍ አካላትን በሰነዶች ውስጥ ወደ አገናኞች ተለውጧል።
- የተሳሳተ የተጠቃሚ ስም ሚስጥር በ Docker ማተም ውስጥ ያስተካክሉ።
- በነጭ ወረቀት ላይ ትንሽ የትየባ ያስተካክሉ።
- ለተሻለ የፋይል መዋቅር mod.rs አጠቃቀምን ይፈቅዳል።
- main.rs ወደ የተለየ ሣጥን ይውሰዱ እና ለሕዝብ blockchain ፈቃድ ይፍጠሩ።
- በደንበኛ ክሊ ውስጥ መጠይቅን ያክሉ።
- ከጭብጨባ ወደ structopts ለ cli ፍልሰት።
- ቴሌሜትሪ ወደ ያልተረጋጋ የአውታረ መረብ ሙከራ ይገድቡ።
- ባህሪያትን ወደ ስማርት ኮንትራክተሮች ሞዱል ይውሰዱ።
- Sed -i "s/world_state_view/wsv/g"
- ብልጥ ኮንትራቶችን ወደ ተለየ ሞጁል ያንቀሳቅሱ።
- Iroha የአውታረ መረብ ይዘት ርዝመት bugfix.
- ለተዋናይ መታወቂያ የተግባር አካባቢያዊ ማከማቻን ይጨምራል። መቆለፊያን ለመለየት ይጠቅማል።
- የሞት መቆለፊያ ምርመራን ወደ CI ያክሉ
- ኢንትሮስፔክተር ማክሮን ይጨምሩ።
- የስራ ፍሰት ስሞችን እንዲሁም እርማቶችን በመቅረጽ ላይ ያታልላል
- የጥያቄ ኤፒአይ ለውጥ።
- ከአሲንክ-ኤስዲ ወደ ቶኪዮ ስደት።
- የቴሌሜትሪ ትንታኔን ወደ ci ያክሉ።- ለአይሮሃ የወደፊት ቴሌሜትሪ ያክሉ።
- ለእያንዳንዱ የአስመር ተግባር የኢሮሃ የወደፊትን ያክሉ።
- ለምርጫዎች ብዛት ታዛቢነት የኢሮሃ የወደፊትን ያክሉ።
- በእጅ ማሰማራት እና ማዋቀር ወደ README ታክሏል።
- የጋዜጠኞች ማስተካከያ.
- የመልእክት ማክሮን ያክሉ።
- ቀላል የተዋንያን ማዕቀፍ ያክሉ።
- dependabot ውቅር ያክሉ.
- ጥሩ ሽብር እና ስህተት ዘጋቢዎችን ያክሉ።
- የዝገት ስሪት ወደ 1.52.1 እና ተዛማጅ ጥገናዎች ፍልሰት።
- በተለየ ክሮች ውስጥ የሲፒዩ ከፍተኛ ተግባራትን ማገድ።
- ከ crates.io ልዩ_ወደብ እና ካርጎ-ሊንቶችን ይጠቀሙ።
- ከመቆለፊያ ነፃ WSV አስተካክል፡
  - በኤፒአይ ውስጥ አላስፈላጊ Dashmaps እና ቁልፎችን ያስወግዳል
  - የተፈጠሩትን ከመጠን በላይ የሆኑ ብሎኮችን ያስተካክላል (የተጣሉ ግብይቶች አልተመዘገቡም)
  - ለስህተቶች ሙሉ የስህተት መንስኤን ያሳያል
- የቴሌሜትሪ ተመዝጋቢ ያክሉ።
- ሚናዎች እና ፈቃዶች ጥያቄዎች.
- ብሎኮችን ከኩራ ወደ wsv ይውሰዱ።
- በ wsv ውስጥ ከመቆለፊያ ነፃ የሆኑ የውሂብ አወቃቀሮችን ይቀይሩ።
- የአውታረ መረብ ጊዜ ማብቂያ ማስተካከል.
- የጤና የመጨረሻ ነጥብ አስተካክል.
- ሚናዎችን ያስተዋውቃል.
- የግፋ ዶከር ምስሎችን ከዴቭ ቅርንጫፍ ያክሉ።
- የበለጠ ኃይለኛ ሽፋንን ይጨምሩ እና ድንጋጤን ከኮድ ያስወግዱ።
- ለመመሪያዎች የማስፈጸሚያ ባህሪን እንደገና መሥራት።
- የድሮውን ኮድ ከiroha_config ያስወግዱ።
- IR-1060 ለሁሉም ነባር ፍቃዶች የግራንት ፍተሻዎችን ይጨምራል።
- ለiroha_network ገደብ እና ጊዜ ማብቂያ ያስተካክሉ።
- የ Ci የጊዜ ማብቂያ ሙከራ ማስተካከያ።
- ትርጉማቸው ሲወገድ ሁሉንም ንብረቶች ያስወግዱ።
- ንብረት ሲጨምሩ wsv ፍርሃትን ያስተካክሉ።
- ለሰርጦች Arc እና Rwlockን ያስወግዱ።
- Iroha አውታረ መረብ መጠገን.
- የፈቃድ አረጋጋጮች በቼኮች ውስጥ ማጣቀሻዎችን ይጠቀማሉ።
- የስጦታ መመሪያ.
- ለሕብረቁምፊ ርዝመት ገደቦች የታከለ ውቅር እና የመታወቂያ ማረጋገጫ ለNewAccount፣ Domain እና AssetDefinition IR-1036።
- በክትትል ሊብ ይተኩ።
- ለሰነዶች ci ቼክ ይጨምሩ እና dbg ማክሮን ይክዱ።
- ሊፈቀዱ የሚችሉ ፈቃዶችን ያስተዋውቃል።
- Iroha_config crate ጨምር።
ሁሉንም ገቢ የውህደት ጥያቄዎችን ለማጽደቅ @alerdenisov እንደ ኮድ ባለቤት ያክሉ።
- በስምምነት ጊዜ የግብይቱን መጠን ቼክ ማስተካከል።
- የ async-std ማሻሻልን ይመልሱ።
- አንዳንድ ወጪዎችን በ 2 IR-1035 ኃይል ይተኩ።
- የግብይት ታሪክን IR-1024 ለማውጣት መጠይቅ ያክሉ።
- ለማከማቻ እና የፈቃድ አረጋጋጮችን መልሶ ማዋቀር የፍቃዶችን ማረጋገጫ ያክሉ።
- ለመለያ ምዝገባ አዲስ አካውንት ያክሉ።
- ለንብረት ፍቺ ዓይነቶችን ያክሉ።
- ሊዋቀሩ የሚችሉ የሜታዳታ ገደቦችን ያስተዋውቃል።
- የግብይት ዲበ ውሂብን ያስተዋውቃል።
- በጥያቄዎች ውስጥ መግለጫዎችን ያክሉ።
- lints.toml ያክሉ እና ማስጠንቀቂያዎችን ያስተካክሉ።
- የታመኑ_አቻዎችን ከ config.json ይለዩ።
- በቴሌግራም ውስጥ ለI18NT0000076X 2 ማህበረሰብ በዩአርኤል ውስጥ የትየባ ያስተካክሉ።
- ቅንጥብ ማስጠንቀቂያዎችን ያስተካክሉ።
- ለመለያ ቁልፍ እሴት ሜታዳታ ድጋፍን ያስተዋውቃል።
- የብሎኮችን ስሪት ያክሉ።
- የ ci linting ድግግሞሾችን ያስተካክሉ።
- mul,div,mod,ወደ አገላለጾች ማሳደግ.
- ለሥሪት ወደ_v* ያክሉ።
- ምትክ ስህተት :: msg ከስህተት ማክሮ ጋር።
- Iroha_http_server እንደገና ይፃፉ እና የቶሪ ስህተቶችን እንደገና ይስሩ።
 - Norito ስሪትን ወደ 2 አሻሽሏል።
- የነጫጭ ወረቀት ስሪት መግለጫ።
- የማይሳሳት ገጽ. ጉዳዩን ያስተካክሉ በስህተት በገጽ መፃፍ የማያስፈልግ ሲሆን በምትኩ ባዶ ስብስቦችን አይመልስም።
- ለኢነምሶች መነሻ (ስህተት) ያክሉ።
- የምሽት ስሪት አስተካክል.
- Iroha_error crate ጨምር።
- የተሻሻሉ መልዕክቶች.
- የመያዣ ሥሪት ፕሪሚቲቭን ያስተዋውቃል።
- መለኪያዎችን ያስተካክሉ።
- ገጽን ያክሉ።
- የቫሪንት ኢንኮዲንግ መፍታትን ያክሉ።
- የጥያቄ ጊዜ ማህተም ወደ u128 ቀይር።
- የቧንቧ መስመር ክስተቶች ውድቅ ምክንያት enum ያክሉ.
- ጊዜ ያለፈባቸውን መስመሮች ከጄኔሲስ ፋይሎች ያስወግዳል. መድረሻው በቀደሙት ግዴታዎች ከመመዝገቢያ ISI ተወግዷል።
- መመዝገብን ያቃልላል እና አይኤስአይኤስን ያስወጣል።
- በ 4 አቻ አውታረመረብ ውስጥ አለመላኩን የቁርጥ ቀን ጊዜን ያስተካክሉ።
- ቶፖሎጂ በለውጥ እይታ ላይ ይዋዥቃል።- ከVariant የሚመነጨው ማክሮ ሌሎች መያዣዎችን ያክሉ።
- ለደንበኛ cli MST ድጋፍን ያክሉ።
- ከVariant ማክሮ እና የጽዳት ኮድ ቤዝ ያክሉ።
- i1i1 ወደ ኮድ ባለቤቶች ያክሉ።
- የሀሜት ግብይቶች።
- ለመመሪያዎች እና መግለጫዎች ርዝመትን ይጨምሩ.
- ጊዜን ለማገድ እና የጊዜ መለኪያዎችን ለመፈጸም ሰነዶችን ያክሉ።
- በ TryFrom ተተካ አረጋግጥ እና ባህሪያትን ተቀበል።
- ለዝቅተኛው የእኩዮች ብዛት ብቻ መጠበቅን ያስተዋውቁ።
- አፒን በiroha2-java ለመሞከር የgithub ድርጊትን ያክሉ።
- ለዶከር-ድርሰት-single.yml ዘፍጥረት ይጨምሩ።
- የመለያው ነባሪ ፊርማ ማረጋገጫ ሁኔታ።
- ከበርካታ ፈራሚዎች ጋር ለመለያ ሙከራ ያክሉ።
- ለ MST የደንበኛ API ድጋፍን ያክሉ።
- በዶክተር ውስጥ ይገንቡ.
- ዘፍጥረትን ወደ ዶከር አቀናብር ያክሉ።
- ሁኔታዊ MST ያስተዋውቁ።
- መጠበቅ_ለአክቲቭ_እኩዮች impl ያክሉ።
- በiroha_http_server ውስጥ ለ isahc ደንበኛ ሙከራን ይጨምሩ።
- የደንበኛ API spec
- በመግለጫዎች ውስጥ የጥያቄ አፈፃፀም።
- መግለጫዎችን እና አይኤስአይኤስን ያዋህዳል።
- ለ ISI መግለጫዎች.
- የመለያ ውቅረት መለኪያዎችን ያስተካክሉ።
- ለደንበኛው የመለያ ውቅር ያክሉ።
- `submit_blocking` አስተካክል.
- የቧንቧ መስመር ዝግጅቶች ይላካሉ.
- Iroha ደንበኛ የድር ሶኬት ግንኙነት.
- የቧንቧ መስመር እና የውሂብ ክስተቶች ክስተቶች መለያየት.
- ለፈቃዶች የውህደት ሙከራ።
- የተቃጠለ እና ሚንት የፍቃድ ፍተሻዎችን ያክሉ።
- የ ISI ፍቃድን ያውጡ።
- ለዓለም መዋቅር PR መለኪያዎችን ያስተካክሉ።
- የዓለም መዋቅርን ያስተዋውቁ።
- የጄኔሲስ ማገጃውን የመጫኛ ክፍልን ተግባራዊ ያድርጉ.
- የዘፍጥረት መለያን አስተዋውቅ።
- የፍቃዶች አረጋጋጭ ገንቢን ያስተዋውቁ።
- መለያዎችን ወደ Iroha2 PRs በ Github Actions ያክሉ።
- የፈቃዶችን መዋቅር ማስተዋወቅ።
- ወረፋ tx tx ቁጥር ገደብ እና Iroha ማስጀመሪያ ጥገናዎች።
- Hashን በህንፃ ውስጥ ጠቅልለው።
- የምዝግብ ማስታወሻ ደረጃን ማሻሻል;
  - የመረጃ ደረጃ ምዝግብ ማስታወሻዎችን ወደ ስምምነት ያክሉ።
  - የአውታረ መረብ ግንኙነት መዝገቦችን እንደ የመከታተያ ደረጃ ምልክት ያድርጉ።
  - ብዜት ስለሆነ እና ሁሉንም ብሎክቼይን በምዝግብ ማስታወሻዎች ውስጥ ስላሳየ የብሎክ ቬክተርን ከ WSV ያስወግዱ።
  - የመረጃ ምዝግብ ማስታወሻ ደረጃን እንደ ነባሪ ያዘጋጁ።
- ለማረጋገጫ የሚለወጡ የWSV ማጣቀሻዎችን ያስወግዱ።
- የሄም ስሪት ጭማሪ።
- ነባሪ የታመኑ አቻዎችን ወደ ውቅሩ ያክሉ።
- የደንበኛ ኤፒአይ ወደ http ፍልሰት።
- ማስተላለፍ isi ወደ CLI ያክሉ።
- የ Iroha የአቻ ተዛማጅ መመሪያዎች ውቅር።
- የጎደሉትን ISI ትግበራ ዘዴዎችን እና ሙከራዎችን መፈጸም.
- የኡርል መጠይቅ ፓራሞችን በመተንተን ላይ
- `HttpResponse::ok()`፣ `HttpResponse::upgrade_required(..)` አክል
- የድሮ መመሪያ እና መጠይቅ ሞዴሎችን በ Iroha DSL አቀራረብ መተካት።
- የ BLS ፊርማዎችን ድጋፍ ያክሉ።
- http አገልጋይ crate ያስተዋውቁ.
- የተለጠፈ libssl.so.1.0.0 በሲምሊንክ።
- ለግብይት የመለያ ፊርማ ያረጋግጣል።
- Refactor የግብይት ደረጃዎች.
- የመጀመሪያ ጎራዎች ማሻሻያዎች።
- የ DSL ፕሮቶታይፕን ተግብር።
- Torii ቤንችማርኮችን አሻሽል፡ ወደ መመዘኛዎች መግባትን አሰናክል፣ የስኬት ጥምርታ አክል።
- የሙከራ ሽፋን ቧንቧን ያሻሽሉ፡ `tarpaulin`ን በ`grcov` ይተካዋል፣የፈተና ሽፋን ሪፖርትን ለ`codecov.io` ያትሙ።
- የ RTD ገጽታን ያስተካክሉ።
- ለአይሮሀ ንኡስ ፕሮጀክቶች ማቅረቢያ ቅርሶች።
- `SignedQueryRequest` ያስተዋውቁ.
- በፊርማ ማረጋገጫ ስህተትን ያስተካክሉ።
- የመመለሻ ግብይቶችን ይደግፋል።
- የመነጨ ቁልፍ-ጥንድ እንደ json ያትሙ።
- `Secp256k1` ቁልፍ-ጥንድ ይደግፉ።
- ለተለያዩ crypto ስልተ ቀመሮች የመጀመሪያ ድጋፍ።
- DEX ባህሪዎች
- ሃርድ ኮድ የተደረገ የማዋቀሪያ መንገድ በ cli param ይተኩ።
- የቤንች ዋና የስራ ፍሰት ማስተካከል.
- Docker ክስተት ግንኙነት ሙከራ.
- Iroha የመቆጣጠሪያ መመሪያ እና CLI.
- ክስተቶች cli ማሻሻያዎች.
- ክስተቶች ማጣሪያ.
- የክስተት ግንኙነቶች.
- በዋና የሥራ ሂደት ውስጥ ያስተካክሉ።
- Rtd ለአይሮሃ2.
- Merkle tree root hash ለብሎክ ግብይቶች።
- ህትመት ወደ docker hub.
- ለጥገና ግንኙነት የ CLI ተግባር።
- ለጥገና ግንኙነት የ CLI ተግባር።
- ማክሮ ለመግባት Eprintln.- የምዝግብ ማስታወሻ ማሻሻያዎች.
- IR-802 የሁኔታ ለውጦችን የሚያግድ ምዝገባ።
- ግብይቶችን እና እገዳዎችን የሚላኩ ክስተቶች።
- የ Sumeragi የመልእክት አያያዝን ወደ መልእክት ኢምፕል ያንቀሳቅሳል።
- አጠቃላይ የግንኙነት ዘዴ.
- ምንም-std ደንበኛ Iroha ጎራ አካላት ማውጣት.
- ግብይቶች TTL.
- ከፍተኛ ግብይቶች በብሎክ ውቅር።
- ልክ ያልሆኑ ብሎኮች hashes ያከማቹ።
- ብሎኮችን በቡድን ያመሳስሉ ።
- የግንኙነት ተግባራትን ማዋቀር.
- ከ Iroha ተግባር ጋር ይገናኙ።
- የማረጋገጫ እርማቶችን አግድ.
- ማመሳሰልን አግድ: ንድፎችን.
- ከ Iroha ተግባር ጋር ይገናኙ።
ድልድይ: ደንበኞችን ያስወግዱ.
- ማመሳሰልን አግድ።
- AddPeer ISI.
- የመመሪያውን ስም ለመቀየር ትእዛዝ።
- ቀላል መለኪያዎች የመጨረሻ ነጥብ።
ድልድይ፡ የተመዘገቡ ድልድዮችን እና የውጭ ንብረቶችን ያግኙ።
- Docker በቧንቧ ውስጥ ሙከራን ያቀናብሩ።
- በቂ ድምጾች የሉም Sumeragi ሙከራ።
- ሰንሰለት ማገድ.
- ድልድይ: በእጅ የውጭ ማስተላለፊያዎች አያያዝ.
- ቀላል የጥገና የመጨረሻ ነጥብ.
- ወደ ሰርዴ-ጄሰን ስደት.
- Demint ISI.
- የድልድይ ደንበኞችን፣ የAddSignatory ISI እና የ CanAddSignatory ፍቃድን ያክሉ።
- Sumeragi: ስብስብ b ተዛማጅ TODO ጥገናዎች ውስጥ እኩዮችህ.
- ወደ Sumeragi ከመፈረሙ በፊት እገዳውን ያረጋግጣል።
- ድልድይ ውጫዊ ንብረቶች.
- በ Sumeragi መልዕክቶች ውስጥ ፊርማ ማረጋገጥ.
- ሁለትዮሽ ንብረቶች-ማከማቻ.
- PublicKey ተለዋጭ ስም በአይነት ይተኩ።
- ለህትመት ሳጥኖችን ያዘጋጁ.
- በኔትወርክ ቶፖሎጂ ውስጥ ዝቅተኛ የድምፅ አመክንዮ።
- የግብይት ደረሰኝ ማረጋገጫ ማደስ።
- OnWorldStateViewChange መቀስቀሻ ለውጥ፡ IrohaQuery ከመመሪያው ይልቅ።
- በኔትወርክ ቶፖሎጂ ውስጥ ከመጀመሪያው የተለየ ግንባታ.
- ከ Iroha ክስተቶች ጋር የተያያዙ የ Iroha ልዩ መመሪያዎችን ያክሉ።
- የፍጥረት ጊዜ ማብቂያ አያያዝን አግድ።
- የቃላት መፍቻ እና Iroha ሞዱል ሰነዶች እንዴት እንደሚጨምሩ።
- ሃርድ ኮድ የተደረገ ድልድይ ሞዴል በመነሻ Iroha ሞዴል ይተኩ።
- የአውታረ መረብ ቶፖሎጂ መዋቅርን ያስተዋውቁ።
- ከመመሪያዎች ለውጥ ጋር የፈቃድ አካልን ያክሉ።
- በመልእክት ሞጁል ውስጥ Sumeragi መልእክቶች።
- ለኩራ የዘፍጥረት እገዳ ተግባራዊነት።
- ለ I18NT0000089X ሳጥኖች README ፋይሎችን ያክሉ።
- ድልድይ እና ይመዝገቡ ድልድይ ISI.
- ከ I18NT0000090X ጋር የመጀመሪያ ስራ አድማጮችን ይለውጣል።
- የፍቃድ ፍተሻዎችን ወደ OOB ISI ማስገባት።
- Docker በርካታ እኩዮች ማስተካከል.
- የአቻ ለአቻ ዶከር ምሳሌ።
- የግብይት ደረሰኝ አያያዝ.
- Iroha ፈቃዶች።
- ሞጁል ለዴክስ እና ለብሪጅስ ሳጥኖች።
- የውህደት ሙከራን ከብዙ እኩዮች ጋር ከንብረት ፈጠራ ጋር ያስተካክሉ።
- የንብረት ሞዴልን ወደ EC-S- እንደገና መተግበር.
- የእረፍት ጊዜ አያያዝን ያቁሙ።
- አግድ ራስጌ.
- ለጎራ አካላት አይኤስአይ ተዛማጅ ዘዴዎች።
- የኩራ ሁነታ መቁጠር እና የታመኑ አቻዎች ውቅር።
- የሰነድ ሽፋን ደንብ.
- የተፈፀመ እገዳን ያክሉ።
- ኩራ ከ `sumeragi` መፍታት።
- ከመፈጠሩ በፊት ግብይቶች ባዶ እንዳልሆኑ ያረጋግጡ።
- የ I18NT0000092X ልዩ መመሪያዎችን እንደገና ይተግብሩ።
- የግብይቶች መለኪያዎች እና ሽግግሮችን ያግዳል።
- የግብይቶች የሕይወት ዑደት እና ግዛቶች እንደገና ተሠርተዋል።
- የህይወት ዑደትን እና ግዛቶችን ያግዳል.
- የማረጋገጫ ስህተትን አስተካክል፣ I18NI0000634X loop ዑደት ከብሎክ_build_time_ms የውቅር ግቤት ጋር ተመሳስሏል።
- የ I18NT0000015X አልጎሪዝም በ `sumeragi` ሞጁል ውስጥ መካተት።
- የማሾፍ ሞጁል ለ I18NT0000093X የአውታረ መረብ crate በሰርጦች ይተገበራል።
- ወደ async-std ኤፒአይ ፍልሰት።
- የአውታረ መረብ መሳለቂያ ባህሪ።
- ያልተመሳሰለ ተዛማጅ ኮድ ማፅዳት።
- በግብይት ሂደት ዑደት ውስጥ የአፈጻጸም ማሻሻያዎች።
- የቁልፍ ጥንዶች ማመንጨት ከ Iroha ጅምር ወጥቷል።
- Docker የ I18NT0000095X ማሸጊያ።- Sumeragi መሰረታዊ ሁኔታን አስተዋውቅ።
- Iroha CLI ደንበኛ።
- የቤንች ቡድን ከተገደለ በኋላ የኢሮሃ ጣል.
- `sumeragi` አዋህድ።
- የ`sort_peers` አተገባበርን ወደ ራንድ ውዝዋዜ በቀደመው ብሎክ ሃሽ ይለውጡ።
- በአቻ ሞጁል ውስጥ የመልእክት መጠቅለያውን ያስወግዱ።
- ከአውታረ መረብ ጋር የተገናኘ መረጃን በ`torii::uri` እና I18NI0000639X ውስጥ ይዝጉ።
- ከሃርድ ኮድ አያያዝ ይልቅ የተተገበረ የአቻ መመሪያን ያክሉ።
- የእኩዮች ግንኙነት በታመኑ እኩዮች ዝርዝር።
- በ Torii ውስጥ የአውታረ መረብ ጥያቄዎች አያያዝን ያጠቃልላል።
- በ crypto ሞጁል ውስጥ የ crypto አመክንዮ ማጠቃለል።
- የጊዜ ማህተም እና የቀደመ የብሎክ ሃሽ እንደ የክፍያ ጭነት ያለው ምልክት አግድ።
- በሞጁሉ አናት ላይ የተቀመጡ የ Crypto ተግባራት እና ከዩርሳ ፈራሚ ጋር በፊርማ ውስጥ ተካትተዋል።
- Sumeragi የመጀመሪያ.
- ለማከማቸት ቃል ከመግባቱ በፊት በዓለም ግዛት እይታ ክሎይን ላይ የግብይት መመሪያዎችን ማረጋገጥ።
- የግብይት ተቀባይነት ላይ ፊርማዎችን ያረጋግጡ።
- በጥያቄ ማሰናከል ውስጥ ስህተትን ያስተካክሉ።
- የ I18NT0000097X ፊርማ መተግበር።
- Codebase ለማጽዳት Blockchain ህጋዊ አካል ተወግዷል።
- በግብይቶች ኤፒአይ ላይ የተደረጉ ለውጦች፡ የተሻለ መፍጠር እና ከጥያቄዎች ጋር መስራት።
- ብሎኮችን በባዶ የግብይት ቬክተር ሊፈጥር የሚችለውን ስህተት ያስተካክሉ
- ወደፊት በመጠባበቅ ላይ ያሉ ግብይቶች.
 - በ u128 Norito የተመሰጠረ TCP ፓኬት ከጎደለ ባይት ጋር ስህተትን ያስተካክሉ።
- ዘዴዎችን ለመፈለግ ማክሮዎችን ይግለጹ።
- P2p ሞጁል.
- በቶሪ እና ደንበኛ ውስጥ የiroha_አውታረ መረብ አጠቃቀም።
- አዲስ የ ISI መረጃ ያክሉ።
- ለአውታረ መረብ ሁኔታ ልዩ ቅጽል ስም።
- ቦክስ<dyn ስህተት> በ String ተተካ።
- የአውታረ መረብ ማዳመጥ ሁኔታዊ።
- ለግብይቶች የመጀመሪያ ማረጋገጫ አመክንዮ።
- Iroha_network crate.
- ማክሮን ለ Io ፣ IntoContract እና IntoQuery ባህሪዎችን ያግኙ።
- ለI18NT0000098X-ደንበኛ የጥያቄዎች ትግበራ።
- ትዕዛዞችን ወደ ISI ኮንትራቶች መለወጥ.
- ለሁኔታዊ ብዝሃ-ሲግ የታቀደ ንድፍ ያክሉ።
- ወደ ጭነት የሥራ ቦታዎች ስደት.
- ሞጁሎች ፍልሰት.
- የአካባቢ ተለዋዋጮች በኩል ውጫዊ ውቅር.
- ለTorii የጥያቄዎችን አያያዝ ያግኙ እና ያስቀምጡ።
- Github ci እርማት.
- ጭነት-ሠራሽ ከፈተና በኋላ ብሎኮችን ያጸዳል።
- ማውጫን ከብሎኮች የማጽዳት ተግባር ጋር `test_helper_fns` ሞጁሉን ያስተዋውቁ።
- በ merkle ዛፍ በኩል ማረጋገጫን ተግባራዊ ያድርጉ።
- ጥቅም ላይ ያልዋሉ ምርቶችን ያስወግዱ.
- ማመሳሰልን ያሰራጩ/ይጠብቁ እና ያልጠበቁትን `wsv::put` ያስተካክሉ።
- ከ `futures` crate መቀላቀልን ይጠቀሙ።
- ትይዩ የመደብር አፈጻጸምን ተግብር፡ ወደ ዲስክ መጻፍ እና WSV ን ማዘመን በትይዩ እየሆኑ ነው።
- ለ(de) ተከታታይነት ከባለቤትነት ይልቅ ማጣቀሻዎችን ይጠቀሙ።
- ከፋይሎች ኮድ ማስወጣት.
- ursa::blake2 ይጠቀሙ።
- ስለ mod.rs በአስተዋጽኦ መመሪያ ውስጥ ደንብ።
- ሃሽ 32 ባይት።
- Blake2 hash.
- ዲስክ ለማገድ ማጣቀሻዎችን ይቀበላል.
- የትዕዛዝ ሞጁል እና የመጀመሪያ Merkle ዛፍን እንደገና ማደስ።
- የተስተካከሉ ሞጁሎች መዋቅር.
- ትክክለኛ ቅርጸት።
- ሁሉንም ለማንበብ የሰነድ አስተያየቶችን ያክሉ።
- `read_all`ን ይተግብሩ፣ የማከማቻ ሙከራዎችን እንደገና ያደራጁ እና ከአስመር ተግባራት ጋር ሙከራዎችን ወደ ተመሳሳይ ሙከራዎች ይለውጡ።
- አላስፈላጊ ቀረጻን ያስወግዱ።
- ችግርን ይገምግሙ ፣ ክሊፕን ያስተካክሉ።
- ሰረዝን ያስወግዱ.
- የቅርጸት ቼክ አክል.
- ማስመሰያ አክል.
- ለ github ድርጊቶች rust.yml ይፍጠሩ።
- የዲስክ ማከማቻ ፕሮቶታይፕን ያስተዋውቁ።
- የንብረት ሙከራ እና ተግባራዊነት ያስተላልፉ.
- ነባሪ ማስጀመሪያን ወደ መዋቅር ያክሉ።
- የMSTCache መዋቅር ስም ይቀይሩ።
- የተረሳ ብድር ጨምር።
- የiroha2 ኮድ የመጀመሪያ መግለጫ።
- የመጀመሪያ ኩራ ኤፒአይ።
- አንዳንድ መሰረታዊ ፋይሎችን ያክሉ እና እንዲሁም የiroha v2 ራዕይን የሚገልጽ የነጭ ወረቀት የመጀመሪያ ረቂቅ ይልቀቁ።
- መሰረታዊ የኢሮሃ v2 ቅርንጫፍ።

## [1.5.0] - 2022-04-08

### CI/ሲዲ ይቀየራል።
- Jenkinsfile እና JenkinsCIን ያስወግዱ።

### ታክሏል።- ለቡሮው የ RocksDB ማከማቻ አተገባበርን ያክሉ።
- የትራፊክ ማመቻቸትን በ Bloom-filter ያስተዋውቁ
- በ `batches_cache` ውስጥ በ `OS` ሞጁል ውስጥ እንዲገኝ የ `MST` ሞጁል ኔትወርክን ያዘምኑ።
- የትራፊክ ማመቻቸትን ይጠቁሙ.

### ሰነድ

- ግንባታን ያስተካክሉ። የዲቢ ልዩነቶችን፣ የስደት ልምምድን፣ የጤንነት ማረጋገጫ የመጨረሻ ነጥብን፣ የኢሮሃ-ስዋረም መሳሪያ መረጃን ያክሉ።

### ሌላ

- ለዶክ ግንባታ አስፈላጊ ማስተካከያ.
- ቀሪውን ወሳኝ የመከታተያ ንጥል ነገር ለማጉላት የመልቀቂያ ሰነዶችን ይከርክሙ።
- 'የዶክተር ምስል ካለ ያረጋግጡ' / ሁሉንም መዝለል_ሙከራ ይገንቡ።
-/ሁሉንም መዝለል_ሙከራ ይገንቡ።
- / የዝላይ ሙከራን ይገንቡ; እና ተጨማሪ ሰነዶች።
- `.github/_README.md` አክል.
- `.packer` አስወግድ.
- በሙከራ መለኪያ ላይ ለውጦችን ያስወግዱ.
- የሙከራ ደረጃን ለመዝለል አዲስ መለኪያ ይጠቀሙ።
- ወደ የስራ ፍሰት ይጨምሩ.
- የማከማቻ መላክን ያስወግዱ.
- የማጠራቀሚያ መላክን ያክሉ።
- ለሞካሪዎች መለኪያ ያክሉ።
- የ `proposal_delay` ጊዜ ያለፈበትን ያስወግዱ።

## [1.4.0] - 2022-01-31

### ታክሏል።

- የማመሳሰል መስቀለኛ መንገድ ሁኔታን ያክሉ
- ለ RocksDB መለኪያዎችን ይጨምራል
- በ http እና ሜትሪክስ በኩል የጤንነት ማረጋገጫ በይነገጾችን ያክሉ።

### ማስተካከያዎች

- የአምድ ቤተሰቦችን በ I18NT0000099X v1.4-rc.2 ያስተካክሉ
- በIroha v1.4-rc.1 ውስጥ ባለ 10-ቢት የአበባ ማጣሪያ አክል

### ሰነድ

- የግንባታ deps ዝርዝር ውስጥ ዚፕ እና pkg-config ያክሉ።
- readme ያዘምኑ፡ ሁኔታን ለመገንባት፣ መመሪያን ለመገንባት እና የመሳሰሉትን ለመስራት የተበላሹ አገናኞችን ያስተካክሉ።
- ኮንፊግ እና I18NT0000051X መለኪያዎችን አስተካክል።

### ሌላ

- የ GHA docker መለያን ያዘምኑ።
- Iroha 1 በ g ++11 ሲጠናቀር ስህተቶችን ያስተካክሉ።
- `max_rounds_delay` በ I18NI0000651X ተካ።
- የድሮ የዲቢ ግንኙነት መለኪያዎችን ለማስወገድ የናሙና ማዋቀሪያ ፋይልን ያዘምኑ።