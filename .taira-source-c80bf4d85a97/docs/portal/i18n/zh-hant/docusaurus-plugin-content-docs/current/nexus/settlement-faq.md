---
id: nexus-settlement-faq
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Settlement FAQ
description: Operator-facing answers covering settlement routing, XOR conversion, telemetry, and audit evidence.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

此頁面鏡像內部結算常見問題解答 (`docs/source/nexus_settlement_faq.md`)
因此門戶讀者可以查看相同的指南，而無需深入研究
單一倉庫。它解釋了結算路由器如何處理支出、哪些指標
來監控，以及 SDK 應如何集成 Norito 有效負載。

## 亮點

1. **通道映射**——每個數據空間聲明一個`settlement_handle`
   （`xor_global`、`xor_lane_weighted`、`xor_hosted_custody` 或
   `xor_dual_fund`）。請參閱下面的最新車道目錄
   `docs/source/project_tracker/nexus_config_deltas/`。
2. **確定性轉換** — 路由器通過 XOR 將所有結算轉換為 XOR
   治理批准的流動性來源。專用通道為 XOR 緩衝區預先提供資金；
   僅當緩衝區偏離政策時才適用折扣。
3. **遙測** — 手錶 `nexus_settlement_latency_seconds`、轉換計數器、
   和理髮儀。儀表板位於 `dashboards/grafana/nexus_settlement.json`
   以及 `dashboards/alerts/nexus_audit_rules.yml` 中的警報。
4. **證據** — 存檔配置、路由器日誌、遙測導出和
   審計的調節報告。
5. **SDK 責任** — 每個 SDK 必須公開結算助手、車道 ID、
   和 Norito 有效負載編碼器，以與路由器保持奇偶校驗。

## 流程示例

|車道類型|需要捕捉的證據|它證明了什麼 |
|----------|--------------------------------|----------------|
|私人 `xor_hosted_custody` |路由器日誌 + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC 緩衝借方確定性 XOR 和削減保持在政策範圍內。 |
|公共 `xor_global` |路由器日誌 + DEX/TWAP 參考 + 延遲/轉換指標 |共享流動性路徑以公佈的 TWAP 定價，且折價為零。 |
|混合 `xor_dual_fund` |路由器日誌顯示公共與屏蔽分離 + 遙測計數器 |屏蔽/公共混合尊重治理比率並記錄應用於每條腿的理髮。 |

## 需要更多細節嗎？

- 完整常見問題解答：`docs/source/nexus_settlement_faq.md`
- 結算路由器規格：`docs/source/settlement_router.md`
- CBDC 政策手冊：`docs/source/cbdc_lane_playbook.md`
- 操作手冊：[Nexus 操作](./nexus-operations)