---
lang: ka
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-12-29T18:16:35.091070+00:00"
translation_last_reviewed: 2026-02-07
---

# Telemetry Requirements

## Prometheus Targets

Scrape the relay and orchestrator with the following labels:

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

1. `dashboards/grafana/soranet_testnet_overview.json` *(to be published)* — load the JSON, import variables `region` and `relay_id`.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(existing SNNet-8 asset)* — ensure the privacy bucket panels render without gaps.

## Alert Rules

Thresholds must match the playbook expectation:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` increase > 0 over 10 minutes triggers `critical`.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 per 30 minutes triggers `warning`.
- `up{job="soranet-relay"}` == 0 for 2 minutes triggers `critical`.

Load your rules into Alertmanager with the `testnet-t0` receiver; validate with `amtool check-config`.

## Metrics Evaluation

Aggregate a 14-day snapshot and feed it to the SNNet-10 validator:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Replace the sample file with your exported snapshot when running against live data.
- A `status = fail` result blocks promotion; resolve the highlighted check(s) before retrying.

## Reporting

Every week upload:

- Query snapshots (`.png` or `.pdf`) showing PQ ratio, circuit success rate, and PoW solve histogram.
- Prometheus recording rule output for `soranet_privacy_throttles_per_minute`.
- A brief narrative describing any alerts that fired and mitigation steps (include timestamps).
