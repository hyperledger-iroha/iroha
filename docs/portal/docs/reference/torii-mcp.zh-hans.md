<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
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
description: 使用 Torii 的本机模型上下文协议桥的参考指南。
---

Torii 在 `/v1/mcp` 上公开本机模型上下文协议 (MCP) 桥。
此端点允许代理发现工具并通过 JSON-RPC 调用 Torii/Connect 路由。

## 端点形状

- `GET /v1/mcp` 返回功能元数据（未封装 JSON-RPC）。
- `POST /v1/mcp` 接受 JSON-RPC 2.0 请求。
- 如果为 `torii.mcp.enabled = false`，则两条路由均未公开。
- 如果启用 `torii.require_api_token`，则在 JSON-RPC 调度之前丢失/无效的令牌将被拒绝。

## 配置

在 `torii.mcp` 下启用 MCP：

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

关键行为：

- `profile` 控制工具可见性（`read_only`、`writer`、`operator`）。
- `allow_tool_prefixes`/`deny_tool_prefixes` 应用其他基于名称的策略。
- `rate_per_minute`/`burst` 对 MCP 请求应用令牌桶限制。
- 使用 `async_job_ttl_secs` 和 `async_job_max_entries` 将 `tools/call_async` 中的异步作业状态保留在内存中。

## 推荐的客户端流程

1. 拨打 `initialize`。
2. 调用`tools/list`并缓存`toolsetVersion`。
3. 使用 `tools/call` 进行正常操作。
4. 使用 `tools/call_async` + `tools/jobs/get` 进行较长时间的操作。
5. 当 `listChanged` 为 `true` 时，重新运行 `tools/list`。

不要对完整的工具目录进行硬编码。在运行时发现。

## 方法和语义

支持的 JSON-RPC 方法：

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

注意事项：- `tools/list` 接受 `toolset_version` 和 `toolsetVersion`。
- `tools/jobs/get` 接受 `job_id` 和 `jobId`。
- `tools/list.cursor` 是数字字符串偏移量；无效值回退到 `0`。
- `tools/call_batch` 是每一项的最大努力（一次失败的调用不会使同级调用失败）。
- `tools/call_async` 仅立即验证包络形状；执行错误稍后出现在作业状态中。
- `jsonrpc` 应为 `"2.0"`；出于兼容性考虑，接受省略的 `jsonrpc`。

## 授权和转发

MCP 调度不会绕过 Torii 授权。调用执行正常的路由处理程序和身份验证检查。

Torii 转发入站身份验证相关标头以进行工具调度：

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

客户端还可以通过 `arguments.headers` 提供额外的每次调用标头。
`content-length`、`host` 和 `arguments.headers` 中的 `connection` 将被忽略。

## 错误模型

HTTP层：

- `400` 无效 JSON
- `403` API 令牌在 JSON-RPC 处理之前被拒绝
- `413` 有效负载超过 `max_request_bytes`
- `429` 速率受限
- `200` 用于 JSON-RPC 响应（包括 JSON-RPC 错误）

JSON-RPC 层：- 顶级 `error.data.error_code` 稳定（例如 `invalid_request`、`invalid_params`、`tool_not_found`、`tool_not_allowed`、`job_not_found`、`rate_limited`）。
- 工具故障表现为 MCP 工具结果 `isError = true` 和结构化细节。
- 路由调度工具故障将 HTTP 状态映射到 `structuredContent.error_code`（例如 `forbidden`、`not_found`、`server_error`）。

## 工具命名

OpenAPI 派生工具使用稳定的基于路由的名称：

- `torii.<method>_<path...>`
- 示例：`torii.get_v1_accounts`

精选的别名也会在 `iroha.*` 和 `connect.*` 下公开。

## 规范规范

完整的线路级合同保存在：

- `crates/iroha_torii/docs/mcp_api.md`

当 `crates/iroha_torii/src/mcp.rs` 或 `crates/iroha_torii/src/lib.rs` 中的行为发生变化时，
在同一更改中更新该规范，然后在此处镜像密钥使用指南。