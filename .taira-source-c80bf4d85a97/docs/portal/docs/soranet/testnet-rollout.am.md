---
lang: am
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 887123dcafff50fb243d9788415b759da3691876e44b3cd7c800eede25a5ab09
source_last_modified: "2026-01-05T09:28:11.916414+00:00"
translation_last_reviewed: 2026-02-07
id: testnet-rollout
title: SoraNet testnet rollout (SNNet-10)
sidebar_label: Testnet Rollout (SNNet-10)
description: Phased activation plan, onboarding kit, and telemetry gates for SoraNet testnet promotions.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

SNNet-10 በአውታረ መረቡ ላይ ያለውን የሶራኔት ስም-አልባ መደራረብን ያስተባብራል። SoraNet ነባሪው መጓጓዣ ከመሆኑ በፊት እያንዳንዱ ኦፕሬተር የሚጠበቀውን ነገር እንዲረዳ የፍኖተ ካርታውን ጥይት ወደ ኮንክሪት ማጓጓዣዎች፣ runbooks እና telemetry በሮች ለመተርጎም ይህን እቅድ ይጠቀሙ።

## ደረጃዎችን ማስጀመር

| ደረጃ | የጊዜ መስመር (ዒላማ) | ወሰን | አስፈላጊ ቅርሶች |
|--------|----
| ** T0 - ተዘግቷል Testnet ** | Q4 2026 | 20–50 ሬሌሎች በ≥3 ASNs በዋና አስተዋጽዖ አበርካቾች የሚንቀሳቀሱ። | ቴስትኔት የቦርዲንግ ኪት፣ የጥበቃ መሰኪያ ጭስ ስብስብ፣ የመነሻ መዘግየት + የፖው ሜትሪክስ፣ ቡናማ መውጫ መሰርሰሪያ ሎግ። |
| ** T1 - ይፋዊ ቤታ *** | Q1 2027 | ≥100 ሬሌሎች፣ የጥበቃ ማሽከርከር ነቅቷል፣ ከግንኙነት መውጣት ተፈጻሚ ሆኗል፣ የኤስዲኬ ቤታስ ነባሪ ወደ SoraNet ከ`anon-guard-pq` ጋር። | የተዘመነ የመሳፈሪያ ኪት፣ ከዋኝ የማረጋገጫ ዝርዝር፣ የማውጫ ህትመት SOP፣ የቴሌሜትሪ ዳሽቦርድ ጥቅል፣ የክስተቶች ልምምድ ሪፖርቶች። |
| ** T2 - የሜይንኔት ነባሪ *** | Q2 2027 (በ SNNet-6/7/9 ማጠናቀቅ ላይ የተከለለ) | የምርት አውታር ነባሪ ወደ SoraNet; obfs/MASQUE መጓጓዣዎች እና PQ ratchet ማስፈጸም ነቅቷል። | የአስተዳደር ማጽደቂያ ደቂቃዎች፣ ቀጥታ ብቻ የመልሶ ማቋቋሚያ ሂደት፣ የማንቂያ ደወሎችን ዝቅ ማድረግ፣ የተፈረመ የስኬት መለኪያዎች ሪፖርት። |

** ምንም የመዝለል መንገድ የለም *** - እያንዳንዱ ደረጃ ከማስተዋወቅ በፊት የቴሌሜትሪ እና የአስተዳደር ቅርሶችን ከቀዳሚው ደረጃ መላክ አለበት።

## Testnet የመሳፈሪያ መሣሪያ

እያንዳንዱ የዝውውር ኦፕሬተር ከሚከተሉት ፋይሎች ጋር የሚወስን ጥቅል ይቀበላል።

| Artefact | መግለጫ |
|-------|-----------|
| I18NI0000002X | አጠቃላይ እይታ፣ የእውቂያ ነጥቦች እና የጊዜ መስመር። |
| `02-checklist.md` | የቅድመ በረራ ማረጋገጫ ዝርዝር (ሃርድዌር፣ የአውታረ መረብ ተደራሽነት፣ የጥበቃ ፖሊሲ ማረጋገጫ)። |
| `03-config-example.toml` | አነስተኛው የሶራኔት ማስተላለፊያ + ኦርኬስትራ ውቅር ከSNNet-9 ተገዢነት ብሎኮች ጋር የተስተካከለ፣የ `guard_directory` ብሎክን ጨምሮ የቅርብ ጊዜ የጥበቃ ቅጽበተ-ፎቶ ሃሽ የሚሰካ። |
| `04-telemetry.md` | የ SoraNet ግላዊነት መለኪያዎች ዳሽቦርዶች እና የማንቂያ ገደቦችን ለማገናኘት መመሪያዎች። |
| `05-incident-playbook.md` | ብራውን መውጣት/ማውረድ የምላሽ ሂደት ከእድገት ማትሪክስ ጋር። |
| `06-verification-report.md` | የአብነት ኦፕሬተሮች የጭስ ፈተና ካለፉ በኋላ ይመለሳሉ። |

አንድ ቅጂ በI18NI0000009X ውስጥ ይኖራል። እያንዳንዱ ማስተዋወቂያ ኪቱን ያድሳል; የስሪት ቁጥሮች ደረጃውን ይከታተላሉ (ለምሳሌ `testnet-kit-vT0.1`)።

ለህዝብ-ቤታ (T1) ኦፕሬተሮች፣ በ`docs/source/soranet/snnet10_beta_onboarding.md` ውስጥ ያለው አጭር የመሳፈሪያ አጭር መግለጫ ቅድመ ሁኔታዎችን፣ የቴሌሜትሪ ማቅረቢያዎችን እና የማስረከቢያ የስራ ሂደትን ወደ ወሳኙ ኪት እና አረጋጋጭ ረዳቶች በማሳየት ያጠቃልላል።

`cargo xtask soranet-testnet-feed` የማስተዋወቂያ መስኮቱን፣ የማስተላለፊያ ዝርዝሩን፣ የሜትሪክ ዘገባን፣ የመሰርሰሪያ ማስረጃን እና በመድረክ-ጌት አብነት የተጠቀሰውን አባሪ ሃሽ የሚያጠቃልለውን የJSON ምግብ ያመነጫል። ምግቡ `drill_log.signed = true` መመዝገብ እንዲችል በመጀመሪያ ከ`cargo xtask soranet-testnet-drill-bundle` ጋር የመሰርሰሪያ ምዝግብ ማስታወሻዎችን እና አባሪዎችን ይፈርሙ።

## የስኬት መለኪያዎች

በደረጃዎች መካከል ማስተዋወቅ በሚከተለው ቴሌሜትሪ ላይ ተዘግቷል ፣ ቢያንስ ለሁለት ሳምንታት ተሰብስቧል።

- `soranet_privacy_circuit_events_total`: 95% ወረዳዎች ያለ ቡኒ ወይም ዝቅ ያለ ክስተቶች የተጠናቀቁ ናቸው; ቀሪው 5% በPQ አቅርቦት ተሸፍኗል።
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`፡ <1% የማምጣት ክፍለ-ጊዜዎች በቀን ከታቀዱ ልምምዶች ውጭ ቡኒ መውጣትን ይቀሰቅሳሉ።
- `soranet_privacy_gar_reports_total`: ከተጠበቀው የ GAR ምድብ ድብልቅ በ ± 10% ውስጥ ልዩነት; ስፒሎች በፀደቁ የፖሊሲ ማሻሻያዎች መገለጽ አለባቸው።
- የፖው ቲኬት ስኬት መጠን: ≥99% በ 3 ዎቹ የዒላማ መስኮት ውስጥ; በ `soranet_privacy_throttles_total{scope="congestion"}` በኩል ሪፖርት ተደርጓል.
- መዘግየት (95ኛ ፐርሰንታይል) በየክልሉ፡<200ሚሴ አንዴ ወረዳዎች ሙሉ በሙሉ ከተገነቡ፣በ`soranet_privacy_rtt_millis{percentile="p95"}` ይያዛሉ።

ዳሽቦርድ እና ማንቂያ አብነቶች በ `dashboard_templates/` እና `alert_templates/`; ወደ ቴሌሜትሪ ማከማቻዎ ያንጸባርቁ እና ወደ CI lint checks ያክሏቸው። ማስተዋወቅ ከመጠየቅዎ በፊት የአስተዳደር ተኮር ሪፖርት ለማመንጨት `cargo xtask soranet-testnet-metrics` ይጠቀሙ።

የመድረክ-በር ማስረከቦች `docs/source/soranet/snnet10_stage_gate_template.md` መከተል አለባቸው፣ ይህም ለመቅዳት ዝግጁ የሆነውን የማርክዳውን ቅጽ በ`docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md` ይገናኛል።

## የማረጋገጫ ዝርዝር

ወደ እያንዳንዱ ደረጃ ከመግባትዎ በፊት ኦፕሬተሮች የሚከተሉትን መፈረም አለባቸው።

- ✅ የማስተላለፊያ ማስታወቂያ ከአሁኑ የመግቢያ ኤንቨሎፕ ጋር ተፈርሟል።
- ✅ የጥበቃ ሽክርክር ጭስ ፈተና (`tools/soranet-relay --check-rotation`) ያልፋል።
- ✅ I18NI0000026X ነጥብ በመጨረሻው `GuardDirectorySnapshotV2` artefact እና `expected_directory_hash_hex` ከኮሚቴው አፈጣጠር ጋር ይዛመዳል (ቅብብል ማስጀመሪያ የተረጋገጠውን ሃሽ ይመዘግባል)።
- ✅ PQ ratchet metrics (`sorafs_orchestrator_pq_ratio`) ለተጠየቀው ደረጃ ከታቀደው ገደብ በላይ ይቆያሉ።
- GAR compliance config ከቅርቡ መለያ ጋር ይዛመዳል (SNNet-9 ካታሎግ ይመልከቱ)።
- ✅ የደወል ማስመሰልን ዝቅ ያድርጉ ( ሰብሳቢዎችን ያሰናክሉ፣ በ5 ደቂቃ ውስጥ ማንቂያ ይጠብቁ)።
- ✅ የPoW/DoS ቁፋሮ በሰነድ የቅነሳ እርምጃዎች ተፈፅሟል።

አስቀድሞ የተሞላ አብነት በቦርዲንግ ኪት ውስጥ ተካትቷል። ኦፕሬተሮች የምርት ማስረጃዎችን ከማግኘታቸው በፊት የተጠናቀቀውን ሪፖርት ለአስተዳደር ዴስክ ያቀርባሉ።

##አስተዳደር እና ሪፖርት ማድረግ

- **የለውጥ ቁጥጥር፡** ማስተዋወቂያዎች የምክር ቤቱ ቃለ ጉባኤ ውስጥ ተመዝግበው ከሁኔታ ገጽ ጋር ተያይዞ የአስተዳደር ምክር ቤት ይሁንታ ያስፈልጋቸዋል።
- **የሁኔታ መፍጨት፡** የዝውውር ብዛትን፣ የPQ ምጥጥንን፣ ቡናማ መውጣትን እና አስደናቂ የድርጊት እቃዎችን የሚያካትት ሳምንታዊ ዝማኔዎችን ያትሙ (በ I18NI000000030X ውስጥ አንዴ ክዳኔው ከጀመረ)።
- ** የድጋሚ መልሶ ማግኘቶች፡** ዲ ኤን ኤስ/ የጥበቃ መሸጎጫ መሸጎጫ እና የደንበኛ ግንኙነት አብነቶችን ጨምሮ በ30 ደቂቃ ውስጥ አውታረ መረቡን ወደ ቀድሞው ምዕራፍ የሚመልስ የተፈረመ የመመለሻ እቅድ ያዙ።

## የሚደግፉ ንብረቶች

- `cargo xtask soranet-testnet-kit [--out <dir>]` የመሳፈሪያ ኪቱን ከ`xtask/templates/soranet_testnet/` ወደ ዒላማው ማውጫ (የ `docs/examples/soranet_testnet_operator_kit/` ነባሪዎች) እውን ያደርጋል።
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` የ SNNet-10 የስኬት መለኪያዎችን ይገመግማል እና ለአስተዳደር ግምገማዎች ተስማሚ የሆነ የተቀናጀ ማለፊያ/ ውድቀት ሪፖርት ያወጣል። የናሙና ቅጽበታዊ ገጽ እይታ በI18NI0000035X ውስጥ ይኖራል።
- Grafana እና Alertmanager አብነቶች በ `dashboard_templates/soranet_testnet_overview.json` እና `alert_templates/soranet_testnet_rules.yml` ስር ይኖራሉ; ወደ ቴሌሜትሪ ማከማቻዎ ይቅዱዋቸው ወይም ወደ CI lint checks ያሽጉዋቸው።
- ለኤስዲኬ/ፖርታል መልእክት መላላኪያ የወረደው የግንኙነት አብነት በI18NI0000038X ውስጥ ይኖራል።
- ሳምንታዊ ሁኔታ መፍጨት `docs/source/status/soranet_testnet_weekly_digest.md` እንደ ቀኖናዊ ቅጽ መጠቀም አለበት።

የልቀት ዕቅዱ ቀኖናዊ ሆኖ እንዲቆይ የመጎተት ጥያቄዎች ከማንኛቸውም የስነ ጥበብ ወይም የቴሌሜትሪ ለውጦች ጋር ይህን ገጽ ማዘመን አለባቸው።