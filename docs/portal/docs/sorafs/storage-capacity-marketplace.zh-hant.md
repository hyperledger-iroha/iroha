---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 868edd6aa7401c64b8757db188edb13aa8e6ca8959966b6fea02e44bc298c6b7
source_last_modified: "2026-01-05T09:28:11.910794+00:00"
translation_last_reviewed: 2026-02-07
id: storage-capacity-marketplace
title: SoraFS Storage Capacity Marketplace
sidebar_label: Capacity Marketplace
description: SF-2c plan for the capacity marketplace, replication orders, telemetry, and governance hooks.
translator: machine-google-reviewed
---

:::注意規範來源
:::

# SoraFS 存儲容量市場（SF-2c 草案）

SF-2c 路線圖項目引入了一個受監管的市場，其中存儲
提供商聲明承諾容量、接收復制訂單並賺取費用
與交付的可用性成正比。本文檔涵蓋了可交付成果
首次發布所需的內容並將其分解為可操作的軌道。

## 目標

- Express 提供商容量承諾（總字節數、每通道限制、到期時間）
  以可供治理、SoraNet 傳輸和 Torii 使用的可驗證形式。
- 根據聲明的容量、股權和數量在提供商之間分配 pin
  政策約束，同時保持確定性行為。
- 計量存儲交付（複製成功、正常運行時間、完整性證明）以及
  導出遙測數據以進行費用分配。
- 提供撤銷和爭議流程，以便不誠實的提供商可以
  受到處罰或除名。

## 領域概念

|概念|描述 |初始交付 |
|--------|-------------|---------------------|
| `CapacityDeclarationV1` | Norito 有效負載描述提供商 ID、分塊配置文件支持、承諾 GiB、通道特定限制、定價提示、質押承諾和到期日。 | `sorafs_manifest::capacity` 中的架構 + 驗證器。 |
| `ReplicationOrder` |治理髮布的指令將清單 CID 分配給一個或多個提供商，包括冗餘級別和 SLA 指標。 | Norito 架構與 Torii + 智能合約 API 共享。 |
| `CapacityLedger` |鏈上/鏈下註冊表跟踪活動容量聲明、複製訂單、性能指標和費用應計。 |具有確定性快照的智能合約模塊或鏈下服務存根。 |
| `MarketplacePolicy` |定義最低股權、審計要求和懲罰曲線的治理政策。 | `sorafs_manifest` + 治理文檔中的配置結構。 |

### 已實施的架構（狀態）

## 工作分解

### 1. 架構和註冊表層

|任務|所有者 |筆記|
|------|----------|--------|
|定義 `CapacityDeclarationV1`、`ReplicationOrderV1`、`CapacityTelemetryV1`。 |存儲團隊/治理|使用 Norito；包括語義版本控制和功能參考。 |
|在 `sorafs_manifest` 中實現解析器 + 驗證器模塊。 |存儲團隊|強制執行單調 ID、容量限制、權益要求。 |
|每個配置文件使用 `min_capacity_gib` 擴展分塊器註冊表元數據。 |工具工作組 |幫助客戶強制執行每個配置文件的最低硬件要求。 |
| `MarketplacePolicy` 草案文件包含入場護欄和處罰表。 |治理委員會|在文檔中與默認策略一起發布。 |

#### 架構定義（已實現）

- `CapacityDeclarationV1` 捕獲每個提供商簽署的容量承諾，包括規範分塊器句柄、功能參考、可選通道上限、定價提示、有效性窗口和元數據。驗證確保非零權益、規範句柄、重複數據刪除別名、聲明總數內的每通道上限以及單調 GiB 會計。 【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` 將清單綁定到具有冗餘目標、SLA 閾值和每個分配保證的治理髮布的分配；驗證器在 Torii 或註冊中心接收訂單之前強制執行規範的分塊句柄、唯一提供者和截止日期約束。 【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` 表示提供費用分配的紀元快照（聲明的與使用的 GiB、複製計數器、正常運行時間/PoR 百分比）。邊界檢查將利用率保持在聲明範圍內，百分比保持在 0 – 100% 範圍內。 【crates/sorafs_manifest/src/capacity.rs:476】
- 共享助手（`CapacityMetadataEntry`、`PricingScheduleV1`、通道/分配/SLA 驗證器）提供 CI 和下游工具可以重用的確定性密鑰驗證和錯誤報告。 【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` 現在通過 `/v1/sorafs/capacity/state` 呈現鏈上快照，結合確定性 Norito JSON 後面的提供商聲明和費用分類帳條目。 【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- 驗證覆蓋範圍執行規範句柄執行、重複檢測、每通道邊界、複製分配保護和遙測範圍檢查，以便回歸立即在 CI 中顯現。 【crates/sorafs_manifest/src/capacity.rs:792】
- 操作員工具：`sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` 將人類可讀的規範轉換為規範的 Norito 有效負載、base64 blob 和 JSON 摘要，以便操作員可以使用本地暫存 `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry` 和復制順序固定裝置驗證。 【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】參考夾具位於 `fixtures/sorafs_manifest/replication_order/`（`order_v1.json`、`order_v1.to`）中，並通過 `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` 生成。

### 2. 控制平面集成

|任務|所有者 |筆記|
|------|----------|--------|
|添加具有 Norito JSON 負載的 `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry`、`/v1/sorafs/capacity/orders` Torii 處理程序。 | Torii 團隊 |鏡像驗證器邏輯；重用 Norito JSON 幫助程序。 |
|將 `CapacityDeclarationV1` 快照傳播到 Orchestrator 記分板元數據和網關獲取計劃中。 |工具工作組/協調器團隊|使用容量參考擴展 `provider_metadata`，以便多源評分遵守通道限制。 |
|將復制訂單輸入編排器/網關客戶端以驅動分配和故障轉移提示。 |網絡 TL/網關團隊 |記分板構建器使用治理簽名的複制訂單。 |
| CLI 工具：使用 `capacity declare`、`capacity telemetry`、`capacity orders import` 擴展 `sorafs_cli`。 |工具工作組 |提供確定性 JSON + 記分板輸出。 |

### 3. 市場政策與治理

|任務|所有者 |筆記|
|------|----------|--------|
|批准 `MarketplacePolicy`（最低股權、處罰乘數、審計節奏）。 |治理委員會|在文檔中發布，捕獲修訂歷史記錄。 |
|添加治理掛鉤，以便議會可以批准、更新和撤銷聲明。 |治理委員會/智能合約團隊|使用 Norito 事件 + 清單攝取。 |
|實施與遙測 SLA 違規行為相關的處罰計劃（費用減少、保證金削減）。 |治理委員會/財政部|與 `DealEngine` 結算輸出對齊。 |
|記錄爭議流程和升級矩陣。 |文檔/治理|指向爭議 Runbook + CLI 幫助程序的鏈接。 |

### 4. 計量和費用分配

|任務|所有者 |筆記|
|------|----------|--------|
|展開 Torii 計量攝取以接受 `CapacityTelemetryV1`。 | Torii 團隊 |驗證 GiB 小時、PoR 成功、正常運行時間。 |
|更新 `sorafs_node` 計量管道以報告每訂單利用率 + SLA 統計數據。 |存儲團隊|與復制順序和分塊器句柄對齊。 |
|結算管道：將遙測+複製數據轉換為以異或計價的支出，生成可供治理的摘要，並記錄賬本狀態。 |財務/存儲團隊|電匯至交易引擎/金庫出口。 |
|導出儀表板/警報以測量運行狀況（攝取積壓、過時的遙測）。 |可觀察性|擴展 SF-6/SF-7 引用的 Grafana 包。 |

- Torii 現在公開 `/v1/sorafs/capacity/telemetry` 和 `/v1/sorafs/capacity/state` (JSON + Norito)，以便操作員可以提交紀元遙測快照，檢查員可以檢索規範賬本以進行審計或證據包裝。 【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- `PinProviderRegistry` 集成確保可通過同一端點訪問複製訂單； CLI 助手 (`sorafs_cli capacity telemetry --from-file telemetry.json`) 現在通過確定性哈希和別名解析來驗證/發布來自自動化運行的遙測數據。
- 計量快照生成固定到 `metering` 快照的 `CapacityTelemetrySnapshot` 條目，Prometheus 導出為 `docs/source/grafana_sorafs_metering.json` 處的準備導入 Grafana 板提供數據，以便計費團隊可以監控預計的 GiB·小時累積情況nano-SORA費用，以及實時SLA合規性。 【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- 啟用計量平滑後，快照包括 `smoothed_gib_hours` 和 `smoothed_por_success_bps`，因此運營商可以將 EMA 趨勢值與治理用於支付的原始計數器進行比較。 【crates/sorafs_node/src/metering.rs:401】

### 5. 爭議和撤銷處理

|任務|所有者 |筆記|
|------|----------|--------|
|定義 `CapacityDisputeV1` 有效負載（投訴人、證據、目標提供商）。 |治理委員會| Norito 架構 + 驗證器。 |
| CLI 支持提交爭議並作出回應（帶有證據附件）。 |工具工作組 |確保證據包的確定性散列。 |
|添加對重複違反 SLA 的自動檢查（自動升級為爭議）。 |可觀察性|警報閾值和治理掛鉤。 |
|文檔撤銷手冊（寬限期、撤出固定數據）。 |文檔/存儲團隊 |政策文檔和操作手冊的鏈接。 |

## 測試和 CI 要求- 所有新模式驗證器的單元測試 (`sorafs_manifest`)。
- 模擬集成測試：聲明→複製順序→計量→支付。
- CI 工作流程，用於重新生成樣本容量聲明/遙測並確保簽名保持同步（擴展 `ci/check_sorafs_fixtures.sh`）。
- 註冊表 API 的負載測試（模擬 10k 個提供商、100k 個訂單）。

## 遙測和儀表板

- 儀表板面板：
  - 每個提供商聲明的容量與使用的容量。
  - 複製訂單積壓和平均分配延遲。
  - SLA 合規性（正常運行時間百分比、PoR 成功率）。
  - 每個週期的費用累積和處罰。
- 警報：
  - 提供商低於最低承諾容量。
  - 複製順序卡住 > SLA。
  - 計量管道故障。

## 文檔交付成果

- 操作員指南，用於聲明容量、更新承諾和監控利用率。
- 審批申報、發布命令、處理糾紛的治理指南。
- 容量端點和復制順序格式的 API 參考。
- 開發人員的市場常見問題解答。

## GA 準備清單

路線圖項目**SF-2c**根據會計領域的具體證據進行生產推廣，
爭議處理和入職。使用以下工件來保持驗收標準
與實施同步。

### 每晚記賬和異或對賬
- 導出同一窗口的容量狀態快照和異或賬本導出，然後運行：
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  幫助程序因缺少/多付的和解或罰款而退出非零，並發出 Prometheus
  文本文件摘要。
- 警報 `SoraFSCapacityReconciliationMismatch`（在 `dashboards/alerts/sorafs_capacity_rules.yml` 中）
  每當調節指標報告差距時就會觸發；儀表板位於下面
  `dashboards/grafana/sorafs_capacity_penalties.json`。
- 將 JSON 摘要和哈希值存檔在 `docs/examples/sorafs_capacity_marketplace_validation/` 下
  與治理包一起。

### 爭議和削減證據
- 通過 `sorafs_manifest_stub capacity dispute` 提出爭議（測試：
  `cargo test -p sorafs_car --test capacity_cli`），因此有效負載保持規範。
- 運行 `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` 和懲罰
  套件（`record_capacity_telemetry_penalises_persistent_under_delivery`）來證明爭議和
  斜杠確定性地重播。
- 遵循 `docs/source/sorafs/dispute_revocation_runbook.md` 進行證據捕獲和升級；
  將罷工批准鏈接回驗證報告。

### 提供商入職和退出冒煙測試
- 使用 `sorafs_manifest_stub capacity ...` 重新生成聲明/遙測工件並重播
  提交前進行 CLI 測試 (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)。
- 通過 Torii (`/v1/sorafs/capacity/declare`) 提交，然後捕獲 `/v1/sorafs/capacity/state` 加
  Grafana 屏幕截圖。按照 `docs/source/sorafs/capacity_onboarding_runbook.md` 中的退出流程進行操作。
- 將簽名的工件和對賬輸出存檔在內部
  `docs/examples/sorafs_capacity_marketplace_validation/`。

## 依賴關係和排序

1. 完成 SF-2b（准入政策）——市場依賴於經過審查的提供商。
2. 在Torii集成之前實現模式+註冊表層（本文檔）。
3. 在啟用支付之前完成計量管道。
4. 最後一步：一旦計量數據在暫存階段得到驗證，即可啟用治理控制的費用分配。

應在路線圖中參考本文檔來跟踪進展情況。一旦每個主要部分（架構、控制平面、集成、計量、爭議處理）達到功能完成狀態，就更新路線圖。