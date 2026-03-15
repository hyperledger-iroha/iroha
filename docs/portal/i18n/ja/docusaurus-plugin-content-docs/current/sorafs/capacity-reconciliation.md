---
id: capacity-reconciliation
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

ロードマップ項目 **SF-2c** は、トレジャリーが容量手数料台帳と毎夜実行される XOR 転送が一致することを証明することを要求します。`scripts/telemetry/capacity_reconcile.py` を使って `/v2/sorafs/capacity/state` のスナップショットを実行済み転送バッチと比較し、Alertmanager 向けの Prometheus テキストファイルメトリクスを出力してください。

## 前提条件
- Torii からエクスポートされた容量状態スナップショット (`fee_ledger` エントリ)。
- 同一ウィンドウの台帳エクスポート (JSON または NDJSON、`provider_id_hex`,
  `kind` = settlement/penalty, `amount_nano` を含む)。
- アラートが必要な場合は node_exporter textfile collector のパス。

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- 終了コード: 一致なら `0`、settlement/penalty の欠落または過払いがある場合は `1`、入力が無効な場合は `2`。
- JSON サマリー + ハッシュをトレジャリーパケットに添付してください:
  `docs/examples/sorafs_capacity_marketplace_validation/`。
- `.prom` ファイルが textfile collector に到達すると、アラート
  `SoraFSCapacityReconciliationMismatch` (参照:
  `dashboards/alerts/sorafs_capacity_rules.yml`) が、欠落・過払い・想定外の
  プロバイダー転送を検出したときに発火します。

## 出力
- プロバイダーごとのステータスと settlement/penalty の差分。
- ゲージとしてエクスポートされる合計値:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## 期待範囲と許容差
- リコンシリエーションは厳密です: settlement/penalty の expected と actual の
  nanos はゼロ許容で一致する必要があります。ゼロ以外の差分はオペレーターを
  ページングします。
- CI は容量手数料台帳の 30 日 soak digest を
  (テスト `capacity_fee_ledger_30_day_soak_deterministic`) により
  `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1` に固定します。
  価格設定や cooldown の意味が変わる場合にのみ digest を更新してください。
- soak プロファイル (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) では
  penalties はゼロのままです。プロダクションでは利用率/uptime/PoR の閾値を
  超えた場合にのみ penalties を発行し、連続した slashes の前に設定された
  cooldown を遵守してください。
