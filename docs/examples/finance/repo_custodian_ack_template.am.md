---
lang: am
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c52d7f2c5ec9dc4cda81895561bc1261659935c94bf3f7febb0867f4981fe616
source_last_modified: "2026-01-22T16:26:46.472177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo ሞግዚት የምስጋና አብነት

ሪፖ (የሁለትዮሽ ወይም የሶስትዮሽ ፓርቲ) ጠባቂን ሲጠቅስ ይህን አብነት ይጠቀሙ
በ `RepoAgreement::custodian` በኩል. ግቡ የማሳደጊያ SLA, ማዘዋወር መመዝገብ ነው
ንብረቶቹ ከመንቀሳቀስዎ በፊት መለያዎች እና ግንኙነቶችን ይሰርዙ። አብነቱን ወደ እርስዎ ይቅዱ
የማስረጃ ማውጫ (ለምሳሌ
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`)፣ መሙላት
ቦታ ያዢዎች፣ እና ፋይሉን ሃሽ እንደ የአስተዳደር ፓኬት አካል በተገለጸው ውስጥ
`docs/source/finance/repo_ops.md` §2.8.

## 1. ሜታዳታ

| መስክ | ዋጋ |
|-------|------|
| የስምምነት መለያ | `<repo-yyMMdd-XX>` |
| የጠባቂ መለያ መታወቂያ | `<ih58...>` |
| የተዘጋጀው በ / ቀን | `<custodian ops lead>` |
| የዴስክ እውቂያዎች እውቅና አግኝተዋል | `<desk lead + counterparty>` |
| የማስረጃ ማውጫ | ``artifacts/finance/repo/<slug>/`` |

## 2. የጥበቃ ወሰን

- ** የዋስትና መግለጫዎች ተቀብለዋል: ** `<list of asset definition ids>`
- ** የገንዘብ እግር ምንዛሬ / የሰፈራ ባቡር፡** `<xor#sora / other>`
- ** የጥበቃ መስኮት: ** `<start/end timestamps or SLA summary>`
- ** ቋሚ መመሪያዎች: ** `<hash + path to standing instruction document>`
- ** ራስ-ሰር ቅድመ-ሁኔታዎች-** `<scripts, configs, or runbooks custodian will invoke>`

## 3. መስመር እና ክትትል

| ንጥል | ዋጋ |
|--------|
| የጥበቃ የኪስ ቦርሳ / ደብተር መለያ | `<asset ids or ledger path>` |
| መከታተያ ቻናል | `<Slack/phone/on-call rotation>` |
| የመሰርሰሪያ ግንኙነት | `<primary + backup>` |
| አስፈላጊ ማንቂያዎች | `<PagerDuty service, Grafana board, etc.>` |

## 4. መግለጫዎች

1. * የጥበቃ ዝግጁነት፡* “የደረጃውን የ `repo initiate` ክፍያን ከ
   ከላይ ያሉት ለዪዎች እና በተዘረዘረው SLA መሰረት ዋስትና ለመቀበል ተዘጋጅተዋል።
   በ§2 ውስጥ።
2. *የመልሶ መልስ ቁርጠኝነት፡* “ከላይ የተጠቀሰውን የመልሶ ማጫወቻ መጽሐፍ እናስፈጽማለን።
   በአደጋው አዛዥ ተመርቷል፣ እና የCLI ሎጎችን እና ሃሾችን ይሰጣል
   `governance/drills/<timestamp>.log`።
3. *የማስረጃ ማቆየት፡* “እውቅናውን እንቆማለን፣ እንቆማለን።
   መመሪያዎችን እና የ CLI ምዝግብ ማስታወሻዎችን ቢያንስ ለ `<duration>` እና ለ
   የፋይናንስ ምክር ቤት ሲጠየቅ።

ከዚህ በታች ይፈርሙ (በኤሌክትሮኒክ ፊርማዎች በአስተዳደር ሲተላለፉ ተቀባይነት አላቸው።
መከታተያ)።

| ስም | ሚና | ፊርማ / ቀን |
|-------------|---|
| `<custodian ops lead>` | ሞግዚት ኦፕሬተር | `<signature>` |
| `<desk lead>` | ዴስክ | `<signature>` |
| `<counterparty>` | ተቃዋሚ | `<signature>` |

> አንዴ ከተፈረመ ፋይሉን ሃሽ (ለምሳሌ `sha256sum custodian_ack_<cust>.md`) እና
> ገምጋሚዎች ማረጋገጥ እንዲችሉ በአስተዳደር ፓኬት ሠንጠረዥ ውስጥ የምግብ መፍጫውን ይመዝግቡ
> በድምጽ መስጫ ጊዜ ተጠቅሷል እውቅና ባይት።