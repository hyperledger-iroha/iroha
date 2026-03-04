---
lang: zh-hans
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c52d7f2c5ec9dc4cda81895561bc1261659935c94bf3f7febb0867f4981fe616
source_last_modified: "2026-01-22T16:26:46.472177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 回购托管人确认模板

当回购协议（双边或三方）引用托管人时使用此模板
通过 `RepoAgreement::custodian`。目标是记录托管SLA、路由
账户，并在资产转移之前钻取联系人。将模板复制到您的
证据目录（例如
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`），填写
占位符，并将文件作为治理数据包的一部分进行哈希处理，如
`docs/source/finance/repo_ops.md` §2.8。

## 1. 元数据

|领域|价值|
|--------|--------|
|协议标识符| `<repo-yyMMdd-XX>` |
|托管账户 ID | `<ih58...>` |
|准备时间/日期 | `<custodian ops lead>` |
|已确认桌面联系人 | `<desk lead + counterparty>` |
|证据目录| ``artifacts/finance/repo/<slug>/`` |

## 2. 托管范围

- **收到的附带定义：** `<list of asset definition ids>`
- **现金支线货币/结算轨：** `<xor#sora / other>`
- **保管窗口：** `<start/end timestamps or SLA summary>`
- **常规说明：** `<hash + path to standing instruction document>`
- **自动化先决条件：** `<scripts, configs, or runbooks custodian will invoke>`

## 3. 路由和监控

|项目 |价值|
|------|--------|
|托管钱包/账本账户 | `<asset ids or ledger path>` |
|监控通道| `<Slack/phone/on-call rotation>` |
|联系钻头 | `<primary + backup>` |
|所需警报 | `<PagerDuty service, Grafana board, etc.>` |

## 4. 声明

1. *托管准备情况：*“我们审查了分阶段的 `repo initiate` 有效负载，
   上面的标识符并准备接受列出的 SLA 下的抵押品
   在§2中。”
2. *回滚承诺：*“我们将执行上面指定的回滚剧本，如果
   由事件指挥官指挥，并将提供 CLI 日志以及哈希值
   `governance/drills/<timestamp>.log`”。
3. *证据保留：*“我们将保留该确认书，
   至少 `<duration>` 的指令和 CLI 日志，并将它们提供给
   财务委员会应要求。”

在下面签名（通过治理时可接受电子签名）
跟踪器）。

|名称 |角色 |签名/日期|
|------|------|------------------|
| `<custodian ops lead>` |托管运营商| `<signature>` |
| `<desk lead>` |书桌| `<signature>` |
| `<counterparty>` |交易对手 | `<signature>` |

> 签名后，对文件进行哈希处理（例如：`sha256sum custodian_ack_<cust>.md`）并
> 将摘要记录在治理数据包表中，以便审阅者可以验证
> 投票期间引用的确认字节。