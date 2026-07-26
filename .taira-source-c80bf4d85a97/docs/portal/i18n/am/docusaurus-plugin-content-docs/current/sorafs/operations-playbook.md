---
id: operations-playbook
lang: am
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Operations Playbook
sidebar_label: Operations Playbook
description: Incident response guides and chaos drill procedures for SoraFS operators.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
ይህ ገጽ በ`docs/source/sorafs_ops_playbook.md` ስር የተቀመጠውን የሩጫ መጽሐፍ ያንጸባርቃል። የ Sphinx ሰነድ ስብስብ ሙሉ በሙሉ እስኪሰደድ ድረስ ሁለቱንም ቅጂዎች በማመሳሰል ያቆዩዋቸው።
::

## ቁልፍ ማጣቀሻዎች

- ታዛቢነት ያላቸው ንብረቶች፡ በ `dashboards/grafana/` እና Prometheus የማስጠንቀቂያ ደንቦች ስር ያሉትን Grafana ዳሽቦርዶችን በI18NI0000009X ይመልከቱ።
- ሜትሪክ ካታሎግ: `docs/source/sorafs_observability_plan.md`.
- ኦርኬስትራ ቴሌሜትሪ ንጣፎች: `docs/source/sorafs_orchestrator_plan.md`.

## Escalation ማትሪክስ

| ቅድሚያ | ቀስቅሴ ምሳሌዎች | የመጀመሪያ ደረጃ ጥሪ | ምትኬ | ማስታወሻ |
|-------------|--------|
| P1 | የአለም አቀፋዊ መግቢያ ማቋረጥ፣ የPoR ውድቀት መጠን > 5% (15 ደቂቃ)፣ የማባዛት የኋላ ሎግ በየ10 ደቂቃው በእጥፍ ይጨምራል | ማከማቻ SRE | ታዛቢነት TL | ተፅዕኖው ከ30 ደቂቃ በላይ ከሆነ የአስተዳደር ምክር ቤቱን ያሳትፉ። |
| P2 | ክልላዊ መግቢያ በር መዘግየት SLO መጣስ፣ ኦርኬስትራ ያለ SLA ተጽዕኖ ድጋሚ ይሞክሩ | ታዛቢነት TL | ማከማቻ SRE | ልቀቱን ቀጥል ግን አዲስ መገለጫዎችን አስገባ። |
| P3 | ወሳኝ ያልሆኑ ማንቂያዎች (የግልፅ ቆይታ፣ አቅም 80–90%) | የመግቢያ ልዩነት | Ops ጓድ | በሚቀጥለው የስራ ቀን ውስጥ አድራሻ |

## የጌትዌይ መቋረጥ / የተበላሸ ተገኝነት

** ማወቂያ ***

- ማንቂያዎች፡ I18NI0000012X፣ `SoraFSGatewayLatencySlo`።
- ዳሽቦርድ: `dashboards/grafana/sorafs_gateway_overview.json`.

** አፋጣኝ እርምጃዎች ***

1. ወሰን ያረጋግጡ (ነጠላ አቅራቢ እና ፍሊት) በጥያቄ-ተመን ፓነል በኩል።
2. በኦፕስ ውቅረት (`docs/source/sorafs_gateway_self_cert.md`) `sorafs_gateway_route_weights` በመቀያየር Torii ወደ ጤናማ አቅራቢዎች (ባለብዙ አቅራቢ ከሆነ) ቀይር።
3. ሁሉም አቅራቢዎች ተጽዕኖ ካደረጉ፣ ለCLI/SDK ደንበኞች (`docs/source/sorafs_node_client_protocol.md`) “ቀጥታ ማምጣት” ውድቀትን አንቃ።

**ትሪጅ**

- የዥረት ማስመሰያ አጠቃቀምን ከ`sorafs_gateway_stream_token_limit` ጋር ያረጋግጡ።
- የመግቢያ መዝገቦችን ለTLS ወይም የመግቢያ ስህተቶችን ይፈትሹ።
- ወደ ውጭ የተላከው የአገናኝ መንገዱ ንድፍ ከተጠበቀው ስሪት ጋር እንደሚመሳሰል ለማረጋገጥ `scripts/telemetry/run_schema_diff.sh` ን ያሂዱ።

**የማስተካከያ አማራጮች**

- የተጎዳውን የመግቢያ ሂደት ብቻ እንደገና ያስጀምሩ; ብዙ አቅራቢዎች ካልተሳኩ በስተቀር ሙሉውን ክላስተር እንደገና ጥቅም ላይ ማዋልን ያስወግዱ።
- ሙሌት ከተረጋገጠ የዥረት ማስመሰያ ገደቡን በጊዜያዊነት ከ10-15% ይጨምሩ።
- ከተረጋጋ በኋላ የራስ ሰር ማረጋገጫ (`scripts/sorafs_gateway_self_cert.sh`) እንደገና ያሂዱ።

**ከአደጋ በኋላ**

- `docs/source/sorafs/postmortem_template.md` በመጠቀም የ P1 ድህረ ሞት ያስገቡ።
- ማሻሻያው በእጅ በሚደረጉ ጣልቃገብነቶች ላይ ከተመረኮዘ የክትትል ትርምስ መሰርሰሪያን መርሐግብር ያስይዙ።

## አለመሳካቱን የሚያረጋግጥ ስፒክ (PoR/PoTR)

** ማወቂያ ***

- ማንቂያዎች፡ I18NI0000022X፣ `SoraFSPoTRDeadlineMiss`።
- ዳሽቦርድ: I18NI0000024X.
- ቴሌሜትሪ: I18NI0000025X እና I18NI0000026X ክስተቶች ከ I18NI0000027X ጋር።

** አፋጣኝ እርምጃዎች ***

1. የአንጸባራቂውን መዝገብ (`docs/source/sorafs/manifest_pipeline.md`) በማመልከት አዲስ የሰነድ መግባቶችን ያቁሙ።
2. ለተጎዱ አቅራቢዎች የሚሰጠውን ማበረታቻ ለአፍታ እንዲያቆም ለመንግስት ያሳውቁ።

**ትሪጅ**

- የPoR ፈታኝ ወረፋ ጥልቀት ከ `sorafs_node_replication_backlog_total` ጋር ያረጋግጡ።
- በቅርብ ጊዜ ለተሰማሩ የማረጋገጫ ቧንቧ መስመር (`crates/sorafs_node/src/potr.rs`) ያረጋግጡ።
- የአቅራቢውን የጽኑ ትዕዛዝ ስሪቶች ከኦፕሬተር መዝገብ ቤት ጋር ያወዳድሩ።

**የማስተካከያ አማራጮች**

- `sorafs_cli proof stream` በመጠቀም የPoR ድግግሞሾችን ከቅርብ ጊዜው አንጸባራቂ ጋር።
- ማስረጃዎች በተከታታይ ካልተሳኩ የአስተዳደር መዝገብ ቤቱን በማዘመን እና የኦርኬስትራ የውጤት ሰሌዳዎችን እንዲያድሱ በማስገደድ አቅራቢውን ከገባሪው ያስወግዱት።

**ከአደጋ በኋላ**

- ከሚቀጥለው የምርት ማሰማራቱ በፊት የPoR chaos drill scenarioን ያሂዱ።
- በድህረ ሞት አብነት ውስጥ ትምህርቶችን ይያዙ እና የአቅራቢውን የብቃት ማረጋገጫ ዝርዝር ያዘምኑ።

## የማባዛት መዘግየት / የኋላ እድገት

** ማወቂያ ***

- ማንቂያዎች: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. አስመጣ
  `dashboards/alerts/sorafs_capacity_rules.yml` እና አሂድ
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  ማስተዋወቂያ ከመደረጉ በፊት Alertmanager በሰነድ የተቀመጡትን ገደቦች ያንፀባርቃል።
- ዳሽቦርድ: `dashboards/grafana/sorafs_capacity_health.json`.
- መለኪያዎች: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

** አፋጣኝ እርምጃዎች ***

1. የኋላ መዝገብ ወሰን (ነጠላ አቅራቢ ወይም መርከቦች) ያረጋግጡ እና አስፈላጊ ያልሆኑ የማባዛት ስራዎችን ለአፍታ ያቁሙ።
2. የኋላ መዝገብ የተገለለ ከሆነ፣ በጊዜያዊነት አዲስ ትዕዛዞችን ለተለዋጭ አቅራቢዎች በማባዛት መርሐግብር ይመድቡ።

**ትሪጅ**

- የኋላ ታሪክን ሊያበላሹ የሚችሉ ፍንዳታዎችን እንደገና ለመሞከር ኦርኬስትራ ቴሌሜትሪ ይፈትሹ።
- የማጠራቀሚያ ኢላማዎች በቂ ዋና ክፍል (`sorafs_node_capacity_utilisation_percent`) እንዳላቸው ያረጋግጡ።
- የቅርብ ጊዜ የውቅረት ለውጦችን ይገምግሙ (የመገለጫ ዝማኔዎች ፣ የማረጋገጫ ማረጋገጫ)።

**የማስተካከያ አማራጮች**

- ይዘትን እንደገና ለማሰራጨት `sorafs_cli`ን በ`--rebalance` አማራጭ ያሂዱ።
- ለተጎዳው አቅራቢው የማባዛት ሰራተኞችን በአግድም ያስመዝኑ።
- የቲቲኤል መስኮቶችን እንደገና ለማቀናጀት አንጸባራቂ እድሳት ያስነሱ።

**ከአደጋ በኋላ**

- በአቅራቢው ሙሌት ውድቀት ላይ የሚያተኩር የአቅም ቁፋሮ መርሐግብር ያስይዙ።
- በ `docs/source/sorafs_node_client_protocol.md` ውስጥ የማባዛት SLA ሰነዶችን ያዘምኑ።

## የኋላ መዝገብ እና SLA ጥሰቶችን መጠገን

** ማወቂያ ***

- ማንቂያዎች
  - `SoraFSRepairBacklogHigh` (የወረፋ ጥልቀት> 50 ወይም በጣም የቆየ ወረፋ ዕድሜ> 4 ሰ ለ 10 ሜትር)።
  - `SoraFSRepairEscalations` (> 3 escalations / ሰዓት).
  - `SoraFSRepairLeaseExpirySpike` (> 5 የኪራይ ውል የሚያልቅበት/ሰዓት)።
  - `SoraFSRetentionBlockedEvictions` (በመጨረሻው 15 ሜትር ውስጥ በንቁ ጥገናዎች ማቆየት ታግዷል).
- ዳሽቦርድ: I18NI0000047X.

** አፋጣኝ እርምጃዎች ***

1. የተጎዱ አቅራቢዎችን ይለዩ (የወረፋ ጥልቀት ነጠብጣቦች) እና ለእነሱ አዲስ ፒን / የማባዛት ትዕዛዞችን ለአፍታ ያቁሙ።
2. የጥገና ሰራተኛውን ህይወት ያረጋግጡ እና ደህንነቱ የተጠበቀ ከሆነ የሰራተኛውን ምንዛሬ ይጨምሩ።

**ትሪጅ**

- `torii_sorafs_repair_backlog_oldest_age_seconds` ከ 4h SLA መስኮት ጋር ያወዳድሩ።
- ለብልሽት/የሰዓት-skew ቅጦች `torii_sorafs_repair_lease_expired_total{outcome=...}`ን መርምር።
- ለተደጋጋሚ አንጸባራቂ/አቅራቢ ጥንዶች የተባባሱ ትኬቶችን ይገምግሙ እና የማስረጃ እሽጎችን ያረጋግጡ።

**የማስተካከያ አማራጮች**

- የቆሙ የጥገና ሠራተኞችን እንደገና መመደብ ወይም እንደገና ማስጀመር; በመደበኛ የይገባኛል ጥያቄ ፍሰት ወላጅ አልባ የኪራይ ውል ማፅዳት።
- ጥገናዎች ተጨማሪ SLA ጫና ለመከላከል ሲሉ አዲስ ካስማዎች ስሮት.
- ፍጥነቱ ከቀጠለ ወደ አስተዳደር እንሸጋገር እና የጥገና ኦዲት ቅርሶችን በማያያዝ።

## ማቆየት / ጂሲ ፍተሻ (ተነባቢ-ብቻ)

** ማወቂያ ***

- ማንቂያዎች፡ I18NI0000050X ወይም ቀጣይነት ያለው `torii_sorafs_storage_bytes_used`> 90%.
- ዳሽቦርድ: I18NI0000052X.

** አፋጣኝ እርምጃዎች ***

1. የአካባቢ ማቆያ ቅጽበተ-ፎቶን ያሂዱ፡-
   ```bash
   iroha app sorafs gc inspect --data-dir /var/lib/sorafs
   ```
2. የአገልግሎት ጊዜው ያለፈበት-ብቻ እይታን ያንሱ፡-
   ```bash
   iroha app sorafs gc dry-run --data-dir /var/lib/sorafs
   ```
3. ለኦዲትነት የJSON ውጤቶችን ከአደጋው ትኬት ጋር ያያይዙ።

**ትሪጅ**

- የትኛውን ሪፖርት እንደሚያሳይ ያረጋግጡ `retention_epoch=0` (የማለቂያ ጊዜ የለም) እና የግዜ ገደቦች ካላቸው ጋር።
- የትኛው ገደብ ውጤታማ እንደሆነ ለማየት በGC JSON ውፅዓት ውስጥ `retention_sources` ይጠቀሙ
  ማቆየት (`deal_end`፣ `governance_cap`፣ `pin_policy`፣ ወይም I18NI0000058X)። ስምምነት እና የአስተዳደር ገደቦች
  በአንጸባራቂ ሜታዳታ ቁልፎች `sorafs.retention.deal_end_epoch` እና
  `sorafs.retention.governance_cap_epoch`.
- የ`dry-run` ሪፖርቶች ጊዜያቸው ያለፈባቸው መገለጫዎች ነገር ግን አቅሙ እንደተሰካ ከቀጠለ፣ ምንም ያረጋግጡ
  ንቁ ጥገና ወይም ማቆየት ፖሊሲ ከቤት ማስወጣትን ያግዳል።
  በችሎታ የተቀሰቀሱ ጠራርጎዎች ማስወጣት ጊዜው አልፎበታል ቢያንስ በቅርብ ጊዜ ጥቅም ላይ በዋለ ትእዛዝ
  `manifest_id` ክራባት ሰሪዎች።

**የማስተካከያ አማራጮች**

- GC CLI ተነባቢ-ብቻ ነው። በምርት ውስጥ መግለጫዎችን ወይም ቁርጥራጮችን እራስዎ አይሰርዙ።
- የማቆያ ፖሊሲ ማስተካከያ ወይም የአቅም ማስፋፋት ወደ አስተዳደር ማደግ
  ጊዜው ያለፈበት መረጃ በራስ-ሰር ሳይወጣ ሲከማች።

## Chaos Drill Cadence

- ** በየሩብ ዓመቱ ***: የተዋሃደ የመግቢያ መንገድ መቋረጥ + ኦርኬስትራ የማዕበል ማስመሰልን እንደገና ይሞክሩ።
- **ሁለትዮሽ ***፡- የPoR/PoTR ውድቀት በሁለት አቅራቢዎች ከማገገም ጋር።
- ** ወርሃዊ የቦታ-ቼክ ***: የማሳያ መግለጫዎችን በመጠቀም የማባዛት መዘግየት ሁኔታ።
- ልምምዶችን በተጋራው የሩጫ መጽሐፍ ምዝግብ ማስታወሻ (`ops/drill-log.md`) በኩል ይከታተሉ፡-

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- ከመግባትዎ በፊት መዝገቡን ያረጋግጡ፡-

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- ለሚመጡት ልምምዶች `--status scheduled`፣ ለተጠናቀቁ ሩጫዎች `pass`/I18NI0000066X፣ እና የድርጊት እቃዎች ክፍት ሲሆኑ `follow-up` ይጠቀሙ።
- መድረሻውን በ I18NI0000068X ለደረቅ ሩጫዎች ወይም በራስ ሰር ማረጋገጥ; ያለ እሱ ስክሪፕቱ `ops/drill-log.md` ማዘመን ይቀጥላል።

## የድህረ ሞት አብነት

ለእያንዳንዱ P1/P2 ክስተት እና ለትርምስ መሰርሰሪያ የኋላ እይታዎች `docs/source/sorafs/postmortem_template.md` ይጠቀሙ። አብነቱ የጊዜ መስመርን፣ የተፅዕኖ አቆጣጠርን፣ አስተዋጽዖ ምክንያቶችን፣ የማስተካከያ እርምጃዎችን እና የክትትል ማረጋገጫ ስራዎችን ይሸፍናል።