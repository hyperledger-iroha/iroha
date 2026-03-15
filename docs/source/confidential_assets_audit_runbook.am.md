---
lang: am
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! በ`roadmap.md:M4` የተጠቀሰው ሚስጥራዊ ንብረቶች ኦዲት እና ኦፕሬሽኖች የመጫወቻ መጽሐፍ።

# ሚስጥራዊ ንብረቶች ኦዲት እና ኦፕሬሽኖች Runbook

ይህ መመሪያ ኦዲተሮች እና ኦፕሬተሮች የሚተማመኑባቸውን ማስረጃዎች ያጠናክራል።
ሚስጥራዊ-ንብረት ፍሰቶችን ሲያረጋግጥ. የማዞሪያ መጫወቻ ደብተርን ያሟላል
(`docs/source/confidential_assets_rotation.md`) እና የካሊብሬሽን መዝገብ
(`docs/source/confidential_assets_calibration.md`)።

## 1. የተመረጡ ይፋ መግለጫ እና የክስተት ምግቦች

- እያንዳንዱ ሚስጥራዊ መመሪያ የተዋቀረ `ConfidentialEvent` ጭነት ያስወጣል።
  (`Shielded`፣ `Transferred`፣ `Unshielded`) ተይዟል
  `crates/iroha_data_model/src/events/data/events.rs:198` እና ተከታታይ በ
  አስፈፃሚዎች (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`-`4021`)።
  የሪግሬሽን ስብስብ የኮንክሪት ሸክሞችን ስለሚለማመድ ኦዲተሮች ሊተማመኑበት ይችላሉ።
  የሚወስኑ JSON አቀማመጦች (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`)።
- Torii እነዚህን ክስተቶች በመደበኛ SSE/WebSocket ቧንቧ በኩል ያጋልጣል; ኦዲተሮች
  `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`) በመጠቀም ይመዝገቡ
  እንደ አማራጭ ወደ ነጠላ የንብረት ፍቺ መፈተሽ። CLI ምሳሌ፡-

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- የፖሊሲ ዲበ ውሂብ እና በመጠባበቅ ላይ ያሉ ሽግግሮች ይገኛሉ
  `GET /v2/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`)፣ በስዊፍት ኤስዲኬ የተንጸባረቀ
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) እና በ ውስጥ ተመዝግቧል
  ሁለቱም ሚስጥራዊ-ንብረት ንድፍ እና የኤስዲኬ መመሪያዎች
  (`docs/source/confidential_assets.md:70`፣ `docs/source/sdk/swift/index.md:334`)።

## 2. ቴሌሜትሪ፣ ዳሽቦርድ እና የካሊብሬሽን ማስረጃ

- የሩጫ ጊዜ መለኪያዎች የወለል ዛፍ ጥልቀት፣ ቁርጠኝነት/የድንበር ታሪክ፣ ስርወ ማስወጣት
  ቆጣሪዎች፣ እና አረጋጋጭ-መሸጎጫ መምታት ሬሾዎች
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`)። Grafana ዳሽቦርዶች በ
  `dashboards/grafana/confidential_assets.json` ተያያዥ ፓነሎችን እና
  ማንቂያዎች፣ በ`docs/source/confidential_assets.md:401` ውስጥ ከተመዘገበው የስራ ፍሰት ጋር።
- የካሊብሬሽን ስራዎች (NS/op፣ gas/op፣ ns/gas) ከተፈረሙ ምዝግብ ማስታወሻዎች ጋር በቀጥታ ወደ ውስጥ
  `docs/source/confidential_assets_calibration.md`. የቅርብ ጊዜ አፕል ሲሊኮን
  NEON ሩጫ በ ላይ ተቀምጧል
  `docs/source/confidential_assets_calibration_neon_20260428.log`, እና ተመሳሳይ
  ደብተር እስከ ሲምዲ-ገለልተኛ እና AVX2 መገለጫዎች ጊዜያዊ መልቀቂያዎችን ይመዘግባል
  የ x86 አስተናጋጆች መስመር ላይ ይመጣሉ።

## 3. የክስተት ምላሽ እና ኦፕሬተር ተግባራት

- የማሽከርከር/የማሻሻል ሂደቶች ይኖራሉ
  `docs/source/confidential_assets_rotation.md`፣ እንዴት አዲስ ደረጃ ማድረግ እንደሚቻል ይሸፍናል።
  የመለኪያ ቅርቅቦች፣ የፖሊሲ ማሻሻያዎችን መርሐግብር እና የኪስ ቦርሳ/ኦዲተሮችን አሳውቅ። የ
  መከታተያ (`docs/source/project_tracker/confidential_assets_phase_c.md`) ዝርዝሮች
  runbook ባለቤቶች እና የመለማመጃ የሚጠበቁ.
- ለምርት ልምምዶች ወይም የድንገተኛ መስኮቶች ኦፕሬተሮች ማስረጃዎችን ያያይዙ
  `status.md` ግቤቶች (ለምሳሌ፣ ባለብዙ መስመር መለማመጃ ምዝግብ ማስታወሻ) እና የሚከተሉትን ያካትታሉ፡
  `curl` የመመሪያ ሽግግሮች ማረጋገጫ፣ Grafana ቅጽበታዊ ገጽ እይታዎች እና ተዛማጅ ክስተት
  ኦዲተሮች ሚንት → ማስተላለፍ →የጊዜ መስመሮችን መግለጥ እንዲችሉ ይዋሃዳል።

## 4. የውጭ ግምገማ Cadence

- የደህንነት ግምገማ ወሰን: ሚስጥራዊ ወረዳዎች, የመለኪያ መዝገብ ቤቶች, ፖሊሲ
  ሽግግሮች, እና ቴሌሜትሪ. ይህ ሰነድ እና የካሊብሬሽን ደብተር ቅጾች
  ለሻጮች የተላከው የማስረጃ ፓኬት; የግምገማ መርሐግብር ክትትል የሚደረገው በ በኩል ነው።
  M4 በ `docs/source/project_tracker/confidential_assets_phase_c.md`.
- ኦፕሬተሮች በማንኛውም የአቅራቢ ግኝቶች ወይም ክትትል `status.md` ማዘመን አለባቸው
  የድርጊት እቃዎች. የውጭ ግምገማው እስኪጠናቀቅ ድረስ፣ ይህ runbook የ
  የክዋኔ ቤዝላይን ኦዲተሮች ሊፈትኑ ይችላሉ።