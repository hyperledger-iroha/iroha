---
lang: mn
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

# Iroha JS CI лавлагаа

`@iroha/iroha-js` багц нь `iroha_js_host`-ээр дамжуулан эх холболтуудыг багцалдаг. Ямар ч
Туршилт эсвэл бүтээцийг гүйцэтгэдэг CI дамжуулах хоолой нь Node.js ажиллах хугацааг хоёуланг нь хангах ёстой
болон Rust хэрэгслийн гинжийг ашигласнаар эх багцыг туршилтын өмнө эмхэтгэх боломжтой
гүйх.

## Санал болгож буй алхамууд

1. `actions/setup-node` эсвэл өөрийн CI-ээр дамжуулан Node LTS хувилбарыг (18 эсвэл 20) ашиглана уу
   тэнцүү.
2. `rust-toolchain.toml`-д жагсаасан Rust хэрэгслийн гинжийг суулгана уу. Бид санал болгож байна
   GitHub үйлдлүүд дэх `dtolnay/rust-toolchain@v1`.
3. Үүнээс зайлсхийхийн тулд ачааны бүртгэл/git индекс болон `target/` лавлахыг кэш хийнэ үү.
   ажил бүрт төрөлх нэмэлтийг дахин бүтээх.
4. `npm install`, дараа нь `npm run lint:test` ажиллуул. Хосолсон скрипт нь хэрэгжүүлдэг
   ESLint нь тэг анхааруулгатай, үндсэн нэмэлтийг бүтээж, Node тестийг ажиллуулдаг
   CI нь хувилбарын гарцын ажлын урсгалтай таарч байна.
5. Сонголтоор `node --test`-г хурдан утааны шат болгон ажиллуулна `npm run build:native`
   нэмэлтийг үүсгэсэн (жишээ нь, дахин ашигладаг хурдан шалгах эгнээг урьдчилан оруулах
   кэшлэгдсэн олдворууд).
6. Хэрэглэгчийнхээ нэмэлт хөвөн эсвэл форматын чекийг давхарлана
   илүү хатуу бодлого шаардлагатай үед `npm run lint:test` дээр төсөл.
7. Үйлчилгээний хооронд тохиргоог хуваалцахдаа `iroha_config`-г ачаалж, дамжуулаарай.
   баримтыг `resolveToriiClientConfig({ config })` руу задлан шинжилж, зангилааны үйлчлүүлэгчид
   Бусад байршуулалтын адил завсарлага/дахин оролдох/токен бодлогыг дахин ашиглах (харна уу
   Бүрэн жишээний хувьд `docs/source/sdk/js/quickstart.md`).

## GitHub үйлдлийн загвар

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

## Хурдан утаатай ажил (заавал биш)

Зөвхөн баримт бичиг эсвэл TypeScript-ийн тодорхойлолтод хүрдэг татах хүсэлтийн хувьд a
Хамгийн бага ажил нь кэшэд хадгалагдсан олдворуудыг дахин ашиглах, эх модулийг дахин бүтээх, ажиллуулах боломжтой
Зангилааны туршилтын гүйгч шууд:

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

Энэ ажил нь уугуул нэмэлт программыг эмхэтгэсэн эсэхийг шалгахын зэрэгцээ хурдан дуусна
мөн Node тестийн багц тэнцсэн.

> **Лавлах хэрэгжилт:** репозитор орно
> `.github/workflows/javascript-sdk.yml`, энэ нь дээрх алхмуудыг a болгон холбодог
> Ачааны кэштэй зангилаа 18/20 матриц.