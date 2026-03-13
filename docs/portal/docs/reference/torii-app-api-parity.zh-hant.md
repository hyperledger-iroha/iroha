---
lang: zh-hant
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b8769f4d2a6dd365b851c583d936fd78594ca60087fa3bbdcc6dc8177ee12be6
source_last_modified: "2026-01-30T12:29:10.183882+00:00"
translation_last_reviewed: 2026-02-07
id: torii-app-api-parity
title: Torii app API parity audit
description: Mirror of the TORII-APP-1 review so SDK and platform teams can confirm public coverage.
translator: machine-google-reviewed
---

狀態：已完成 2026-03-21  
所有者：Torii 平台，SDK 項目負責人  
路線圖參考：TORII-APP-1 — `app_api` 奇偶校驗審計

此頁面鏡像內部 `TORII-APP-1` 審核 (`docs/source/torii/app_api_parity_audit.md`)
因此 mono-repo 之外的讀者可以看到哪些 `/v2/*` 表面已接線、經過測試、
並記錄下來。審計跟踪通過`Torii::add_app_api_routes`再導出的路由，
`add_contracts_and_vk_routes` 和 `add_connect_routes`。

## 範圍和方法

審計檢查了`crates/iroha_torii/src/lib.rs:256-522`中的公開再出口和
功能門控路線構建器。對於路線圖中的每個 `/v2/*` 表面，我們驗證了：

- `crates/iroha_torii/src/routing.rs` 中的處理程序實現和 DTO 定義。
- `app_api` 或 `connect` 功能組下的路由器註冊。
- 現有的集成/單元測試和負責長期覆蓋的擁有團隊。

賬戶資產/交易和資產持有者列表接受可選的 `asset_id` 查詢參數
除了現有的分頁/反壓限制之外，還用於預過濾。

## 授權和規範簽名

- 面向應用程序的 GET/POST 端點接受從 `METHOD\n/path\nsorted_query\nsha256(body)` 構建的可選規範請求標頭（`X-Iroha-Account`、`X-Iroha-Signature`）； Torii 在執行器驗證之前將它們包裝到 `QueryRequestWithAuthority` 中，因此它們鏡像 `/query`。
- SDK 助手在所有主要客戶端中提供：
  - JS/TS：`buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` 來自 `canonicalRequest.js`。
  - 斯威夫特：`CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`。
  - Android（Kotlin/Java）：`CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`。
- 示例片段：
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "i105...", method: "get", path: "/v2/accounts/i105.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v2/accounts/i105.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "i105...",
                                                  method: "get",
                                                  path: "/v2/accounts/i105.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("i105...", "get", "/v2/accounts/i105.../assets", "limit=5", ByteArray(0), signer)
```

## 端點庫存

### 帳戶權限 (`/v2/accounts/{id}/permissions`) — 涵蓋
- 處理程序：`handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)。
- DTO：`filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)。
- 路由器綁定：`Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- 測試：`crates/iroha_torii/tests/accounts_endpoints.rs:126` 和 `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`。
- 所有者：Torii 平台。
- 注意：響應是 Norito JSON 正文，其中包含 `items`/`total`，匹配 SDK 分頁助手。

### 別名 OPRF 評估 (`POST /v2/aliases/voprf/evaluate`) — 已覆蓋
- 處理程序：`handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)。
- DTO：`AliasVoprfEvaluateRequestDto`、`AliasVoprfEvaluateResponseDto`、`AliasVoprfBackendDto`
  （`crates/iroha_torii/src/routing.rs:809-865`）。
- 路由器綁定：`Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)。
- 測試：內聯處理程序測試 (`crates/iroha_torii/src/lib.rs:9945-9986`) 以及 SDK 覆蓋範圍
  （`javascript/iroha_js/test/toriiClient.test.js:72`）。
- 所有者：Torii 平台。
- 注意：響應面強制執行確定性十六進制和後端標識符； SDK 使用 DTO。

### 證明事件 SSE (`GET /v2/events/sse`) — 涵蓋
- 處理程序：`handle_v1_events_sse`，帶過濾器支持 (`crates/iroha_torii/src/routing.rs:14008-14133`)。
- DTO：`EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) 加上防爆濾波器接線。
- 路由器綁定：`Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- 測試：特定證明的 SSE 套件（`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`，
  `sse_proof_callhash.rs`、`sse_proof_verified_fields.rs`、`sse_proof_rejected_fields.rs`) 和管道 SSE 煙霧測試
  （`integration_tests/tests/events/sse_smoke.rs`）。
- 所有者：Torii 平台（運行時）、集成測試工作組（固定裝置）。
- 注：證明過濾器路徑經過端到端驗證；文檔位於 `docs/source/zk_app_api.md` 下。

### 合同生命週期 (`/v2/contracts/*`) — 涵蓋
- 處理程序：`handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)，
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)。
- DTO：`DeployContractDto`、`DeployAndActivateInstanceDto`、`ActivateInstanceDto`、`ContractCallDto`
  （`crates/iroha_torii/src/routing.rs:3124-3463`）。
- 路由器綁定：`Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- 測試：路由器/集成套件 `contracts_deploy_integration.rs`、`contracts_activate_integration.rs`、
  `contracts_instance_activate_integration.rs`、`contracts_call_integration.rs`、
  `contracts_instances_list_router.rs`。
- 所有者：擁有 Torii 平台的智能合約工作組。
- 注意：端點對簽名事務進行排隊並重用共享遙測指標（`handle_transaction_with_metrics`）。

### 驗證密鑰生命週期 (`/v2/zk/vk/*`) — 涵蓋
- 處理程序：`handle_post_vk_register`、`handle_post_vk_update`、`handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) 和 `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)。
- DTO：`ZkVkRegisterDto`、`ZkVkUpdateDto`、`ZkVkDeprecateDto`、`VkListQuery`、`ProofFindByIdQueryDto`
  （`crates/iroha_torii/src/routing.rs:3619-4279`）。
- 路由器綁定：`Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- 測試：`crates/iroha_torii/tests/zk_vk_get_integration.rs`，
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`，
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`。
- 所有者：ZK 工作組，提供 Torii 平台支持。
- 注意：DTO 與 SDK 引用的 Norito 架構保持一致；通過 `limits.rs` 強制執行速率限制。

### Nexus 連接 (`/v2/connect/*`) — 涵蓋（功能 `connect`）
- 處理程序：`handle_connect_session`、`handler_connect_session_delete`、`handle_connect_ws`、
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)。
- DTO：`ConnectSessionRequest`、`ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)、
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)。
- 路由器綁定：`Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)。
- 測試：`crates/iroha_torii/tests/connect_gating.rs`（功能門控、會話生命週期、WS 握手）和
  路由器功能矩陣覆蓋範圍 (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`)。
- 所有者：Nexus 連接工作組。
- 注意：通過 `limits::rate_limit_key` 跟踪的速率限制密鑰；遙測計數器提供 `connect.*` 指標。

### Kaigi 中繼遙測 — 涵蓋
- 處理程序：`handle_v1_kaigi_relays`、`handle_v1_kaigi_relay_detail`、
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  （`crates/iroha_torii/src/routing.rs:14510-14787`）。
- DTO：`KaigiRelaySummaryDto`、`KaigiRelaySummaryListDto`、
  `KaigiRelayDetailDto`、`KaigiRelayDomainMetricsDto`、
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)。
- 路由器綁定：`Torii::add_app_api_routes`
  （`crates/iroha_torii/src/lib.rs:6805-6840`）。
- 測試：`crates/iroha_torii/tests/kaigi_endpoints.rs`。
- 注意：SSE流在執行時重用全局廣播通道
  遙測剖面選通；響應模式記錄在
  `docs/source/torii/kaigi_telemetry_api.md`。

## 測試覆蓋率總結

- 路由器冒煙測試 (`crates/iroha_torii/tests/router_feature_matrix.rs`) 確保功能組合註冊每個
  路線和 OpenAPI 一代保持同步。
- 特定於端點的套件涵蓋帳戶查詢、合約生命週期、ZK 驗證密鑰、SSE 證明過濾器和 Nexus
  連接行為。
- SDK 奇偶校驗工具（JavaScript、Swift、Python）已使用 Alias VOPRF 和 SSE 端點；無需額外工作
  需要。

## 保持此鏡像最新

更新此頁面和源審核 (`docs/source/torii/app_api_parity_audit.md`)
每當 Torii 應用 API 行為發生變化時，SDK 所有者和外部讀者保持一致。