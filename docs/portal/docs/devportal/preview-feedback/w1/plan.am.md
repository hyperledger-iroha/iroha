---
lang: am
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c3eddff3a7b9b5dc4eac39251c9df72d0533a6e1b5865c716d54dc3b1c5de164
source_last_modified: "2025-12-29T18:16:35.108030+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-plan
title: W1 partner preflight plan
sidebar_label: W1 plan
description: Tasks, owners, and evidence checklist for the partner preview cohort.
translator: machine-google-reviewed
---

| ንጥል | ዝርዝሮች |
| --- | --- |
| ማዕበል | W1 - አጋሮች እና I18NT0000002X integrators |
| የዒላማ መስኮት | Q2 2025 ሳምንት 3 |
| Artefact መለያ (የታቀደ) | `preview-2025-04-12` |
| የመከታተያ ጉዳይ | `DOCS-SORA-Preview-W1` |

# አላማዎች

1. ለአጋር ቅድመ እይታ ውሎች ደህንነቱ የተጠበቀ የህግ + የአስተዳደር ማፅደቆች።
2. በግብዣ ጥቅል ውስጥ ጥቅም ላይ የዋለውን ፕሮክሲ እና ቴሌሜትሪ ቅጽበተ-ፎቶዎችን ይሞክሩ።
3. በቼክሱም የተረጋገጠ ቅድመ ዕይታ ቅርስ እና የምርመራ ውጤቶችን ያድሱ።
4. ግብዣዎች ከመላካቸው በፊት የአጋር ስም ዝርዝር + የጥያቄ አብነቶችን ያጠናቅቁ።

##የተግባር ትንተና

| መታወቂያ | ተግባር | ባለቤት | የሚከፈልበት | ሁኔታ | ማስታወሻ |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | ለቅድመ እይታ ውሎች ተጨማሪ ህጋዊ ፈቃድ ያግኙ ሰነዶች/DevRel አመራር → ህጋዊ | 2025-04-05 | ✅ ተጠናቀቀ | ሕጋዊ ቲኬት I18NI0000017X ተፈርሟል 2025-04-05; ፒዲኤፍ ከመከታተያው ጋር ተያይዟል። |
| W1-P2 | ያንሱት ፕሮክሲ ማዘጋጃ መስኮትን ይሞክሩት (2025-04-10) እና የተኪ ጤናን ያረጋግጡ | ሰነዶች/DevRel + Ops | 2025-04-06 | ✅ ተጠናቀቀ | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` ተፈጽሟል 2025-04-06; የCLI ግልባጭ + I18NI0000019X በማህደር ተቀምጧል። |
| W1-P3 | የቅድመ እይታ አርቴፋክትን (`preview-2025-04-12`)፣ `scripts/preview_verify.sh` + `npm run probe:portal`፣ የማህደር ገላጭ/Checksumsን ያሂዱ | ፖርታል TL | 2025-04-08 | ✅ ተጠናቀቀ | Artefact + የማረጋገጫ ምዝግብ ማስታወሻዎች በ `artifacts/docs_preview/W1/preview-2025-04-12/`; የመመርመሪያ ውፅዓት ከመከታተያ ጋር ተያይዟል። |
| W1-P4 | የአጋር ቅበላ ቅጾችን ይገምግሙ (I18NI0000024X)፣ እውቂያዎችን + NDAs ያረጋግጡ | የአስተዳደር ግንኙነት | 2025-04-07 | ✅ ተጠናቀቀ | ሁሉም ስምንቱ ጥያቄዎች ጸድቀዋል (የመጨረሻዎቹ ሁለት ጸድተዋል 2025-04-11); በክትትል ውስጥ የተገናኙ ማጽደቆች። |
| W1-P5 | ረቂቅ የግብዣ ቅጂ (በ`docs/examples/docs_preview_invite_template.md` ላይ የተመሰረተ)፣ ለእያንዳንዱ አጋር `<preview_tag>` እና `<request_ticket>` አዘጋጅ | ሰነዶች/DevRel አመራር | 2025-04-08 | ✅ ተጠናቀቀ | የግብዣ ረቂቅ ከ2025-04-12 15:00UTC ከሥነ ጥበብ ማገናኛዎች ጋር ተልኳል። |

## የቅድመ በረራ ማረጋገጫ ዝርዝር

> ጠቃሚ ምክር፡ ደረጃ 1-5ን በራስ ሰር ለመፈፀም `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` ን ያሂዱ (ግንባታ፣ ቼክሰም ማረጋገጥ፣ የፖርታል ፍተሻ፣ አገናኝ አራሚ እና ተኪ ማዘመኛ ይሞክሩት)። ስክሪፕቱ ከመከታተያ ችግር ጋር ማያያዝ የምትችለውን የJSON ሎግ ይመዘግባል።

1. `npm run build` (ከ I18NI0000030X ጋር) `build/checksums.sha256` እና `build/release.json` እንደገና ለማዳበር።
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` እና I18NI0000036X በማህደር ገላጭ አጠገብ።
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ወይም ተገቢውን ዒላማ በ `--tryit-target` ያቅርቡ); የዘመነውን `.env.tryit-proxy` አስገባ እና `.bak` ለመልሶ ማቆየት።
6. የW1 መከታተያ ችግርን በሎግ ዱካዎች ያዘምኑ (ገላጭ ቼክተም፣ የፍተሻ ውፅዓት፣ ተኪ ለውጥ ይሞክሩት፣ I18NT0000000X ቅጽበተ-ፎቶዎች)።

## የማስረጃ ዝርዝር

- [x] ከI18NI0000041X ጋር ተያይዟል (ፒዲኤፍ ወይም ቲኬት ማገናኛ) የተፈረመ ሕጋዊ ፈቃድ።
- [x] I18NT0000001X ቅጽበታዊ ገጽ እይታዎች ለI18NI0000042X፣ `TryItProxyErrors`፣ `DocsPortal/GatewayRefusals`።
- [x] `preview-2025-04-12` ገላጭ + የቼክ መዝገብ በ `artifacts/docs_preview/W1/` ስር ተቀምጧል።
- [x] በ`invite_sent_at` የጊዜ ማህተሞች የተሞሉ (የመከታተያ W1 ሎግ ይመልከቱ) የስም ዝርዝር ሠንጠረዥን ይጋብዙ።
- [x] የግብረመልስ ቅርሶች በ[`preview-feedback/w1/log.md`](./log.md) በአንድ ረድፍ በአንድ ባልደረባ (የዘመነ 2025-04-26 ከሮስተር/ቴሌሜትሪ/ጉዳይ መረጃ ጋር)።

ተግባራት እየገፉ ሲሄዱ ይህንን እቅድ ያዘምኑ; መከታተያው የመንገድ ካርታውን ለመጠበቅ ይጠቅሳል
ኦዲት ሊደረግ የሚችል።

## የግብረመልስ የስራ ሂደት

1. ለእያንዳንዱ ገምጋሚ አብነቱን በ ውስጥ ያባዙት።
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)፣
   ሜታዳታውን ይሙሉ እና የተጠናቀቀውን ቅጂ ከስር ያከማቹ
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. ግብዣዎችን ማጠቃለል፣ የቴሌሜትሪ ፍተሻ ነጥቦችን እና ጉዳዮችን በቀጥታ መዝገብ ውስጥ ይክፈቱ
   [`preview-feedback/w1/log.md`](./log.md) ስለዚህ የአስተዳደር ገምጋሚዎች ሙሉውን ሞገድ እንደገና ማጫወት ይችላሉ
   ከማጠራቀሚያው ሳይወጡ.
3. የእውቀት ቼክ ወይም የዳሰሳ ጥናት ወደ ውጭ በሚላኩበት ጊዜ፣ በምዝግብ ማስታወሻው ላይ በተጠቀሰው የጥበብ መንገድ አያይዟቸው
   እና የመከታተያ ጉዳይን ያቋርጡ።