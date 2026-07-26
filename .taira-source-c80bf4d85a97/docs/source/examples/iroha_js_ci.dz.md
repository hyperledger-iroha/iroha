---
lang: dz
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

# Iroha JS CI གཞི་བསྟུན།

`@iroha/iroha-js` ཐུམ་སྒྲིལ་འདི་གིས་ `iroha_js_host` བརྒྱུད་དེ་ མི་ཁུངས་བཱའིན་ཌིང་ཚུ་ བསྡུ་སྒྲིག་འབདཝ་ཨིན། ག་འབད་རུང
བརྟག་དཔྱད་ཡང་ན་བཟོ་བསྐྲུན་པ་ཚུ་ལག་ལེན་འཐབ་མི་ CI གི་མདོང་ལམ་འདི་གིས་ Node.js རན་ཊའིམ་གཉིས་ཆ་ར་བྱིན་དགོ།
དང་ རཱསིཊི་ལག་ཆས་རྒྱུན་རིམ་འདི་ བརྟག་ཞིབ་ཚུ་གི་ཧེ་མ་ རང་བཞིན་གྱི་བཱན་ཌལ་འདི་ བསྡུ་སྒྲིག་འབད་ཚུགས།
རྒྱུག༌ནི།

## འོས་འཚམས་པའི་རིམ་པ་ནི།

1. `actions/setup-node` ཡང་ན་ ཁྱོད་ཀྱི་ CI བརྒྱུད་དེ་ Node LTS གསར་བཏོན་ (༡༨ ཡང་ན་ ༢༠) ལག་ལེན་འཐབ།
   དོ༌མཉམ།
2. `rust-toolchain.toml` ནང་ཐོ་བཀོད་འབད་ཡོད་པའི་ Rust ལག་ཆས་རྒྱུན་རིམ་བཙུགས། ང་ཚོས་གྲོས་འཆར་བཀོད།
   `dtolnay/rust-toolchain@v1` ནང་ གི་ཊི་ཧབ་བྱ་སྤྱོད་ནང་།
3. དངོས་པོ་ཐོ་བཀོད་/git ཟུར་ཐོ་ཚུ་དང་ `target/` སྣོད་ཐོ་ཚུ་ འདྲ་མཛོད་འབད་དགོ།
   ལཱ་ག་རའི་ནང་ ས་གནས་ཀྱི་ addon བསྐྱར་བཟོ་འབད་ནི།
4. `npm install`, དེ་ལས་ `npm run lint:test` རྒྱུག་ནི། མཉམ་སྡེབ་ཡིག་ཚུགས་ བརྟན་བརྟན་ཚུ།
   ESLint ཉེན་བརྡ་ཀླད་ཀོར་དང་ ས་གནས་ཀྱི་ཨེ་ཌི་ཌོན་བཟོ་བསྐྲུན་འབད་ཞིནམ་ལས་ ནོཌི་བརྟག་དཔྱད་འདི་གཡོག་བཀོལཝ་ཨིན།
   pere དེ་འབདཝ་ལས་ CI གིས་ གསར་བཏོན་འབད་སྒོ་ལཱ་གི་རྒྱུན་རིམ་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
༥ གདམ་ཁའི་ཐོག་ལས་ `node --test` འདི་ `npm run build:native` གིས་ མགྱོགས་དྲགས་སྦེ་ དུ་པའི་རིམ་པ་ཅིག་སྦེ་ གཡོག་བཀོལ་དོ་ཡོདཔ་ཨིན།
   addon (དཔེར་ན་ ལོག་སྟེ་ལག་ལེན་འཐབ་མི་ མགྱོགས་དྲགས་ཞིབ་དཔྱད་ཀྱི་ལམ་ཚུ་ བཏོན་ཡོདཔ་ཨིན།
   cared aryacts).
༦ ཁྱོད་རའི་ཉོ་སྤྱོད་པ་ལས་ ཁ་སྐོང་གྲལ་ཐིག་ཡང་ན་ རྩ་སྒྲིག་འབད་ནིའི་ཞིབ་དཔྱད་གང་རུང་ཚུ་ བང་རིམ་འབད།
   ལས་འགུལ་འདི་ སྲིད་བྱུས་དམ་དྲག་དགོ་པའི་སྐབས་ `npm run lint:test` གི་ཐོག་ཁར་ཨིན།
7. ཞབས་ཏོག་ཚུ་ནང་རིམ་སྒྲིག་བརྗེ་སོར་འབད་བའི་སྐབས་ `iroha_config` མངོན་གསལ་འབད་ཞིནམ་ལས་ ༡ ལུ་སྤྲོད་དགོ།
   `resolveToriiClientConfig({ config })` ལུ་དབྱེ་དཔྱད་འབད་ཡོདཔ་ལས་ ནོ་ཌི་མཁོ་སྤྲོད་པ་ཚུ།
   བཀྲམ་སྤེལ་ལྷག་ལུས་ཚུ་བཟུམ་སྦེ་ དུས་ཚོད་གཅིག་མཚུངས་/རེ་ཊི་/ཊོ་ཀེན་སྲིད་བྱུས་འདི་ལོག་ལག་ལེན་འཐབ། (བལྟ། (see
   དཔེ་ཆ་ཚང་གི་དོན་ལུ་ `docs/source/sdk/js/quickstart.md`)།

## གིཏ་ཧབ་བྱ་བའི་དཔེ་ཚད།

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

## མགྱོགས་མྱུར་གྱི་ཐ་མག་ལཱ་ (གདམ་ཁ་)

འཐེན་པའི་ཞུ་བ་ཚུ་གི་དོན་ལུ་ ཡིག་ཆ་ཡང་ན་ ཡིག་གཟུགས་ཡིག་གཟུགས་ཀྱི་ངེས་ཚིག་ཚུ་ ཨེབ་གཏང་འབད་ནི་གི་དོན་ལུ་ཨིན།
ལཱ་ཉུང་ཤོས་འདི་གིས་ འདྲ་མཛོད་འབད་ཡོད་པའི་ཅ་རྙིང་ཚུ་ལོག་ལག་ལེན་འཐབ་བཏུབ་ནི་ཨིན་ དེ་ལས་ རང་བྱུང་ཚད་གཞི་འདི་ལོག་སྟེ་བཟོ་བསྐྲུན་འབད་དེ་ གཡོག་བཀོལ་ཚུགས།
ནའུཊི་བརྟག་དཔྱད་རྒྱུག་འཕྲིན།

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

ལཱ་འདི་ ད་ལྟོ་ཡང་ ས་གནས་ཀྱི་ addon བསྡུ་སྒྲིག་འབད་མི་ཚུ་ཨིནམ་སྦེ་ བདེན་དཔྱད་འབད་བའི་སྐབས་ མཇུག་བསྡུཝ་ཨིན།
དང་ ནོ་ཌི་བརྟག་དཔྱད་སྡེ་ཚན་འདི་ བརྒལ་འགྱོཝ་ཨིན།

> * བསྟུན་པའི་ལག་ལེན:** མཛོད་ཁང་ནང་ཚུད་ཡོད།
> `.github/workflows/javascript-sdk.yml`, གིས་ གོང་འཁོད་ཀྱི་གོམ་པ་ཚུ་ ༡ ལུ་ཐགཔ་སྦེ་ གློག་ཐག་བཏངམ་ཨིན།
> མཐུད་མཚམས་ ༡༨/༢༠ མེ་ཊིགསི་དང་གཅིག་ཁར་ ཅ་ཆས་ཚུ་ འདྲ་མཛོད་ནང་།