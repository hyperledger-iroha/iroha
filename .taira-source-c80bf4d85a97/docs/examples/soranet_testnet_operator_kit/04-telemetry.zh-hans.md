---
lang: zh-hans
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 遥测要求

## Prometheus 目标

使用以下标签抓取中继和协调器：

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

## 所需的仪表板

1. `dashboards/grafana/soranet_testnet_overview.json` *（待发布）* — 加载 JSON，导入变量 `region` 和 `relay_id`。
2. `dashboards/grafana/soranet_privacy_metrics.json` *（现有 SNNet-8 资产）* — 确保隐私桶面板渲染无间隙。

## 警报规则

阈值必须符合剧本期望：

- `soranet_privacy_circuit_events_total{kind="downgrade"}` 在 10 分钟内增加 > 0 会触发 `critical`。
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 每 30 分钟 5 次触发 `warning`。
- `up{job="soranet-relay"}` == 0 持续 2 分钟会触发 `critical`。

使用 `testnet-t0` 接收器将您的规则加载到 Alertmanager 中；使用 `amtool check-config` 进行验证。

## 指标评估

聚合 14 天的快照并将其提供给 SNNet-10 验证器：

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- 针对实时数据运行时，将示例文件替换为导出的快照。
- `status = fail` 结果阻止升级；在重试之前解决突出显示的检查。

## 报告

每周上传：

- 查询快照（`.png` 或 `.pdf`）显示 PQ 比率、电路成功率和 PoW 求解直方图。
- Prometheus `soranet_privacy_throttles_per_minute` 的记录规则输出。
- 描述任何触发的警报和缓解步骤的简短叙述（包括时间戳）。