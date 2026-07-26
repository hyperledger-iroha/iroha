---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9174f3d9559c8656bbe8062e64fc22d93ad654347409798f29231d09ba1628e6
source_last_modified: "2026-01-05T09:28:11.903551+00:00"
translation_last_reviewed: 2026-02-07
id: orchestrator-ops
title: SoraFS Orchestrator Operations Runbook
sidebar_label: Orchestrator Ops Runbook
description: Step-by-step operational guide for rolling out, monitoring, and rolling back the multi-source orchestrator.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

ይህ Runbook የባለብዙ-ምንጭ ፈልሳፊ ኦርኬስትራውን በማዘጋጀት፣ በመልቀቅ እና በማንቀሳቀስ SREዎችን ይመራቸዋል። የገንቢ መመሪያውን ለምርት ልቀቶች ከተስተካከሉ ሂደቶች ጋር ያሟላል።

> ** በተጨማሪ ይመልከቱ፡** የ[ባለብዙ-ምንጭ ልቀት Runbook](./multi-source-rollout.md) የሚያተኩረው መርከቦችን በሚዘረጋ የታቀፉ ሞገዶች እና የድንገተኛ አደጋ አቅራቢዎች ውድቅ ላይ ነው። ይህንን ሰነድ ለዕለት ተዕለት የኦርኬስትራ ስራዎች በሚጠቀሙበት ጊዜ ለአስተዳደር/ዝግጅት ማስተባበር ያመልክቱ።

## 1. የቅድመ በረራ ማረጋገጫ ዝርዝር

1. **የአቅራቢ ግብአቶችን ሰብስብ**
   - ለታለመው መርከቦች የቅርብ ጊዜ የአቅራቢ ማስታወቂያዎች (`ProviderAdvertV1`) እና የቴሌሜትሪ ቅጽበታዊ ገጽ እይታ።
   - የመጫኛ እቅድ (I18NI0000004X) በሙከራ ላይ ካለው አንጸባራቂ የተገኘ።
2. ** የሚወስን የውጤት ሰሌዳ ይስጡ**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - I18NI0000005X እያንዳንዱን የምርት አቅራቢ እንደ `eligible` መዘረዘሩን ያረጋግጡ።
   - JSON ማጠቃለያውን ከውጤት ሰሌዳው ጋር በማህደር ያስቀምጡ; ኦዲተሮች የለውጥ ጥያቄውን በሚያረጋግጡበት ጊዜ እንደገና በመሞከር ቆጣሪዎች ላይ ይተማመናሉ።
3. ** በደረቅ አሂድ ከመሳሪያዎች ጋር *** - የኦርኬስትራ ሁለትዮሽ የምርት ጭነት ጭነት ከመንካትዎ በፊት ከተጠበቀው ስሪት ጋር መዛመዱን ለማረጋገጥ በ `docs/examples/sorafs_ci_sample/` ውስጥ በሕዝብ መጫዎቻዎች ላይ ተመሳሳይ ትእዛዝ ያድርጉ።

## 2. የታቀደ ልቀት ሂደት

1. ** የካናሪ ደረጃ (≤2 አቅራቢዎች)**
   - የውጤት ሰሌዳውን እንደገና ይገንቡ እና ኦርኬስትራውን ወደ ትንሽ ንዑስ ስብስብ ለመጠቅለል በ `--max-peers=2` ያሂዱ።
   - ተቆጣጠር:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - አንድ ጊዜ ቀጥል ለተሟላ አንጸባራቂ ለማምጣት ተመኖች ከ1% በታች ይቀራሉ እና ምንም አቅራቢ ውድቀቶችን አያከማችም።
2. **የራምፕ ደረጃ (50% አቅራቢዎች)**
   - `--max-peers` ይጨምሩ እና በአዲስ የቴሌሜትሪ ቅጽበታዊ ፎቶ ያሂዱ።
   - እያንዳንዱን ሩጫ በI18NI0000013X እና `--chunk-receipts-out` ቀጥል። ቅርሶቹን ለ≥7 ቀናት ያቆዩ።
3. **ሙሉ ልቀት**
   - `--max-peers` አስወግድ (ወይም ወደ ሙሉ ብቁ ቆጠራ ያዋቅሩት)።
   - በደንበኛ ማሰማራቶች ውስጥ የኦርኬስትራ ሁነታን ያንቁ፡ የቀጠለውን የውጤት ሰሌዳ እና ውቅር JSON በእርስዎ የውቅር አስተዳደር ስርዓት በኩል ያሰራጩ።
   - `sorafs_orchestrator_fetch_duration_ms` p95/p99 ለማሳየት ዳሽቦርዶችን ያዘምኑ እና በየክልሉ ሂስቶግራም ይሞክሩ።

## 3. የእኩዮች መመዝገብ እና መጨመር

የአስተዳደር ዝመናዎችን ሳይጠብቁ ጤናማ ያልሆኑ አቅራቢዎችን ለመለየት የCLI የውጤት አሰጣጥ ፖሊሲን ይጠቀሙ።

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` ለአሁኑ ክፍለ ጊዜ ከግምት ውስጥ የተዘረዘሩትን ተለዋጭ ስሞች ያስወግዳል።
- `--boost-provider=<alias>=<weight>` የአቅራቢውን የጊዜ ሰሌዳ ክብደት ከፍ ያደርገዋል. እሴቶች ለተለመደው የውጤት ሰሌዳ ክብደት ተጨማሪ ናቸው እና ለአካባቢው ሩጫ ብቻ ይተገበራሉ።
- በአደጋው ​​ትኬት ውስጥ መሻሮችን ይመዝግቡ እና የJSON ውጤቶችን ያያይዙ ስለዚህ ዋናው ጉዳይ ከተስተካከለ በኋላ የባለቤትነት ቡድኑ ሁኔታውን ማስታረቅ ይችላል።

ለቋሚ ለውጦች የCLI መሻሮችን ከማጽዳትዎ በፊት የምንጭ ቴሌሜትሪውን ያሻሽሉ (ወንጀለኛው እንደተቀጣ ምልክት ያድርጉበት) ወይም ማስታወቂያውን በተዘመኑ የዥረት በጀቶች ያድሱ።

## 4. አለመሳካት Triage

ማምጣት ሳይሳካ ሲቀር፡-

1. ከመድገምዎ በፊት የሚከተሉትን ቅርሶች ይያዙ፡-
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. በሰው ሊነበብ ለሚችለው የስህተት ሕብረቁምፊ I18NI0000023X መርምር፡
   - `no providers were supplied` → የአቅራቢ መንገዶችን እና ማስታወቂያዎችን ያረጋግጡ።
   - `retry budget exhausted ...` → `--retry-budget` ይጨምሩ ወይም ያልተረጋጉ እኩዮችን ያስወግዱ።
   - `no compatible providers available ...` → የአጥፊውን አቅራቢ ክልል አቅም ሜታዳታ ኦዲት ያድርጉ።
3. የአቅራቢውን ስም ከ`sorafs_orchestrator_provider_failures_total` ጋር ያዛምዱ እና ሜትሪክ ሹል ከሆነ የመከታተያ ትኬት ይፍጠሩ።
4. ውድቀቱን በቆራጥነት ለማባዛት ማውጣቱን ከመስመር ውጭ በ`--scoreboard-json` እና በተያዘው ቴሌሜትሪ እንደገና ያጫውቱ።

## 5. ወደ ኋላ መመለስ

የኦርኬስትራ ልቀትን ለመመለስ፡-

2. የውጤት ሰሌዳው ወደ ገለልተኛ ክብደት እንዲመለስ ማንኛውንም የI18NI0000030X መሻሮችን ያስወግዱ።
3. ምንም ቀሪ ፈልሳፊዎች በበረራ ላይ አለመኖራቸውን ለማረጋገጥ ቢያንስ ለአንድ ቀን የኦርኬስትራ መለኪያዎችን መቧቀስዎን ይቀጥሉ።

በሥነ-ሥርዓት የታገዘ የቅርስ ቀረጻ እና የታቀዱ ልቀቶችን ማቆየት የባለብዙ ምንጭ ኦርኬስትራ በተለያዩ አቅራቢ መርከቦች ላይ ደህንነቱ በተጠበቀ ሁኔታ እንዲሠራ እና የታዛቢነት እና የኦዲት መስፈርቶችን ጠብቆ እንዲቆይ ያረጋግጣል።