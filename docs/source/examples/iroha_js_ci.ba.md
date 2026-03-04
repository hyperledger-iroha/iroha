---
lang: ba
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

# Iroha JS CI Һылтанма

`@iroha/iroha-js` пакет өйөмдәре туған бәйләүҙәр аша `iroha_js_host`. Теләһә ниндәй
CI торба үткәргес, тип һынауҙар йәки төҙөүҙәр башҡарыла тейеш, тип тәьмин итеү өсөн Node.js эшләү ваҡыты .
һәм Rust инструменттар слет шулай тыуған өйөмө һынауҙар алдынан төҙөргә мөмкин
йүгерергә.

## Тәҡдим ителгән аҙымдар

1. Ҡулланыу Node LTS релиз (18 йәки 20) аша `actions/setup-node` йәки һеҙҙең CI .
   тиңдәш.
. Беҙ кәңәш итәбеҙ
   GitHub ғәмәлдәрендә `dtolnay/rust-toolchain@v1`.
3. Кэш йөк реестры/гит индекстары һәм `target/` каталогы ҡотолоу өсөн
   һәр эштә тыуған аддонды яңынан төҙөү.
4. Run `npm install`, һуңынан `npm run lint:test`. Берләштерелгән сценарий үтәй
   ESLint нуль иҫкәртмәләр менән, тыуған addon төҙөй, һәм Node һынау үткәрә
   люкс шулай CI тап килә релиз ҡапҡа эш ағымы.
.
   addon етештергән (мәҫәлән, тиҙ тикшерелгән һыҙаттарҙы ҡабаттан ҡулланыу тип фаразлана
   кэш артефакттары).
6. Ҡатлам ниндәй ҙә булһа өҫтәмә линт йәки форматлау тикшерелгән һеҙҙең ҡулланыусы
   проекты өҫтөндә `npm run lint:test` ҡатыраҡ сәйәсәт талап ителә.
7. Хеҙмәттәр буйынса конфигурация менән бүлешкәндә, `iroha_config` йөк һәм тапшырыу
   анализланған документ `resolveToriiClientConfig({ config })` шулай Node клиенттары
   ҡабаттан ҡулланыу шул уҡ тайм-аут/ҡабатлау/жетон сәйәсәте ҡалған таратыу (ҡара
   Тулы миҫал өсөн `docs/source/sdk/js/quickstart.md`).

## GitHub ғәмәлдәре ҡалыптары

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

## Тиҙ төтөн эш (Һоралы)

тартыу өсөн запростар, тип тик сенсор документация йәки TypeScript билдәләмәләре, а
минималь эш кэшланған артефакттарҙы ҡабаттан ҡуллана ала, тыуған модулде яңынан төҙөй һәм йүгерә ала
Төйөн һынау йүгерсеһе туранан-тура:

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

Был эш тиҙ тамамлана, шул уҡ ваҡытта тикшерергә, тип туған addon компиляция
һәм Node тест люксы үтә тип.

> **Һылтанма тормошҡа ашырыу:** һаҡлағыс инә.
> `.github/workflows/javascript-sdk.yml`, был өҫтәге аҙымдарҙы сымдар а
> Йөк кэшлауы менән 18/20 матрицаһы.