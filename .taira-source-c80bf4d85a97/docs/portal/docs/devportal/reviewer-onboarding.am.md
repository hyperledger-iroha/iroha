---
lang: am
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f42888a06cb49f9fe53f424ef77c84e2fa3a305f558e202be0fbbd4b3b0ea1d7
source_last_modified: "2025-12-29T18:16:35.114535+00:00"
translation_last_reviewed: 2026-02-07
id: reviewer-onboarding
title: Preview reviewer onboarding
sidebar_label: Reviewer onboarding
description: Process and checklists for enrolling reviewers in the docs portal public preview.
translator: machine-google-reviewed
---

## አጠቃላይ እይታ

DOCS-I18NT0000005X የገንቢ ፖርታልን በዝግጅቱ ይከታተላል። Checksum-gated ግንባታዎች
(`npm run serve`) እና ጠንከር ያለ ይሞክሩት ይፈሳል የሚቀጥለውን ምዕራፍ ያንሱ፡
ይፋዊ ቅድመ እይታው በሰፊው ከመከፈቱ በፊት የተረጋገጡ ገምጋሚዎችን ማሳፈር። ይህ መመሪያ
ጥያቄዎችን እንዴት እንደሚሰበስብ፣ ብቁነትን ማረጋገጥ፣ የአቅርቦት ተደራሽነት እና
ከቦርድ ውጪ ያሉ ተሳታፊዎች በሰላም። የሚለውን ተመልከት
[የግብዣ ፍሰት ቅድመ እይታ](./preview-invite-flow.md) ለቡድን እቅድ፣ ግብዣ
cadaence, እና ቴሌሜትሪ ወደ ውጭ መላክ; ከታች ያሉት እርምጃዎች በሚወሰዱት እርምጃዎች ላይ ያተኩራሉ
አንዴ ገምጋሚ ከተመረጠ።

- ** ወሰን፡** የሰነዶቹ ቅድመ እይታ መዳረሻ የሚያስፈልጋቸው ገምጋሚዎች (`docs-preview.sora`፣
  GitHub ገጾች ይገነባል ወይም SoraFS ቅርቅቦች) ከጂኤ በፊት።
- ** ከወሰን ውጪ:** Torii ወይም SoraFS ኦፕሬተሮች (በራሳቸው ተሳፍሪ ተሸፍኗል)
  ኪት) እና የምርት ፖርታል ማሰማራት (ተመልከት
  [`devportal/deploy-guide`](./deploy-guide.md))።

## ሚናዎች እና ቅድመ ሁኔታዎች

| ሚና | የተለመዱ ግቦች | አስፈላጊ ቅርሶች | ማስታወሻ |
| --- | --- | --- | --- |
| ዋና ጠባቂ | አዲስ መመሪያዎችን ያረጋግጡ፣ የጭስ ሙከራዎችን ያካሂዱ። | GitHub እጀታ፣ የማትሪክስ እውቂያ፣ በፋይል ላይ CLA የተፈረመ። | ብዙውን ጊዜ ቀድሞውኑ በ I18NI0000026X GitHub ቡድን ውስጥ; አሁንም ጥያቄ ያቅርቡ ስለዚህ መዳረሻ ኦዲት ይደረጋል። |
| አጋር ገምጋሚ ​​| ይፋዊ ከመልቀቁ በፊት የኤስዲኬ ቅንጥቦችን ወይም የአስተዳደር ይዘትን ያረጋግጡ። | የድርጅት ኢሜይል፣ ህጋዊ POC፣ የተፈረመ የቅድመ እይታ ውሎች። | የቴሌሜትሪ + የውሂብ አያያዝ መስፈርቶችን መቀበል አለበት። |
| የማህበረሰብ በጎ ፈቃደኛ | በመመሪያዎች ላይ የአጠቃቀም ግብረመልስ ይስጡ። | GitHub እጀታ፣ ተመራጭ እውቂያ፣ የሰዓት ሰቅ፣ የ CoC መቀበል። | ቡድኖቹን ትንሽ ያቆዩ; የአስተዋጽዖ ስምምነቱን የፈረሙ ገምጋሚዎች ቅድሚያ ይስጡ። |

ሁሉም የገምጋሚ ዓይነቶች የሚከተሉትን ማድረግ አለባቸው:

1. ለቅድመ እይታ ቅርሶች ተቀባይነት ያለው የአጠቃቀም ፖሊሲን እውቅና ይስጡ።
2. የደህንነት / ታዛቢነት ተጨማሪዎችን ያንብቡ
   ([`security-hardening`](./security-hardening.md)፣
   [`observability`](./observability.md)፣
   [`incident-runbooks`](./incident-runbooks.md))።
3. ማንኛውንም ከማገልገልዎ በፊት I18NI0000030X ለማሄድ ይስማሙ
   ቅጽበተ-ፎቶ በአካባቢው.

## የመግቢያ የስራ ሂደት

1. ጠያቂውን እንዲሞላው ይጠይቁ
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   ቅጽ (ወይም ወደ አንድ ጉዳይ ገልብጠው/ለጥፈው)። ቢያንስ ይያዙ፡ ማንነት፣ እውቂያ
   ዘዴ፣ GitHub እጀታ፣ የታሰበ የግምገማ ቀናት እና ማረጋገጫ የ
   የደህንነት ሰነዶች ተነበዋል.
2. ጥያቄውን በ `docs-preview` መከታተያ ውስጥ ይመዝግቡ (የጊትህብ ጉዳይ ወይም አስተዳደር
   ቲኬት) እና አጽዳቂን ይመድቡ.
3. ቅድመ ሁኔታዎችን ያረጋግጡ፡-
   - በፋይል ላይ የ CLA / የአስተዋጽኦ ስምምነት (ወይም የአጋር ውል ማጣቀሻ)።
   - ተቀባይነት ያለው-የአጠቃቀም እውቅና በጥያቄው ውስጥ ተከማችቷል።
   - የአደጋ ግምገማ ተጠናቅቋል (ለምሳሌ፣ በህጋዊ የጸደቁ የአጋር ገምጋሚዎች)።
4. አጽዳቂው በጥያቄው ላይ ተፈርሞ የመከታተያ ጉዳዩን ከማንኛውም ጋር ያገናኛል።
   የለውጥ አስተዳደር ግቤት (ለምሳሌ `DOCS-SORA-Preview-####`)።

## አቅርቦት እና መገልገያ

1. ** ቅርሶችን ያካፍሉ *** - የቅርብ ጊዜውን የቅድመ እይታ ገላጭ + መዝገብ ያቅርቡ
   የ CI የስራ ፍሰት ወይም I18NT0000002X ፒን (`docs-portal-preview` artefact)። አስታውስ
   የሚሄዱ ገምጋሚዎች፡-

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **ከቼክሰም ማስፈጸሚያ ጋር አገልግሉ** — ገምጋሚዎችን በቼክሱም-ጌት ያመልክቱ
   ትዕዛዝ፡-

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   ይህ `scripts/serve-verified-preview.mjs` እንደገና ይጠቀማል ስለዚህ ያልተረጋገጠ ግንባታ ሊኖር አይችልም።
   በድንገት ተጀመረ።

3. **የGitHub መዳረሻን ይስጡ (አማራጭ)** — ገምጋሚዎች ያልታተሙ ቅርንጫፎች ከፈለጉ፣
   ለግምገማው ጊዜ ወደ `docs-preview` GitHub ቡድን ያክሏቸው እና
   በጥያቄው ውስጥ የአባልነት ለውጥ ይመዝግቡ።

4. ** የድጋፍ ቻናሎችን ይገናኙ *** - በጥሪ ላይ ያለውን ግንኙነት ያጋሩ (ማትሪክስ/ስላክ)
   እና የክስተቱ ሂደት ከ [`incident-runbooks`](./incident-runbooks.md)።

5. **ቴሌሜትሪ + ግብረ መልስ** — ስም-አልባ ትንታኔዎች መሆናቸውን ገምጋሚዎችን አስታውስ
  ተሰብስቧል ([`observability` ይመልከቱ](./observability.md)። አስተያየቱን ይስጡ
  በግብዣው ውስጥ የተጠቀሰውን ቅጽ ወይም አብነት ያወጣል እና ክስተቱን በ
  [`preview-feedback-log`](./preview-feedback-log) ረዳት ስለዚህ የሞገድ ማጠቃለያ
  ወቅታዊ ሆኖ ይቆያል.

## የገምጋሚ ማረጋገጫ ዝርዝር

ቅድመ እይታውን ከመድረስዎ በፊት ገምጋሚዎች የሚከተሉትን ማጠናቀቅ አለባቸው፡

1. የወረዱትን ቅርሶች (`preview_verify.sh`) ያረጋግጡ።
2. ፖርታሉን በI18NI0000041X (ወይም `serve:verified`) በኩል ያስጀምሩት
   የቼክሱም ጠባቂ ንቁ ነው።
3. ከላይ የተገናኙትን የደህንነት እና ታዛቢነት ማስታወሻዎችን ያንብቡ።
4. OAuth/የመሳሪያ ኮድ መግቢያን በመጠቀም (የሚመለከተው ከሆነ) ኮንሶሉን ይሞክሩት።
   የምርት ምልክቶችን እንደገና ከመጠቀም ይቆጠቡ.
5. ግኝቶችን በተስማማው መከታተያ (ጉዳይ፣ የተጋራ ሰነድ ወይም ቅጽ) ያስገቡ እና መለያ ያድርጉ
   ከቅድመ እይታ ልቀት መለያ ጋር።

## ተሳፋሪዎች እና ተሳፋሪዎችን ማስጠበቅ

| ደረጃ | ድርጊቶች |
| --- | --- |
| Kickoff | የመግቢያ ማረጋገጫ ዝርዝር ከጥያቄው ጋር መያያዙን ያረጋግጡ፣ ቅርሶችን + መመሪያዎችን ያካፍሉ፣ የ`invite-sent` ግቤት በ[`preview-feedback-log`](./preview-feedback-log) ያክሉ እና ግምገማው ከአንድ ሳምንት በላይ የሚቆይ ከሆነ የመሃል ነጥብ ማመሳሰልን ያስይዙ። |
| ክትትል | የቅድመ እይታ ቴሌሜትሪ ይከታተሉ (ያልተለመደ ይሞክሩት ትራፊክ ይሞክሩ፣ አለመሳካቶችን ይፈትሹ) እና አጠራጣሪ ነገር ከተፈጠረ የክስተቱን Runbook ይከተሉ። ግኝቶች ሲደርሱ የ `feedback-submitted`/I18NI0000046X ክስተቶችን ይመዝገቡ ስለዚህ የሞገድ መለኪያዎች ትክክለኛ ሆነው ይቆያሉ። |
| ከመሳፈር ውጪ | ጊዜያዊ የ GitHub ወይም SoraFS መዳረሻን ይሰርዙ፣ `access-revoked` ይቅረጹ፣ ጥያቄውን በማህደር ያስቀምጡ (የአስተያየት ማጠቃለያ + አስደናቂ ተግባራትን ያካትቱ) እና የገምጋሚውን መዝገብ ያዘምኑ። ገምጋሚው የሀገር ውስጥ ግንባታዎችን እንዲያጸዳ እና ከ[`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) የተፈጠረውን መፍጨት እንዲያያይዝ ይጠይቁ። |

ገምጋሚዎችን በማዕበል መካከል በሚዞሩበት ጊዜ ተመሳሳይ ሂደት ይጠቀሙ። በማስቀመጥ ላይ
የወረቀት ዱካ በሪፖ (እትም + አብነቶች) DOCS-SORA ኦዲት ተደርጎ እንዲቆይ እና
የአስተዳደር ቅድመ እይታ መዳረሻ በሰነድ የተቀመጡትን መቆጣጠሪያዎች የተከተለ መሆኑን ያረጋግጣል።

## አብነቶችን ይጋብዙ እና መከታተል

- እያንዳንዱን ግንኙነት በ
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  ፋይል. አነስተኛውን የህግ ቋንቋ ይይዛል፣የቼክሰም መመሪያዎችን ቅድመ እይታ፣
  እና ገምጋሚዎች ተቀባይነት ያለውን የአጠቃቀም ፖሊሲን እንደሚገነዘቡ ይጠበቃል።
- አብነቱን በሚያርትዑበት ጊዜ ለ `<preview_tag>` ቦታ ያዥዎችን ይተኩ ፣
  `<request_ticket>`, እና የእውቂያ ጣቢያዎች. የመጨረሻውን መልእክት ቅጂ አስቀምጥ
  ገምጋሚዎች፣ አጽዳቂዎች እና ኦዲተሮች የመግቢያ ትኬቱን ማጣቀስ ይችላሉ።
  የተላከው ትክክለኛ ቃል.
- ግብዣውን ከላኩ በኋላ የመከታተያ የተመን ሉህ ያዘምኑ ወይም ችግሩን ያቅርቡ
  የ `invite_sent_at` የጊዜ ማህተም እና የሚጠበቀው የመጨረሻ ቀን ስለዚህ የ
  [የግብዣ ፍሰትን ቅድመ እይታ](./preview-invite-flow.md) ሪፖርት ቡድኑን መውሰድ ይችላል
  በራስ-ሰር.