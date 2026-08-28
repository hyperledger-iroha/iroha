---
title: SoraFS Orchestrator Telemetry & Alerting Plan
summary: Instrumentation, dashboards, and alert catalog for the multi-source fetch orchestrator.
---

# SoraFS Orchestrator Telemetry & Alerting Plan

## Metrics

The orchestrator emits Prometheus metrics via `iroha_telemetry::metrics::Metrics` and structured
`telemetry::sorafs.fetch.*` events through the shared logger. There is no in-process OTLP metrics
exporter; deployments that require OTLP can translate the Prometheus scrape or structured event
stream in their collector layer.

**Prometheus / scrape path**

- `sorafs_orchestrator_active_fetches{manifest_id,region}` — active fetch sessions.
- `sorafs_orchestrator_fetch_duration_ms_bucket{manifest_id,region,le}` — fetch duration histogram.
- `sorafs_orchestrator_fetch_failures_total{manifest_id,region,failure_reason}` — orchestrator failures.
- `sorafs_orchestrator_retries_total{manifest_id,provider_id,retry_reason}` — retry counts by provider.
- `sorafs_orchestrator_provider_failures_total{manifest_id,provider_id,failure_reason}` — provider failure matrix.

## Events/Logs

- Provider ban/unban with reason.
- Token exhaustion events.
- Proof verification failure details.

## Dashboards

- **Overview**: throughput, success ratio, active fetches.
- **Provider Health**: failure counts, latency, stall rate.
- **Retries**: histogram and cumulative count by reason.
- **Per-Manifest**: chunk progress, outstanding tokens.

See `fixtures/documentation/sorafs_fetch_dashboard.json` for the baseline Grafana dashboard wired to these
metrics (matching the panels described above).

## Alerts

- Success ratio < 99% for >5 minutes.
- Provider failure rate > 5% over 10-minute window.
- Chunk latency P95 > 250 ms sustained >10 minutes.
- Token exhaustion events > threshold per minute.
- Proof verification failures observed (with severity rating).

Alerting rules are codified in `fixtures/documentation/sorafs_fetch_alerts.yaml`, suitable for ingestion by
Prometheus Alertmanager or Mimir ruler.

## Integration

- Scrape orchestrator metrics into Prometheus/Mimir.
- Link dashboards with gateway telemetry (shared labels `manifest`, `provider`).
- Align alerts with SLOs defined in `sorafs_observability_plan.md`.

## Implementation Status

- Prometheus gauges/counters are emitted from `crates/sorafs_orchestrator/src/lib.rs` via
  `FetchMetricsCtx`, ensuring scoreboard-driven fetches always update success/failure telemetry.
- Lifecycle, retry, provider-failure, stall, and terminal-error records are emitted by
  `FetchTelemetryCtx` as bounded structured events. CLI and SDK integrations do not need to install
  or coordinate a second metrics provider.

## Rollout & Tuning Guide

- **Configuration.** Register the shared `Metrics` instance with the existing `/metrics` endpoint and
  configure the deployment's Prometheus-compatible collector to scrape it. Region is supplied by the
  orchestrator call and is already present on the relevant series.
- **Sampling cadence.** Choose a scrape interval appropriate for the alert windows (5–15 seconds is
  typical for staging). The application does not own a second push cadence.
- **Failure scenarios.**
  - Rising `sorafs_orchestrator_fetch_failures_total` usually indicates capability mismatches or exhausted retry
    budgets—inspect the `failure_reason` labels and gate the orchestrator accordingly.
  - Spikes in `sorafs_orchestrator_provider_failures_total` pinpoint unhealthy providers; correlate with the
    “Retries per Provider” panel to decide on temporary blacklisting.
  - Sustained `sorafs_orchestrator_fetch_duration_ms` p95 above 250 ms triggers the bundled alert rule; tune
    `FetchOptions::global_parallel_limit` and retry budgets before paging.
- **Validation.** Start rollout with the Grafana panels in `fixtures/documentation/sorafs_fetch_dashboard.json`,
  then enable the alert set from `fixtures/documentation/sorafs_fetch_alerts.yaml` once baseline latency and
  failure rates stabilise.

## Label Taxonomy

Orchestrator metrics adopt the same label casing and semantic rules as the gateway and node plans so
cross-service joins remain deterministic.

| Label | Applies to | Description |
|-------|------------|-------------|
| `manifest_id` | gauges, histograms, counters | Stable Norito CID of the manifest being fetched. Absent for global metrics (e.g., total retries). |
| `provider_id` | provider-scoped metrics | Governance-issued provider identifier (`prov_xxx`). For multi-hop fetches this is the terminal provider serving the chunk. |
| `job_id` | structured events | Random identifier that groups a single fetch request and its retries without adding unbounded Prometheus labels. |
| `region` | all metrics | Orchestrator deployment region (`us-east-1`, `eu-central-1`). Enables regional dashboards and alert routing. |
| `failure_reason` | failure counters | Enumerated reason (`timeout`, `digest_mismatch`, `http_5xx`, `token_exhausted`). |
| `retry_reason` | retry metrics | Classification for retries (`retry`, `session_failure`, `length_mismatch`, etc.). |

Label hygiene rules:
- Prefer `manifest_id` + `provider_id` for joins. If either is missing, downstream dashboards treat the
  metric as aggregate.
- Keep cardinality bounded by retaining `job_id` in structured events rather than adding it as a
  regular label on Prometheus histograms.

## Metric Delivery Architecture

- **Export mechanism.** The existing `/metrics` handler exposes the canonical Prometheus registry.
- **Collector pipeline.** A Prometheus-compatible collector scrapes that endpoint and may remote-write
  into Mimir or another metrics backend. Structured `telemetry::sorafs.fetch.*` events flow through the
  configured log pipeline.
- **Failure behavior.** A collector outage does not create an application-side telemetry queue or
  alter fetch execution. Scraping resumes from the current counters and gauges when the collector
  recovers.
- **Correlation.** `manifest_id`, `provider_id`, and `region` link metrics to structured events;
  `job_id` remains event-only to avoid unbounded metric cardinality.

## Grafana Coordination

- **Template library.** Work with the Observability team to add orchestrator panels to the shared Grafana
  library (`grafana/provisioning/dashboards/sorafs.jsonnet`). Templates include KPI overview, provider
  drill-down, retry matrix, and manifest progress boards.
- **Panel ownership.** Define dashboard owners (`Storage On-Call`) and add contact metadata so alerts link
  directly to responsible rotation.
- **Panel validation.** Run staging dry-runs combining orchestrator metrics with gateway dashboards to
  ensure label joins (`manifest_id`, `provider_id`, `region`) align. Update `sorafs_observability_plan.md`
  with screenshots/links once panels stabilize.
- **Alert integration.** Observability team provisions Alertmanager routes mapping orchestrator alerts to
  Slack/PagerDuty channels. Each alert rule references the shared SLO definitions and uses consistent
  annotations (`summary`, `runbook_url`) so operators land on the same remediation guides.
