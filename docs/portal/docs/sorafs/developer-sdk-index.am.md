---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 28cc43e407412d66481f25146c19d35f0e102523d22f954be3c106231d95e891
source_last_modified: "2026-01-05T09:28:11.868629+00:00"
translation_last_reviewed: 2026-02-07
id: developer-sdk-index
title: SoraFS SDK Guides
sidebar_label: SDK Guides
description: Language-specific snippets for integrating SoraFS artefacts.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

በ I18NT0000000X የመሳሪያ ሰንሰለት የሚላኩ የቋንቋ አጋዥዎችን ለመከታተል ይህንን ማዕከል ይጠቀሙ።
ለዝገት-ተኮር ቅንጥቦች ወደ [ዝገት ኤስዲኬ ቅንጣቢዎች](./developer-sdk-rust.md) ይዝለሉ።

## የቋንቋ ረዳቶች

- **ፓይቶን** — `sorafs_multi_fetch_local` (የአካባቢው ኦርኬስትራ የጭስ ሙከራዎች) እና
  `sorafs_gateway_fetch` (የጌትዌይ E2E ልምምዶች) አሁን አማራጭን ተቀበሉ
  `telemetry_region` እና `transport_policy` መሻር
  (`"soranet-first"`፣ `"soranet-strict"`፣ ወይም `"direct-only"`)፣ CLIን በማንፀባረቅ
  የመልቀቂያ ቁልፎች. የአካባቢ QUIC ፕሮክሲ ሲሽከረከር፣
  `sorafs_gateway_fetch` የአሳሹን አንጸባራቂ ይመልሳል
  `local_proxy_manifest` ስለዚህ ሙከራዎች የእምነት ቅርቅቡን ለአሳሽ አስማሚዎች መስጠት ይችላሉ።
- ** ጃቫ ስክሪፕት *** - `sorafsMultiFetchLocal` የ Python አጋዥን ያንጸባርቃል ፣ ይመለሳል
  የጭነት ባይት እና ደረሰኝ ማጠቃለያዎች፣ `sorafsGatewayFetch` ልምምዶች
  Torii ጌትዌይስ፣ ክሮች የአካባቢ ተኪ መገለጫዎች፣ እና ተመሳሳይ ያጋልጣል
  ቴሌሜትሪ/ትራንስፖርት እንደ CLI ይሽራል።
- ** ዝገት *** - አገልግሎቶች መርሐግብር አውጪውን በቀጥታ በ በኩል መክተት ይችላሉ።
  `sorafs_car::multi_fetch`; [ዝገት ኤስዲኬ ቅንጣቢዎችን](./developer-sdk-rust.md) ይመልከቱ።
  የማረጋገጫ-ዥረት ረዳቶች እና ኦርኬስትራ ውህደት ማጣቀሻ።
- **አንድሮይድ** — `HttpClientTransport.sorafsGatewayFetch(…)` Torii HTTP እንደገና ይጠቀማል
  አስፈፃሚ እና ክብር `GatewayFetchOptions`. ጋር ያዋህዱት
  `ClientConfig.Builder#setSorafsGatewayUri` እና የPQ ሰቀላ ፍንጭ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) ሰቀላዎች መጣበቅ ሲገባቸው
  PQ-ብቻ መንገዶች።

## የውጤት ሰሌዳ እና የፖሊሲ ቁልፎች

ሁለቱም Python (`sorafs_multi_fetch_local`) እና JavaScript
(`sorafsMultiFetchLocal`) ረዳቶች በቴሌሜትሪ የሚታወቅ የጊዜ ሰሌዳ ቆጣሪን ያጋልጣሉ
በ CLI ጥቅም ላይ ይውላል:

- የምርት ሁለትዮሽ በነባሪ የውጤት ሰሌዳውን ማንቃት; አዘጋጅ `use_scoreboard=True`
  (ወይም የ `telemetry` ግቤቶችን ያቅርቡ) መገልገያዎችን እንደገና ሲጫወቱ ረዳቱ እንዲያገኝ
  ከማስታወቂያ ሜታዳታ እና የቅርብ ጊዜ የቴሌሜትሪ ቅጽበተ-ፎቶዎች ክብደት ያለው አቅራቢ ማዘዝ።
- የተሰሉትን ክብደቶች ከቁርጥ ጋር ለመቀበል `return_scoreboard=True` ያዘጋጁ
  የ CI ምዝግብ ማስታወሻዎች ምርመራዎችን እንዲይዙ ደረሰኞች።
- አቻዎችን ላለመቀበል ወይም ለመጨመር `deny_providers` ወይም I18NI0000028X ድርድሮችን ይጠቀሙ
  መርሐግብር አውጪው አቅራቢዎችን ሲመርጥ `priority_delta`።
- የመቀነስ ደረጃ ካላደረጉ በስተቀር ነባሪውን የ I18NI0000030X አቀማመጥ ያስቀምጡ; አቅርቦት
  `"direct-only"` ብቻ ተገዢ ክልል ቅብብል መራቅ አለበት ጊዜ ወይም መቼ
  የSNNet-5a ውድቀትን በመለማመድ እና `"soranet-strict"` ለPQ-ብቻ ያስቀምጡ
  የአስተዳደር ፍቃድ ያላቸው አብራሪዎች.
- የጌትዌይ ረዳቶችም `scoreboardOutPath` እና `scoreboardNowUnixSecs` ያጋልጣሉ።
  የተሰላውን የውጤት ሰሌዳ ለመቀጠል `scoreboardOutPath` ያቀናብሩ (የ CLI ን ያሳያል)
  `--scoreboard-out` ባንዲራ) ስለዚህ `cargo xtask sorafs-adoption-check` ማረጋገጥ ይችላል
  የኤስዲኬ ቅርሶች፣ እና ቋሚዎች ቋሚ ሲፈልጉ `scoreboardNowUnixSecs` ይጠቀሙ
  `assume_now` እሴት ሊባዛ ለሚችል ሜታዳታ። በጃቫስክሪፕት ረዳት ውስጥ
  በተጨማሪ `scoreboardTelemetryLabel`/I18NI0000041X ማዘጋጀት ይችላል;
  መለያው ሲቀር `region:<telemetryRegion>` (ወደ ኋላ መውደቅ) ያገኛል
  ወደ `sdk:js`)። የ Python አጋዥ በራስ-ሰር `telemetry_source="sdk:python"` ያወጣል።
  የውጤት ሰሌዳ በሚቆይበት ጊዜ እና ስውር ሜታዳታ እንዲሰናከል ያደርጋል።

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```