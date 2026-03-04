---
lang: ur
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-11-04T17:24:14.014911+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/soranet_testnet_operator_kit/04-telemetry.md کا اردو ترجمہ -->

# ٹیلی میٹری کی ضروریات

## Prometheus Targets

درج ذیل labels کے ساتھ relay اور orchestrator کو scrape کریں:

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

1. `dashboards/grafana/soranet_testnet_overview.json` *(to be published)* - JSON لوڈ کریں اور `region` اور `relay_id` variables امپورٹ کریں.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(existing SNNet-8 asset)* - یقینی بنائیں کہ privacy bucket پینلز بغیر gaps کے رینڈر ہوں.

## Alert Rules

thresholds کو playbook کی توقعات سے میچ ہونا چاہئے:

- `soranet_privacy_circuit_events_total{kind="downgrade"}` increase > 0 (10 منٹ) پر `critical` ٹرگر کرتا ہے.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 (30 منٹ) پر `warning` ٹرگر کرتا ہے.
- `up{job="soranet-relay"}` == 0 (2 منٹ) پر `critical` ٹرگر کرتا ہے.

اپنے rules کو `testnet-t0` receiver کے ساتھ Alertmanager میں لوڈ کریں؛ `amtool check-config` سے ویلیڈیٹ کریں.

## Metrics Evaluation

14 دن کا snapshot اکٹھا کریں اور اسے SNNet-10 validator کو دیں:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- live data پر چلانے کے لئے sample فائل کو اپنے export شدہ snapshot سے بدل دیں.
- `status = fail` نتیجہ promotion کو روکتا ہے؛ highlighted checks حل کر کے دوبارہ چلائیں.

## Reporting

ہر ہفتے اپ لوڈ کریں:

- Query snapshots (`.png` یا `.pdf`) جو PQ ratio، circuit success rate، اور PoW solve histogram دکھائیں.
- `soranet_privacy_throttles_per_minute` کے لئے Prometheus recording rule output.
- ایک مختصر narrative جو fired alerts اور mitigation steps بیان کرے (timestamps شامل کریں).

</div>
