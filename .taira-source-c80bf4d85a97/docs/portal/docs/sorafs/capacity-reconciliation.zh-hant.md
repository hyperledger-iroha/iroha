---
lang: zh-hant
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

路線圖項目 **SF-2c** 要求財務部門證明容量費用分類賬
匹配每晚執行的 XOR 傳輸。使用
`scripts/telemetry/capacity_reconcile.py` 幫助程序比較
`/v1/sorafs/capacity/state` 針對執行的傳輸批次的快照以及
為 Alertmanager 發出 Prometheus 文本文件指標。

## 先決條件
- 從 Torii 導出的容量狀態快照（`fee_ledger` 條目）。
- 同一窗口的分類帳導出（帶有 `provider_id_hex` 的 JSON 或 NDJSON，
  `kind` = 和解/罰款，以及 `amount_nano`)。
- 如果需要警報，node_exporter 文本文件收集器的路徑。

## 操作手冊
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- 退出代碼：乾淨比賽時為 `0`，結算/處罰缺失時為 `1`
  或多付，`2` 無效輸入。
- 將 JSON 摘要 + 哈希值附加到金庫數據包中
  `docs/examples/sorafs_capacity_marketplace_validation/`。
- 當 `.prom` 文件進入文本文件收集器時，警報
  `SoraFSCapacityReconciliationMismatch`（參見
  `dashboards/alerts/sorafs_capacity_rules.yml`) 丟失時觸發，
  檢測到支付過高或意外的提供商轉賬。

## 輸出
- 每個提供商的狀態以及和解和處罰方面的差異。
- 以儀表形式導出的總計：
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## 預期範圍和公差
- 核對準確：預期與實際結算/罰款納米級應零容忍匹配。任何非零差異都應該分頁運算符。
- CI 將容量費用分類帳（測試 `capacity_fee_ledger_30_day_soak_deterministic`）的 30 天浸泡摘要固定到 `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`。僅當定價或冷卻語義發生變化時刷新摘要。
- 在浸泡曲線（`penalty_bond_bps=0`、`strike_threshold=u32::MAX`）中，懲罰保持為零；生產只應在利用率/正常運行時間/PoR 底線被破壞時發出懲罰，並在連續削減之前遵守配置的冷卻時間。