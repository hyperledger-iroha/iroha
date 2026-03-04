---
lang: am
direction: ltr
source: docs/source/mochi/packaging.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ab0877a6f43402d6ec13a44c4a7c2b68e4a49e6103bb50d7469d9e71aaa953
source_last_modified: "2025-12-29T18:16:35.984945+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI የማሸጊያ መመሪያ

ይህ መመሪያ የ MOCHI ዴስክቶፕ ሱፐርቫይዘር ጥቅል እንዴት እንደሚገነባ ያብራራል፣ ይመርምሩ
የተፈጠሩት ቅርሶች፣ እና የሩጫ ሰዓቱን ያስተካክሉት ያንን መርከብ ከ
ጥቅል። ሊባዛ በሚችል ማሸጊያ ላይ በማተኮር ፈጣን ጅምርን ያሟላል።
እና CI አጠቃቀም.

## ቅድመ ሁኔታዎች

- ዝገት የመሳሪያ ሰንሰለት (እ.ኤ.አ. 2024 / Rust 1.82+) ከስራ ቦታ ጥገኛዎች ጋር
  አስቀድሞ ተገንብቷል.
- `irohad`፣ `iroha_cli`፣ እና `kagami` ለተፈለገው ግብ ተሰብስቧል። የ
  ጥቅል ከ `target/<profile>/` ሁለትዮሽዎችን እንደገና ይጠቀማል።
- በ `target/` ወይም በብጁ ለጥቅል ውፅዓት በቂ የዲስክ ቦታ
  መድረሻ.

ማቀፊያውን ከማሄድዎ በፊት ጥገኞቹን አንድ ጊዜ ይገንቡ፡-

```bash
cargo build -p irohad -p iroha_cli -p iroha_kagami
```

## ጥቅሉን መገንባት

የተወሰነውን የ`xtask` ትዕዛዝ ከማከማቻ ስር ጥራ፡

```bash
cargo xtask mochi-bundle
```

በነባሪ ይህ በ `target/mochi-bundle/` ስር የመልቀቂያ ጥቅል ከ ሀ
የፋይል ስም ከአስተናጋጁ ስርዓተ ክወና እና አርክቴክቸር የተገኘ (ለምሳሌ፣
`mochi-macos-aarch64-release.tar.gz`). ለማበጀት የሚከተሉትን ባንዲራዎች ይጠቀሙ
ግንባታው:

- `--profile <name>` - የካርጎ መገለጫ ይምረጡ (`release` ፣ `debug` ፣ ወይም
  ብጁ መገለጫ)።
- `--no-archive` - `.tar.gz` ሳይፈጥሩ የተስፋፋውን ማውጫ ያስቀምጡ
  ማህደር (ለአካባቢያዊ ሙከራ ጠቃሚ ነው).
- `--out <path>` - በምትኩ ጥቅሎችን ወደ ብጁ ማውጫ ይፃፉ
  `target/mochi-bundle/`.
- `--kagami <path>` - አስቀድሞ የተሰራ `kagami` በ ውስጥ እንዲካተት ማስፈጸሚያ ያቅርቡ
  ማህደር. ሲቀር፣ ጥቅሉ ሁለትዮሽውን ከ. እንደገና ይጠቀማል (ወይም ይገነባል።)
  የተመረጠ መገለጫ.
- `--matrix <path>` - የጥቅል ሜታዳታ ወደ JSON ማትሪክስ ፋይል ጨምር (ከተፈጠረው
  ጠፍቷል) ስለዚህ CI ቧንቧዎች በ ሀ ውስጥ የተሰራውን እያንዳንዱን አስተናጋጅ/መገለጫ መመዝገብ ይችላሉ።
  መሮጥ ግቤቶች የጥቅል ማውጫውን፣ አንጸባራቂ መንገድን እና SHA-256ን፣ አማራጭን ያካትታሉ
  የማህደር ቦታ፣ እና የመጨረሻው የጭስ-ሙከራ ውጤት።
- `--smoke` - የታሸገውን `mochi --help` እንደ ቀላል ክብደት ያለው የጭስ በር ያስፈጽሙ
  ከጥቅል በኋላ; አለመሳካቶች አንድን ከማተምዎ በፊት የጎደሉትን ጥገኞች ያሳያሉ
  artefact.
- `--stage <path>` - የተጠናቀቀውን ጥቅል (እና ሲመረት ማህደር) ይቅዱ
  ባለብዙ ፕላትፎርም ግንባታዎች ቅርሶችን በአንድ ላይ ማስቀመጥ እንዲችሉ የዝግጅት ማውጫ ማውጫ
  ያለ ተጨማሪ ስክሪፕት ቦታ።

ትዕዛዙ `mochi-ui-egui`, `kagami`, `LICENSE`, ናሙናውን ይገለበጣል.
ማዋቀር፣ እና `mochi/BUNDLE_README.md` ወደ ጥቅል። መወሰኛ
`manifest.json` የ CI ስራዎች ፋይሉን መከታተል እንዲችሉ ከሁለትዮሽ ጎን ለጎን ነው የተፈጠረው
hashes እና መጠኖች.

## ቅርቅብ አቀማመጥ እና ማረጋገጫ

የተዘረጋ ጥቅል በ`BUNDLE_README.md` ውስጥ የተመዘገበውን አቀማመጥ ይከተላል፡-

```
bin/mochi
bin/kagami
config/sample.toml
docs/README.md
manifest.json
LICENSE
```

የ`manifest.json` ፋይል እያንዳንዱን ቅርስ በSHA-256 ሃሽ ይዘረዝራል። አረጋግጥ
ጥቅሉን ወደ ሌላ ስርዓት ከገለበጠ በኋላ፡-

```bash
jq -r '.files[] | "\(.sha256)  \(.path)"' manifest.json | sha256sum --check
```

CI ቧንቧዎች የተዘረጋውን ማውጫ መሸጎጥ፣ ማህደሩን መፈረም ወይም ማተም ይችላሉ።
አንጸባራቂው ከመልቀቂያ ማስታወሻዎች ጋር። አንጸባራቂው ጄነሬተርን ያካትታል
የመገለጫ፣ የዒላማ ሶስት እጥፍ እና የፍጥረት ጊዜ ማህተም የፕሮቨንስን መከታተልን ለመርዳት።

## የሩጫ ጊዜ ይሽራል።

MOCHI የረዳት ሁለትዮሽ እና የአሂድ ጊዜ ቦታዎችን በCLI ባንዲራዎች ወይም
የአካባቢ ተለዋዋጮች- `--data-root` / `MOCHI_DATA_ROOT` - ለአቻ ጥቅም ላይ የዋለውን የስራ ቦታ ይሽሩ
  ማዋቀር፣ ማከማቻ እና ምዝግብ ማስታወሻዎች።
- `--profile` - በቶፖሎጂ ቅድመ-ቅምጦች መካከል ይቀያይሩ (`single-peer` ፣
  `four-peer-bft`).
- `--torii-start`, `--p2p-start` - ሲመደብ ጥቅም ላይ የሚውሉትን የመሠረት ወደቦች ይለውጡ.
  አገልግሎቶች.
- `--irohad` / `MOCHI_IROHAD` - በአንድ የተወሰነ `irohad` ሁለትዮሽ ላይ ያመልክቱ።
- `--kagami` / `MOCHI_KAGAMI` - የተጠቀለለውን `kagami` ይሽሩ።
- `--iroha-cli` / `MOCHI_IROHA_CLI` - የአማራጭ CLI ረዳትን ይሽሩ።
- `--restart-mode <never|on-failure>` - አውቶማቲክ ዳግም ማስጀመርን ያሰናክሉ ወይም ያስገድዱት
  ገላጭ የኋላ ማጥፋት ፖሊሲ።
- `--restart-max <attempts>` - መቼ ዳግም ማስጀመር ሙከራዎችን ይሽሩ
  በ `on-failure` ሁነታ እየሄደ ነው።
- `--restart-backoff-ms <millis>` - ለራስ-ሰር ዳግም ማስጀመር መሰረቱን ያቀናብሩ።
- `MOCHI_CONFIG` - ብጁ `config/local.toml` መንገድ ያቅርቡ።

የCLI እገዛ (`mochi --help`) ሙሉ ባንዲራ ዝርዝሩን ያትማል። አካባቢ ይሽራል።
ማስጀመሪያ ላይ ይተገበራል እና በ ውስጥ ካለው የቅንጅቶች ንግግር ጋር ሊጣመር ይችላል።
ዩአይ

## CI አጠቃቀም ፍንጮች

- የሚችል ማውጫ ለማመንጨት `cargo xtask mochi-bundle --no-archive` ን ያሂዱ
  በመድረክ-ተኮር መሣሪያ (ዚፕ ለዊንዶውስ፣ ታርቦል ለ
  ዩኒክስ)።
- የጥቅል ሜታዳታ በ`cargo xtask mochi-bundle --matrix dist/matrix.json` ይያዙ
  ስለዚህ የተለቀቁ ስራዎች እያንዳንዱን አስተናጋጅ/መገለጫ የሚዘረዝር አንድ JSON ኢንዴክስ ማተም ይችላሉ።
  በቧንቧ ውስጥ የሚመረተው አርቲፊኬት.
- በእያንዳንዱ ላይ `cargo xtask mochi-bundle --stage /mnt/staging/mochi` (ወይም ተመሳሳይ) ይጠቀሙ
  ጥቅሉን ለመስቀል እና ወደ የተጋራው ማውጫ ውስጥ ለማስቀመጥ ወኪል ይገንቡ
  የህትመት ስራ ሊፈጅ ይችላል.
- ኦፕሬተሮች ቅርቅቡን ማረጋገጥ እንዲችሉ ሁለቱንም ማህደሩን እና `manifest.json` ያትሙ
  ታማኝነት ።
- የጭስ ሙከራዎችን ዘር ለመዝራት የተሰራውን ማውጫ እንደ የግንባታ ጥበብ ያከማቹ
  ተቆጣጣሪውን በቆራጥነት በታሸጉ ሁለትዮሽዎች ይለማመዱ።
- ለወደፊት በሚለቀቁት ማስታወሻዎች ወይም በ`status.md` ምዝግብ ማስታወሻ ውስጥ የጥቅል ሀሽዎችን ይቅዱ
  የፕሮቬንሽን ቼኮች.