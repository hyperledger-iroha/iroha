---
id: deal-engine
lang: zh-hans
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

:::注意规范来源
:::

# SoraFS 交易引擎

SF-8 路线图轨道引入了 SoraFS 交易引擎，提供
之间的存储和检索协议的确定性核算
客户和提供商。协议通过 Norito 有效负载进行描述
`crates/sorafs_manifest/src/deal.rs` 中定义，涵盖交易条款、债券
锁定、概率小额支付和结算记录。

嵌入式 SoraFS 工作线程 (`sorafs_node::NodeHandle`) 现在实例化一个
每个节点进程的 `DealEngine` 实例。发动机：

- 使用 `DealTermsV1` 验证和注册交易；
- 报告复制使用情况时，会产生以 XOR 计价的费用；
- 使用确定性评估概率性小额支付窗口
  基于Blake3的采样；和
- 生成适合治理的账本快照和结算有效负载
  出版。

单元测试涵盖验证、小额支付选择和结算流程，以便
操作员可以放心地使用 API。定居点现在排放
`DealSettlementV1` 治理有效负载，直接接线到 SF-12
发布管道，并更新 `sorafs.node.deal_*` OpenTelemetry 系列
（`deal_settlements_total`、`deal_expected_charge_nano`、`deal_client_debit_nano`、
`deal_outstanding_nano`、`deal_bond_slash_nano`、`deal_publish_total`) 用于 Torii 仪表板和 SLO
执法。后续项目重点关注审计员发起的削减自动化和
协调取消语义与治理策略。

使用情况遥测现在还提供 `sorafs.node.micropayment_*` 指标集：
`micropayment_charge_nano`、`micropayment_credit_generated_nano`、
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`，以及售票柜台
（`micropayment_tickets_processed_total`，`micropayment_tickets_won_total`，
`micropayment_tickets_duplicate_total`）。这些总数揭示了概率
彩票流程，以便运营商可以将小额支付中奖与信用结转关联起来
与结算结果。

## Torii 集成

Torii 公开专用端点，以便提供商可以报告使用情况并驱动
无需定制接线的交易生命周期：

- `POST /v2/sorafs/deal/usage` 接受 `DealUsageReport` 遥测并返回
  确定性会计结果 (`UsageOutcome`)。
- `POST /v2/sorafs/deal/settle` 完成当前窗口，流式传输
  生成的 `DealSettlementRecord` 以及 Base64 编码的 `DealSettlementV1`
  准备好治理 DAG 发布。
- Torii 的 `/v2/events/sse` 源现在广播 `SorafsGatewayEvent::DealUsage`
  总结每次使用提交的记录（纪元、计量 GiB 小时、票证
  计数器，确定性费用），`SorafsGatewayEvent::DealSettlement`
  记录包括规范结算账本快照以及
  磁盘治理工件的 BLAKE3 摘要/大小/base64，以及
  每当 PDP/PoTR 阈值达到时，`SorafsGatewayEvent::ProofHealth` 就会发出警报
  超出（提供者、窗口、罢工/冷却状态、罚款金额）。消费者可以
  按提供商过滤，无需轮询即可对新的遥测、结算或健康证明警报做出反应。

两个端点都通过新的 SoraFS 配额框架参与
`torii.sorafs.quota.deal_telemetry` 窗口，允许操作员调整
每次部署允许的提交率。