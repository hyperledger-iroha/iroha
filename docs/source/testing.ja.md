<!-- Japanese translation of docs/source/testing.md -->

---
lang: ja
direction: ltr
source: docs/source/testing.md
status: complete
translator: manual
---

# テストとトラブルシューティングガイド

本ガイドは統合シナリオの再現手順、稼働させるべきインフラ、ログの収集方法を説明します。作業前にプロジェクト全体の[ステータスレポート](../../status.md)を参照し、現時点でグリーンなコンポーネントを把握してください。

## 再現手順

### 統合テスト（`integration_tests` クレート）

1. 依存関係を含めてビルド: `cargo build --workspace`
2. フルログで統合テストを実行: `cargo test -p integration_tests -- --nocapture`
3. 個別シナリオを再実行する場合はモジュールパスを指定（例: `cargo test -p integration_tests settlement::happy_path -- --nocapture`）
4. Norito でシリアライズしたフィクスチャを用意し、ノード間で一貫した入力を共有:
   ```rust
   use norito::json;

   let genesis_payload = json::to_string_pretty(&json::json!({
       "chain" : "testnet",
       "peers" : ["127.0.0.1:1337"],
       "accounts" : [{
           "id" : "soraカタカナ...",
           "public_key" : "ed0120..."
       }]
   }))?;
   ```
   生成した Norito JSON はテスト成果物と一緒に保管し、ピアが同じ状態を再生できるようにします。

### Python クライアントテスト（`pytests` ディレクトリ）

1. 仮想環境内で `pip install -r pytests/requirements.txt` を実行し依存関係を導入。
2. 上記で生成した Norito 対応フィクスチャを共有パスまたは環境変数経由でエクスポート。
3. 詳細ログ付きで実行: `pytest -vv pytests`
4. 調査対象を絞る場合は `pytest -k "Query" pytests/tests/test_queries.py --log-cli-level=INFO`

## 必要なポートとサービス

以下のサービスに接続できることを確認してから各スイートを実行してください。

- **Torii HTTP API**: 既定 `127.0.0.1:1337`。`docs/source/references/peer.template.toml` を参照し、設定ファイルの `torii.address` で上書き。
- **Torii WebSocket 通知**: 既定 `127.0.0.1:8080`。`pytests` のクライアント購読で使用。
- **テレメトリエクスポータ**: 既定 `127.0.0.1:8180`。統合テストはヘルス確認のためメトリクスが流れることを前提とします。
- **PostgreSQL**（有効時）: 既定 `127.0.0.1:5432`。[`defaults/docker-compose.local.yml`](../../defaults/docker-compose.local.yml) のプロファイルと認証情報を合わせてください。

エンドポイントが利用できない場合は[テレメトリトラブルシューティングガイド](telemetry.md)も参照。

### 埋め込みピアの安定性

`NetworkBuilder::start()` はジェネシス後 5 秒間のライブネスウィンドウを全埋め込みピアに対して課しています。期間中にプロセスが終了すると、キャッシュされた stdout/stderr ログを示す詳細エラーでビルダーが中断します。リソースが限られた環境では `IROHA_TEST_POST_GENESIS_LIVENESS_MS` を用いてウィンドウ（ミリ秒）を延長できます。`0` に設定するとガードを無効化します。統合スイート開始直後の数秒間、ピアがブロック 1 に到達できるよう十分な CPU 余裕を確保してください。

## ログ収集と分析

古い成果物に埋もれないよう、クリーンな作業ディレクトリから開始しましょう。以下のスクリプトは Norito ツールチェーンで扱いやすい形式でログを収集します。

- テスト後に [`scripts/analyze_telemetry.sh`](../../scripts/analyze_telemetry.sh) を実行し、ノードメトリクスをタイムスタンプ付き Norito JSON に集約。
- ネットワーク調査時は [`scripts/run_iroha_monitor_demo.py`](../../scripts/run_iroha_monitor_demo.py) を使用し、Torii イベントを `monitor_output.norito.json` にストリーム。
- 統合テストのログは `integration_tests/target/` 以下に保存されます。共有時は [`scripts/profile_build.sh`](../../scripts/profile_build.sh) で圧縮。
- Python クライアントのログは `pytests/.pytest_cache` に出力されます。テレメトリと併せて以下で収集:
  ```bash
  ./scripts/report_red_team_failures.py --tests pytests --artifacts out/logs
  ```

インシデントを報告する前に、統合・Python・テレメトリの全バンドルを揃え、メンテナが Norito トレースを再生できるようにしてください。

## 次のステップ

リリース固有のチェックリストは [pipeline](pipeline.md) を参照。リグレッションや失敗を発見した場合は共通の[ステータストラッカー](../../status.md)に記録し、必要に応じて [sumeragi troubleshooting](sumeragi.md) へのリンクを併記してください。
