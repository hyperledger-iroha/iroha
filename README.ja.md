# Hyperledger Iroha（日本語ガイド）

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha は **分散型台帳技術 (DLT)** を基盤としたシンプルかつ効率的なブロックチェーン台帳です。設計思想は日本の改善活動「改善（Kaizen）」に由来し、無駄（*muri*）のない堅牢な実装を目指しています。

Iroha はアカウントや資産、オンチェーンのデータストアを管理しつつ、高効率なスマートコントラクトを提供します。ビザンチン障害およびクラッシュ障害に対する耐性を備えているため、企業ユースにも適しています。

> _ドキュメントの同期状況:_ 本書は `status.md`（最終更新日: 2026-02-12）の内容に合わせて更新されています。最新のコンポーネント状況や直近のマイルストーンについてはそちらを参照してください。

## 特徴

Iroha はフル機能のブロックチェーン台帳で、以下のことが可能です。

- 決定論的な Norito スキーマを用いてカスタム代替性資産および非代替性資産を作成・管理
- 階層ドメイン、マルチシグ方針、設定可能なメタデータを備えたユーザーアカウント管理
- ワイドオペコード Kotodama コンパイラで Iroha Virtual Machine (IVM) を対象にしたスマートコントラクト、もしくは組み込み Special Instructions の実行
- VRF コミット／リビールのランダムネスと RBC ベースのデータ可用性、ガバナンス連動のスラッシング証跡を備えた NPoS Sumeragi コンセンサスパイプラインの運用
- Confidential Feature Digest、検証鍵レジストリ、決定論的なプルーフタイムアウトによる秘匿トランザクション保護
- テレメトリ、CLI、Norito ツールチェーンを同梱し、許可型／非許可型どちらのネットワーク構成にも対応

Iroha が提供するその他の特長:

- NPoS Sumeragi パイプラインにより最大 33% のビザンチン障害率でも合意を維持
- メモリ常駐のワールドステートと Norito 永続化レイヤにより決定論的に実行
- テレメトリ／エビデンス／ランダムネスの詳細レポートを標準搭載（詳しくは [テレメトリ](#テレメトリ) を参照）
- モジュール化されたアーキテクチャでクレートを明確に分離
- SSE／WebSocket を通じた強い型付けのイベント配信と Norito JSON プロジェクション

## 概要

- [システム要件](#システム要件) と [Iroha のビルド・テスト・実行](#iroha-のビルド・テスト・実行) 手順を確認
- 提供される [クレート群](#統合) を把握
- Iroha の [設定と運用方法](#メンテナンス) を学習
- ネットワーク初期化に用いる [ジェネシス設定](./docs/genesis.md) を確認
- [P2P キューの容量とメトリクス](./docs/source/p2p.md) を理解
- エンドツーエンドの [トランザクション処理パイプライン](./docs/source/pipeline.md) と [new_pipeline.md](./new_pipeline.md) のメトリクスを把握
- [Sumeragi コンセンサスガイド](./docs/source/sumeragi.md) と [ガバナンス API リファレンス](./docs/source/governance_api.md) を参照
- Norito のストリーミングとテレメトリについては [norito_streaming.md](./docs/source/norito_streaming.md) を参照
- [Iroha 関連資料](#参考資料) をさらに読む
  - ZK アプリ API（添付ファイル + プルーフレポート）: `docs/source/zk_app_api.md`
  - CI スモークテスト: `scripts/ci/zk_smoke.sh`（`register-asset` と `shield` のワークフロー）

コミュニティとの交流:
- リポジトリへの [貢献方法](./CONTRIBUTING.md)
- [コンタクト方法](./CONTRIBUTING.md#contact) を通じたサポート

## システム要件

必要な RAM とストレージは、ビルド用途かネットワーク運用用途か、ネットワーク規模、トランザクション量などによって変わります。以下に目安を示します。

| 用途             | CPU                | RAM   | ストレージ[^1] |
|------------------|--------------------|-------|----------------|
| ビルド（最小）   | デュアルコア CPU   | 4GB   | 20GB           |
| ビルド（推奨）   | AMD Ryzen™ 5 1600  | 16GB  | 40GB           |
| 運用（小規模）   | デュアルコア CPU   | 8GB+  | 20GB+          |
| 運用（大規模）   | AMD Epyc™ 64-core  | 128GB | 128GB+         |

[^1]: すべての処理は RAM 上で完結するため、理論上は永続ストレージなしでも動作可能です。ただし、停電後の再同期には時間がかかるため、ストレージの追加を推奨します。

RAM 要件の目安:

- アカウント 1 件あたり平均 5 KiB が必要です。1 000 000 アカウントでは約 5 GiB を消費します。
- `Transfer` や `Mint` 命令は 1 件あたり 1 KiB を消費します。
- すべてのトランザクションをメモリに保持するため、トランザクションスループットや稼働時間に応じて線形にメモリを消費します。

CPU に関する注意:

- Rust のビルドは Apple M1™、AMD Ryzen™/Threadripper™/Epyc™、Intel Alder Lake™ といったマルチコア CPU で特に高い性能を発揮します。
- メモリ制約のある環境でコア数が多い場合、`SIGKILL` によりビルドが失敗することがあります。その際は `cargo build -j <number>` を使用し、`<number>`（山括弧は除く）に「RAM 容量の半分を切り捨てた値」を指定してビルドスレッド数を制限してください。

## Iroha のビルド・テスト・実行

### 前提条件

- [Rust](https://www.rust-lang.org/learn/get-started)（安定版ツールチェーン。プロファイリング用ビルドのみ [CONTRIBUTING.md#profiling](./CONTRIBUTING.md#profiling) に従って nightly が必要）
- （任意）[Docker](https://docs.docker.com/get-docker/)
- （任意）[Docker Compose](https://docs.docker.com/compose/install/)
- （任意）テストで利用する事前ビルド済み IVM バイトコード（旧 `scripts/build_ivm.sh` は削除済み）

### Iroha をビルドする

- ワークスペース全体をビルド:

  ```bash
  cargo build --workspace
  ```

- （任意）インクリメンタルコンパイルを有効化:

  ```bash
  CARGO_INCREMENTAL=1 cargo build
  ```

- （任意）詳細なビルド診断を取得:

  ```bash
  cargo build -vv --timings
  ```

- （任意）最新の Iroha Docker イメージを作成:

  ```bash
  docker build . -t hyperledger/iroha:dev
  ```

  スキップした場合は、利用可能な最新イメージが使用されます。

### コーデックサンプルの再生成

データモデルのスキーマを更新した場合は、Norito コーデックのサンプルを再生成してください。

```bash
cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml
```

生成物は `crates/iroha_kagami/samples/codec/` に出力されます。

### UI テストを手動実行する

UI テストはコンパイル時の診断を検証します（CI ではスキップ）。

```bash
cargo test -p iroha_data_model --test ui
```

### ワークスペース全体をテストする

ユニットテスト／統合テスト／Doc テストを含む全テストは以下で実行できます。

```bash
cargo test --workspace
```

### Iroha を起動する

ビルド後、最小構成のネットワークを起動するには以下を実行します。

```bash
docker compose up
```

起動済みの `docker compose` インスタンスに対して [Iroha Client CLI](crates/iroha_cli/README.md) を使用する例:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml
```

### TUI モニター（iroha_monitor） — attach-first / spawn-lite

ノードの状態や P2P 指標を観察できるシンセウェーブ調のターミナル UI を同梱しています。

- 既存ノードにアタッチする（推奨）:

  ```bash
  # 1 ノード
  cargo run -p iroha_monitor -- --attach http://127.0.0.1:8080 --use-alice

  # 複数ノード
  cargo run -p iroha_monitor -- \
    --attach http://hostA:8080 http://hostB:8080 --use-alice

  # TOML ファイルから読み込む場合
  cargo run -p iroha_monitor -- --attach-config attach.toml
  # 最小構成の attach.toml 例:
  # endpoints = ["http://127.0.0.1:8080"]
  # [account]
  # id = "ed...@wonderland"
  # private_key = "ed25519:..."
  ```

- ローカルにテストネットワークをスポーン:

  ```bash
  cargo run -p iroha_monitor -- --spawn --peers 1
  ```

  `irohad` のビルドに失敗した場合はキャプチャした `cargo` ログを表示します。修正するかアタッチモードを利用してください。

- スポーンライト（ノードなしでスタブを起動）:

  ```bash
  cargo run -p iroha_monitor -- --spawn-lite --peers 2
  ```

  `/status`（JSON）と `/metrics`（Prometheus）を提供する軽量スタブを起動し、ブロックチェーンノードなしで TUI を試せます。

TUI ショートカット: `n/p` または `+/-`（フォーカス移動）、`0..N`（ジャンプ）、`r`（自動ローテーション）、`m`（メトリクス）、`c`（CRT）、`x`（CRT MAX）、`?`（ヘルプ）、`b`（ビープ）、`q`（終了）。`Ctrl+C` も終了します。`--crt` は緑色のブラウン管スタイルを有効化し、`--crt-max` は全体にラスターノイズと星空背景を追加します。ターミナル幅が狭い場合は自動的にコンパクト表示へ切り替わります。

### Torii の JSON API

Torii は署名済み Norito API と利便性の高い JSON エンドポイントを既定で公開します（`iroha_torii` の `app_api` と `transparent_api` フィーチャーが有効）。`transparent_api` によりデータモデルの「可変構造体」機能が転送され、Torii が台帳オブジェクトを追加のラッパー無しで JSON に射影できます。

- `GET /v1/accounts/{accountId}/transactions`、`POST /v1/accounts/{accountId}/transactions/query`
- `GET /v1/accounts/{accountId}/permissions`（アカウント単位で直接付与された権限トークンの一覧）
- `GET /v1/accounts/{accountId}/assets`、`GET /v1/assets/{assetDefinitionId}/holders`
- `GET /v1/accounts` や `GET /v1/assets/definitions` などの一覧系クエリ（`filter`／`sort`／`limit`／`offset` 対応）
- `GET /v1/events/sse` で SSE ストリームを取得（クエリ文字列で Norito 互換フィルタを URL エンコード）
- `POST /v1/contracts/code` でスマートコントラクトコード登録をラップし送信、`GET /v1/contracts/code/{code_hash}` でマニフェスト取得

簡易例:

```bash
TORII=http://127.0.0.1:8080
curl -s "$TORII/v1/parameters" | jq .
curl -N "$TORII/v1/events/sse?filter=%7B%22op%22%3A%22eq%22%2C%22args%22%3A%5B%22tx_status%22%2C%22Approved%22%5D%7D"
```

### 実験的機能: ID のみの投影

リソース一覧で ID のみを返す実験的機能を有効化するには、`ids_projection` フィーチャーでビルドします。

```bash
cargo test -p iroha_core --features ids_projection --no-run
cargo test -p iroha_cli --features "cli_integration_harness ids_projection" --no-run

cargo run --bin iroha --features ids_projection -- \
  domain list all --select ids
```

対応リソースはドメイン、アカウント、資産定義、NFT、ロール、トリガーです。既定では無効であり、仕様は今後変更される可能性があります。

## テレメトリ

Iroha は Prometheus 互換のメトリクスを公開します。P2P やコンセンサスのゲージに加えて、トランザクションパイプラインはステージごとのタイミングやオーバーレイ統計をエクスポートします。公開メトリクスの一覧と推奨クエリは `docs/source/telemetry.md` および `docs/source/telemetry.ja.md` を参照してください。`pipeline.parallel_apply=true` の場合は検証済みブロックごとに以下をカウントします。

- `pipeline_detached_prepared`: デタッチ実行用に準備されたトランザクション
- `pipeline_detached_merged`: ブロックステートへマージに成功したデタッチデルタ
- `pipeline_detached_fallback`: 逐次適用にフォールバックしたトランザクション

これらのゲージはブロックごとにリセットされ、逐次モードでは 0 のままになります。詳細は [new_pipeline.md](./new_pipeline.md) を参照してください。

例:

```bash
TORII=http://127.0.0.1:8080
curl -s "$TORII/v1/accounts/alice@wonderland/assets" | jq .
curl -s "$TORII/v1/assets/rose#wonderland/holders/query" \
  -H 'Content-Type: application/json' \
  -d '{"sort":[{"key":"quantity","order":"desc"}],"pagination":{"limit":10}}' | jq .
```

## 統合

Iroha は複数の Rust クレートによって構成されています。主なクレートと役割は以下の通りです。

- [`iroha`](crates/iroha): ピアと通信するクライアントライブラリ
- [`irohad`](crates/irohad): Iroha ピアをデプロイするコマンドラインアプリケーション
- [`iroha_cli`](crates/iroha_cli): リファレンス CLI（クライアント SDK の使用例）
- [`iroha_core`](crates/iroha_core): ピアのエンドポイントや実行パイプラインを含む中核ライブラリ
- [`iroha_config`](crates/iroha_config): 設定項目とドキュメント生成
- [`iroha_derive`](crates/iroha_derive): `ReadConfig` などのマクロ（`config_base` フィーチャー）
- [`iroha_crypto`](crates/iroha_crypto): 暗号機能全般
- [`iroha_kagami`](crates/iroha_kagami): キー生成、デフォルトジェネシス、設定リファレンス、スキーマ生成
- [`iroha_data_model`](crates/iroha_data_model): 共通データモデル定義
- [`norito`](crates/norito): 決定論的シリアライゼーション。CRC64-XZ の CLMUL/PMULL アクセラレーション、zstd 圧縮、スキーマハッシュ、ストリーミング API を提供
- [`iroha_futures`](crates/iroha_futures): 非同期処理ユーティリティ
- [`iroha_logger`](crates/iroha_logger): `tracing` ベースのロギング
- [`iroha_macro`](crates/iroha_macro): 便利なマクロ群
- [`iroha_p2p`](crates/iroha_p2p): ピア作成とハンドシェイクロジック
- [`ivm`](crates/ivm): Iroha Virtual Machine の実装
  - Kotodama スマートコントラクトは IVM バイトコード (`.to`) へコンパイルされ、スタンドアロンの RISC-V ISA を対象としません。RISC-V 風のエンコーディングは IVM 命令フォーマットの内部実装です。
  - 詳細は `ivm.md`、`docs/source/ivm_syscalls.md`、`docs/source/kotodama_grammar.md` を参照
- [`iroha_telemetry`](crates/iroha_telemetry): テレメトリ収集と解析
- [`iroha_version`](crates/iroha_version): ノードの段階的アップグレード向けバージョン管理

例:
- Kotodama + IVM の実行例は `examples/` にあります。外部ツールが利用可能な場合は `make examples-run`（`KOTO` や `IVM` のパスを上書き可）を実行するか、`examples/README.md` を参照してください。

## トランザクションパイプライン

Iroha 2 ネットワークにおけるトランザクションの流れ:

1. **構築と署名** — クライアント（SDK または `iroha_cli`）がトランザクションを組み立て、メタデータを設定し、アカウントのクォーラムに応じて署名します。
2. **Torii へ送信** — 署名済みトランザクションをピアへ送信し、Norito スキーマをデシリアライズして軽量チェックを行います。
3. **アドミッションとステートレス検証** — 署名・サイズ・命令数・TTL/nonce などを検証し、合格したトランザクションをインメモリキューへ追加します。
4. **提案と順序付け（Sumeragi）** — リーダーがキューから提案を組み立て再検証し、委員会へブロードキャストします。
5. **コンセンサス** — ピアが提案を検証して投票し、署名を集約します。コミットで順序が確定します。
6. **実行と状態遷移** — 各ピアが順序付けられた命令を World State View に適用し、権限を検査しつつ IVM コントラクトや ISI を決定論的に実行します。イベントが配信されます。
7. **ブロックコミット（Kura）** — ヘッダと署名を含むブロックを永続化し、遅延ピアへゴシップします。
8. **可観測性** — テレメトリと WebSocket が `blocks`、`commit_time_ms`、`queue_size` などのメトリクスとストリームを公開します。

高速化は SIMD/GPU フィーチャーフラグで制御され、CPU フォールバックと同一の結果を保証します。Sumeragi のタイミングは `SumeragiParameter::{BlockTimeMs,CommitTimeMs}` で調整できます。

## メンテナンス

Iroha の設定と運用に関する概要です。

- [設定](#設定)
- [エンドポイント](#エンドポイント)
- [ログ](#ログ)
- [モニタリング](#モニタリング)
- [ストレージ](#ストレージ)
- [スケーラビリティ](#スケーラビリティ)

### 設定

設定パラメータは TOML ファイル経由で渡すのが推奨です。

```bash
irohad --config /path/to/config.toml
```

Sumeragi（コンセンサス）設定の概要は `docs/source/sumeragi.md` を参照してください。`[sumeragi]` セクション例:

```toml
[sumeragi]
role = "validator"
allow_view0_slack = false
collectors_k = 3
collectors_redundant_send_r = 3
msg_channel_cap_votes = 8192
msg_channel_cap_block_payload = 128
msg_channel_cap_rbc_chunks = 1024
msg_channel_cap_blocks = 256
control_msg_channel_cap = 1024
```

テンプレートは `docs/source/references/peer.template.toml` にあります。詳細な設定リファレンスは現在も更新中です。

### エンドポイント

エンドポイントと操作一覧、パラメータの詳細は [Iroha Docs > Torii Endpoints](https://docs.iroha.tech/reference/torii-endpoints.html) を参照してください。

### ログ

既定では人が読みやすい形式で `stdout` にログを出力します。ログレベルは設定ファイルの `logger.level` または `/configuration` エンドポイント経由で動的に変更できます。

```bash
curl -X POST \
  -H 'Content-Type: application/json' \
  http://127.0.0.1:8080/configuration \
  -d '{"logger":{"level":"DEBUG"}}' -i
```

ログフォーマットは `logger.format` で `full`（既定）／`compact`／`pretty`／`json` を指定できます。ファイル出力やローテーションは運用者が管理してください。

### モニタリング

`/status` は JSON、`/metrics` は Prometheus 互換フォーマットを返します。Prometheus を使用する場合は [公式ガイド](https://prometheus.io/docs/introduction/first_steps/) と `docs/source/references/prometheus.template.yml` を参照してください。

Sumeragi が公開する主なメトリクス:

- `sumeragi_collectors_k`／`sumeragi_redundant_send_r`: マルチコレクタ構成（K と r）
- `sumeragi_redundant_sends_total`: タイムアウト駆動の冗長送信
- `sumeragi_redundant_sends_by_collector{idx}`／`..._by_peer{peer}`／`..._by_role_idx{role_idx}`: 冗長送信の詳細内訳
- `sumeragi_collectors_targeted_current`: 現在の投票ブロックでターゲットにしたコレクタ数
- `sumeragi_collectors_targeted_per_block`: コミットブロックごとのコレクタ数（ヒストグラム）
- `sumeragi_cert_size`: 最終化証明書の署名数（ヒストグラム）
- `sumeragi_dropped_block_messages_total`／`sumeragi_dropped_control_messages_total`: チャネル満杯によるドロップ

Prometheus クエリ例:

- `sum by (le) (rate(sumeragi_collectors_targeted_per_block_bucket[5m]))`
- `histogram_quantile(0.95, sum by (le) (rate(sumeragi_collectors_targeted_per_block_bucket[5m])))`
- `rate(sumeragi_redundant_sends_total[5m])`
- `topk(5, sum by (peer) (rate(sumeragi_redundant_sends_by_peer[5m])))`
- `rate(sumeragi_dropped_block_messages_total[5m])`

### ストレージ

Iroha はブロックとスナップショットを `storage` ディレクトリに保存します。`kura.block_store_path` を設定すると配置場所を上書きできます（設定ファイルの位置からの相対パス）。

### スケーラビリティ

同一マシン・同一ディレクトリで複数の Iroha ピアやクライアントバイナリを動かすことは可能です。ただし、各インスタンスにはクリーンな作業ディレクトリを割り当てることを推奨します。`docker-compose` による最小ネットワーク例も提供されています。

## 参考資料

- [Iroha 2 ドキュメント](https://docs.iroha.tech)
  - [用語集](https://docs.iroha.tech/reference/glossary.html)
  - [Iroha Special Instructions](https://docs.iroha.tech/blockchain/instructions.html)
  - [Torii API リファレンス](https://docs.iroha.tech/reference/torii-endpoints.html)
- [Iroha 2 ホワイトペーパー](./docs/source/iroha_2_whitepaper.md)
- [実装に基づくデータモデルと ISI 仕様](./docs/source/data_model_and_isi_spec.md)
- [エラーマッピングガイド](./docs/source/error_mapping.md)

SDK リンク:

- [Iroha Python](https://github.com/hyperledger-iroha/iroha-python)
- [Iroha Java](https://github.com/hyperledger-iroha/iroha-java)
- [Iroha Javascript/TypeScript](https://github.com/hyperledger-iroha/iroha-javascript)
- Iroha Swift/iOS — [`docs/README.md#swift--ios-sdk-references`](./docs/README.md#swift--ios-sdk-references)

## コントリビュート方法

バグ報告や改善提案は GitHub の Issue / Pull Request で受け付けています。詳しくは [CONTRIBUTING.md](./CONTRIBUTING.md) を参照してください。

## ヘルプを得るには

コミュニティの利用チャネルについては [CONTRIBUTING.md#contact](./CONTRIBUTING.md#contact) を参照してください。

## ライセンス

Iroha のソースコードは Apache License, Version 2.0 の下で提供されます。詳細は [LICENSE](./LICENSE) を参照してください。

ドキュメントは Creative Commons Attribution 4.0 International (CC-BY-4.0) で提供されます。詳細は http://creativecommons.org/licenses/by/4.0/ を参照してください。

---

日本語版は原文の主要項目を翻訳・要約したものであり、追加更新は随時反映します。英語版と整合しない箇所を見つけた場合は Issue や Pull Request でお知らせください。
