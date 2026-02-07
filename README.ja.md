# Hyperledger Iroha（日本語ガイド）

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha は、許可型ネットワークおよびコンソーシアム向けの決定論的ブロックチェーン基盤です。アカウント管理、資産管理、オンチェーン権限管理、および Iroha Virtual Machine (IVM) を通じたスマートコントラクト実行を提供します。

> ワークスペースの状態と最近の変更は [`status.md`](./status.md) に記録されています。

## リリースライン

このリポジトリは、同一コードベースから 2 つの展開ラインを提供します。

- **Iroha 2**: 自己運用の許可型／コンソーシアムネットワーク向け。
- **Iroha 3 (SORA Nexus)**: 同じコアクレートを利用する Nexus 指向のライン。

どちらのラインも、Norito シリアライゼーション、Sumeragi コンセンサス、Kotodama -> IVM ツールチェーンなどのコアコンポーネントを共有しています。

## リポジトリ構成

- [`crates/`](./crates): 主要 Rust クレート（`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `ivm`, `norito` など）。
- [`integration_tests/`](./integration_tests): コンポーネント横断の統合／ネットワークテスト。
- [`IrohaSwift/`](./IrohaSwift): Swift SDK パッケージ。
- [`java/iroha_android/`](./java/iroha_android): Android SDK パッケージ。
- [`docs/`](./docs): ユーザー／運用／開発者向けドキュメント。

## クイックスタート

### 前提条件

- [Rust stable](https://www.rust-lang.org/tools/install)
- 任意: ローカル複数 peer 実行向け Docker + Docker Compose

### ビルドとテスト（workspace）

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

補足:

- ワークスペース全体のビルドには約 20 分かかる場合があります。
- ワークスペース全体のテストには数時間かかる場合があります。
- このワークスペースは `std` を対象としています（WASM/no-std ビルドは非対応）。

### クレート単位のテスト

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK テストコマンド

```bash
cd IrohaSwift
swift test
```

```bash
cd java/iroha_android
JAVA_HOME=$(/usr/libexec/java_home -v 21) \
ANDROID_HOME=~/Library/Android/sdk \
ANDROID_SDK_ROOT=~/Library/Android/sdk \
./gradlew test
```

## ローカルネットワークの起動

同梱の Docker Compose ネットワークを起動します。

```bash
docker compose -f defaults/docker-compose.yml up
```

デフォルトクライアント設定で CLI を使います。

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

デーモンのネイティブ展開手順は [`crates/irohad/README.md`](./crates/irohad/README.md) を参照してください。

## API と可観測性

Torii は Norito API と JSON API の両方を公開します。代表的な運用エンドポイント:

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

エンドポイントの完全なリファレンス:

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## 主要クレート

- [`crates/iroha`](./crates/iroha): クライアントライブラリ。
- [`crates/irohad`](./crates/irohad): peer デーモンバイナリ。
- [`crates/iroha_cli`](./crates/iroha_cli): リファレンス CLI。
- [`crates/iroha_core`](./crates/iroha_core): 台帳コアと実行エンジン。
- [`crates/iroha_config`](./crates/iroha_config): 型付き設定モデル。
- [`crates/iroha_data_model`](./crates/iroha_data_model): 正準データモデル。
- [`crates/iroha_crypto`](./crates/iroha_crypto): 暗号プリミティブ。
- [`crates/norito`](./crates/norito): 決定論的シリアライゼーションコーデック。
- [`crates/ivm`](./crates/ivm): Iroha Virtual Machine。
- [`crates/iroha_kagami`](./crates/iroha_kagami): 鍵／genesis／設定ツール。

## ドキュメントマップ

- メインインデックス: [`docs/README.md`](./docs/README.md)
- Genesis: [`docs/genesis.md`](./docs/genesis.md)
- コンセンサス (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- トランザクションパイプライン: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P 内部仕様: [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM syscalls: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama 文法: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito ワイヤ形式: [`norito.md`](./norito.md)
- 現在の進捗管理: [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## 翻訳

日本語版: [`README.ja.md`](./README.ja.md)

他言語版:
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

翻訳ワークフロー: [`docs/i18n/README.md`](./docs/i18n/README.md)

## コントリビュートと問い合わせ

- コントリビュートガイド: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- コミュニティ／サポート窓口: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## ライセンス

Iroha は Apache-2.0 ライセンスです。[`LICENSE`](./LICENSE) を参照してください。

ドキュメントは CC-BY-4.0 ライセンスです: http://creativecommons.org/licenses/by/4.0/
