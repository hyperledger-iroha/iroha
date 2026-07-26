---
id: preview-feedback-w0-summary
lang: am
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W0 midpoint feedback digest
sidebar_label: W0 feedback (midpoint)
description: Midpoint checkpoints, findings, and action items for the core-maintainer preview wave.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| ንጥል | ዝርዝሮች |
| --- | --- |
| ማዕበል | W0 - ኮር ጠባቂዎች |
| የተፈጨበት ቀን | 2025-03-27 |
| የግምገማ መስኮት | 2025-03-25 → 2025-04-08 |
| ተሳታፊዎች | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| Artefact መለያ | `preview-2025-03-24` |

## ድምቀቶች

1. **የቼክተም የስራ ፍሰት** — ሁሉም ገምጋሚዎች `scripts/preview_verify.sh` አረጋግጠዋል
   ከጋራ ገላጭ/የመዝገብ ቤት ጥንድ ጋር ተቃርቧል። በእጅ የሚሻር የለም።
   ያስፈልጋል።
2. **የአሰሳ ግብረመልስ** — ሁለት ጥቃቅን የጎን አሞሌ ማዘዣ ጉዳዮች ቀርበዋል።
   (`docs-preview/w0 #1–#2`)። ሁለቱም ወደ ሰነዶች/DevRel ተወስደዋል እና አያግዱም።
   ሞገድ.
3. **I18NT0000000X runbook ተመሳሳይነት** — sorafs-ops-01 ይበልጥ ግልጽ የሆኑ ማገናኛዎች ጠይቋል።
   በ `sorafs/orchestrator-ops` እና `sorafs/multi-source-rollout` መካከል። ክትትል
   ጉዳይ ቀረበ; ከ W1 በፊት መቅረብ አለበት.
4. ** የቴሌሜትሪ ግምገማ *** - ታዛቢነት-01 የተረጋገጠ `docs.preview.integrity`፣
   `TryItProxyErrors`፣ እና ይሞክሩት የተኪ ምዝግብ ማስታወሻዎች አረንጓዴ ሆነው ቆዩ። ምንም ማንቂያዎች አልተተኮሱም።

## የእርምጃ ዕቃዎች

| መታወቂያ | መግለጫ | ባለቤት | ሁኔታ |
| --- | --- | --- | --- |
| W0-A1 | የዴቭፖርታል የጎን አሞሌ ግቤቶችን ወደ ገጽ ገምጋሚ-ተኮር ሰነዶች (`preview-invite-*` ቡድን አንድ ላይ) እንደገና ይዘዙ። | ሰነዶች-ኮር-01 | ✅ ተጠናቋል - የጎን አሞሌ አሁን የገምጋሚውን ሰነዶች በተከታታይ ይዘረዝራል (`docs/portal/sidebars.js`)። |
| W0-A2 | በ`sorafs/orchestrator-ops` እና `sorafs/multi-source-rollout` መካከል ግልጽ የሆነ መስቀለኛ መንገድ ያክሉ። | Sorafs-ops-01 | ✅ ተጠናቅቋል - እያንዳንዱ የሩጫ መጽሐፍ አሁን ከሌላው ጋር ይገናኛል ስለዚህ ኦፕሬተሮች በታቀዱ ጊዜ ሁለቱንም መመሪያዎች ያዩታል። |
| W0-A3 | የቴሌሜትሪ ቅጽበተ-ፎቶዎችን + የጥያቄ ቅርቅብ ከአስተዳደር መከታተያ ጋር ያጋሩ። | ታዛቢነት-01 | ✅ ተጠናቅቋል - ጥቅል ከ`DOCS-SORA-Preview-W0` ጋር ተያይዟል። |

## የመውጣት ማጠቃለያ (2025-04-08)

- አምስቱም ገምጋሚዎች መጠናቀቁን አረጋግጠዋል፣ የአካባቢ ግንባታዎችን አጽድተው ከወጡበት ወጥተዋል።
  ቅድመ እይታ መስኮት; በ`DOCS-SORA-Preview-W0` ውስጥ የተመዘገቡ የመዳረሻ ስረዛዎች።
- በማዕበል ወቅት ምንም አይነት ክስተቶች ወይም ማንቂያዎች አልተነሱም; ቴሌሜትሪ ዳሽቦርዶች ቆዩ
  ለሙሉ ጊዜ አረንጓዴ.
- አሰሳ + አቋራጭ እርምጃዎች (W0-A1/A2) ተተግብረዋል እና ተንጸባርቀዋል
  ከላይ ያሉት ሰነዶች; የቴሌሜትሪ ማስረጃ (W0-A3) ከመከታተያው ጋር ተያይዟል።
- የማስረጃ ጥቅል በማህደር ተቀምጧል፡ ቴሌሜትሪ ቅጽበታዊ ገጽ እይታዎች፣ እውቅናዎችን ይጋብዙ እና
  ይህ መፍጨት ከክትትል ጉዳይ ጋር የተገናኘ ነው።

## ቀጣይ እርምጃዎች

- W1 ከመክፈትዎ በፊት የ W0 የድርጊት እቃዎችን ይተግብሩ።
- ህጋዊ ማረጋገጫ እና የተኪ ማቆያ ቦታ ያግኙ፣ ከዚያ የአጋር-ሞገድን ይከተሉ
  የቅድመ በረራ እርምጃዎች በ [የቅድመ እይታ ግብዣ ፍሰት](../../preview-invite-flow.md) ውስጥ ተዘርዝረዋል።

_ይህ መፈጨት ከ[የቅድመ እይታ ግብዣ መከታተያ](../../preview-invite-tracker.md) ጋር የተገናኘ ነው።
የ DOCS-SORA ፍኖተ ካርታ መከታተያ እንዲኖር ያድርጉ።_