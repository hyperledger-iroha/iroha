---
lang: zh-hans
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha 连接示例（Rust 应用程序/钱包）

使用 Torii 节点端到端运行两个 Rust 示例。

先决条件
- Torii 节点，在 `http://127.0.0.1:8080` 处启用 `connect`。
- Rust 工具链（稳定）。
- 安装了 `iroha-python` 软件包的 Python 3.9+（用于下面的 CLI 帮助程序）。

示例
- 应用程序示例：`crates/iroha_torii_shared/examples/connect_app.rs`
- 钱包示例：`crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI 帮助程序：`python -m iroha_python.examples.connect_flow`

启动顺序
1）终端A——App（打印sid + token，连接WS，发送SignRequestTx）：

    Cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   示例输出：

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS已连接
    应用程序：发送 SignRequestTx
    （等待回复）

2) 终端B——钱包（连接token_wallet，回复SignResultOk）：

    Cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   示例输出：

    钱包：连接WS
    钱包：SignRequestTx len=3 在 seq 1
    钱包：已发送 SignResultOk

3）App终端打印结果：

    应用程序：得到 SignResultOk algo=ed25519 sig=deadbeef

  使用新的 `connect_norito_decode_envelope_sign_result_alg` 帮助程序（以及
  此文件夹中的 Swift/Kotlin 包装器）在以下情况下检索算法字符串
  解码有效负载。

注释
- 示例从 `sid` 派生演示临时文件，以便应用程序/钱包自动互操作。不要在生产中使用。
- SDK 强制执行 AEAD AAD 绑定和 seq-as-nonce；批准后控制帧应加密。
- 对于 Swift 客户端，请参阅 `docs/connect_swift_integration.md` / `docs/connect_swift_ios.md` 并使用 `make swift-ci` 进行验证，以便仪表板遥测与 Rust 示例保持一致，并且 Buildkite 元数据 (`ci/xcframework-smoke:<lane>:device_tag`) 保持不变。
- Python CLI 帮助程序用法：

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

  CLI 打印键入的会话信息，转储连接状态快照，并发出 Norito 编码的 `ConnectControlOpen` 帧。传递 `--send-open` 将有效负载发送回 Torii，使用 `--frame-output-format binary` 写入原始字节，使用 `--frame-json-output` 表示对 base64 友好的 JSON blob，并在需要类型化快照以实现自动化时使用 `--status-json-output`。您还可以通过包含 `name`、`url` 和 `icon_hash` 字段的 `--app-metadata-file metadata.json` 从 JSON 文件加载应用程序元数据（请参阅 `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`）。使用 `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json` 生成新模板。对于仅遥测运行，您可以使用 `--status-only` 完全跳过会话创建，并可选择通过 `--status-json-output status.json` 转储 JSON。