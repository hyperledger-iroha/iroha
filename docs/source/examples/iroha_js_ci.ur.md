---
lang: ur
direction: rtl
source: docs/source/examples/iroha_js_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d21afd33da5ee459b1f6ffb6ac7c42adc0852ed7929e69993f81914637b5e6b5
source_last_modified: "2026-01-03T18:08:00.440949+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
  This document provides guidance for running @iroha/iroha-js in CI systems.
-->

# Iroha JS CI حوالہ

`@iroha/iroha-js` پیکیج `iroha_js_host` کے ذریعے آبائی پابندیاں بنڈل کرتا ہے۔ کوئی بھی
سی آئی پائپ لائن جو ٹیسٹوں یا تعمیرات کو انجام دیتی ہے دونوں کو نوڈ ڈاٹ جے ایس رن ٹائم فراہم کرنا چاہئے
اور زنگ ٹولچین تاکہ مقامی بنڈل ٹیسٹ سے پہلے مرتب کیا جاسکے
چلائیں

## تجویز کردہ اقدامات

1. `actions/setup-node` یا اپنے CI کے توسط سے نوڈ LTS ریلیز (18 یا 20) استعمال کریں
   مساوی
2. `rust-toolchain.toml` میں درج زنگ ٹول چین کو انسٹال کریں۔ ہم تجویز کرتے ہیں
   `dtolnay/rust-toolchain@v1` گٹ ہب ایکشنز میں۔
3. کیشے کارگو رجسٹری/گٹ انڈیکس اور `target/` ڈائرکٹری سے بچنے کے لئے
   ہر کام میں آبائی اڈون کی تعمیر نو۔
4. چلائیں `npm install` ، پھر `npm run lint:test`۔ مشترکہ اسکرپٹ نافذ کرتا ہے
   صفر انتباہ کے ساتھ ایسلنٹ ، آبائی اڈون بناتا ہے ، اور نوڈ ٹیسٹ چلاتا ہے
   سویٹ تو سی آئی ریلیز گیٹنگ ورک فلو سے مماثل ہے۔
5. اختیاری طور پر `node --test` کو تیز دھواں مرحلہ کے طور پر چلائیں ایک بار `npm run build:native`
   ایڈون تیار کیا ہے (مثال کے طور پر ، فوری چیک لینیں جو دوبارہ استعمال کرتے ہیں
   کیشڈ نمونے)۔
6. اپنے صارف سے کوئی اضافی لنٹنگ یا فارمیٹنگ چیک پرت کریں
   جب سخت پالیسیاں درکار ہوں تو `npm run lint:test` کے اوپری حصے پر پروجیکٹ۔
7. جب خدمات میں ترتیب کا اشتراک کرتے ہو تو ، `iroha_config` کو لوڈ کریں اور پاس کریں
   `resolveToriiClientConfig({ config })` پر پارسڈ دستاویز تو نوڈ کلائنٹ
   باقی تعیناتی کی طرح ایک ہی ٹائم آؤٹ/دوبارہ کوشش/ٹوکن پالیسی کو دوبارہ استعمال کریں (دیکھیں
   `docs/source/sdk/js/quickstart.md` ایک مکمل مثال کے لئے)۔

## گٹ ہب ایکشن ٹیمپلیٹ

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

## تیز دھواں کی نوکری (اختیاری)

پل کی درخواستوں کے لئے جو صرف دستاویزات یا ٹائپ اسکرپٹ تعریفوں کو چھوتے ہیں ، a
کم سے کم ملازمت کیچڈ نمونے کو دوبارہ استعمال کرسکتی ہے ، آبائی ماڈیول کی تعمیر نو کر سکتی ہے اور چل سکتی ہے
نوڈ ٹیسٹ رنر براہ راست:

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

یہ کام تیزی سے مکمل ہوجاتا ہے جبکہ ابھی بھی اس بات کی تصدیق کرتا ہے کہ آبائی ایڈون مرتب کرتا ہے
اور یہ کہ نوڈ ٹیسٹ سویٹ گزرتا ہے۔

> ** حوالہ عمل: ** ذخیرہ میں شامل ہیں
> `.github/workflows/javascript-sdk.yml` ، جو اوپر والے اقدامات کو ایک میں تاروں سے بناتا ہے
> کارگو کیچنگ کے ساتھ نوڈ 18/20 میٹرکس۔