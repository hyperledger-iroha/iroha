---
id: payment-settlement-plan
lang: zh-hans
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Payment & Settlement Plan
sidebar_label: Payment & settlement plan
description: Playbook for routing SNS registrar revenue, reconciling steward/treasury splits, and producing evidence bundles.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> 规范来源：[`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md)。

路线图任务 **SN-5 — 支付和结算服务** 引入了确定性
Sora 名称服务的支付层。每次注册、续订或退款
必须发出结构化的 Norito 有效负载，以便财务部门、管理者和治理部门能够
无需电子表格即可重播财务流。此页面提炼了规格
面向门户受众。

## 收入模式

- 基本费用 (`gross_fee`) 来自注册商定价矩阵。  
- 财政部收到 `gross_fee × 0.70`，管家收到减去的剩余部分
  推荐奖金（上限为 10%）。  
- 可选的扣留允许治理在争议期间暂停管理人的支付。  
- 定居点捆绑暴露了带有混凝土的 `ledger_projection` 块
  `Transfer` ISI，因此自动化可以将异或运算直接发布到 Torii 中。

## 服务和自动化

|组件|目的|证据|
|------------|---------|----------|
| `sns_settlementd` |应用政策、签署捆绑包、表面 `/v1/sns/settlements`。 | JSON 捆绑 + 哈希。 |
|结算队列和写入器|由 `iroha_cli app sns settlement ledger` 驱动的幂等队列 + 账本提交器。 |捆绑哈希 ↔ tx 哈希清单。 |
|对账工作 | `docs/source/sns/reports/` 下的每日差异 + 月度报表。 | Markdown + JSON 摘要。 |
|退款柜台 |通过 `/settlements/{id}/refund` 获得治理批准的退款。 | `RefundRecordV1` + 票。 |

CI 助手镜像这些流程：

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## 可观察性和报告

- 仪表板：`dashboards/grafana/sns_payment_settlement.json`（财务与财务）
  管家总数、推荐支出、队列深度和退款延迟。
- 警报：`dashboards/alerts/sns_payment_settlement_rules.yml` 监视器待处理
  年龄、对账失败和账本漂移。
- 报表：每日摘要 (`settlement_YYYYMMDD.{json,md}`) 转入每月
  报告 (`settlement_YYYYMM.md`) 已上传到 Git 和
  治理对象存储 (`s3://sora-governance/sns/settlements/<period>/`)。
- 治理数据包捆绑仪表板、CLI 日志和理事会批准
  签核。

## 推出清单

1. 原型报价 + 账本助手并捕获暂存包。
2. 使用队列 + writer、wire 仪表板和练习启动 `sns_settlementd`
   警报测试 (`promtool test rules ...`)。
3. 提供退款助手及月结单模板；将文物镜像到
   `docs/portal/docs/sns/reports/`。
4. 进行合作伙伴排练（整月的结算）并捕捉
   治理投票将 SN-5 标记为完成。

请返回源文档以获取确切的模式定义，打开
问题以及未来的修改。