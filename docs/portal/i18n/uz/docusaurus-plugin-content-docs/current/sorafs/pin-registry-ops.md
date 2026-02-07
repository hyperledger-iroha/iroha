---
id: pin-registry-ops
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
---

:::note Canonical Source
:::

## Overview

This runbook documents how to monitor and triage the SoraFS pin registry and its replication service-level agreements (SLAs). The metrics originate from `iroha_torii` and are exported via Prometheus under the `torii_sorafs_*` namespace. Torii samples the registry state on a 30 second interval in the background, so dashboards remain current even when no operators are polling the `/v1/sorafs/pin/*` endpoints. Import the curated dashboard (`docs/source/grafana_sorafs_pin_registry.json`) for a ready-to-use Grafana layout that maps directly to the sections below.

## Metric Reference

| Metric | Labels | Description |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | On-chain manifest inventory by lifecycle state. |
| `torii_sorafs_registry_aliases_total` | — | Count of active manifest aliases recorded in the registry. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Replication order backlog segmented by status. |
| `torii_sorafs_replication_backlog_total` | — | Convenience gauge mirroring `pending` orders. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA accounting: `met` counts completed orders within deadline, `missed` aggregates late completions + expirations, `pending` mirrors outstanding orders. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Aggregated completion latency (epochs between issuance and completion). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Pending-order slack windows (deadline minus issued epoch). |

All gauges reset on every snapshot pull, so dashboards should sample at `1m` cadence or faster.

## Grafana Dashboard

The dashboard JSON ships with seven panels that cover operator workflows. The queries are listed below for quick reference if you prefer to build bespoke charts.

1. **Manifest lifecycle** – `torii_sorafs_registry_manifests_total` (grouped by `status`).
2. **Alias catalogue trend** – `torii_sorafs_registry_aliases_total`.
3. **Order queue by status** – `torii_sorafs_registry_orders_total` (grouped by `status`).
4. **Backlog vs expired orders** – combines `torii_sorafs_replication_backlog_total` and `torii_sorafs_registry_orders_total{status="expired"}` to surface saturation.
5. **SLA success ratio** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latency vs deadline slack** – overlay `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` and `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Use Grafana transformations to add `min_over_time` views when you need the absolute slack floor, for example:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Missed orders (1h rate)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Alert Thresholds

- **SLA success < 0.95 for 15 min**
  - Threshold: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Action: Page SRE; start replication backlog triage.
- **Pending backlog above 10**
  - Threshold: `torii_sorafs_replication_backlog_total > 10` sustained for 10 min
  - Action: Check provider availability and the Torii capacity scheduler.
- **Expired orders > 0**
  - Threshold: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Action: Inspect governance manifests to confirm provider churn.
- **Completion p95 > deadline slack avg**
  - Threshold: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Action: Verify providers are committing before deadlines; consider issuing reassignments.

### Example Prometheus Rules

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Triage Workflow

1. **Identify cause**
   - If SLA misses spike while backlog remains low, focus on provider performance (PoR failures, late completions).
   - If backlog grows with stable misses, inspect admission (`/v1/sorafs/pin/*`) to confirm manifests awaiting council approval.
2. **Validate provider status**
   - Run `iroha app sorafs providers list` and verify the advertised capabilities match replication requirements.
   - Check `torii_sorafs_capacity_*` gauges to confirm provisioned GiB and PoR success.
3. **Reassign replication**
   - Issue new orders via `sorafs_manifest_stub capacity replication-order` when backlog slack (`stat="avg"`) drops below 5 epochs (manifest/CAR packaging uses `iroha app sorafs toolkit pack`).
   - Notify governance if aliases lack active manifest bindings (`torii_sorafs_registry_aliases_total` drops unexpectedly).
4. **Document outcome**
   - Record incident notes in the SoraFS operations log with timestamps and affected manifest digests.
   - Update this runbook if new failure modes or dashboards are introduced.

## Rollout Plan

Follow this staged procedure when enabling or tightening the alias cache policy in production:

1. **Prepare configuration**
   - Update `torii.sorafs_alias_cache` in `iroha_config` (user → actual) with the agreed TTLs and grace windows: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, and `governance_grace`. The defaults match the policy in `docs/source/sorafs_alias_policy.md`.
   - For SDKs, distribute the same values through their configuration layers (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` in Rust / NAPI / Python bindings) so client enforcement matches the gateway.
2. **Dry-run in staging**
   - Deploy the config change to a staging cluster that mirrors production topology.
   - Run `cargo xtask sorafs-pin-fixtures` to confirm the canonical alias fixtures still decode and round-trip; any mismatch implies upstream manifest drift that must be addressed first.
   - Exercise the `/v1/sorafs/pin/{digest}` and `/v1/sorafs/aliases` endpoints with synthetic proofs covering fresh, refresh-window, expired, and hard-expired cases. Validate the HTTP status codes, headers (`Sora-Proof-Status`, `Retry-After`, `Warning`), and JSON body fields against this runbook.
3. **Enable in production**
   - Roll out the new configuration via the standard change window. Apply it to Torii first, then restart gateways/SDK services once the node confirms the new policy in logs.
   - Import `docs/source/grafana_sorafs_pin_registry.json` into Grafana (or update existing dashboards) and pin the alias cache refresh panels to the NOC workspace.
4. **Post-deployment verification**
   - Monitor `torii_sorafs_alias_cache_refresh_total` and `torii_sorafs_alias_cache_age_seconds` for 30 minutes. Spikes in the `error`/`expired` curves should correlate with policy refresh windows; unexpected growth means operators must inspect alias proofs and provider health before continuing.
   - Confirm client-side logs show the same policy decisions (SDKs will surface errors when the proof is stale or expired). Absence of client warnings indicates a misconfiguration.
5. **Fallback**
   - If alias issuance falls behind and the refresh window trips frequently, temporarily relax the policy by increasing `refresh_window` and `positive_ttl` in config, then redeploy. Keep `hard_expiry` intact so truly stale proofs are still rejected.
   - Revert to the prior configuration by restoring the previous `iroha_config` snapshot if telemetry continues to show elevated `error` counts, then open an incident to trace alias generation delays.

## Related Materials

- `docs/source/sorafs/pin_registry_plan.md` — implementation roadmap and governance context.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — storage worker operations, complements this registry playbook.
