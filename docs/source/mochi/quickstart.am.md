---
lang: am
direction: ltr
source: docs/source/mochi/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44faf6c98d141959cf8cf40b1df7d3d82c3448e6f2b1bc4fa54cdeceb97994b0
source_last_modified: "2025-12-29T18:16:35.985408+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI ፈጣን ጅምር

**MOCHI** ለአካባቢያዊ Hyperledger Iroha አውታረ መረቦች የዴስክቶፕ ተቆጣጣሪ ነው። ይህ መመሪያ ያልፋል
ቅድመ ሁኔታዎችን መጫን, አፕሊኬሽኑን መገንባት, የ egui ሼልን ማስጀመር እና የ
ለቀን-ቀን ልማት የሩጫ ጊዜ መሳሪያዎች (ቅንብሮች፣ ቅጽበተ-ፎቶዎች፣ መጥረጊያዎች)።

## ቅድመ ሁኔታዎች

- Rust toolchain: `rustup default stable` (የስራ ቦታ ኢላማዎች እትም 2024 / Rust 1.82+)።
- የመሳሪያ ሰንሰለት;
  - MacOS: Xcode Command Line Tools (`xcode-select --install`)።
  - ሊኑክስ፡ GCC፣ pkg-config፣ OpenSSL ራስጌዎች (`sudo apt install build-essential pkg-config libssl-dev`)።
- Iroha የስራ ቦታ ጥገኞች፡-
  - `cargo xtask mochi-bundle` የተገነባ `irohad`፣ `kagami` እና `iroha_cli` ይፈልጋል። አንድ ጊዜ በ በኩል ይገንቧቸው
    `cargo build -p irohad -p kagami -p iroha_cli`.
- አማራጭ፡ `direnv` ወይም `cargo binstall` ለአካባቢው የካርጎ ሁለትዮሽ አስተዳደር።

MOCHI ወደ CLI ሁለትዮሾች ይወጣል። በአካባቢ ተለዋዋጮች በኩል ሊገኙ እንደሚችሉ ያረጋግጡ
ከታች ወይም በ PATH ላይ ይገኛል፡

| ሁለትዮሽ | አካባቢ መሻር | ማስታወሻ |
|------------------|------------------------------------|
| `irohad` | `MOCHI_IROHAD` | እኩዮችን ይቆጣጠራል |
| `kagami` | `MOCHI_KAGAMI` | ዘፍጥረትን ያመነጫል/ቅጽበተ-ፎቶ |
| `iroha_cli` | `MOCHI_IROHA_CLI` | ለመጪው የረዳት ባህሪያት አማራጭ |

## MOCHI መገንባት

ከማከማቻ ስር፡-

```bash
cargo build -p mochi-ui-egui
```

ይህ ትዕዛዝ ሁለቱንም `mochi-core` እና egui frontend ይገነባል። ሊሰራጭ የሚችል ጥቅል ለማምረት፣ ያሂዱ፡-

```bash
cargo xtask mochi-bundle
```

የጥቅል ተግባር በ`target/mochi-bundle` ስር ሁለትዮሾችን፣ አንጸባራቂ እና ማዋቀርን ይሰበስባል።

## የ egui ሼል ማስጀመር

UI ን በቀጥታ ከጭነት ያሂዱ፡-

```bash
cargo run -p mochi-ui-egui
```

በነባሪ MOCHI በጊዜያዊ የውሂብ ማውጫ ውስጥ የአንድ-አቻ ቅድመ-ቅምጥን ይፈጥራል፡-

- የውሂብ ሥር: `$TMPDIR/mochi`.
- Torii ቤዝ ወደብ: `8080`.
- P2P ቤዝ ወደብ: `1337`.

ሲጀመር ነባሪዎችን ለመሻር የCLI ባንዲራዎችን ይጠቀሙ፡-

```bash
cargo run -p mochi-ui-egui -- \
  --data-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

የአካባቢ ተለዋዋጮች የCLI ባንዲራዎች ሲቀሩ ተመሳሳይ መሻሮችን ያንፀባርቃሉ፡ `MOCHI_DATA_ROOT` አዘጋጅ፣
`MOCHI_PROFILE`፣ `MOCHI_CHAIN_ID`፣ `MOCHI_TORII_START`፣ `MOCHI_P2P_START`፣ `MOCHI_RESTART_MODE`፣
`MOCHI_RESTART_MAX`፣ ወይም `MOCHI_RESTART_BACKOFF_MS` የበላይ ተቆጣጣሪውን መገንቢያ ቅድመ ዝግጅት ማድረግ; ሁለትዮሽ መንገዶች
`MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI` እና `MOCHI_CONFIG` ነጥቦችን ማክበርዎን ይቀጥሉ
ግልጽ `config/local.toml`.

## ቅንጅቶች እና ጽናት

የሱፐርቫይዘሩን ውቅረት ለማስተካከል **ቅንጅቶች** ከዳሽቦርዱ የመሳሪያ አሞሌ ይክፈቱ፡

- ** የውሂብ ስር ** - ለአቻ ውቅሮች ፣ ማከማቻ ፣ ምዝግብ ማስታወሻዎች እና ቅጽበተ-ፎቶዎች የመሠረት ማውጫ።
- ** Torii / P2P የመሠረት ወደቦች *** - ለመወሰኛ ምደባ መነሻ ወደቦች።
- ** የምዝግብ ማስታወሻ ታይነት *** — በሎግ መመልከቻ ውስጥ stdout/stderr/system channels ቀይር።

እንደ ተቆጣጣሪው ዳግም ማስጀመር መመሪያ ያሉ የላቁ ቁልፎች በቀጥታ በ ውስጥ
`config/local.toml`. ለማሰናከል `[supervisor.restart] mode = "never"` አዘጋጅ
በስህተት ማረም ጊዜ በራስ-ሰር እንደገና ይጀምራል ወይም ያስተካክሉ
`max_restarts`/`backoff_ms` (በማዋቀር ፋይሉ ወይም በCLI ባንዲራዎች በኩል)
`--restart-mode`፣ `--restart-max`፣ `--restart-backoff-ms`) እንደገና ይሞክሩ
ባህሪ.ለውጦችን መተግበር ተቆጣጣሪውን እንደገና ይገነባል፣ ማንኛቸውም አቻዎችን እንደገና ያስጀምራል እና መሻሪያዎቹን ይጽፋል
`config/local.toml`. የላቁ ተጠቃሚዎች ማቆየት እንዲችሉ የውቅረት ውህደት የማይገናኙ ቁልፎችን ይጠብቃል።
በMOCHI ከሚተዳደሩ እሴቶች ጋር በእጅ የተደረጉ ለውጦች።

## ቅጽበታዊ ገጽ እይታዎች እና መጥረግ/ዳግም-ዘፍጥረት

**የጥገና** ንግግር ሁለት የደህንነት ስራዎችን ያጋልጣል፡-

- ** ቅጽበታዊ ገጽ እይታን ወደ ውጭ ላክ *** - የአቻ ማከማቻ/ውቅረት/ምዝግብ ማስታወሻዎችን እና የአሁኑን ዘፍጥረት ይገለጻል።
  `snapshots/<label>` በንቁ የመረጃ ስር። መለያዎች በራስ-ሰር ይጸዳሉ።
- ** ቅጽበተ-ፎቶን ወደነበረበት መልስ *** - የአቻ ማከማቻን፣ ቅጽበተ-ፎቶ ሥሮችን፣ ማዋቀርን፣ ምዝግቦችን እና ዘፍጥረትን ያድሳል።
  ከነባር ጥቅል አንጸባራቂ። `Supervisor::restore_snapshot` ፍጹም መንገድ ወይም ይቀበላል
  የጸዳው `snapshots/<label>` የአቃፊ ስም; UI ይህንን ፍሰት ያንፀባርቃል ስለዚህ ጥገና → ወደነበረበት መመለስ
  ፋይሎችን በእጅ ሳይነኩ የማስረጃ እሽጎችን እንደገና ማጫወት ይችላል።
- ** መጥረግ እና እንደገና ማመንጨት *** - እኩዮችን መሮጥ ያቆማል፣ የማከማቻ ማውጫዎችን ያስወግዳል፣ ዘፍጥረትን በ
  Kagami፣ እና ማጽዳቱ ሲጠናቀቅ እኩዮቹን እንደገና ይጀምራል።

ሁለቱም ፍሰቶች በእንደገና ሙከራዎች (`export_snapshot_captures_storage_and_metadata`፣
`wipe_and_regenerate_resets_storage_and_genesis`) የሚወስኑ ውጤቶችን ዋስትና ለመስጠት።

## መዝገቦች እና ዥረቶች

ዳሽቦርዱ በጨረፍታ ውሂብ/መለኪያዎችን ያጋልጣል፡-

- ** መዝገቦች *** - `irohad` stdout/stderr/የስርዓት የህይወት ዑደት መልዕክቶችን ይከተላል። በቅንብሮች ውስጥ ሰርጦችን ይቀያይሩ።
- ** ያግዳል/ክስተቶች** — የሚተዳደሩ ዥረቶች ከሰፊ የኋላ መጥፋት እና ማብራሪያ ክፈፎች ጋር በራስ-ሰር ዳግም ይገናኙ
  ከ Norito-የተገለጹ ማጠቃለያዎች ጋር።
- ** ሁኔታ *** — ምርጫዎች `/status` እና ለወረፋ ጥልቀት፣ ውፅዓት እና መዘግየት ብልጭታዎችን ይሰጣል።
- ** የጅምር ዝግጁነት *** - ከተጫኑ በኋላ ** ጀምር *** (ነጠላ አቻ ወይም ሁሉም እኩዮች) ፣ MOCHI ምርመራዎች።
  `/status` ከታሰረ ጀርባ ጋር; ባነር እያንዳንዱ እኩያ ሲዘጋጅ ሪፖርት ያደርጋል (ከተስተዋለው ጋር
  የወረፋ ጥልቀት) ወይም የዝግጁነት ጊዜ ካለፈ የTorii ስህተቱን ይሸፍነዋል።

የስቴት አሳሽ እና አቀናባሪ ትሮች ወደ መለያዎች፣ ንብረቶች፣ አቻዎች እና የተለመዱ ፈጣን መዳረሻ ይሰጣሉ
ከዩአይዩ ሳይወጡ መመሪያዎች። ማረጋገጥ እንዲችሉ የእኩዮች እይታ የ`FindPeers` መጠይቁን ያንፀባርቃል
የውህደት ሙከራዎችን ከማካሄድዎ በፊት የትኞቹ የህዝብ ቁልፎች በአሁኑ ጊዜ በአረጋጋጭ ስብስብ ውስጥ ተመዝግበዋል ።

የመፈረሚያ ባለስልጣናትን ለማስመጣት ወይም ለማርትዕ የአቀናባሪውን የመሳሪያ አሞሌ **አቀናብር ፊርማ ማከማቻን ይጠቀሙ። የ
መገናኛ ወደ ንቁው የአውታረ መረብ ስርወ (`<data_root>/<profile>/signers.json`) ግቤቶችን ይጽፋል እና ተቀምጧል
የቮልት ቁልፎች ወዲያውኑ ለግብይት ቅድመ እይታዎች እና ግቤቶች ይገኛሉ። ማስቀመጫው በሚሆንበት ጊዜ
ባዶ አቀናባሪው ተመልሶ ወደ ተሰቀለው የእድገት ቁልፎች ስለሚወድቅ የሀገር ውስጥ የስራ ፍሰቶች መስራታቸውን ይቀጥላሉ።
ቅጾች አሁን ሚንት/ማቃጠል/ማስተላለፍ (ስውር መቀበልን ጨምሮ)፣ ጎራ/መለያ/ንብረት-ፍቺን ይሸፍናሉ።
ምዝገባ፣ የመለያ መግቢያ ፖሊሲዎች፣ ባለብዙ ሲግ ፕሮፖዛል፣ የቦታ ማውጫ መግለጫዎች (AXT/AMX)፣
SoraFS ፒን ይገለጣል፣ እና የአስተዳደር እርምጃዎች በጣም የተለመዱ ሚናዎችን መስጠት ወይም መሻር
የመንገድ ካርታ-ደራሲ ስራዎች ያለ እጅ-መፃፍ Norito ጭነቶች ሊለማመዱ ይችላሉ.

## ማፅዳት እና መላ መፈለግ- ክትትል የሚደረግባቸው እኩዮችን ለማቋረጥ ማመልከቻውን ያቁሙ።
- ሁሉንም ሁኔታ እንደገና ለማስጀመር የውሂብ ስር (`rm -rf <data_root>`) ያስወግዱ።
- Kagami ወይም Irohad አካባቢዎች ከተቀየሩ የአካባቢ ተለዋዋጮችን ያዘምኑ ወይም MOCHIን በ
  ተገቢ የ CLI ባንዲራዎች; የቅንጅቶች መገናኛ በሚቀጥለው ተፈጻሚነት ላይ አዳዲስ መንገዶችን ይቀጥላል.

ለተጨማሪ አውቶሜሽን ፍተሻ `mochi/mochi-core/tests` (ተቆጣጣሪ የህይወት ዑደት ሙከራዎች) እና
`mochi/mochi-integration` ለተሳለቁ Torii ሁኔታዎች። ጥቅሎችን ለመላክ ወይም ሽቦውን ለመላክ
ዴስክቶፕ ወደ CI ቧንቧዎች፣ የ{doc}`mochi/packaging` መመሪያን ይመልከቱ።

## የአካባቢ የሙከራ በር

ጥገናዎችን ከመላክዎ በፊት `ci/check_mochi.sh` ን ያሂዱ ስለዚህ የጋራው CI በር ሦስቱንም MOCHI ይለማመዳል
ሳጥኖች:

```bash
./ci/check_mochi.sh
```

ረዳቱ `cargo check`/`cargo test` ለ `mochi-core`፣ `mochi-ui-egui` እና
`mochi-integration`፣ የቋሚ ተንሸራታች (ቀኖናዊ ብሎክ/የዝግጅት ቀረጻ) እና egui መታጠቂያ የሚይዝ
በአንድ ምት ውስጥ regressions. ስክሪፕቱ የቆዩ መጫዎቻዎችን ሪፖርት ካደረገ፣ ችላ የተባሉትን የማደስ ሙከራዎችን እንደገና ያካሂዱ፣
ለምሳሌ፡-

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

እንደገና ከታደሰ በኋላ በሩን እንደገና ማስኬድ የተዘመነው ባይት ከመግፋትዎ በፊት ወጥነት ባለው መልኩ መቆየቱን ያረጋግጣል።