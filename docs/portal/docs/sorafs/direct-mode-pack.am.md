---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9260ec51a96f2c9cb340026ae8c0529d8168a08e35168058a859a9852aa10fb
source_last_modified: "2026-01-22T14:35:36.799403+00:00"
translation_last_reviewed: 2026-02-07
id: direct-mode-pack
title: SoraFS Direct-Mode Fallback Pack (SNNet-5a)
sidebar_label: Direct-Mode Fallback Pack
description: Required configuration, compliance checks, and rollout steps when operating SoraFS in direct Torii/QUIC mode during the SNNet-5a transition.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

የሶራኔት ወረዳዎች ለSoraFS ነባሪ መጓጓዣ ሆነው ይቆያሉ፣ ነገር ግን የፍኖተ ካርታ ንጥል **SNNet-5a** ቁጥጥር የሚደረግበት ውድቀትን ይፈልጋል ስለሆነም ኦፕሬተሮች የማንነት መታወቂያው ሲጠናቀቅ ንባብ መዳረሻን እንዲጠብቁ። ይህ ጥቅል የግላዊነት ማጓጓዣዎችን ሳይነኩ SoraFSን በቀጥታ Torii/QUIC ሁነታ ለማስኬድ የሚያስፈልጉትን የCLI/SDK ቁልፎች፣ የውቅረት መገለጫዎች፣ የተግባር ፈተናዎች እና የስምሪት ማረጋገጫ ዝርዝር ይይዛል።

ከSNNet-5 እስከ SNNet-9 ድረስ የዝግጅታቸውን በሮች እስኪያፀዱ ድረስ የድጋፍ መመለሻው በመደርደር እና በተስተካከለ የምርት አካባቢዎች ላይም ይሠራል። ኦፕሬተሮች በማይታወቁ እና በፍላጎት ቀጥታ ሁነታዎች መካከል መለዋወጥ እንዲችሉ ቅርሶቹን ከተለመደው የSoraFS የስምሪት ማስያዣ ጎን ያኑሩ።

## 1. CLI እና SDK ባንዲራዎች

- `sorafs_cli fetch --transport-policy=direct-only …` የዝውውር መርሐግብርን ያሰናክላል እና Torii/QUIC መጓጓዣዎችን ያስፈጽማል። የCLI እገዛ አሁን `direct-only` እንደ ተቀባይነት እሴት ይዘረዝራል።
- ኤስዲኬዎች የ"ቀጥታ ሁነታ" መቀያየርን ባጋለጡ ቁጥር `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` ማዘጋጀት አለባቸው። በI18NI0000020X እና I18NI0000021X ውስጥ ያሉት የመነጩ ማሰሪያዎች ተመሳሳይ ቁጥርን ያስተላልፋሉ።
- የጌትዌይ ማሰሪያዎች (`sorafs_fetch`፣ Python bindings) በቀጥታ-ብቻ መቀያየርን በተጋሩ I18NT0000000X JSON አጋዥዎች በኩል አውቶሜትድ ተመሳሳይ ባህሪን እንዲቀበል ማድረግ ይችላል።

ከአካባቢ ተለዋዋጮች ይልቅ በአጋር ፊት ለፊት በሚታዩ runbooks እና በ`iroha_config` በኩል የሚቀያየር የሽቦ ባህሪ ባንዲራውን ይመዝግቡ።

## 2. የጌትዌይ ፖሊሲ መገለጫዎች

የሚወስን ኦርኬስትራ ውቅረትን ለመቀጠል Norito JSON ይጠቀሙ። በ`docs/examples/sorafs_direct_mode_policy.json` ውስጥ ያለው የምሳሌ መገለጫ የሚከተለውን ኮድ ይይዛል፡-

- `transport_policy: "direct_only"` - የሶራኔት ቅብብሎሽ ማጓጓዣዎችን ብቻ የሚያስተዋውቁ አቅራቢዎችን ውድቅ ያድርጉ።
- `max_providers: 2` - በቀጥታ እኩዮችን ወደ በጣም አስተማማኝ I18NT0000007X/QUIC የመጨረሻ ነጥቦች። በክልል ተገዢነት አበል መሰረት ያስተካክሉ።
- `telemetry_region: "regulated-eu"` - የቴሌሜትሪ ዳሽቦርዶች እና ኦዲቶች የመውደቅ ሩጫዎችን የሚለዩ መለኪያዎችን ይሰይሙ።
- ወግ አጥባቂ ድጋሚ ሞክር በጀቶች (`retry_budget: 2`፣ `provider_failure_threshold: 3`) የተሳሳቱ የመተላለፊያ መንገዶችን እንዳይሸፍኑ።

መመሪያውን ለኦፕሬተሮች ከማጋለጥዎ በፊት JSON ን በI18NI0000030X (አውቶሜሽን) ወይም በኤስዲኬ ማሰሪያ (`config_from_json`) ይጫኑ። ለኦዲት መንገዶች የውጤት ሰሌዳውን (`persist_path`) ቀጥል።

የጌትዌይ-ጎን ማስፈጸሚያ ቁልፎች በ`docs/examples/sorafs_gateway_direct_mode.toml` ውስጥ ተይዘዋል። አብነቱ ከ`iroha app sorafs gateway direct-mode enable` የሚገኘውን ውጤት ያንጸባርቃል፣የኤንቨሎፕ/የመግቢያ ቼኮችን በማሰናከል፣የሽቦ ፍጥነት-ገደብ ነባሪዎች እና የ`direct_mode` ሰንጠረዡን በእቅድ በተገኙ የአስተናጋጅ ስሞች እና አንጸባራቂ መግለጫዎች ይሞላል። ቅንጣቢውን ወደ ውቅረት አስተዳደር ከማድረግዎ በፊት የቦታ ያዥ እሴቶችን በታቀደ ልቀት ዕቅድዎ ይተኩ።

## 3. Compliance Test Suite

የቀጥታ ሁነታ ዝግጁነት አሁን በሁለቱም ኦርኬስትራ እና CLI ሳጥኖች ውስጥ ሽፋንን ያካትታል፡

- `direct_only_policy_rejects_soranet_only_providers` ዋስትና ይሰጣል `TransportPolicy::DirectOnly` እያንዳንዱ እጩ ማስታወቂያ የሶራኔት ሪሌይስን ብቻ የሚደግፍ ሲሆን በፍጥነት እንደሚወድቅ ዋስትና ይሰጣል።
- `direct_only_policy_prefers_direct_transports_when_available` Torii/QUIC ማጓጓዣዎች በሚኖሩበት ጊዜ ጥቅም ላይ እንደሚውሉ እና የሶራኔት ማስተላለፊያዎች ከክፍለ-ጊዜው እንደሚገለሉ ያረጋግጣል።【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` `docs/examples/sorafs_direct_mode_policy.json` ሰነዱ ከረዳት መገልገያዎች ጋር የተጣጣመ መቆየቱን ለማረጋገጥ Xን ይተነትናል።
- `fetch_command_respects_direct_transports` ልምምዶች `sorafs_cli fetch --transport-policy=direct-only` በተሳለቀበት I18NT0000009X መግቢያ በር ላይ፣ ቀጥተኛ መጓጓዣዎችን ለሚሰካ ቁጥጥር ለሚደረግባቸው አካባቢዎች የጭስ ምርመራ ያቀርባል።【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` ተመሳሳይ ትዕዛዝ ከፖሊሲው JSON እና የውጤት ሰሌዳ ጽናት ጋር ለመልቀቅ አውቶማቲክ ይጠቀልላል።

ዝማኔዎችን ከማተምዎ በፊት ትኩረት የተደረገበትን ስብስብ ያሂዱ፡

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

የመሥሪያ ቦታ ማጠናቀር ከወደቁ ለውጦች የተነሳ ካልተሳካ፣የማገድ ስህተቱን በ`status.md` ይቅረጹ እና ጥገኝነቱ እንደደረሰ እንደገና ያስጀምሩ።

## 4. አውቶማቲክ ጭስ ይሠራል

የCLI ሽፋን ብቻውን አካባቢን-ተኮር ሪግሬሽን (ለምሳሌ የጌትዌይ ፖሊሲ መንዳት ወይም አለመዛመድን ያሳያል) አያጋልጥም። የተወሰነ የጭስ ረዳት በI18NI0000045X ውስጥ ይኖራል እና `sorafs_cli fetch` ከቀጥታ ሞድ ኦርኬስትራ ፖሊሲ፣ የውጤት ሰሌዳ ጽናት እና ማጠቃለያ ቀረጻ ጋር ይጠቀልላል።

የአጠቃቀም ምሳሌ፡-

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- ስክሪፕቱ ሁለቱንም የ CLI ባንዲራዎች እና የቁልፍ = እሴት ማዋቀር ፋይሎችን ያከብራል (`docs/examples/sorafs_direct_mode_smoke.conf` ይመልከቱ)። ከመሮጥዎ በፊት የሰነድ መግለጫውን እና የአቅራቢውን የማስታወቂያ ግቤቶችን በምርት ዋጋዎች ይሙሉ።
- `--policy` ነባሪዎች ለ `docs/examples/sorafs_direct_mode_policy.json`፣ ነገር ግን ማንኛውም ኦርኬስትራ JSON በ `sorafs_orchestrator::bindings::config_to_json` ሊቀርብ ይችላል። CLI መመሪያውን በI18NI0000051X ይቀበላል፣ይህም ሊባዙ የሚችሉ የእጅ ማስተካከያ ባንዲራዎች የሌሉበት ነው።
- `sorafs_cli` በ I18NI0000053X ላይ በማይሆንበት ጊዜ ረዳቱ ይገነባል.
  `sorafs_orchestrator` crate (የመልቀቅ መገለጫ) ስለዚህ ጭስ ይሮጣል የአካል ብቃት እንቅስቃሴ
  የማጓጓዣ ቀጥታ ሁነታ የቧንቧ መስመር.
- ውጤቶች:
  - የተሰበሰበው ክፍያ (`--output`፣ ለ `artifacts/sorafs_direct_mode/payload.bin` ነባሪዎች)።
  - ለማጠቃለያ (I18NI0000057X፣ ነባሪዎች ከክፍያው ጋር) የቴሌሜትሪ ክልል እና ለመልቀቅ ማስረጃ የሚያገለግሉ የአቅራቢ ሪፖርቶችን የያዘ።
  - የውጤት ሰሌዳ ቅጽበተ-ፎቶ በመመሪያው JSON (ለምሳሌ `fetch_state/direct_mode_scoreboard.json`) ወደተገለጸው መንገድ ቀጥሏል። ይህንን ከለውጥ ቲኬቶች ማጠቃለያ ጋር በማህደር ያስቀምጡ።
- የጉዲፈቻ በር አውቶሜሽን፡ ማምጣቱ አንዴ እንደጨረሰ ረዳቱ የማያቋርጥ የውጤት ሰሌዳ እና የማጠቃለያ መንገዶችን በመጠቀም `cargo xtask sorafs-adoption-check` ጥሪ ያደርጋል። የሚፈለገው ምልአተ ጉባኤ በትዕዛዝ መስመሩ ላይ ለሚቀርቡት የአቅራቢዎች ብዛት ይቋረጣል። ትልቅ ናሙና ሲፈልጉ በI18NI0000060X ይሽሩት። የጉዲፈቻ ሪፖርቶች ከማጠቃለያው ቀጥሎ የተፃፉ ናቸው (`--adoption-report=<path>` ብጁ ቦታ ሊያዘጋጅ ይችላል) እና ረዳቱ `--require-direct-only` በነባሪ (ከመውደቅ ጋር የሚዛመድ) እና I18NI0000063X የሚዛመደውን የCLI ባንዲራ ባቀረቡ ቁጥር ያልፋል። ተጨማሪ የxtask ነጋሪ እሴቶችን ለማስተላለፍ `XTASK_SORAFS_ADOPTION_FLAGS` ይጠቀሙ (ለምሳሌ `--allow-single-source` በፀደቀው የደረጃ ዝቅጠት ወቅት በሩ ሁለቱንም ይታገሣል እና ውድቀትን ያስፈጽማል)። የአካባቢ ምርመራዎችን ሲያካሂዱ የጉዲፈቻ በርን በ `--skip-adoption-check` ብቻ ይዝለሉ። ፍኖተ ካርታው የጉዲፈቻ ሪፖርት ቅርቅቡን ለማካተት እያንዳንዱን የተስተካከለ የቀጥታ ሁነታ ሩጫ ያስፈልገዋል።

## 5. የታቀዱ የፍተሻ ዝርዝር

1. **የማዋቀር ቅዝቃዛ፡** የቀጥታ ሞድ JSON መገለጫን በእርስዎ `iroha_config` ማከማቻ ውስጥ ያከማቹ እና ሀሽኑን በለውጥ ትኬትዎ ውስጥ ይመዝግቡ።
2. **የጌትዌይ ኦዲት፡** ቀጥተኛ ሁነታን ከመገልበጥዎ በፊት የTorii የመጨረሻ ነጥቦችን TLS፣ አቅም ያለው TLVs እና የኦዲት ምዝግብ ማስታወሻን ያረጋግጡ። የጌትዌይ ፖሊሲ መገለጫን ለኦፕሬተሮች ያትሙ።
3. **የማስፈጸሚያ ማቋረጥ፡** የተዘመነውን የመጫወቻ ደብተር ከታዛዥነት/የቁጥጥር ገምጋሚዎች ጋር ያካፍሉ እና ከስም-አልባ ተደራቢ ውጭ ለመሮጥ ማረጋገጫዎችን ይያዙ።
4. **ደረቅ ሩጫ፡** የታዛዥነት ሙከራ ስብስብን እና የዝግጅት ማፈላለጊያን ከታወቁ ጥሩ Torii አቅራቢዎች ጋር ያስፈጽሙ። የውጤት ሰሌዳ ውጤቶችን እና የCLI ማጠቃለያዎችን በማህደር ያስቀምጡ።
5. **የምርት መቁረጫ፡** የለውጥ መስኮቱን ያሳውቁ፣ `transport_policy` ወደ I18NI0000069X (ወደ `soranet-first` ከገቡ) ገልብጥ እና የቀጥታ ሞድ ዳሽቦርዶችን (`sorafs_fetch` የአቅራቢውን አለመሳካት) ይቆጣጠሩ። SNNet-4/5/5a/5b/6a/7/8/12/13 በI18NI0000072X ከተመረቁ በኋላ ወደ SoraNet-መጀመሪያ መመለስ እንዲችሉ የመመለሻ ዕቅዱን ይመዝግቡ።
6. **ድህረ-ለውጥ ግምገማ፡** የውጤት ሰሌዳ ቅጽበተ-ፎቶዎችን ያያይዙ፣ ማጠቃለያዎችን እና የክትትል ውጤቶችን በለውጥ ትኬት ላይ ያያይዙ። `status.md` ከተሰራበት ቀን እና ከማንኛውም ያልተለመዱ ሁኔታዎች ጋር ያዘምኑ።

ኦፕሬተሮች ከቀጥታ ማቀያየር በፊት የስራ ሂደቱን እንዲለማመዱ የማረጋገጫ ዝርዝሩን ከI18NI0000074X አሂድ ጋር ያቆዩት። SNNet-5 ወደ GA ሲመረቅ፣ በምርት ቴሌሜትሪ ውስጥ ያለውን ተመሳሳይነት ካረጋገጠ በኋላ ውድቀትን ያቋርጡ።

## 6. ማስረጃ እና የማደጎ በር መስፈርቶች

የቀጥታ ሁነታ ቀረጻዎች አሁንም የ SF-6c ጉዲፈቻ በርን ማርካት አለባቸው። እሽግ
የውጤት ሰሌዳ፣ ማጠቃለያ፣ አንጸባራቂ ኤንቨሎፕ እና የጉዲፈቻ ሪፖርት ለእያንዳንዱ ሩጫ እንዲሁ
`cargo xtask sorafs-adoption-check` የውድቀትን አቀማመጥ ማረጋገጥ ይችላል። የጠፋ
መስኮች በሩ እንዲወድቅ ያስገድዳሉ፣ ስለዚህ የሚጠበቀውን ሜታዳታ በለውጥ ይመዝግቡ
ቲኬቶች.

- ** የመጓጓዣ ሜታዳታ፡** `scoreboard.json` ማስታወቅ አለበት።
  `transport_policy="direct_only"` (እና `transport_policy_override=true` ገልብጥ
  ማሽቆልቆሉን ሲያስገድዱ). የተጣመሩ ስም-አልባነት ፖሊሲ መስኮችን ያስቀምጡ
  ገምጋሚዎች እርስዎ መሆን አለመሆኑን ለማየት እንዲችሉ ነባሪዎችን በሚወርሱበት ጊዜም እንኳ ተሞልተዋል።
  ከተዘጋጀው ማንነትን ከመደበቅ እቅድ ወጥቷል።
- **የአቅራቢ ቆጣሪዎች፡** ጌትዌይ-ብቻ ክፍለ ጊዜዎች `provider_count=0` መቀጠል አለባቸው
  እና I18NI0000080X በ Torii አቅራቢዎች ብዛት ይሙሉ
  ተጠቅሟል። JSONን በእጅ ከማርትዕ ይቆጠቡ - CLI/SDK ቆጠራዎቹን እና
  የጉዲፈቻ ደጃፍ ክፍፍሉን የሚተዉ ቀረጻዎችን ውድቅ ያደርጋል።
- **ማስረጃን አሳይ፡** Torii መግቢያ መንገዶች ሲሳተፉ የተፈረመውን ይለፉ
  `--gateway-manifest-envelope <path>` (ወይም ኤስዲኬ አቻ) እንዲሁ
  `gateway_manifest_provided` እና `gateway_manifest_id`/`gateway_manifest_cid`
  በ `scoreboard.json` ውስጥ ተመዝግበዋል. `summary.json` ማዛመጃውን መያዙን ያረጋግጡ
  `manifest_id`/`manifest_cid`; የትኛውም ፋይል ከሆነ የጉዲፈቻ ቼክ አይሳካም።
  ጥንድ ይጎድላል.
- **የቴሌሜትሪ የሚጠበቁ ነገሮች፡** ቴሌሜትሪ ከቀረጻው ጋር ሲሄድ ያሂዱ
  በር ከ `--require-telemetry` ጋር ስለዚህ የጉዲፈቻ ዘገባው መለኪያዎቹ እንደነበሩ ያረጋግጣል
  የተለቀቀው. የአየር ክፍተት ያላቸው ልምምዶች ባንዲራውን ሊተዉ ይችላሉ፣ ግን CI እና ቲኬቶችን ይቀይሩ
  መቅረቱን መመዝገብ አለበት።

ምሳሌ፡-

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
````adoption_report.json` ከውጤት ቦርዱ፣ማጠቃለያ፣ማሳያ ጋር ያያይዙ
ኤንቨሎፕ ፣ እና የጭስ ማውጫ ጥቅል። እነዚህ ቅርሶች የ CI ጉዲፈቻ ሥራ ምን እንደሆነ ያንፀባርቃሉ
(`ci/check_sorafs_orchestrator_adoption.sh`) ያስገድዳል እና ቀጥተኛ ሁነታን ያስቀምጣል
ኦዲት የሚቻለውን ዝቅ ያደርጋል።