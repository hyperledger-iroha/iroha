---
lang: am
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 303a947895c10c7673b98e9187c3431c4012093c69d899252c121b53f9c48bb1
source_last_modified: "2026-01-05T09:28:11.823299+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS CLI እና SDK — የመልቀቂያ ማስታወሻዎች (v0.1.0)

## ድምቀቶች
- `sorafs_cli` አሁን ሙሉውን የማሸጊያ ቧንቧ መስመር (`car pack`፣ `manifest build`፣
  `proof verify`፣ `manifest sign`፣ `manifest verify-signature`) ስለዚህ የCI ሯጮች
  ከረዳት ረዳቶች ይልቅ ነጠላ ሁለትዮሽ። አዲሱ ቁልፍ-አልባ የመፈረሚያ ፍሰት ነባሪ ነው።
  `SIGSTORE_ID_TOKEN`፣ GitHub Actions OIDC አቅራቢዎችን ይገነዘባል እና ቆራጥነት ያመነጫል።
  JSON ማጠቃለያ ከፊርማ ቅርቅቡ ጋር።
- ባለብዙ-ምንጭ ማምጣት * የውጤት ሰሌዳ * እንደ `sorafs_car` አካል ሆኖ ይላካል፡ መደበኛ ያደርጋል
  አቅራቢ ቴሌሜትሪ፣ የችሎታ ቅጣቶችን ያስፈጽማል፣ የJSON/I18NT0000000X ሪፖርቶችን ይቀጥላል፣ እና
  የኦርኬስትራ ሲሙሌተርን (`sorafs_fetch`) በጋራ የመመዝገቢያ መያዣ በኩል ይመገባል።
  በ `fixtures/sorafs_manifest/ci_sample/` ስር ያሉ ቋሚዎች ቆራጥነትን ያሳያሉ
  ሲአይ/ሲዲ ይለያሉ ተብሎ የሚጠበቁ ግብዓቶች እና ውጤቶች።
- የልቀት አውቶማቲክ በ `ci/check_sorafs_cli_release.sh` እና የተስተካከለ ነው።
  `scripts/release_sorafs_cli.sh`. እያንዳንዱ ልቀት አሁን አንጸባራቂ ቅርቅቡን በማህደር ያስቀምጣል።
  ፊርማ፣ `manifest.sign/verify` ማጠቃለያዎች፣ እና የውጤት ሰሌዳው ቅጽበታዊ ገጽ እይታ ስለዚህ አስተዳደር
  ገምጋሚዎች የቧንቧ መስመሩን እንደገና ሳይሰሩ ቅርሶችን መፈለግ ይችላሉ።

## ደረጃዎችን ማሻሻል
1. በስራ ቦታዎ ውስጥ ያሉትን የተጣጣሙ ሳጥኖች ያዘምኑ፡
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. የfmt/clippy/የሙከራ ሽፋንን ለማረጋገጥ የመልቀቂያውን በር በአገር ውስጥ (ወይም በ CI) እንደገና ያስኪዱ።
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. የተፈረሙ ቅርሶችን እና ማጠቃለያዎችን በተዘጋጀው ውቅር እንደገና ማመንጨት፡-
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   የታደሱ ጥቅሎችን/ማስረጃዎችን ወደ `fixtures/sorafs_manifest/ci_sample/` ከሆነ ይቅዱ
   ዝማኔዎች ቀኖናዊ ዕቃዎችን ይልቀቁ።

## ማረጋገጫ
- የሚለቀቅበት በር: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` ወዲያውኑ በሩ ከተሳካ በኋላ)።
- `ci/check_sorafs_cli_release.sh` ውፅዓት፡ ውስጥ ተቀምጧል
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (ከተለቀቀው ጥቅል ጋር ተያይዟል).
- የጥቅል መፈጨትን አሳይ፡ `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`)።
- የማረጋገጫ ማጠቃለያ መፍጨት፡ `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`)።
- የምግብ መፈጨትን ይግለጹ (ለታችኛው ተፋሰስ ማረጋገጫ መስቀለኛ ቼኮች)
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (ከ`manifest.sign.summary.json`)።

## ማስታወሻዎች ለኦፕሬተሮች
- የTorii መግቢያ በር አሁን የ`X-Sora-Chunk-Range` የችሎታ ራስጌን ያስፈጽማል። አዘምን
  የፍቃድ ዝርዝሮች አዲሱን የዥረት ማስመሰያ ወሰን የሚያቀርቡ ደንበኞች እንዲቀበሉ ፣ የቆዩ ምልክቶች
  ያለ ክልል የይገባኛል ጥያቄ ይቀነሳል።
- `scripts/sorafs_gateway_self_cert.sh` አንጸባራቂ ማረጋገጫን ያዋህዳል። ሲሮጥ
  የራስ ሰር ማረጋገጫው መታጠቂያው፣ መጠቅለያው እንዲችል አዲስ የተፈጠረውን አንጸባራቂ ጥቅል ያቅርቡ
  በፊርማ ተንሸራታች ላይ በፍጥነት አለመሳካት።
- ቴሌሜትሪ ዳሽቦርዶች አዲሱን የውጤት ሰሌዳ ወደ ውጭ መላክ (`scoreboard.json`) ማስገባት አለባቸው
  የአቅራቢውን ብቁነት፣ የክብደት ምደባዎችን እና እምቢተኝነት ምክንያቶችን ማስታረቅ።
- በእያንዳንዱ የታቀደ አራቱን ቀኖናዊ ማጠቃለያዎች በማህደር ያስቀምጡ፡
  `manifest.bundle.json`፣ `manifest.sig`፣ `manifest.sign.summary.json`፣
  `manifest.verify.summary.json`. የአስተዳደር ትኬቶች እነዚህን ትክክለኛ ፋይሎች በወቅቱ ይጠቅሳሉ
  ማጽደቅ.

## ምስጋናዎች
- የማከማቻ ቡድን - ከጫፍ እስከ ጫፍ CLI ማጠናከሪያ፣ ቸንክ-ፕላን ሰሪ እና የውጤት ሰሌዳ
  ቴሌሜትሪ የቧንቧ መስመር.
- Tooling WG - የመልቀቂያ ቧንቧ (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) እና የሚወስን ቋሚ ጥቅል።
- የጌትዌይ ኦፕሬሽኖች - የችሎታ መግቢያ ፣ የዥረት ማስመሰያ ፖሊሲ ግምገማ እና የዘመነ
  ራስን ማረጋገጫ የመጫወቻ መጽሐፍት.