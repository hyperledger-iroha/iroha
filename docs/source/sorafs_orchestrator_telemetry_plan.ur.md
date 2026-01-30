---
lang: ur
direction: rtl
source: docs/source/sorafs_orchestrator_telemetry_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f697d58faa5c634516a58566671cae33538fff8de31422bc34e4e860c0e8dc13
source_last_modified: "2026-01-03T18:07:57.023587+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Orchestrator Telemetry & Alerting Plan
summary: Instrumentation, dashboards, and alert catalog for the multi-source fetch orchestrator.
---

# SoraFS Orchestrator Telemetry & Alerting Plan

## Metrics

The orchestrator emits Prometheus metrics via `iroha_telemetry::metrics::Metrics`, and mirrors the
same signals through OpenTelemetry instruments prefixed with `sorafs.fetch.*` when the
`iroha_telemetry/otel-exporter` feature is enabled.

**Prometheus / scrape path**

- `sorafs_orchestrator_active_fetches{manifest_id,region}` — active fetch sessions.
- `sorafs_orchestrator_fetch_duration_ms_bucket{manifest_id,region,le}` — fetch duration histogram.
- `sorafs_orchestrator_fetch_failures_total{manifest_id,region,failure_reason}` — orchestrator failures.
- `sorafs_orchestrator_retries_total{manifest_id,provider_id,retry_reason}` — retry counts by provider.
- `sorafs_orchestrator_provider_failures_total{manifest_id,provider_id,failure_reason}` — provider failure matrix.

**OpenTelemetry / OTLP push**

- `sorafs.fetch.active{manifest_id,region,job_id}` — active fetches (up/down counter).
- `sorafs.fetch.duration_ms{manifest_id,region,job_id}` — fetch duration histogram (milliseconds).
- `sorafs.fetch.failures_total{manifest_id,region,failure_reason}` — orchestrator-level failures.
- `sorafs.fetch.retries_total{manifest_id,region,job_id,provider_id,retry_reason}` — retry attempts.
- `sorafs.fetch.provider_failures_total{manifest_id,region,job_id,provider_id,failure_reason}` — provider failure matrix.

## Events/Logs

- Provider ban/unban with reason.
- Token exhaustion events.
- Proof verification failure details.

## Dashboards

- **Overview**: throughput, success ratio, active fetches.
- **Provider Health**: failure counts, latency, stall rate.
- **Retries**: histogram and cumulative count by reason.
- **Per-Manifest**: chunk progress, outstanding tokens.

See `docs/examples/sorafs_fetch_dashboard.json` for the baseline Grafana dashboard wired to these
metrics (matching the panels described above).

## Alerts

- Success ratio < 99% for >5 minutes.
- Provider failure rate > 5% over 10-minute window.
- Chunk latency P95 > 250 ms sustained >10 minutes.
- Token exhaustion events > threshold per minute.
- Proof verification failures observed (with severity rating).

Alerting rules are codified in `docs/examples/sorafs_fetch_alerts.yaml`, suitable for ingestion by
Prometheus Alertmanager or Mimir ruler.

## Integration

- Ingest orchestrator metrics via OpenTelemetry -> Prometheus/Mimir.
- Link dashboards with gateway telemetry (shared labels `manifest`, `provider`).
- Align alerts with SLOs defined in `sorafs_observability_plan.md`.

## Implementation Status

- Prometheus gauges/counters are emitted from `crates/sorafs_orchestrator/src/lib.rs` via
  `FetchMetricsCtx`, ensuring scoreboard-driven fetches always update success/failure telemetry.
- OpenTelemetry metrics are provided by `iroha_telemetry::metrics::SorafsFetchOtel`; the helper
  `install_sorafs_fetch_otlp_exporter` configures an OTLP push pipeline (see
  `crates/iroha_telemetry/src/metrics.rs`).
- CLI / SDK integrations can call the exporter helper with the desired OTLP endpoint and region
  resource attributes before invoking `Orchestrator::fetch_*`, so both Prometheus scraping and OTLP
  streaming stay in sync.

## Rollout & Tuning Guide

- **Configuration.** Before issuing fetches, call
  `install_sorafs_fetch_otlp_exporter("http://127.0.0.1:4317", "sorafs-orchestrator", &[("deployment.region", region)], Duration::from_secs(5))`
  to align OTLP push cadence with the local collector. This keeps the OTLP stream and Prometheus text
  endpoint emitting the same measurements.
- **Sampling cadence.** The OTLP helper streams every 2 s; keep the collector batch interval at 5 s so
  Grafana panels based on `rate()`/`histogram_quantile()` reflect near-real-time behaviour without
  oversampling.
- **Failure scenarios.**
  - Rising `sorafs.fetch.failures_total` usually indicates capability mismatches or exhausted retry
    budgets—inspect the `failure_reason` labels and gate the orchestrator accordingly.
  - Spikes in `sorafs.fetch.provider_failures_total` pinpoint unhealthy providers; correlate with the
    “Retries per Provider” panel to decide on temporary blacklisting.
  - Sustained `sorafs.fetch.duration_ms` p95 above 250 ms triggers the bundled alert rule; tune
    `FetchOptions::global_parallel_limit` and retry budgets before paging.
- **Validation.** Start rollout with the Grafana panels in `docs/examples/sorafs_fetch_dashboard.json`,
  then enable the alert set from `docs/examples/sorafs_fetch_alerts.yaml` once baseline latency and
  failure rates stabilise.

## Label Taxonomy

Orchestrator metrics adopt the same label casing and semantic rules as the gateway and node plans so
cross-service joins remain deterministic.

| Label | Applies to | Description |
|-------|------------|-------------|
| `manifest_id` | gauges, histograms, counters | Stable Norito CID of the manifest being fetched. Absent for global metrics (e.g., total retries). |
| `provider_id` | provider-scoped metrics | Governance-issued provider identifier (`prov_xxx`). For multi-hop fetches this is the terminal provider serving the chunk. |
| `job_id` | orchestration jobs | UUID that groups a single fetch request and its retries. Included on latency/retry metrics to correlate traces. |
| `region` | all metrics | Orchestrator deployment region (`us-east-1`, `eu-central-1`). Enables regional dashboards and alert routing. |
| `failure_reason` | failure counters | Enumerated reason (`timeout`, `digest_mismatch`, `http_5xx`, `token_exhausted`). |
| `retry_reason` | retry metrics | Classification for retries (`retry`, `session_failure`, `length_mismatch`, etc.). |

Label hygiene rules:
- Prefer `manifest_id` + `provider_id` for joins. If either is missing, downstream dashboards treat the
  metric as aggregate.
- Keep cardinality bounded by mapping `job_id` via exemplar traces only (`job_id` exported as exemplar tag
  rather than a regular label on Prometheus histograms).
- When emitting OpenTelemetry spans, replicate these labels as trace attributes (`sora.manifest_id`,
  `sora.provider_id`) so the tracing and metrics backends share vocabulary.

## Metric Delivery Architecture

- **Export mechanism.** Call `iroha_telemetry::metrics::install_sorafs_fetch_otlp_exporter` to initialise
  the OTLP pipeline. Deployments typically point the helper at a local OpenTelemetry Collector sidecar;
  metrics stream every 2s instead of relying solely on scrapes.
- **Collector pipeline.**
  1. The orchestrator pushes OTLP metrics to the sidecar (Tokio runtime + OTLP gRPC exporter).
  2. The collector batches per 5s window, attaches resource attributes (`service.name=sorafs-orchestrator`,
     `deployment.region`), and forwards to the central Prometheus remote-write gateway.
  3. Remote-write gateway writes into Mimir/Prometheus. Streaming ensures chunk latency histograms carry
     exemplars for job-level debugging.
- **Fail-safe polling.** The Prometheus registry remains available via the existing `/metrics` handler.
  If OTLP push fails for >30s the collector can fall back to scraping, keeping dashboards populated
  during collector outages.
- **Trace correlation.** Traces flow alongside metrics via the same sidecar. Each span includes
  `manifest_id`, `provider_id`, and `job_id` so Grafana Tempo/Jaeger queries can start from a metric
  datapoint and pivot into traces.

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
