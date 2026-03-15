---
lang: zh-hant
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

Norito-RPC 是 Torii API 的二進制傳輸。它重用相同的 HTTP 路徑
作為 `/v1/pipeline`，但交換包含模式的 Norito 幀有效負載
哈希值和校驗和。當您需要確定性、經過驗證的響應或
當管道 JSON 響應成為瓶頸時。

## 為什麼要切換？
- 使用 CRC64 和模式哈希的確定性幀可減少解碼錯誤。
- 跨 SDK 共享 Norito 幫助程序讓您可以重用現有的數據模型類型。
- Torii 已在遙測中標記 Norito 會話，以便操作員可以監控
通過提供的儀表板進行採用。

## 提出請求

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v1/transactions/submit
```

1. 使用 Norito 編解碼器（`iroha_client`、SDK 幫助程序或
   `norito::to_bytes`）。
2. 使用 `Content-Type: application/x-norito` 發送請求。
3. 使用 `Accept: application/x-norito` 請求 Norito 響應。
4. 使用匹配的 SDK 幫助程序對響應進行解碼。

SDK特定指南：
- **Rust**：設置時 `iroha_client::Client` 自動協商 Norito
  `Accept` 標頭。
- **Python**：使用 `iroha_python.norito_rpc` 中的 `NoritoRpcClient`。
- **Android**：在中使用 `NoritoRpcClient` 和 `NoritoRpcRequestOptions`
  安卓 SDK。
- **JavaScript/Swift**：在 `docs/source/torii/norito_rpc_tracker.md` 中跟踪助手
  並將作為 NRPC-3 的一部分著陸。

## Try It 控制台示例

開發者門戶提供了 Try It 代理，以便審閱者可以重播 Norito
無需編寫定制腳本即可加載有效負載。

1. [啟動代理](./try-it.md#start-the-proxy-locally)並設置
   `TRYIT_PROXY_PUBLIC_URL` 因此小部件知道將流量發送到哪裡。
2. 打開此頁面上的**嘗試**卡或 `/reference/torii-swagger`
   For MCP/agent flows, use `/reference/torii-mcp`.
   面板並選擇一個端點，例如 `POST /v1/pipeline/submit`。
3.將**Content-Type**切換為`application/x-norito`，選擇**Binary**
   編輯器，並上傳`fixtures/norito_rpc/transfer_asset.norito`
   （或中列出的任何有效負載
   `fixtures/norito_rpc/transaction_fixtures.manifest.json`）。
4. 通過 OAuth 設備代碼小部件或手動令牌提供不記名令牌
   字段（配置時代理接受 `X-TryIt-Auth` 覆蓋
   `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`）。
5. 提交請求並驗證 Torii 是否與中列出的 `schema_hash` 相呼應
   `fixtures/norito_rpc/schema_hashes.json`。匹配的哈希值確認
   Norito 標頭在瀏覽器/代理躍點中倖存下來。

如需路線圖證據，請將“嘗試一下”屏幕截圖與一系列
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`。腳本換行
`cargo xtask norito-rpc-verify`，將 JSON 摘要寫入
`artifacts/norito_rpc/<timestamp>/`，並捕獲與
門戶已消耗。

## 故障排除

|症狀|它出現在哪裡 |可能的原因 |修復 |
| ---| ---| ---| ---|
| `415 Unsupported Media Type` | Torii 響應 | `Content-Type` 標頭缺失或不正確 |在發送有效負載之前設置 `Content-Type: application/x-norito`。 |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii 響應正文/標頭 |夾具架構哈希與 Torii 版本不同 |使用 `cargo xtask norito-rpc-fixtures` 重新生成燈具並確認 `fixtures/norito_rpc/schema_hashes.json` 中的哈希值；如果端點尚未啟用 Norito，則回退到 JSON。 |
| `{"error":"origin_forbidden"}` (HTTP 403) |嘗試一下代理響應 |請求來自 `TRYIT_PROXY_ALLOWED_ORIGINS` | 中未列出的來源將門戶源（例如 `https://docs.devnet.sora.example`）添加到環境變量並重新啟動代理。 |
| `{"error":"rate_limited"}` (HTTP 429) |嘗試一下代理響應 |每個 IP 配額超出了 `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` 預算 |增加內部負載測試的限製或等待窗口重置（請參閱 JSON 響應中的 `retryAfterMs`）。 |
| `{"error":"upstream_timeout"}` (HTTP 504) 或 `{"error":"upstream_error"}` (HTTP 502) |嘗試一下代理響應 | Torii 超時或代理無法到達配置的後端 |驗證 `TRYIT_PROXY_TARGET` 是否可訪問，檢查 Torii 運行狀況，或使用更大的 `TRYIT_PROXY_TIMEOUT_MS` 重試。 |

更多 Try It 診斷和 OAuth 提示請參見
[`devportal/try-it.md`](./try-it.md#norito-rpc-samples)。

## 其他資源
- 傳輸 RFC：`docs/source/torii/norito_rpc.md`
- 執行摘要：`docs/source/torii/norito_rpc_brief.md`
- 動作追踪器：`docs/source/torii/norito_rpc_tracker.md`
- 試用代理說明：`docs/portal/docs/devportal/try-it.md`
