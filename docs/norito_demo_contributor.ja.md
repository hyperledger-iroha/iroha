<!-- Japanese translation of docs/norito_demo_contributor.md -->

---
lang: ja
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
translator: manual
---

# Norito SwiftUI デモ コントリビューターガイド

このドキュメントは、ローカルの Torii ノードとモック台帳に対して SwiftUI デモを実行するための手動セットアップ手順をまとめたものです。日々の開発作業に焦点を置き、`docs/norito_bridge_release.md` を補完します。アプリへの組み込みや Connect/ChaChaPoly の実装手順を詳しく知りたい場合は `docs/connect_swift_integration.md` も参照してください。

## 環境構築

1. `rust-toolchain.toml` に記載された Rust ツールチェーンをインストールする。
2. macOS 上で Swift 5.7 以上と Xcode コマンドラインツールをインストールする。
3. （任意）Lint 用に [SwiftLint](https://github.com/realm/SwiftLint) を導入する。
4. `cargo build -p irohad` を実行し、ノードがホスト環境でビルドできることを確認する。
5. `examples/ios/NoritoDemoXcode/Configs/demo.env.example` を `.env` としてコピーし、自分の環境に合わせて値を調整する。アプリは起動時に次の環境変数を読み込みます。
   - `TORII_NODE_URL` — REST のベース URL（WebSocket はここから導出されます）。
   - `CONNECT_SESSION_ID` — 32 バイトのセッション ID（base64 / base64url）。
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — `/v1/connect/session` が返す各ロール用トークン。
   - `CONNECT_CHAIN_ID` — コントロールハンドシェイクで通知するチェーン ID。
   - `CONNECT_ROLE` — UI で事前選択されるロール（`app` または `wallet`）。
   - 任意のテスト補助: `CONNECT_PEER_PUB_B64`, `CONNECT_SHARED_KEY_B64`,
     `CONNECT_APPROVE_ACCOUNT_ID`, `CONNECT_APPROVE_PRIVATE_KEY_B64`,
     `CONNECT_APPROVE_SIGNATURE_B64`。
   - `NORITO_ACCEL_CONFIG_PATH` — （任意）Metal/NEON のしきい値を含む `iroha_config`
     JSON/TOML ファイルへのパス。未指定の場合はバンドル済みファイルまたはデフォルトが使われます。

## Torii とモック台帳の起動

リポジトリには、デモ用アカウントがあらかじめロードされたインメモリ台帳と Torii ノードを起動するヘルパースクリプトが付属しています。

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

スクリプトの出力:

- Torii ノードログ: `artifacts/torii.log`
- レジャーのメトリクス（Prometheus フォーマット）: `artifacts/metrics.prom`
- クライアントアクセストークン: `artifacts/torii.jwt`

`start.sh` は `Ctrl+C` を押すまでデモ用ピアを起動し続けます。実行完了後は、基準データとなる `artifacts/ios_demo_state.json`（他成果物のソース）を保存し、Torii の標準出力ログをコピーし、`/metrics` がスクレイプ可能になるまでポーリングし、設定されたアカウント情報から `torii.jwt` を生成します（設定に秘密鍵が含まれていれば出力に含まれます）。出力ディレクトリを変更する `--artifacts`、カスタム Torii 設定に合わせる `--telemetry-profile`、非対話的な CI ジョブ向けの `--exit-after-ready` などのオプションがあります。

`SampleAccounts.json` 内の各エントリでは以下のフィールドを指定できます:

- `name`（文字列, 任意） — アカウントメタデータ `alias` として保存。
- `public_key`（マルチハッシュ文字列, 必須） — アカウントの署名鍵。
- `private_key`（任意） — クライアント用 JWT を生成する際に `torii.jwt` に含める。
- `domain`（任意） — 省略した場合は資産ドメインを使用。
- `asset_id`（文字列, 必須） — アカウントにミントする資産定義。
- `initial_balance`（文字列, 必須） — ミントする数値量。

## SwiftUI デモの実行

1. `docs/norito_bridge_release.md` の手順に従って XCFramework をビルドする。
2. Xcode で `NoritoDemoXcode` プロジェクトを開く。
3. `NoritoDemo` スキームを選択し、iOS シミュレータをターゲットに設定する。
4. `.env` ファイルをスキームの環境変数経由で参照させ、`/v1/connect/session` から得た `CONNECT_*` 値を設定しておくと、起動時に UI が自動入力されます。
5. アプリを実行する。Torii URL が未設定の場合はホーム画面で入力を求められる。
6. 「Connect」セッションを開始し、アカウント更新のサブスクリプションを開始する。
7. IRH トランスファーを送信し、画面上のログ出力を確認する。

## ハードウェアアクセラレーション

`NoritoDemoXcode/App.swift` では `DemoAccelerationConfig.load().apply()` を呼び出し、Metal/NEON
設定を次の順序で解決します。

1. `NORITO_ACCEL_CONFIG_PATH` — `.env` で指定する `iroha_config` JSON/TOML ファイルへのパス。
2. アプリバンドル内の `acceleration.{json,toml}` または `client.{json,toml}`。
3. いずれも見つからない場合は `AccelerationSettings()` で定義されたデフォルト。

`acceleration.toml` の例:

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

設定が存在しない場合や `[accel]` セクションが欠落している場合は安全に CPU パスへフォールバックします。
Metal をサポートしないシミュレータでは、設定に関わらずスカラー実装が使用されます。

## 統合テスト

- 統合テストは `Tests/NoritoDemoTests` に配置予定（macOS CI の対応後に追加）。
- テストは前述のスクリプトで Torii を起動し、Swift パッケージ経由で WebSocket の購読・残高確認・送金フローを検証します。
- テスト実行時のログは `artifacts/tests/<timestamp>/` に、メトリクスや台帳ダンプと合わせて保存されます。

## CI パリティチェック

- デモやフィクスチャを変更する前に `make swift-ci` を実行してください。フィクスチャ差分確認だけでなく、ダッシュボード用 JSON の検証と CLI レンダリングも行われます。CI では Buildkite メタデータ
  (`ci/xcframework-smoke:<lane>:device_tag`) に依存しており、`iphone-sim` や `strongbox` などのレーン特定に利用されます。パイプラインやエージェントタグを変更した場合は、メタデータが出力されていることを確認してください。
- `make swift-ci` が失敗した場合は `docs/source/swift_parity_triage.md` を参照し、レンダリングされた `mobile_ci` 出力を確認して、どのレーンに再生成やインシデント対応が必要か判断します。

## トラブルシューティング

- デモが Torii に接続できない場合は、ノード URL と TLS 設定を確認してください。
- JWT トークン（必要な場合）が有効かつ期限切れでないことを確認してください。
- サーバー側のエラーは `artifacts/torii.log` をチェックしてください。
- WebSocket の問題が疑われる場合は、アプリ側ログウィンドウや Xcode コンソール出力を参照してください。
