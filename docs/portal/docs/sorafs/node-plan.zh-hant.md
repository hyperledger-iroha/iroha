---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3852a0f039b664344f9cbce7d2514172cfe97cd838b68755f764d4fe183b22cc
source_last_modified: "2026-01-05T09:28:11.898207+00:00"
translation_last_reviewed: 2026-02-07
id: node-plan
title: SoraFS Node Implementation Plan
sidebar_label: Node Implementation Plan
description: Translate the SF-3 storage roadmap into actionable engineering work with milestones, tasks, and test coverage.
translator: machine-google-reviewed
---

:::注意規範來源
:::

SF-3 提供了第一個可運行的 `sorafs-node` 包，可將 Iroha/Torii 進程轉變為 SoraFS 存儲提供程序。在對可交付成果進行排序時，請將此計劃與[節點存儲指南](node-storage.md)、[提供商准入政策](provider-admission-policy.md) 和[存儲容量市場路線圖](storage-capacity-marketplace.md) 一起使用。

## 目標範圍（里程碑 M1）

1. **塊存儲集成。 ** 使用持久後端包裝 `sorafs_car::ChunkStore`，該後端在配置的數據目錄中存儲塊字節、清單和 PoR 樹。
2. **網關端點。 ** 公開 Norito HTTP 端點，用於 Torii 進程中的 pin 提交、塊獲取、PoR 採樣和存儲遙測。
3. **配置管道。 ** 添加通過 `iroha_config`、`iroha_core` 和 `iroha_torii` 連接的 `SoraFsStorage` 配置結構（啟用標誌、容量、目錄、並發限制）。
4. **配額/調度。 ** 通過背壓強制執行操作員定義的磁盤/並行度限制和隊列請求。
5. **遙測。 ** 發出 pin 成功、塊獲取延遲、容量利用率和 PoR 採樣結果的指標/日誌。

## 工作分解

### A. 板條箱和模塊結構

|任務|所有者 |筆記|
|------|----------|--------|
|使用以下模塊創建 `crates/sorafs_node`：`config`、`store`、`gateway`、`scheduler`、`telemetry`。 |存儲團隊|重新導出可重用類型以進行 Torii 集成。 |
|實現從 `SoraFsStorage` 映射的 `StorageConfig`（用戶 → 實際 → 默認值）。 |存儲團隊/配置工作組 |確保 Norito/`iroha_config` 層保持確定性。 |
|提供 `NodeHandle` 外觀 Torii 用於提交 pin/fetch。 |存儲團隊|封裝存儲內部結構和異步管道。 |

### B. 持久塊存儲

|任務|所有者 |筆記|
|------|----------|--------|
|使用磁盤清單索引 (`sled`/`sqlite`) 構建磁盤後端包裝 `sorafs_car::ChunkStore`。 |存儲團隊|確定性佈局：`<data_dir>/<manifest_cid>/chunk_{idx}.bin`。 |
|使用 `ChunkStore::sample_leaves` 維護 PoR 元數據（64KiB/4KiB 樹）。 |存儲團隊|支持重啟後回放；腐敗問題快速失敗。 |
|在啟動時實施完整性重放（重新散列清單、修剪不完整的引腳）。 |存儲團隊|阻止 Torii 啟動，直到重播完成。 |

### C. 網關端點

|端點 |行為 |任務 |
|----------|------------|--------|
| `POST /sorafs/pin` |接受 `PinProposalV1`，驗證清單，隊列攝取，使用清單 CID 進行響應。 |驗證塊配置文件、強製配額、通過塊存儲傳輸數據。 |
| `GET /sorafs/chunks/{cid}` + 範圍查詢 |使用 `Content-Chunker` 標頭提供塊字節；遵守範圍能力規範。 |使用調度程序 + 流預算（與 SF-2d 範圍功能相關）。 |
| `POST /sorafs/por/sample` |對清單和退貨證明包運行 PoR 採樣。 |重用塊存儲采樣，使用 Norito JSON 有效負載進行響應。 |
| `GET /sorafs/telemetry` |摘要：容量、PoR 成功、獲取錯誤計數。 |為儀表板/操作員提供數據。 |

運行時管道通過 `sorafs_node::por` 線程 PoR 交互：跟踪器記錄每個 `PorChallengeV1`、`PorProofV1` 和 `AuditVerdictV1`，因此 `CapacityMeter` 指標反映治理結論，無需定制 Torii邏輯.【crates/sorafs_node/src/scheduler.rs#L147】

實施注意事項：

- 將 Torii 的 Axum 堆棧與 `norito::json` 有效負載結合使用。
- 添加 Norito 響應模式（`PinResultV1`、`FetchErrorV1`、遙測結構）。

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` 現在公開積壓深度以及最舊的紀元/截止日期和
  每個提供商最近的成功/失敗時間戳，由
  `sorafs_node::NodeHandle::por_ingestion_status` 和 Torii 記錄
  `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` 儀表儀表板.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. 調度程序和配額執行

|任務|詳情 |
|------|---------|
|磁盤配額|跟踪磁盤上的字節；當超過 `max_capacity_bytes` 時拒絕新的引腳。為未來的政策提供驅逐鉤子。 |
|獲取並發|全局信號量 (`max_parallel_fetches`) 加上來自 SF-2d 範圍上限的每個提供商的預算。 |
|引腳隊列 |限制未完成的攝取作業；公開隊列深度的 Norito 狀態端點。 |
| PoR 節奏 |由 `por_sample_interval_secs` 驅動的後台工作者。 |

### E. 遙測和記錄

指標（Prometheus）：

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds`（帶有 `result` 標籤的直方圖）
- `torii_sorafs_storage_bytes_used`、`torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`、`torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`、`torii_sorafs_storage_por_samples_failed_total`

日誌/事件：

- 用於治理攝取的結構化 Norito 遙測 (`StorageTelemetryV1`)。
- 當利用率 >90% 或 PoR 故障連續超過閾值時發出警報。

### F. 測試策略

1. **單元測試。 ** 塊存儲持久性、配額計算、調度程序不變量（請參閱 `crates/sorafs_node/src/scheduler.rs`）。  
2. **集成測試** (`crates/sorafs_node/tests`)。 Pin → 取回往返、重啟恢復、配額拒絕、PoR 採樣證明驗證。  
3. **Torii 集成測試。 ** 在啟用存儲的情況下運行 Torii，通過 `assert_cmd` 執行 HTTP 端點。  
4. **混亂路線圖。 ** 未來的演練將模擬磁盤耗盡、IO 緩慢、提供程序刪除。

## 依賴關係

- SF-2b 准入策略 — 確保節點在發布廣告之前驗證准入信封。  
- SF-2c 容量市場 — 將遙測技術與容量聲明聯繫起來。  
- SF-2d 廣告擴展 — 消耗範圍能力 + 可用的流預算。

## 里程碑退出標準

- `cargo run -p sorafs_node --example pin_fetch` 適用於本地裝置。  
- Torii 使用 `--features sorafs-storage` 構建並通過集成測試。  
- 使用默認配置+ CLI 示例更新了文檔（[節點存儲指南](node-storage.md)）；提供操作手冊。  
- 遙測在暫存儀表板中可見；針對容量飽和和 PoR 故障配置的警報。

## 文檔和運營交付成果

- 使用默認配置、CLI 用法和故障排除步驟更新[節點存儲參考](node-storage.md)。  
- 隨著 SF-3 的發展，保持[節點操作運行手冊](node-operations.md) 與實施保持一致。  
- 在開發人員門戶內發布 `/sorafs/*` 端點的 API 參考，並在 Torii 處理程序登陸後將它們連接到 OpenAPI 清單中。