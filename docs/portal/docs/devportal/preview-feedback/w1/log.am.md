---
lang: am
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 51971e1dc4e763ac7017f76c7239eef943bc21151e49e827988b61972fa58245
source_last_modified: "2025-12-29T18:16:35.107308+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-log
title: W1 feedback & telemetry log
sidebar_label: W1 feedback log
description: Aggregate roster, telemetry checkpoints, and reviewer notes for the first partner preview wave.
translator: machine-google-reviewed
---

ይህ ምዝግብ ማስታወሻ የግብዣ ዝርዝርን፣ የቴሌሜትሪ ፍተሻ ነጥቦችን እና የግምገማ ግብረመልስን ለ
** W1 የአጋር ቅድመ-እይታ** ከመቀበል ተግባራት ጋር አብሮ የሚሄድ
[`preview-feedback/w1/plan.md`](./plan.md) እና የሞገድ መከታተያ መግቢያ
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md)። በማንኛውም ግብዣ ጊዜ ያዘምኑት።
ተልኳል፣ የቴሌሜትሪ ቅጽበታዊ ገጽ እይታ ተቀርጿል፣ ወይም የአስተዳዳሪ ገምጋሚዎች እንደገና መጫወት እንዲችሉ የግብረመልስ ንጥል ተስተካክሏል።
የውጭ ትኬቶችን ሳያሳድዱ ማስረጃው.

## የቡድን ስም ዝርዝር

| የአጋር መታወቂያ | ትኬት ጠይቅ | NDA ተቀብለዋል | ግብዣ ተልኳል (UTC) | Ack/የመጀመሪያ መግቢያ (UTC) | ሁኔታ | ማስታወሻ |
| --- | --- | --- | --- | --- | --- | --- |
| አጋር-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ ተጠናቀቀ 2025-04-26 | sorafs-op-01; በኦርኬስትራ ዶክተሪ ማስረጃ ላይ ያተኮረ። |
| አጋር-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ ተጠናቀቀ 2025-04-26 | sorafs-op-02; የተረጋገጠ Norito/ቴሌሜትሪ መስቀለኛ መንገድ። |
| አጋር-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ ተጠናቀቀ 2025-04-26 | sorafs-op-03; ባለብዙ ምንጭ ውድቀት ልምምዶችን አካሄደ። |
| አጋር-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ ተጠናቀቀ 2025-04-26 | torii-int-01; Torii `/v1/pipeline` + ይሞክሩት የምግብ አዘገጃጀት መመሪያ። |
| አጋር-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ ተጠናቀቀ 2025-04-26 | torii-int-02; በ ላይ ተጣምሯል ቅጽበታዊ ገጽ እይታን ይሞክሩት (ሰነዶች-ቅድመ እይታ/w1 #2)። |
| አጋር-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ ተጠናቀቀ 2025-04-26 | sdk-አጋር-01; JS/Swift የምግብ አዘገጃጀት መመሪያ + የ ISO ድልድይ የንጽህና ፍተሻዎች። |
| አጋር-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ ተጠናቀቀ 2025-04-26 | sdk-አጋር-02; ተገዢነት ጸድቷል 2025-04-11፣ በ Connect/ቴሌሜትሪ ማስታወሻዎች ላይ ያተኮረ። |
| አጋር-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ ተጠናቀቀ 2025-04-26 | ጌትዌይ-ኦፕስ-01; ኦዲት የተደረገ ጌትዌይ ኦፕስ መመሪያ + ማንነቱ ያልታወቀ የፕሮክሲ ፍሰት ይሞክሩት። |

የወጪ ኢሜል እንደወጣ **ግብዣ የተላከውን *** እና **አክ** ጊዜ ማህተሞችን ይሙሉ።
ሰዓቶቹን በW1 እቅድ ውስጥ በተገለጸው የUTC መርሐግብር ላይ ያስይዙ።

## የቴሌሜትሪ ፍተሻዎች

| የጊዜ ማህተም (UTC) | ዳሽቦርዶች / መመርመሪያዎች | ባለቤት | ውጤት | Artefact |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`፣ `TryItProxyErrors`፣ `DocsPortal/GatewayRefusals` | ሰነዶች/DevRel + Ops | ✅ ሁሉም አረንጓዴ | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` ግልባጭ | ኦፕስ | ✅ መድረክ | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | `docs.preview.integrity`፣ `TryItProxyErrors`፣ `DocsPortal/GatewayRefusals`፣ `probe:portal` | ሰነዶች/DevRel + Ops | ✅ ቅጽበታዊ ገጽ እይታን ቀድመው ይጋብዙ፣ ምንም ለውጥ የለም | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | ከላይ ያሉት ዳሽቦርዶች + ይሞክሩት የተኪ መዘግየት ልዩነት | ሰነዶች/DevRel አመራር | ✅ የመሃል ነጥብ ቼክ አልፏል (0 ማንቂያዎች፤ ይሞክሩት መዘግየት p95=410ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | ዳሽቦርዶች ከላይ + መውጫ መፈተሻ | ሰነዶች/DevRel + የአስተዳደር ግንኙነት | ✅ ከቅጽበተ-ፎቶ ውጣ፣ ዜሮ አስደናቂ ማንቂያዎች | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

ዕለታዊ የቢሮ-ሰዓት ናሙናዎች (2025-04-13 → 2025-04-25) እንደ NDJSON + PNG ወደ ውጭ እንደሚላኩ ተጠቃለዋል።
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` ከፋይል ስሞች ጋር
`docs-preview-integrity-<date>.json` እና ተዛማጅ ቅጽበታዊ ገጽ እይታዎች።

## ግብረ መልስ እና ጉዳይ ምዝግብ ማስታወሻ

በግምገማ የቀረቡ ግኝቶችን ለማጠቃለል ይህን ሰንጠረዥ ይጠቀሙ። እያንዳንዱን ግቤት ከ GitHub/ውይይት ጋር ያገናኙ
ቲኬት እና የተዋቀረ ቅጽ በ በኩል ተያዘ
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)።

| ዋቢ | ከባድነት | ባለቤት | ሁኔታ | ማስታወሻ |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | ዝቅተኛ | ሰነዶች-ኮር-02 | ✅ የተፈታ 2025-04-18 | ተብራርቷል የቃላት አወጣጥ + የጎን አሞሌ መልህቅን ይሞክሩ (`docs/source/sorafs/tryit.md` በአዲስ መለያ የዘመነ)። |
| `docs-preview/w1 #2` | ዝቅተኛ | ሰነዶች-ኮር-03 | ✅ የተፈታ 2025-04-19 | ታድሷል ቅጽበታዊ ገጽ እይታን ይሞክሩት + መግለጫ ፅሁፍ በአንድ ገምጋሚ ​​ጥያቄ; artefact `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | መረጃ | ሰነዶች/DevRel አመራር | 🟢 ተዘግቷል | ቀሪ አስተያየቶች ጥያቄ እና መልስ ብቻ ነበሩ; በእያንዳንዱ አጋር የግብረመልስ ቅጽ በ `artifacts/docs_preview/W1/preview-2025-04-12/feedback/` ስር ተይዟል። |

## የእውቀት ፍተሻ እና የዳሰሳ ጥናት ክትትል

1. ለእያንዳንዱ ገምጋሚ የፈተና ጥያቄዎችን ይመዝግቡ (ዒላማ ≥90%); ወደ ውጭ የተላከውን ሲኤስቪ ከ ጋር ያያይዙ
   ቅርሶችን ይጋብዙ።
2. በግብረመልስ ቅጽ አብነት የተያዙትን የጥራት ዳሰሳ መልሶች ይሰብስቡ እና ያንጸባርቁዋቸው
   በ `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. ከመነሻው በታች ነጥብ ለሚያመጣ ማንኛውም ሰው መርሐግብር ያስይዙ እና በዚህ ፋይል ውስጥ ያስገቡት።

በእውቀት ቼክ ላይ ሁሉም ስምንቱ ገምጋሚዎች ≥94% አስመዝግበዋል (CSV፡
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`)። ምንም የማስተካከያ ጥሪዎች የሉም
ተፈላጊ ነበሩ; የዳሰሳ ጥናት ለእያንዳንዱ አጋር በቀጥታ ወደ ውጭ መላክ
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## የአርቴፍክት ክምችት

- ገላጭ/የቼክ ጥቅል ቅድመ እይታ፡ `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- መመርመሪያ + አገናኝ-ቼክ ማጠቃለያ: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- የተኪ ለውጥ መዝገብ ይሞክሩት፡ `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- ቴሌሜትሪ ወደ ውጭ መላክ: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- ዕለታዊ የቢሮ-ሰዓት ቴሌሜትሪ ጥቅል: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- ግብረ መልስ + የዳሰሳ ጥናት ወደ ውጭ መላክ፡ ገምጋሚ-ተኮር አቃፊዎችን ከስር አስቀምጣቸው
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- የእውቀት ማረጋገጫ CSV እና ማጠቃለያ: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

ዕቃውን ከመከታተያ ችግር ጋር በማመሳሰል ያቆዩት። ቅርሶችን ወደ የ
አስተዳደር ትኬት ስለዚህ ኦዲተሮች ሼል መዳረሻ ያለ ፋይሎቹን ማረጋገጥ ይችላሉ.