<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
id: torii-mcp
title: Torii MCP API
description: 使用 Torii 的本機模型上下文協定橋的參考指南。
---

Torii 在 `/v1/mcp` 上公開本機模型上下文協定 (MCP) 橋。
此端點允許代理程式發現工具並透過 JSON-RPC 呼叫 Torii/Connect 路由。

## 端點形狀

- `GET /v1/mcp` 傳回功能元資料（未封裝 JSON-RPC）。
- `POST /v1/mcp` 接受 JSON-RPC 2.0 請求。
- 若為 `torii.mcp.enabled = false`，則兩條路由均未公開。
- 如果啟用 `torii.require_api_token`，則在 JSON-RPC 調度之前遺失/無效的令牌將被拒絕。

## 配置

在 `torii.mcp` 下啟用 MCP：

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "profile": "read_only",
      "expose_operator_routes": false,
      "allow_tool_prefixes": [],
      "deny_tool_prefixes": [],
      "rate_per_minute": 240,
      "burst": 120,
      "async_job_ttl_secs": 300,
      "async_job_max_entries": 2000
    }
  }
}
```

關鍵行為：

- `profile` 控制工具可見度（`read_only`、`writer`、`operator`）。
- `allow_tool_prefixes`/`deny_tool_prefixes` 應用其他基於名稱的策略。
- `rate_per_minute`/`burst` 對 MCP 請求套用令牌桶限制。
- 使用 `async_job_ttl_secs` 和 `async_job_max_entries` 將 `tools/call_async` 中的非同步作業狀態保留在記憶體中。

## 推薦的客戶端流程

1. 撥打 `initialize`。
2. 呼叫`tools/list`並快取`toolsetVersion`。
3. 使用 `tools/call` 進行正常操作。
4. 使用 `tools/call_async` + `tools/jobs/get` 進行較長時間的操作。
5. 當 `listChanged` 為 `true` 時，重新運行 `tools/list`。

不要對完整的工具目錄進行硬編碼。在運行時發現。

## 方法和語義

支援的 JSON-RPC 方法：

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

注意事項：- `tools/list` 接受 `toolset_version` 和 `toolsetVersion`。
- `tools/jobs/get` 接受 `job_id` 和 `jobId`。
- `tools/list.cursor` 是數字字串偏移量；無效值回退到 `0`。
- `tools/call_batch` 是每一項的最大努力（一次失敗的呼叫不會使同級呼叫失敗）。
- `tools/call_async` 僅立即驗證包絡形狀；執行錯誤稍後出現在作業狀態。
- `jsonrpc` 應為 `"2.0"`；出於相容性考慮，接受省略的 `jsonrpc`。

## 授權和轉發

MCP 調度不會繞過 Torii 授權。呼叫執行正常的路由處理程序和身份驗證檢查。

Torii 轉送入站驗證相關標頭以進行工具調度：

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

客戶端還可以透過 `arguments.headers` 提供額外的每次呼叫標頭。
`content-length`、`host` 和 `arguments.headers` 中的 `connection` 將被忽略。

## 錯誤模型

HTTP層：

- `400` 無效 JSON
- `403` API 令牌在 JSON-RPC 處理前被拒絕
- `413` 有效負載超過 `max_request_bytes`
- `429` 速率受限
- `200` 用於 JSON-RPC 回應（包括 JSON-RPC 錯誤）

JSON-RPC 層：- 頂級 `error.data.error_code` 穩定（例如 `invalid_request`、`invalid_params`、`tool_not_found`、`tool_not_allowed`、`job_not_found`、I1800071X、`job_not_found`、I18072X00000072X）。
- 工具故障表現為 MCP 工具結果 `isError = true` 和結構化細節。
- 路由調度工具故障將 HTTP 狀態對應到 `structuredContent.error_code`（例如 `forbidden`、`not_found`、`server_error`）。

## 工具命名

OpenAPI 派生工具使用穩定的基於路由的名稱：

- `torii.<method>_<path...>`
- 範例：`torii.get_v1_accounts`

精選的別名也會在 `iroha.*` 和 `connect.*` 下公開。

## 規範規範

完整的線路級合約保存在：

- `crates/iroha_torii/docs/mcp_api.md`

當 `crates/iroha_torii/src/mcp.rs` 或 `crates/iroha_torii/src/lib.rs` 中的行為改變時，
在相同變更中更新該規範，然後在此處鏡像金鑰使用指南。