---
lang: am
direction: ltr
source: README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:30:39.016220+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Hyperledger Iroha

[![ፍቃድ](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha ለፈቃድ እና ለጋራ ማሰማራቶች የሚወስን blockchain መድረክ ነው። በIroha ቨርቹዋል ማሽን (IVM) የመለያ/ንብረት አስተዳደር፣ በሰንሰለት ላይ ፍቃዶችን እና ዘመናዊ ኮንትራቶችን ያቀርባል።

> የስራ ቦታ ሁኔታ እና የቅርብ ጊዜ ለውጦች በ[`status.md`](./status.md) ውስጥ ክትትል ይደረግባቸዋል።

## የመልቀቂያ ትራኮች

ይህ ማከማቻ ከተመሳሳይ ኮድ ቤዝ ሁለት የማሰማራት ትራኮችን ይልካል።

- **Iroha 2**: በራሳቸው የሚስተናገዱ የተፈቀደ/የኮንሰርቲየም ኔትወርኮች።
- ** Iroha 3 (SORA I18NT0000019X)**: Nexus-ተኮር የማሰማራት ትራክ ተመሳሳይ ኮር ሳጥኖችን በመጠቀም።

ሁለቱም ትራኮች Norito ተከታታይነት፣ Sumeragi ስምምነት እና የKotodama -> IVM የመሳሪያ ሰንሰለትን ጨምሮ ተመሳሳይ ዋና ክፍሎችን ይጋራሉ።

## የማጠራቀሚያ አቀማመጥ

- [`crates/`](./crates): ኮር ዝገት ሳጥኖች (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `iroha_core`, `iroha_core`,010000079X,010000000X,I01000079X. `norito`፣ ወዘተ)።
- [`integration_tests/`](./integration_tests): ተሻጋሪ አካል አውታረ መረብ / ውህደት ሙከራዎች.
- [`IrohaSwift/`](./IrohaSwift)፡ ስዊፍት ኤስዲኬ ጥቅል።
- [`java/iroha_android/`](./java/iroha_android)፡ የአንድሮይድ ኤስዲኬ ጥቅል።
- [`docs/`](./docs): ተጠቃሚ/ኦፕሬተር/ገንቢ ሰነድ።

## ፈጣን ጅምር

### ቅድመ ሁኔታ

- [ዝገት የተረጋጋ](https://www.rust-lang.org/tools/install)
- አማራጭ፡ I18NT0000009X + Docker ለሀገር ውስጥ ባለ ብዙ አቻ ሩጫዎች አዘጋጅ

### ይገንቡ እና ይሞክሩ (የስራ ቦታ)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

ማስታወሻዎች፡-

- ሙሉ የስራ ቦታ መገንባት 20 ደቂቃ ያህል ሊወስድ ይችላል።
- ሙሉ የስራ ቦታ ሙከራዎች ብዙ ሰዓታት ሊወስዱ ይችላሉ።
- የስራ ቦታው `std` ኢላማ ያደርጋል (WASM/no-std ግንባታዎች አይደገፉም)።

### የታለሙ የሙከራ ትዕዛዞች

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### የኤስዲኬ ሙከራ ትዕዛዞች

```bash
cd IrohaSwift
swift test
```

```bash
cd java/iroha_android
JAVA_HOME=$(/usr/libexec/java_home -v 21) \
ANDROID_HOME=~/Library/Android/sdk \
ANDROID_SDK_ROOT=~/Library/Android/sdk \
./gradlew test
```

## የአካባቢ አውታረ መረብን ያሂዱ

የቀረበውን Docker ጻፍ አውታረ መረብ ይጀምሩ፡

```bash
docker compose -f defaults/docker-compose.yml up
```

CLI ን ከነባሪው የደንበኛ ውቅር ጋር ተጠቀም፡-

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

ለዴሞን-ተኮር ቤተኛ ማሰማራት ደረጃዎች፣ [`crates/irohad/README.md`](./crates/irohad/README.md) ይመልከቱ።

## ኤፒአይ እና ታዛቢነት

Torii ሁለቱንም I18NT0000007X እና JSON APIs ያጋልጣል። የጋራ ኦፕሬተር የመጨረሻ ነጥቦች፡-

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

ሙሉውን የማጠቃለያ ነጥብ በዚህ ውስጥ ይመልከቱ፡-

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## ኮር ሳጥኖች

- [`crates/iroha`](./crates/iroha): የደንበኛ ቤተ መጻሕፍት.
- [`crates/irohad`](./crates/irohad): አቻ ዴሞን ሁለትዮሾች.
- [`crates/iroha_cli`](./crates/iroha_cli): ማጣቀሻ CLI.
- [`crates/iroha_core`](./crates/iroha_core): ደብተር/ዋና ማስፈጸሚያ ሞተር።
- [`crates/iroha_config`] (./crates/iroha_config): የተተየበው የውቅር ሞዴል.
- [`crates/iroha_data_model`] (./crates/iroha_data_model): ቀኖናዊ ውሂብ ሞዴል.
- [`crates/iroha_crypto`](./crates/iroha_crypto): ምስጠራ ፕሪሚቲቭ.
- [`crates/norito`](./crates/norito): deterministic ተከታታይ ኮድ.
- [`crates/ivm`] (./crates/ivm): Iroha ምናባዊ ማሽን.
- [`crates/iroha_kagami`] (./crates/iroha_kagami): ቁልፍ / ዘፍጥረት / ማዋቀር መሳሪያ.

## የሰነድ ካርታ

- ዋና ሰነዶች መረጃ ጠቋሚ፡ [`docs/README.md`](./docs/README.md)
- ዘፍጥረት፡ [`docs/genesis.md`](./docs/genesis.md)
- ስምምነት (I18NT0000003X)፡ [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- የግብይት መስመር፡ [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P ውስጣዊ፡ [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM syscals: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama ሰዋሰው፡ [I18NI0000113X](./docs/source/kotodama_grammar.md)
- Norito ሽቦ ቅርጸት፡ [`norito.md`](./norito.md)
- የአሁኑ የስራ ክትትል፡ [`status.md`](./status.md)፣ [`roadmap.md`](./roadmap.md)

## ትርጉሞች

የጃፓን አጠቃላይ እይታ፡ [`README.ja.md`](./README.ja.md)

ሌሎች አጠቃላይ እይታዎች፡-
[`README.he.md`](./README.he.md)፣ [`README.es.md`](./README.es.md)፣ [`README.pt.md`](./README.pt.md)፣ [`README.fr.md`](./README.fr.md)፣ [`README.ru.md`](./README.ru.md)፣ [`README.ar.md`](./README.ar.md)፣ [`README.ur.md`](./README.ur.md)

የትርጉም የስራ ሂደት፡ [I18NI0000125X](./docs/i18n/README.md)

## ማበርከት እና ማገዝ

- የአስተዋጽኦ መመሪያ፡ [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- የማህበረሰብ/የድጋፍ ቻናሎች፡ [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## ፍቃድ

Iroha በ Apache-2.0 ፍቃድ ተሰጥቶታል። [`LICENSE`](./LICENSE) ይመልከቱ።

ሰነዱ በ CC-BY-4.0፡ http://creativecommons.org/licenses/by/4.0/ ፍቃድ ተሰጥቶታል።