---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1017858988f6bbc1c58029ca0476e2eee7b011c3c65ba5b33a80c049165600ca
source_last_modified: "2025-12-29T18:16:35.115752+00:00"
translation_last_reviewed: 2026-02-07
title: Norito-RPC Overview
translator: machine-google-reviewed
---

# Norito-RPC 概述

Norito-RPC 是 Torii API 的二进制传输。它重用相同的 HTTP 路径
作为 `/v2/pipeline`，但交换包含模式的 Norito 帧有效负载
哈希值和校验和。当您需要确定性、经过验证的响应或
当管道 JSON 响应成为瓶颈时。

## 为什么要切换？
- 使用 CRC64 和模式哈希的确定性帧可减少解码错误。
- 跨 SDK 共享 Norito 帮助程序让您可以重用现有的数据模型类型。
- Torii 已在遥测中标记 Norito 会话，以便操作员可以监控
通过提供的仪表板进行采用。

## 提出请求

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v2/transactions/submit
```

1. 使用 Norito 编解码器（`iroha_client`、SDK 帮助程序或
   `norito::to_bytes`）。
2. 使用 `Content-Type: application/x-norito` 发送请求。
3. 使用 `Accept: application/x-norito` 请求 Norito 响应。
4. 使用匹配的 SDK 帮助程序对响应进行解码。

SDK特定指南：
- **Rust**：设置时 `iroha_client::Client` 自动协商 Norito
  `Accept` 标头。
- **Python**：使用 `iroha_python.norito_rpc` 中的 `NoritoRpcClient`。
- **Android**：在中使用 `NoritoRpcClient` 和 `NoritoRpcRequestOptions`
  安卓 SDK。
- **JavaScript/Swift**：在 `docs/source/torii/norito_rpc_tracker.md` 中跟踪助手
  并将作为 NRPC-3 的一部分着陆。

## Try It 控制台示例

开发者门户提供了 Try It 代理，以便审阅者可以重播 Norito
无需编写定制脚本即可加载有效负载。

1. [启动代理](./try-it.md#start-the-proxy-locally)并设置
   `TRYIT_PROXY_PUBLIC_URL` 因此小部件知道将流量发送到哪里。
2. 打开此页面上的**尝试**卡或 `/reference/torii-swagger`
   For MCP/agent flows, use `/reference/torii-mcp`.
   面板并选择一个端点，例如 `POST /v2/pipeline/submit`。
3.将**Content-Type**切换为`application/x-norito`，选择**Binary**
   编辑器，并上传`fixtures/norito_rpc/transfer_asset.norito`
   （或中列出的任何有效负载
   `fixtures/norito_rpc/transaction_fixtures.manifest.json`）。
4. 通过 OAuth 设备代码小部件或手动令牌提供不记名令牌
   字段（配置时代理接受 `X-TryIt-Auth` 覆盖
   `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`）。
5. 提交请求并验证 Torii 是否与中列出的 `schema_hash` 相呼应
   `fixtures/norito_rpc/schema_hashes.json`。匹配的哈希值确认
   Norito 标头在浏览器/代理跃点中幸存下来。

如需路线图证据，请将“尝试一下”屏幕截图与一系列
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`。脚本换行
`cargo xtask norito-rpc-verify`，将 JSON 摘要写入
`artifacts/norito_rpc/<timestamp>/`，并捕获与
门户已消耗。

## 故障排除

|症状|它出现在哪里 |可能的原因 |修复 |
| ---| ---| ---| ---|
| `415 Unsupported Media Type` | Torii 响应 | `Content-Type` 标头缺失或不正确 |在发送有效负载之前设置 `Content-Type: application/x-norito`。 |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii 响应正文/标头 |夹具架构哈希与 Torii 版本不同 |使用 `cargo xtask norito-rpc-fixtures` 重新生成灯具并确认 `fixtures/norito_rpc/schema_hashes.json` 中的哈希值；如果端点尚未启用 Norito，则回退到 JSON。 |
| `{"error":"origin_forbidden"}` (HTTP 403) |尝试一下代理响应 |请求来自 `TRYIT_PROXY_ALLOWED_ORIGINS` | 中未列出的来源将门户源（例如 `https://docs.devnet.sora.example`）添加到环境变量并重新启动代理。 |
| `{"error":"rate_limited"}` (HTTP 429) |尝试一下代理响应 |每个 IP 配额超出了 `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` 预算 |增加内部负载测试的限制或等待窗口重置（请参阅 JSON 响应中的 `retryAfterMs`）。 |
| `{"error":"upstream_timeout"}` (HTTP 504) 或 `{"error":"upstream_error"}` (HTTP 502) |尝试一下代理响应 | Torii 超时或代理无法到达配置的后端 |验证 `TRYIT_PROXY_TARGET` 是否可访问，检查 Torii 运行状况，或使用更大的 `TRYIT_PROXY_TIMEOUT_MS` 重试。 |

更多 Try It 诊断和 OAuth 提示请参见
[`devportal/try-it.md`](./try-it.md#norito-rpc-samples)。

## 其他资源
- 传输 RFC：`docs/source/torii/norito_rpc.md`
- 执行摘要：`docs/source/torii/norito_rpc_brief.md`
- 动作追踪器：`docs/source/torii/norito_rpc_tracker.md`
- 试用代理说明：`docs/portal/docs/devportal/try-it.md`
