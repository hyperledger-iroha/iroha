---
lang: zh-hans
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-12-29T18:16:35.080764+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Capacity Simulation Toolkit

This directory ships the reproducible artefacts for the SF-2c capacity marketplace
simulation. The toolkit exercises quota negotiation, failover handling, and slashing
remediation using the production CLI helpers and a lightweight analysis script.

## Prerequisites

- Rust toolchain capable of running `cargo run` for workspace members.
- Python 3.10+ (standard library only).

## Quickstart

```bash
# 1. Generate canonical CLI artefacts
./run_cli.sh ./artifacts

# 2. Aggregate the results and emit Prometheus metrics
./analyze.py --artifacts ./artifacts
```

The `run_cli.sh` script invokes `sorafs_manifest_stub capacity` to build:

- Deterministic provider declarations for the quota negotiation fixture set.
- A replication order matching the negotiation scenario.
- Telemetry snapshots for the failover window.
- A dispute payload capturing the slashing request.

The script writes Norito bytes (`*.to`), base64 payloads (`*.b64`), Torii request
bodies, and human-readable summaries (`*_summary.json`) under the chosen artifact
directory.

`analyze.py` consumes the generated summaries, produces an aggregated report
(`capacity_simulation_report.json`), and emits a Prometheus textfile
(`capacity_simulation.prom`) carrying:

- `sorafs_simulation_quota_*` gauges describing negotiated capacity and allocation
  share per provider.
- `sorafs_simulation_failover_*` gauges highlighting downtime deltas and the selected
  replacement provider.
- `sorafs_simulation_slash_requested` recording the remediation percentage extracted
  from the dispute payload.

Import the Grafana bundle in `dashboards/grafana/sorafs_capacity_simulation.json`
and point it at a Prometheus datasource that scrapes the generated textfile (for
example via the node-exporter textfile collector). The runbook at
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` walks through the full
workflow, including Prometheus configuration tips.

## Fixtures

- `scenarios/quota_negotiation/` — Provider declaration specs and replication order.
- `scenarios/failover/` — Telemetry windows for the primary outage and failover lift.
- `scenarios/slashing/` — Dispute spec referencing the same replication order.

These fixtures are validated in `crates/sorafs_car/tests/capacity_simulation_toolkit.rs`
to guarantee they remain in sync with the CLI schema.
