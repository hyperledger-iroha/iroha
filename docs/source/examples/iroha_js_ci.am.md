---
lang: am
direction: ltr
source: docs/source/examples/iroha_js_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d21afd33da5ee459b1f6ffb6ac7c42adc0852ed7929e69993f81914637b5e6b5
source_last_modified: "2025-12-29T18:16:35.953373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
  This document provides guidance for running @iroha/iroha-js in CI systems.
-->

# Iroha JS CI ማጣቀሻ

የ`@iroha/iroha-js` ጥቅል ቤተኛ ማሰሪያዎችን በ`iroha_js_host` በኩል ያጠባል። ማንኛውም
ሙከራዎችን የሚያከናውን ወይም የሚገነባ CI ቧንቧ ሁለቱንም የ Node.js አሂድ ጊዜ መስጠት አለበት።
እና የ Rust toolchain ስለዚህ ቤተኛ ጥቅል ከሙከራዎቹ በፊት ሊጠናቀር ይችላል።
መሮጥ

## የሚመከሩ እርምጃዎች

1. የመስቀለኛ LTS ልቀት (18 ወይም 20) በ`actions/setup-node` ወይም በእርስዎ CI በኩል ይጠቀሙ
   ተመጣጣኝ.
2. በ `rust-toolchain.toml` ውስጥ የተዘረዘሩትን Rust የመሳሪያ ሰንሰለት ይጫኑ. እንመክራለን
   `dtolnay/rust-toolchain@v1` በ GitHub ድርጊቶች።
3. የካርጎ መዝገብ/ጂት ኢንዴክሶችን እና የ`target/` ማውጫን ለማስወገድ
   በእያንዳንዱ ሥራ ውስጥ የአገሬውን አዶን እንደገና መገንባት።
4. `npm install`, ከዚያ `npm run lint:test` አሂድ. የተጣመረ ስክሪፕት ያስፈጽማል
   ESLint ከዜሮ ማስጠንቀቂያዎች ጋር፣ ቤተኛ አዶን ይገነባል እና የመስቀለኛ መንገድ ፈተናን ያካሂዳል
   suite so CI ከተለቀቀው gating የስራ ፍሰት ጋር ይዛመዳል።
5. `node --test`ን እንደ ፈጣን የጭስ እርምጃ አንድ ጊዜ ያሂዱ `npm run build:native`
   አዶን ሠርቷል (ለምሳሌ፣ እንደገና ጥቅም ላይ የሚውሉ ፈጣን ፍተሻ መስመሮችን አስቀድመው ያስገቡ
   የተሸጎጡ ቅርሶች)።
6. ከሸማችህ የሚመጡትን ማንኛውንም ተጨማሪ የመሸፈኛ ወይም የቅርጸት ቼኮች ደርድር
   ጥብቅ ፖሊሲዎች በሚያስፈልጉበት ጊዜ በ `npm run lint:test` ላይ ፕሮጀክት.
7. ውቅረትን በአገልግሎቶች ላይ ሲያጋሩ `iroha_config` ይጫኑ እና ይለፉ
   የተተነተነ ሰነድ ለ`resolveToriiClientConfig({ config })` ስለዚህ የመስቀለኛ መንገድ ደንበኞች
   ከተቀረው ማሰማራቱ ጋር ተመሳሳይ የጊዜ ማብቂያ/እንደገና ይሞክሩ/ ማስመሰያ ፖሊሲን እንደገና ይጠቀሙ (ይመልከቱ
   `docs/source/sdk/js/quickstart.md` ለሙሉ ምሳሌ)።

## GitHub የተግባር አብነት

```yaml
name: iroha-js-ci

on:
  push:
    branches: [ main ]
  pull_request:

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        node-version: [18, 20]
    steps:
      - uses: actions/checkout@v4

      - name: Set up Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: npm

      - name: Set up Rust toolchain
        uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable

      - name: Cache cargo artifacts
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - run: npm install
      - run: npm run lint:test
```

## ፈጣን የጭስ ሥራ (አማራጭ)

ሰነዶችን ወይም የTyScript ፍቺዎችን ብቻ ለሚነኩ የመሳብ ጥያቄዎች፣ ሀ
አነስተኛ ስራ የተሸጎጡ ቅርሶችን እንደገና መጠቀም፣ ቤተኛ ሞጁሉን እንደገና መገንባት እና ማስኬድ ይችላል።
የመስቀለኛ መንገድ ሙከራ ሯጭ በቀጥታ፡-

```yaml
jobs:
  smoke:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable
      - run: npm ci
      - run: npm run build:native
      - run: node --test
```

ቤተኛ አድዶን ማጠናቀሩን እያረጋገጠ ይህ ስራ በፍጥነት ይጠናቀቃል
እና የመስቀለኛ ፈተና ስብስብ ያልፋል.

> ** የማመሳከሪያ አተገባበር፡** ማከማቻው ያካትታል
> `.github/workflows/javascript-sdk.yml`፣ እሱም ከላይ ያሉትን ደረጃዎች ወደ ሀ
> መስቀለኛ መንገድ 18/20 ማትሪክስ ከጭነት መሸጎጫ ጋር።