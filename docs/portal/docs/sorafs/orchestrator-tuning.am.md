---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c14c7c9e998078f3f713b334556944759cd414fd8b7e22312f8731eadaf9345f
source_last_modified: "2026-01-22T14:45:01.319734+00:00"
translation_last_reviewed: 2026-02-07
id: orchestrator-tuning
title: Orchestrator Rollout & Tuning
sidebar_label: Orchestrator Tuning
description: Practical defaults, tuning guidance, and audit checkpoints for taking the multi-source orchestrator to GA.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# ኦርኬስትራ ልቀት እና ማስተካከያ መመሪያ

ይህ መመሪያ በ [የማዋቀር ማጣቀሻ](orchestrator-config.md) እና
[ባለብዙ-ምንጭ መልቀቅ runbook](multi-source-rollout.md)። ያስረዳል።
ኦርኬስትራውን ለእያንዳንዱ የታቀዱ ደረጃዎች እንዴት ማስተካከል እንደሚቻል ፣ እንዴት እንደሚተረጎም
የውጤት ሰሌዳ ቅርሶች፣ እና የትኞቹ የቴሌሜትሪ ምልክቶች ከዚህ በፊት በቦታው ላይ መሆን አለባቸው
የትራፊክ መስፋፋት. ምክሮቹን በCLI፣ ኤስዲኬዎች እና በወጥነት ተግብር
አውቶማቲክ ስለዚህ እያንዳንዱ መስቀለኛ መንገድ አንድ አይነት የመወሰን ፖሊሲን ይከተላል።

## 1. የመሠረት ፓራሜትር ስብስቦች

ከተጋራ ውቅር አብነት ይጀምሩ እና እንደ ትንሽ የመንኮራኩሮች ስብስብ ያስተካክሉ
ልቀቱ እየቀጠለ ነው። ከዚህ በታች ያለው ሰንጠረዥ የሚመከሩትን እሴቶች ይይዛል
በጣም የተለመዱ ደረጃዎች; ያልተዘረዘሩ እሴቶች ወደ ነባሪው ይመለሳሉ
`OrchestratorConfig::default()` እና `FetchOptions::default()`።

| ደረጃ | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | ማስታወሻ |
|----------------------------
| ** ላብ / CI *** | `3` | `2` | `2` | `2500` | `300` | ጥብቅ የቆይታ ቆብ እና የጸጋ መስኮት ወለል ጫጫታ ቴሌሜትሪ በፍጥነት። ልክ ያልሆኑ አንጸባራቂዎችን በቶሎ ለማጋለጥ ሙከራዎችን ይቀንሱ። |
| ** መድረክ *** | `4` | `3` | `3` | `4000` | `600` | ለአሳሽ እኩዮች ከዋናው ክፍል ሲወጡ የምርት ነባሪዎችን ያንጸባርቃል። |
| **ካናሪ** | `6` | `3` | `3` | `5000` | `900` | ግጥሚያዎች ነባሪዎች; ዳሽቦርዶች የካናሪ ትራፊክን እንዲቀይሩ `telemetry_region` ያዘጋጁ። |
| ** አጠቃላይ ተገኝነት *** | `None` (ብቁ የሆኑትን ይጠቀሙ) | `4` | `4` | `5000` | `900` | ኦዲት መወሰኑን መተግበሩን በሚቀጥልበት ጊዜ አላፊ ጥፋቶችን ለመቀበል ድጋሚ መሞከር እና ውድቅ ጣራዎችን ይጨምሩ። |

- የታችኛው ተፋሰስ ካልሆነ በስተቀር `scoreboard.weight_scale` በነባሪ I18NI0000041X ይቆያል
  ስርዓቱ የተለየ የኢንቲጀር ጥራት ይፈልጋል። መጠኑን መጨመር አያመጣም
  ለውጥ አቅራቢ ማዘዝ; ጥቅጥቅ ያለ የብድር ስርጭት ብቻ ነው የሚያወጣው።
- በደረጃዎች መካከል ሲሰደዱ የJSON ቅርቅቡን ይቀጥሉ እና ይጠቀሙ
  `--scoreboard-out` ስለዚህ የኦዲት ዱካ ትክክለኛውን መለኪያ ስብስብ ይመዘግባል።

## 2. የውጤት ሰሌዳ ንጽሕና

የውጤት ሰሌዳው አንጸባራቂ መስፈርቶችን፣ የአቅራቢ ማስታወቂያዎችን እና ቴሌሜትሪን ያጣምራል።
ወደፊት ከመንከባለል በፊት፡-

1. **የቴሌሜትሪ ትኩስነትን ያረጋግጡ።** የተጠቀሱ ቅጽበተ-ፎቶዎችን ያረጋግጡ
   `--telemetry-json` በተዋቀረው የጸጋ መስኮት ውስጥ ተይዟል። ግቤቶች
   ከተዋቀረው I18NI0000044X በላይ የቆየ አይሳካም።
   `TelemetryStale { last_updated }`. ይህንን እንደ ጠንካራ ማቆሚያ ይያዙት እና ያድሱት።
   ከመቀጠልዎ በፊት ቴሌሜትሪ ወደ ውጪ መላክ.
2. ** የብቃት ምክንያቶችን መርምር።** ቅርሶችን በ በኩል ቀጥል።
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. እያንዳንዱ ግቤት
   ከትክክለኛው የውድቀት መንስኤ ጋር `eligibility` ብሎክ ይይዛል። አትሻር
   የአቅም አለመመጣጠን ወይም ጊዜ ያለፈባቸው ማስታወቂያዎች; ወደ ላይ ያለውን ጭነት አስተካክል።
3. **የክብደት ዴልታዎችን ይገምግሙ።** የ`normalised_weight` መስክን ከ
   ቀዳሚ ልቀት. የክብደት ፈረቃ>10% ሆን ተብሎ ከሚታወቅ ማስታወቂያ ጋር መዛመድ አለበት።
   ወይም ቴሌሜትሪ ይቀየራል እና በታቀደ ምዝግብ ማስታወሻ ውስጥ መታወቅ አለበት።
4. **የቅርሶች ማህደር።** እያንዳንዱ ሩጫ እንዲለቅ `scoreboard.persist_path` አዋቅር
   የመጨረሻው የውጤት ሰሌዳ ቅጽበታዊ እይታ. አርቲፊኬቱን ከተለቀቀው መዝገብ ጋር ያያይዙት።
   ከማንፀባረቂያው እና ከቴሌሜትሪ ጥቅል ጋር።
5. **የመመዝገብ አቅራቢ ቅይጥ ማስረጃ።** `scoreboard.json` ሜታዳታ _እና_ የ
   ተዛማጅ `summary.json` I18NI0000052X ማጋለጥ አለበት፣
   `gateway_provider_count`፣ እና የተገኘው I18NI0000054X መለያ ስለዚህ ገምጋሚዎች
   ሩጫው I18NI0000055X፣ `gateway-only`፣ ወይም `mixed` መሆኑን ማረጋገጥ ይችላል።
   ጌትዌይ ስለዚህ `provider_count=0` plus ሪፖርት አድርግ
   `provider_mix="gateway-only"`፣ የተቀላቀሉ ሩጫዎች ግን ዜሮ ያልሆኑ ቆጠራዎች ያስፈልጋቸዋል
   ሁለቱም ምንጮች. `cargo xtask sorafs-adoption-check` እነዚህን መስኮች ያስፈጽማል (እና
   ቆጠራዎቹ/ስያሜዎቹ በማይስማሙበት ጊዜ አይሳካም)፣ ስለዚህ ሁል ጊዜ ከጎኑ ያሂዱት
   `ci/check_sorafs_orchestrator_adoption.sh` ወይም የርስዎ ቀረጻ ስክሪፕት ወደ
   የ `adoption_report.json` የማስረጃ ጥቅል ያዘጋጁ። Torii በሮች ሲሆኑ
   ተሳታፊ፣ `gateway_manifest_id`/I18NI0000064X በውጤት ሰሌዳው ውስጥ ያቆዩት።
   ሜታዳታ ስለዚህ የማደጎ በር አንጸባራቂውን ፖስታ ከ
   የተያዙ የአቅራቢዎች ድብልቅ.

ለዝርዝር የመስክ ፍቺዎች ይመልከቱ
`crates/sorafs_car/src/scoreboard.rs` እና የCLI ማጠቃለያ መዋቅር በ ተጋልጧል
`sorafs_cli fetch --json-out`.

## CLI እና ኤስዲኬ ባንዲራ ማጣቀሻ

`sorafs_cli fetch` (`crates/sorafs_car/src/bin/sorafs_cli.rs` ይመልከቱ) እና እ.ኤ.አ.
`iroha_cli app sorafs fetch` መጠቅለያ (I18NI0000070X)
ተመሳሳይ የኦርኬስትራ ውቅር ገጽን ያካፍሉ። የሚከተሉትን ባንዲራዎች ሲጠቀሙ ይጠቀሙ
የታቀዱ ማስረጃዎችን ማንሳት ወይም ቀኖናዊ መጫዎቻዎችን እንደገና መጫወት፡-

የተጋራ ባለብዙ-ምንጭ ባንዲራ ማጣቀሻ (ይህን ፋይል በማርትዕ ብቻ የCLI እገዛን እና ሰነዶችን ማመሳሰል ያቆዩ)።

- `--max-peers=<count>` ምን ያህል ብቁ አቅራቢዎች ከውጤት ሰሌዳ ማጣሪያ እንደሚተርፉ ይገድባል። ከእያንዳንዱ ብቁ አቅራቢ ለመለቀቅ እንዳልተዋቀረ ይተዉ፣ ወደ `1` ያቀናብሩ ነጠላ-ምንጭ ውድቀትን ሆን ብለው ሲለማመዱ ብቻ። በኤስዲኬዎች (`SorafsGatewayFetchOptions.maxPeers`፣ I18NI0000075X) ውስጥ የ`maxPeers` ቁልፍን ያንጸባርቃል።
- `--retry-budget=<count>` በ`FetchOptions` ወደተፈፀመው በእያንዳንዱ-ቻንክ የድጋሚ ሙከራ ገደብ ያስተላልፋል። ለሚመከሩት እሴቶች በመቃኛ መመሪያው ውስጥ የታቀደውን ሠንጠረዥ ይጠቀሙ; ማስረጃን የሚሰበስበው CLI ሩጫዎች እኩልነትን ለመጠበቅ ከኤስዲኬ ነባሪዎች ጋር መዛመድ አለባቸው።
- `--telemetry-region=<label>` መለያዎች `sorafs_orchestrator_*` Prometheus ተከታታይ (እና OTLP ሪሌይ) ከክልል/ኤንቪ መለያ ጋር ዳሽቦርዶች የላብራቶሪ፣ የደረጃ፣ የካናሪ እና የ GA ትራፊክን ይለያሉ።
- `--telemetry-json=<path>` በውጤት ሰሌዳው የተጠቀሰውን ቅጽበታዊ ገጽ እይታ ያስገባል። ኦዲተሮች ሩጫውን እንደገና እንዲጫወቱ ከውጤት ሰሌዳው ቀጥሎ ያለውን JSON ያቆዩት (እና ስለዚህ `cargo xtask sorafs-adoption-check --require-telemetry` የትኛው የ OTLP ዥረት መቅረቡን እንደመገበ ማረጋገጥ ይችላል)።
- `--local-proxy-*` (`--local-proxy-mode`፣ `--local-proxy-norito-spool`፣ `--local-proxy-kaigi-spool`፣ `--local-proxy-kaigi-policy`) የድልድይ ተመልካቾችን መንጠቆዎች ያነቃል። ሲዋቀር ኦርኬስትራተሩ በአካባቢያዊው I18NT0000004X/Kaigi ፕሮክሲ በኩል ይለቀቃል ስለዚህ የአሳሽ ደንበኞች፣ የጥበቃ መሸጎጫዎች እና የካይጊ ክፍሎች በሩስት የሚለቀቁትን ተመሳሳይ ደረሰኞች ይቀበላሉ።
- `--scoreboard-out=<path>` (በአማራጭ ከ`--scoreboard-now=<unix_secs>` ጋር የተጣመረ) ለኦዲተሮች የብቁነት ቅጽበታዊ ገጽ እይታ እንደቀጠለ ነው። ሁልጊዜ የቀጠለውን JSON ከቴሌሜትሪ እና በመለቀቂያ ትኬቱ ላይ ከተጠቀሱት የሰነድ ማስረጃዎች ጋር ያጣምሩ።
- `--deny-provider name=ALIAS`/I18NI0000090X በማስታወቂያ ዲበ ዳታ ላይ የሚወስኑ ማስተካከያዎችን ይተግብሩ። እነዚህን ባንዲራዎች ለልምምድ ብቻ ይጠቀሙ; የምርት ማሽቆልቆል በአስተዳደር ቅርሶች በኩል መፍሰስ አለበት ስለዚህ እያንዳንዱ መስቀለኛ መንገድ አንድ አይነት የፖሊሲ ጥቅል ይተገበራል።
- `--provider-metrics-out` / I18NI0000092X በታቀደ ቼክ ዝርዝሩ የተጠቀሰውን የእያንዳንዱን አቅራቢ የጤና መለኪያዎችን እና ቁርጥራጭ ደረሰኞችን ይይዛል። የማደጎ ማስረጃ በሚያስገቡበት ጊዜ ሁለቱንም ቅርሶች አያይዝ።

ምሳሌ (የታተመውን መሣሪያ በመጠቀም)

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

ኤስዲኬዎች በዛገቱ ውስጥ በI18NI0000093X በኩል ተመሳሳይ ውቅር ይበላሉ
ደንበኛ (`crates/iroha/src/client.rs`)፣ የጄኤስ ማሰሪያዎች
(`javascript/iroha_js/src/sorafs.js`)፣ እና ስዊፍት ኤስዲኬ
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`)። እነዚያን ረዳቶች ያቆዩዋቸው
ኦፕሬተሮች በራስ ሰር ፖሊሲዎችን መቅዳት እንዲችሉ ከCLI ነባሪዎች ጋር መቆለፍ
ያለ ተተርጉሟል የትርጉም ንብርብሮች.

## 3. የፖሊሲ ማስተካከል

`FetchOptions` ባህሪን፣ ተመሳሳይነትን እና ማረጋገጫን እንደገና ይሞክሩ። መቼ
ማስተካከል፡

- ** ድጋሚ ሙከራዎች:** `per_chunk_retry_limit` ከ `4` በላይ ማሳደግ መልሶ ማግኘትን ይጨምራል
  ጊዜ ግን የአቅራቢውን ስህተቶች መደበቅ አደጋ ላይ ይጥላል። `4`ን እንደ ጣሪያ አድርጎ ማስቀመጥን እመርጣለሁ።
  በአቅራቢው አዙሪት ላይ በመተማመን ደካማ ፈጻሚዎችን ወደላይ ማዞር።
- ** የመውደቅ ገደብ፡** `provider_failure_threshold` የሚገዛው ሀ
  ለቀሪው ክፍለ ጊዜ አቅራቢው ተሰናክሏል። ይህን እሴት ከ ጋር አስተካክል።
  ድጋሚ ሞክር ፖሊሲ፡ ከድጋሚ ሙከራ ባጀት ያነሰ ደረጃ ኦርኬስትራውን ያስገድደዋል
  ሁሉም ሙከራዎች ከመሟሟቸው በፊት እኩያውን ማስወጣት።
- **ተዛማጅነት፡** `global_parallel_limit` እንዳልተዋቀረ ይተውት (`None`) ካልሆነ በስተቀር
  የተወሰነ አካባቢ የማስታወቂያ ክልሎችን ሊሞላው አይችልም። ሲዋቀር ያረጋግጡ
  እሴቱ ≤ ረሃብን ለማስወገድ የአቅራቢዎች ፍሰት በጀት ድምር ነው።
- ** የማረጋገጫ መቀየሪያዎች:** `verify_lengths` እና `verify_digests` መቆየት አለባቸው
  በምርት ላይ የነቃ. የተቀላቀሉ አቅራቢ መርከቦች ሲሆኑ ቆራጥነት ዋስትና ይሰጣሉ
  በጨዋታ ላይ ናቸው; በገለልተኛ ግራ መጋባት ውስጥ ብቻ ያሰናክሏቸው።

## 4. የትራንስፖርት እና ማንነትን መደበቅ ዝግጅት

የ `rollout_phase`፣ `anonymity_policy` እና `transport_policy` መስኮችን ይጠቀሙ።
የግላዊነት አቀማመጥን ይወክላሉ፡-- `rollout_phase="snnet-5"`ን ምረጥ እና ነባሪው የስም ማጥፋት ፖሊሲን ፍቀድ
  የ SNNet-5 ደረጃዎችን ይከታተሉ። በ`anonymity_policy_override` ብቻ ይሽሩ
  አስተዳደር የተፈረመ መመሪያ ሲያወጣ.
- SNNet-4/5/5a/5b/6a/7/8/12/13 ሲሆኑ `transport_policy="soranet-first"` እንደ መነሻ መስመር ያቆዩት 🈺
  (`roadmap.md` ይመልከቱ)። ለሰነድ ብቻ `transport_policy="direct-only"` ይጠቀሙ
  የማሳነስ/የማክበር ልምምዶችን፣ እና የPQ ሽፋን ግምገማን ከዚህ በፊት ይጠብቁ
  ወደ `transport_policy="soranet-strict"` በማስተዋወቅ ላይ - ያ ደረጃ በፍጥነት አይሳካም።
  ክላሲካል ቅብብሎሽ ብቻ ይቀራል።
- `write_mode="pq-only"` መተግበር ያለበት እያንዳንዱ የመጻፍ መንገድ (ኤስዲኬ፣
  ኦርኬስትራ ፣ የአስተዳደር መሣሪያ) የ PQ መስፈርቶችን ማሟላት ይችላል። ወቅት
  የድንገተኛ ጊዜ ምላሾች ሊታመኑ ስለሚችሉ መልቀቅ `write_mode="allow-downgrade"` ያስቀምጣል።
  ቴሌሜትሪ የመቀነሱን ምልክት በሚያሳይበት ቀጥታ መንገዶች ላይ።
- የጥበቃ ምርጫ እና የወረዳ ዝግጅት በሶራኔት ማውጫ ላይ ይመሰረታል። ያቅርቡ
  የተፈረመ I18NI000001117X ቅጽበታዊ ገጽ እይታ እና የ `guard_set` መሸጎጫውን ይቀጥሉ ስለዚህ ይጠብቁ
  churn በተስማማው የማቆያ መስኮት ውስጥ ይቆያል። የመሸጎጫ አሻራ ገብቷል።
  በ`sorafs_cli fetch` የታቀዱ ማስረጃዎችን ይመሰርታል።

## 5. ዝቅ ማድረግ እና ተገዢነት መንጠቆዎች

ሁለት ኦርኬስትራ ንዑስ ስርዓቶች መመሪያን ያለ በእጅ ጣልቃ ገብነት ለማስፈጸም ያግዛሉ፡

- ** የወረደ ማሻሻያ *** (`downgrade_remediation`): ማሳያዎች
  `handshake_downgrade_total` ክስተቶች እና, ከተዋቀረ በኋላ `threshold` ነው
  በ`window_secs` ውስጥ አልፏል፣የአካባቢውን ፕሮክሲ ወደ `target_mode` ያስገድዳል።
  (ሜታዳታ-ብቻ በነባሪ)። ነባሪዎቹን አቆይ (`threshold=3`፣ `window=300`፣
  `cooldown=900`) የአደጋ ግምገማዎች የተለየ ስርዓተ-ጥለት እስካላሳዩ ድረስ። ማንኛውም ሰነድ
  የታቀዱ ምዝግብ ማስታወሻዎችን ይሽሩ እና የዳሽቦርዶችን ዱካ ያረጋግጡ
  `sorafs_proxy_downgrade_state`.
- ** ተገዢነት ፖሊሲ *** (`compliance`)፡ የስልጣን እና የዝርዝር መግለጫዎች
  በአስተዳደር የሚተዳደር የመርጦ መውጣት ዝርዝሮች ውስጥ ይፈስሳል። በፍፁም የመስመር ውስጥ ማስታወቂያ አይሽረው
  በማዋቀሪያው ጥቅል ውስጥ; በምትኩ፣ የተፈረመ ዝማኔ ይጠይቁ
  `governance/compliance/soranet_opt_outs.json` እና የመነጨውን JSON ን እንደገና ማሰማራት።

ለሁለቱም ስርዓቶች፣ የተገኘውን የውቅር ቅርቅብ ይቀጥሉ እና ያካትቱት።
ኦዲተሮች እንዴት ወደታች ፈረቃ እንደተቀሰቀሱ ለማወቅ ማስረጃዎችን ይልቀቁ።

## 6. ቴሌሜትሪ እና ዳሽቦርዶች

ልቀቱን ከማስፋትዎ በፊት፣ የሚከተሉት ምልክቶች በ ውስጥ ቀጥታ መሆናቸውን ያረጋግጡ
ኢላማ አካባቢ፡

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  ካናሪው ከተጠናቀቀ በኋላ ዜሮ መሆን አለበት.
- `sorafs_orchestrator_retries_total` እና
  `sorafs_orchestrator_retry_ratio` - ከ 10% በታች መረጋጋት አለበት
  ካናሪ እና ከጂኤ በኋላ ከ 5% በታች ይቆዩ።
- `sorafs_orchestrator_policy_events_total` - የሚጠበቀውን ያረጋግጣል
  የመልቀቅ ደረጃ ንቁ ነው (`stage` መለያ) እና ቡኒ መውጫዎችን በ`outcome` ይመዘግባል።
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` - የ PQ ቅብብል አቅርቦትን ይከታተሉ
  ፖሊሲ የሚጠበቁ.
- `telemetry::sorafs.fetch.*` የምዝግብ ማስታወሻዎች - ወደ የተጋራው መዝገብ መተላለፍ አለባቸው
  ለ `status=failed` ከተቀመጡ ፍለጋዎች ጋር ሰብሳቢ።

ቀኖናዊውን I18NT0000002X ዳሽቦርዱን ከ ይጫኑ
`dashboards/grafana/sorafs_fetch_observability.json` (በፖርታሉ ውስጥ ወደ ውጭ ተልኳል
በ **SoraFS → ታዛቢነት አምጣ**) ስለዚህ ክልሉ/መራጮችን አሳይ፣
አቅራቢው የሙቀት ካርታ፣ የተቆራረጡ መዘግየት ሂስቶግራሞች እና የስቶል ቆጣሪዎች ይዛመዳሉ
በቃጠሎ ጊዜ SRE ምን ይገመግማል። የ Alertmanager ደንቦችን በሽቦ
`dashboards/alerts/sorafs_fetch_rules.yml` እና የ I18NT0000001X አገባብ ያረጋግጡ
በ `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ረዳቱ በራስ-ሰር
`promtool test rules` በአገር ውስጥ ወይም በI18NT0000006X ይሰራል። ማንቂያ የእጅ ማጥፋት ያስፈልገዋል
ኦፕሬተሮች ማስረጃውን እንዲሰኩ ስክሪፕቱ የሚያትመው ተመሳሳይ የማዞሪያ እገዳ
የመልቀቅ ትኬቱ።

### ቴሌሜትሪ የሚቃጠል የስራ ፍሰት

የመንገድ ካርታ ንጥል **SF-6e** ከመገልበጥዎ በፊት የ30 ቀን ቴሌሜትሪ ማቃጠልን ይጠይቃል።
ባለብዙ ምንጭ ኦርኬስትራ ወደ GA ነባሪዎቹ። የማከማቻ ስክሪፕቶችን ይጠቀሙ
በመስኮቱ ውስጥ ለእያንዳንዱ ቀን ሊባዛ የሚችል የቅርስ ቅርቅብ ይያዙ፡

1. ከተቃጠለው አካባቢ ጋር `ci/check_sorafs_orchestrator_adoption.sh` ን ያሂዱ
   አንጓዎች ተዘጋጅተዋል. ምሳሌ፡-

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   ረዳቱ `fixtures/sorafs_orchestrator/multi_peer_parity_v1` ይደግማል፣
   ይጽፋል `scoreboard.json`፣ `summary.json`፣ `provider_metrics.json`፣
   `chunk_receipts.json`፣ እና `adoption_report.json` ስር
   `artifacts/sorafs_orchestrator/<timestamp>/`፣ እና አነስተኛ ቁጥር ያስፈጽማል
   በ`cargo xtask sorafs-adoption-check` በኩል ብቁ አቅራቢዎች።
2. የተቃጠሉ ተለዋዋጮች በሚገኙበት ጊዜ ስክሪፕቱ እንዲሁ ይወጣል
   `burn_in_note.json`፣ መለያውን፣ የቀን መረጃ ጠቋሚውን፣ የሰነድ መታወቂያውን፣ ቴሌሜትሪውን በመያዝ
   ምንጭ, እና artefact ተፈጭተው. ይህን JSON ከታቀደ ምዝግብ ማስታወሻ ጋር ያያይዙት።
   በ30-ቀን መስኮት ውስጥ የትኛው ቀረጻ በየቀኑ እንደሚረካ ግልጽ ነው።
3. የዘመነውን Grafana ሰሌዳ (`dashboards/grafana/sorafs_fetch_observability.json`) አስመጣ
   ወደ ስቴጅንግ/ምርት የስራ ቦታ፣ በተቃጠለው መለያ መለያ ይስጡት፣ እና
   እያንዳንዱ ፓነል በሙከራ ላይ ላለው አንጸባራቂ/ክልል ናሙናዎችን እንደሚያሳይ ያረጋግጡ።
4. `scripts/telemetry/test_sorafs_fetch_alerts.sh` (ወይም `promtool test rules …`) አሂድ
   በማንኛውም ጊዜ `dashboards/alerts/sorafs_fetch_rules.yml` ወደ ሰነድ ሲቀየር
   ማንቂያ ማዘዋወር በተቃጠለው ጊዜ ወደ ውጭ ከተላኩት መለኪያዎች ጋር ይዛመዳል።
5. የተገኘውን ዳሽቦርድ ቅጽበታዊ ገጽ እይታ፣ የማንቂያ ሙከራ ውጤት እና የሎግ ጅራትን በማህደር ያስቀምጡ
   ከኦርኬስትራ ጋር ከ `telemetry::sorafs.fetch.*` ፍለጋዎች
   ቅርሶች ስለዚህ አስተዳደር መለኪያዎችን ከ ሳያስገባ ማስረጃውን እንደገና ማጫወት ይችላል።
   የቀጥታ ስርዓቶች.

## 7. የታቀዱ የማረጋገጫ ዝርዝር

1. የእጩውን ውቅረት እና ቀረጻ በመጠቀም የውጤት ሰሌዳዎችን በCI ውስጥ ያድሱ
   በስሪት ቁጥጥር ስር ያሉ ቅርሶች።
2. በእያንዳንዱ አካባቢ (ላቦራቶሪ፣ ዝግጅት፣
   ካናሪ፣ ምርት) እና `--scoreboard-out` እና `--json-out` ያያይዙ
   ወደ የታቀደው መዝገብ ላይ ቅርሶች.
3. ሁሉንም መለኪያዎች በማረጋገጥ የቴሌሜትሪ ዳሽቦርዶችን ከጥሪ መሐንዲሱ ጋር ይገምግሙ
   ከላይ የቀጥታ ናሙናዎች አላቸው.
4. የመጨረሻውን የውቅር መንገድ ይመዝግቡ (ብዙውን ጊዜ በ `iroha_config`) እና
   ለማስታወቂያ እና ተገዢነት የሚያገለግለው የአስተዳደር መዝገብ ቤት ቁርጠኝነት።
5. የታቀደውን መከታተያ ያዘምኑ እና ለኤስዲኬ ቡድኖች ደንበኛ እንዲሆኑ አዲስ ነባሪዎች ያሳውቁ
   ውህደቶች ተስተካክለው ይቆያሉ.

ይህንን መመሪያ በመከተል የኦርኬስትራ ስምምነቶችን የሚወስኑ እና ኦዲት እንዲደረጉ ያደርጋል
ድጋሚ ሞክር በጀቶችን ለማስተካከል ግልጽ የግብረመልስ ምልከታዎችን በሚሰጥበት ጊዜ፣ አቅራቢ
አቅም እና የግላዊነት አቀማመጥ።