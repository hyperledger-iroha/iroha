---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Name Service Suffix Catalog
sidebar_label: Suffix catalog
description: Canonical allowlist of SNS suffixes, stewards, and pricing knobs for `.sora`, `.nexus`, and `.dao`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sora 名称服务后缀目录

SNS 路线图跟踪每个批准的后缀 (SN-1/SN-2)。此页面反映了
真实来源目录，以便运营商运行注册商、DNS 网关或钱包
工具可以加载相同的参数，而无需抓取状态文档。

- **快照：** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **消费者：** `iroha sns policy`、SNS 入门套件、KPI 仪表板和
  DNS/网关发布脚本都读取相同的 JSON 包。
- **状态：** `active`（允许注册）、`paused`（暂时关闭）、
  `revoked`（已发布，但当前不可用）。

## 目录架构

|领域|类型 |描述 |
|--------|------|-------------|
| `suffix` |字符串|带前导点的人类可读后缀。 |
| `suffix_id` | `u16` |标识符存储在账本上的 `SuffixPolicyV1::suffix_id` 中。 |
| `status` |枚举 | `active`、`paused` 或 `revoked` 描述启动准备情况。 |
| `steward_account` |字符串|负责管理的帐户（匹配注册商政策挂钩）。 |
| `fund_splitter_account` |字符串|根据 `fee_split` 在路由之前接收付款的帐户。 |
| `payment_asset_id` |字符串|用于结算的资产（初始队列为 `61CtjvNd9T3THAR65GsMVHr82Bjc`）。 |
| `min_term_years` / `max_term_years` |整数 |购买保单的期限限制。 |
| `grace_period_days` / `redemption_period_days` |整数 |由 Torii 强制执行的更新安全窗口。 |
| `referral_cap_bps` |整数 |治理允许的最大推荐剥离（基点）。 |
| `reserved_labels` |数组|受治理保护的标签对象 `{label, assigned_to, release_at_ms, note}`。 |
| `pricing` |数组|具有 `label_regex`、`base_price`、`auction_kind` 和持续时间界限的层对象。 |
| `fee_split` |对象| `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` 基点分割。 |
| `policy_version` |整数 |每当治理编辑策略时，单调计数器就会递增。 |

## 当前目录

|后缀 | ID (`hex`) |管家|资金分割 |状态 |支付资产|推荐上限 (bps) |期限（最短 – 最长年）|恩典/救赎（天）|定价等级（正则表达式 → 基本价格/拍卖）|保留标签|费用分割（T/S/R/E bps）|政策版本|
|--------|------------|---------|-------------|--------|----------------|--------------------|--------------------------|----------------------------|------------------------------------------------------------|-----------------|----------------------------------------|----------------|
| `.sora` | `0x0001` | `soraカタカナ...` | `soraカタカナ...` |活跃| `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1 – 5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → soraカタカナ...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `soraカタカナ...` | `soraカタカナ...` |暂停 | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 300 1 – 3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → soraカタカナ...`、`guardian → soraカタカナ...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `soraカタカナ...` | `soraカタカナ...` |撤销| `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1 – 2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON 摘录

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "soraカタカナ...",
      "payment_asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc", "amount": 120},
          "auction_kind": "vickrey_commit_reveal",
          "min_duration_years": 1,
          "max_duration_years": 5
        }
      ],
      "...": "see docs/examples/sns/suffix_catalog_v1.json for the full record"
    }
  ]
}
```

## 自动化笔记

1. 加载 JSON 快照并对其进行哈希/签名，然后再分发给操作员。
2. 注册商工具应显示 `suffix_id`、期限限制和定价
   每当请求到达 `/v1/sns/*` 时，就会从目录中获取。
3. DNS/网关助手在生成 GAR 时读取保留的标签元数据
   模板，以便 DNS 响应与治理控制保持一致。
4. KPI 附件作业标记仪表板导出并带有后缀元数据，以便警报与
   此处记录启动状态。