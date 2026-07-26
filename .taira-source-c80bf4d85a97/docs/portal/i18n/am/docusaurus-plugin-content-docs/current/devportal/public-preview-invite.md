---
id: public-preview-invite
lang: am
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Public preview invite playbook
sidebar_label: Preview invite playbook
description: Checklist for announcing the docs portal preview to external reviewers.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## የፕሮግራም ግቦች

ይህ የመጫወቻ ደብተር አንዴ ይፋዊ እይታን እንዴት ማስታወቅ እና ማስኬድ እንደሚቻል ያብራራል።
ገምጋሚ የቦርዲንግ የስራ ፍሰት ቀጥታ ነው። የ DOCS-SORA የመንገድ ካርታ በታማኝነት ይጠብቃል።
እያንዳንዱ የግብዣ መርከቦችን ሊረጋገጡ በሚችሉ ቅርሶች፣ የደህንነት መመሪያ እና ሀ
ግልጽ የግብረመልስ መንገድ.

- **ታዳሚዎች፡** የተመረጡ የማህበረሰብ አባላት፣ አጋሮች እና ጠባቂዎች ዝርዝር
  ተቀባይነት ያለው የአጠቃቀም መመሪያውን በቅድመ-እይታ ላይ ፈርመዋል።
- ** ጣሪያዎች:** ነባሪ የሞገድ መጠን ≤ 25 ገምጋሚዎች፣ የ14-ቀን መዳረሻ መስኮት፣ ክስተት
  በ 24h ውስጥ ምላሽ.

## የጌቲንግ ዝርዝርን አስጀምር

ማንኛውንም ግብዣ ከመላክዎ በፊት እነዚህን ተግባራት ያጠናቅቁ፡

1. በCI ውስጥ የተሰቀሉ የቅርብ ጊዜ ቅድመ እይታ ቅርሶች (`docs-portal-preview`፣
   የቼክሰም መግለጫ፣ ገላጭ፣ SoraFS ጥቅል)።
2. `npm run --prefix docs/portal serve` (checksum-gated) በተመሳሳይ መለያ ላይ ተፈትኗል።
3. ገምጋሚ ​​የመሳፈሪያ ትኬቶች ጸድቀው ከግብዣ ሞገድ ጋር ተገናኝተዋል።
4. ደህንነት፣ ታዛቢነት እና የአደጋ ሰነዶች ተረጋግጠዋል
   ([`security-hardening`](I18NU0000009X)፣
   [`observability`](./observability.md)፣
   [`incident-runbooks`](./incident-runbooks.md))።
5. የግብረመልስ ቅጽ ወይም የተዘጋጀ አብነት (ለከባድነት መስኮችን ያካትቱ)
   የማባዛት ደረጃዎች፣ ቅጽበታዊ ገጽ እይታዎች እና የአካባቢ መረጃ)።
6. የማስታወቂያ ቅጂ በDocs/DevRel + Governance የተገመገመ።

## የጥቅል ግብዣ

እያንዳንዱ ግብዣ የሚከተሉትን ማካተት አለበት:

1. **የተረጋገጡ ቅርሶች** — የSoraFS አንጸባራቂ/ዕቅድ ወይም GitHub artefact ያቅርቡ
   አገናኞች እና የቼክሰም መግለጫ እና ገላጭ። ማረጋገጫውን ያጣቅሱ
   ገምጋሚዎች ጣቢያውን ከመክፈትዎ በፊት እንዲያሄዱት በግልፅ ማዘዝ።
2. **መመሪያዎችን ያቅርቡ *** - በቼክሰም-የተዘጋ ቅድመ እይታ ትዕዛዝ ያካትቱ፡

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **የደህንነት አስታዋሾች** - ቶከኖች በራስ-ሰር እንደሚያልቁ ይደውሉ
   መጋራት የለበትም፣ እና ክስተቶች ወዲያውኑ ሪፖርት መደረግ አለባቸው።
4. ** የግብረመልስ ሰርጥ *** - ከጉዳዩ አብነት/ቅፅ ጋር ያገናኙ እና ምላሹን ያብራሩ
   ጊዜ የሚጠበቁ.
5. **የፕሮግራም ቀኖች** — የመጀመሪያ/የማጠናቀቂያ ቀናትን፣የስራ ሰአቶችን ወይም የማመሳሰል ስብሰባዎችን ያቅርቡ፣
   እና የሚቀጥለው የማደስ መስኮት.

የናሙና ኢሜይል ወደ ውስጥ
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
እነዚህን መስፈርቶች ይሸፍናል. ቦታ ያዢዎቹን ያዘምኑ (ቀኖች፣ ዩአርኤሎች፣ አድራሻዎች)
ከመላኩ በፊት.

## የቅድመ እይታ አስተናጋጁን ያጋልጡ

የመሳፈሪያው ሂደት እንደተጠናቀቀ እና ትኬቱን ለመቀየር የቅድመ እይታ አስተናጋጁን ብቻ ያስተዋውቁ
ይፀድቃል። [የቅድመ እይታ የአስተናጋጅ መጋለጥ መመሪያን ይመልከቱ](./preview-host-exposure.md)
በዚህ ክፍል ውስጥ ጥቅም ላይ የዋለውን ከጫፍ እስከ ጫፍ ግንባታ/ማተም/ማረጋገጫ ደረጃዎች።

1. ** ይገንቡ እና ያሽጉ: *** የመልቀቂያ መለያውን ማህተም ያድርጉ እና ቆራጥነትን ያመርቱ
   ቅርሶች.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   የፒን ስክሪፕቱ `portal.car`፣ `portal.manifest.*`፣ `portal.pin.proposal.json`፣
   እና `portal.dns-cutover.json` በ I18NI0000028X ስር። እነዚያን ፋይሎች ከ
   እያንዳንዱ ገምጋሚ ተመሳሳዩን ቢት ማረጋገጥ እንዲችል የጋብዝ ሞገድ።

2. **የቅድመ እይታ ተለዋጭ ስም ያትሙ፡** ያለ `--skip-submit` ትዕዛዙን እንደገና ያሂዱ።
   (አቅርቦት `TORII_URL`፣ `AUTHORITY`፣ `PRIVATE_KEY[_FILE]`፣ እና
   በአስተዳደር የተሰጠ ተለዋጭ ማስረጃ)። ስክሪፕቱ አንጸባራቂውን ያስራል።
   `docs-preview.sora` እና `portal.manifest.submit.summary.json` ፕላስ ልቀት
   `portal.pin.report.json` ለማስረጃ ጥቅል።

3. **ስምረቱን መርምር፡** ተለዋጭ ስም ፍቺዎችን እና የቼክሱም ግጥሚያዎችን ያረጋግጡ።
   ግብዣዎችን ከመላክዎ በፊት መለያው ።

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   እንደ
   ገምጋሚዎች የቅድመ ዕይታ ጫፉ ቢነፋ የአካባቢያዊ ቅጂ ማሽከርከር እንዲችሉ ወደኋላ መመለስ።

## የግንኙነት ጊዜ

| ቀን | ድርጊት | ባለቤት |
| --- | --- | --- |
| D-3 | የግብዣ ቅጂን ያጠናቅቁ ፣ ቅርሶችን ያድሱ ፣ የደረቀ ማረጋገጫ | ሰነዶች/DevRel |
| D-2 | የአስተዳደር መቋረጥ + ትኬት ለውጥ | ሰነዶች/DevRel + አስተዳደር |
| D-1 | አብነት በመጠቀም ግብዣዎችን ይላኩ ፣ መከታተያ በተቀባይ ዝርዝር ያዘምኑ | ሰነዶች/DevRel |
| መ | የመክፈቻ ጥሪ/የቢሮ ሰዓት፣የቴሌሜትሪ ዳሽቦርዶችን ይቆጣጠሩ | ሰነዶች/DevRel + ጥሪ ላይ |
| D+7 | የመሃል ነጥብ ግብረ መልስ መፍጨት፣ የመለየት እገዳ ጉዳዮች | ሰነዶች/DevRel |
| D+14 | ማዕበልን ዝጋ፣ ጊዜያዊ መዳረሻን ሰርዝ፣ ማጠቃለያ በI18NI0000038X | ሰነዶች/DevRel |

## የመዳረሻ ክትትል እና ቴሌሜትሪ

1. እያንዳንዱን ተቀባይ ይመዝግቡ፣ የጊዜ ማህተም ይጋብዙ እና የተሻረ ቀን በ
   የግብረ መልስ ምዝግብ ማስታወሻን አስቀድመው ይመልከቱ (ይመልከቱ
   [`preview-feedback-log`](./preview-feedback-log)) ስለዚህ እያንዳንዱ ሞገድ ያጋራል
   ተመሳሳይ ማስረጃ;

   ```bash
   # Append a new invite event to artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   የሚደገፉ ዝግጅቶች I18NI0000040X፣ `acknowledged`፣
   `feedback-submitted`፣ `issue-opened`፣ እና `access-revoked`። ሎግ የሚኖረው በ
   `artifacts/docs_portal_preview/feedback_log.json` በነባሪ; ጋር አያይዘው
   የግብዣ ሞገድ ትኬት ከስምምነት ቅጾች ጋር። የማጠቃለያ ረዳትን ተጠቀም
   ከመዝጊያው ማስታወሻ በፊት ኦዲት ሊደረግ የሚችል ጥቅል ለማዘጋጀት፡-

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   የJSON ማጠቃለያ ግብዣዎችን በየማዕበል፣ ክፍት ተቀባዮችን፣ ግብረመልስን ይዘረዝራል።
   ይቆጥራል፣ እና የቅርቡ ክስተት የጊዜ ማህተም። ረዳቱ የተደገፈ ነው።
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)፣
   ስለዚህ ተመሳሳይ የስራ ሂደት በአካባቢው ወይም በ CI ውስጥ ሊሰራ ይችላል. ውስጥ የመፍጨት አብነት ተጠቀም
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   የማዕበል ዳግመኛ ሲታተም.
2. ለሞገድ የሚያገለግሉ የቴሌሜትሪ ዳሽቦርዶችን ከ `DOCS_RELEASE_TAG` ጋር መለያ ያድርጉ።
   ስፒሎች ከግብዣ ቡድኖች ጋር ሊጣመሩ ይችላሉ።
3. ወደ ማሰማራት በኋላ I18NI0000049X አሂድ
   የቅድመ እይታ አካባቢ ትክክለኛውን የልቀት ዲበ ውሂብ እንደሚያስተዋውቅ ያረጋግጡ።
4. በ runbook አብነት ውስጥ ያሉ ማንኛቸውም ክስተቶችን ያንሱ እና ከተሰበሰበው ቡድን ጋር ያገናኙዋቸው።

## ግብረ መልስ እና ቅርብ-ውጭ

1. አስተያየቶችን በጋራ ሰነድ ወይም እትም ቦርድ ውስጥ ያዋህዱ። ንጥሎቹን በ ሰይፉ
   የመንገድ ካርታ ባለቤቶች በቀላሉ መጠየቅ እንዲችሉ `docs-preview/<wave>`።
2. የሞገድ ሪፖርቱን ለመሙላት የቅድመ እይታ ሎገር ማጠቃለያ ውፅዓትን ተጠቀም፣ በመቀጠል
   በ `status.md` (ተሳታፊዎች ፣ ዋና ግኝቶች ፣ የታቀዱ) ስብስቦችን ማጠቃለል
   ጥገናዎች) እና የ DOCS-SORA ምእራፍ ከተለወጠ I18NI0000052X ያዘምኑ።
3. ከቦርዲንግ ውጪ ያሉትን ደረጃዎች ይከተሉ
   [`reviewer-onboarding`](./reviewer-onboarding.md)፡ መዳረሻን ሰርዝ፣ ማህደር
   ጥያቄዎች እና ተሳታፊዎችን አመሰግናለሁ።
4. የሚቀጥለውን ሞገድ በማደስ ቅርሶችን ማዘጋጀት፣ የቼክ በሮችን እንደገና በማካሄድ፣
   እና የግብዣውን አብነት በአዲስ ቀናት ማዘመን።

ይህንን የመጫወቻ ደብተር በቋሚነት መተግበር የቅድመ እይታ ፕሮግራሙን ኦዲት ያደርገዋል እና ያቆያል
ፖርታሉ ወደ GA ሲቃረብ ግብዣዎችን ለመለካት Docs/DevRel ተደጋጋሚ መንገድ ይሰጣል።