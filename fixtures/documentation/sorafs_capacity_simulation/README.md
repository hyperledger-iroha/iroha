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

The `run_cli.sh` script invokes `sorafs_manifest_builder capacity` to build:

- Deterministic provider declarations for the quota negotiation fixture set.
- A replication order matching the negotiation scenario.
- Telemetry snapshots for the failover window.
- A dispute payload capturing the slashing request.

The script writes Norito bytes (`*.to`), base64 payloads (`*.b64`), and JSON
summaries (`*_summary.json`) under the chosen artifact directory. Declaration
submission summaries contain only `declaration_b64`; validity and metadata remain
inside that canonical payload, and registration time is assigned by consensus.
The builder accepts no signing keys and does not submit transactions.

`analyze.py` consumes the copied declaration authoring specs (`*_spec.json`) and
the telemetry, replication and dispute summaries, then produces an aggregated report
(`capacity_simulation_report.json`), and emits a Prometheus textfile
(`capacity_simulation.prom`) carrying:

- `sorafs_simulation_quota_*` gauges describing negotiated capacity and allocation
  share per provider.
- `sorafs_simulation_failover_*` gauges highlighting downtime deltas and the selected
  replacement provider.
- `sorafs_simulation_slash_requested` recording the remediation percentage extracted
  from the dispute payload.

Quota reports identify their declaration source as `authoring_spec`; these are
simulation inputs, not a finalized capacity registry. The Rust fixture test
decodes each canonical declaration and checks provider identity and capacity
against its source specification.

Import the Grafana bundle in `dashboards/grafana/sorafs_capacity_simulation.json`
and point it at a Prometheus datasource that scrapes the generated textfile (for
example via the node-exporter textfile collector). The runbook at
`specs/sorafs/runbooks/sorafs_capacity_simulation.md` walks through the full
workflow, including Prometheus configuration tips.

## Fixtures

- `scenarios/quota_negotiation/` — Provider declaration specs and replication order.
- `scenarios/failover/` — Telemetry windows for the primary outage and failover lift.
- `scenarios/slashing/` — Dispute spec referencing the same replication order.

These fixtures are validated in `crates/sorafs_car/tests/capacity_simulation_toolkit.rs`
to guarantee they remain in sync with the CLI schema.
