---
lang: ja
direction: ltr
source: docs/source/mochi/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f5bc20c9545dd210ea8cd3adec2e8dc9957e99d5939f45afc8c28def7f1c6b1
source_last_modified: "2025-11-20T04:32:51.480819+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/mochi/quickstart.md -->

# MOCHI クイックスタート

**MOCHI** はローカルの Hyperledger Iroha ネットワーク用デスクトップ
スーパーバイザです。本ガイドでは、前提条件のインストール、
アプリのビルド、egui シェルの起動、日常開発で使う runtime ツール
（設定、スナップショット、ワイプ）を説明します。

## 前提条件

- Rust toolchain: `rustup default stable`（workspace は edition 2024 / Rust 1.82+）。
- プラットフォーム toolchain:
  - macOS: Xcode Command Line Tools (`xcode-select --install`).
  - Linux: GCC, pkg-config, OpenSSL headers (`sudo apt install build-essential pkg-config libssl-dev`).
- Iroha workspace の依存:
  - `cargo xtask mochi-bundle` には `irohad`, `kagami`, `iroha_cli` のビルドが必要。
    `cargo build -p irohad -p kagami -p iroha_cli` で一度ビルドする。
- オプション: ローカル cargo バイナリ管理に `direnv` または `cargo binstall`。

MOCHI は CLI バイナリをシェル経由で呼び出します。以下の環境変数で指定するか、
PATH 上で見つかるようにしてください:

| バイナリ | 環境変数オーバーライド | 備考 |
|----------|------------------------|------|
| `irohad` | `MOCHI_IROHAD` | ピアを監督 | 
| `kagami` | `MOCHI_KAGAMI` | genesis マニフェスト/スナップショットを生成 |
| `iroha_cli` | `MOCHI_IROHA_CLI` | 今後のヘルパー機能向け（任意） |

## MOCHI のビルド

リポジトリルートから:

```bash
cargo build -p mochi-ui-egui
```

このコマンドは `mochi-core` と egui フロントエンドを両方ビルドします。
配布用バンドルを作るには次を実行します:

```bash
cargo xtask mochi-bundle
```

バンドルタスクはバイナリ、マニフェスト、設定スタブを `target/mochi-bundle` に
まとめます。

## egui シェルの起動

cargo から UI を直接実行します:

```bash
cargo run -p mochi-ui-egui
```

デフォルトでは、MOCHI は一時データディレクトリに single-peer プリセットを作成します:

- データルート: `$TMPDIR/mochi`.
- Torii ベースポート: `8080`.
- P2P ベースポート: `1337`.

起動時に CLI フラグでデフォルトを上書きできます:

```bash
cargo run -p mochi-ui-egui -- \
  --data-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

CLI フラグを省略した場合は環境変数が同じ上書きを提供します。
`MOCHI_DATA_ROOT`, `MOCHI_PROFILE`, `MOCHI_CHAIN_ID`, `MOCHI_TORII_START`,
`MOCHI_P2P_START`, `MOCHI_RESTART_MODE`, `MOCHI_RESTART_MAX`,
`MOCHI_RESTART_BACKOFF_MS` を設定して supervisor builder を事前設定できます。
バイナリパスは `MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI` を引き続き尊重し、
`MOCHI_CONFIG` は明示的な `config/local.toml` を指します。

## 設定と永続化

ダッシュボードのツールバーから **Settings** ダイアログを開き、
supervisor 設定を調整します:

- **Data root** — peer 設定、ストレージ、ログ、スナップショットのベースディレクトリ。
- **Torii / P2P base ports** — 決定論的な割り当ての開始ポート。
- **Log visibility** — log viewer の stdout/stderr/system チャンネル切替。

supervisor の再起動ポリシーなどの高度なノブは `config/local.toml` にあります。
`[supervisor.restart] mode = "never"` を設定すると、インシデントのデバッグ中に
自動再起動を無効化できます。`max_restarts`/`backoff_ms` は設定ファイルまたは
CLI フラグ `--restart-mode`, `--restart-max`, `--restart-backoff-ms` で調整します。

変更を適用すると supervisor が再構築され、稼働中の peer が再起動され、
オーバーライドが `config/local.toml` に書き込まれます。設定マージは無関係なキーを
保持するため、高度なユーザは MOCHI 管理値と手動調整を共存させられます。

## スナップショット & wipe/re-genesis

**Maintenance** ダイアログには 2 つの安全操作があります:

- **Export snapshot** — peer の storage/config/log と現在の genesis マニフェストを
  有効な data root 配下の `snapshots/<label>` にコピーします。ラベルは自動的に
  サニタイズされます。
- **Restore snapshot** — 既存バンドルから peer storage、snapshot roots、config、logs、
  genesis マニフェストを復元します。`Supervisor::restore_snapshot` は絶対パスまたは
  サニタイズされた `snapshots/<label>` フォルダ名を受け取り、UI はこのフローを
  反映するため、Maintenance → Restore で手動操作なしに証拠バンドルを再生できます。
- **Wipe & re-genesis** — 稼働中の peer を停止し、ストレージディレクトリを削除し、
  Kagami により genesis を再生成し、ワイプ完了後に peer を再起動します。

これらのフローは回帰テスト（`export_snapshot_captures_storage_and_metadata`,
`wipe_and_regenerate_resets_storage_and_genesis`）でカバーされ、
決定論的な出力が保証されます。

## ログ & ストリーム

ダッシュボードはデータ/メトリクスを一望できるように提供します:

- **Logs** — `irohad` の stdout/stderr/system ライフサイクルメッセージを追跡。
  Settings でチャンネルを切替可能。
- **Blocks / Events** — 管理ストリームは指数バックオフで自動再接続し、
  Norito デコード済みサマリをフレームに付与。
- **Status** — `/status` をポーリングし、キュー深度、スループット、
  レイテンシのスパークラインを描画。
- **Startup readiness** — **Start** を押すと（単一または全 peer）、MOCHI は
  `/status` を制限付き backoff でチェック。バナーは各 peer の ready 状態と
  観測キュー深度を報告し、タイムアウト時は Torii エラーを表示します。

state explorer と composer のタブは、アカウント、資産、peer、
一般的な命令へ UI から離れずにアクセスできます。Peers ビューは `FindPeers` クエリを
ミラーし、統合テスト前にバリデータセットへ登録されている公開鍵を確認できます。

composer ツールバーの **Manage signing vault** ボタンで署名権限をインポート/編集できます。
ダイアログはアクティブなネットワークルート（`<data_root>/<profile>/signers.json`）に
エントリを書き込み、保存された vault キーはトランザクションのプレビュー/送信に
即座に利用可能です。vault が空の場合、composer は同梱の開発キーにフォールバックして
ローカル作業を継続します。フォームは mint/burn/transfer（暗黙の受領を含む）、
ドメイン/アカウント/資産定義の登録、アカウント入会ポリシー、マルチシグ提案、
Space Directory マニフェスト（AXT/AMX）、SoraFS ピンマニフェスト、
役割の付与/剥奪などのガバナンス操作をカバーし、
Norito payload を手書きせずにロードマップ作業をリハーサルできます。

## クリーンアップとトラブルシューティング

- アプリを停止して監督された peer を終了します。
- データルートを削除（`rm -rf <data_root>`）して状態をリセットします。
- Kagami や irohad の場所が変わった場合、環境変数を更新するか、
  適切な CLI フラグで MOCHI を再実行します。Settings ダイアログが次回の適用で
  新しいパスを保存します。

自動化の追加は `mochi/mochi-core/tests`（supervisor ライフサイクルテスト）と
`mochi/mochi-integration`（モック Torii シナリオ）を参照してください。バンドルの
配布やデスクトップの CI パイプライン連携は {doc}`mochi/packaging` を参照します。

## ローカルテストゲート

パッチ送信前に `ci/check_mochi.sh` を実行し、共有 CI ゲートが MOCHI の 3 クレートを
検証できるようにします:

```bash
./ci/check_mochi.sh
```

このヘルパーは `mochi-core`, `mochi-ui-egui`, `mochi-integration` に対して
`cargo check`/`cargo test` を実行し、
フィクスチャのドリフト（正準ブロック/イベント捕捉）と egui ハーネスの
リグレッションをまとめて検出します。スクリプトが古いフィクスチャを報告した場合は、
無視されている再生成テストを再実行してください。例:

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

再生成後にゲートを再実行し、更新されたバイト列が一貫していることを確認してから
プッシュします。
