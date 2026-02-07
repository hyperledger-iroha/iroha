---
id: multi-source-rollout
lang: am
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Multi-Source Client Rollout & Blacklisting Runbook
sidebar_label: Multi-Source Rollout Runbook
description: Operational checklist for staged multi-source rollouts and emergency provider blacklisting.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

#ዓላማ

ይህ Runbook SRE እና የጥሪ መሐንዲሶችን በሁለት ወሳኝ የስራ ፍሰቶች ይመራቸዋል፡

1. የባለብዙ ምንጭ ኦርኬስትራውን በተቆጣጠሩት ሞገዶች ውስጥ ማስወጣት.
2. ያሉትን ክፍለ-ጊዜዎች ሳያስታውሱ የተሳሳቱ ባህሪ አቅራቢዎችን መጥቀስ ወይም ቅድሚያ መስጠት።

በSF-6 ስር የቀረበው የኦርኬስትራ ቁልል አስቀድሞ ተዘርግቷል (`sorafs_orchestrator`፣ ጌትዌይ ቸንክ-ሬንጅ ኤፒአይ፣ የቴሌሜትሪ ላኪዎች) ያስባል።

> ** በተጨማሪ ይመልከቱ፡** [የኦርኬስትራ ኦፕሬሽንስ Runbook](./orchestrator-ops.md) በየሩጫ ሂደቶች (የውጤት ሰሌዳ ቀረጻ፣ የታቀዱ ልቀቶች መቀያየር፣ ጥቅልል ​​መመለስ) ውስጥ ዘልቆ ይገባል። በቀጥታ ለውጦች ጊዜ ሁለቱንም ማጣቀሻዎች አንድ ላይ ተጠቀም።

## 1. የቅድመ በረራ ማረጋገጫ

1. **የአስተዳደር ግብአቶችን ያረጋግጡ።**
   - ሁሉም እጩ አቅራቢዎች የ `ProviderAdvertV1` ፖስታዎችን ከክልል አቅም ሸክሞች እና የዥረት በጀት ጋር ማተም አለባቸው። በI18NI0000008X በኩል ያረጋግጡ እና ከሚጠበቁ የችሎታ መስኮች ጋር ያወዳድሩ።
   - የቴሌሜትሪ ቅጽበታዊ ገጽ እይታዎች የማዘግየት/የመውደቅ ተመኖችን የሚያቀርቡት ከእያንዳንዱ የካናሪ ሩጫ በፊት <15 ደቂቃ መሆን አለበት።
2. **የደረጃ ውቅር።**
   - ኦርኬስትራውን JSON አወቃቀሩን በተነባበረው I18NI0000009X ዛፍ ላይ ያቆዩት፡

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     በታቀደ-ተኮር ገደቦች (`max_providers`፣ በጀቶችን እንደገና ይሞክሩ) JSONን ያዘምኑ። ልዩነቱ ትንሽ ሆኖ እንዲቆይ ተመሳሳዩን ፋይል ወደ ዝግጅት/ምርት ይመግቡ።
3. ** ቀኖናዊ ዕቃዎችን የአካል ብቃት እንቅስቃሴ ያድርጉ።
   - የአንጸባራቂ/የማስመሰያ አካባቢ ተለዋዋጮችን ይሙሉ እና ወሳኙን ማምጣት ያሂዱ፡-

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     የአካባቢ ተለዋዋጮች በካናሪ ውስጥ ለሚሳተፈው ለእያንዳንዱ አቅራቢ አንጸባራቂ የክፍያ ዳይጄስት (ሄክስ) እና ቤዝ64-ኢንኮድ የተደረገ ዥረት ቶከን መያዝ አለባቸው።
   - ከቀዳሚው ልቀት አንፃር I18NI00000011 ልዩነት። ማንኛውም አዲስ ብቁ ያልሆነ አቅራቢ ወይም የክብደት ለውጥ>10% ግምገማ ያስፈልገዋል።
4. ** ቴሌሜትሪ በሽቦ መያዙን ያረጋግጡ።**
   - የGrafana ኤክስፖርትን በI18NI0000012X ይክፈቱ። ከሂደቱ በፊት የ`sorafs_orchestrator_*` ሜትሪክስ በዝግጅት ላይ መሞላቱን ያረጋግጡ።

## 2. የአደጋ ጊዜ አቅራቢ ጥቁር መዝገብ

አቅራቢው የተበላሹ ክፍሎችን ሲያገለግል፣ ያለማቋረጥ ጊዜ ሲያልቅ ወይም የተገዢነት ማረጋገጫዎችን ሲያጣ ይህን አሰራር ይከተሉ።

1. **ማስረጃ ይያዙ።**
   - የቅርብ ጊዜ ማምጣት ማጠቃለያ (`--json-out` ውፅዓት) ወደ ውጭ ላክ። ያልተሳኩ የትኩረት ኢንዴክሶችን፣ የአቅራቢ ስሞችን ይመዝግቡ እና አለመዛመጃዎችን መፍጨት።
   - ተዛማጅነት ያላቸውን የምዝግብ ማስታወሻዎች ከ `telemetry::sorafs.fetch.*` ዒላማዎች ያስቀምጡ።
2. ** ወዲያውኑ መሻርን ይተግብሩ።**
   - አቅራቢውን ለኦርኬስትራ በተሰራጨው የቴሌሜትሪ ቅጽበታዊ ገጽ እይታ (`penalty=true` ወይም clamp `token_health` to `0`) ላይ ምልክት ያድርጉ። የሚቀጥለው የውጤት ሰሌዳ ግንባታ አቅራቢውን በራስ-ሰር ያስወግዳል።
   - ለአድሆክ ጭስ ሙከራዎች `--deny-provider gw-alpha` ወደ `sorafs_cli fetch` ያስተላልፉ ስለዚህ የቴሌሜትሪ ስርጭትን ሳይጠብቁ ውድቀት መንገዱ ይከናወናል።
   - የተዘመነውን የቴሌሜትሪ/የውቅረት ቅርቅብ ለተጎዳው አካባቢ (ማዘጋጀት → ካናሪ → ምርት) እንደገና ማሰማራት። በአደጋው ​​መዝገብ ውስጥ ያለውን ለውጥ ይመዝግቡ።
3. ** መሻርን ያረጋግጡ።**
   - ቀኖናዊውን መፈልፈያ እንደገና ያሂዱ። የውጤት ሰሌዳውን ያረጋግጡ አቅራቢውን በምክንያት `policy_denied`።
   - ቆጣሪው ለተከለከለው አቅራቢ መጨመር መቆሙን ለማረጋገጥ `sorafs_orchestrator_provider_failures_total`ን ይፈትሹ።
4. ** ለረጅም ጊዜ የሚቆዩ እገዳዎች መጨመር.
   - አቅራቢው ለ> 24 ሰአት እንደታገደ የሚቆይ ከሆነ፣ ማስታወቂያውን ለማሽከርከር ወይም ለማገድ የአስተዳደር ትኬት ከፍ ያድርጉ። ድምጹ እስኪያልፍ ድረስ፣ አቅራቢው ወደ የውጤት ሰሌዳው እንዳይገባ የክህደት ዝርዝሩን በቦታው ያስቀምጡ እና የቴሌሜትሪ ቅጽበተ-ፎቶዎችን ያድሱ።
5. **የመመለሻ ፕሮቶኮል**
   - አቅራቢውን ወደነበረበት ለመመለስ፣ ከተከለከለው ዝርዝር ውስጥ ያስወግዱት፣ እንደገና ያሰራጩ እና አዲስ የውጤት ሰሌዳ ቅጽበተ-ፎቶ ያንሱ። ለውጡን ከድንገተኛ ሞት በኋላ ያያይዙት።

## 3. የታቀደ ልቀት እቅድ

| ደረጃ | ወሰን | አስፈላጊ ምልክቶች | መሄድ/አለመሄድ መስፈርት |
|-------|--------|
| ** ላብ *** | የተዋሃደ ስብስብ | በእጅ CLI ከቋሚ ጭነቶች ጋር ማምጣት | ሁሉም ክፍሎች ተሳክተዋል፣ የአቅራቢዎች አለመሳካት ቆጣሪዎች በ0 ይቆያሉ፣ የድጋሚ ሙከራ መጠን <5% |
| ** መድረክ *** | ሙሉ ቁጥጥር-አይሮፕላን ዝግጅት | Grafana ዳሽቦርድ ተገናኝቷል; የማስጠንቀቂያ ደንቦች በማስጠንቀቂያ-ብቻ ሁነታ | `sorafs_orchestrator_active_fetches` ከእያንዳንዱ ሙከራ በኋላ ወደ ዜሮ ይመለሳል; ምንም `warn/critical` ማንቂያ ተኩስ. |
| **ካናሪ** | ≤10% የምርት ትራፊክ | ፔጀር ድምጸ-ከል ሆኗል ነገር ግን የቴሌሜትሪ ክትትል በእውነተኛ ሰዓት | ጥምርታ እንደገና ይሞክሩ <10%፣ የአቅራቢዎች ውድቀቶች ከሚታወቁ ጫጫታ እኩዮች ጋር ተነጥለው፣የዘገየ ሂስቶግራም ከመሠረታዊ መስመር ±20% ጋር ይዛመዳል። |
| ** አጠቃላይ ተገኝነት *** | 100% መልቀቅ | Pager ሕጎች ንቁ | ዜሮ `NoHealthyProviders` ስህተቶች ለ 24h, እንደገና ይሞክሩ ሬሾ የተረጋጋ, ዳሽቦርድ SLA ፓነሎች አረንጓዴ. |

ለእያንዳንዱ ደረጃ:

1. ኦርኬስትራውን JSON በታሰበው `max_providers` ያዘምኑ እና በጀቶችን እንደገና ይሞክሩ።
2. `sorafs_cli fetch` ወይም የኤስዲኬ ውህደት ሙከራ ስብስብን ከቀኖናዊው አካል እና ከአካባቢው ከሚገለጽ ተወካይ ጋር ያሂዱ።
3. የውጤት ሰሌዳ + ማጠቃለያ ቅርሶችን ያንሱ እና ከተለቀቀው መዝገብ ጋር አያይዟቸው።
4. ወደ ቀጣዩ ደረጃ ከማስተዋወቅዎ በፊት የቴሌሜትሪ ዳሽቦርዶችን በጥሪ መሐንዲስ ይከልሱ።

## 4. ታዛቢነት እና ክስተት መንጠቆዎች

- ** መለኪያዎች፡** የአለርትማኔጀር `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` እና `sorafs_orchestrator_retries_total` መከታተላቸውን ያረጋግጡ። ድንገተኛ ስፒል በተለምዶ አቅራቢው በጭነት ውስጥ እያዋረደ ነው ማለት ነው።
- ** ምዝግብ ማስታወሻዎች፡** የ`telemetry::sorafs.fetch.*` ኢላማዎችን ወደ የተጋራው የምዝግብ ማስታወሻ ሰብሳቢ ያዙሩ። መለያየትን ለማፋጠን ለ`event=complete status=failed` የተቀመጡ ፍለጋዎችን ይገንቡ።
- ** የውጤት ሰሌዳዎች: *** እያንዳንዱን የውጤት ሰሌዳ አርቲፊኬት ለረጅም ጊዜ ማከማቻ ያቆዩ። JSON ለታዛዥ ግምገማዎች እና የታቀዱ መልሶ ማገገሚያዎች እንደ የማስረጃ መንገድ በእጥፍ ይጨምራል።
- ** ዳሽቦርዶች: ** ቀኖናዊውን I18NT0000002X ሰሌዳ (`docs/examples/sorafs_fetch_dashboard.json`) ወደ የምርት አቃፊ ከ `docs/examples/sorafs_fetch_alerts.yaml` የማስጠንቀቂያ ደንቦች ጋር ያዙሩ።

## 5. ግንኙነት እና ሰነድ

- በኦፕሬሽኖች ለውጥ ሎግ ውስጥ ያለውን እያንዳንዱን እምቢታ/የማሳደግ ለውጥ በጊዜ ማህተም፣ ከዋኝ፣ በምክንያት እና በተዛመደ ክስተት ይመዝገቡ።
- የአቅራቢ ክብደቶች ወይም በጀቶች ከደንበኛ-ጎን የሚጠበቁትን ለማጣጣም ሲቀየሩ ለኤስዲኬ ቡድኖች ያሳውቁ።
- GA ከተጠናቀቀ በኋላ `status.md`ን ከታቀደው ማጠቃለያ ጋር ያዘምኑ እና ይህንን የሩጫ መጽሐፍ ማጣቀሻ በመልቀቂያ ማስታወሻዎች ውስጥ ያስቀምጡ።