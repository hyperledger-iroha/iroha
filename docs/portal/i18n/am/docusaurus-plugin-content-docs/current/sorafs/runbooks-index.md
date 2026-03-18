---
id: runbooks-index
lang: am
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Operator Runbooks Index
description: Canonical entry point for the migrated SoraFS operator runbooks.
sidebar_label: Runbook Index
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> በ`docs/source/sorafs/runbooks/` ስር የሚኖረውን የባለቤት ደብተር ያንጸባርቃል።
> እያንዳንዱ አዲስ የI18NT0000000X ኦፕሬሽን መመሪያ አንዴ ከወጣ እዚህ ጋር መያያዝ አለበት።
> ፖርታል ግንባታ።

የትኛዎቹ runbooks ከ ፍልሰት እንዳጠናቀቁ ለማረጋገጥ ይህን ገጽ ይጠቀሙ
ምንጭ ዱካ፣ እና የፖርታል ቅጂው ስለዚህ ገምጋሚዎች በቀጥታ ወደሚፈለገው መዝለል ይችላሉ።
በቅድመ-ይሁንታ ቅድመ እይታ ወቅት መመሪያ።

## የቅድመ-ይሁንታ አስተናጋጅ

የDocOps ሞገድ አሁን በግምገማ የጸደቀውን የቅድመ-ይሁንታ አስተናጋጅ በ ላይ አስተዋውቋል
`https://docs.iroha.tech/`. ኦፕሬተሮችን ወይም ገምጋሚዎችን ወደተሰደደ ሲጠቁም።
runbook፣ የአስተናጋጅ ስም በማጣቀስ በቼክሰም-የተከለለ ፖርታል እንዲለማመዱ
ቅጽበታዊ ገጽ እይታ የማተም/የመልሶ መመለስ ሂደቶች በቀጥታ ስርጭት
[`devportal/preview-host-exposure`](I18NU0000002X)።

| Runbook | ባለቤት(ዎች) | ፖርታል ቅጂ | ምንጭ |
|--------|-----------|------------|--------|
| ጌትዌይ እና ዲ ኤን ኤስ መጀመር | የአውታረ መረብ ግንኙነት TL፣ Ops Automation፣ Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS ኦፕሬሽኖች መጫወቻ መጽሐፍ | ሰነዶች/DevRel | [`sorafs/operations-playbook`](I18NU0000004X) | `docs/source/sorafs/operations_playbook.md` |
| የአቅም ማስታረቅ | ግምጃ ቤት / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| ፒን መዝገብ ቤት ops | Tooling WG | [`sorafs/pin-registry-ops`](I18NU0000006X) | `docs/source/sorafs/pin_registry_ops.md` |
| የመስቀለኛ መንገድ ስራዎች ማረጋገጫ ዝርዝር | የማከማቻ ቡድን, SRE | [`sorafs/node-operations`](I18NU0000007X) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| ክርክር እና መሻር runbook | አስተዳደር ምክር ቤት | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| አንጸባራቂ የመጫወቻ መጽሐፍ | ሰነዶች/DevRel | [`sorafs/staging-manifest-playbook`](I18NU0000009X) | `docs/source/sorafs/staging_manifest_playbook.md` |
| ታይካይ መልህቅ ታዛቢነት | የሚዲያ መድረክ WG / DA ፕሮግራም / አውታረ መረብ TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## የማረጋገጫ ዝርዝር

- [x] ወደዚህ መረጃ ጠቋሚ (የጎን አሞሌ መግቢያ) ፖርታል ግንባታ አገናኞች።
- [x] እያንዳንዱ የተሰደደ runbook ገምጋሚዎችን ለማቆየት የቀኖናዊ ምንጭ ዱካ ይዘረዝራል።
  በዶክ ግምገማዎች ጊዜ የተጣጣመ.
- [x] የDocOps ቅድመ እይታ የቧንቧ መስመር ብሎኮች የተዘረዘረው የሩጫ መጽሐፍ ሲጠፋ ይዋሃዳሉ
  ከፖርታል ውፅዓት.

የወደፊት ፍልሰት (ለምሳሌ፣ አዲስ ትርምስ ልምምዶች ወይም የአስተዳደር ተጨማሪዎች) መጨመር አለባቸው ሀ
ከላይ ባለው ሠንጠረዥ ረድፉ እና የተከተተውን የ DocOps ማረጋገጫ ዝርዝር ያዘምኑ
`docs/examples/docs_preview_request_template.md`.