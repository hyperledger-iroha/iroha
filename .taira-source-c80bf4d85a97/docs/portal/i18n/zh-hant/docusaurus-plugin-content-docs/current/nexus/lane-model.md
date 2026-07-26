---
id: nexus-lane-model
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/lane-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus lane model
description: Logical lane taxonomy, configuration geometry, and world-state merge rules for Sora Nexus.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Nexus 通道模型和 WSV 分區

> **狀態：** NX-1 可交付成果 — 通道分類、配置幾何結構和存儲佈局已準備好實施。  
> **所有者：** Nexus 核心工作組、治理工作組  
> **路線圖參考：** `roadmap.md` 中的 NX-1

此門戶頁面反映了規範的 `docs/source/nexus_lanes.md` 簡介，因此 Sora
Nexus 操作員、SDK 所有者和審閱者無需閱讀車道指南即可
深入研究 mono-repo 樹。目標架構保持世界狀態
確定性，同時允許單獨的數據空間（通道）公共運行或
具有隔離工作負載的私有驗證器集。

## 概念

- **Lane:** Nexus 賬本的邏輯分片，具有自己的驗證器集和
  執行積壓。由穩定的 `LaneId` 標識。
- **數據空間：** 治理桶分組一個或多個共享通道
  合規性、路由和結算政策。
- **車道清單：** 描述驗證器、DA 的治理控制元數據
  策略、gas 代幣、結算規則和路由權限。
- **全球承諾：** 由車道發出的總結新狀態根的證明，
  結算數據和可選的跨車道傳輸。全球 NPoS 環
  訂單承諾。

## 泳道分類

車道類型規範地描述了它們的可見性、治理面和
結算掛鉤。配置幾何 (`LaneConfig`) 捕獲這些
屬性，以便節點、SDK 和工具可以推斷佈局，而無需
定制邏輯。

|車道類型|能見度|驗證者會員資格 | WSV 暴露 |違約治理 |落戶政策 |典型用途|
|------------|------------|----------------------|------------------------|--------------------|--------------------|-------------|
| `default_public` |公共|無需許可（全球股權）|完整狀態副本 | SORA 議會 | `xor_global` |基線公共分類賬 |
| `public_custom` |公共|無需許可或股權門禁 |完整狀態副本 |股權加權模塊| `xor_lane_weighted` |高通量公共應用|
| `private_permissioned` |限制|固定驗證器集（政府批准）|承諾及證明|聯邦理事會| `xor_hosted_custody` | CBDC、聯盟工作負載 |
| `hybrid_confidential` |限制|混合會員資格；包裝 ZK 樣張 |承諾+選擇性披露|可編程貨幣模塊| `xor_dual_fund` |保護隱私的可編程貨幣|

所有車道類型必須聲明：

- 數據空間別名 — 綁定合規策略的人類可讀分組。
- 治理句柄 — 通過 `Nexus.governance.modules` 解析的標識符。
- 結算句柄 — 結算路由器用於借方 XOR 的標識符
  緩衝區。
- 可選的遙測元數據（描述、聯繫人、業務領域）浮出水面
  通過 `/status` 和儀表板。

## 車道配置幾何形狀 (`LaneConfig`)

`LaneConfig` 是從經過驗證的通道目錄派生的運行時幾何圖形。它
**不**取代治理清單；相反，它提供了確定性
每個配置通道的存儲標識符和遙測提示。

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- 無論何時配置，`LaneConfig::from_catalog` 都會重新計算幾何形狀
  已加載（`State::set_nexus`）。
- 別名被清理為小寫字母；連續的非字母數字
  字符折疊成 `_`。如果別名產生一個空的 slug，我們就會回退
  至 `lane{id}`。
- `shard_id` 源自目錄元數據密鑰 `da_shard_id`（默認
  到 `lane_id`）並驅動持久分片游標日誌以保持 DA 重放
  重新啟動/重新分片時具有確定性。
- 密鑰前綴確保 WSV 保持每通道密鑰範圍不相交，即使
  共享相同的後端。
- Kura 段名稱在主機之間具有確定性；審核員可以交叉檢查
  無需定制工具即可對目錄和清單進行分段。
- 合併段（`lane_{id:03}_merge`）保存最新的合併提示根和
  全球國家對該車道的承諾。

## 世界狀態劃分

- 邏輯 Nexus 世界狀態是每通道狀態空間的並集。公共
  車道保持滿狀態；私人/機密通道導出默克爾/承諾
  根到合併分類賬。
- MV 存儲為每個鍵添加 4 字節通道前綴
  `LaneConfigEntry::key_prefix`，產生諸如`[00 00 00 01] ++之類的鍵
  打包密鑰`。
- 共享表（賬戶、資產、觸發器、治理記錄）因此存儲
  條目按通道前綴分組，保持範圍掃描的確定性。
- 合併賬本元數據鏡像相同的佈局：每個通道寫入合併提示
  根並將全局狀態根減少為 `lane_{id:03}_merge`，從而允許
  當車道退役時有針對性的保留或驅逐。
- 跨通道索引（賬戶別名、資產註冊表、治理清單）
  存儲明確的車道前綴，以便操作員可以快速協調條目。
- **保留政策** — 公共車道保留完整的街區主體；僅限承諾
  檢查站後車道可能會擠滿較舊的屍體，因為承諾是
  權威的。機密通道將密文日誌保存在專用的通道中
  段以避免阻塞其他工作負載。
- **工具** — 維護實用程序（`kagami`、CLI 管理命令）應該
  在公開指標、Prometheus 標籤時引用 slugged 命名空間，或者
  歸檔 Kura 片段。

## 路由和 API

- Torii REST/gRPC 端點接受可選的 `lane_id`；缺席意味著
  `lane_default`。
- SDK 使用通道選擇器將用戶友好的別名映射到 `LaneId`
  車道目錄。
- 路由規則在經過驗證的目錄上運行，並且可以選擇車道和
  數據空間。 `LaneConfig` 為儀表板和
  日誌。

## 結算及費用

- 每個通道向全局驗證器集支付異或費用。車道可能會收集原生
  天然氣代幣，但必須與承諾一起託管異或等價物。
- 結算證明包括金額、轉換元數據和託管證明
  （例如，轉賬至全球費用庫）。
- 統一結算路由器（NX-3）使用同一通道借記緩衝區
  前綴，因此沉降遙測與存儲幾何結構一致。

## 治理

- 通道通過目錄聲明其治理模塊。 `LaneConfigEntry`
  攜帶原始別名和 slug 以保留遙測和審計跟踪
  可讀。
- Nexus 註冊表分發簽名的通道清單，其中包括
  `LaneId`，數據空間綁定，治理句柄，結算句柄，以及
  元數據。
- 運行時升級掛鉤繼續執行治理策略
  （默認情況下為 `gov_upgrade_id`）並通過遙測橋記錄差異
  （`nexus.config.diff` 事件）。

## 遙測和狀態

- `/status` 公開通道別名、數據空間綁定、治理句柄和
  沉降概況，源自目錄和 `LaneConfig`。
- 調度程序指標 (`nexus_scheduler_lane_teu_*`) 渲染通道別名/slugs
  運營商可以快速繪製積壓訂單和 TEU 壓力圖。
- `nexus_lane_configured_total` 計算派生車道條目的數量，並且
  配置更改時重新計算。每當以下情況時，遙測都會發出簽名差異
  車道幾何形狀發生變化。
- 數據空間積壓量表包括別名/描述元數據以提供幫助
  運營商將隊列壓力與業務領域聯繫起來。

## 配置和 Norito 類型

- `LaneCatalog`、`LaneConfig` 和 `DataSpaceCatalog` 實時存在
  `iroha_data_model::nexus` 並提供 Norito 格式結構
  清單和 SDK。
- `LaneConfig` 存在於 `iroha_config::parameters::actual::Nexus` 中並派生
  自動從目錄中；它不需要 Norito 編碼，因為它
  是一個內部運行時助手。
- 面向用戶的配置 (`iroha_config::parameters::user::Nexus`)
  繼續接受聲明性通道和數據空間描述符；現在解析
  導出幾何圖形並拒絕無效別名或重複的車道 ID。

## 出色的工作

- 將結算路由器更新 (NX-3) 與新幾何圖形集成，以便 XOR 緩衝區
  借方和收據均由通道標記標記。
- 擴展管理工具以列出列族、緊湊退役通道以及
  使用 slugged 命名空間檢查每個通道的塊日誌。
- 最終確定合併算法（排序、修剪、衝突檢測）並
  附加回歸裝置以進行跨車道重播。
- 添加白名單/黑名單和可編程貨幣的合規掛鉤
  政策（在 NX-12 下跟踪）。

---

*本頁面將繼續跟踪 NX-1 的後續行動，如 NX-2 至 NX-18 著陸。
請在 `roadmap.md` 或治理跟踪器中提出未解決的問題，以便
門戶與規範文檔保持一致。 *