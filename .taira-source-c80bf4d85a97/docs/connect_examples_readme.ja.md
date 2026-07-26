<!-- Japanese translation of docs/connect_examples_readme.md -->

---
lang: ja
direction: ltr
source: docs/connect_examples_readme.md
status: complete
translator: manual
---

## Iroha Connect サンプル（Rust アプリ／ウォレット）

この手順では、Torii ノードと 2 つの Rust サンプルをエンドツーエンドで実行します。

前提条件
- `http://127.0.0.1:8080` で `connect` が有効化された Torii ノード。
- Rust ツールチェーン（stable）。
- Python 3.9 以上と `iroha-python` パッケージ（下記の CLI ヘルパーを利用する場合）。

サンプル
- アプリ側サンプル: `crates/iroha_torii_shared/examples/connect_app.rs`
- ウォレット側サンプル: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI ヘルパー: `python -m iroha_python.examples.connect_flow`

起動順
1) ターミナル A — アプリ（sid とトークンを表示し、WS に接続、SignRequestTx を送信）:

    cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   出力例:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS connected
    app: sent SignRequestTx
    (waiting for reply)

2) ターミナル B — ウォレット（token_wallet で接続し、SignResultOk で応答）:

    cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   出力例:

    wallet: connected WS
    wallet: SignRequestTx len=3 at seq 1
    wallet: sent SignResultOk

3) アプリのターミナルが結果を表示:

    app: got SignResultOk algo=ed25519 sig=deadbeef

  ペイロードをデコードする際は、新しい `connect_norito_decode_envelope_sign_result_alg` ヘルパー（このフォルダの Swift/Kotlin ラッパー）を使ってアルゴリズム文字列を取得してください。

備考:
- サンプルはデモ用に `sid` からエフェメラルキーを派生させるため、アプリとウォレットが自動で相互運用できます。本番環境では使用しないでください。
- SDK は AEAD の AAD バインディングと `seq` をノンスとして利用する制約を強制します。承認後の制御フレームは暗号化して送信してください。
- Swift クライアントでは `docs/connect_swift_integration.md` / `docs/connect_swift_ios.md` を参照し、`make swift-ci` を実行してダッシュボードのテレメトリ（`ci/xcframework-smoke:<lane>:device_tag` メタデータを含む）が Rust の例と整合していることを確認してください。
- Python CLI ヘルパーの例:

    ```bash
    python -m iroha_python.examples.connect_flow \
      --base-url http://127.0.0.1:8080 \
      --sid demo-session \
      --chain-id dev-chain \
      --auth-token admin-token \
      --app-name "Demo App" \
      --frame-output connect-open.hex \
      --frame-json-output connect-open.json \
      --status-json-output connect-status.json
    ```

  CLI はセッション情報と Connect ステータスを整形表示し、Norito でエンコードされた `ConnectControlOpen` フレームを生成します。`--send-open` を付与すると Torii へ POST され、`--frame-output-format binary` を指定すると生バイトとして保存できます。`--frame-json-output` を使うと REST ツール向けに base64 を含む JSON 形式で書き出せ、`--status-json-output` でステータスの JSON スナップショットを取得できます。アプリのメタデータは `--app-metadata-file metadata.json` で JSON ファイルから読み込むことも可能です（`name` 必須、`url` / `icon_hash` は任意）。サンプルは `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json` を参照してください。テンプレートを生成する場合は `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json` を実行します。状態だけを確認したい場合は `--status-only`（必要に応じて `--status-json-output status.json`）を指定すると、セッション作成を行わずにステータスを取得できます。
