---
id: nexus-fee-model
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus fee model updates
description: Mirror of `docs/source/nexus_fee_model.md`, documenting the lane settlement receipts and reconciliation surfaces.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
此页面镜像 `docs/source/nexus_fee_model.md`。在迁移日语、希伯来语、西班牙语、葡萄牙语、法语、俄语、阿拉伯语和乌尔都语翻译时，保持两个副本对齐。
:::

# Nexus 费用模型更新

统一结算路由器现在捕获确定性的每通道收据，因此
运营商可以根据 Nexus 费用模型调节天然气借方。

- 对于完整的路由器架构、缓冲区策略、遥测矩阵和部署
  测序参见 `docs/settlement-router.md`。该指南解释了如何
  此处记录的参数与 NX-3 路线图可交付成果以及 SRE 如何联系起来
  应监控生产中的路由器。
- 天然气资产配置 (`pipeline.gas.units_per_gas`) 包括
  `twap_local_per_xor` 十进制，一个 `liquidity_profile` (`tier1`, `tier2`,
  或 `tier3`)，以及 `volatility_class` (`stable`、`elevated`、`dislocated`)。
  这些标志馈送到结算路由器，因此生成的 XOR
  quote 与车道的规范 TWAP 和理发等级相匹配。
- 每笔支付gas的交易都会记录一个`LaneSettlementReceipt`。  每个
  收据存储调用者提供的源标识符、本地微量、
  立即到期的 XOR、理发后预计的 XOR、已实现的
  方差 (`xor_variance_micro`) 和区块时间戳（以毫秒为单位）。
- 块执行聚合每个通道/数据空间的收据并发布它们
  通过 `/v2/sumeragi/status` 中的 `lane_settlement_commitments`。  总计
  公开 `total_local_micro`、`total_xor_due_micro` 和
  `total_xor_after_haircut_micro` 在每晚的块上求和
  调节出口。
- 新的 `total_xor_variance_micro` 计数器跟踪安全裕度是多少
  消耗（应有的异或和理发后期望之间的差异），
  和 `swap_metadata` 记录确定性转换参数
  （TWAP、epsilon、流动性概况和波动性类别），以便审计师可以
  验证独立于运行时配置的报价输入。

消费者可以在现有车道旁观看 `lane_settlement_commitments`
和数据空间承诺快照，以验证费用缓冲区、理发层、
和交换执行匹配配置的 Nexus 费用模型。