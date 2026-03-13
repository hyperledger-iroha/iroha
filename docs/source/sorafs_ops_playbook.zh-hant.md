---
lang: zh-hant
direction: ltr
source: docs/source/sorafs_ops_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76e8798c76382e69b1aa54c11fbcdce68b1266b61c74d174360b5924b4193d8f
source_last_modified: "2026-01-22T14:35:37.605960+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Operations Playbook
summary: Incident response guides and chaos drill procedures for SoraFS operators.
---

# SoraFS Operations Playbook

This playbook bundles the operational runbooks required for SF-7b. It covers the
high-priority incident classes (gateway outage, proof failure, replication lag),
defines the chaos-drill cadence, and provides a reusable postmortem template for
incident retrospectives.

> **Portal:** This runbook is mirrored in the Docusaurus portal at
> `docs/portal/docs/sorafs/operations-playbook.md`. Keep the two copies in sync

## Key References

- Observability assets: refer to the Grafana dashboards under
  `dashboards/grafana/` and Prometheus alert rules in `dashboards/alerts/`.
- Metric catalog: `docs/source/sorafs_observability_plan.md`.
- Orchestrator telemetry surfaces: `docs/source/sorafs_orchestrator_plan.md`.

## Auth & Governance Checklist

- RBAC tokens: SoraFS instructions remain gated by their dedicated permission tokens (pin register/approve/retire/alias; capacity declare/telemetry/dispute; replication order issue/complete; pricing/credit upsert). Keep grants in sync with governance onboarding/offboarding.
- Provider binding: every provider id must be bound to an owner account via config/genesis/CLI before issuing orders or telemetry; `gov.sorafs_telemetry.require_submitter/require_nonce` defaults stay on, with global and per-provider submitter allow-lists enforced by the executor.
- Repair workers: `/v2/sorafs/audit/repair/{claim,heartbeat,complete,fail}` require signed worker requests (including `manifest_digest`) plus `CanOperateSorafsRepair { provider_id }` on-chain permissions. Provider owners are auto-granted and may delegate/revoke; there is no admin-only repair override path in production.
- SoraNet privacy ingest: `/v2/soranet/privacy/{event,share}` stays disabled until `torii.soranet_privacy_ingest.enabled=true`. Tokens must match `torii.soranet_privacy_ingest.tokens` (`X-SoraNet-Privacy-Token` or `X-API-Token`), submitters must come from `allow_cidrs` (empty list denies), and rate limits apply via `rate_per_sec`/`burst`; rejects emit `soranet_privacy_ingest_reject_total{endpoint,reason}`.
- Operations: when rotating submitter tokens/allow-lists, update `torii.soranet_privacy_ingest.*` and `gov.sorafs_telemetry` maps together, deploy the config bundle, and confirm a test submission succeeds while rejects counters reset; rotate provider ownership with `RegisterProviderOwner`/`UnregisterProviderOwner` before issuing orders/telemetry.

## Escalation Matrix

| Priority | Trigger examples | Primary on-call | Backup | Notes |
|----------|------------------|-----------------|--------|-------|
| P1 | Global gateway outage, PoR failure rate > 5% (15 min), replication backlog doubling every 10 min | Storage SRE | Observability TL | Engage governance council if impact exceeds 30 min. |
| P2 | Regional gateway latency SLO breach, orchestrator retry spike without SLA impact | Observability TL | Storage SRE | Continue rollout but gate new manifests. |
| P3 | Non-critical alerts (manifest staleness, capacity 80–90%) | Intake triage | Ops guild | Address within next business day. |

## Runbooks

### Gateway Outage / Degraded Availability

**Detection**

- Alerts: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Dashboard: `dashboards/grafana/sorafs_gateway_overview.json`.

**Immediate actions**

1. Confirm scope (single provider vs fleet) via request-rate panel.
2. Switch Torii routing to healthy providers (if multi-provider) by toggling
   `sorafs_gateway_route_weights` in the ops config (documented in `docs/source/sorafs_gateway_self_cert.md`).
3. If all providers impacted, enable “direct fetch” fallback for CLI/SDK
   clients (see `docs/source/sorafs_node_client_protocol.md`).

**Triage**

- Check stream token utilisation against `sorafs_gateway_stream_token_limit`.
- Inspect gateway logs for TLS or admission errors.
- Run `scripts/telemetry/run_schema_diff.sh` to ensure the gateway exported
  schema matches the expected version.

**Remediation options**

- Restart only the affected gateway process; do not recycle the entire cluster
  unless multiple providers failing.
- Increase stream token limit by 10–15% temporarily if saturation confirmed.
- Re-run self-cert (`scripts/sorafs_gateway_self_cert.sh`) after stabilisation.

**Post-incident**

- File a P1 postmortem using the template in `docs/source/sorafs/postmortem_template.md`.
- Schedule follow-up chaos drill if remediation relied on manual interventions.

### Proof Failure Spike (PoR / PoTR)

**Detection**

- Alerts: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Dashboard: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetry: `torii_sorafs_proof_stream_events_total` and
  `sorafs.fetch.error` events with `provider_reason=corrupt_proof`.

**Immediate actions**

1. Freeze new manifest admissions by flagging the manifest registry (see
   `docs/source/sorafs/manifest_pipeline.md`).
2. Notify Governance to pause incentives for affected providers.

**Triage**

- Check PoR challenge queue depth vs `sorafs_node_replication_backlog_total`.
- Validate proof verification pipeline (`crates/sorafs_node/src/potr.rs`)
  for recent deployments.
- Compare provider firmware versions with the operator registry.

**Remediation options**

- Trigger PoR replays using `sorafs_cli proof stream` with the latest manifest.
- If proofs consistently fail, remove provider from active set by updating the
  governance registry and forcing orchestrator scoreboards to refresh.

**Post-incident**

- Run the PoR chaos drill scenario before the next production deploy.
- Capture lessons in the postmortem template and update provider
  qualification checklist.

### Replication Lag / Backlog Growth

**Detection**

- Alerts: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. The rule pack
  lives in `dashboards/alerts/sorafs_capacity_rules.yml`; run
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  before publishing changes so Alertmanager stays in sync with Grafana evidence.
- Dashboard: `dashboards/grafana/sorafs_capacity_health.json`.
- Metrics: `sorafs_node_replication_backlog_total`,
  `sorafs_node_manifest_refresh_age_seconds`.

**Immediate actions**

1. Verify backlog scope (single provider or fleet) and pause non-essential
   replication tasks.
2. If backlog isolated, temporarily reassign new orders to alternate
   providers via the replication scheduler.

**Triage**

- Inspect orchestrator telemetry for retry bursts that may cascade backlog.
- Confirm storage targets have sufficient headroom (`sorafs_node_capacity_utilisation_percent`).
- Review recent configuration changes (chunk profile updates, proof cadence).

**Remediation options**

- Run `sorafs_cli` with the `--rebalance` option to redistribute content.
- Scale replication workers horizontally for the impacted provider.
- Trigger manifest refresh to re-align TTL windows.

**Post-incident**

- Schedule a capacity drill focusing on provider saturation failure.
- Update replication SLA documentation in `docs/source/sorafs_node_client_protocol.md`.

### Repair Backlog & SLA Breaches

**Detection**

- Alerts:
  - `SoraFSRepairBacklogHigh` (queue depth > 50 or oldest queued age > 4h for 10m).
  - `SoraFSRepairEscalations` (> 3 escalations/hour).
  - `SoraFSRepairLeaseExpirySpike` (> 5 lease expiries/hour).
  - `SoraFSRetentionBlockedEvictions` (retention blocked by active repairs in last 15m).
- Dashboard: `dashboards/grafana/sorafs_capacity_health.json` (Repair SLA Escalations, Repair Queue Depth by Provider, Retention Blocked Evictions).

**Immediate actions**

1. Identify affected providers (queue depth spikes) and pause new pins/replication orders for them.
2. Verify repair worker liveness (recent heartbeats) and bump worker concurrency if safe.

**Triage**

- Compare `torii_sorafs_repair_backlog_oldest_age_seconds` against the 4h SLA window.
- Inspect `torii_sorafs_repair_lease_expired_total{outcome=...}` for crash/clock-skew patterns.
- Review escalated tickets for repeated manifest/provider pairs and verify evidence bundles.

**Remediation options**

- Reassign or restart stalled repair workers; clear orphaned leases via the normal claim flow.
- Throttle new pins while repairs drain to prevent additional SLA pressure.
- Escalate to governance if escalations persist and attach the repair audit artefacts.

### Retention / GC Inspection (Read-only)

**Detection**

- Alerts: `SoraFSCapacityPressure`, `SoraFSGCStalled`, `SoraFSGCBlocked`, or `SoraFSGCErrorRuns`.
- Dashboard: `dashboards/grafana/sorafs_capacity_health.json` (GC runs/evictions/bytes freed panels).

**Immediate actions**

1. Run a local retention snapshot:
   ```bash
   iroha app sorafs gc inspect --data-dir /var/lib/sorafs
   ```
2. Capture an expired-only view for triage:
   ```bash
   iroha app sorafs gc dry-run --data-dir /var/lib/sorafs
   ```
3. Attach the JSON outputs to the incident ticket for auditability.

**Triage**

- Confirm which manifests report `retention_epoch=0` (no expiry) vs. those with deadlines.
- Use `retention_sources` in the GC JSON output to see which constraint set the effective
  retention (`deal_end`, `governance_cap`, `pin_policy`, or `unbounded`). Deal and governance caps
  are supplied via manifest metadata keys `sorafs.retention.deal_end_epoch` and
  `sorafs.retention.governance_cap_epoch`.
- If `dry-run` reports expired manifests but capacity remains pinned, verify no
  active repairs or retention policy overrides block eviction.
- Check `torii_sorafs_gc_expired_manifests`, `torii_sorafs_gc_oldest_expired_age_seconds`,
  `torii_sorafs_gc_evictions_total`, and `torii_sorafs_gc_blocked_total` to confirm GC is running
  and to identify block reasons (for example, `repair_active`).
  Capacity-triggered sweeps evict expired manifests by least-recently-used order with
  `manifest_id` tie-breakers.

**Remediation options**

- The GC CLI is read-only. Do not delete manifests or chunks manually in production.
- Escalate to governance for retention policy adjustments or capacity expansion
  when expired data accumulates without automated eviction.
- If GC is blocked by repairs, address outstanding repair tickets first (see the
  repair worker runbook section) to unblock retention cleanup.

## Chaos Drill Cadence

- **Quarterly**: Combined gateway outage + orchestrator retry storm simulation.
- **Biannual**: PoR/PoTR failure injection across two providers with recovery.
- **Monthly spot-check**: Replication lag scenario using staging manifests.
- Track drills in the shared runbook log (`ops/drill-log.md`) via the helper
  script:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

  The script appends a Markdown row with the provided metadata so drills remain
  auditable and can be correlated with postmortems or governance reports. Validate
  the log before merging changes with:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Use `--status scheduled` when announcing upcoming drills, `pass`/`fail` for
  completed runs, and `follow-up` when action items remain open.
- Override the destination with `--log` for dry-runs or automated verification;
  without it the script continues to update `ops/drill-log.md`.

## Postmortem Template

Use `docs/source/sorafs/postmortem_template.md` for every P1/P2 incident and
for chaos drill retrospectives. The template includes sections for timeline,
impact quantification, contributing factors, corrective actions, and follow-up
verification tasks.
