---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS 節點 ↔ 客戶端協議

本指南總結了規範協議定義
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)。
使用字節級 Norito 佈局和變更日誌的上游規範；門戶網站
副本使操作要點與 SoraFS 操作手冊的其餘部分保持接近。

## 提供商廣告和驗證

SoraFS 提供商八卦 `ProviderAdvertV1` 有效負載（請參閱
`crates/sorafs_manifest::provider_advert`) 由受監管運營商簽署。
廣告引腳發現元數據和多源護欄
Orchestrator 在運行時強制執行。

- **終身** — `issued_at < expires_at ≤ issued_at + 86 400 s`。供應商
  應每 12 小時刷新一次。
- **功能 TLV** — TLV 列表通告傳輸功能（Torii、
  QUIC+Noise、SoraNet 繼電器、供應商擴展）。未知代碼可能會被跳過
  當 `allow_unknown_capabilities = true` 時，遵循 GREASE 指南。
- **QoS 提示** — `availability` 層（熱/暖/冷），最大檢索
  延遲、並發限制和可選的流預算。 QoS 必須符合
  觀察遙測並通過入場審核。
- **端點和集合主題** — 具有 TLS/ALPN 的具體服務 URL
  元數據以及客戶端在構建時應訂閱的發現主題
  守衛組。
- **路徑多樣性策略** — `min_guard_weight`、AS/池扇出上限，以及
  `provider_failure_threshold` 使確定性的多點獲取成為可能。
- **配置文件標識符** - 提供者必須公開規範句柄（例如
  `sorafs.sf1@1.0.0`);可選的 `profile_aliases` 幫助舊客戶端遷移。

驗證規則拒絕零權益、空能力/端點/主題列表，
生命週期順序錯誤或缺少 QoS 目標。錄取信封比較
八卦更新之前的廣告和提案機構（`compare_core_fields`）。

### 範圍獲取擴展

支持範圍的提供程序包括以下元數據：

|領域|目的|
|--------|---------|
| `CapabilityType::ChunkRangeFetch` |聲明 `max_chunk_span`、`min_granularity` 和對齊/驗證標誌。 |
| `StreamBudgetV1` |可選並發/吞吐量信封（`max_in_flight`、`max_bytes_per_sec`、可選 `burst`）。需要範圍能力。 |
| `TransportHintV1` |有序的傳輸首選項（例如，`torii_http_range`、`quic_stream`、`soranet_relay`）。優先級為 `0–15`，重複項將被拒絕。 |

工具支持：

- 提供商廣告管道必須驗證範圍能力、流預算和
  在發出確定性負載進行審計之前傳輸提示。
- `cargo xtask sorafs-admission-fixtures` 捆綁規範多源
  降級賽程旁邊的廣告
  `fixtures/sorafs_manifest/provider_admission/`。
- 省略 `stream_budget` 或 `transport_hints` 的範圍功能廣告是
  在調度之前被 CLI/SDK 加載器拒絕，保留多源
  安全帶符合 Torii 錄取預期。

## 網關範圍端點

網關接受鏡像廣告元數據的確定性 HTTP 請求。

### `GET /v1/sorafs/storage/car/{manifest_id}`

|要求 |詳情 |
|-------------|---------|
| **標題** | `Range`（與塊偏移對齊的單個窗口）、`dag-scope: block`、`X-SoraFS-Chunker`、可選 `X-SoraFS-Nonce` 和強制 base64 `X-SoraFS-Stream-Token`。 |
| **回應** | `206` 與 `Content-Type: application/vnd.ipld.car`、描述服務窗口的 `Content-Range`、`X-Sora-Chunk-Range` 元數據以及回顯的分塊器/令牌標頭。 |
| **故障模式** | `416` 表示範圍未對齊，`401` 表示丟失/無效令牌，`429` 表示超出流/字節預算。 |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

使用相同標頭加上確定性塊摘要的單塊獲取。
當不需要 CAR 切片時，對於重試或取證下載很有用。

## 多源 Orchestrator 工作流程

當啟用 SF-6 多源獲取時（通過 `sorafs_fetch` 的 Rust CLI，
SDK 通過 `sorafs_orchestrator`）：

1. **收集輸入** — 解碼清單塊計劃，拉取最新廣告，
   並可選擇傳遞遙測快照（`--telemetry-json` 或
   `TelemetrySnapshot`）。
2. **構建記分板** — `Orchestrator::build_scoreboard` 評估
   資格和記錄拒絕原因； `sorafs_fetch --scoreboard-out`
   保留 JSON。
3. **調度塊** — `fetch_with_scoreboard`（或 `--plan`）強制範圍
   限制、流預算、重試/對等上限 (`--retry-budget`、
   `--max-peers`），並為每個請求發出清單範圍的流令牌。
4. **驗證收據** — 輸出包括 `chunk_receipts` 和
   `provider_reports`； CLI 摘要持續存在 `provider_reports`，
   `chunk_receipts` 和 `ineligible_providers` 用於證據包。

向運營商/SDK 提出的常見錯誤：

|錯誤 |描述 |
|--------|-------------|
| `no providers were supplied` |篩選後沒有符合條件的條目。 |
| `no compatible providers available for chunk {index}` |特定塊的範圍或預算不匹配。 |
| `retry budget exhausted after {attempts}` |增加 `--retry-budget` 或驅逐失敗的對等點。 |
| `no healthy providers remaining` |所有提供商在多次失敗後都被禁用。 |
| `streaming observer failed` |下游 CAR 編寫器中止。 |
| `orchestrator invariant violated` |捕獲清單、記分板、遙測快照和 CLI JSON 以進行分類。 |

## 遙測和證據

- 協調器發出的指標：  
  `sorafs_orchestrator_active_fetches`，`sorafs_orchestrator_fetch_duration_ms`，
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  （按清單/區域/提供商標記）。在配置中或通過設置 `telemetry_region`
  CLI 標記，以便儀表板按隊列分區。
- CLI/SDK 獲取摘要包括持久記分板 JSON、塊收據、
  和提供商報告，必須以 SF-6/SF-7 門的部署包形式提供。
- 網關處理程序暴露 `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  因此，SRE 儀表板可以將編排器決策與服務器行為關聯起來。

## CLI 和 REST 助手

- `iroha app sorafs pin list|show`、`alias list` 和 `replication list` 包裹
  pin-registry REST 端點並使用證明塊打印原始 Norito JSON
  為審計證據。
- `iroha app sorafs storage pin` 和 `torii /v1/sorafs/pin/register` 接受 Norito
  或 JSON 清單加上可選的別名證明和後繼者；格式錯誤的證明
  提高 `400`，用 `Warning: 110` 證明過時的樣張表面 `503`，以及
  硬過期的證明返回 `412`。
- `iroha app sorafs repair list` 鏡像修復隊列過濾器，同時
  `repair claim|complete|fail|escalate` 提交簽名的工作人員操作或斜線
  向 Torii 提出建議。 Slash 提案可能包含治理批准摘要
  （批准/拒絕/棄權票數加上approved_at/finalized_at
  時間戳）；當存在時，它必須滿足法定人數和爭議/上訴窗口，
  否則，該提案將一直存在爭議，直到截止日期投票結果出來為止。
- 修復列表和工作隊列選擇按照 SLA 截止日期、故障嚴重性和提供商待辦事項以及確定性決定因素（排隊時間、清單摘要、票證 ID）進行排序。
- 修復狀態響應包括包含 base64 Norito 的 `events` 數組
  `RepairTaskEventV1` 按出現次數排序的條目以進行審計跟踪；清單
  僅限於最近的轉換。
- `iroha app sorafs gc inspect|dry-run --data-dir=/var/lib/sorafs` 發出只讀信號
  來自本地艙單存儲的保留報告作為審計證據。
- REST 端點（`/v1/sorafs/pin`、`/v1/sorafs/aliases`、
  `/v1/sorafs/replication`）包括證明結構，以便客戶可以
  在採取行動之前根據最新的塊頭驗證數據。

## 參考文獻

- 規範規範：
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito 類型：`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI 幫助程序：`crates/iroha_cli/src/commands/sorafs.rs`，
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Orchestrator 箱：`crates/sorafs_orchestrator`
- 儀表板包：`dashboards/grafana/sorafs_fetch_observability.json`