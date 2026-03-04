---
id: nexus-telemetry-remediation
lang: am
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus telemetry remediation plan (B2)
description: Mirror of `docs/source/nexus_telemetry_remediation_plan.md`, documenting the telemetry gap matrix and operational workflow.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# አጠቃላይ እይታ

የመንገድ ካርታ ንጥል **B2 — የቴሌሜትሪ ክፍተት ባለቤትነት** የታተመ የእቅድ ማያያዝን ይጠይቃል
እያንዳንዱ የላቀ Nexus የቴሌሜትሪ ክፍተት ወደ ሲግናል፣ የማስጠንቀቂያ ጥበቃ፣ ባለቤት፣
የQ1 2026 የኦዲት መስኮቶች ከመጀመራቸው በፊት ቀነ-ገደብ እና የማረጋገጫ ቅርስ።
ይህ ገጽ `docs/source/nexus_telemetry_remediation_plan.md` ያንጸባርቃል ስለዚህ ይለቀቁ
የኢንጂነሪንግ፣ የቴሌሜትሪ ኦፕስ እና የኤስዲኬ ባለቤቶች ሽፋንን ከዚህ በፊት ማረጋገጥ ይችላሉ።
ራውድ-ዱካ እና I18NI0000011X ልምምዶች።

# ክፍተት ማትሪክስ

| ክፍተት መታወቂያ | ሲግናል እና ማንቂያ ትራይል | ባለቤት / Escalation | የሚከፈልበት (UTC) | ማስረጃ እና ማረጋገጫ |
|--------|-------------|--------|----------|
| `GAP-TELEM-001` | ሂስቶግራም `torii_lane_admission_latency_seconds{lane_id,endpoint}` ከማንቂያ ጋር **I18NI0000014X** `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` ለ 5minutes (`dashboards/alerts/soranet_lane_rules.yml`) ሲተኮስ። | `@torii-sdk` (ምልክት) + I18NI0000018X (ማስጠንቀቂያ); በI18NT0000004X በተሰየመ የጥሪ ዱካ ማሳደግ። | 2026-02-23 | የማንቂያ ሙከራዎች በ`dashboards/alerts/tests/soranet_lane_rules.test.yml` እና በ `TRACE-LANE-ROUTING` የመለማመጃ ቀረጻ የተኩስ/የተገኘ ማንቂያ እና Torii `/metrics` scrape በ[I18NT000000019X] ውስጥ ተቀምጧል። |
| `GAP-TELEM-002` | Counter `nexus_config_diff_total{knob,profile}` ከጠባቂ ሀዲድ `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` gating ያሰማራው (`docs/source/telemetry.md`)። | `@nexus-core` (መሳሪያ) → `@telemetry-ops` (ማስጠንቀቂያ); የአስተዳደር ተረኛ ባለሥልጣን ቆጣሪ ሳይታሰብ ሲጨምር ብቅ ብሏል። | 2026-02-26 | ከ `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` አጠገብ የተከማቹ የአስተዳደር ደረቅ አሂድ ውጤቶች; የመልቀቂያ ማረጋገጫ ዝርዝሩ የI18NT0000000X መጠይቅ ቅጽበታዊ ገጽ እይታ እና `StateTelemetry::record_nexus_config_diff` ልዩነቱን እንደተለቀቀ የሚያረጋግጥ የምዝግብ ማስታወሻ ያካትታል። |
| `GAP-TELEM-003` | ክስተት `TelemetryEvent::AuditOutcome` (ሜትሪክ `nexus.audit.outcome`) ከማስጠንቀቂያ ጋር **`NexusAuditOutcomeFailure`** ውድቀቶች ወይም የጎደሉ ውጤቶች ለ> 30 ደቂቃዎች (`dashboards/alerts/nexus_audit_rules.yml`) ሲቆዩ። | `@telemetry-ops` (የቧንቧ መስመር) ወደ `@sec-observability` በማደግ ላይ። | 2026-02-27 | CI gate `scripts/telemetry/check_nexus_audit_outcome.py` ማህደሮች NDJSON የሚጭኑ ሲሆን የ TRACE መስኮት የስኬት ክስተት ሲጎድልበት አይሳካም። የማስጠንቀቂያ ቅጽበታዊ ገጽ እይታዎች ከተዘዋዋሪ-ክትትል ዘገባ ጋር ተያይዘዋል። |
| `GAP-TELEM-004` | መለኪያ `nexus_lane_configured_total` ከጠባቂ ሀዲድ `nexus_lane_configured_total != EXPECTED_LANE_COUNT` ጋር የSRE የጥሪ ማመሳከሪያ ዝርዝርን መመገብ። | `@telemetry-ops` (መለኪያ/መላክ) ወደ `@nexus-core` በማደግ ላይ ያሉ አንጓዎች የማይጣጣሙ የካታሎግ መጠኖችን ሲዘግቡ። | 2026-02-28 | የጊዜ መርሐግብር የቴሌሜትሪ ሙከራ I18NI0000043X ልቀትን ያረጋግጣል; ኦፕሬተሮች Prometheus diff + I18NI0000044X ምዝግብ ማስታወሻ ከ TRACE የመለማመጃ ጥቅል ጋር አያይዘውታል። |

# ተግባራዊ የስራ ሂደት

1. ** ሳምንታዊ መለያየት።** ባለቤቶች በI18NT0000006X ዝግጁነት ጥሪ ላይ ያለውን ሂደት ሪፖርት ያደርጋሉ።
   አጋጆች እና የማንቂያ-ሙከራ ቅርሶች በ`status.md` ውስጥ ገብተዋል።
2. **የደረቁ ሩጫዎች ማንቂያ ያድርጉ።** እያንዳንዱ የማንቂያ ደንብ ከ ሀ
   `dashboards/alerts/tests/*.test.yml` ግቤት ስለዚህ CI `promtool ሙከራን ይፈጽማል
   የጥበቃ ሀዲድ በሚቀየርበት ጊዜ ሁሉ ህጎች።
3. **የኦዲት ማስረጃ።** በ `TRACE-LANE-ROUTING` እና
   `TRACE-TELEMETRY-BRIDGE` ልምምዶች በጥሪው ላይ የI18NT0000002X ጥያቄን ይይዛል
   ውጤቶች፣ የማንቂያ ታሪክ እና ተዛማጅ የስክሪፕት ውጤቶች
   (`scripts/telemetry/check_nexus_audit_outcome.py`፣
   `scripts/telemetry/check_redaction_status.py` ለተቆራኙ ምልክቶች) እና
   በተበላሹ ቅርሶች ያከማቻል።
4. **መባባስ።** ከተለማመደው መስኮት ውጭ የትኛውም የጥበቃ መንገድ ከተተኮሰ የባለቤትነት መብቱ የተጠበቀ ነው።
   ቡድኑ ይህንን እቅድ በመጥቀስ የNexus የአደጋ ትኬት ፋይል ያደርጋል፣ የ
   ኦዲት ከመቀጠልዎ በፊት ሜትሪክ ቅጽበተ-ፎቶ እና ቅነሳ እርምጃዎች።

በዚህ ማትሪክስ ታትሟል - እና ከሁለቱም `roadmap.md` እና የተጠቀሰው።
`status.md` — የመንገድ ካርታ ንጥል **B2** አሁን “ኃላፊነቱን፣ ቀነ-ገደቡን፣
ማንቂያ, ማረጋገጫ "የመቀበል መስፈርቶች.