---
lang: zh-hans
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

# Iroha JS CI 参考

`@iroha/iroha-js` 软件包通过 `iroha_js_host` 捆绑本机绑定。任意
执行测试或构建的 CI 管道必须提供 Node.js 运行时
和 Rust 工具链，以便可以在测试之前编译本机包
跑。

## 推荐步骤

1. 通过 `actions/setup-node` 或您的 CI 使用 Node LTS 版本（18 或 20）
   等价。
2. 安装 `rust-toolchain.toml` 中列出的 Rust 工具链。我们推荐
   GitHub 操作中的 `dtolnay/rust-toolchain@v1`。
3.缓存cargoregistry/git索引和`target/`目录以避免
   在每项工作中重建本机插件。
4. 运行 `npm install`，然后运行 ​​`npm run lint:test`。组合脚本强制执行
   零警告的 ESLint，构建本机插件，并运行 Node 测试
   套件，以便 CI 与发布门控工作流程相匹配。
5. 可以选择将 `node --test` 作为快速烟雾步骤运行一次 `npm run build:native`
   已生成插件（例如，预提交快速检查通道可重复使用）
   缓存的工件）。
6. 对消费者进行的任何额外的 linting 或格式检查进行分层
   当需要更严格的政策时，项目位于 `npm run lint:test` 之上。
7、跨服务共享配置时，加载`iroha_config`并传递
   将文档解析为 `resolveToriiClientConfig({ config })` 所以 Node 客户端
   重用与部署的其余部分相同的超时/重试/令牌策略（请参阅
   `docs/source/sdk/js/quickstart.md` 为完整示例）。

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

## 快速烟雾作业（可选）

对于仅涉及文档或 TypeScript 定义的拉取请求，
最小作业可以重用缓存的工件，重建本机模块，并运行
直接节点测试运行器：

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

这项工作很快完成，同时仍然验证本机插件是否编译
并且 Node 测试套件通过了。

> **参考实现：** 存储库包括
> `.github/workflows/javascript-sdk.yml`，将上述步骤连接到
> 具有货物缓存的节点 18/20 矩阵。