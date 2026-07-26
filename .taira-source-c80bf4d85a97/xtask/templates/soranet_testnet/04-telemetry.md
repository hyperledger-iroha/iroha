# Telemetry Requirements

## Prometheus Targets

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

## Required Dashboards

1. Import `dashboard_templates/soranet_testnet_overview.json` and bind the `Prometheus` data source.
2. Import `dashboards/grafana/soranet_privacy_metrics.json` (SNNet-8 asset).

## Alerts

Load `alert_templates/soranet_testnet_rules.yml` into Alertmanager. Required receivers:

- `Severity: critical` → governance on-call bridge.
- `Severity: warning` → testnet coordination channel.

Validate with `amtool check-config` before enabling production forwarding.

## Metrics Evaluation

Aggregate a 14-day snapshot and feed it to the SNNet-10 validator:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Replace the sample file with your exported snapshot when running against live data.
- A `status = fail` result blocks promotion; resolve the highlighted check(s) before retrying.
