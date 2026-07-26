---
lang: zh-hant
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

# Iroha JS CI 參考

`@iroha/iroha-js` 軟件包通過 `iroha_js_host` 捆綁本機綁定。任意
執行測試或構建的 CI 管道必須提供 Node.js 運行時
和 Rust 工具鏈，以便可以在測試之前編譯本機包
跑。

## 推薦步驟

1. 通過 `actions/setup-node` 或您的 CI 使用 Node LTS 版本（18 或 20）
   等價。
2. 安裝 `rust-toolchain.toml` 中列出的 Rust 工具鏈。我們推薦
   GitHub 操作中的 `dtolnay/rust-toolchain@v1`。
3.緩存cargoregistry/git索引和`target/`目錄以避免
   在每項工作中重建本機插件。
4. 運行 `npm install`，然後運行 ​​`npm run lint:test`。組合腳本強制執行
   零警告的 ESLint，構建本機插件，並運行 Node 測試
   套件，以便 CI 與發布門控工作流程相匹配。
5. 可以選擇將 `node --test` 作為快速煙霧步驟運行一次 `npm run build:native`
   已生成插件（例如，預提交快速檢查通道可重複使用）
   緩存的工件）。
6. 對消費者進行的任何額外的 linting 或格式檢查進行分層
   當需要更嚴格的政策時，項目位於 `npm run lint:test` 之上。
7、跨服務共享配置時，加載`iroha_config`並傳遞
   將文檔解析為 `resolveToriiClientConfig({ config })` 所以 Node 客戶端
   重用與部署的其餘部分相同的超時/重試/令牌策略（請參閱
   `docs/source/sdk/js/quickstart.md` 為完整示例）。

## GitHub 操作模板

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

## 快速煙霧作業（可選）

對於僅涉及文檔或 TypeScript 定義的拉取請求，
最小作業可以重用緩存的工件，重建本機模塊，並運行
直接節點測試運行器：

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

這項工作很快完成，同時仍然驗證本機插件是否編譯
並且 Node 測試套件通過了。

> **參考實現：** 存儲庫包括
> `.github/workflows/javascript-sdk.yml`，將上述步驟連接到
> 具有貨物緩存的節點 18/20 矩陣。