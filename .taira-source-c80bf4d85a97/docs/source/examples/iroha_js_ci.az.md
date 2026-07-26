---
lang: az
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

# Iroha JS CI Referansı

`@iroha/iroha-js` paketi `iroha_js_host` vasitəsilə yerli bağlamaları birləşdirir. İstənilən
Testləri və ya qurmaları həyata keçirən CI kəməri həm Node.js icra müddətini təmin etməlidir
və Rust alətlər silsiləsi belə ki, yerli paket sınaqlardan əvvəl tərtib oluna bilsin
qaçmaq.

## Tövsiyə olunan addımlar

1. `actions/setup-node` və ya CI vasitəsilə Node LTS buraxılışını (18 və ya 20) istifadə edin
   ekvivalent.
2. `rust-toolchain.toml`-də sadalanan Rust alətlər zəncirini quraşdırın. Biz tövsiyə edirik
   GitHub Fəaliyyətlərində `dtolnay/rust-toolchain@v1`.
3. Qarşısının alınması üçün yük reyestrini/git indekslərini və `target/` kataloqunu keş edin
   hər işdə yerli addonu yenidən qurmaq.
4. `npm install`, sonra `npm run lint:test`-i işə salın. Birləşdirilmiş skript tətbiq edir
   Sıfır xəbərdarlıqlarla ESLint, yerli addon qurur və Node testini həyata keçirir
   paketi beləliklə CI buraxılış qapısı iş axınına uyğun gəlir.
5. İstəyə görə `node --test`-i bir dəfə sürətli tüstü addımı kimi işə salın `npm run build:native`
   addon istehsal etdi (məsələn, təkrar istifadə edən sürətli yoxlama zolaqlarını əvvəlcədən təqdim edin
   keşlənmiş artefaktlar).
6. İstehlakçınızdan hər hansı əlavə linting və ya format çeklərini qatlayın
   daha sərt siyasətlər tələb olunduqda `npm run lint:test` üzərində layihə.
7. Konfiqurasiyanı xidmətlər arasında paylaşarkən, `iroha_config` yükləyin və
   Sənədi `resolveToriiClientConfig({ config })`-ə təhlil etdi, beləliklə Node müştəriləri
   yerləşdirmənin qalan hissəsi ilə eyni vaxt aşımı/yenidən cəhd/token siyasətini yenidən istifadə edin (bax
   Tam nümunə üçün `docs/source/sdk/js/quickstart.md`).

## GitHub Fəaliyyət Şablonu

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

## Sürətli Siqaret İşi (Könüllü)

Yalnız sənədlərə və ya TypeScript təriflərinə toxunan çəkmə sorğuları üçün a
minimal iş keşlənmiş artefaktları təkrar istifadə edə, yerli modulu yenidən qura və işlədə bilər
Birbaşa node test qaçışı:

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

Doğma addonun tərtib etdiyini yoxlayarkən bu iş tez tamamlanır
və Node test paketinin keçdiyini.

> **İstinadın həyata keçirilməsi:** repozitoriya daxildir
> `.github/workflows/javascript-sdk.yml`, yuxarıdakı addımları a
> Yük keşləməsi ilə qovşaq 18/20 matrisi.