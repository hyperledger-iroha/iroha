---
lang: hy
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

# Iroha JS CI Հղում

`@iroha/iroha-js` փաթեթը միավորում է բնիկ կապերը `iroha_js_host`-ի միջոցով: Ցանկացած
CI խողովակաշարը, որն իրականացնում է թեստեր կամ կառուցումներ, պետք է ապահովի երկուսն էլ Node.js գործարկման ժամանակ
և Rust գործիքների շղթան, որպեսզի հայրենի փաթեթը հնարավոր լինի կազմել թեստերից առաջ
վազել.

## Առաջարկվող քայլեր

1. Օգտագործեք Node LTS թողարկում (18 կամ 20) `actions/setup-node`-ի կամ ձեր CI-ի միջոցով
   համարժեք։
2. Տեղադրեք Rust գործիքների շղթան, որը նշված է `rust-toolchain.toml`-ում: Մենք խորհուրդ ենք տալիս
   `dtolnay/rust-toolchain@v1` GitHub Actions-ում:
3. Խուսափելու համար պահեք բեռների ռեեստրի/git ինդեքսները և `target/` գրացուցակը
   վերակառուցելով հայրենի հավելումը յուրաքանչյուր աշխատանքում:
4. Գործարկեք `npm install`, ապա `npm run lint:test`: Համակցված սցենարը ուժի մեջ է մտնում
   ESLint-ը զրոյական նախազգուշացումներով, կառուցում է բնիկ հավելումը և գործարկում Node թեստը
   հավաքակազմ, որպեսզի CI-ն համապատասխանի թողարկման դարպասի աշխատանքային հոսքին:
5. Ընտրովի գործարկեք `node --test` որպես արագ ծխի քայլ մեկ անգամ `npm run build:native`
   ստեղծել է հավելումը (օրինակ՝ նախապես ուղարկել արագ ստուգման ուղիներ, որոնք նորից օգտագործվում են
   քեշավորված արտեֆակտներ):
6. Շերտավորեք ձեր սպառողից ցանկացած հավելյալ երեսպատման կամ ձևաչափման ստուգումներ
   նախագիծ `npm run lint:test`-ի վերևում, երբ պահանջվում է ավելի խիստ քաղաքականություն:
7. Ծառայությունների միջև կոնֆիգուրացիան կիսելիս բեռնեք `iroha_config` և փոխանցեք
   վերլուծված փաստաթուղթը `resolveToriiClientConfig({ config })`-ին, որպեսզի Node-ի հաճախորդներին
   վերօգտագործել նույն ժամանակի վերջնաժամկետը/կրկնել/նշան քաղաքականությունը, ինչպես մնացած տեղակայումը (տես
   `docs/source/sdk/js/quickstart.md` ամբողջական օրինակի համար):

## GitHub Գործողությունների Կաղապար

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

## Արագ ծխի աշխատանք (ըստ ցանկության)

Ձգվող հարցումների համար, որոնք վերաբերում են միայն փաստաթղթերին կամ TypeScript սահմանումներին, ա
նվազագույն աշխատանքը կարող է նորից օգտագործել քեշավորված արտեֆակտները, վերակառուցել բնիկ մոդուլը և գործարկել
Հանգույցի փորձարկման վազող ուղղակիորեն.

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

Այս աշխատանքն ավարտվում է արագ՝ միաժամանակ հաստատելով, որ հայրենի հավելումը կազմվում է
և որ Node թեստային փաթեթը անցնում է:

> **Հղման իրականացում.** պահեստը ներառում է
> `.github/workflows/javascript-sdk.yml`, որը վերը նշված քայլերը միացնում է a
> Հանգույց 18/20 մատրիցա՝ բեռների քեշավորումով: