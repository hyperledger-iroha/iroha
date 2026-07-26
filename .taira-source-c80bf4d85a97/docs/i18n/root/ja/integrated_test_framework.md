<!-- Japanese translation of integrated_test_framework.md -->

---
lang: ja
direction: ltr
source: integrated_test_framework.md
status: complete
translator: manual
---

# Hyperledger Iroha 統合テストフレームワーク（7 ノード構成）

## はじめに

Hyperledger Iroha v2 には、ドメイン・アセット・権限・ピア管理のための豊富な Iroha Special Instruction (ISI) が用意されています。本ドキュメントでは、7 ピア構成のネットワークにおいて、通常時および障害時（最大 2 ピアの故障を許容）における正しさ・合意・ノード間一貫性を検証する統合テストフレームワークを定義します。

内容は、マルチトランザクションブロックへ移行した最新のジェネシススキーマに対応しています。

API について：このコードベースでは Torii は HTTP/WebSocket (Axum) で提供されます。テストは HTTP エンドポイント（通常 8080 番台のポート）を利用してください。ポート 50051 で待ち受ける追加の RPC サービスは存在しません。

## 目的

- 自動化された 7 ピアオーケストレーション：高速な CI を目的にプログラムからピアを起動・管理できる（Docker Compose による起動は任意）。
- ジェネシス設定：決定論的なアカウント／鍵と必要な権限を備えた共通の Norito ジェネシスから開始する。
- ISI の網羅：トランザクションを通じて ISI を体系的に実行し、結果の状態を検証する。
- ピア間一貫性：ステップごとに複数ピアへ問い合わせ、台帳状態が一致していることを確認する。
- フォールトトレランス検証：最大 2 ピアが停止または分断されても残りのピアが処理を継続し、復帰したピアが矛盾なく追いつけることを保証する。

## オーケストレーションモード

- Rust ハーネス（推奨）：`crates/iroha_test_network` がローカルに `irohad` プロセスを起動し、ポート確保・設定ファイル／Norito `.nrt` ジェネシス作成・起動監視・ブロック高監視・シャットダウン／再起動ユーティリティを提供します。
- Docker Compose（任意）：`crates/iroha_swarm` が N ピア分の Compose ファイルを生成します。コンテナ化ネットワークや外部オーケストレーションを利用したい場合に選択します。

## テストネットワークのセットアップ

**ジェネシスブロック**

- ソース：`defaults/genesis.json`。命令は `transactions` 配列にまとめられています。テストでは `NetworkBuilder::with_genesis_instruction` で命令を追加し、`.next_genesis_transaction()` で新しいトランザクションを開始できます。最終的なブロックは Norito `.nrt` としてシリアライズされます。
- トポロジ：1 件目のトランザクション（`transactions[0].topology`）に 7 ピアすべて（公開鍵とアドレス）が格納され、全ピアが起動時からネットワークを認識します。
- アカウント／権限：`crates/iroha_test_samples` の標準アカウント（`ALICE_ID`、`BOB_ID`、`SAMPLE_GENESIS_ACCOUNT_KEYPAIR` など）と必要な権限（例：`CanManagePeers`、`CanManageRoles`、`CanMintAssetWithDefinition`）を利用してください。
- 投入手順：ハーネスの場合、通常はジェネシスを 1 ピア（“genesis submitter”）へ投げ込み、他ピアはブロック同期で追従します。Compose 実行時は全ピアに同じジェネシスパスを指定します。

**ネットワークとポート**

- Torii HTTP API：`API_ADDRESS`（Axum）。Compose では 7 ピア分のポート `8080`〜`8086` をホストへ割り当てます。ハーネスはループバックポートを自動割り当てします。
- P2P：ピア間通信アドレスは `trusted_peers` に記載され、ゴシップで共有されます。ハーネスは実行ごとに `trusted_peers` を設定します。

**データストレージ**

- Kura：Iroha v2 のブロックストレージ（外部データベースは不要）。`[kura]`（例：`store_dir`）で設定します。テストでは `[snapshot]` でスナップショットを無効にしてください。

**ハーネスのフロー**

1. ネットワーク構築：`NetworkBuilder::new().with_peers(7)`。必要に応じて `.with_pipeline_time(...)` や `.with_config_layer(...)` で設定を上書きし、`IvmFuelConfig` で IVM 燃料を選択します。
2. ピア起動：`.start()` / `.start_blocking()`。設定レイヤーを出力し、`trusted_peers` を設定し、1 ピアにジェネシスを投入してから起動完了を待ちます。
3. 起動確認：`Network::ensure_blocks(height)` や `once_blocks_sync(...)` で想定ブロック高に達したことを確認します。`Client::get_status()` をポーリングする方法もあります。

**Docker Compose のフロー（任意）**

1. Compose 生成：`iroha_swarm::Swarm` で N ピア分の Compose ファイルを出力し、API ポートをホストへマッピングします。環境変数（CHAIN、鍵、TRUSTED_PEERS、GENESIS）を指定します。
2. 起動：`docker compose up`
3. 起動確認：Torii HTTP エンドポイントをポーリングし、ヘルシーかつブロック高 ≥ 1 になったか監視します。

## テストハーネスの実装（Rust）

**クライアントとトランザクション**

- `iroha::client::Client`（HTTP/WebSocket）を用いて Torii へトランザクション・クエリを送信します。
- ISI の `InstructionBox` 列や IVM バイトコードからトランザクションを構築し、`iroha_test_samples` の決定論的な `KeyPair` で署名してください。
- 便利な API：`submit_blocking`、`submit_all_blocking`、`query(...).execute()/execute_all()`、`get_status()`、ブロック／イベントストリーム（WebSocket）。

**ピア間一貫性**

- 操作の都度、稼働中の各ピアに問い合わせ（`Network::peers()` → `peer.client()`）を行い、残高・定義・ピア一覧などが一致しているか比較します。単一ピアのみを確認するより強固な検証が可能です。

**フォールトインジェクション（コンテナ未使用時）**

- `integration_tests/tests/extra_functional/unstable_network.rs` のリレー／プロキシユーティリティを利用し、`trusted_peers` を TCP プロキシへ書き換えて特定のリンクを一時停止できます。これにより分断・ドロップ・再接続といったシナリオを再現できます。

**IVM の前提条件**

- 一部テストは事前にビルドした IVM サンプルを要求します。`crates/ivm/target/prebuilt/build_config.toml` が存在することを確認してください。未整備の場合、現行の統合テストはスキップ処理を行います。

## 7 ノードシナリオの概要

1. ハーネスまたは Compose で 7 ピアネットワークを起動し、全ピアでジェネシスがコミットされるまで待機します。
2. 以下を含む ISI スイートを実行します：
   - ドメイン／アカウント／アセットの登録、権限の付与／剥奪、アセットのミント／バーン／転送、Key-Value の設定／削除、トリガーの登録、エグゼキュータのアップグレード。
3. 各ステップ後に全ピアへクエリを送り、状態が一致していることを検証します。
4. 1〜2 ピアを停止し、残り 5 ピアでトランザクションを送信し続けます。稼働ピア間で進行と一貫性が保たれていることを確認します。
5. 停止したピアを再起動し、追いつきとピア間の整合性を再確認します。

## CI での利用

- ビルド：`cargo build --workspace`
- 必要であれば IVM サンプルを事前ビルド
- テスト：`cargo test --workspace`
- 厳格な lint（任意）：`cargo clippy --workspace --all-targets -- -D warnings`

## まとめ

本フレームワークは、リポジトリ内の Rust ハーネス（`iroha_test_network`）と HTTP ベースのクライアント（`iroha::client::Client`）を活用し、7 ピア Iroha v2 ネットワークを検証します。ピア間一貫性、現実的な障害シナリオ、再現性の高いセットアップ／クリーンアップを重視し、CI に組み込みやすい構成となっています。コンテナ利用が望ましい場合は `iroha_swarm` による Docker Compose が選択可能です。

## エクスプローラ

- Torii HTTP/WebSocket に対応したブロックチェーンエクスプローラであれば、各ピアへ直接接続できます。各ピアは Torii エンドポイント（ホスト:ポート）を公開し、ステータス・ブロック・クエリ・イベントの取得が可能です。
- Rust ハーネス使用時：ネットワーク構築後に以下の要領で全ピアの URL を取得できます。

  ```rust
  use iroha_test_network::NetworkBuilder;

  let network = NetworkBuilder::new().with_peers(7).build();
  let urls = network.torii_urls();
  // 例: ["http://127.0.0.1:8080", ..., "http://127.0.0.1:8086"]
  ```

  `peer.api_address()` や `peer.torii_url()` などのヘルパーも利用できます。

- Docker Compose（`iroha_swarm`）使用時：7 ピア分の Compose を生成し、ホストに `8080`〜`8086` をマッピングします。エクスプローラからそれぞれのアドレスへ接続してください。複数エンドポイントをサポートするエクスプローラであれば全 7 ピアを設定し、そうでなければピアごとに個別のエクスプローラインスタンスを動かします。
