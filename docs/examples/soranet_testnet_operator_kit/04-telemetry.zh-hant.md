---
lang: zh-hant
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 遙測要求

## Prometheus 目標

使用以下標籤抓取中繼和協調器：

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

## 所需的儀表板

1. `dashboards/grafana/soranet_testnet_overview.json` *（待發布）* — 加載 JSON，導入變量 `region` 和 `relay_id`。
2. `dashboards/grafana/soranet_privacy_metrics.json` *（現有 SNNet-8 資產）* — 確保隱私桶麵板渲染無間隙。

## 警報規則

閾值必須符合劇本期望：

- `soranet_privacy_circuit_events_total{kind="downgrade"}` 在 10 分鐘內增加 > 0 會觸發 `critical`。
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 每 30 分鐘 5 次觸發 `warning`。
- `up{job="soranet-relay"}` == 0 持續 2 分鐘會觸發 `critical`。

使用 `testnet-t0` 接收器將您的規則加載到 Alertmanager 中；使用 `amtool check-config` 進行驗證。

## 指標評估

聚合 14 天的快照並將其提供給 SNNet-10 驗證器：

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- 針對實時數據運行時，將示例文件替換為導出的快照。
- `status = fail` 結果阻止升級；在重試之前解決突出顯示的檢查。

## 報告

每週上傳：

- 查詢快照（`.png` 或 `.pdf`）顯示 PQ 比率、電路成功率和 PoW 求解直方圖。
- Prometheus `soranet_privacy_throttles_per_minute` 的記錄規則輸出。
- 描述任何觸發的警報和緩解步驟的簡短敘述（包括時間戳）。