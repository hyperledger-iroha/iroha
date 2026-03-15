---
id: deal-engine
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Deal Engine
sidebar_label: Deal Engine
description: Overview of the SF-8 deal engine, Torii integration, and telemetry surfaces.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

# SoraFS 交易引擎

SF-8 路線圖軌道引入了 SoraFS 交易引擎，提供
之間的存儲和檢索協議的確定性核算
客戶和提供商。協議通過 Norito 有效負載進行描述
`crates/sorafs_manifest/src/deal.rs` 中定義，涵蓋交易條款、債券
鎖定、概率小額支付和結算記錄。

嵌入式 SoraFS 工作線程 (`sorafs_node::NodeHandle`) 現在實例化一個
每個節點進程的 `DealEngine` 實例。發動機：

- 使用 `DealTermsV1` 驗證和註冊交易；
- 報告複製使用情況時，會產生以 XOR 計價的費用；
- 使用確定性評估概率性小額支付窗口
  基於Blake3的採樣；和
- 生成適合治理的賬本快照和結算有效負載
  出版。

單元測試涵蓋驗證、小額支付選擇和結算流程，以便
操作員可以放心地使用 API。定居點現在排放
`DealSettlementV1` 治理有效負載，直接接線到 SF-12
發布管道，並更新 `sorafs.node.deal_*` OpenTelemetry 系列
（`deal_settlements_total`、`deal_expected_charge_nano`、`deal_client_debit_nano`、
`deal_outstanding_nano`、`deal_bond_slash_nano`、`deal_publish_total`) 用於 Torii 儀表板和 SLO
執法。後續項目重點關注審計員發起的削減自動化和
協調取消語義與治理策略。

使用情況遙測現在還提供 `sorafs.node.micropayment_*` 指標集：
`micropayment_charge_nano`、`micropayment_credit_generated_nano`、
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`，以及售票櫃檯
（`micropayment_tickets_processed_total`，`micropayment_tickets_won_total`，
`micropayment_tickets_duplicate_total`）。這些總數揭示了概率
彩票流程，以便運營商可以將小額支付中獎與信用結轉關聯起來
與結算結果。

## Torii 集成

Torii 公開專用端點，以便提供商可以報告使用情況並驅動
無需定制接線的交易生命週期：

- `POST /v2/sorafs/deal/usage` 接受 `DealUsageReport` 遙測並返回
  確定性會計結果 (`UsageOutcome`)。
- `POST /v2/sorafs/deal/settle` 完成當前窗口，流式傳輸
  生成的 `DealSettlementRecord` 以及 Base64 編碼的 `DealSettlementV1`
  準備好治理 DAG 發布。
- Torii 的 `/v2/events/sse` 源現在廣播 `SorafsGatewayEvent::DealUsage`
  總結每次使用提交的記錄（紀元、計量 GiB 小時、票證
  計數器，確定性費用），`SorafsGatewayEvent::DealSettlement`
  記錄包括規範結算賬本快照以及
  磁盤治理工件的 BLAKE3 摘要/大小/base64，以及
  每當 PDP/PoTR 閾值達到時，`SorafsGatewayEvent::ProofHealth` 就會發出警報
  超出（提供者、窗口、罷工/冷卻狀態、罰款金額）。消費者可以
  按提供商過濾，無需輪詢即可對新的遙測、結算或健康證明警報做出反應。

兩個端點都通過新的 SoraFS 配額框架參與
`torii.sorafs.quota.deal_telemetry` 窗口，允許操作員調整
每次部署允許的提交率。