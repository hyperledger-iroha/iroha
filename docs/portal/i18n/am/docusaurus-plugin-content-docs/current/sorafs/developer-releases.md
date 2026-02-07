---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Release Process
summary: Run the CLI/SDK release gate, apply the shared versioning policy, and publish canonical release notes.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# የመልቀቅ ሂደት

SoraFS ሁለትዮሽ (`sorafs_cli`፣ `sorafs_fetch`፣ ረዳቶች) እና ኤስዲኬ ሳጥኖች
(`sorafs_car`፣ `sorafs_manifest`፣ `sorafs_chunker`) አንድ ላይ ይርከብ። የተለቀቀው
የቧንቧ መስመር CLI እና ቤተ-መጻሕፍትን ያቆያል፣ የሊንት/የሙከራ ሽፋንን ያረጋግጣል፣ እና
ለታችኛው ተፋሰስ ተጠቃሚዎች ቅርሶችን ይይዛል። ለእያንዳንዱ ከዚህ በታች ያለውን የማረጋገጫ ዝርዝር ያሂዱ
የእጩ መለያ.

## 0. የደህንነት ግምገማ ማቋረጥን ያረጋግጡ

የቴክኒካዊ መልቀቂያ በርን ከመፈፀምዎ በፊት የቅርብ ጊዜውን የደህንነት ግምገማ ይያዙ
ቅርሶች፡-

- በጣም የቅርብ ጊዜውን የ SF-6 የደህንነት ግምገማ ማስታወሻ አውርድ ([ሪፖርቶች/sf6-የደህንነት-ግምገማ](./reports/sf6-security-review.md))
  እና የእሱን SHA256 hash በመልቀቂያ ቲኬቱ ውስጥ ይመዝግቡ።
- የማሻሻያ ትኬት ማገናኛን (ለምሳሌ `governance/tickets/SF6-SR-2026.md`) ያያይዙ እና መቋረጥን ያስተውሉ
  ከደህንነት ምህንድስና እና ከመሳሪያ ስራ ቡድን አጽዳቂዎች።
- በማስታወሻው ውስጥ ያለው የማገገሚያ ዝርዝር መዘጋቱን ያረጋግጡ; ያልተፈቱ ነገሮች ልቀቱን ያግዱታል።
- የተመጣጣኝ መታጠቂያ ምዝግብ ማስታወሻዎችን ለመስቀል ያዘጋጁ (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  ከአንጸባራቂው ጥቅል ጋር።
- ለማስኬድ ያቀዱትን የፊርማ ትዕዛዝ ያረጋግጡ `--identity-token-provider` እና ግልጽ
  `--identity-token-audience=<aud>` ስለዚህ Fulcio scope የሚለቀቀው ማስረጃ ውስጥ ተያዘ.

አስተዳደርን ሲያስታውቁ እና ልቀቱን ሲያትሙ እነዚህን ቅርሶች ያካትቱ።

## 1. የመልቀቂያውን/የፈተናውን በር ያስፈጽሙ

የ`ci/check_sorafs_cli_release.sh` ረዳት ቅርጸት መስራትን፣ ክሊፒን እና ሙከራዎችን ይሰራል።
በCLI እና ኤስዲኬ ሳጥኖች ከስራ ቦታ-አካባቢያዊ ዒላማ ማውጫ (`.target`) ጋር
በ CI ኮንቴይነሮች ውስጥ በሚከናወኑበት ጊዜ የፍቃድ ግጭቶችን ለማስወገድ።

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

ስክሪፕቱ የሚከተሉትን ማረጋገጫዎች ይፈጽማል፡-

- `cargo fmt --all -- --check` (የስራ ቦታ)
- `cargo clippy --locked --all-targets` ለ `sorafs_car` (ከ`cli` ባህሪ ጋር)
  `sorafs_manifest`፣ እና `sorafs_chunker`
- `cargo test --locked --all-targets` ለእነዚያ ተመሳሳይ ሳጥኖች

ማንኛውም እርምጃ ካልተሳካ፣ መለያ ከመስጠትዎ በፊት ተሃድሶውን ያስተካክሉት። የመልቀቂያ ግንባታዎች መሆን አለባቸው
ከዋናው ጋር ቀጣይነት ያለው; ወደ መልቀቂያ ቅርንጫፎች የቼሪ-አስተካክል አይምረጡ. በሩ
እንዲሁም ቁልፍ አልባ ፊርማ ባንዲራዎችን ይፈትሻል (`--identity-token-issuer`፣ `--identity-token-audience`)
በሚተገበርበት ቦታ ይሰጣሉ; የጎደሉ ክርክሮች ሩጫውን ወድቀዋል።

## 2. የስርጭት ፖሊሲን ይተግብሩ

ሁሉም SoraFS CLI/SDK ሳጥኖች SemVer ይጠቀማሉ፡

- `MAJOR`: ለመጀመሪያው 1.0 ልቀት አስተዋውቋል። ከ 1.0 በፊት የ I18NI0000031X ጥቃቅን እብጠት
  ** በ CLI ወለል ወይም I18NT0000000X እቅዶች ላይ መበላሸት ለውጦችን ያሳያል።
  ከአማራጭ ፖሊሲ ጀርባ የተከለሉ መስኮች፣ የቴሌሜትሪ ተጨማሪዎች)።
- `PATCH`፡ የሳንካ ጥገናዎች፣ የሰነድ-ብቻ ልቀቶች እና የጥገኝነት ዝማኔዎች
  የሚታይ ባህሪን አይቀይሩ.

ሁልጊዜ `sorafs_car`፣ `sorafs_manifest` እና I18NI0000035Xን በተመሳሳይ ያቆዩ።
ሥሪት ስለዚህ የታችኛው የኤስዲኬ ሸማቾች በአንድ የተስተካከለ ሥሪት ላይ ሊመኩ ይችላሉ።
ሕብረቁምፊ. ስሪቶችን በሚያደናቅፉበት ጊዜ፡-

1. በእያንዳንዱ የሣጥን `Cargo.toml` የI18NI0000036X መስኮችን ያዘምኑ።
2. `Cargo.lock` በ `cargo update -p <crate>@<new-version>` በኩል ያድሱ (የ
   የስራ ቦታ ግልጽ ስሪቶችን ያስፈጽማል).
3. የቆዩ ቅርሶች እንዳልቀሩ ለማረጋገጥ የመልቀቂያውን በር እንደገና አስኪዱ።

## 3. የመልቀቂያ ማስታወሻዎችን ያዘጋጁ

እያንዳንዱ ልቀት CLIን፣ ኤስዲኬን እናን የሚያደምቅ የማርክ ማድረጊያ ሎግ ማተም አለበት።
የአስተዳደር-ተፅዕኖ ለውጦች. አብነቱን በ ውስጥ ይጠቀሙ
`docs/examples/sorafs_release_notes.md` (ወደ የተለቀቁት ቅርሶችዎ ይቅዱት።
ማውጫ እና ክፍሎቹን በተጨባጭ ዝርዝሮች ይሙሉ).

ዝቅተኛ ይዘት፡

- ** ዋና ዋና ዜናዎች *** ለ CLI እና SDK ሸማቾች የባህሪ አርዕስተ ዜናዎች።
  መስፈርቶች.
- ** ደረጃዎችን አሻሽል ***: TL; DR የጭነት ጥገኛዎችን ለማደናቀፍ እና እንደገና ለመሮጥ ትዕዛዞችን ይሰጣል
  የሚወስኑ ቋሚዎች.
- ** ማረጋገጫ ***: የትዕዛዝ ውጤት hashes ወይም ኤንቨሎፕ እና ትክክለኛው
  `ci/check_sorafs_cli_release.sh` ክለሳ ተፈፅሟል።

የተሞሉትን የመልቀቂያ ማስታወሻዎች ከመለያው ጋር ያያይዙ (ለምሳሌ፣ GitHub የተለቀቀው አካል) እና ማከማቻ
በቆራጥነት ከተፈጠሩ ቅርሶች ጋር።

## 4. የመልቀቂያ መንጠቆዎችን ያስፈጽም

የፊርማ ቅርቅቡን ለማመንጨት `scripts/release_sorafs_cli.sh` ን ያሂዱ
የማረጋገጫ ማጠቃለያ ከእያንዳንዱ ልቀት ጋር። መጠቅለያው CLI ን ይገነባል።
አስፈላጊ ሲሆን `sorafs_cli manifest sign` ይደውላል እና ወዲያውኑ እንደገና ያጫውታል።
`manifest verify-signature` ስለዚህ መለያ ከመስጠትዎ በፊት ወድቋል። ምሳሌ፡-

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

ጠቃሚ ምክሮች

- የመልቀቂያ ግብዓቶችን (የክፍያ ጭነት፣ ዕቅዶች፣ ማጠቃለያዎች፣ የሚጠበቀው ቶከን ሃሽ) በእርስዎ ውስጥ ይከታተሉ
  ስክሪፕቱ ሊባዛ የሚችል ሆኖ እንዲቆይ repo ወይም ማሰማራት ውቅር። የሲ.አይ.ፒ
  ጥቅል በ `fixtures/sorafs_manifest/ci_sample/` ስር ቀኖናዊውን አቀማመጥ ያሳያል።
- ቤዝ CI አውቶሜሽን በ `.github/workflows/sorafs-cli-release.yml`; የሚለውን ያስኬዳል
  የመልቀቂያ በር፣ ከላይ ያለውን ስክሪፕት ጠርቶ፣ እና ቅርቅቦችን/ፊርማዎችን በማህደር እንደ
  የስራ ሂደት ቅርሶች. ተመሳሳዩን የትእዛዝ ቅደም ተከተል ያንጸባርቁ (የልቀት በር → ምልክት →
  ማረጋገጥ) በሌሎች CI ስርዓቶች ስለዚህ የኦዲት ምዝግብ ማስታወሻዎች ከተፈጠረው ሃሽ ጋር ይሰለፋሉ።
- የተፈጠረውን I18NI0000047X፣ `manifest.sig`፣
  `manifest.sign.summary.json` እና `manifest.verify.summary.json` አንድ ላይ - እነሱ
  በአስተዳደር ማስታወቂያ ውስጥ የተጠቀሰውን ፓኬት ይመሰርቱ።
- የተለቀቀው ቀኖናዊ መጫዎቻዎችን ሲያዘምን ፣ የታደሰውን አንጸባራቂ ይቅዱ ፣
  chunk plan፣ እና ወደ `fixtures/sorafs_manifest/ci_sample/` (እና ማዘመን
  `docs/examples/sorafs_ci_sample/manifest.template.json`) መለያ ከመስጠትዎ በፊት።
  የታችኛው ተፋሰስ ኦፕሬተሮች ልቀቱን እንደገና ለማባዛት በቁርጠኝነት በተቀመጡት ዕቃዎች ላይ ይወሰናሉ።
  ጥቅል።
- ለI18NI0000053X ወሰን-ሰርጥ ማረጋገጫ የሩጫ ምዝግብ ማስታወሻን ያንሱ እና ከ
  የማስረጃ ዥረት መከላከያዎችን ለማሳየት የመልቀቂያ ፓኬት ገቢር ሆኖ ይቆያል።
- በመልቀቂያ ማስታወሻዎች ውስጥ በሚፈርሙበት ጊዜ ጥቅም ላይ የዋለውን ትክክለኛውን I18NI0000054X ይመዝግቡ; አስተዳደር
  ህትመቱን ከማጽደቁ በፊት ተመልካቾችን በፉልሲዮ ፖሊሲ ላይ ያጣራል።

ልቀቱ ሀ ሲይዝ `scripts/sorafs_gateway_self_cert.sh` ይጠቀሙ
መግቢያ በር መልቀቅ. ምስክሩን ለማረጋገጥ በተመሳሳዩ አንጸባራቂ ጥቅል ላይ ያመልክቱ
ከእጩው ቅርስ ጋር ይዛመዳል

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. መለያ ስጥ እና አትም

ቼኮች ካለፉ እና መንጠቆዎቹ ከተጠናቀቀ በኋላ፡-

1. ሁለትዮሾችን ለማረጋገጥ `sorafs_cli --version` እና I18NI0000057X ን ያሂዱ
   አዲሱን ስሪት ሪፖርት ያድርጉ.
2. የመልቀቂያ ውቅር በተረጋገጠ `sorafs_release.toml` ውስጥ ያዘጋጁ
   (የተሻለ) ወይም ሌላ የማዋቀር ፋይል በእርስዎ የማሰማራት ሪፖ ክትትል የሚደረግበት። አስወግዱ
   በአድ-ሆክ የአካባቢ ተለዋዋጮች ላይ መተማመን; ወደ CLI መንገዶችን ማለፍ
   `--config` (ወይም ተመጣጣኝ) ስለዚህ የመልቀቂያ ግብዓቶች ግልጽ እና
   ሊባዛ የሚችል.
3. የተፈረመ መለያ (የተመረጠ) ወይም የተብራራ መለያ ይፍጠሩ፡
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. ቅርሶችን ይስቀሉ (የ CAR ቅርቅቦች፣ መግለጫዎች፣ የማረጋገጫ ማጠቃለያዎች፣ የመልቀቂያ ማስታወሻዎች፣
   የማረጋገጫ ውጤቶች) ከአስተዳደሩ በኋላ ለፕሮጀክት መዝገብ ቤት
   የማረጋገጫ ዝርዝር በ [የማሰማራት መመሪያ](./developer-deployment.md)። ከተለቀቀ
   አዳዲስ መገልገያዎችን ፈጥረው፣ ወደ የተጋራው fixture repo ወይም የነገር ማከማቻ ይግፏቸው
   ኦዲት አውቶማቲክ የታተመውን ጥቅል ከምንጭ ቁጥጥር ጋር ሊለያይ ይችላል።
5. ከተፈረመበት መለያ ጋር አገናኞችን ለአስተዳደር ጣቢያው ያሳውቁ ፣ ማስታወሻዎችን ይልቀቁ ፣
   አንጸባራቂ ቅርቅብ/ፊርማ hashes፣ በማህደር የተቀመጠ `manifest.sign/verify` ማጠቃለያ፣
   እና ማንኛውም የማረጋገጫ ኤንቨሎፕ። ያንን የCI job URL (ወይም የመዝገብ መዝገብ) ያካትቱ
   `ci/check_sorafs_cli_release.sh` እና `scripts/release_sorafs_cli.sh` ሮጧል። አዘምን
   የአስተዳደር ትኬቱ ኦዲተሮች ከቅርሶች ጋር ማፅደቂያ ማግኘት እንዲችሉ፣ መቼ
   `.github/workflows/sorafs-cli-release.yml` የስራ ልጥፎች ማሳወቂያዎች፣ ያገናኙት።
   የአድ-ሆክ ማጠቃለያዎችን ከመለጠፍ ይልቅ የተመዘገቡ የሃሽ ውጤቶች።

## 6. ከተለቀቀ በኋላ ክትትል

- ወደ አዲሱ ስሪት የሚጠቁሙ ሰነዶችን ያረጋግጡ (ፈጣን ጅምር ፣ CI አብነቶች)
  ተዘምኗል ወይም ምንም ለውጦች አያስፈልጉም ያረጋግጡ።
- ከተከታታይ ሥራ (ለምሳሌ፣ የፍልሰት ባንዲራዎች፣ መቋረጥ) የመንገድ ካርታ ግቤቶችን ያቅርቡ
- ለኦዲተሮች የመልቀቂያ በር የውጤት መዝገቦችን በማህደር ያስቀምጡ - ከተፈረመው አጠገብ ያከማቹ
  ቅርሶች.

ይህንን የቧንቧ መስመር ተከትሎ የ CLI፣ ኤስዲኬ ሳጥኖች እና የአስተዳደር ዋስትናን ያቆያል
ለእያንዳንዱ የመልቀቂያ ዑደት የመቆለፊያ ደረጃ.