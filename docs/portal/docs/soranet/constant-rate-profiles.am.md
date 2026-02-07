---
lang: am
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7334c5f2ccfa93c15a0827390e78b6026bb65e80ac9d624321da84f2287ce581
source_last_modified: "2026-01-05T09:28:11.911258+00:00"
translation_last_reviewed: 2026-02-07
id: constant-rate-profiles
title: SoraNet constant-rate profiles
sidebar_label: Constant-Rate Profiles
description: SNNet-17B1 preset catalogue for core/home production relays plus the SNNet-17A2 null dogfood profile, with tick->bandwidth math, CLI helpers, and MTU guardrails.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

SNNet-17B ቋሚ-ተመን የትራንስፖርት መስመሮችን ያስተዋውቃል ስለዚህ ቅብብሎሽ በ1,024 ቢ ሴሎች ውስጥ ትራፊክን ያንቀሳቅሳል
የመጫኛ መጠን. ኦፕሬተሮች ከሶስት ቅድመ-ቅምጦች ይመርጣሉ።

- ** ዋና *** - ለመሸፈን > = 30 ሜጋ ባይት በሰከንድ ሊሰጥ የሚችል የውሂብ ማዕከል ወይም በሙያዊ የተስተናገዱ ቅብብሎች
  ትራፊክ.
- ** ቤት *** - አሁንም የማይታወቁ ፈላጊዎች የሚያስፈልጋቸው የመኖሪያ ወይም ዝቅተኛ-አገናኝ ኦፕሬተሮች
  ግላዊነት-ወሳኝ ወረዳዎች.
- ** ባዶ *** - የ SNNet-17A2 የሙከራ ስሪት ቅድመ ዝግጅት። ተመሳሳዩን TLVs/ኤንቨሎፕ ይይዛል ነገርግን ይዘልቃል
  ለዝቅተኛ ባንድዊድዝ ዝግጅት ምልክት እና ጣሪያ።

## ቅድመ ዝግጅት ማጠቃለያ

| መገለጫ | ምልክት አድርግ (ሚሴ) | ሕዋስ (B) | ሌይን ካፕ | ዱሚ ወለል | የሌይን ክፍያ (Mb/s) | የጣሪያ ጭነት (Mb/s) | ጣሪያ % ወደላይ ማገናኛ | የሚመከር uplink (Mb/s) | የጎረቤት ቆብ | ቀስቅሴን (%) |
|-------------|-------
| አንኳር | 5.0 | 1024 | 12 | 4 | 1.64 | 19.50 | 65 | 30.0 | 8 | 85 |
| ቤት | 10.0 | 1024 | 4 | 2 | 0.82 | 4.00 | 40 | 10.0 | 2 | 70 |
| ባዶ | 20.0 | 1024 | 2 | 1 | 0.41 | 0.75 | 15 | 5.0 | 1 | 55 |

- ** የሌይን ካፕ *** - ከፍተኛው በተመሳሳይ ቋሚ-ተመን ጎረቤቶች። ማስተላለፊያው አንድ ጊዜ ተጨማሪ ወረዳዎችን ውድቅ ያደርጋል
  ቆብ ተመታ እና `soranet_handshake_capacity_reject_total` ይጨምራል።
- **ዱሚ ወለል** - ትክክለኛ ቢሆንም እንኳ ከደማቅ ትራፊክ ጋር በሕይወት የሚቆዩ ዝቅተኛው የመንገዶች ብዛት
  ፍላጎት ዝቅተኛ ነው.
- ** የጣሪያ ጭነት *** - ጣሪያውን ከተጠቀሙ በኋላ ለቋሚ-ተመን መስመሮች የተወሰነ የበጀት ወጪ
  ክፍልፋይ ተጨማሪ የመተላለፊያ ይዘት ቢኖርም ኦፕሬተሮች ከዚህ በጀት መብለጥ የለባቸውም።
- ** ቀስቅሴን በራስ-ሰር አሰናክል *** - ዘላቂ ሙሌት መቶኛ (በቅድመ ቅምጥ አማካይ)
  ወደ ዳሚው ወለል የሚወርድበት ጊዜ። የመልሶ ማግኛ ገደብ በኋላ አቅም ወደነበረበት ተመልሷል
  (75% ለ I18NI0000005X፣ 60% ለ`home`፣ 45% ለ`null`)።

** አስፈላጊ፡** የ`null` ቅድመ ዝግጅት ለዝግጅት እና ችሎታ የሙከራ ምግብ ብቻ ነው። አያሟላም
ለምርት ወረዳዎች የግላዊነት ዋስትናዎች ያስፈልጋሉ።

## ምልክት ያድርጉ -> የመተላለፊያ ይዘት ሰንጠረዥ

እያንዳንዱ የመጫኛ ሴል 1,024 ቢ ይይዛል፣ ስለዚህ የኪቢ/ሰከንድ አምድ በእያንዳንዱ ከሚለቀቁት የሴሎች ብዛት ጋር እኩል ነው።
ሁለተኛ. ጠረጴዛውን በብጁ መዥገሮች ለማራዘም አጋዥውን ይጠቀሙ።

| ምልክት አድርግ (ሚሴ) | ሕዋሳት/ሰከንድ | ጭነት ኪቢ/ሰከንድ | ጭነት Mb/s |
|--------|--------|
| 5.0 | 200.00 | 200.00 | 1.64 |
| 7.5 | 133.33 | 133.33 | 1.09 |
| 10.0 | 100.00 | 100.00 | 0.82 |
| 15.0 | 66.67 | 66.67 | 0.55 |
| 20.0 | 50.00 | 50.00 | 0.41 |

ቀመር፡

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

CLI ረዳት፡

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

`--format markdown` ለቅድመ ዝግጅት ማጠቃለያ እና ለአማራጭ ምልክት ማጭበርበር የ GitHub አይነት ሰንጠረዦችን ያወጣል
ሉህ ስለዚህ ወሳኙን ውጤት በፖርታሉ ውስጥ መለጠፍ ይችላሉ። ለማህደር ከ`--json-out` ጋር ያጣምሩት።
ለአስተዳደር ማስረጃ የቀረበው መረጃ.

## ማዋቀር እና መሻር

`tools/soranet-relay` በሁለቱም የማዋቀር ፋይሎች እና የሩጫ ጊዜ መሻሮች ውስጥ ያሉትን ቅድመ-ቅምጦች ያጋልጣል፡

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

የማዋቀር ቁልፉ I18NI0000012X፣ `home`፣ ወይም `null` (ነባሪው `core`) ይቀበላል። CLI መሻር ጠቃሚ ነው።
ውቅሮችን እንደገና ሳይጽፉ የግዴታ ዑደቱን በጊዜያዊነት የሚቀንሱ የዝግጅት ልምምዶች ወይም የ SOC ጥያቄዎች።

## MTU የጥበቃ መንገዶች

- የመጫኛ ሴሎች 1,024 B ሲደመር ~96 B ከ I18NT0000000X+ ጫጫታ ፍሬም እና አነስተኛውን የQUIC/UDP ራስጌዎችን ይጠቀማሉ።
  እያንዳንዱን ዳታግራም ከ IPv6 1,280 B ቢያንስ MTU በታች ማቆየት።
- ዋሻዎች (WireGuard/IPsec) ተጨማሪ ማቀፊያ ሲጨምሩ **መቀነስ አለብህ** `padding.cell_size`
  ስለዚህ `cell_size + framing <= 1,280 B`. የዝውውር አረጋጋጭ ያስፈጽማል
  `padding.cell_size <= 1,136 B` (1,280 B - 48 B UDP/IPv6 overhead - 96 B ፍሬም)።
- `core` መገለጫዎች ስራ ፈት እያሉም ቢሆን 4 ጎረቤቶችን መሰካት አለባቸው ስለዚህ ደብዛዛ መስመሮች ሁል ጊዜ የንዑስ ስብስብን ይሸፍናሉ
  PQ ጠባቂዎች. `home` መገለጫዎች የቋሚ-ተመን ዑደቶችን በኪስ ቦርሳ/ተሰብሳቢዎች ሊገድቡ ይችላሉ ነገር ግን መተግበር አለባቸው
  ለሶስት ቴሌሜትሪ መስኮቶች ሙሌት ከ 70% በላይ በሚሆንበት ጊዜ የኋላ ግፊት።

## ቴሌሜትሪ እና ማንቂያዎች

ማስተላለፎች በአንድ ቅድመ ዝግጅት የሚከተሉትን መለኪያዎች ወደ ውጭ ይልካሉ፡

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

መቼ ማንቂያ፦

1. Dummy ሬሾ ከተዘጋጀው ወለል በታች (`core >= 4/8`፣ `home >= 2/2`፣ `null >= 1/1`) በላይ ይቆያል።
   ሁለት መስኮቶች.
2. `soranet_constant_rate_ceiling_hits_total` በአምስት ደቂቃ ከአንድ ምት በላይ በፍጥነት ያድጋል።
3. `soranet_constant_rate_degraded` ከታቀደው መሰርሰሪያ ውጭ ወደ `1` ይገለብጣል።

ኦዲተሮች ቋሚ-ተመንን ማረጋገጥ እንዲችሉ የቅድመ ዝግጅት መለያውን እና የጎረቤቶችን ዝርዝር በክስተቶች ሪፖርቶች ውስጥ ይመዝግቡ
ፖሊሲዎች ከፍኖተ ካርታ መስፈርቶች ጋር ይዛመዳሉ።