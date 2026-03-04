---
id: nexus-overview
lang: am
direction: ltr
source: docs/portal/docs/nexus/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Nexus overview
description: High-level summary of the Iroha 3 (Sora Nexus) architecture with pointers to the canonical mono-repo docs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Nexus (Iroha 3) Iroha 2ን በባለብዙ ሌይን አፈጻጸም፣ በአስተዳደር-ወሰን
የውሂብ ቦታዎች፣ እና በሁሉም ኤስዲኬ ላይ የጋራ መገልገያ። ይህ ገጽ አዲሱን ያንፀባርቃል
`docs/source/nexus_overview.md` አጭር በ mono-repo ስለዚህ ፖርታል አንባቢዎች ይችላሉ።
የሕንፃው ክፍሎች እንዴት እንደሚጣመሩ በፍጥነት ይረዱ።

## የመልቀቂያ መስመሮች

- **Iroha 2** - ለኮንሰርቲየም ወይም ለግል አውታረ መረቦች በራስ የሚስተናገዱ ማሰማራት።
- **Iroha 3 / Sora Nexus** - ኦፕሬተሮች ያሉበት ባለብዙ መስመር የህዝብ አውታረ መረብ
  የውሂብ ቦታዎችን (ዲኤስ) መመዝገብ እና የጋራ አስተዳደርን, አሰፋፈርን እና
  ታዛቢነት መሳሪያ.
- ሁለቱም መስመሮች ከተመሳሳይ የስራ ቦታ (IVM + Kotodama Toolchain) ያጠናቅራሉ, ስለዚህ ኤስዲኬ
  ጥገናዎች፣ የ ABI ዝማኔዎች እና Norito ቋሚዎች ተንቀሳቃሽ ሆነው ይቆያሉ። ኦፕሬተሮች ማውረድ
  የ `iroha3-<version>-<os>.tar.zst` ጥቅል I18NT0000008X ለመቀላቀል; ተመልከት
  `docs/source/sora_nexus_operator_onboarding.md` ለሙሉ ስክሪን ማረጋገጫ ዝርዝር።

## የግንባታ ብሎኮች

| አካል | ማጠቃለያ | ፖርታል መንጠቆዎች |
|--------|-----|-------------|
| የውሂብ ክፍተት (DS) | በመንግስት የተገለፀ የማስፈጸሚያ/የማከማቻ ጎራ የአንድ ወይም ከዚያ በላይ መስመሮች ባለቤት፣አረጋጋጭ ስብስቦችን፣የግላዊነት ክፍልን፣ክፍያ +DA ፖሊሲን ያውጃል። | ለአንጸባራቂ ዕቅዱ [Nexus spec](./nexus-spec) ይመልከቱ። |
| መስመር | የአፈፃፀም ቆራጥ ቁርጥራጭ; የአለምአቀፍ NPoS ቀለበት ያዘዘውን ቃል ኪዳን ያወጣል። የሌይን ክፍሎች I18NI0000031X፣ `public_custom`፣ `private_permissioned`፣ እና `hybrid_confidential` ያካትታሉ። | [ሌይን ሞዴል](I18NU0000017X) ጂኦሜትሪ፣ የማከማቻ ቅድመ ቅጥያዎችን እና ማቆየትን ይይዛል። |
| የሽግግር እቅድ | የቦታ ያዥ ለዪዎች፣ የማዞሪያ ደረጃዎች እና ባለሁለት መገለጫ እሽግ እንዴት ባለ አንድ መስመር ዝርጋታ ወደ Nexus እንደሚቀየር ይከታተላሉ። | [የሽግግር ማስታወሻዎች](./nexus-transition-notes) እያንዳንዱን የፍልሰት ደረጃ ይመዝግቡ። |
| የጠፈር ማውጫ | ዲኤስ መግለጫዎችን + ስሪቶችን የሚያከማች የመመዝገቢያ ውል። ኦፕሬተሮች ከመቀላቀላቸው በፊት የካታሎግ ግቤቶችን ከዚህ ማውጫ ጋር ያስታርቃሉ። | አንጸባራቂ ልዩነት መከታተያ በ`docs/source/project_tracker/nexus_config_deltas/` ስር ይኖራል። |
| ሌይን ካታሎግ | የአይ18NI00000036ኤክስ ማዋቀር ክፍል መታወቂያዎችን ወደ ተለዋጭ ስሞች፣የማዞሪያ መመሪያዎች እና የዲኤ ገደቦችን የሚያዘጋጅ። `irohad --sora --config … --trace-config` ለኦዲት የተፈታ ካታሎግ ያትማል። | ለCLI የእግር ጉዞ `docs/source/sora_nexus_operator_onboarding.md` ይጠቀሙ። |
| የሰፈራ ራውተር | የግል CBDC መስመሮችን ከሕዝብ ፈሳሽነት መስመሮች ጋር የሚያገናኝ የXOR ማስተላለፊያ ኦርኬስትራ። | `docs/source/cbdc_lane_playbook.md` የፖሊሲ ቁልፎችን እና የቴሌሜትሪ በሮች ይገልፃል። |
| ቴሌሜትሪ/SLO | ዳሽቦርዶች + ማንቂያዎች በI18NI0000040X ቀረጻ ሌይን ቁመት፣ DA backlog፣ የሰፈራ መዘግየት እና የአስተዳደር ወረፋ ጥልቀት። | [የቴሌሜትሪ ማሻሻያ ዕቅድ](./nexus-telemetry-remediation) ዳሽቦርዶችን፣ ማንቂያዎችን እና የኦዲት ማስረጃዎችን ይገልፃል። |

## የልቀት ቅጽበታዊ ገጽ እይታ

| ደረጃ | ትኩረት | መውጫ መስፈርት |
|-------|-------|----------|
| N0 – የተዘጋ ቤታ | በካውንስል የሚተዳደር ሬጅስትራር (`.sora`)፣ በእጅ የሚሰራ ኦፕሬተር፣ የማይንቀሳቀስ ሌይን ካታሎግ። | የተፈረመ DS ይገለጣል + የተለማመዱ የአስተዳደር እጃዎች። |
| N1 - የህዝብ ማስጀመሪያ | የ `.nexus` ቅጥያዎችን፣ ጨረታዎችን፣ የራስ አገልግሎት ሬጅስትራርን፣ XOR የሰፈራ ሽቦን ይጨምራል። | የመፍትሄ/የበረንዳ ማመሳሰል ሙከራዎች፣የሂሳብ አከፋፈል ማስታረቅ ዳሽቦርዶች፣ሙግት የጠረጴዛ ልምምዶች። |
| N2 - ማስፋፊያ | `.dao`፣ ሻጭ ኤፒአይዎችን፣ ትንታኔዎችን፣ የሙግት ፖርታልን፣ የመጋቢ የውጤት ካርዶችን ያስተዋውቃል። | ተገዢ የሆኑ ቅርሶች የተስተካከሉ፣ የፖሊሲ-ዳኞች መሣሪያ መስመር ላይ፣ የግምጃ ቤት ግልጽነት ሪፖርቶች። |
| NX-12/13/14 በር | ተገዢነት ሞተር፣ የቴሌሜትሪ ዳሽቦርዶች እና ሰነዶች ከአጋር አብራሪዎች በፊት አብረው መላክ አለባቸው። | [Nexus አጠቃላይ እይታ](I18NU0000020X) + [Nexus ኦፕሬሽኖች](./nexus-operations) ታትሟል፣ ዳሽቦርዶች በሽቦ፣ የፖሊሲ ሞተር ተዋህዷል። |

## የኦፕሬተር ሀላፊነቶች

1. ** ንፅህናን ማዋቀር *** - `config/config.toml` ከታተመው ሌይን ጋር ማመሳሰል
   የውሂብ ቦታ ካታሎግ; በእያንዳንዱ የመልቀቂያ ትኬት `--trace-config` ውፅዓት ያስቀምጡ።
2. ** መከታተያ ይግለጹ *** - የካታሎግ ግቤቶችን ከቅርብ ጊዜው ቦታ ጋር ያስታርቁ
   አንጓዎችን ከመቀላቀል ወይም ከማሻሻል በፊት የማውጫ ቅርቅብ።
3. ** የቴሌሜትሪ ሽፋን *** - `nexus_lanes.json`፣ `nexus_settlement.json`፣
   እና ተዛማጅ የኤስዲኬ ዳሽቦርዶች; የሽቦ ማንቂያዎችን ለፔጀርዱቲ እና በየሩብ ዓመቱ ግምገማዎችን በቴሌሜትሪ ማሻሻያ ዕቅድ ያሂዱ።
4. **የአጋጣሚ ነገር ሪፖርት ማድረግ** - የክብደቱን ማትሪክስ በ ውስጥ ይከተሉ
   [Nexus ክወናዎች](I18NU0000022X) እና RCAs በአምስት የስራ ቀናት ውስጥ ያስገቡ።
5. **የመንግስት ዝግጁነት** - በ Nexus የምክር ቤት ድምጽ በመስመሮችዎ ላይ ተጽእኖ ያሳድራሉ
   የመመለሻ መመሪያዎችን በየሩብ ዓመቱ ይለማመዱ (በ በኩል ክትትል የሚደረግበት
   `docs/source/project_tracker/nexus_config_deltas/`).

## ይመልከቱ

- ቀኖናዊ አጠቃላይ እይታ: `docs/source/nexus_overview.md`
- ዝርዝር መግለጫ፡ [./nexus-spec](I18NU0000023X)
- ሌይን ጂኦሜትሪ፡ [./nexus-lane-model](./nexus-lane-model)
- የሽግግር እቅድ፡ [./nexus-transition-notes](./nexus-transition-notes)
- የቴሌሜትሪ ማስተካከያ እቅድ፡ [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- ኦፕሬሽኖች runbook፡ [./nexus-operations](./nexus-operations)
- ኦፕሬተር የመሳፈሪያ መመሪያ: `docs/source/sora_nexus_operator_onboarding.md`