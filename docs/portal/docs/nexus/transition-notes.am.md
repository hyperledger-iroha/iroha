---
lang: am
direction: ltr
source: docs/portal/docs/nexus/transition-notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 90869bf46975e3ac40cf606b2ac27597f81e34acbce25261dc9f588e0fac3103
source_last_modified: "2026-01-06T05:56:41.006216+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-transition-notes
title: Nexus transition notes
description: Mirror of `docs/source/nexus_transition_notes.md`, covering Phase B transition evidence, audit schedule, and mitigations.
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus የሽግግር ማስታወሻዎች

ይህ ምዝግብ ማስታወሻ የዘገየውን ** ደረጃ B — Nexus የሽግግር መሠረቶች ** ሥራን ይከታተላል
ባለብዙ መስመር ማስጀመሪያ ማረጋገጫ ዝርዝሩ እስኪያልቅ ድረስ። የድል ጉዞን ይጨምራል
በ I18NI00000011 ውስጥ ያስገባ እና በ B1-B4 የተጠቀሰውን ማስረጃ በአንድ ቦታ ያስቀምጣል.
ስለዚህ አስተዳደር፣ SRE እና ኤስዲኬ መሪዎች ተመሳሳይ የእውነት ምንጭ ሊጋሩ ይችላሉ።

## ወሰን እና Cadence

- የተዘዋወሩ ኦዲቶችን እና የቴሌሜትሪ መከላከያ መንገዶችን (B1/B2) ይሸፍናል።
  በአስተዳደር የጸደቀ ውቅር ዴልታ ስብስብ (B3) እና ባለብዙ መስመር ማስጀመሪያ
  የመለማመጃ ክትትል (B4).
- ከዚህ ቀደም እዚህ ይኖሩ የነበሩትን ጊዜያዊ የቃላት ማስታወሻ ይተካዋል; ከ 2026 ጀምሮ
  Q1 ዝርዝር ሪፖርቱ በውስጡ ይኖራል
  `docs/source/nexus_routed_trace_audit_report_2026q1.md`፣ የዚህ ገጽ ባለቤት ሆኖ ሳለ
  የሩጫ መርሃ ግብሩ እና የቅናሽ መዝገብ.
- ከእያንዳንዱ የመከታተያ መስኮት፣ የአስተዳደር ድምጽ ወይም ከተጀመረ በኋላ ሰንጠረዦቹን ያዘምኑ
  ልምምድ ማድረግ. ቅርሶች በሚንቀሳቀሱበት ጊዜ፣ በዚህ ገጽ ውስጥ ያለውን አዲስ ቦታ ያንጸባርቁት
  ስለዚህ የታችኛው ተፋሰስ ሰነዶች (ሁኔታ፣ ዳሽቦርድ፣ ኤስዲኬ መግቢያዎች) ወደ መረጋጋት ሊገናኙ ይችላሉ።
  መልህቅ

## የማስረጃ ቅጽበታዊ ገጽ እይታ (2026 Q1–Q2)

| የስራ ፍሰት | ማስረጃ | ባለቤት(ዎች) | ሁኔታ | ማስታወሻ |
|--------|----------|------
| **B1 - የዱካ ክትትል ኦዲቶች** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @መንግስት | ✅ ሙሉ (Q1 2026) | ሶስት የኦዲት መስኮቶች ተመዝግበዋል; የቲኤልኤስ መዘግየት ከI18NI0000015X በQ2 ድጋሚ ሲካሄድ ተዘግቷል። |
| ** B2 - የቴሌሜትሪ ማስተካከያ እና መከላከያ መንገዶች *** | `docs/source/nexus_telemetry_remediation_plan.md`፣ `docs/source/telemetry.md`፣ `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | ✅ ሙሉ | የማንቂያ ጥቅል፣ የዳይፍ ቦት ፖሊሲ እና የ OTLP ባች መጠን (`nexus.scheduler.headroom` log + Grafana headroom panel) ተልኳል; ምንም ክፍት ጥፋቶች. |
| ** B3 - የዴልታ ማጽደቆችን ያዋቅሩ *** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`፣ `defaults/nexus/config.toml`፣ `defaults/nexus/genesis.json` | @release-eng, @መንግስት | ✅ ሙሉ | GOV-2026-03-19 ድምጽ ተያዘ; የተፈረመ ጥቅል ከዚህ በታች የተመለከተውን የቴሌሜትሪ ጥቅል ይመገባል። |
| **B4 — ባለብዙ መስመር ማስጀመሪያ ልምምድ** | `docs/source/runbooks/nexus_multilane_rehearsal.md`፣ `docs/source/project_tracker/nexus_rehearsal_2026q1.md`፣ `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`፣ `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`፣ `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-ኮር, @sre-ኮር | ✅ ሙሉ (Q2 2026) | Q2 canary rerun TLS መዘግየትን ዘግቷል; የማረጋገጫ መግለጫ + I18NI0000028X የቀረጻ ማስገቢያ ክልል 912–936፣ የስራ ጫና ዘር I18NI0000029X፣ እና የተመዘገበው የTLS መገለጫ ሃሽ ከድጋሚ። |

## በየሩብ ጊዜ የሚዘዋወር-የዱካ ኦዲት መርሃ ግብር

| መከታተያ መታወቂያ | መስኮት (UTC) | ውጤት | ማስታወሻ |
|-------|-------------|--------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | ✅ ማለፍ | ወረፋ መግቢያ P95 ከ ≤750 ሚሴ ኢላማ በታች ሆኖ ቆይቷል። ምንም እርምጃ አያስፈልግም። |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | ✅ ማለፍ | ከ `status.md` ጋር ተያይዟል OTLP ድጋሚ አጫውት; የኤስዲኬ ልዩነት ቦት እኩልነት ዜሮ መንሸራተትን አረጋግጧል። |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | ✅ ተፈቷል | በQ2 ድጋሚ በሚካሄድበት ጊዜ የTLS መገለጫ መዘግየት ተዘግቷል፤ የቴሌሜትሪ ጥቅል ለI18NI0000034X መዛግብት TLS ፕሮፋይል hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (`artifacts/nexus/tls_profile_rollout_2026q2/` ይመልከቱ) እና ዜሮ stragglers። |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12–10:14 | ✅ ማለፍ | የሥራ ጫና ዘር `NEXUS-REH-2026Q2`; የቴሌሜትሪ ጥቅል + ማኒፌስት/መፍጨት በ`artifacts/nexus/rehearsals/2026q1/` (የማስገቢያ ክልል 912-936) በ`artifacts/nexus/rehearsals/2026q2/` ውስጥ ካለው አጀንዳ ጋር። |

የወደፊቱ ሰፈሮች አዲስ ረድፎችን ማከል እና ማንቀሳቀስ አለባቸው
ጠረጴዛው ከአሁኑ በላይ ሲያድግ ወደ አባሪ የተጠናቀቁ ግቤቶች
ሩብ. ይህንን ክፍል ከትራክ ሪፖርቶች ወይም የአስተዳደር ደቂቃዎች ይመልከቱ
የ `#quarterly-routed-trace-audit-schedule` መልህቅን በመጠቀም።

## ቅነሳ እና የኋላ መዝገብ ዕቃዎች

| ንጥል | መግለጫ | ባለቤት | ዒላማ | ሁኔታ / ማስታወሻዎች |
|-------------|-------|--------|
| `NEXUS-421` | በ`TRACE-CONFIG-DELTA` ጊዜ የዘገየውን የTLS ፕሮፋይል ማሰራጨቱን ይጨርሱ፣ የድጋሚ ማስረጃዎችን ይያዙ እና የቅናሽ ምዝግብ ማስታወሻውን ይዝጉ። | @release-eng, @sre-ኮር | Q2 2026 የዱካ መከታተያ መስኮት | ✅ ተዘግቷል - TLS ፕሮፋይል ሃሽ `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` በ `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` ተይዟል; ድጋሚ መሮጥ ምንም ተሳፋሪዎች አልተረጋገጠም። |
| `TRACE-MULTILANE-CANARY` መሰናዶ | የQ2 ልምምዱን መርሐግብር ያስይዙ፣ መጫዎቻዎችን ከቴሌሜትሪ ጥቅል ጋር አያይዘው እና የኤስዲኬ ማሰሪያዎች የተረጋገጠውን ረዳት እንደገና መጠቀማቸውን ያረጋግጡ። | @telemetry-ops፣ SDK ፕሮግራም | የእቅድ ጥሪ 2026-04-30 | ✅ ተጠናቅቋል - አጀንዳ በ `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` ከስሎድ/የስራ ጫና ሜታዳታ ጋር ተከማችቷል፤ መታጠቂያ እንደገና ጥቅም ላይ ማዋል በክትትል ውስጥ ተጠቅሷል። |
| ቴሌሜትሪ ጥቅል መፍጨት ሽክርክር | ከእያንዳንዱ ልምምድ/መለቀቅ በፊት `scripts/telemetry/validate_nexus_telemetry_pack.py` ን ያሂዱ እና ከኮንፍተሬሽን ዴልታ መከታተያ ቀጥሎ የምዝግብ ማስታወሻዎች። | @telemetry-ops | በመልቀቅ እጩ | ✅ ተጠናቅቋል - `telemetry_manifest.json` + `.sha256` በ `artifacts/nexus/rehearsals/2026q1/` የተለቀቀ (የማስገቢያ ክልል `912-936` ፣ ዘር `NEXUS-REH-2026Q2`); መፍጨት ወደ መከታተያ እና የማስረጃ ኢንዴክስ ተቀድቷል። |

## የዴልታ ቅርቅብ ውህደትን ያዋቅሩ

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ይቀራል
  ቀኖናዊ ልዩነት ማጠቃለያ። አዲስ `defaults/nexus/*.toml` ወይም ዘፍጥረት ሲቀየር
  መሬት፣ መጀመሪያ ያንን መከታተያ ያዘምኑ፣ ከዚያ ዋና ዋናዎቹን እዚህ ያንጸባርቁ።
- የተፈረሙ የውቅር ቅርቅቦች የመለማመጃውን ቴሌሜትሪ ጥቅል ይመገባሉ። ጥቅሉ፣ የተረጋገጠ
  በ `scripts/telemetry/validate_nexus_telemetry_pack.py`፣ መታተም አለበት።
  ኦፕሬተሮች ትክክለኛውን ነገር እንደገና ማጫወት እንዲችሉ ከማዋቀር ዴልታ ማስረጃ ጋር
  በ B4 ጊዜ ጥቅም ላይ የዋሉ ቅርሶች.
- Iroha 2 ጥቅሎች ከመንገድ ነፃ ሆነው ይቆያሉ፡ ከ`nexus.enabled = false` ጋር ውቅር
  የNexus መገለጫ ካልነቃ በስተቀር መስመር/የውሂብ ቦታ/ማዞሪያ ይሽራል
  (`--sora`)፣ ስለዚህ I18NI0000060X ክፍሎችን ከአንድ ሌይን አብነቶች አውጣ።
- የአስተዳደር ድምጽ ምዝግብ ማስታወሻውን (GOV-2026-03-19) ከሁለቱም መከታተያ እና ተያያዥነት ያቆዩ።
  ይህ ማስታወሻ ስለዚህ የወደፊት ድምጾች ቅርጸቱን እንደገና ሳያውቁ መቅዳት ይችላሉ።
  የማጽደቅ ሥነ ሥርዓት.

## የመለማመጃ ክትትልን አስጀምር

- `docs/source/runbooks/nexus_multilane_rehearsal.md` የካናሪ እቅዱን ይይዛል ፣
  የአሳታፊ ዝርዝር, እና የመመለሻ ደረጃዎች; በሌይኑ በማንኛውም ጊዜ የሩጫ መጽሐፍን ያድሱ
  ቶፖሎጂ ወይም ቴሌሜትሪ ላኪዎች ይለወጣሉ።
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` እያንዳንዱን ቅርስ ይዘረዝራል።
  በኤፕሪል 9 ልምምድ ወቅት የተረጋገጠ እና አሁን የQ2 መሰናዶ ማስታወሻዎችን/አጀንዳዎችን ይይዛል።
  የወደፊት ልምምዶችን አንድ ጊዜ ከመክፈት ይልቅ በተመሳሳዩ መከታተያ ላይ ጨምሩ
  መከታተያዎች አንድ ማስረጃ ብቻ ለማቆየት.
- የOTLP ሰብሳቢ ቅንጣቢዎችን እና Grafana ወደ ውጭ የሚላኩ ነገሮችን ያትሙ (`docs/source/telemetry.md` ይመልከቱ)
  ላኪው ባቲንግ መመሪያ ሲቀየር; የQ1 ዝማኔው ጎድቶታል።
  የጭንቅላት ክፍል ማንቂያዎችን ለመከላከል ባች መጠን ወደ 256 ናሙናዎች።
- ባለብዙ መስመር CI/የሙከራ ማስረጃ አሁን ይኖራል
  `integration_tests/tests/nexus/multilane_pipeline.rs` እና በ ውስጥ ይሰራል
  `Nexus Multilane Pipeline` የስራ ፍሰት
  (`.github/workflows/integration_tests_multilane.yml`)፣ ጡረተኛውን በመተካት።
  `pytests/nexus/test_multilane_pipeline.py` ማጣቀሻ; ሃሽውን ጠብቅ
  `defaults/nexus/config.toml` (`nexus.enabled = true`፣ blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) በማመሳሰል
  የመልመጃ እሽጎችን በሚያድሱበት ጊዜ ከመከታተያው ጋር።

## Runtime Lane Lifecycle

- የሩጫ መስመር የህይወት ዑደት ዕቅዶች አሁን የውሂብ ቦታ ማሰሪያዎችን ያረጋግጣሉ እና መቼ ያቋርጣሉ
  የኩራ/የደረጃ ማከማቻ ማስታረቅ አልተሳካም፣ይህም ካታሎግ አልተለወጠም። የ
  ረዳቶች የተሸጎጡ የሌይን ማስተላለፊያዎችን ለጡረተኞች መስመሮች ይቆርጣሉ ስለዚህም የመሪ ውህደት
  የቆዩ ማረጋገጫዎችን እንደገና አይጠቀምም.
- ዕቅዶችን በI18NT0000008X ውቅር/የህይወት ረዳቶች (`State::apply_lane_lifecycle`፣
  `Queue::apply_lane_lifecycle`) እንደገና ሳይጀመር መስመሮችን ለመጨመር / ለመልቀቅ; ማዘዋወር፣
  የTEU ቅጽበተ-ፎቶዎች እና የሰነድ መዝገብ ቤቶች ከተሳካ እቅድ በኋላ በራስ-ሰር እንደገና ይጫናሉ።
- የኦፕሬተር መመሪያ፡ ዕቅዱ ሳይሳካ ሲቀር፣ የጎደሉ የውሂብ ቦታዎችን ወይም ማከማቻዎችን ያረጋግጡ
  ሊፈጠሩ የማይችሉ ሥሮች (የደረደሩ የቀዝቃዛ ሥር/የኩራ ሌይን ማውጫዎች)። አስተካክል።
  የመደገፊያ መንገዶች እና እንደገና ይሞክሩ; የተሳካላቸው ዕቅዶች የሌይን/የዳታ ቦታ ቴሌሜትሪ እንደገና ይለቃሉ
  diff ስለዚህ ዳሽቦርዶች አዲሱን ቶፖሎጂ ያንፀባርቃሉ።

## NPoS ቴሌሜትሪ እና የኋላ ግፊት ማስረጃ

የPhaseB ማስጀመሪያ-የልምምድ ሬትሮ የሚወስን ቴሌሜትሪ ጠየቀ
የNPoS የልብ ምት መቆጣጠሪያ እና የሐሜት ሽፋኖች በጀርባ ግፊታቸው ውስጥ መቆየታቸውን ያረጋግጡ
ገደቦች. የውህደት ማሰሪያው በ
`integration_tests/tests/sumeragi_npos_performance.rs` ልምምድ ያደርጋል
ሁኔታዎች እና JSON ማጠቃለያዎችን ያወጣል (`sumeragi_baseline_summary::<scenario>::…`)
አዲስ መለኪያዎች በሚያርፉበት ጊዜ። በአካባቢው ያካሂዱት፡-

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS`፣ `SUMERAGI_NPOS_STRESS_COLLECTORS_K` አዘጋጅ፣ ወይም
`SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` ከፍተኛ ውጥረት ያለባቸውን ቶፖሎጂዎችን ለመመርመር; የ
ነባሪዎች B4 ውስጥ ጥቅም ላይ የዋለውን 1s/I18NI0000078X ሰብሳቢ መገለጫ ያንፀባርቃሉ።

| ሁኔታ / ፈተና | ሽፋን | ቁልፍ ቴሌሜትሪ |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | የማስረጃ ጥቅሉን በተከታታይ ከማዘጋጀቱ በፊት የኤኤምኤ የቆይታ ፖስታዎችን፣ የወረፋ ጥልቀቶችን እና ተደጋጋሚ መላክ መለኪያዎችን ለመመዝገብ 12 ዙሮችን ከልምምድ ጊዜ ጋር ያግዳል። | `sumeragi_phase_latency_ema_ms`፣ `sumeragi_collectors_k`፣ `sumeragi_redundant_send_r`፣ `sumeragi_bg_post_queue_depth*`። |
| `npos_queue_backpressure_triggers_metrics` | የግብይት ወረፋውን ያጥለቀለቀው የመግቢያ መዘግየት በቆራጥነት መጀመሩን እና ወረፋው የአቅም/የሙሌት ቆጣሪዎችን ወደ ውጭ መላክ ነው። | `sumeragi_tx_queue_depth`፣ `sumeragi_tx_queue_capacity`፣ `sumeragi_tx_queue_saturated`፣ `sumeragi_pacemaker_backpressure_deferrals_total`፣ `sumeragi_rbc_backpressure_deferrals_total`። |
| `npos_pacemaker_jitter_within_band` | የተቀናበረው ±125‰ ባንድ መተግበሩን እስኪያረጋግጥ ድረስ የልብ ምት ሰሪ ጂተርን ያሳዩ እና የጊዜ ማብቂያዎችን ይመልከቱ። | `sumeragi_pacemaker_jitter_ms`፣ `sumeragi_pacemaker_view_timeout_target_ms`፣ `sumeragi_pacemaker_jitter_frac_permille`። |
| `npos_rbc_store_backpressure_records_metrics` | ክፍለ-ጊዜዎችን እና ባይት ቆጣሪዎችን መውጣት፣ መመለስ እና መደብሩን ሳይሞላው እንዲረጋጋ ለማድረግ ትላልቅ RBC የሚጫኑ ጭነቶችን ወደ ለስላሳ/ደረቅ የመደብር ገደቦች ይገፋል። | `sumeragi_rbc_store_pressure`፣ `sumeragi_rbc_store_sessions`፣ `sumeragi_rbc_store_bytes`፣ `sumeragi_rbc_backpressure_deferrals_total`። |
| `npos_redundant_send_retries_update_metrics` | ድጋሚ ለማስተላለፍ ያስገድዳል ስለዚህ የድግግሞሽ-መላክ ጥምርታ መለኪያዎች እና ሰብሳቢዎች-በዒላማ ቆጣሪዎች ወደፊት እንዲራመዱ ያስገድዳል፣ ይህም ሬትሮ የተጠየቀው ቴሌሜትሪ ከጫፍ እስከ ጫፍ የተገጠመ መሆኑን ያረጋግጣል። | `sumeragi_collectors_targeted_current`፣ `sumeragi_redundant_sends_total`። |
| `npos_rbc_chunk_loss_fault_reports_backlog` | የኋላ ሎግ ማሳያዎችን ለማረጋገጥ በቆራጥነት የተራራቁ ቁርጥራጮችን ይጥላል። | `sumeragi_rbc_backlog_sessions_pending`፣ `sumeragi_rbc_backlog_chunks_total`፣ `sumeragi_rbc_backlog_chunks_max`። |የJSON መስመሮችን የታጠቁ ህትመቶችን ከ I18NT0000000X ቧጨራ ጋር ያያይዙ
በሩጫ ወቅት በቁጥጥር ስር ውለዋል
ማንቂያዎች ከልምምድ ቶፖሎጂ ጋር ይዛመዳሉ።

## የማረጋገጫ ዝርዝር አዘምን

1. አዲስ የተዘዋወሩ መስኮቶችን ጨምር እና ሩብ ክፍል ሲንከባለል አሮጌዎቹን ጡረታ አስወጣ።
2. ከእያንዳንዱ የአለርትማኔጀር ክትትል በኋላ የመቀነስ ሰንጠረዡን ያዘምኑ፣ ምንም እንኳን የ
   እርምጃ ትኬቱን መዝጋት ነው።
3. ውቅር ዴልታስ ሲቀየር፣ መከታተያውን፣ ይህን ማስታወሻ እና ቴሌሜትሪውን ያዘምኑ
   በተመሳሳዩ የመሳብ ጥያቄ ውስጥ የምግብ መፍጫ ዝርዝርን ያሽጉ።
4. ማንኛውንም አዲስ የመለማመጃ/የቴሌሜትሪ ቅርሶችን እዚህ ያገናኙ ስለዚህ የወደፊት የመንገድ ካርታ ሁኔታ
   ዝማኔዎች ከተበታተኑ የማስታወቂያ ማስታወሻዎች ይልቅ አንድን ሰነድ ሊጠቅሱ ይችላሉ።

## ማስረጃ ኢንዴክስ

| ንብረት | አካባቢ | ማስታወሻ |
|-------|----------|-------|
| የዱካ ክትትል ኦዲት ሪፖርት (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | ቀኖናዊ ምንጭ ለ PhaseB1 ማስረጃ; በ `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` ስር ላለው ፖርታል የተንጸባረቀ። |
| የዴልታ መከታተያ አዋቅር | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | የ TRACE-CONFIG-DELTA ልዩነት ማጠቃለያዎች፣ የገምጋሚ ፊደሎች እና GOV-2026-03-19 የድምፅ ምዝግብ ማስታወሻ ይዟል። |
| የቴሌሜትሪ ማስተካከያ እቅድ | `docs/source/nexus_telemetry_remediation_plan.md` | የማንቂያ ጥቅሉን፣ የOTLP ባች መጠንን እና ከ B2 ጋር የተሳሰሩ የበጀት መከላከያ መንገዶችን ወደ ውጭ ይላካል። |
| ባለብዙ መስመር መለማመጃ መከታተያ | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | የኤፕሪ9 የመለማመጃ ቅርሶች፣ አረጋጋጭ መግለጫ/መፍጨት፣ የQ2 መሰናዶ ማስታወሻዎች/አጀንዳ፣ እና የመመለሻ ማስረጃዎችን ይዘረዝራል። |
| ቴሌሜትሪ ጥቅል አንጸባራቂ/መፍጨት (የቅርብ ጊዜ) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | መዛግብት ማስገቢያ ክልል 912-936, ዘር `NEXUS-REH-2026Q2`, እና artefact hashes ለ አስተዳደር ቅርቅቦች. |
| TLS መገለጫ አንጸባራቂ | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | በQ2 ድጋሚ ሲካሄድ የተያዘው የተፈቀደው የTLS መገለጫ ሃሽ፤ በተዘዋዋሪ-ዱካ አባሪዎች ውስጥ ጥቀስ። |
| ዱካ-ባለብዙ-ካናሪ አጀንዳ | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | ለQ2 ልምምዱ (መስኮት፣ ማስገቢያ ክልል፣ የስራ ጫና ዘር፣ የድርጊት ባለቤቶች) የእቅድ ማስታወሻዎች። |
| የልምምድ runbook አስጀምር | `docs/source/runbooks/nexus_multilane_rehearsal.md` | የአሠራር ማረጋገጫ ዝርዝር ለማሰናዳት → አፈፃፀም → መልሶ መመለስ; የሌይን ቶፖሎጂ ወይም ላኪ መመሪያ ሲቀየር ማዘመን። |
| ቴሌሜትሪ ጥቅል አረጋጋጭ | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI በ B4 retro የተጠቀሰው; ማሸጊያው በተቀየረ ቁጥር ማህደር ከክትትል ጋር ይዋሃዳል። |
| ባለብዙ መስመር መመለሻ | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | `nexus.enabled = true` ለባለብዙ ሌይን ውቅሮች ያረጋግጣል፣የሶራ ካታሎግ ሃሽዎችን ይጠብቃል እና ሌይን-አካባቢያዊ Kura/merge-log paths (`blocks/lane_{id:03}_{slug}`) በ`ConfigLaneRouter` በኩል አርቲፊሻል ዲጀስትስ ከማተም በፊት ያቀርባል። |