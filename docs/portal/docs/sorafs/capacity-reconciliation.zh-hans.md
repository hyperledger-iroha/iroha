---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e26cc8232dd7d3b392d56646fdfbf809952f017532a37aafbfde3c8cc704ae0e
source_last_modified: "2025-12-29T18:16:35.177959+00:00"
translation_last_reviewed: 2026-02-07
id: capacity-reconciliation
title: SoraFS Capacity Reconciliation
description: Nightly workflow for matching capacity fee ledgers to XOR transfer exports.
translator: machine-google-reviewed
---

路线图项目 **SF-2c** 要求财务部门证明容量费用分类账
匹配每晚执行的 XOR 传输。使用
`scripts/telemetry/capacity_reconcile.py` 帮助程序比较
`/v1/sorafs/capacity/state` 针对执行的传输批次的快照以及
为 Alertmanager 发出 Prometheus 文本文件指标。

## 先决条件
- 从 Torii 导出的容量状态快照（`fee_ledger` 条目）。
- 同一窗口的分类帐导出（带有 `provider_id_hex` 的 JSON 或 NDJSON，
  `kind` = 和解/罚款，以及 `amount_nano`)。
- 如果需要警报，node_exporter 文本文件收集器的路径。

## 操作手册
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- 退出代码：干净比赛时为 `0`，结算/处罚缺失时为 `1`
  或多付，`2` 无效输入。
- 将 JSON 摘要 + 哈希值附加到金库数据包中
  `docs/examples/sorafs_capacity_marketplace_validation/`。
- 当 `.prom` 文件进入文本文件收集器时，警报
  `SoraFSCapacityReconciliationMismatch`（参见
  `dashboards/alerts/sorafs_capacity_rules.yml`) 丢失时触发，
  检测到支付过高或意外的提供商转账。

## 输出
- 每个提供商的状态以及和解和处罚方面的差异。
- 以仪表形式导出的总计：
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## 预期范围和公差
- 核对准确：预期与实际结算/罚款纳米级应零容忍匹配。任何非零差异都应该分页运算符。
- CI 将容量费用分类帐（测试 `capacity_fee_ledger_30_day_soak_deterministic`）的 30 天浸泡摘要固定到 `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`。仅当定价或冷却语义发生变化时刷新摘要。
- 在浸泡曲线（`penalty_bond_bps=0`、`strike_threshold=u32::MAX`）中，惩罚保持为零；生产只应在利用率/正常运行时间/PoR 底线被破坏时发出惩罚，并在连续削减之前遵守配置的冷却时间。