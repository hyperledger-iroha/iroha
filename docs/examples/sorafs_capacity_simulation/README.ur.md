---
lang: ur
direction: rtl
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-11-05T17:59:15.481814+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/sorafs_capacity_simulation/README.md کا اردو ترجمہ -->

# SoraFS Capacity Simulation Toolkit

یہ ڈائریکٹری SF-2c capacity marketplace simulation کے لئے reproducible artefacts فراہم کرتی ہے۔ یہ toolkit quota negotiation، failover handling، اور slashing remediation کو production CLI helpers اور ایک lightweight analysis script کے ساتھ exercise کرتی ہے۔

## Prerequisites

- Rust toolchain جو workspace members کے لئے `cargo run` چلا سکے.
- Python 3.10+ (صرف standard library).

## Quickstart

```bash
# 1. Canonical CLI artefacts generate کریں
./run_cli.sh ./artifacts

# 2. نتائج aggregate کریں اور Prometheus metrics emit کریں
./analyze.py --artifacts ./artifacts
```

`run_cli.sh` script `sorafs_manifest_stub capacity` کو invoke کرتی ہے تاکہ یہ بنائے:

- quota negotiation fixture set کے لئے deterministic provider declarations.
- negotiation scenario کے مطابق replication order.
- failover window کے لئے telemetry snapshots.
- slashing request capture کرنے والا dispute payload.

script Norito bytes (`*.to`)، base64 payloads (`*.b64`)، Torii request bodies، اور
human-readable summaries (`*_summary.json`) منتخب artefact directory میں لکھتا ہے۔

`analyze.py` generated summaries consume کرتا ہے، aggregated report
(`capacity_simulation_report.json`) بناتا ہے، اور Prometheus textfile
(`capacity_simulation.prom`) emit کرتا ہے جس میں شامل ہیں:

- `sorafs_simulation_quota_*` gauges جو negotiated capacity اور allocation share بیان کرتے ہیں.
- `sorafs_simulation_failover_*` gauges جو downtime deltas اور منتخب replacement provider دکھاتے ہیں.
- `sorafs_simulation_slash_requested` جو dispute payload سے نکالا گیا remediation percentage ریکارڈ کرتا ہے.

`dashboards/grafana/sorafs_capacity_simulation.json` میں Grafana bundle امپورٹ کریں اور
اسے ایسے Prometheus datasource کی طرف پوائنٹ کریں جو generated textfile scrape کرے (مثال کے طور پر
node-exporter textfile collector کے ذریعے). `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`
والا runbook مکمل workflow بیان کرتا ہے، جس میں Prometheus configuration tips شامل ہیں.

## Fixtures

- `scenarios/quota_negotiation/` — Provider declaration specs اور replication order.
- `scenarios/failover/` — primary outage اور failover lift کے لئے telemetry windows.
- `scenarios/slashing/` — dispute spec جو اسی replication order کا حوالہ دیتا ہے.

یہ fixtures `crates/sorafs_car/tests/capacity_simulation_toolkit.rs` میں validate ہوتے ہیں
تاکہ وہ CLI schema کے ساتھ sync رہیں.

</div>
