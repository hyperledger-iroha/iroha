---
title: SoraFS Observability & SLO Plan
summary: SF-7 telemetry schema, dashboards, and SLO enforcement for gateways and nodes.
---

# SoraFS Observability & SLO Plan

> **Note:** This plan is also mirrored in the Docusaurus developer portal at
> `docs/portal/docs/sorafs/observability-plan.md`. Keep both copies in sync until
> the migration away from the Sphinx documentation set is complete.

## Objectives
- Define metrics/events schema for gateways/nodes/orchestrator.
- Provide Grafana dashboards and alert thresholds.
- Establish SLOs and error budget policies.

## Metric Catalog

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
the last 15 minutes, while `SorafsProofHealthCooldown` raises a warning if a
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
| `telemetry::sorafs.fetch.error` | Structured event | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Terminal failure record, friendly to Loki/splunk ingestion. |
| `telemetry::sorafs.fetch.stall` | Structured event | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Raised when chunk latency breaches the configured cap (mirrors stall counters). |

Example lifecycle payload (redacted fields follow standard `iroha_logger` rules):

```json
{
  "event": "complete",
  "status": "success",
  "manifest": "0b3d…7ac2",
  "region": "us-west-2",
  "job_id": "a17fd9e0b3c84fc48f0eed3ff30d1fd3",
  "chunk_count": 128,
  "total_bytes": 67108864,
  "provider_candidates": 5,
  "retry_budget": 3,
  "retry_budget_unbounded": false,
  "global_parallel_limit": 4,
  "global_parallel_unbounded": false,
  "duration_ms": 1845.23,
  "retries_total": 2,
  "provider_failures_total": 1
}
```

### Node / replication surfaces

| Metric | Type | Labels | Notes |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histogram | `provider_id` | OTEL histogram of storage utilisation percentage (exported as `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Counter | `provider_id` | Monotonic counter for successful PoR samples, derived from scheduler snapshots. |
| `sorafs_node_por_failure_total` | Counter | `provider_id` | Monotonic counter for failed PoR samples. |
| `sorafs_node_deal_publish_total` | Counter | `provider_id`, `result` | Tagged with `result=success|failure` every time a settlement artefact is written to (or fails to reach) the governance DAG. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Gauge | `provider` | Existing Prometheus gauges for bytes used, queue depth, PoR inflight counts. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Gauge | `provider` | Provider capacity/uptime success data surfaced in the capacity dashboard. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Gauge | `provider`, `manifest` | Backlog depth plus the cumulative failure counters exported whenever `/v1/sorafs/por/ingestion/{manifest}` is polled, feeding the “PoR Stalls” panel/alert. |

### Proof of Timely Retrieval (PoTR) and chunk SLA

| Metric | Type | Labels | Producer | Notes |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Histogram | `tier`, `provider` | PoTR coordinator | Deadline slack in milliseconds (positive = met). |
| `sorafs_potr_failures_total` | Counter | `tier`, `provider`, `reason` | PoTR coordinator | Captures reasons (`expired`, `missing_proof`, `corrupt_proof`). |
| `sorafs_chunk_sla_violation_total` | Counter | `provider`, `manifest_id`, `reason` | SLA monitor | Fired when chunk delivery misses SLO (latency, success rate). |
| `sorafs_chunk_sla_violation_active` | Gauge | `provider`, `manifest_id` | SLA monitor | Boolean gauge (0/1) toggled during active breach window. |

## SLO Targets

- Gateway trustless availability: 99.9% (HTTP 2xx/304 responses).
- Trustless TTFB P95: Hot tier ≤ 120 ms, warm tier ≤ 300 ms.
- Proof success rate: ≥ 99.5% per day.
- Orchestrator success (chunk completion) ≥ 99%.

## Dashboards

1. **Gateway Observability** (`dashboards/grafana/sorafs_gateway_observability.json`)
   - Tracks trustless availability, TTFB P95, refusal breakdown, and PoR/PoTR failure rates using the new OTEL metrics.
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`)
   - Existing dashboard covering multi-source orchestrator load (active fetches, retries, provider failures).
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`)
   - Surfaces per-minute anonymised buckets, suppression windows, and collector health driven by `soranet_privacy_last_poll_unixtime`,
     `soranet_privacy_collector_enabled`, `soranet_privacy_poll_errors_total{provider}`, and the aggregated circuit counters.
4. **PDP & PoTR Health** (`dashboards/grafana/sorafs_pdp_potr_health.json`)
   - Consolidates SF-13/SF-14 telemetry for DA-5: PDP challenge rates/success, latency P95, duplicate counters, slash proposal history, PoTR latency histograms, and failure breakdowns filtered by provider/tier so Taikai/CDN reviewers can audit proof health before gating releases.

## Alerts

- `dashboards/alerts/sorafs_gateway_rules.yml` — gateway availability, TTFB P95, proof failure spikes.
- `dashboards/alerts/sorafs_fetch_rules.yml` — orchestrator failure/retry spikes (unchanged).
- `dashboards/alerts/soranet_privacy_rules.yml` — downgrade spikes, bucket suppression, collector-idle detection, and disabled-collector alerts driven by `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`, and poll failure counters.
- `dashboards/alerts/soranet_policy_rules.yml` — anonymity brownout alarms tied to `sorafs_orchestrator_brownouts_total` so SNNet-5 default-on rollouts stay gated.
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai viewer drift/ingest/CEK lag alarms plus the new SoraFS proof-health penalty/cooldown alerts derived from `torii_sorafs_proof_health_*`.
- `scripts/telemetry/test_sorafs_fetch_alerts.sh` exercises
  `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`,
  `dashboards/alerts/tests/soranet_privacy_rules.test.yml`, and
  `dashboards/alerts/tests/soranet_policy_rules.test.yml` via `promtool test rules`
  so the stall burst SLO gates, privacy suppression checks, collector-disabled alarms, and anonymity brownout detectors stay regression-tested alongside chaos harnesses.

## Data Pipeline

- Collectors: OpenTelemetry agents on gateways/nodes.
- Backend: Prometheus + Loki + Tempo (or OTEL exporters to Mimir/Loki).
- Optional eBPF (Tetragon) for low-level tracing.
- OTLP exporters: call `iroha_telemetry::metrics::install_sorafs_gateway_otlp_exporter` for Torii gateways and `install_sorafs_node_otlp_exporter` inside embedded nodes (the orchestrator continues to use `install_sorafs_fetch_otlp_exporter`).

## Metric Naming / Label Conventions

- Metric names follow the existing `torii_sorafs_*` or `sorafs_*` prefixes used by Torii and the
  gateway. Label sets are standardised as:
  - `result` → HTTP outcome (`success`, `refused`, `failed`).
  - `reason` → refusal/error code (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → hex-encoded provider identifier.
  - `manifest` → canonical manifest digest (trimmed for high-cardinality exports).
  - `tier` → declarative tier labels (`hot`, `warm`, `archive`).
- Telemetry emission points:
- Gateway metrics now live under `torii_sorafs_*` and should reuse the naming/conventions adopted
  in `crates/iroha_core/src/telemetry.rs`.
- Orchestrator metrics use the `sorafs_orchestrator_*` prefix noted in the orchestrator plan.
- Orchestrator telemetry emits `telemetry::sorafs.fetch.*` events (`lifecycle`, `retry`,
  `provider_failure`, `error`) tagged with the manifest digest, randomly generated job identifier,
  region, and provider identifiers so dashboards can correlate per-session behaviour.
- Nodes expose `torii_sorafs_storage_*`, `torii_sorafs_capacity_*`, and `torii_sorafs_por_*`
  gauges/counters from the embedded SoraFS node (`crates/sorafs_node`).
- Work with the Observability team to register the metric catalog in the shared Prometheus naming
  doc and include label cardinality expectations (max providers, manifests).

## Tracing Strategy

- Adopt OpenTelemetry end-to-end:
  - Gateways emit OTLP spans (HTTP) annotated with request IDs, manifest digests, and token hash.
  - Orchestrator uses `tracing` + `opentelemetry` crates to export spans for fetch attempts.
  - Sora validators/observers (embedded SoraFS node) export spans for PoR challenges and storage
    operations. All components share a common trace-id propagated via headers (`x-sorafs-trace`).
- `SorafsFetchOtel` bridges the orchestrator metrics into OTLP gauges/histograms while the
  `telemetry::sorafs.fetch.*` events provide lightweight JSON payloads for log/metric backends that
  prefer structured events over scrapes.
- Collectors: run OTEL collectors alongside Prometheus/Loki/Tempo. Operators can forward to Tempo
  (preferred) or Jaeger-compatible backends.
- For high-cardinality operations, attach sampling rules (10% for success, 100% for failures).

## TLS Telemetry Coordination (SF-5b)

- Metric alignment:
  - The TLS automation plan ships `sorafs_gateway_tls_cert_expiry_seconds`,
    `sorafs_gateway_tls_renewal_total{result}`, and `sorafs_gateway_tls_ech_enabled` counters.
  - Include these gauges in the Gateway Overview dashboard under the TLS/Certificates panel.
- Alert linkage:
  - When TLS expiry alerts fire (≤ 14 days remaining) correlate with gateway availability SLO.
  - ECH disablement should emit a secondary alert that references both the TLS and trustless
    availability SLO panels.
- Data pipeline: the TLS automation job publishes events to the same Prometheus stack used for
  gateway metrics; dashboards and Grafana should reference the shared data source. Coordinated work
  items are tracked under SF-5b and SF-7 to prevent duplication.
