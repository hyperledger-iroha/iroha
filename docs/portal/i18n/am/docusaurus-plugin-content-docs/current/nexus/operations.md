---
id: nexus-operations
lang: am
direction: ltr
source: docs/portal/docs/nexus/operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus operations runbook
description: Field-ready summary of the Nexus operator workflow, mirroring `docs/source/nexus_operations.md`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ይህን ገጽ እንደ ፈጣን ማጣቀሻ ወንድም ወይም እህት ይጠቀሙበት
`docs/source/nexus_operations.md`. የተግባር ማመሳከሪያ ዝርዝሩን ያስወግዳል, ይቀይሩ
የአስተዳደር መንጠቆዎች እና የቴሌሜትሪ ሽፋን መስፈርቶች Nexus ኦፕሬተሮች አለባቸው
ተከተል።

## የህይወት ዑደት ማረጋገጫ ዝርዝር

| መድረክ | ድርጊቶች | ማስረጃ |
|-------|--------|------|
| ቅድመ በረራ | የተለቀቀውን ሃሽ/ፊርማ ያረጋግጡ፣ `profile = "iroha3"` ያረጋግጡ እና የውቅር አብነቶችን ያዘጋጁ። | `scripts/select_release_profile.py` ውፅዓት፣ የቼክሰም መዝገብ፣ የተፈረመ የማሳያ ቅርቅብ። |
| ካታሎግ አሰላለፍ | `[nexus]` ካታሎግ፣ የማዘዋወር ፖሊሲ እና የDA ገደቦችን በምክር ቤት የተሰጠ መግለጫ ያዘምኑ፣ ከዚያ `--trace-config`ን ይያዙ። | `irohad --sora --config … --trace-config` ውፅዓት ከመሳፈሪያ ትኬት ጋር ተከማችቷል። |
| ጭስ እና መቁረጫ | `irohad --sora --config … --trace-config` ን ያሂዱ፣ የCLI ጭስ (`FindNetworkStatus`) ያስፈጽሙ፣ የቴሌሜትሪ ኤክስፖርትን ያረጋግጡ እና ለመግባት ይጠይቁ። | የጭስ-ሙከራ ምዝግብ ማስታወሻ + የማስጠንቀቂያ አስተዳዳሪ ማረጋገጫ። |
| የተረጋጋ ሁኔታ | ዳሽቦርዶችን/ማንቂያዎችን ተቆጣጠር፣ በየአስተዳደሩ ቁልፎቹን አሽከርክር፣ እና ለውጦች በሚገለጡበት ጊዜ አወቃቀሮችን/አሂድ ቡክሮችን ያመሳስሉ። | የሩብ ጊዜ ግምገማ ደቂቃዎች፣ ዳሽቦርድ ቅጽበታዊ ገጽ እይታዎች፣ የማዞሪያ ትኬት መታወቂያዎች። |

ዝርዝር የመሳፈሪያ (ቁልፍ ምትክ፣ የማዞሪያ አብነቶች፣ የመገለጫ ደረጃዎች)
በ `docs/source/sora_nexus_operator_onboarding.md` ውስጥ ይቆዩ.

## ለውጥ አስተዳደር

1. ** ዝመናዎችን ይልቀቁ ** - ማስታወቂያዎችን በ `status.md`/`roadmap.md`; ማያያዝ
   የቦርዲንግ ማረጋገጫ ዝርዝር ለእያንዳንዱ የተለቀቀው PR።
2. **የሌይን አንጸባራቂ ለውጦች** - የተፈረሙ ጥቅሎችን ከጠፈር ማውጫው ያረጋግጡ እና
   በ `docs/source/project_tracker/nexus_config_deltas/` ስር አስቀምጣቸው።
3. **የማዋቀር ዴልታዎች** - እያንዳንዱ የ`config/config.toml` ለውጥ ትኬት ይፈልጋል።
   ሌይን/መረጃ-ቦታን በመጥቀስ። ውጤታማ ውቅር የተቀየረ ቅጂ ያከማቹ
   አንጓዎች ሲቀላቀሉ ወይም ሲያሻሽሉ.
4. ** የድጋሚ ልምምዶች *** - በየሩብ ዓመቱ የማቆሚያ / ወደነበረበት መመለስ / የጭስ ሂደቶችን ይለማመዱ; መዝገብ
   ውጤቶች በ `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **የማስፈጸሚያ ማጽደቂያዎች** - የግል/CBDC መስመሮች የታዛዥነት ማቋረጥን ማረጋገጥ አለባቸው
   የDA ፖሊሲን ወይም የቴሌሜትሪ ማሻሻያ ቁልፎችን ከማሻሻል በፊት (ተመልከት
   `docs/source/cbdc_lane_playbook.md`).

## ቴሌሜትሪ እና SLOs

- ዳሽቦርዶች፡ `dashboards/grafana/nexus_lanes.json`፣ `nexus_settlement.json`፣ በተጨማሪም
  ኤስዲኬ-ተኮር እይታዎች (ለምሳሌ፣ `android_operator_console.json`)።
- ማንቂያዎች፡ I18NI0000039X እና I18NT0000013X/Norito ትራንስፖርት
  ደንቦች (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- ለመመልከት መለኪያዎች:
  - `nexus_lane_height{lane_id}` - ለሶስት ቦታዎች በዜሮ ግስጋሴ ላይ ማንቂያ.
  - `nexus_da_backlog_chunks{lane_id}` - ከሌይን-ተኮር ገደቦች በላይ ማንቂያ
    (ነባሪ 64 ይፋዊ/8 የግል)።
  - `nexus_settlement_latency_seconds{lane_id}` - P99 ከ900 ሚሴ በላይ ሲያልፍ ማንቂያ
    (የህዝብ) ወይም 1200ms (የግል)።
  - `torii_request_failures_total{scheme="norito_rpc"}` - የ 5 ደቂቃ ስህተት ከሆነ አስጠንቅቅ
    ጥምርታ>2%
  - `telemetry_redaction_override_total` - Sev2 ወዲያውኑ; መሻርን ያረጋግጡ
    ተገዢነት ትኬቶች አላቸው.
- የቴሌሜትሪ ማሻሻያ ማረጋገጫ ዝርዝሩን አስገባ
  [Nexus ቴሌሜትሪ ማሻሻያ እቅድ](./nexus-telemetry-remediation) ቢያንስ
  በየሩብ ዓመቱ እና የተሞላውን ቅጽ ከኦፕሬሽኖች ግምገማ ማስታወሻዎች ጋር ያያይዙ።

## የክስተት ማትሪክስ

| ከባድነት | ፍቺ | ምላሽ |
|------------------|------|
| ሴቭ1 | የውሂብ-ቦታ ማግለል ጥሰት፣ የሰፈራ ማቆም > 15 ደቂቃ፣ ወይም የአስተዳደር ድምጽ ሙስና። | Page Nexus የመጀመሪያ ደረጃ + መልቀቂያ ኢንጂነሪንግ + ማክበር፣ መግባትን ማቆም፣ ቅርሶችን መሰብሰብ፣ comms ≤60min ያትሙ፣ RCA ≤5 የስራ ቀናት። |
| Sev2 | የሌይን የኋላ ሎግ SLA መጣስ፣ የቴሌሜትሪ ዓይነ ስውር ቦታ > 30 ደቂቃ፣ ያልተሳካ የማሳያ መልቀቅ። | ገጽ Nexus የመጀመሪያ ደረጃ + SRE፣ ≤4ሰአትን ይቀንሱ፣ በ2 የስራ ቀናት ውስጥ ክትትልን ያድርጉ። |
| Sev3 | የማይታገድ ተንሸራታች (ሰነዶች፣ ማንቂያዎች)። | መከታተያ ውስጥ ይግቡ፣ በSprint ውስጥ ለማስተካከል የጊዜ ሰሌዳ ያውጡ። |

የክስተት ትኬቶች የተጎዱትን የሌይን/የመረጃ ቦታ መታወቂያዎችን፣ የሰነድ ማስረጃዎችን መመዝገብ አለባቸው፣
የጊዜ መስመር፣ የድጋፍ መለኪያዎች/ምዝግብ ማስታወሻዎች፣ እና ተከታይ ተግባራት/ባለቤቶች።

## የማስረጃ መዝገብ

- በ `artifacts/nexus/<lane>/<date>/` ጥቅሎችን/ማሳያዎችን/የቴሌሜትሪ ኤክስፖርትን ያከማቹ።
- ለእያንዳንዱ ልቀት የተቀናጁ ውቅሮችን + `--trace-config` ውፅዓት ያቆዩ።
- ሲዋቀሩ ወይም መግለጫው መሬት ሲቀየር የምክር ቤት ደቂቃዎችን + የተፈረሙ ውሳኔዎችን ያያይዙ።
- ከNexus ሜትሪክስ ጋር ተዛማጅነት ያላቸውን ሳምንታዊ የI18NT0000000X ቅጽበተ-ፎቶዎችን ለ12 ወራት ያቆዩ።
- የ runbook አርትዖቶችን በ `docs/source/project_tracker/nexus_config_deltas/README.md` ይመዝግቡ
  ስለዚህ ኦዲተሮች ኃላፊነቶች ሲቀየሩ ያውቃሉ።

## ተዛማጅ ቁሳቁስ

አጠቃላይ እይታ፡ [I18NT0000007X አጠቃላይ እይታ](./nexus-overview)
ዝርዝር፡ [Nexus ዝርዝር](./nexus-spec)
- ሌይን ጂኦሜትሪ፡ [Nexus ሌይን ሞዴል](./nexus-lane-model)
- የመሸጋገሪያ እና የማዞሪያ ሺምስ፡ [Nexus የሽግግር ማስታወሻዎች](./nexus-transition-notes)
- ኦፕሬተር በመሳፈር ላይ፡ [Sora Nexus ኦፕሬተር ተሳፍሮ](./nexus-operator-onboarding)
- ቴሌሜትሪ ማሻሻያ፡ [Nexus የቴሌሜትሪ ማስተካከያ እቅድ](./nexus-telemetry-remediation)