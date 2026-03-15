---
id: torii-app-api-parity
lang: zh-hans
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Torii app API parity audit
description: Mirror of the TORII-APP-1 review so SDK and platform teams can confirm public coverage.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

状态：已完成 2026-03-21  
所有者：Torii 平台，SDK 项目负责人  
路线图参考：TORII-APP-1 — `app_api` 奇偶校验审核

此页面镜像内部 `TORII-APP-1` 审核 (`docs/source/torii/app_api_parity_audit.md`)
因此 mono-repo 之外的读者可以看到哪些 `/v1/*` 表面已接线、经过测试、
并记录下来。审计跟踪通过`Torii::add_app_api_routes`再导出的路由，
`add_contracts_and_vk_routes` 和 `add_connect_routes`。

## 范围和方法

审计检查了`crates/iroha_torii/src/lib.rs:256-522`中的公开再出口和
功能门控路线构建器。对于路线图中的每个 `/v1/*` 表面，我们验证了：

- `crates/iroha_torii/src/routing.rs` 中的处理程序实现和 DTO 定义。
- `app_api` 或 `connect` 功能组下的路由器注册。
- 现有的集成/单元测试和负责长期覆盖的拥有团队。

账户资产/交易和资产持有者列表接受可选的 `asset_id` 查询参数
除了现有的分页/反压限制之外，还用于预过滤。

## 授权和规范签名

- 面向应用程序的 GET/POST 端点接受从 `METHOD\n/path\nsorted_query\nsha256(body)` 构建的可选规范请求标头（`X-Iroha-Account`、`X-Iroha-Signature`）； Torii 在执行器验证之前将它们包装到 `QueryRequestWithAuthority` 中，因此它们镜像 `/query`。
- SDK 助手在所有主要客户端中提供：
  - JS/TS：`buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` 来自 `canonicalRequest.js`。
  - 斯威夫特：`CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`。
  - Android（Kotlin/Java）：`CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`。
- 示例片段：
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "i105...", method: "get", path: "/v1/accounts/i105.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/i105.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "i105...",
                                                  method: "get",
                                                  path: "/v1/accounts/i105.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("i105...", "get", "/v1/accounts/i105.../assets", "limit=5", ByteArray(0), signer)
```

## 端点库存

### 帐户权限 (`/v1/accounts/{id}/permissions`) — 涵盖
- 处理程序：`handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)。
- DTO：`filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)。
- 路由器绑定：`Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- 测试：`crates/iroha_torii/tests/accounts_endpoints.rs:126` 和 `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`。
- 所有者：Torii 平台。
- 注意：响应是 Norito JSON 正文，其中包含 `items`/`total`，匹配 SDK 分页助手。

### 别名 OPRF 评估 (`POST /v1/aliases/voprf/evaluate`) — 已覆盖
- 处理程序：`handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)。
- DTO：`AliasVoprfEvaluateRequestDto`、`AliasVoprfEvaluateResponseDto`、`AliasVoprfBackendDto`
  （`crates/iroha_torii/src/routing.rs:809-865`）。
- 路由器绑定：`Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)。
- 测试：内联处理程序测试 (`crates/iroha_torii/src/lib.rs:9945-9986`) 以及 SDK 覆盖范围
  （`javascript/iroha_js/test/toriiClient.test.js:72`）。
- 所有者：Torii 平台。
- 注意：响应面强制执行确定性十六进制和后端标识符； SDK 使用 DTO。

### 证明事件 SSE (`GET /v1/events/sse`) — 涵盖
- 处理程序：`handle_v1_events_sse`，带过滤器支持 (`crates/iroha_torii/src/routing.rs:14008-14133`)。
- DTO：`EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) 加上防爆滤波器接线。
- 路由器绑定：`Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- 测试：特定证明的 SSE 套件（`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`，
  `sse_proof_callhash.rs`、`sse_proof_verified_fields.rs`、`sse_proof_rejected_fields.rs`) 和管道 SSE 烟雾测试
  （`integration_tests/tests/events/sse_smoke.rs`）。
- 所有者：Torii 平台（运行时）、集成测试工作组（固定装置）。
- 注：证明过滤器路径经过端到端验证；文档位于 `docs/source/zk_app_api.md` 下。

### 合同生命周期 (`/v1/contracts/*`) — 涵盖
- 处理程序：`handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)，
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)。
- DTO：`DeployContractDto`、`DeployAndActivateInstanceDto`、`ActivateInstanceDto`、`ContractCallDto`
  （`crates/iroha_torii/src/routing.rs:3124-3463`）。
- 路由器绑定：`Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- 测试：路由器/集成套件 `contracts_deploy_integration.rs`、`contracts_activate_integration.rs`、
  `contracts_instance_activate_integration.rs`、`contracts_call_integration.rs`、
  `contracts_instances_list_router.rs`。
- 所有者：拥有 Torii 平台的智能合约工作组。
- 注意：端点对签名事务进行排队并重用共享遥测指标（`handle_transaction_with_metrics`）。

### 验证密钥生命周期 (`/v1/zk/vk/*`) — 涵盖
- 处理程序：`handle_post_vk_register`、`handle_post_vk_update`、`handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) 和 `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)。
- DTO：`ZkVkRegisterDto`、`ZkVkUpdateDto`、`ZkVkDeprecateDto`、`VkListQuery`、`ProofFindByIdQueryDto`
  （`crates/iroha_torii/src/routing.rs:3619-4279`）。
- 路由器绑定：`Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- 测试：`crates/iroha_torii/tests/zk_vk_get_integration.rs`，
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`，
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`。
- 所有者：ZK 工作组，提供 Torii 平台支持。
- 注意：DTO 与 SDK 引用的 Norito 架构保持一致；通过 `limits.rs` 强制执行速率限制。

### Nexus 连接 (`/v1/connect/*`) — 涵盖（功能 `connect`）
- 处理程序：`handle_connect_session`、`handler_connect_session_delete`、`handle_connect_ws`、
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)。
- DTO：`ConnectSessionRequest`、`ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)、
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)。
- 路由器绑定：`Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)。
- 测试：`crates/iroha_torii/tests/connect_gating.rs`（功能门控、会话生命周期、WS 握手）和
  路由器功能矩阵覆盖范围 (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`)。
- 所有者：Nexus 连接工作组。
- 注意：通过 `limits::rate_limit_key` 跟踪的速率限制密钥；遥测计数器提供 `connect.*` 指标。

### Kaigi 中继遥测 — 涵盖
- 处理程序：`handle_v1_kaigi_relays`、`handle_v1_kaigi_relay_detail`、
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  （`crates/iroha_torii/src/routing.rs:14510-14787`）。
- DTO：`KaigiRelaySummaryDto`、`KaigiRelaySummaryListDto`、
  `KaigiRelayDetailDto`、`KaigiRelayDomainMetricsDto`、
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)。
- 路由器绑定：`Torii::add_app_api_routes`
  （`crates/iroha_torii/src/lib.rs:6805-6840`）。
- 测试：`crates/iroha_torii/tests/kaigi_endpoints.rs`。
- 注意：SSE流在执行时重用全局广播通道
  遥测剖面选通；响应模式记录在
  `docs/source/torii/kaigi_telemetry_api.md`。

## 测试覆盖率总结

- 路由器冒烟测试 (`crates/iroha_torii/tests/router_feature_matrix.rs`) 确保功能组合注册每个
  路线和 OpenAPI 一代保持同步。
- 特定于端点的套件涵盖帐户查询、合约生命周期、ZK 验证密钥、SSE 证明过滤器和 Nexus
  连接行为。
- SDK 奇偶校验工具（JavaScript、Swift、Python）已使用 Alias VOPRF 和 SSE 端点；无需额外工作
  需要。

## 保持此镜像最新

更新此页面和源审核 (`docs/source/torii/app_api_parity_audit.md`)
每当 Torii 应用 API 行为发生变化时，SDK 所有者和外部读者保持一致。