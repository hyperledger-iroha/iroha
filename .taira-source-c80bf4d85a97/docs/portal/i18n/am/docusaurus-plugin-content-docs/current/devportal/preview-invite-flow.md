---
id: preview-invite-flow
lang: am
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview invite flow
sidebar_label: Preview invite flow
description: Sequencing, evidence, and communications plan for the docs portal public preview waves.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#ዓላማ

የመንገድ ካርታ ንጥል **DOCS-SORA** በመሳፈር ላይ ገምጋሚ እና የህዝብ ቅድመ እይታን ይጠራል
ፖርታሉ ከቅድመ-ይሁንታ ከመውጣትዎ በፊት ፕሮግራሙን እንደ የመጨረሻ አጋቾች ይጋብዙ። ይህ ገጽ
እያንዳንዱን የግብዣ ሞገድ እንዴት መክፈት እንደሚቻል ይገልጻል፣ የትኞቹ ቅርሶች ከዚህ በፊት መላክ አለባቸው
ግብዣዎች ይወጣሉ፣ እና ፍሰቱ ኦዲት የሚቻል መሆኑን እንዴት ማረጋገጥ እንደሚቻል። ከዚህ ጎን ለጎን ይጠቀሙበት፡-

- [`devportal/reviewer-onboarding`](I18NU0000006X) ለ
  በየ ገምጋሚ አያያዝ።
- [`devportal/preview-integrity-plan`](I18NU0000007X) ለቼክ
  ዋስትናዎች.
- [`devportal/observability`](./observability.md) ለቴሌሜትሪ ኤክስፖርት እና
  ማንቂያ መንጠቆዎች.

## የሞገድ እቅድ

| ማዕበል | ታዳሚዎች | የመግቢያ መስፈርት | መውጫ መስፈርት | ማስታወሻ |
| --- | --- | --- | --- | --- |
| ** W0 - ኮር ጠባቂዎች *** | ቀን-አንድ ይዘትን የሚያረጋግጡ ሰነዶች/ኤስዲኬ ጠባቂዎች። | `docs-portal-preview` GitHub ቡድን ተሞልቷል፣ `npm run serve` የፍተሻ በር አረንጓዴ፣ ማስጠንቀቂያ አስተዳዳሪ ለ7 ቀናት ጸጥ ብሏል። | ሁሉም P0 ሰነዶች ተገምግመዋል፣ የኋላ መዝገብ ታግ ተሰጥቷል፣ ምንም የሚያግድ ክስተቶች የሉም። | ፍሰቱን ለማረጋገጥ ጥቅም ላይ ይውላል; የግብዣ ኢሜይል የለም፣ ልክ የቅድመ እይታ ቅርሶችን ያጋሩ። |
| ** W1 - አጋሮች *** | SoraFS ኦፕሬተሮች፣ I18NT0000003X ኢንተግራተሮች፣ የአስተዳደር ገምጋሚዎች በኤንዲኤ ስር። | W0 ወጥቷል፣ ህጋዊ ውሎች ጸድቀዋል፣ ይሞክሩት ፕሮክሲ። | የተሰበሰበ የአጋር ማቋረጥ (ችግር ወይም የተፈረመ ቅጽ)፣ ቴሌሜትሪ ≤10 ተመሳሳይ ገምጋሚዎችን ያሳያል፣ ለ14  ቀናት ምንም የደህንነት ለውጦች የሉም። | የግብዣ አብነት + የጥያቄ ትኬቶችን ያስፈጽሙ። |
| ** W2 - ማህበረሰብ *** | ከማኅበረሰቡ ተጠባባቂ ዝርዝር ውስጥ አስተዋጽዖ አበርካቾች ተመርጠዋል። | W1 ወጥቷል፣ የክስተቶች ልምምዶች ተለማመዱ፣ የህዝብ ጥያቄዎች ተዘምነዋል። | ግብረመልስ ተፈጭቷል፣ ≥2 ሰነዶች የሚለቀቁት በቅድመ-እይታ ቧንቧ መስመር ያለ መልሶ ማገገሚያ ነው። | ካፕ በተመሳሳይ ግብዣዎች (≤25) እና ባች ሳምንታዊ። |

የትኛው ሞገድ በ`status.md` ውስጥ እና በቅድመ እይታ ጥያቄ ውስጥ እንደሚሰራ ሰነድ
መከታተያ ስለዚህ አስተዳደር ፕሮግራሙ በጨረፍታ የት እንደሚቀመጥ ማየት ይችላል።

## የቅድመ በረራ ማረጋገጫ ዝርዝር

የማዕበል ግብዣዎችን ከማዘጋጀትዎ በፊት እነዚህን ድርጊቶች ያጠናቅቁ።

1. **CI artefacts ይገኛሉ**
   - የቅርብ ጊዜ `docs-portal-preview` + ገላጭ በ የተሰቀለ
     `.github/workflows/docs-portal-preview.yml`.
   - SoraFS ፒን በI18NI0000019X ውስጥ ተጠቅሷል
     (አቋራጭ ገላጭ አለ)።
2. **የቼክተም ማስፈጸሚያ**
   - `docs/portal/scripts/serve-verified-preview.mjs` ተጠርቷል።
     `npm run serve`.
   - `scripts/preview_verify.sh` መመሪያዎች በ macOS + Linux ላይ ተፈትነዋል።
3. **የቴሌሜትሪ መነሻ መስመር**
   - `dashboards/grafana/docs_portal.json` ጤናማ ያሳያል ትራፊክ ይሞክሩት እና
     `docs.preview.integrity` ማንቂያ አረንጓዴ ነው።
   - የቅርብ ጊዜ `docs/portal/docs/devportal/observability.md` አባሪ ዘምኗል
     Grafana አገናኞች።
4. **የመንግስት ቅርሶች**
   - የመከታተያ ችግር ዝግጁ ነው (በአንድ ሞገድ አንድ እትም)።
   - የገምጋሚ መዝገብ ቤት አብነት ተቀድቷል (ተመልከት
     [`docs/examples/docs_preview_request_template.md`](I18NU0000009X))።
   - ከጉዳዩ ጋር ተያይዘው ህጋዊ እና SRE የሚፈለጉ ማፅደቆች።

ማንኛውንም ደብዳቤ ከመላክዎ በፊት የቅድመ በረራ ማጠናቀቅን በግብዣ መከታተያ ውስጥ ይመዝግቡ።

## የወራጅ ደረጃዎች

1. **እጩዎችን ምረጥ**
   - ከተጠባባቂው ዝርዝር የተመን ሉህ ወይም የአጋር ወረፋ ይጎትቱ።
   - እያንዳንዱ እጩ የተሟላ የጥያቄ አብነት እንዳለው ያረጋግጡ።
2. **መዳረሻን አጽድቅ**
   - ለግብዣ መከታተያ ጉዳይ አጽዳቂን መድብ።
   - ቅድመ ሁኔታዎችን ያረጋግጡ (CLA / ውል ፣ ተቀባይነት ያለው አጠቃቀም ፣ የደህንነት አጭር መግለጫ)።
3. ** ግብዣዎችን ላክ ***
   - መሙላት
     [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
     ቦታ ያዢዎች (`<preview_tag>`፣ I18NI0000029X፣ እውቂያዎች)።
   - ገላጭውን + ማህደር ሃሽን ያያይዙ ፣ ዩአርኤል ለማዘጋጀት ይሞክሩ እና ይደግፉ
     ቻናሎች.
   - የመጨረሻውን ኢሜል (ወይም ማትሪክስ/ስላክ ግልባጭ) በችግሩ ውስጥ ያከማቹ።
4. ** በመሳፈር ላይ ይከታተሉ ***
   - የግብዣ መከታተያውን በ`invite_sent_at`፣ `expected_exit_at` ያዘምኑ እና
     ሁኔታ (`pending`፣ `active`፣ `complete`፣ `revoked`)።
   - ለኦዲትነት የገምጋሚው ቅበላ ጥያቄ አገናኝ።
5. ** ቴሌሜትሪ ተቆጣጠር ***
   - `docs.preview.session_active` እና I18NI0000037X ማንቂያዎችን ይመልከቱ።
   - ቴሌሜትሪ ከመሠረታዊ መስመሩ ከተለያየ እና መዝግበው ከሆነ ክስተት ያስገቡ
     ከግብዣው መግቢያ ቀጥሎ ያለው ውጤት።
6. ** ግብረ መልስ ሰብስብ እና ውጣ ***
   - የግብረ መልስ መሬት አንዴ ወይም `expected_exit_at` ካለፈ ግብዣዎችን ዝጋ።
   - የማዕበሉን ጉዳይ በአጭር ማጠቃለያ ያዘምኑ (ግኝቶች፣ ክስተቶች፣ ቀጣይ
     ድርጊቶች) ወደ ቀጣዩ ቡድን ከመዛወራቸው በፊት.

## ማስረጃ እና ዘገባ

| Artefact | የት ማከማቸት | አድስ ካዴንስ |
| --- | --- | --- |
| የመከታተያ መጋበዝ ችግር | `docs-portal-preview` GitHub ፕሮጀክት | ከእያንዳንዱ ግብዣ በኋላ ያዘምኑ። |
| የገምጋሚ ዝርዝር ወደ ውጪ መላክ | `docs/portal/docs/devportal/reviewer-onboarding.md` የተገናኘ መዝገብ | በየሳምንቱ። |
| ቴሌሜትሪ ቅጽበተ-ፎቶዎች | `docs/source/sdk/android/readiness/dashboards/<date>/` (የቴሌሜትሪ ቅርቅብ እንደገና መጠቀም) | በእያንዳንዱ ሞገድ + ከአደጋዎች በኋላ። |
| የግብረመልስ መፍጨት | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (አቃፊ በአንድ ማዕበል ፍጠር) | ማዕበል በወጣ በ5 ቀናት ውስጥ። |
| የአስተዳደር ስብሰባ ማስታወሻ | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | ከእያንዳንዱ የ DOCS-SORA የአስተዳደር ማመሳሰል በፊት ብዙ ህዝብ። |

`cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json` አሂድ
ከእያንዳንዱ ቡድን በኋላ በማሽን ሊነበብ የሚችል ክስተት መፍጨት። የተሰራውን ያያይዙ
የአስተዳደር ገምጋሚዎች ያለሱ የግብዣ ብዛት ማረጋገጥ እንዲችሉ JSON ወደ ሞገድ ጉዳይ
መላውን መዝገብ እንደገና በመጫወት ላይ።

ማዕበል ባለቀ ቁጥር የማስረጃ ዝርዝሩን ከI18NI0000045X ጋር አያይዘው ስለዚህ የመንገድ ካርታው
መግቢያ በፍጥነት ሊዘመን ይችላል።

## ወደ ኋላ መመለስ እና ለአፍታ ማቆም መስፈርቶች

ከሚከተሉት ውስጥ አንዱ ሲከሰት የግብዣውን ፍሰት ባለበት ያቁሙ (እና አስተዳደርን ያሳውቁ)

- መልሶ መመለስ የሚያስፈልገው የተኪ ክስተት ይሞክሩት (`npm run manage:tryit-proxy`)።
- የማስጠንቀቂያ ድካም፡ > 3 የማንቂያ ገጾች በ7 ቀናት ውስጥ ለቅድመ እይታ-ብቻ የመጨረሻ ነጥቦች።
- የተገዢነት ክፍተት፡- ያለ ፊርማ ውሎች ወይም ሳይመዘገቡ ይጋብዙ
  አብነት ይጠይቁ.
- የታማኝነት ስጋት፡ የቼክሰም አለመዛመድ በ`scripts/preview_verify.sh` ተገኝቷል።

በግብዣ መከታተያ ውስጥ ያለውን ማሻሻያ ከሰነዱ በኋላ ብቻ ከቆመበት ይቀጥሉ
የቴሌሜትሪ ዳሽቦርድ ቢያንስ ለ48 ሰዓታት የተረጋጋ መሆኑን ማረጋገጥ።