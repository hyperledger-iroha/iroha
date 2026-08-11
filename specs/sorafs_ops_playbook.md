---
title: SoraFS Operations Playbook
summary: Incident response guides and chaos drill procedures for SoraFS operators.
---

# SoraFS Operations Playbook

This playbook bundles the operational runbooks required for SF-7b. It covers the
high-priority incident classes (gateway outage, proof failure, replication lag),
defines the chaos-drill cadence, and provides a reusable postmortem template for
incident retrospectives.

> Public and in-depth documentation is maintained in the sibling
> `iroha-docs` repository and published at <https://docs.iroha.tech/>.

## Key References

- Observability assets: refer to the Grafana dashboards under
  `dashboards/grafana/` and Prometheus alert rules in `dashboards/alerts/`.
- Metric catalog: `specs/sorafs_observability_plan.md`.
- Orchestrator telemetry surfaces: `specs/sorafs_orchestrator_plan.md`.

## Auth & Governance Checklist

- Pin lifecycle: registration is a paid public operation for any authenticated
  account and has no general permission token. Approval authority is the
  verified threshold-governance envelope (which an authenticated account may
  relay), retirement is restricted to the recorded submitter, and only alias
  attachment uses `CanBindSorafsAlias`. Keep the remaining provider,
  telemetry, dispute, replication, pricing, and credit permissions in sync
  with governance onboarding/offboarding.
- Provider binding: configuration may establish bindings only before genesis. Runtime establishment, compare-and-set rebind, and compare-and-remove require an enacted `SorafsProviderGovernanceActionV1`; direct owner ISIs reject. Drain capacity and native reserve state before proposing rebind/removal. `gov.sorafs_telemetry.require_submitter/require_nonce` defaults stay on, with global and per-provider submitter allow-lists enforced by the executor.
- Repair workers: `/v1/sorafs/audit/repair/{claim,heartbeat,complete,fail}`
  accept one caller-signed Iroha transaction containing exactly the matching
  native action and return `202 Accepted` from strict durable ingress. Native
  execution requires the expected task revision, exact live lease generation
  where applicable, and provider-scoped `CanOperateSorafsRepair`; workers must
  reconcile the finalized task/event projection before storage work. Provider
  owners may delegate/revoke the permission, and there is no admin-only repair
  override.
- SoraNet privacy ingest: `/v1/soranet/privacy/{event,share}` stays disabled until `torii.soranet_privacy_ingest.enabled=true`. Every request requires a fresh exact NetworkId-bound operator signature from an allow-listed key; submitters must also come from `allow_cidrs` (empty list denies), and per-operator limits apply via `rate_per_sec`/`burst`. Bearer collector/API tokens are retired; rejects emit `soranet_privacy_ingest_reject_total{endpoint,reason}`.
- Operations: when rotating submitter keys/allow-lists, update `torii.operator_signatures`, `torii.soranet_privacy_ingest`, and `gov.sorafs_telemetry` together, deploy the config bundle, and confirm a signed test submission succeeds while unsigned/replayed/wrong-network samples fail; rotate provider ownership only through the native Parliament proposal/enactment lifecycle before issuing new orders or telemetry.

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
   `sorafs_gateway_route_weights` in the ops config (documented in `specs/sorafs_gateway_self_cert.md`).
3. If all providers impacted, enable “direct fetch” fallback for CLI/SDK
   clients (see `specs/sorafs_node_client_protocol.md`).

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

- File a P1 postmortem using the template in `specs/sorafs/postmortem_template.md`.
- Schedule follow-up chaos drill if remediation relied on manual interventions.

### Proof Failure Spike (PoR / PoTR)

**Detection**

- Alerts: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Dashboard: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetry: `torii_sorafs_proof_stream_events_total` and
  `sorafs.fetch.error` events with `provider_reason=corrupt_proof`.

**Immediate actions**

1. Freeze new manifest admissions by flagging the manifest registry (see
   `specs/sorafs/manifest_pipeline.md`).
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
- Update replication SLA documentation in `specs/sorafs_node_client_protocol.md`.

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
    --link "specs/sorafs/postmortem_template.md"
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

Use `specs/sorafs/postmortem_template.md` for every P1/P2 incident and
for chaos drill retrospectives. The template includes sections for timeline,
impact quantification, contributing factors, corrective actions, and follow-up
verification tasks.
