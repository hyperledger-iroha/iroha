---
lang: uz
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

# Iroha JS CI ma'lumotnomasi

`@iroha/iroha-js` to'plami mahalliy ulanishlarni `iroha_js_host` orqali birlashtiradi. Har qanday
Sinovlar yoki tuzilmalarni bajaradigan CI quvur liniyasi ikkala Node.js ish vaqtini ham ta'minlashi kerak
va Rust asboblar zanjiri, shuning uchun mahalliy to'plamni testlardan oldin kompilyatsiya qilish mumkin
yugur.

## Tavsiya etilgan qadamlar

1. `actions/setup-node` yoki CI orqali Node LTS versiyasidan (18 yoki 20) foydalaning
   ekvivalent.
2. `rust-toolchain.toml` ro'yxatidagi Rust asboblar zanjirini o'rnating. Biz tavsiya qilamiz
   GitHub harakatlarida `dtolnay/rust-toolchain@v1`.
3. Bunga yo'l qo'ymaslik uchun yuk registri/git indekslari va `target/` katalogini keshlang.
   har bir ishda mahalliy qo'shimchani qayta qurish.
4. `npm install`, keyin `npm run lint:test` ni ishga tushiring. Birlashtirilgan skript amalga oshiradi
   ESLint nol ogohlantirishlarga ega, mahalliy qo'shimchani yaratadi va tugun testini ishga tushiradi
   to'plam, shuning uchun CI relizlar ish jarayoniga mos keladi.
5. `npm run build:native` ni ixtiyoriy ravishda tez tutun qadami sifatida ishga tushiring.
   qo'shimchani ishlab chiqardi (masalan, qayta ishlatiladigan tezkor tekshirish yo'llarini oldindan taqdim eting
   keshlangan artefaktlar).
6. Iste'molchidan qo'shimcha linting yoki formatlash tekshiruvlarini joylashtiring
   qat'iyroq siyosatlar talab qilinganda `npm run lint:test` ustidagi loyiha.
7. Xizmatlar boʻylab konfiguratsiyani ulashganda, `iroha_config` ni yuklang va
   Node mijozlari uchun `resolveToriiClientConfig({ config })` ga tahlil qilingan hujjat
   tarqatishning qolgan qismi bilan bir xil vaqt tugashi/qayta urinish/token siyosatini qayta ishlating (qarang
   To'liq misol uchun `docs/source/sdk/js/quickstart.md`).

## GitHub harakatlar shabloni

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

## Tez chekish ishi (ixtiyoriy)

Faqat hujjatlar yoki TypeScript ta'riflariga tegadigan tortishish so'rovlari uchun a
minimal ish keshlangan artefaktlarni qayta ishlatishi, mahalliy modulni qayta qurishi va ishga tushirishi mumkin
To'g'ridan-to'g'ri tugunni sinovdan o'tkazuvchi:

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

Bu ish tezda tugallanadi, shu bilan birga mahalliy qo'shimcha kompilyatsiya qilishini tasdiqlaydi
va Node test to'plami o'tadi.

> **Ma'lumotni amalga oshirish:** omborga kiradi
> `.github/workflows/javascript-sdk.yml`, bu yuqoridagi bosqichlarni a ga o'tkazadi
> Yuklarni keshlash bilan tugun 18/20 matritsasi.