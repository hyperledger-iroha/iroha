---
lang: ja
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-11-04T17:24:14.014911+00:00"
translation_last_reviewed: 2026-01-01
---

# テレメトリ要件

## Prometheus Targets

次のラベルで relay と orchestrator を scrape してください:

```yaml
- job_name: "soranet-relay"
  static_configs:
    - targets: ["relay-host:9898"]
      labels:
        region: "testnet-t0"
        role: "relay"
- job_name: "sorafs-orchestrator"
  static_configs:
    - targets: ["orchestrator-host:9797"]
      labels:
        region: "testnet-t0"
        role: "orchestrator"
```

## 必須ダッシュボード

1. `dashboards/grafana/soranet_testnet_overview.json` *(公開予定)* - JSON を読み込み、`region` と `relay_id` 変数をインポート。
2. `dashboards/grafana/soranet_privacy_metrics.json` *(既存の SNNet-8 アセット)* - privacy bucket のパネルが欠けずに表示されることを確認。

## アラートルール

しきい値は playbook の想定と一致させること:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` increase > 0 (10 分) で `critical`.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 (30 分) で `warning`.
- `up{job="soranet-relay"}` == 0 (2 分) で `critical`.

ルールを `testnet-t0` receiver で Alertmanager に読み込ませ、`amtool check-config` で検証してください。

## メトリクス評価

14 日分の snapshot を集計し、SNNet-10 バリデータに渡します:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- ライブデータで実行する場合はサンプルファイルをエクスポート済み snapshot に置き換えてください。
- `status = fail` の結果は昇格をブロックします。指摘されたチェックを解消して再実行してください。

## レポート

毎週アップロードするもの:

- PQ 比率、回路成功率、PoW 解決ヒストグラムを示すクエリスナップショット (`.png` または `.pdf`).
- `soranet_privacy_throttles_per_minute` の Prometheus recording rule 出力.
- 発火したアラートと緩和手順の簡潔な説明 (timestamps を含める).
