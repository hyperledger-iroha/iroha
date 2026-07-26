---
lang: ja
direction: ltr
source: docs/source/torii/norito_rpc_telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 64e54c9d99f47a3fd83f29d88f2fcdf0ee287666a8a8d7a9578a3d8178a7986a
source_last_modified: "2026-01-04T10:50:53.698462+00:00"
translation_last_reviewed: 2026-01-30
---

## Norito-RPC Telemetry & Alerting (NRPC-2B)

Status: Drafted 2026-03-19  
Owners: Observability liaison, Torii Platform TL

### 1. Telemetry Requirements

#### Scheme Latency & Failures
- **Metric**: `torii_request_duration_seconds_bucket{scheme,instance,le}`  
  - Captures request latency per connection scheme (`http`, `ws`, `norito_rpc`).  
  - Buckets mirror the HTTP histogram (5 ms → 10 s) so Grafana panels can pivot between transports without recomputing tolerances.  
  - Implemented by `Telemetry::observe_torii_request_by_scheme`【crates/iroha_core/src/telemetry.rs:3827】.
- **Metric**: `torii_request_failures_total{scheme,code}`  
  - Increments for 4xx/5xx responses using the originating scheme label and status code.  
  - Powers error-rate SLIs for Norito RPC without mixing JSON traffic.  
  - Emitted from the same telemetry helper so the Norito middleware does not need custom counters.

#### Content-Type Counters & Payload Gauges
- **Metric**: `torii_http_requests_total{content_type,status,method,instance}`  
  - Continues to differentiate Norito (`application/x-norito`) and JSON responses for end-to-end parity tracking.  
- **Metric**: `torii_http_request_duration_seconds_bucket{content_type,method,instance,le}`  
  - Retained for historical dashboards that slice latency by negotiated content type.  
- **Metric**: `torii_http_response_bytes_total{content_type,method,status}`  
  - Tracks payload size regressions; Norito payloads should remain within ±20 % of the JSON equivalent.

#### Decode Failures
- **Metric**: `torii_norito_decode_failures_total{payload_kind,reason}`  
  - Incremented whenever a Norito RPC body fails validation (invalid magic, checksum mismatch, unsupported feature, etc.).  
- Recorded inside the shared extractor (`decode_as_norito` in `crates/iroha_torii/src/utils.rs:527`), ensuring every endpoint reuses the same counters.

#### Connection/Pre-auth Gauges
- Reuse existing gauges/counters (`torii_active_connections_total{scheme}`, `torii_pre_auth_reject_total{reason}`) with new label values:  
  - `scheme = "norito_rpc"` for active connections.  

Instrumentation updates must land behind feature toggles-less behaviour; Prometheus registries should register metrics during startup even if Norito traffic is absent to avoid missing series in dashboards.

#### Mock Harness Metrics (SDK Council 2026-06-18)
SDK CI jobs that reuse the shared mock harness must also export client-side metrics
so Observability can correlate failures back to the harness scenarios:

- **Metric**: `torii_mock_harness_retry_total{sdk,scenario}`  
  Counter incremented whenever the harness triggers a retry (e.g., injected HTTP
  500). Helps confirm retry logic stays deterministic across SDKs.

- **Metric**: `torii_mock_harness_duration_ms{sdk,scenario}`  
  Histogram/summary (bins: 50 ms → 10 s) describing per-scenario runtime. Used to
  spot regressions in client encode/decode performance even before traffic reaches Torii.

- **Metric**: `torii_mock_harness_fixture_version{sdk}`  
  Gauge recording the Norito fixture bundle hash (same value emitted to logs). When
  fixture versions drift between SDKs, dashboards flag the mismatch.

Dashboards should join these metrics with the server-side ones to prove parity.
The expectations are documented in the workshop notes and `tools/torii_mock_harness/README.md`.

### 2. Dashboard Additions

- **Dashboard file**: `dashboards/grafana/torii_norito_rpc_observability.json`.  
  Panels include:
  1. Error-rate chart for `sum(rate(torii_request_failures_total{scheme="norito_rpc",code=~"5.."}[5m]))` with SLO-aligned thresholds.  
  2. Latency panel deriving P95 via `torii_request_duration_seconds_bucket`.  
  3. Request volume summary (stacked area by method) plus the active-connection stat sourced from `torii_active_connections_total{scheme="norito_rpc"}`.  
  4. Payload-size tracker comparing `torii_http_response_bytes_total` against the request rate to flag DTO drift.
  5. Decode-failure counter sourced from `torii_norito_decode_failures_total`, grouped by `reason` to highlight missing layout flags, checksum mismatches, and similar parsing issues.

### 3. Alerting

Alert definitions live in `dashboards/alerts/torii_norito_rpc_rules.yml` and are exercised via `dashboards/alerts/tests/torii_norito_rpc_rules.test.yml`.

| Alert | Purpose | Trigger |
|-------|---------|---------|
| `ToriiNoritoRpcErrorSpike` | Detect sustained 5xx bursts for Norito requests | `increase(torii_request_failures_total{scheme="norito_rpc",code=~"5.."}[5m]) > 5` for 5 minutes |
| `ToriiNoritoRpcLatencyDegraded` | Guard 95th percentile latency budget | `histogram_quantile(0.95, … torii_request_duration_seconds_bucket …) > 0.75` seconds for 5 minutes |
| `ToriiNoritoRpcSilentTraffic` | Warn when Norito traffic disappears unexpectedly | `increase(torii_request_duration_seconds_count{scheme="norito_rpc"}[30m]) == 0` for 30 minutes |

See the alert file for exact expressions and annotations.

### 4. Chaos & Validation Scripts

- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` executes `promtool test rules` against the new alert tests.  
- Operators should extend existing chaos drills to include Norito-specific scenarios (e.g., inject synthetic 5xx responses) and validate dashboards/alerts as part of rollout rehearsals.

### 5. Runbook Expectations

- The operator runbook now lives at [`docs/source/telemetry.md#norito-rpc-degraded-runbook`](../telemetry.md#norito-rpc-degraded-runbook) and covers detection signals, log review, proxy/MTU validation, SDK mock-harness correlation, and the brownout procedure (`torii.transport.norito_rpc.stage`). Keep that section updated whenever instrumentation or rollout knobs change.
- Cross-link this document from the runbook (done) so operators can jump between the how/why details and the metrics reference.

### 6. Ownership & Follow-up

- Observability: implement dashboards + alerts, ensure CI covers rule testing.  
- Platform TL: verify instrumentation exports the required labels.  
- SDK teams: monitor metrics to confirm client rollouts behave as expected.
