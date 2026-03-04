---
lang: kk
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

# Iroha JS CI анықтамасы

`@iroha/iroha-js` бумасы `iroha_js_host` арқылы жергілікті байланыстыруларды жинақтайды. Кез келген
Сынақтарды немесе құрастыруды орындайтын CI құбыры Node.js орындалу уақытының екеуін де қамтамасыз етуі керек
және Rust құралдар тізбегі, осылайша жергілікті буманы сынақтар алдында құрастыруға болады
жүгіру.

## Ұсынылатын қадамдар

1. `actions/setup-node` немесе CI арқылы Node LTS шығарылымын (18 немесе 20) пайдаланыңыз.
   эквивалент.
2. `rust-toolchain.toml` тізімінде берілген Rust құралдар тізбегін орнатыңыз. Біз ұсынамыз
   GitHub әрекеттеріндегі `dtolnay/rust-toolchain@v1`.
3. болдырмау үшін жүк тізілімін/git индекстерін және `target/` каталогын кэштеңіз.
   әрбір жұмыста жергілікті қосымшаны қайта құру.
4. `npm install`, содан кейін `npm run lint:test` іске қосыңыз. Біріктірілген сценарий орындалады
   Нөлдік ескертулері бар ESLint, жергілікті қосымшаны құрастырады және Түйін сынағын іске қосады
   жиынтығы, сондықтан CI шығару шлюзінің жұмыс үрдісіне сәйкес келеді.
5. Қосымша `node --test` жылдам түтін шығару қадамы ретінде `npm run build:native` бір рет іске қосыңыз
   қосымшаны жасады (мысалы, қайта пайдаланылатын жылдам тексеру жолақтарын алдын ала жіберу
   кэштелген артефактілер).
6. Тұтынушының кез келген қосымша линтинг немесе пішімдеу тексерулерін қабаттаңыз
   қатаңырақ саясат қажет болғанда `npm run lint:test` жобасының үстіне.
7. Конфигурацияны қызметтерде ортақ пайдаланған кезде, `iroha_config` жүктеңіз және
   құжат `resolveToriiClientConfig({ config })`, сондықтан Түйін клиенттеріне талданды
   орналастырудың қалған бөлігімен бірдей күту/қайталау/таңбалауыш саясатын қайта пайдаланыңыз (қараңыз
   Толық мысал үшін `docs/source/sdk/js/quickstart.md`).

## GitHub әрекеттер үлгісі

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

## Жылдам түтін шығару жұмысы (қосымша)

Тек құжаттаманы немесе TypeScript анықтамаларын түртетін тарту сұраулары үшін, a
ең аз тапсырма кэштелген артефактілерді қайта пайдалана алады, жергілікті модульді қайта құра алады және іске қоса алады
Түйіннің сынақ жүгірткісі тікелей:

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

Бұл тапсырма жергілікті қосымшаның құрастыруын тексеру кезінде тез аяқталады
және Түйін сынақ жинағы өтеді.

> **Анықтаманы орындау:** репозиторий қамтиды
> `.github/workflows/javascript-sdk.yml`, ол жоғарыдағы қадамдарды a
> Жүк кэштеуімен түйін 18/20 матрицасы.