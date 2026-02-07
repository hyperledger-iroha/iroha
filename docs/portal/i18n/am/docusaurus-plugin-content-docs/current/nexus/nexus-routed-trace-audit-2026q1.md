---
id: nexus-routed-trace-audit-2026q1
lang: am
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: 2026 Q1 routed-trace audit report (B1)
description: Mirror of `docs/source/nexus_routed_trace_audit_report_2026q1.md`, covering the quarterly telemetry rehearsal outcomes.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

::: ማስታወሻ ቀኖናዊ ምንጭ
ይህ ገጽ `docs/source/nexus_routed_trace_audit_report_2026q1.md` ያንጸባርቃል። ቀሪዎቹ ትርጉሞች መሬት እስኪያገኙ ድረስ ሁለቱንም ቅጂዎች አስተካክለው ያስቀምጡ።
::

# 2026 Q1 የተዘዋዋሪ-ዱካ ኦዲት ሪፖርት (B1)

የፍኖተ ካርታ ንጥል **B1 — የተዘዋዋሪ ዱካ ኦዲት እና ቴሌሜትሪ ቤዝላይን** ይጠይቃል ሀ
የ I18NT0000001X ራውተር-ክትትል ፕሮግራም የሩብ ዓመት ግምገማ። ይህ ሪፖርት ሰነድ
Q12026 የኦዲት መስኮት (ከጥር እስከ መጋቢት) የአስተዳደር ካውንስል መፈረም ይችላል።
ከQ2 ማስጀመሪያ ልምምዶች በፊት የቴሌሜትሪ አቀማመጥ።

## ወሰን እና የጊዜ መስመር

| መከታተያ መታወቂያ | መስኮት (UTC) | አላማ |
|-------|-------------|--------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | ባለብዙ ሌይን ከማንቃት በፊት የሌይን መግቢያ ሂስቶግራሞችን፣ የወረፋ ወሬዎችን እና የማንቂያ ፍሰትን ያረጋግጡ። |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | ከ AND4/AND7 ምእራፎች ቀደም ብሎ የOTLP ድግግሞሹን ፣ ልዩነትን እና የኤስዲኬ የቴሌሜትሪ መግቢያን ያረጋግጡ። |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | ከRC1 መቁረጥ በፊት በአስተዳደር የጸደቀ `iroha_config` ዴልታስ እና የመመለሻ ዝግጁነት ያረጋግጡ። |

እያንዳንዱ ልምምዱ በአመራረት መሰል ቶፖሎጂ ላይ የተካሄደው በተዘዋዋሪ መንገድ ነው።
መሣሪያ ነቅቷል (`nexus.audit.outcome` ቴሌሜትሪ + Prometheus ቆጣሪዎች) ፣
የማንቂያ አስተዳዳሪ ደንቦች ተጭነዋል፣ እና ማስረጃ ወደ `docs/examples/` ተልኳል።

## ዘዴ

1. **የቴሌሜትሪ ስብስብ።** ሁሉም አንጓዎች የተዋቀረውን ለቀቁ
   `nexus.audit.outcome` ክስተት እና ተጓዳኝ መለኪያዎች
   (`nexus_audit_outcome_total*`)። ረዳቱ
   `scripts/telemetry/check_nexus_audit_outcome.py` የJSON ምዝግብ ማስታወሻውን ጭራ አድርጎታል፣
   የክስተቱን ሁኔታ አረጋግጧል፣ እና የተጫነውን ጭነት በማህደር አስቀምጧል
   `docs/examples/nexus_audit_outcomes/`.【ስክሪፕቶች/ቴሌሜትሪ/የቼክ_nexus_audit_ውጤት.py:1】
2. ** የማንቂያ ማረጋገጫ።** `dashboards/alerts/nexus_audit_rules.yml` እና ፈተናው
   መታጠቂያ የተረጋገጠ የማንቂያ ጫጫታ ጣራዎች እና የመጫኛ ጭነት ቴምፕሊንግ ቆየ
   ወጥነት ያለው. CI `dashboards/alerts/tests/nexus_audit_rules.test.yml` ላይ ይሰራል
   እያንዳንዱ ለውጥ; በእያንዳንዱ መስኮት ውስጥ ተመሳሳይ ደንቦች በእጅ ተካሂደዋል.
3. ** ዳሽቦርድ ቀረጻ።** ኦፕሬተሮች የዱካ ፓነሎችን ወደ ውጭ ልከዋል።
   `dashboards/grafana/soranet_sn16_handshake.json` (የእጅ መጨባበጥ ጤና) እና የ
   የቴሌሜትሪ አጠቃላይ እይታ ዳሽቦርዶች የወረፋ ጤናን ከኦዲት ውጤቶች ጋር ለማዛመድ።
4. ** የገምጋሚ ማስታወሻዎች።** የአስተዳደር ፀሐፊው የገምጋሚውን የመጀመሪያ ፊደላት አስገብቷል።
   ውሳኔ፣ እና በ[Nexus የሽግግር ማስታወሻዎች](./nexus-transition-notes) ውስጥ ያሉ ማናቸውም የቅናሽ ትኬቶች
   እና የማዋቀር ዴልታ መከታተያ (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`)።

# ግኝቶች

| መከታተያ መታወቂያ | ውጤት | ማስረጃ | ማስታወሻ |
|-------|--------|----------|-------|
| `TRACE-LANE-ROUTING` | ማለፍ | የማንቂያ እሳት / ቅጽበታዊ ገጽ እይታዎችን መልሶ ማግኘት (ውስጣዊ ማገናኛ) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` እንደገና ማጫወት; የቴሌሜትሪ ልዩነቶች በ [Nexus የሽግግር ማስታወሻዎች] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule) ተመዝግቧል። | ወረፋ-መግቢያ P95 612ሚሴ ቀርቷል (ዒላማ ≤750 ሚሴ)። ምንም ክትትል አያስፈልግም. |
| `TRACE-TELEMETRY-BRIDGE` | ማለፍ | በማህደር የተቀመጠ የውጤት ጭነት `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` እና OTLP ድጋሚ አጫውት ሃሽ በ`status.md` ተመዝግቧል። | የኤስዲኬ ማሻሻያ ጨዎች ከ Rust መነሻ መስመር ጋር ይጣጣማሉ; diff bot ዜሮ ዴልታስ ዘግቧል። |
| `TRACE-CONFIG-DELTA` | ማለፍ (መቀነሱ ተዘግቷል) | የአስተዳደር መከታተያ መግቢያ (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS መገለጫ (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + ቴሌሜትሪ ጥቅል መግለጫ (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)። | Q2 ድጋሚ መሮጥ የጸደቀውን የTLS መገለጫ እና የተረጋገጠ ዜሮ ተንቀሳቃሾችን ሃሸድ፤ ቴሌሜትሪ አንጸባራቂ መዛግብት ማስገቢያ ክልል 912-936 እና የስራ ጫና ዘር `NEXUS-REH-2026Q2`. |

ሁሉም ዱካዎች በእነሱ ውስጥ ቢያንስ አንድ የ I18NI0000035X ክስተት አምርተዋል።
መስኮቶች፣ የአለርትማኔጀር ጠባቂዎችን (`NexusAuditOutcomeFailure`
ለሩብ ያህል አረንጓዴ ሆኖ ቆይቷል).

## ክትትሎች

- ዱካ አባሪ በTLS hash I18NI0000037X ዘምኗል።
  ቅነሳ `NEXUS-421` በሽግግር ማስታወሻዎች ተዘግቷል.
- ጥሬ OTLP ድግግሞሾችን እና Torii ልዩ ልዩ ቅርሶችን ከማህደሩ ጋር ማያያዝዎን ይቀጥሉ
  ለአንድሮይድ AND4/AND7 አስተያየቶች የተመጣጠነ ማስረጃን ማጠናከር።
- መጪዎቹ የI18NI0000039X ልምምዶች ተመሳሳይ መሆናቸውን ያረጋግጡ
  የቴሌሜትሪ አጋዥ ስለዚህ Q2 ማቋረጥ ከተረጋገጠው የስራ ሂደት ይጠቅማል።

## Artefact ኢንዴክስ

| ንብረት | አካባቢ |
|-------------|
| ቴሌሜትሪ አረጋጋጭ | `scripts/telemetry/check_nexus_audit_outcome.py` |
| የማንቂያ ደንቦች እና ሙከራዎች | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| የናሙና የውጤት ጭነት | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| የዴልታ መከታተያ አዋቅር | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| የዱካ መርሐግብር እና ማስታወሻዎች | [Nexus የሽግግር ማስታወሻዎች](./nexus-transition-notes) |

ይህ ሪፖርት፣ ከላይ ያሉት ቅርሶች እና የማስጠንቀቂያ/የቴሌሜትሪ ኤክስፖርት መሆን አለባቸው
ለሩብ ዓመት B1 ለመዝጋት ከአስተዳደር ውሳኔ መዝገብ ጋር ተያይዟል.