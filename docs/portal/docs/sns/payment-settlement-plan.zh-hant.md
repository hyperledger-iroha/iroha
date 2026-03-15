---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1be9268784bf75c4c5d1bf854e72c817475a079c0d2bf06ce120ccd325ad6083
source_last_modified: "2026-01-22T14:45:01.248924+00:00"
translation_last_reviewed: 2026-02-07
id: payment-settlement-plan
title: SNS Payment & Settlement Plan
sidebar_label: Payment & settlement plan
description: Playbook for routing SNS registrar revenue, reconciling steward/treasury splits, and producing evidence bundles.
translator: machine-google-reviewed
---

> 規範來源：[`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md)。

路線圖任務 **SN-5 — 支付和結算服務** 引入了確定性
Sora 名稱服務的支付層。每次註冊、續訂或退款
必鬚髮出結構化的 Norito 有效負載，以便財務部門、管理者和治理部門能夠
無需電子表格即可重播財務流。此頁面提煉了規格
面向門戶受眾。

## 收入模式

- 基本費用 (`gross_fee`) 來自註冊商定價矩陣。  
- 財政部收到 `gross_fee × 0.70`，管家收到減去的剩餘部分
  推薦獎金（上限為 10%）。  
- 可選的扣留允許治理在爭議期間暫停管理人的支付。  
- 定居點捆綁暴露了帶有混凝土的 `ledger_projection` 塊
  `Transfer` ISI，因此自動化可以將異或運算直接發佈到 Torii 中。

## 服務和自動化

|組件|目的|證據|
|------------|---------|----------|
| `sns_settlementd` |應用政策、簽署捆綁包、表面 `/v2/sns/settlements`。 | JSON 捆綁 + 哈希。 |
|結算隊列和寫入器|由 `iroha_cli app sns settlement ledger` 驅動的冪等隊列 + 賬本提交器。 |捆綁哈希 ↔ tx 哈希清單。 |
|對賬工作| `docs/source/sns/reports/` 下的每日差異 + 月度報表。 | Markdown + JSON 摘要。 |
|退款櫃檯 |通過 `/settlements/{id}/refund` 獲得治理批准的退款。 | `RefundRecordV1` + 票。 |

CI 助手鏡像這些流程：

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## 可觀察性和報告

- 儀表板：`dashboards/grafana/sns_payment_settlement.json`（財務與財務）
  管家總數、推薦支出、隊列深度和退款延遲。
- 警報：`dashboards/alerts/sns_payment_settlement_rules.yml` 監視器待處理
  年齡、對賬失敗和賬本漂移。
- 報表：每日摘要 (`settlement_YYYYMMDD.{json,md}`) 轉入每月
  報告 (`settlement_YYYYMM.md`) 已上傳到 Git 和
  治理對象存儲 (`s3://sora-governance/sns/settlements/<period>/`)。
- 治理數據包捆綁儀表板、CLI 日誌和理事會批准
  簽核。

## 推出清單

1. 原型報價 + 賬本助手並捕獲暫存包。
2. 使用隊列 + writer、wire 儀表板和練習啟動 `sns_settlementd`
   警報測試 (`promtool test rules ...`)。
3. 提供退款助手及月結單模板；將文物鏡像到
   `docs/portal/docs/sns/reports/`。
4. 進行合作夥伴排練（整月的結算）並捕捉
   治理投票將 SN-5 標記為完成。

請返回源文檔以獲取確切的模式定義，打開
問題以及未來的修改。