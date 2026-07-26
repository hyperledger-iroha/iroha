---
lang: zh-hant
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha 連接示例（Rust 應用程序/錢包）

使用 Torii 節點端到端運行兩個 Rust 示例。

先決條件
- Torii 節點，在 `http://127.0.0.1:8080` 處啟用 `connect`。
- Rust 工具鏈（穩定）。
- 安裝了 `iroha-python` 軟件包的 Python 3.9+（用於下面的 CLI 幫助程序）。

示例
- 應用程序示例：`crates/iroha_torii_shared/examples/connect_app.rs`
- 錢包示例：`crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI 幫助程序：`python -m iroha_python.examples.connect_flow`

啟動順序
1）終端A——App（打印sid + token，連接WS，發送SignRequestTx）：

    Cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   示例輸出：

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS已連接
    應用程序：發送 SignRequestTx
    （等待回复）

2) 終端B——錢包（連接token_wallet，回复SignResultOk）：

    Cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   示例輸出：

    錢包：連接WS
    錢包：SignRequestTx len=3 在 seq 1
    錢包：已發送 SignResultOk

3）App終端打印結果：

    應用程序：得到 SignResultOk algo=ed25519 sig=deadbeef

  使用新的 `connect_norito_decode_envelope_sign_result_alg` 幫助程序（以及
  此文件夾中的 Swift/Kotlin 包裝器）在以下情況下檢索算法字符串
  解碼有效負載。

註釋
- 示例從 `sid` 派生演示臨時文件，以便應用程序/錢包自動互操作。不要在生產中使用。
- SDK 強制執行 AEAD AAD 綁定和 seq-as-nonce；批准後控制幀應加密。
- 對於 Swift 客戶端，請參閱 `docs/connect_swift_integration.md` / `docs/connect_swift_ios.md` 並使用 `make swift-ci` 進行驗證，以便儀表板遙測與 Rust 示例保持一致，並且 Buildkite 元數據 (`ci/xcframework-smoke:<lane>:device_tag`) 保持不變。
- Python CLI 幫助程序用法：

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

  CLI 打印鍵入的會話信息，轉儲連接狀態快照，並發出 Norito 編碼的 `ConnectControlOpen` 幀。傳遞 `--send-open` 將有效負載發送回 Torii，使用 `--frame-output-format binary` 寫入原始字節，使用 `--frame-json-output` 表示對 base64 友好的 JSON blob，並在需要類型化快照以實現自動化時使用 `--status-json-output`。您還可以通過包含 `name`、`url` 和 `icon_hash` 字段的 `--app-metadata-file metadata.json` 從 JSON 文件加載應用程序元數據（請參閱 `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`）。使用 `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json` 生成新模板。對於僅遙測運行，您可以使用 `--status-only` 完全跳過會話創建，並可選擇通過 `--status-json-output status.json` 轉儲 JSON。