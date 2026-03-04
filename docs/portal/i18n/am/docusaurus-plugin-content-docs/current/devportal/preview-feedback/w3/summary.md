---
id: preview-feedback-w3-summary
lang: am
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W3 beta feedback & status
sidebar_label: W3 summary
description: Live digest for the 2026 beta preview wave (finance, observability, SDK, and ecosystem cohorts).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| ንጥል | ዝርዝሮች |
| --- | --- |
| ማዕበል | W3 — የቅድመ-ይሁንታ ስብስብ (ፋይናንስ + ኦፕስ + ኤስዲኬ አጋር + የስነ-ምህዳር ጠበቃ) |
| የግብዣ መስኮት | 2026-02-18 → 2026-02-28 |
| Artefact መለያ | `preview-20260218` |
| የመከታተያ ጉዳይ | `DOCS-SORA-Preview-W3` |
| ተሳታፊዎች | ፋይናንስ-ቤታ-01፣ ታዛቢነት-ops-02፣ አጋር-sdk-03፣ ምህዳር-ተሟጋች-04 |

## ድምቀቶች

1. **ከጫፍ እስከ ጫፍ የማስረጃ መስመር።** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` የያንዳንዱ ሞገድ ማጠቃለያ (`artifacts/docs_portal_preview/preview-20260218-summary.json`) ዳይጀስት (I18NI0000009X) እና `docs/portal/src/data/previewFeedbackSummary.json`ን ያድሳል ስለዚህ የአስተዳደር ገምጋሚዎች በአንድ ነጠላ ላይ ሊተማመኑ ይችላሉ።
2. **ቴሌሜትሪ + የአስተዳደር ሽፋን።** አራቱም ገምጋሚዎች የቼክሰም-ጋድ መዳረሻን አምነዋል፣ ግብረ መልስ ሰጥተዋል እና በሰዓቱ ተሽረዋል። በማዕበል ወቅት ከተሰበሰቡት የGrafana ሩጫዎች ጎን ለጎን የግብረመልስ ጉዳዮችን (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) ይጠቅሳል።
3. **የፖርታል ንጣፍ።** የታደሰው ፖርታል ሠንጠረዥ አሁን የተዘጋውን W3 ሞገድ ዘግይቶ እና የምላሽ-ተመን መለኪያዎችን ያሳያል፣ እና ከታች ያለው አዲሱ የምዝግብ ማስታወሻ ገጽ ጥሬውን የJSON ሎግ ለማይጎትቱ ኦዲተሮች የዝግጅት ጊዜን ያሳያል።

## የእርምጃ ዕቃዎች

| መታወቂያ | መግለጫ | ባለቤት | ሁኔታ |
| --- | --- | --- | --- |
| W3-A1 | የቅድመ እይታ መፍጨትን ያንሱ እና ከመከታተያ ጋር ያያይዙ። | ሰነዶች/DevRel አመራር | ✅ ተጠናቀቀ 2026-02-28 |
| W3-A2 | ማስረጃዎችን ወደ ፖርታል + የመንገድ ካርታ/ሁኔታ ጋብዝ/መፍጨት። | ሰነዶች/DevRel አመራር | ✅ ተጠናቀቀ 2026-02-28 |

## የመውጫ ማጠቃለያ (2026-02-28)

- ግብዣዎች 2026-02-18 ከደቂቃዎች በኋላ ከተመዘገቡ ምስጋናዎች ጋር; የመጨረሻው የቴሌሜትሪ ፍተሻ ካለፈ በኋላ የቅድመ እይታ መዳረሻ በ2026-02-28 ተሽሯል።
- Digest + ማጠቃለያ በ`artifacts/docs_portal_preview/` ስር ተይዟል፣ ከጥሬ ምዝግብ ማስታወሻው በ `artifacts/docs_portal_preview/feedback_log.json` ለእንደገና መጫወት።
- በአስተዳደር መከታተያ `DOCS-SORA-Preview-20260218` በ `docs-preview/20260218` ስር የቀረቡ ክትትሎችን መስጠት; CSP/ ወደ ታዛቢነት/የፋይናንስ ባለቤቶች የተላለፈ ማስታወሻዎችን ይሞክሩ እና ከማዋሃድ ጋር የተገናኙት።
- የመከታተያ ረድፍ ወደ 🈴 ተጠናቋል እና የፖርታል ግብረመልስ ሠንጠረዥ የተዘጋውን ሞገድ ያንፀባርቃል፣ ቀሪውን DOCS-SORA የቅድመ-ይሁንታ ዝግጁነት ተግባርን ያጠናቅቃል።