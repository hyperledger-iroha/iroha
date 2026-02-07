---
id: capacity-simulation
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Simulation Runbook
sidebar_label: Capacity Simulation Runbook
description: Exercising the SF-2c capacity marketplace simulation toolkit with reproducible fixtures, Prometheus exports, and Grafana dashboards.
---

:::note Canonical Source
:::

This runbook explains how to run the SF-2c capacity marketplace simulation kit and visualise the resulting metrics. It validates quota negotiation, failover handling, and slashing remediation end-to-end using the deterministic fixtures in `docs/examples/sorafs_capacity_simulation/`. Capacity payloads still use `sorafs_manifest_stub capacity`; use `iroha app sorafs toolkit pack` for manifest/CAR packaging flows.

## 1. Generate CLI artefacts

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` wraps `sorafs_manifest_stub capacity` to emit Norito payloads, base64 blobs, Torii request bodies, and JSON summaries for:

- Three provider declarations participating in the quota negotiation scenario.
- A replication order allocating the staged manifest across those providers.
- Telemetry snapshots for the pre-outage baseline, outage interval, and failover recovery.
- A dispute payload requesting slashing after the simulated outage.

All artefacts land under `./artifacts` (override by passing a different directory as the first argument). Inspect the `_summary.json` files for human-readable context.

## 2. Aggregate results & emit metrics

```bash
./analyze.py --artifacts ./artifacts
```

The analyzer produces:

- `capacity_simulation_report.json` - aggregated allocations, failover deltas, and dispute metadata.
- `capacity_simulation.prom` - Prometheus textfile metrics (`sorafs_simulation_*`) suitable for the node-exporter textfile collector or a standalone scrape job.

Example Prometheus scrape configuration:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

Point the textfile collector at `capacity_simulation.prom` (when using node-exporter copy it into the directory passed via `--collector.textfile.directory`).

## 3. Import the Grafana dashboard

1. In Grafana, import `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Bind the `Prometheus` datasource variable to the scrape target configured above.
3. Verify the panels:
   - **Quota Allocation (GiB)** shows committed/assigned balances for each provider.
   - **Failover Trigger** flips to *Failover Active* when the outage metrics stream in.
   - **Uptime Drop During Outage** charts the percentage loss for provider `alpha`.
   - **Requested Slash Percentage** visualises the remediation ratio extracted from the dispute fixture.

## 4. Expected checks

- `sorafs_simulation_quota_total_gib{scope="assigned"}` equals `600` while the committed total remains >=600.
- `sorafs_simulation_failover_triggered` reports `1` and the replacement provider metric highlights `beta`.
- `sorafs_simulation_slash_requested` reports `0.15` (15% slash) for the `alpha` provider identifier.

Run `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` to confirm the fixtures are still accepted by the CLI schema.
