---
id: nexus-fee-model
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus fee model updates
description: Mirror of `docs/source/nexus_fee_model.md`, documenting the lane settlement receipts and reconciliation surfaces.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
此頁面鏡像 `docs/source/nexus_fee_model.md`。在遷移日語、希伯來語、西班牙語、葡萄牙語、法語、俄語、阿拉伯語和烏爾都語翻譯時，保持兩個副本對齊。
:::

# Nexus 費用模型更新

統一結算路由器現在捕獲確定性的每通道收據，因此
運營商可以根據 Nexus 費用模型調節天然氣借方。

- 對於完整的路由器架構、緩衝區策略、遙測矩陣和部署
  測序參見 `docs/settlement-router.md`。該指南解釋瞭如何
  此處記錄的參數與 NX-3 路線圖可交付成果以及 SRE 如何联繫起來
  應監控生產中的路由器。
- 天然氣資產配置 (`pipeline.gas.units_per_gas`) 包括
  `twap_local_per_xor` 十進制，一個 `liquidity_profile` (`tier1`, `tier2`,
  或 `tier3`)，以及 `volatility_class` (`stable`、`elevated`、`dislocated`)。
  這些標誌饋送到結算路由器，因此生成的 XOR
  quote 與車道的規範 TWAP 和理髮等級相匹配。
- 每筆支付gas的交易都會記錄一個`LaneSettlementReceipt`。  每個
  收據存儲調用者提供的源標識符、本地微量、
  立即到期的 XOR、理髮後預計的 XOR、已實現的
  方差 (`xor_variance_micro`) 和區塊時間戳（以毫秒為單位）。
- 塊執行聚合每個通道/數據空間的收據並發布它們
  通過 `/v1/sumeragi/status` 中的 `lane_settlement_commitments`。  總計
  公開 `total_local_micro`、`total_xor_due_micro` 和
  `total_xor_after_haircut_micro` 在每晚的塊上求和
  調節出口。
- 新的 `total_xor_variance_micro` 計數器跟踪安全裕度是多少
  消耗（應有的異或和理髮後期望之間的差異），
  和 `swap_metadata` 記錄確定性轉換參數
  （TWAP、epsilon、流動性概況和波動性類別），以便審計師可以
  驗證獨立於運行時配置的報價輸入。

消費者可以在現有車道旁觀看 `lane_settlement_commitments`
和數據空間承諾快照，以驗證費用緩衝區、理髮層、
和交換執行匹配配置的 Nexus 費用模型。