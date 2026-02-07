---
id: observability-plan
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Observability & SLO Plan
sidebar_label: Observability & SLOs
description: Telemetry schema, dashboards, and error-budget policy for SoraFS gateways, nodes, and the multi-source orchestrator.
---

:::note Canonical Source
:::

## Objectives
- Define metrics and structured events for gateways, nodes, and the multi-source orchestrator.
- Provide Grafana dashboards, alert thresholds, and validation hooks.
- Establish SLO targets alongside error-budget and chaos-drill policies.

## Metric Catalogue

### Gateway surfaces

| Metric | Type | Labels | Notes |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Gauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Emitted via `SorafsGatewayOtel`; tracks in-flight HTTP operations per endpoint/method combination. |
| `sorafs_gateway_responses_total` | Counter | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Every completed gateway request increments once; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogram | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Time-to-first-byte latency for gateway responses; exported as Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Counter | `profile_version`, `result`, `error_code` | Proof verification outcomes captured at request time (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogram | `profile_version`, `result`, `error_code` | Verification latency distribution for PoR receipts. |
| `telemetry::sorafs.gateway.request` | Structured event | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Structured log emitted on every request completion for Loki/Tempo correlation. |

`telemetry::sorafs.gateway.request` events mirror the OTEL counters with structured payloads, surfacing `endpoint`, `method`, `variant`, `status`, `error_code`, and `duration_ms` for Loki/Tempo correlation while dashboards consume the OTLP series for SLO tracking.

### Proof-health telemetry

| Metric | Type | Labels | Notes |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Counter | `provider_id`, `trigger`, `penalty` | Increments every time `RecordCapacityTelemetry` emits a `SorafsProofHealthAlert`. `trigger` distinguishes PDP/PoTR/Both failures, while `penalty` captures whether collateral was actually slashed or suppressed by cooldown. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Gauge | `provider_id` | Latest PDP/PoTR counts reported inside the offending telemetry window so teams can quantify how far providers overshot policy. |
| `torii_sorafs_proof_health_penalty_nano` | Gauge | `provider_id` | Nano-XOR amount slashed on the last alert (zero when cooldown suppressed enforcement). |
| `torii_sorafs_proof_health_cooldown` | Gauge | `provider_id` | Boolean gauge (`1` = alert suppressed by cooldown) to surface when follow-up alerts are temporarily muted. |
| `torii_sorafs_proof_health_window_end_epoch` | Gauge | `provider_id` | Epoch recorded for the telemetry window tied to the alert so operators can correlate against Norito artefacts. |

These feeds now power the Taikai viewer dashboard’s proof-health row
(`dashboards/grafana/taikai_viewer.json`), giving CDN operators live visibility
into alert volumes, PDP/PoTR trigger mix, penalties, and cooldown state per
provider.

The same metrics now back two Taikai viewer alert rules:
`SorafsProofHealthPenalty` fires whenever
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` increases in
the last 15 minutes, while `SorafsProofHealthCooldown` raises a warning if a
provider remains in cooldown for five minutes. Both alerts live in
`dashboards/alerts/taikai_viewer_rules.yml` so SREs receive immediate context
whenever PoR/PoTR enforcement escalates.

### Orchestrator surfaces

| Metric / Event | Type | Labels | Producer | Notes |
|----------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Gauge | `manifest_id`, `region` | `FetchMetricsCtx` | Sessions currently in-flight. |
| `sorafs_orchestrator_fetch_duration_ms` | Histogram | `manifest_id`, `region` | `FetchMetricsCtx` | Duration histogram in milliseconds; 1 ms→30 s buckets. |
| `sorafs_orchestrator_fetch_failures_total` | Counter | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Reasons: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Distinguishes retry causes (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Captures session-level disablement / failure tallies. |
| `sorafs_orchestrator_chunk_latency_ms` | Histogram | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Per-chunk fetch latency distribution (ms) for throughput/SLO analysis. |
| `sorafs_orchestrator_bytes_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Bytes delivered per manifest/provider; derive throughput via `rate()` in PromQL. |
| `sorafs_orchestrator_stalls_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Counts chunks exceeding `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Structured event | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Mirrors job lifecycle (start/complete) with Norito JSON payload. |
| `telemetry::sorafs.fetch.retry` | Structured event | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Emitted per provider retry streak; `attempts` counts incremental retries (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Structured event | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Surfaced when a provider crosses the failure threshold. |
| `telemetry::sorafs.fetch.error` | Structured event | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Terminal failure record, friendly to Loki/Splunk ingestion. |
| `telemetry::sorafs.fetch.stall` | Structured event | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Raised when chunk latency breaches the configured cap (mirrors stall counters). |

### Node / replication surfaces

| Metric | Type | Labels | Notes |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histogram | `provider_id` | OTEL histogram of storage utilisation percentage (exported as `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Counter | `provider_id` | Monotonic counter for successful PoR samples, derived from scheduler snapshots. |
| `sorafs_node_por_failure_total` | Counter | `provider_id` | Monotonic counter for failed PoR samples. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Gauge | `provider` | Existing Prometheus gauges for bytes used, queue depth, PoR inflight counts. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Gauge | `provider` | Provider capacity/uptime success data surfaced in the capacity dashboard. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Gauge | `provider`, `manifest` | Backlog depth plus the cumulative failure counters exported whenever `/v1/sorafs/por/ingestion/{manifest}` is polled, feeding the “PoR Stalls” panel/alert. |

### Repair & SLA

| Metric | Type | Labels | Notes |
|--------|------|--------|-------|
| `sorafs_repair_tasks_total` | Counter | `status` | OTEL counter for repair task transitions. |
| `sorafs_repair_latency_minutes` | Histogram | `outcome` | OTEL histogram for repair lifecycle latency. |
| `sorafs_repair_queue_depth` | Histogram | `provider` | OTEL histogram of queued tasks per provider (snapshot-style). |
| `sorafs_repair_backlog_oldest_age_seconds` | Histogram | — | OTEL histogram of the oldest queued task age (seconds). |
| `sorafs_repair_lease_expired_total` | Counter | `outcome` | OTEL counter for lease expiries (`requeued`/`escalated`). |
| `sorafs_repair_slash_proposals_total` | Counter | `outcome` | OTEL counter for slash proposal transitions. |
| `torii_sorafs_repair_tasks_total` | Counter | `status` | Prometheus counter for task transitions. |
| `torii_sorafs_repair_latency_minutes_bucket` | Histogram | `outcome` | Prometheus histogram for repair lifecycle latency. |
| `torii_sorafs_repair_queue_depth` | Gauge | `provider` | Prometheus gauge for queued tasks per provider. |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | Gauge | — | Prometheus gauge for the oldest queued task age (seconds). |
| `torii_sorafs_repair_lease_expired_total` | Counter | `outcome` | Prometheus counter for lease expiries. |
| `torii_sorafs_slash_proposals_total` | Counter | `outcome` | Prometheus counter for slash proposal transitions. |

Governance audit JSON metadata mirrors the repair telemetry labels (`status`, `ticket_id`, `manifest`, `provider` on repair events; `outcome` on slash proposals) so metrics and audit artefacts can be correlated deterministically.

### Retention & GC

| Metric | Type | Labels | Notes |
|--------|------|--------|-------|
| `sorafs_gc_runs_total` | Counter | `result` | OTEL counter for GC sweeps, emitted by the embedded node. |
| `sorafs_gc_evictions_total` | Counter | `reason` | OTEL counter for evicted manifests grouped by reason. |
| `sorafs_gc_bytes_freed_total` | Counter | `reason` | OTEL counter for bytes freed grouped by reason. |
| `sorafs_gc_blocked_total` | Counter | `reason` | OTEL counter for evictions blocked by active repairs or policy. |
| `torii_sorafs_gc_runs_total` | Counter | `result` | Prometheus counter for GC sweeps (success/error). |
| `torii_sorafs_gc_evictions_total` | Counter | `reason` | Prometheus counter for evicted manifests grouped by reason. |
| `torii_sorafs_gc_bytes_freed_total` | Counter | `reason` | Prometheus counter for bytes freed grouped by reason. |
| `torii_sorafs_gc_blocked_total` | Counter | `reason` | Prometheus counter for blocked evictions grouped by reason. |
| `torii_sorafs_gc_expired_manifests` | Gauge | — | Current count of expired manifests observed by GC sweeps. |
| `torii_sorafs_gc_oldest_expired_age_seconds` | Gauge | — | Age in seconds of the oldest expired manifest (after retention grace). |

### Reconciliation

| Metric | Type | Labels | Notes |
|--------|------|--------|-------|
| `sorafs.reconciliation.runs_total` | Counter | `result` | OTEL counter for reconciliation snapshots. |
| `sorafs.reconciliation.divergence_total` | Counter | — | OTEL counter of divergence counts per run. |
| `torii_sorafs_reconciliation_runs_total` | Counter | `result` | Prometheus counter for reconciliation runs. |
| `torii_sorafs_reconciliation_divergence_count` | Gauge | — | Latest divergence count observed in a reconciliation report. |

### Proof of Timely Retrieval (PoTR) & chunk SLA

| Metric | Type | Labels | Producer | Notes |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Histogram | `tier`, `provider` | PoTR coordinator | Deadline slack in milliseconds (positive = met). |
| `sorafs_potr_failures_total` | Counter | `tier`, `provider`, `reason` | PoTR coordinator | Reasons: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Counter | `provider`, `manifest_id`, `reason` | SLA monitor | Fired when chunk delivery misses SLO (latency, success rate). |
| `sorafs_chunk_sla_violation_active` | Gauge | `provider`, `manifest_id` | SLA monitor | Boolean gauge (0/1) toggled during active breach window. |

## SLO Targets

- Gateway trustless availability: **99.9 %** (HTTP 2xx/304 responses).
- Trustless TTFB P95: hot tier ≤ 120 ms, warm tier ≤ 300 ms.
- Proof success rate: ≥ 99.5 % per day.
- Orchestrator success (chunk completion): ≥ 99 %.

## Dashboards & Alerting

1. **Gateway Observability** (`dashboards/grafana/sorafs_gateway_observability.json`) — tracks trustless availability, TTFB P95, refusal breakdown, and PoR/PoTR failures via the OTEL metrics.
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`) — covers multi-source load, retries, provider failures, and stall bursts.
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`) — charts anonymised relay buckets, suppression windows, and collector health via `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`, and `soranet_privacy_poll_errors_total{provider}`.
4. **Capacity Health** (`dashboards/grafana/sorafs_capacity_health.json`) — tracks provider headroom plus repair SLA escalations, repair queue depth by provider, and GC sweeps/evictions/bytes freed/blocked reasons/expired-manifest age and reconciliation divergence snapshots.

Alert bundles:

- `dashboards/alerts/sorafs_gateway_rules.yml` — gateway availability, TTFB, proof failure spikes.
- `dashboards/alerts/sorafs_fetch_rules.yml` — orchestrator failures/retries/stalls; validated via `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml`, and `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/sorafs_capacity_rules.yml` — capacity pressure plus repair SLA/backlog/lease-expiry alerts and GC stall/blocked/error alerts for retention sweeps.
- `dashboards/alerts/soranet_privacy_rules.yml` — privacy downgrade spikes, suppression alarms, collector-idle detection, and disabled-collector alerts (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — anonymity brownout alarms wired to `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai viewer drift/ingest/CEK lag alarms plus the new SoraFS proof-health penalty/cooldown alerts powered by `torii_sorafs_proof_health_*`.

## Tracing Strategy

- Adopt OpenTelemetry end-to-end:
  - Gateways emit OTLP spans (HTTP) annotated with request IDs, manifest digests, and token hashes.
  - The orchestrator uses `tracing` + `opentelemetry` to export spans for fetch attempts.
  - Embedded SoraFS nodes export spans for PoR challenges and storage operations. All components share a common trace ID propagated via `x-sorafs-trace`.
- `SorafsFetchOtel` bridges orchestrator metrics into OTLP histograms while `telemetry::sorafs.fetch.*` events provide lightweight JSON payloads for log-centric backends.
- Collectors: run OTEL collectors alongside Prometheus/Loki/Tempo (Tempo preferred). Jaeger API exporters remain optional.
- High-cardinality operations should be sampled (10 % for success paths, 100 % for failures).

## TLS Telemetry Coordination (SF-5b)

- Metric alignment:
  - TLS automation ships `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}`, and `sorafs_gateway_tls_ech_enabled`.
  - Include these gauges in the Gateway Overview dashboard under the TLS/Certificates panel.
- Alert linkage:
  - When TLS expiry alerts fire (≤ 14 days remaining) correlate with the trustless availability SLO.
  - ECH disablement emits a secondary alert referencing both TLS and availability panels.
- Pipeline: the TLS automation job exports to the same Prometheus stack as gateway metrics; coordination with SF-5b ensures deduplicated instrumentation.

## Metric Naming & Label Conventions

- Metric names follow the existing `torii_sorafs_*` or `sorafs_*` prefixes used by Torii and the gateway.
- Label sets are standardised:
  - `result` → HTTP outcome (`success`, `refused`, `failed`).
  - `reason` → refusal/error code (`unsupported_chunker`, `timeout`, etc.).
  - `status` → repair task state (`queued`, `in_progress`, `completed`, `failed`, `escalated`).
  - `outcome` → repair lease or latency outcome (`requeued`, `escalated`, `completed`, `failed`).
  - `provider` → hex-encoded provider identifier.
  - `manifest` → canonical manifest digest (trimmed when high-cardinality).
  - `tier` → declarative tier labels (`hot`, `warm`, `archive`).
- Telemetry emission points:
  - Gateway metrics live under `torii_sorafs_*` and reuse conventions from `crates/iroha_core/src/telemetry.rs`.
  - The orchestrator emits `sorafs_orchestrator_*` metrics and `telemetry::sorafs.fetch.*` events (lifecycle, retry, provider failure, error, stall) tagged with manifest digest, job ID, region, and provider identifiers.
  - Nodes surface `torii_sorafs_storage_*`, `torii_sorafs_capacity_*`, and `torii_sorafs_por_*`.
- Coordinate with Observability to register the metric catalogue in the shared Prometheus naming doc, including label cardinality expectations (provider/manifests upper bounds).

## Data Pipeline

- Collectors deploy alongside each component, exporting OTLP to Prometheus (metrics) and Loki/Tempo (logs/traces).
- Optional eBPF (Tetragon) enriches low-level tracing for gateways/nodes.
- Use `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` for Torii and embedded nodes; the orchestrator continues to call `install_sorafs_fetch_otlp_exporter`.

## Validation Hooks

- Run `scripts/telemetry/test_sorafs_fetch_alerts.sh` during CI to ensure Prometheus alert rules remain in lockstep with stall metrics and privacy suppression checks.
- Keep Grafana dashboards under version control (`dashboards/grafana/`) and update screenshots/links when panels change.
- Chaos drills log outcomes via `scripts/telemetry/log_sorafs_drill.sh`; validation leverages `scripts/telemetry/validate_drill_log.sh` (see the [Operations Playbook](operations-playbook.md)).
