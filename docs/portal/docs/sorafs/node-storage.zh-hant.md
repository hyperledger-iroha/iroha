---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/node-storage.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffa884bf745ab5f79c20d4b20baaba842878301dc56a66463c4520275ce4fd0b
source_last_modified: "2026-01-05T09:28:11.899124+00:00"
translation_last_reviewed: 2026-02-07
id: node-storage
title: SoraFS Node Storage Design
sidebar_label: Node Storage Design
description: Storage architecture, quotas, and lifecycle hooks for Torii nodes hosting SoraFS data.
translator: machine-google-reviewed
---

:::注意規範來源
:::

## SoraFS節點存儲設計（草案）

本說明細化了 Iroha (Torii) 節點如何選擇加入 SoraFS 數據
可用性層並專用於本地磁盤片用於存儲和服務
大塊。它補充了 `sorafs_node_client_protocol.md` 發現規範，並且
SF-1b 夾具通過概述存儲端架構、資源來工作
必須落在節點和網關中的控制和配置管道
代碼路徑。實際操作員演習現場
[節點操作操作手冊](./node-operations)。

### 目標

- 允許任何驗證器或輔助 Iroha 進程將備用磁盤公開為
  SoraFS 提供者不影響核心賬本職責。
- 保持存儲模塊確定性和 Norito 驅動：清單、
  塊計劃、可檢索性證明 (PoR) 根和提供商廣告是
  真相的來源。
- 強制執行運營商定義的配額，以便節點無法耗儘自己的資源
  接受過多的 pin 或獲取請求。
- 表面健康/遙測（PoR 採樣、塊獲取延遲、磁盤壓力）
  回到治理和客戶。

### 高層架構

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

關鍵模塊：

- **網關**：公開 Norito HTTP 端點以進行 pin 建議、塊獲取
  請求、PoR 採樣和遙測。它驗證 Norito 有效負載和
  將請求編組到塊存儲中。重用現有的 Torii HTTP 堆棧
  以避免新的守護進程。
- **Pin 註冊表**：`iroha_data_model::sorafs` 中跟踪的清單 pin 狀態
  和 `iroha_core`。當清單被接受時，註冊表會記錄
  清單摘要、塊計劃摘要、PoR 根和提供者能力標誌。
- **塊存儲**：攝取的磁盤支持的 `ChunkStore` 實現
  簽署清單，使用 `ChunkProfile::DEFAULT` 實現塊計劃，以及
  在確定性佈局下保留塊。每個塊都與一個相關聯
  內容指紋和 PoR 元數據，因此採樣可以重新驗證，無需
  重新讀取整個文件。
- **配額/調度程序**：強制執行操作員配置的限制（最大磁盤字節，
  最大未完成引腳、最大並行讀取、塊 TTL）和坐標
  IO 因此節點的賬本職責不會匱乏。調度器也是
  負責使用有限的 CPU 提供 PoR 證明和採樣請求。

### 配置

向 `iroha_config` 添加新部分：

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # optional human friendly tag
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`：參與切換。當 false 時，網關返回 503
  存儲端點和節點不在發現中通告。
- `data_dir`：塊數據、PoR 樹和獲取遙測數據的根目錄。
  默認為 `<iroha.data_dir>/sorafs`。
- `max_capacity_bytes`：固定塊數據的硬限制。後台任務
  當達到限制時拒絕新的引腳。
- `max_parallel_fetches`：調度程序強制執行並發上限以平衡
  帶寬/磁盤 IO 與驗證器工作負載的比較。
- `max_pins`：應用之前節點接受的清單引腳的最大數量
  驅逐/背壓。
- `por_sample_interval_secs`：自動 PoR 採樣作業的節奏。每份工作
  樣本 `N` 離開（可根據清單進行配置）並發出遙測事件。
  治理可以通過設置容量元數據來確定性地擴展 `N`
  密鑰 `profile.sample_multiplier`（整數 `1-4`）。該值可能是單個
  數字/字符串或具有每個配置文件覆蓋的對象，例如
  `{"default":2,"sorafs.sf2@1.0.0":3}`。
- `adverts`：提供商廣告生成器用來填充的結構
  `ProviderAdvertV1` 字段（權益指針、QoS 提示、主題）。如果省略
  節點使用治理註冊表中的默認值。

配置管道：

- `[sorafs.storage]` 在 `iroha_config` 中定義為 `SorafsStorage`，並且是
  從節點配置文件加載。
- `iroha_core` 和 `iroha_torii` 將存儲配置線程到網關中
  啟動時的構建器和塊存儲。
- 存在開發/測試環境覆蓋（`SORAFS_STORAGE_*`、`SORAFS_STORAGE_PIN_*`），但是
  生產部署應依賴配置文件。

### CLI 實用程序

雖然 Torii 的 HTTP 表面仍在接線，但 `sorafs_node` 板條箱運送了
精簡 CLI，以便操作員可以針對持久性數據編寫攝取/導出演練腳本
後端.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` 需要 Norito 編碼的清單 `.to` 文件以及匹配的有效負載
  字節。它根據清單的分塊配置文件重建塊計劃，
  強制摘要奇偶校驗，保留塊文件，並可選擇發出
  `chunk_fetch_specs` JSON blob，以便下游工具可以對
  佈局。
- `export` 接受清單 ID 並將存儲的清單/有效負載寫入磁盤
  （使用可選的計劃 JSON），因此燈具在不同環境中保持可重複性。

這兩個命令都將 Norito JSON 摘要打印到標準輸出，從而可以輕鬆通過管道輸入
腳本。 CLI 包含集成測試，以確保清單和
在 Torii API 落地之前有效負載乾淨利落地往返。 【crates/sorafs_node/tests/cli.rs:1】

> HTTP 奇偶校驗
>
> Torii 網關現在公開由相同支持的只讀幫助程序
> `NodeHandle`：
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — 返回存儲的
> Norito 清單（base64）和摘要/元數據。 【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — 返回確定性
> 用於下游工具的塊計劃 JSON (`chunk_fetch_specs`)。 【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> 這些端點鏡像 CLI 輸出，以便管道可以從本地切換
> 無需更改解析器即可編寫HTTP探針腳本。 【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### 節點生命週期

1. **啟動**：
   - 如果啟用存儲，節點將使用以下命令初始化塊存儲：
     配置目錄和容量。這包括驗證或創建
     PoR 清單數據庫並將固定清單重播到熱緩存。
   - 註冊 SoraFS 網關路由（Norito JSON POST/GET 引腳端點，
     獲取、PoR 樣本、遙測）。
   - 生成 PoR 採樣工作人員和配額監視器。
2. **發現/廣告**：
   - 使用當前容量/運行狀況生成 `ProviderAdvertV1` 文件，簽名
     使用理事會批准的密鑰，並通過發現頻道發布。
     可用的。
3. **固定工作流程**：
   - 網關收到簽名清單（包括 chunk plan、PoR root、council
     簽名）。驗證別名列表（需要 `sorafs.sf1@1.0.0`）並
     確保塊計劃與清單元數據匹配。
   - 檢查配額。如果超出容量/引腳限制，請響應
     策略錯誤（Norito 結構）。
   - 將塊數據流式傳輸到 `ChunkStore`，在我們攝取時驗證摘要。
     更新 PoR 樹並將清單元數據存儲在註冊表中。
4. **獲取工作流程**：
   - 服務來自磁盤的塊範圍請求。調度程序強制執行
     `max_parallel_fetches` 並在飽和時返回 `429`。
   - 發出結構化遙測數據 (Norito JSON)，包含延遲、服務字節數和
     下游監控的錯誤計數。
5. **PoR 採樣**：
   - 工作人員選擇與重量成比例的清單（例如，存儲的字節數）和
     使用塊存儲的 PoR 樹運行確定性採樣。
   - 保留治理審計的結果並將摘要包含在提供商中
     廣告/遙測端點。
6. **驅逐/配額執行**：
   - 當達到容量時，節點默認拒絕新的引腳。可選地，
     操作員可以配置驅逐策略（例如，基於 TTL、LRU）
     治理模式已達成一致；目前，該設計假設有嚴格的配額，並且
     操作員發起的取消固定操作。

### 容量聲明和調度集成- Torii 現在從 `/v2/sorafs/capacity/declare` 中繼 `CapacityDeclarationRecord` 更新
  到嵌入式 `CapacityManager`，因此每個節點都會構建其自身的內存視圖
  已提交的分塊器和車道分配。管理器公開只讀快照
  用於遙測 (`GET /v2/sorafs/capacity/state`) 並強制按配置文件或按通道
  接受新訂單前的預約。 【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- `/v2/sorafs/capacity/schedule` 端點接受治理頒發的 `ReplicationOrderV1`
  有效負載。當訂單針對本地提供商時，經理會檢查
  重複調度，驗證分塊器/通道容量，保留切片，以及
  返回 `ReplicationPlan` 描述剩餘容量，以便編排工具
  可以繼續攝入。其他供應商的訂單均通過
  `ignored` 響應以簡化多操作員工作流程。 【crates/iroha_torii/src/routing.rs:4845】
- 完成掛鉤（例如，在攝取成功後觸發）命中
  `POST /v2/sorafs/capacity/complete` 通過以下方式釋放預訂
  `CapacityManager::complete_order`。響應包括 `ReplicationRelease`
  快照（剩餘總數、分塊器/通道殘差），因此編排工具可以
  無需輪詢即可對下一個訂單進行排隊。後續工作會將其連接到塊中
  一旦攝取邏輯落地，就存儲管道。 【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- 嵌入式 `TelemetryAccumulator` 可以通過以下方式進行變異
  `NodeHandle::update_telemetry`，讓後台工作人員記錄 PoR/正常運行時間樣本
  並最終導出規範的 `CapacityTelemetryV1` 有效負載，而無需觸及
  調度器內部結構。 【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### 集成和未來的工作

- **治理**：通過存儲遙測擴展 `sorafs_pin_registry_tracker.md`
  （PoR 成功率、磁盤利用率）。入學政策可能要求最低
  接受廣告之前的容量或最低 PoR 成功率。
- **客戶端 SDK**：公開新的存儲配置（磁盤限制、別名），以便
  管理工具可以通過編程方式引導節點。
- **遙測**：與現有指標堆棧集成（Prometheus /
  OpenTelemetry），因此存儲指標出現在可觀察性儀表板中。
- **安全性**：在專用異步任務池中運行存儲模塊
  背壓並考慮通過 io_uring 或 tokio 的沙箱塊讀取
  有界池，以防止惡意客戶端耗盡資源。

這種設計使存儲模塊保持可選性和確定性，同時給出
操作員參與 SoraFS 數據可用性所需的旋鈕
層。實施它將涉及 `iroha_config`、`iroha_core`、
`iroha_torii` 和 Norito 網關，以及提供商廣告工具。