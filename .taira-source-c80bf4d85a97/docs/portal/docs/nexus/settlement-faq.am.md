---
lang: am
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e992cea8d0c835b30bd9e91860f6b6f87bed79a2c25bd6d0544639685834f80c
source_last_modified: "2025-12-29T18:16:35.146583+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-settlement-faq
title: Settlement FAQ
description: Operator-facing answers covering settlement routing, XOR conversion, telemetry, and audit evidence.
translator: machine-google-reviewed
---

ይህ ገጽ ውስጣዊ የሰፈራ FAQ (`docs/source/nexus_settlement_faq.md`) ያንጸባርቃል
ስለዚህ ፖርታል አንባቢዎች ሳይቆፍሩ ተመሳሳይ መመሪያን መከለስ ይችላሉ።
ሞኖ-ሪፖ. የማቋቋሚያ ራውተር ክፍያዎችን እንዴት እንደሚያስኬድ፣ ምን አይነት መለኪያዎችን ያብራራል።
ለመከታተል፣ እና ኤስዲኬዎች የNorito ክፍያ ጭነቶችን እንዴት እንደሚያዋህዱ።

## ድምቀቶች

1. ** ሌይን ካርታ *** - እያንዳንዱ የውሂብ ቦታ `settlement_handle` ያውጃል
   (`xor_global`፣ `xor_lane_weighted`፣ `xor_hosted_custody`፣ ወይም
   I18NI0000009X). የቅርብ ጊዜውን የሌይን ካታሎግ በስር ይመልከቱ
   `docs/source/project_tracker/nexus_config_deltas/`.
2. ** ቆራጥ ልወጣ *** — ራውተር ሁሉንም ሰፈሮች በ XOR ይለውጣል
   በአስተዳደር የተፈቀዱ የፈሳሽ ምንጮች. የግል መስመሮች ቅድመ ፈንድ XOR ማቋቋሚያዎች;
   የፀጉር መቆረጥ የሚሠራው መከላከያዎች ከፖሊሲ ውጭ ሲንሸራተቱ ብቻ ነው።
3. ** ቴሌሜትሪ *** - `nexus_settlement_latency_seconds` ይመልከቱ ፣ የመቀየሪያ ቆጣሪዎች ፣
   እና የፀጉር መቁረጫዎች መለኪያዎች. ዳሽቦርዶች በI18NI0000012X ይኖራሉ
   እና ማንቂያዎች በ I18NI0000013X.
4. ** ማስረጃ *** - የማህደር ውቅሮች፣ ራውተር ምዝግብ ማስታወሻዎች፣ ቴሌሜትሪ ወደ ውጭ መላክ እና
   የማስታረቅ ሪፖርቶች ለኦዲት.
5. **የኤስዲኬ ኃላፊነቶች** — እያንዳንዱ ኤስዲኬ የሰፈራ ረዳቶችን፣ የሌይን መታወቂያዎችን፣
   እና Norito ከራውተሩ ጋር ያለውን እኩልነት ለመጠበቅ የመክፈያ ማመሳከሪያዎች።

## ምሳሌ ይፈስሳል

| የሌይን አይነት | ማስረጃ ለመያዝ | ምን ያረጋግጣል |
|---------------
| የግል `xor_hosted_custody` | የራውተር መዝገብ + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | የሲቢሲሲ ማቋቋሚያ ዴቢት መወሰኛ XOR እና የፀጉር ማቆሚያዎች በፖሊሲ ውስጥ ይቆያሉ። |
| የህዝብ `xor_global` | የራውተር መዝገብ + DEX/TWAP ማጣቀሻ + መዘግየት/የልወጣ መለኪያዎች | የተጋራ ፈሳሽ መንገድ ዝውውሩን በታተመው TWAP በዜሮ ፀጉር ዋጋ አስከፍሏል። |
| ዲቃላ `xor_dual_fund` | የራውተር ሎግ ይፋዊ vs የተከለለ ስንጥቅ + የቴሌሜትሪ ቆጣሪዎች | የተከለለ/የህዝብ ቅይጥ የተከበረ የአስተዳደር ምጥጥን እና በእያንዳንዱ እግር ላይ የተተገበረውን የፀጉር አሠራር መዝግቧል። |

## ተጨማሪ ዝርዝር ይፈልጋሉ?

- ሙሉ ተደጋጋሚ ጥያቄዎች፡ I18NI0000019X
- የሰፈራ ራውተር ዝርዝር: `docs/source/settlement_router.md`
- CBDC ፖሊሲ ጨዋታ መጽሐፍ: I18NI0000021X
- የክወናዎች runbook: [Nexus ክወናዎች](./nexus-operations)