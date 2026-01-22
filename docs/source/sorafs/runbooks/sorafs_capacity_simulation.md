# SoraFS Capacity Simulation Runbook

This runbook explains how to exercise the SF-2c capacity marketplace simulation
toolkit and visualise the resulting metrics. The goal is to validate quota
negotiation, failover handling, and slashing remediation end-to-end using the
reproducible fixtures under `docs/examples/sorafs_capacity_simulation/`.
Capacity payloads still use `sorafs_manifest_stub capacity`; use
`iroha app sorafs toolkit pack` for manifest/CAR packaging flows.

## 1. Generate CLI artefacts

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

The script invokes `sorafs_manifest_stub capacity` to emit deterministic Norito
payloads, base64 encodings, Torii request bodies, and JSON summaries for:

- Three provider declarations participating in the quota negotiation scenario.
- A replication order allocating the staged manifest across the providers.
- Telemetry snapshots capturing the pre-outage baseline, the outage window, and
  the failover recovery.
- A dispute payload requesting slashing after the simulated outage.

The artefacts are written to `./artifacts` (or the path supplied as the first
argument). Inspect the `_summary.json` files for human-readable state.

## 2. Aggregate results & emit metrics

```bash
./analyze.py --artifacts ./artifacts
```

The analysis script produces:

- `capacity_simulation_report.json` — Aggregated allocations, failover deltas,
  and dispute metadata.
- `capacity_simulation.prom` — Prometheus textfile metrics (`sorafs_simulation_*`)
  suitable for import via the node-exporter textfile collector or a standalone
  Prometheus scrape job.

Example `prometheus.yml` scrape configuration:

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

Point the textfile collector at the generated `.prom` file (for node-exporter,
copy it into the configured `--collector.textfile.directory`).

## 3. Import Grafana dashboard

1. In Grafana, import `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Bind the `Prometheus` datasource input to the scrape configuration above.
3. Verify the panels:
   - **Quota Allocation (GiB)** shows committed/assigned balance for each provider.
   - **Failover Trigger** flips to *Failover Active* when the outage metrics are
     loaded.
   - **Uptime Drop During Outage** charts the percentage loss for provider `alpha`.
   - **Requested Slash Percentage** visualises the remediation ratio extracted
     from the dispute fixture.

## 4. Expected checks

- `sorafs_simulation_quota_total_gib{scope="assigned"}` equals 600 while the
  committed total remains ≥600.
- `sorafs_simulation_failover_triggered` reports `1` and the replacement provider
  metric highlights `beta`.
- `sorafs_simulation_slash_requested` reports `0.15` (15% slash) for the `alpha`
  provider identifier.

Run `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`
to confirm the fixtures still validate with the CLI schema.
