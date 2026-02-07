---
id: preview-integrity-plan
lang: am
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Checksum-Gated Preview Plan
sidebar_label: Preview Integrity Plan
description: Implementation roadmap for securing the docs portal preview pipeline with checksum validation and notarised artefacts.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ይህ እቅድ እያንዳንዱን የፖርታል ቅድመ እይታ አርቲፊኬት ከመታተሙ በፊት የተረጋገጠ ለማድረግ የቀረውን ስራ ይዘረዝራል። ግቡ ገምጋሚዎች በCI ውስጥ የተሰራውን ትክክለኛ ቅጽበታዊ ገጽ እይታ እንዲያወርዱ፣ የቼክተም መግለጫው የማይለወጥ መሆኑን እና ቅድመ እይታው በSoraFS በI18NT0000001X ሜታዳታ እንደሚገኝ ማረጋገጥ ነው።

# አላማዎች

- ** ቆራጥ ግንባታዎች፡** `npm run build` ሊባዛ የሚችል ምርት ማፍራቱን እና ሁልጊዜ `build/checksums.sha256` እንደሚያመነጭ ያረጋግጡ።
- **የተረጋገጡ ቅድመ-እይታዎች፡** እያንዳንዱን ቅድመ-ዕይታ ቅርስ በቼክሰም ማኒፌክት ለመላክ እና ማረጋገጫው ሳይሳካ ሲቀር ህትመቶችን ውድቅ አድርግ።
- **Norito-የታተመ ሜታዳታ፡** ቅድመ-እይታ ገላጭዎችን (ሜታዳታ፣ ቼክሰም ዳይጀስት፣ I18NT000000008X CID) እንደ Norito JSON ስለዚህ የአስተዳደር መሳሪያዎች ልቀቶችን ኦዲት ማድረግ ይችላሉ።
- ** የኦፕሬተር መሳሪያ ስራ፡** ሸማቾች በአገር ውስጥ ሊሰሩ የሚችሉ የአንድ-ደረጃ ማረጋገጫ ስክሪፕት ያቅርቡ (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`)። ስክሪፕቱ አሁን የቼክሱን + ገላጭ ማረጋገጫ ፍሰት ከጫፍ እስከ ጫፍ ያጠቃልላል። መደበኛው የቅድመ እይታ ትዕዛዝ (`npm run serve`) አሁን ይህንን ረዳት ከ`docusaurus serve` በፊት ይጣራል ስለዚህ የአካባቢ ቅጽበታዊ ገጽ እይታዎች በቼክ ሱም-ጌት ሆነው ይቆያሉ (በ `npm run serve:verified` እንደ ግልፅ ተለዋጭ ስም ተቀምጧል)።

## ደረጃ 1 - CI ማስፈጸሚያ

1. `.github/workflows/docs-portal-preview.yml` ወደ፡
   - ከ I18NT0000000X ግንባታ በኋላ `node docs/portal/scripts/write-checksums.mjs` ን ያሂዱ (ቀድሞውኑ በአካባቢው የተጠራ)።
   - I18NI0000026X ን ያስፈጽም እና በተመጣጣኝ አለመመጣጠን ስራውን ወድቋል።
   - የግንባታ ማውጫውን እንደ `artifacts/preview-site.tar.gz` ያሸጉት፣ የቼክተም ማኒፌክተሩን ይቅዱ፣ `scripts/generate-preview-descriptor.mjs` ይደውሉ እና `scripts/sorafs-package-preview.sh`ን በJSON ውቅር ያስፈጽሙ (I18NI0000030X ይመልከቱ) ስለዚህ የስራ ፍሰቱ ሁለቱንም ሜታዳታ እና ቆራጥ የሆነ I18000000000
   - የማይንቀሳቀስ ቦታውን፣ የሜታዳታ ቅርሶችን (`docs-portal-preview`፣ `docs-portal-preview-metadata`) እና የSoraFS ጥቅል (`docs-portal-preview-sorafs`) ስቀል አንጸባራቂው፣ CAR ማጠቃለያ እና እቅዱ ግንባታውን እንደገና ሳይሰራ መፈተሽ ይችላል።
2. የቼክ ድምር የማረጋገጫ ውጤቱን በጉተታ ጥያቄዎች ውስጥ የሚያጠቃልል የCI ባጅ አስተያየት ያክሉ (✅ በ`docs-portal-preview.yml` GitHub Script አስተያየት ደረጃ የተተገበረ)።
3. በ `docs/portal/README.md` (CI ክፍል) ውስጥ ያለውን የስራ ሂደት ይመዝግቡ እና በህትመት ማረጋገጫ ዝርዝር ውስጥ ካለው የማረጋገጫ ደረጃዎች ጋር ያገናኙ.

## የማረጋገጫ ስክሪፕት።

`docs/portal/scripts/preview_verify.sh` የወረዱ ቅድመ እይታ ቅርሶችን በእጅ የ`sha256sum` ጥሪዎችን ሳያስፈልገው ያረጋግጣል። ስክሪፕቱን ለማስኬድ እና `npm run serve` (ወይንም ግልጽ የሆነውን I18NI0000039X ተለዋጭ ስም) ይጠቀሙ እና `docusaurus serve`ን በአንድ እርምጃ ለማስጀመር የሀገር ውስጥ ቅጽበተ-ፎቶዎችን ሲያጋሩ። የማረጋገጫ ሎጂክ፡-

1. ተገቢውን የSHA መሳሪያ (`sha256sum` ወይም I18NI0000042X) ከ `build/checksums.sha256` ጋር ይሰራል።
2. እንደ አማራጭ የቅድመ እይታ ገላጭውን I18NI0000044X መፍጨት/የፋይል ስም እና፣ ሲቀርብ፣ የቅድመ እይታ ማህደር ዳይጄስት/ፋይል ስም ያወዳድራል።
3. ገምጋሚዎች የተበላሹ ቅድመ እይታዎችን ማገድ እንዲችሉ ማንኛውም አለመዛመድ ሲገኝ ከዜሮ ውጭ ይወጣል።

የምሳሌ አጠቃቀም (የCI artefacts ካወጣን በኋላ)፡-

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

CI እና የመልቀቅ መሐንዲሶች የቅድመ እይታ ጥቅል ሲያወርዱ ወይም ቅርሶችን ከመልቀቂያ ትኬት ጋር በማያያዝ ወደ ስክሪፕቱ መደወል አለባቸው።

## ደረጃ 2 - SoraFS ማተም

1. የቅድመ እይታ የስራ ሂደትን በሚከተለው ስራ ያራዝሙ፡-
   - `sorafs_cli car pack` እና `manifest submit` በመጠቀም የተገነባውን ቦታ ወደ SoraFS የማስተናገጃ መግቢያ በር ይሰቀላል።
   - የተመለሰውን አንጸባራቂ መፍጨት እና SoraFS CID ይይዛል።
   - ተከታታይ I18NI0000047X ወደ I18NT0000004X JSON (I18NI0000048X) ያደርጋል።
2. ገላጩን ከግንባታ ጥበብ ጎን ያከማቹ እና CID ን በመጎተት ጥያቄ አስተያየት ላይ ያጋልጡ።
3. ወደፊት የሚደረጉ ለውጦች የሜታዳታ ንድፍ እንዲረጋጋ ለማድረግ `sorafs_cli` በደረቅ አሂድ ሁነታ የሚለማመዱ የውህደት ሙከራዎችን ያክሉ።

## ደረጃ 3 - አስተዳደር እና ኦዲት

1. በ`docs/portal/schemas/` ስር ያለውን ገላጭ መዋቅር የሚገልጽ የI18NT0000005X schema (`PreviewDescriptorV1`) ያትሙ።
2. የ DOCS-SORA የህትመት ማረጋገጫ ዝርዝር ያዘምኑ፡
   - `sorafs_cli manifest verify` ከተሰቀለው CID ጋር በማሄድ ላይ።
   - በተለቀቀው የ PR መግለጫ ውስጥ የቼክሰም ማኒፌክት መፍጨት እና CID መቅዳት።
3. በሚለቀቁበት ጊዜ ገላጩን ከቼክሰም ማኒፌክት ጋር ለመፈተሽ የአስተዳደር አውቶማቲክን ሽቦ ያድርጉ።

## የሚላኩ እና ባለቤትነት

| ወሳኝ ምዕራፍ | ባለቤት(ዎች) | ዒላማ | ማስታወሻ |
|-------------|--------|
| የ CI ቼክተም ማስፈጸሚያ አረፈ | ሰነዶች መሠረተ ልማት | ሳምንት 1 | የብልሽት በር + የጥበብ ሰቀላዎችን ይጨምራል። |
| SoraFS ቅድመ እይታ ማተም | ሰነዶች መሠረተ ልማት / ማከማቻ ቡድን | ሳምንት 2 | የማረጋገጫ ምስክርነቶችን እና የNorito የመርሃግብር ማሻሻያዎችን ማግኘት ይፈልጋል። |
| የአስተዳደር ውህደት | ሰነዶች/DevRel አመራር / አስተዳደር WG | ሳምንት 3 | የመርሃግብር + የማረጋገጫ ዝርዝሮችን እና የመንገድ ካርታ ግቤቶችን ያትማል። |

## ክፍት ጥያቄዎች

- የትኛው የSoraFS አካባቢ የቅድመ እይታ ቅርሶችን መያዝ አለበት (የቅድመ እይታ መስመርን ማቀድ)?
- ከመታተሙ በፊት በቅድመ-እይታ ገላጭ ላይ ድርብ ፊርማዎች (Ed25519 + ML-DSA) ያስፈልጉናል?
- አይ18NI00000054X ን ሲሰራ የኦርኬስትራ ውቅር (`orchestrator_tuning.json`) መገለጫዎችን እንደገና መባዛት እንዲችል የCI የስራ ፍሰት ፒን ማድረግ አለበት?

በI18NI0000055X ውስጥ ውሳኔዎችን ያንሱ እና ያልታወቁት ከተፈቱ ይህን እቅድ ያዘምኑ።