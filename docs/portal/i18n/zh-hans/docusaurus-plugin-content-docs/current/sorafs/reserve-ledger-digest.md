---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Reserve+Rent 政策（路线图项目 **SFM-6**）现在提供 `sorafs reserve`
CLI 助手加上 `scripts/telemetry/reserve_ledger_digest.py` 转换器
国债运行可以产生确定性的租金/储备转移。此页面镜像
`docs/source/sorafs_reserve_rent_plan.md` 中定义的工作流程并解释
如何将新的传输源连接到 Grafana + Alertmanager 中，如此经济且
治理审核员可以审核每个计费周期。

## 端到端工作流程

1. **报价+账本投影**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account <katakana-i105-account-id> \
    --treasury-account <katakana-i105-account-id> \
    --reserve-account <katakana-i105-account-id> \
    --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   账本助手附加一个 `ledger_projection` 块（租金到期，保留
   短缺、充值增量、承保布尔值）加上 Norito `Transfer`
   ISI 需要在国库账户和储备账户之间进行异或运算。

2. **生成摘要 + Prometheus/NDJSON 输出**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   摘要助手将微异或总计标准化为异或，记录是否
   预测满足承保要求，并发出**转移源**指标
   `sorafs_reserve_ledger_transfer_xor` 和
   `sorafs_reserve_ledger_instruction_total`。当需要多个账本时
   处理（例如，一批提供者），重复 `--ledger`/`--label` 对和
   帮助程序写入一个包含每个摘要的 NDJSON/Prometheus 文件，以便
   仪表板吸收了整个周期，无需定制胶水。 `--out-prom`
   文件的目标是节点导出器文本文件收集器 - 将 `.prom` 文件放入
   导出者的监视目录或将其上传到遥测存储桶
   由 Reserve 仪表板作业消耗 — 而 `--ndjson-out` 则提供相同的数据
   有效负载进入数据管道。

3. **发布文物+证据**
   - 将摘要存储在 `artifacts/sorafs_reserve/ledger/<provider>/` 和链接下
     每周经济报告中的 Markdown 摘要。
   - 将 JSON 摘要附加到租金燃尽表中（以便审核员可以重播
     math）并将校验和包含在治理证据包中。
   - 如果摘要表明存在充值或承保违规，请参考警报
     ID（`SoraFSReserveLedgerTopUpRequired`，
     `SoraFSReserveLedgerUnderwritingBreach`）并记下哪些传输 ISI
     应用。

## 指标 → 仪表板 → 警报

|来源指标 | Grafana面板|警报/政策挂钩 |笔记|
|----------------|---------------|---------------------|--------|
| `torii_da_rent_base_micro_total`、`torii_da_protocol_reserve_micro_total`、`torii_da_provider_reward_micro_total` | `dashboards/grafana/sorafs_capacity_health.json` 中的“DA 租金分配（异或/小时）”|提供每周财政部摘要；储备流量的峰值传播到 `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`)。 |
| `torii_da_rent_gib_months_total` | “容量使用情况（GiB 月）”（同一仪表板）|与账本摘要配对，证明发票存储与 XOR 传输相匹配。 |
| `sorafs_reserve_ledger_rent_due_xor`、`sorafs_reserve_ledger_reserve_shortfall_xor`、`sorafs_reserve_ledger_top_up_shortfall_xor` | `dashboards/grafana/sorafs_reserve_economics.json` 中的“保留快照 (XOR)”+ 状态卡 | `SoraFSReserveLedgerTopUpRequired` 当 `requires_top_up=1` 时触发； `SoraFSReserveLedgerUnderwritingBreach` 当 `meets_underwriting=0` 时触发。 |
| `sorafs_reserve_ledger_transfer_xor`、`sorafs_reserve_ledger_instruction_total` | “按种类转移”、“最新转移明细”以及 `dashboards/grafana/sorafs_reserve_economics.json` 中的承保卡 |即使需要租金/充值，当传输馈送缺失或归零时，`SoraFSReserveLedgerInstructionMissing`、`SoraFSReserveLedgerRentTransferMissing` 和 `SoraFSReserveLedgerTopUpTransferMissing` 也会发出警告；在相同情况下，承保卡会降至 0%。 |

租用周期完成后，刷新 Prometheus/NDJSON 快照，确认
Grafana 面板采用新的 `label`，并附上屏幕截图 +
租金治理数据包的 Alertmanager ID。这证明了 CLI 投影，
遥测和治理工件都源于**相同的**传输源和
使路线图的经济仪表板与储备+租金保持一致
自动化。覆盖卡应显示 100%（或 1.0）并且新警报
一旦租金和储备金充值转移出现在摘要中，就应该清除。