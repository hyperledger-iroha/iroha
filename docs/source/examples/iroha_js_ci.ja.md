<!-- Japanese translation of docs/source/examples/iroha_js_ci.md -->

---
lang: ja
direction: ltr
source: docs/source/examples/iroha_js_ci.md
status: complete
translator: manual
---

<!--
  SPDX-License-Identifier: Apache-2.0
  このドキュメントは CI 環境で @iroha/iroha-js を実行するためのガイドです。
-->

# Iroha JS CI リファレンス

`@iroha/iroha-js` パッケージは `iroha_js_host` 経由でネイティブバインディングを同梱しています。テストやビルドを実行する CI パイプラインは、テストが走る前にネイティブモジュールをコンパイルできるよう、Node.js ランタイムと Rust ツールチェーンの両方を用意する必要があります。

## 推奨ステップ

1. Node LTS (18 または 20) を `actions/setup-node` などでセットアップします。
2. `rust-toolchain.toml` に記載された Rust ツールチェーンをインストールします。GitHub Actions では `dtolnay/rust-toolchain@v1` を推奨します。
3. ネイティブアドオンの再ビルドを避けるため、cargo の registry/git と `target/` ディレクトリをキャッシュします。
4. `npm install` → `npm run build:native` → `npm test` の順に実行します。
5. フル `npm` パイプラインが不要な場合は、`node --test` を高速なスモークテストとして利用できます（例: ドキュメント変更のみの PR）。
6. 必要に応じて `npx eslint` やフォーマッタのチェックを追加できます。SDK 自体は特定のスタイルを強制しません。

## GitHub Actions テンプレート

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
      - run: npm run build:native
      - run: npm test
```

## 高速スモークジョブ（オプション）

ドキュメントや型定義のみを変更する Pull Request 向けに、キャッシュ済みアーティファクトを再利用しつつネイティブモジュールを再ビルドし、Node のテストランナーだけを実行する軽量ジョブを用意できます。

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

このジョブは短時間で完了しつつ、ネイティブアドオンがコンパイルされることと Node のテストスイートが成功することを確認できます。
