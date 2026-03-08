---
lang: am
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Local → Global Address Toolkit
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ይህ ገጽ [`docs/source/sns/local_to_global_toolkit.md`](../../../source/sns/local_to_global_toolkit.md) ያንጸባርቃል
ከሞኖ-ሪፖ. በፍኖተ ካርታ ንጥል ** ADDR-5c** የሚፈለጉትን የCLI አጋዥ እና የሩጫ መጽሐፍትን ያጠቃልላል።

## አጠቃላይ እይታ

- `scripts/address_local_toolkit.sh` `iroha` CLI ይጠቀልላል፡-
  - `audit.json` - ከ I18NI0000009X የተዋቀረ ውፅዓት።
  - `normalized.txt` — የተቀየረ ተመራጭ IH58/ሁለተኛ-ምርጥ የታመቀ (`sora`) ቃል በቃል ለእያንዳንዱ የአካባቢ-ጎራ መራጭ።
- ስክሪፕቱን ከአድራሻ ኢንጌስት ዳሽቦርድ (`dashboards/grafana/address_ingest.json`) ጋር ያጣምሩ
  እና የአለርትማኔጀር ደንቦች (`dashboards/alerts/address_ingest_rules.yml`) የአካባቢ-8 / ለማረጋገጥ
  የአካባቢ-12 መቁረጫ ደህንነቱ የተጠበቀ ነው። የአካባቢ-8 እና የአካባቢ-12 የግጭት ፓነሎች እና የ
  `AddressLocal8Resurgence`፣ `AddressLocal12Collision`፣ እና `AddressInvalidRatioSlo` ማንቂያዎች በፊት
  አንጸባራቂ ለውጦችን ማስተዋወቅ።
- [የአድራሻ ማሳያ መመሪያዎችን](address-display-guidelines.md) እና
  [የአድራሻ ማንፌስት runbook](../../../source/runbooks/address_manifest_ops.md) ለ UX እና ለአደጋ ምላሽ አውድ።

##አጠቃቀም

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format ih58
```

አማራጮች፡-

- `--format compressed` ለ `sora…` ውፅዓት ከ IH58 ይልቅ።
- ባዶ ቃል በቃል ለመልቀቅ `domainless output (default)`።
- የልወጣ ደረጃን ለመዝለል `--audit-only`።
- `--allow-errors` የተበላሹ ረድፎች ሲታዩ መቃኘቱን ለመቀጠል (ከ CLI ባህሪ ጋር ይዛመዳል)።

ስክሪፕቱ በሩጫው መጨረሻ ላይ የስነ ጥበብ መንገዶችን ይጽፋል። ሁለቱንም ፋይሎች ያያይዙ
የእርስዎ የለውጥ አስተዳደር ትኬት ከ Grafana ቅጽበታዊ ገጽ እይታ ጋር ዜሮ መሆኑን ያረጋግጣል
የአካባቢ-8 ግኝቶች እና ዜሮ አካባቢያዊ-12 ግጭቶች ለ≥30 ቀናት።

## CI ውህደት

1. ስክሪፕቱን በልዩ ሥራ ውስጥ ያሂዱ እና ውጤቱን ይስቀሉ።
2. I18NI0000022X የአካባቢያዊ መራጮችን (`domain.kind = local12`) ሲዘግብ ይዋሃዳል።
   በነባሪ የ`true` ዋጋ (ወደ `false` በdev/የሙከራ ስብስቦች ላይ መሻር ብቻ
   መመለሻዎችን መመርመር) እና መጨመር
   `iroha tools address normalize` ወደ CI ስለዚህ regression
   ምርቱን ከመምታቱ በፊት ሙከራዎች አልተሳኩም.

ለተጨማሪ ዝርዝሮች፣ የናሙና ማስረጃዎችን ዝርዝር እና የመልቀቂያ-ማስታወሻ ቅንጣቢውን ለደንበኞች ሲያስታውቁ እንደገና ለመጠቀም የምንጭ ሰነዱን ይመልከቱ።