---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: observability-plan
title: SoraFS Observability اور SLO پلان
sidebar_label: Observability اور SLOs
description: SoraFS gateways، nodes اور multi-source orchestrator کے لیے telemetry schema، dashboards اور error-budget policy۔
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs_observability_plan.md` میں برقرار رکھے گئے منصوبے کی عکاسی کرتا ہے۔ جب تک پرانا Sphinx سیٹ مکمل طور پر منتقل نہ ہو جائے دونوں نقول کو ہم آہنگ رکھیں۔
:::

## Objectives
- gateways، nodes اور multi-source orchestrator کے لیے metrics اور structured events کی تعریف کریں۔
- Grafana dashboards، alert thresholds اور validation hooks فراہم کریں۔
- error-budget اور chaos-drill policies کے ساتھ SLO targets قائم کریں۔

## Metric Catalogue

### Gateway surfaces

| Metric | Type | Labels | Notes |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Gauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel` کے ذریعے emit ہوتا ہے؛ ہر endpoint/method کمبینیشن کے لیے in-flight HTTP operations ٹریک کرتا ہے۔ |
| `sorafs_gateway_responses_total` | Counter | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | ہر مکمل gateway request ایک بار increment ہوتی ہے؛ `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogram | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | gateway responses کے لیے time-to-first-byte latency؛ Prometheus `_bucket/_sum/_count` کے طور پر export۔ |
| `sorafs_gateway_proof_verifications_total` | Counter | `profile_version`, `result`, `error_code` | request time پر proof verification outcomes capture کیے جاتے ہیں (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogram | `profile_version`, `result`, `error_code` | PoR receipts کے لیے verification latency distribution۔ |
| `telemetry::sorafs.gateway.request` | Structured event | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | ہر request completion پر structured log emit ہوتا ہے تاکہ Loki/Tempo correlation ہو سکے۔ |

`telemetry::sorafs.gateway.request` events OTEL counters کو structured payloads کے ساتھ mirror کرتے ہیں، Loki/Tempo correlation کے لیے `endpoint`, `method`, `variant`, `status`, `error_code` اور `duration_ms` دکھاتے ہیں جبکہ dashboards SLO tracking کے لیے OTLP series استعمال کرتے ہیں۔

### Proof-health telemetry

| Metric | Type | Labels | Notes |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Counter | `provider_id`, `trigger`, `penalty` | جب بھی `RecordCapacityTelemetry` ایک `SorafsProofHealthAlert` emit کرے تو increment ہوتا ہے۔ `trigger` PDP/PoTR/Both failures فرق کرتا ہے، جبکہ `penalty` دکھاتا ہے کہ collateral واقعی slash ہوا یا cooldown نے suppress کیا۔ |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Gauge | `provider_id` | offending telemetry window میں رپورٹ ہونے والی تازہ PDP/PoTR counts تاکہ ٹیمیں جان سکیں کہ providers نے policy سے کتنا تجاوز کیا۔ |
| `torii_sorafs_proof_health_penalty_nano` | Gauge | `provider_id` | آخری alert پر slash ہونے والی Nano-XOR مقدار (cooldown نے enforcement suppress کیا تو صفر). |
| `torii_sorafs_proof_health_cooldown` | Gauge | `provider_id` | Boolean gauge (`1` = alert cooldown نے suppress کیا) تاکہ follow-up alerts وقتی طور پر mute ہونے پر دکھایا جا سکے۔ |
| `torii_sorafs_proof_health_window_end_epoch` | Gauge | `provider_id` | alert سے منسلک telemetry window کا epoch تاکہ operators Norito artefacts سے correlation کر سکیں۔ |

یہ feeds اب Taikai viewer dashboard کی proof-health row کو چلاتے ہیں
(`dashboards/grafana/taikai_viewer.json`)، جس سے CDN operators کو alert volumes، PDP/PoTR trigger mix، penalties اور cooldown state فی provider کی live visibility ملتی ہے۔

یہی metrics اب Taikai viewer کے دو alert rules کو سپورٹ کرتے ہیں:
`SorafsProofHealthPenalty` اس وقت fire ہوتا ہے جب
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` میں
گزشتہ 15 منٹ میں اضافہ ہو، جبکہ `SorafsProofHealthCooldown` warning دیتا ہے اگر
provider پانچ منٹ تک cooldown میں رہے۔ دونوں alerts
`dashboards/alerts/taikai_viewer_rules.yml` میں موجود ہیں تاکہ SREs کو PoR/PoTR
enforcement بڑھنے پر فوری context ملے۔

### Orchestrator surfaces

| Metric / Event | Type | Labels | Producer | Notes |
|----------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Gauge | `manifest_id`, `region` | `FetchMetricsCtx` | موجودہ in-flight sessions۔ |
| `sorafs_orchestrator_fetch_duration_ms` | Histogram | `manifest_id`, `region` | `FetchMetricsCtx` | duration histogram (milliseconds)؛ 1 ms سے 30 s buckets۔ |
| `sorafs_orchestrator_fetch_failures_total` | Counter | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Reasons: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | retry causes فرق کرتا ہے (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | session-level disablement / failure tallies capture کرتا ہے۔ |
| `sorafs_orchestrator_chunk_latency_ms` | Histogram | `manifest_id`, `provider_id` | `FetchMetricsCtx` | per-chunk fetch latency distribution (ms) throughput/SLO analysis کے لیے۔ |
| `sorafs_orchestrator_bytes_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | manifest/provider کے حساب سے delivered bytes؛ PromQL میں `rate()` کے ذریعے throughput نکالیں۔ |
| `sorafs_orchestrator_stalls_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` سے تجاوز کرنے والے chunks گنتا ہے۔ |
| `telemetry::sorafs.fetch.lifecycle` | Structured event | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | job lifecycle (start/complete) کو Norito JSON payload کے ساتھ mirror کرتا ہے۔ |
| `telemetry::sorafs.fetch.retry` | Structured event | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | provider retry streak کے لیے emit ہوتا ہے؛ `attempts` incremental retries شمار کرتا ہے (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Structured event | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | جب provider failure threshold cross کرے تو ظاہر ہوتا ہے۔ |
| `telemetry::sorafs.fetch.error` | Structured event | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | terminal failure record، Loki/Splunk ingestion کے لیے مناسب۔ |
| `telemetry::sorafs.fetch.stall` | Structured event | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | chunk latency configured cap سے بڑھنے پر emit ہوتا ہے (stall counters کو mirror کرتا ہے). |

### Node / replication surfaces

| Metric | Type | Labels | Notes |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histogram | `provider_id` | storage utilisation percentage کا OTEL histogram ( `_bucket/_sum/_count` کے طور پر export ). |
| `sorafs_node_por_success_total` | Counter | `provider_id` | scheduler snapshots سے derived successful PoR samples کا monotonic counter۔ |
| `sorafs_node_por_failure_total` | Counter | `provider_id` | failed PoR samples کا monotonic counter۔ |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Gauge | `provider` | bytes used، queue depth اور PoR inflight counts کے لیے موجودہ Prometheus gauges۔ |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Gauge | `provider` | provider capacity/uptime success data جو capacity dashboard میں دکھایا جاتا ہے۔ |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Gauge | `provider`, `manifest` | backlog depth اور cumulative failure counters جو ہر `/v1/sorafs/por/ingestion/{manifest}` poll پر export ہوتے ہیں، "PoR Stalls" panel/alert کو feed کرتے ہیں۔ |

### Proof of Timely Retrieval (PoTR) اور chunk SLA

| Metric | Type | Labels | Producer | Notes |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Histogram | `tier`, `provider` | PoTR coordinator | deadline slack milliseconds میں (positive = met). |
| `sorafs_potr_failures_total` | Counter | `tier`, `provider`, `reason` | PoTR coordinator | Reasons: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Counter | `provider`, `manifest_id`, `reason` | SLA monitor | جب chunk delivery SLO miss کرے تو fire ہوتا ہے (latency، success rate). |
| `sorafs_chunk_sla_violation_active` | Gauge | `provider`, `manifest_id` | SLA monitor | Boolean gauge (0/1) جو active breach window میں toggle ہوتا ہے۔ |

## SLO Targets

- Gateway trustless availability: **99.9%** (HTTP 2xx/304 responses).
- Trustless TTFB P95: hot tier ≤ 120 ms, warm tier ≤ 300 ms.
- Proof success rate: ≥ 99.5% per day.
- Orchestrator success (chunk completion): ≥ 99%.

## Dashboards & Alerting

1. **Gateway Observability** (`dashboards/grafana/sorafs_gateway_observability.json`) — trustless availability، TTFB P95، refusal breakdown اور PoR/PoTR failures کو OTEL metrics کے ذریعے ٹریک کرتا ہے۔
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`) — multi-source load، retries، provider failures اور stall bursts کو کور کرتا ہے۔
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`) — anonymised relay buckets، suppression windows اور collector health کو `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` اور `soranet_privacy_poll_errors_total{provider}` کے ذریعے چارٹ کرتا ہے۔

Alert bundles:

- `dashboards/alerts/sorafs_gateway_rules.yml` — gateway availability، TTFB، proof failure spikes۔
- `dashboards/alerts/sorafs_fetch_rules.yml` — orchestrator failures/retries/stalls؛ `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml`, اور `dashboards/alerts/tests/soranet_policy_rules.test.yml` کے ذریعے validate۔
- `dashboards/alerts/soranet_privacy_rules.yml` — privacy downgrade spikes، suppression alarms، collector-idle detection اور disabled-collector alerts (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — anonymity brownout alarms جو `sorafs_orchestrator_brownouts_total` سے wired ہیں۔
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai viewer drift/ingest/CEK lag alarms کے ساتھ ساتھ نئی SoraFS proof-health penalty/cooldown alerts جو `torii_sorafs_proof_health_*` سے powered ہیں۔

## Tracing Strategy

- OpenTelemetry کو end-to-end اپنائیں:
  - Gateways OTLP spans (HTTP) emit کرتے ہیں جن پر request IDs، manifest digests اور token hashes ہوتے ہیں۔
  - orchestrator `tracing` + `opentelemetry` استعمال کر کے fetch attempts کے spans export کرتا ہے۔
  - Embedded SoraFS nodes PoR challenges اور storage operations کے spans export کرتے ہیں۔ تمام components `x-sorafs-trace` کے ذریعے propagate ہونے والا common trace ID share کرتے ہیں۔
- `SorafsFetchOtel` orchestrator metrics کو OTLP histograms میں bridge کرتا ہے جبکہ `telemetry::sorafs.fetch.*` events log-centric backends کے لیے lightweight JSON payloads فراہم کرتے ہیں۔
- Collectors: OTEL collectors کو Prometheus/Loki/Tempo کے ساتھ چلائیں (Tempo preferred). Jaeger API exporters اختیاری رہتے ہیں۔
- High-cardinality operations کو sample کریں (success paths کے لیے 10%، failures کے لیے 100%).

## TLS Telemetry Coordination (SF-5b)

- Metric alignment:
  - TLS automation `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}`, اور `sorafs_gateway_tls_ech_enabled` بھیجتی ہے۔
  - ان gauges کو Gateway Overview dashboard میں TLS/Certificates panel کے تحت شامل کریں۔
- Alert linkage:
  - جب TLS expiry alerts fire ہوں (≤ 14 days remaining) تو trustless availability SLO کے ساتھ correlate کریں۔
  - ECH disablement ایک secondary alert emit کرتا ہے جو TLS اور availability دونوں panels کو reference کرتا ہے۔
- Pipeline: TLS automation job اسی Prometheus stack پر export کرتا ہے جس پر gateway metrics ہیں؛ SF-5b کے ساتھ coordination deduplicated instrumentation یقینی بناتی ہے۔

## Metric Naming & Label Conventions

- Metric names موجودہ `torii_sorafs_*` یا `sorafs_*` prefixes کو follow کرتے ہیں جو Torii اور gateway استعمال کرتے ہیں۔
- Label sets standardised ہیں:
  - `result` → HTTP outcome (`success`, `refused`, `failed`).
  - `reason` → refusal/error code (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → hex-encoded provider identifier۔
  - `manifest` → canonical manifest digest (high-cardinality میں trim کیا جاتا ہے). 
  - `tier` → declarative tier labels (`hot`, `warm`, `archive`).
- Telemetry emission points:
  - Gateway metrics `torii_sorafs_*` کے تحت رہتے ہیں اور `crates/iroha_core/src/telemetry.rs` کی conventions reuse کرتے ہیں۔
  - orchestrator `sorafs_orchestrator_*` metrics اور `telemetry::sorafs.fetch.*` events (lifecycle, retry, provider failure, error, stall) emit کرتا ہے جن پر manifest digest، job ID، region اور provider identifiers tags ہوتے ہیں۔
  - Nodes `torii_sorafs_storage_*`, `torii_sorafs_capacity_*`, اور `torii_sorafs_por_*` دکھاتے ہیں۔
- Observability کے ساتھ coordinate کریں تاکہ metric catalogue کو shared Prometheus naming doc میں register کیا جائے، جس میں label cardinality expectations (provider/manifests upper bounds) شامل ہوں۔

## Data Pipeline

- Collectors ہر component کے ساتھ deploy ہوتے ہیں، OTLP کو Prometheus (metrics) اور Loki/Tempo (logs/traces) پر export کرتے ہیں۔
- Optional eBPF (Tetragon) gateways/nodes کے لیے low-level tracing کو enrich کرتا ہے۔
- `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` کو Torii اور embedded nodes کے لیے استعمال کریں؛ orchestrator `install_sorafs_fetch_otlp_exporter` کو کال کرتا رہتا ہے۔

## Validation Hooks

- CI کے دوران `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں تاکہ Prometheus alert rules stall metrics اور privacy suppression checks کے ساتھ lockstep رہیں۔
- Grafana dashboards کو version control (`dashboards/grafana/`) کے تحت رکھیں اور panels میں تبدیلی پر screenshots/links اپڈیٹ کریں۔
- Chaos drills کے نتائج `scripts/telemetry/log_sorafs_drill.sh` کے ذریعے log ہوتے ہیں؛ validation `scripts/telemetry/validate_drill_log.sh` استعمال کرتی ہے (دیکھیے [Operations Playbook](operations-playbook.md)).
