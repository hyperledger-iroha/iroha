---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cac03d504c6a7dcfacaa4b298e14f0a71ccbcb5ec58f1977b5bf124300c8ec61
source_last_modified: "2026-01-05T09:28:11.865094+00:00"
translation_last_reviewed: 2026-02-07
id: developer-deployment
title: SoraFS Deployment Notes
sidebar_label: Deployment Notes
description: Checklist for promoting the SoraFS pipeline from CI to production.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# የማሰማራት ማስታወሻዎች

የ SoraFS የማሸጊያ የስራ ፍሰት ቆራጥነትን ያጠነክራል፣ ስለዚህ ከCI ወደ
ማምረት በዋናነት የሚሠራ የጥበቃ መንገዶችን ይፈልጋል። ይህንን የማረጋገጫ ዝርዝር መቼ ይጠቀሙ
መሣሪያውን ወደ እውነተኛ መግቢያዎች እና ማከማቻ አቅራቢዎች በማዞር ላይ።

## ቅድመ በረራ

- ** የመዝገብ ቤት አሰላለፍ *** - የ chunker መገለጫዎችን ያረጋግጡ እና የማጣቀሻውን ያሳያል
  ተመሳሳይ `namespace.name@semver` tuple (I18NI0000006X).
- ** የመግቢያ ፖሊሲ *** - የተፈረመውን አቅራቢ ማስታወቂያ እና ተለዋጭ ማስረጃዎችን ይከልሱ
  ለ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`) ያስፈልጋል።
- ** የፒን መመዝገቢያ መሮጫ መጽሐፍ *** - `docs/source/sorafs/runbooks/pin_registry_ops.md` አቆይ
  ለመልሶ ማግኛ ሁኔታዎች (ተለዋዋጭ ማሽከርከር ፣ ማባዛት አለመሳካቶች) ምቹ።

## የአካባቢ ውቅር

- መተላለፊያ መንገዶች የማረጋገጫ ዥረት መጨረሻ ነጥብ (`POST /v2/sorafs/proof/stream`) ማንቃት አለባቸው
  ስለዚህ CLI የቴሌሜትሪ ማጠቃለያዎችን ሊያወጣ ይችላል።
በ ውስጥ ያሉትን ነባሪዎች በመጠቀም የ`sorafs_alias_cache` ፖሊሲን ያዋቅሩ
  `iroha_config` ወይም የ CLI አጋዥ (`sorafs_cli manifest submit --alias-*`)።
- የዥረት ማስመሰያዎች (ወይም Torii ምስክርነቶችን) በአስተማማኝ ሚስጥራዊ አስተዳዳሪ በኩል ያቅርቡ።
- ቴሌሜትሪ ላኪዎችን አንቃ (`torii_sorafs_proof_stream_*`፣
  `torii_sorafs_chunk_range_*`) እና ወደ የእርስዎ I18NT0000000X/OTel ቁልል ይላካቸው።

## የልቀት ስትራቴጂ

1. **ሰማያዊ/አረንጓዴ መገለጫዎች**
   - ለእያንዳንዱ የታቀደ ምላሾችን ለማስቀመጥ `manifest submit --summary-out` ይጠቀሙ።
   - ችሎታን ለመያዝ `torii_sorafs_gateway_refusals_total` ይከታተሉ
     ቀደም ብሎ አለመመጣጠን።
2. **የማረጋገጫ ማረጋገጫ**
   - በ `sorafs_cli proof stream` ውስጥ ያሉ ውድቀቶችን እንደ ማሰማሪያ ማገጃዎች ይያዙ; መዘግየት
     ሾጣጣዎች ብዙውን ጊዜ የአቅራቢዎች መጨናነቅን ወይም የተሳሳቱ ደረጃዎችን ያመለክታሉ።
   - `proof verify` CARን ለማረጋገጥ የድህረ-ፒን ጭስ ሙከራ አካል መሆን አለበት
     በአቅራቢዎች የሚስተናገደው አሁንም ከማንፀባረቂያው መፈጨት ጋር ይዛመዳል።
3. **የቴሌሜትሪ ዳሽቦርዶች**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` ወደ I18NT0000001X አስመጣ።
   - ለፒን መዝገብ ቤት ጤና ተጨማሪ ፓነሎችን ንብር
     (`docs/source/sorafs/runbooks/pin_registry_ops.md`) እና ቸንክ ክልል ስታቲስቲክስ።
4. **ባለብዙ ምንጭ ማስቻል**
   - የታቀዱትን የታቀዱ ደረጃዎችን ይከተሉ
     `docs/source/sorafs/runbooks/multi_source_rollout.md` ሲበራ
     ኦርኬስትራ፣ እና ለኦዲት የውጤት ሰሌዳ/ቴሌሜትሪ ቅርሶችን በማህደር ያስቀምጡ።

## የአደጋ አያያዝ

- በ `docs/source/sorafs/runbooks/` ውስጥ የመጨመር መንገዶችን ይከተሉ:
  - `sorafs_gateway_operator_playbook.md` ለጌትዌይ መቋረጥ እና የዥረት ማስመሰያ
    ድካም.
  - `dispute_revocation_runbook.md` የማባዛት አለመግባባቶች ሲከሰቱ።
  - `sorafs_node_ops.md` የመስቀለኛ ደረጃ ጥገና።
  - `multi_source_rollout.md` ለኦርኬስትራ መሻሮች፣ የአቻ ጥቁር መዝገብ እና
    የታቀዱ ልቀቶች ።
- በነባሩ በኩል በ GovernanceLog ውስጥ የማስረጃ ውድቀቶችን እና የቆይታ ጉድለቶችን ይመዝግቡ
  አስተዳደር የአቅራቢውን አፈጻጸም መገምገም እንዲችል የPoR መከታተያ ኤፒአይዎች።

## ቀጣይ እርምጃዎች

- ኦርኬስትራ አውቶሜሽን (`sorafs_car::multi_fetch`) አንድ ጊዜ ያዋህዱ
  ባለብዙ ምንጭ ፈልሳፊ ኦርኬስትራ (SF-6b) መሬቶች።
- በSF-13/SF-14 ስር የ PDP/PoTR ማሻሻያዎችን ይከታተሉ። CLI እና ሰነዶች ወደ ዝግመተ ለውጥ ይሄዳሉ
  የገጽታ ማብቂያ ጊዜዎች እና የደረጃ ምርጫ እነዚያ ማረጋገጫዎች ከተረጋጉ በኋላ።

እነዚህን የማሰማራት ማስታወሻዎች ከፈጣን ጅምር እና ከCI የምግብ አዘገጃጀት መመሪያዎች፣ ቡድኖች ጋር በማጣመር
ከአካባቢያዊ ሙከራዎች ወደ ምርት ደረጃ SoraFS የቧንቧ መስመሮች ከኤ
ሊደገም የሚችል, የሚታይ ሂደት.