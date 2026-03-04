---
id: preview-feedback-w1-summary
lang: am
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W1 partner feedback & exit summary
sidebar_label: W1 summary
description: Findings, actions, and exit evidence for the partner/Torii integrator preview wave.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| ንጥል | ዝርዝሮች |
| --- | --- |
| ማዕበል | W1 - አጋሮች እና I18NT0000001X integrators |
| የግብዣ መስኮት | 2025-04-12 → 2025-04-26 |
| Artefact መለያ | `preview-2025-04-12` |
| የመከታተያ ጉዳይ | `DOCS-SORA-Preview-W1` |
| ተሳታፊዎች | sorafs-op-01…03፣ torii-int-01…02፣ sdk-partner-01…02፣ ጌትዌይ-ኦፕስ-01 |

## ድምቀቶች

1. ** Checksum የስራ ፍሰት *** - ሁሉም ገምጋሚዎች ገላጭውን/ማህደሩን በI18NI0000006X አረጋግጠዋል። ምዝግብ ማስታወሻዎች ከግብዣ እውቅናዎች ጋር ተከማችተዋል።
2. ** ቴሌሜትሪ *** - `docs.preview.integrity`፣ `TryItProxyErrors`፣ እና `DocsPortal/GatewayRefusals` ዳሽቦርዶች ለጠቅላላው ሞገድ አረንጓዴ ሆነው ቆዩ። ምንም አይነት ክስተቶች ወይም ማንቂያ ገጾች አልተተኮሱም።
3. ** የሰነድ ግብረመልስ (`docs-preview/w1`)** - ሁለት ጥቃቅን ኒቶች ገብተዋል፡-
   - `docs-preview/w1 #1`: ይሞክሩት ክፍል (የተፈታ) ውስጥ የናቭ ቃላትን ግልጽ ያድርጉ።
   - `docs-preview/w1 #2`: አዘምን ቅጽበታዊ ገጽ እይታን ይሞክሩት (የተፈታ)።
4. ** Runbook ተመሳሳይነት ** — SoraFS ኦፕሬተሮች በ `orchestrator-ops` እና `multi-source-rollout` መካከል ያለውን አዲስ ማቋረጫ የ W0 ስጋታቸውን አረጋግጠዋል።

## የእርምጃ ዕቃዎች

| መታወቂያ | መግለጫ | ባለቤት | ሁኔታ |
| --- | --- | --- | --- |
| W1-A1 | አዘምን በ `docs-preview/w1 #1` የቃላት አጻጻፍ ይሞክሩት። | ሰነዶች-ኮር-02 | ✅ ተጠናቋል (2025-04-18)። |
| W1-A2 | አድስ በ`docs-preview/w1 #2` ቅጽበታዊ ገጽ እይታ ይሞክሩት። | ሰነዶች-ኮር-03 | ✅ ተጠናቅቋል (2025-04-19)። |
| W1-A3 | የመንገድ ካርታ/ሁኔታ ላይ የአጋር ግኝቶችን + የቴሌሜትሪ ማስረጃዎችን ማጠቃለል። | ሰነዶች/DevRel አመራር | ✅ ተጠናቅቋል (tracker + status.md ይመልከቱ)። |

## የመውጣት ማጠቃለያ (2025-04-26)

- ስምንቱም ገምጋሚዎች በመጨረሻው የስራ ሰዓት መጠናቀቁን አረጋግጠዋል፣ የሀገር ውስጥ ቅርሶችን አጽድተዋል እና መዳረሻቸው ተሰርዟል።
- ቴሌሜትሪ በመውጣት አረንጓዴ ሆኖ ቆይቷል; የመጨረሻ ቅጽበተ-ፎቶዎች ከ ​​`DOCS-SORA-Preview-W1` ጋር ተያይዘዋል።
- የግብዣ ምዝግብ ማስታወሻ በመውጫ እውቅናዎች የዘመነ; መከታተያ W1ን ወደ 🈴 ገለበጠ እና የፍተሻ ነጥቡን ጨምሯል።
-የማስረጃ ጥቅል (ገላጭ፣ የቼክሰም ሎግ፣ የፍተሻ ውፅዓት፣ ይሞክሩት ተኪ ትራንስክሪፕት፣ ቴሌሜትሪ ስክሪፕቶች፣ የግብረ-መልስ መፍጫ) በ`artifacts/docs_preview/W1/` ስር ተቀምጧል።

## ቀጣይ እርምጃዎች

- የW2 ማህበረሰብ ቅበላ እቅድ ያዘጋጁ (የመንግስት ማፅደቅ + የአብነት ማስተካከያዎች)።
- ለ W2 ሞገድ የቅድመ እይታ አርቲፊክት መለያን ያድሱ እና ቀኖች እንደጨረሱ የቅድመ በረራ ስክሪፕቱን እንደገና ያስጀምሩ።
- የሚመለከታቸው የW1 ግኝቶች ወደ ፍኖተ ካርታ/ሁኔታ የማህበረሰብ ማዕበል የቅርብ መመሪያ እንዲኖረው።