---
lang: zh-hans
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9eafd2f3786dbe0ccfa7db2ec138d1d91e306e1691fbee801ba04a4165131655
source_last_modified: "2026-01-05T09:28:11.915061+00:00"
translation_last_reviewed: 2026-02-07
id: privacy-metrics-pipeline
title: SoraNet Privacy Metrics Pipeline (SNNet-8)
sidebar_label: Privacy Metrics Pipeline
description: Privacy-preserving telemetry collection for SoraNet relays and orchestrators.
---

:::note Canonical Source
:::

# SoraNet Privacy Metrics Pipeline

SNNet-8 introduces a privacy-aware telemetry surface for the relay runtime. The
relay now aggregates handshake and circuit events into minute-sized buckets and
exports only coarse Prometheus counters, keeping individual circuits
unlinkable while giving operators actionable visibility.

## Aggregator Overview

- The runtime implementation lives in `tools/soranet-relay/src/privacy.rs` as
  `PrivacyAggregator`.
- Buckets are keyed by wall-clock minute (`bucket_secs`, default 60 seconds) and
  stored in a bounded ring (`max_completed_buckets`, default 120). Collector
  shares keep their own bounded backlog (`max_share_lag_buckets`, default 12)
  so stale Prio windows are flushed as suppressed buckets rather than leaking
  memory or masking stuck collectors.
- `RelayConfig::privacy` maps straight into `PrivacyConfig`, exposing tuning
  knobs (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`,
  `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`,
  `expected_shares`). The production runtime keeps the defaults while SNNet-8a
  introduces secure aggregation thresholds.
- Runtime modules record events through typed helpers:
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes`, and `record_gar_category`.

## Relay Admin Endpoint

Operators can poll the relay's admin listener for raw observations via
`GET /privacy/events`. The endpoint returns newline-delimited JSON
(`application/x-ndjson`) containing `SoranetPrivacyEventV1` payloads mirrored
from the internal `PrivacyEventBuffer`. The buffer retains the newest events up
to `privacy.event_buffer_capacity` entries (default 4 096) and is drained on
read, so scrapers should poll frequently enough to avoid gaps. Events cover the
same handshake, throttle, verified bandwidth, active circuit, and GAR signals
that power the Prometheus counters, allowing downstream collectors to archive
privacy-safe breadcrumbs or feed secure aggregation workflows.

## Relay Configuration

Operators adjust privacy telemetry cadences in the relay configuration file via
the `privacy` section:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Field defaults match the SNNet-8 spec and are validated at load time:

| Field | Description | Default |
|-------|-------------|---------|
| `bucket_secs` | Width of each aggregation window (seconds). | `60` |
| `min_handshakes` | Minimum contributor count before a bucket can emit counters. | `12` |
| `flush_delay_buckets` | Completed buckets to wait before attempting a flush. | `1` |
| `force_flush_buckets` | Maximum age before we emit a suppressed bucket. | `6` |
| `max_completed_buckets` | Retained bucket backlog (prevents unbounded memory). | `120` |
| `max_share_lag_buckets` | Retention window for collector shares before suppression. | `12` |
| `expected_shares` | Prio collector shares required before combining. | `2` |
| `event_buffer_capacity` | NDJSON event backlog for the admin stream. | `4096` |

Setting `force_flush_buckets` lower than `flush_delay_buckets`, zeroing the
thresholds, or disabling the retention guard now fails validation to avoid
deployments that would leak per-relay telemetry.

The `event_buffer_capacity` limit also bounds `/admin/privacy/events`, ensuring
scrapers cannot fall behind indefinitely.

## Prio collector shares

SNNet-8a deploys dual collectors that emit secret-shared Prio buckets. The
orchestrator now parses the `/privacy/events` NDJSON stream for both
`SoranetPrivacyEventV1` entries and `SoranetPrivacyPrioShareV1` shares,
forwarding them into `SoranetSecureAggregator::ingest_prio_share`. Buckets emit
once `PrivacyBucketConfig::expected_shares` contributions arrive, mirroring the
relay behaviour. Shares are validated for bucket alignment and histogram shape
before being combined into `SoranetPrivacyBucketMetricsV1`. If the combined
handshake count falls below `min_contributors`, the bucket is exported as
`suppressed`, mirroring the behaviour of the in-relay aggregator. Suppressed
windows now emit a `suppression_reason` label so operators can distinguish
between `insufficient_contributors`, `collector_suppressed`,
`collector_window_elapsed`, and `forced_flush_window_elapsed` scenarios when
diagnosing telemetry gaps. The `collector_window_elapsed` reason also fires
when Prio shares linger past `max_share_lag_buckets`, making stuck collectors
visible without leaving stale accumulators in memory.

## Torii Ingestion Endpoints

Torii now exposes two telemetry-gated HTTP endpoints so relays and collectors
can forward observations without embedding a bespoke transport:

- `POST /v1/soranet/privacy/event` accepts a
  `RecordSoranetPrivacyEventDto` payload. The body wraps a
  `SoranetPrivacyEventV1` plus an optional `source` label. Torii validates the
  request against the active telemetry profile, records the event, and responds
  with HTTP `202 Accepted` alongside a Norito JSON envelope containing the
  computed bucket window (`bucket_start_unix`, `bucket_duration_secs`) and the
  relay mode.
- `POST /v1/soranet/privacy/share` accepts a `RecordSoranetPrivacyShareDto`
  payload. The body carries a `SoranetPrivacyPrioShareV1` and an optional
  `forwarded_by` hint so operators can audit collector flows. Successful
  submissions return HTTP `202 Accepted` with a Norito JSON envelope summarising
  the collector, bucket window, and suppression hint; validation failures map to
  a telemetry `Conversion` response to preserve deterministic error handling
  across collectors. The orchestrator’s event loop now emits these shares as it
  polls relays, keeping Torii’s Prio accumulator in sync with on-relay buckets.

Both endpoints honour the telemetry profile: they emit `503 Service
Unavailable` when metrics are disabled. Clients may send either Norito binary
(`application/x.norito`) or Norito JSON (`application/x.norito+json`) bodies;
the server automatically negotiates the format via the standard Torii
extractors.

## Prometheus Metrics

Each exported bucket carries `mode` (`entry`, `middle`, `exit`) and
`bucket_start` labels. The following metric families are emitted:

| Metric | Description |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Handshake taxonomy with `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Throttle counters with `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Aggregated cooldown durations contributed by throttled handshakes. |
| `soranet_privacy_verified_bytes_total` | Verified bandwidth from blinded measurement proofs. |
| `soranet_privacy_active_circuits_{avg,max}` | Mean and peak active circuits per bucket. |
| `soranet_privacy_rtt_millis{percentile}` | RTT percentile estimates (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Hashed Governance Action Report counters keyed by category digest. |
| `soranet_privacy_bucket_suppressed` | Buckets withheld because the contributor threshold was not met. |
| `soranet_privacy_pending_collectors{mode}` | Collector share accumulators pending combination, grouped by relay mode. |
| `soranet_privacy_suppression_total{reason}` | Suppressed bucket counters with `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` so dashboards can attribute privacy gaps. |
| `soranet_privacy_snapshot_suppression_ratio` | Last drain’s suppressed/drained ratio (0–1), useful for alert budgets. |
| `soranet_privacy_last_poll_unixtime` | UNIX timestamp of the most recent successful poll (drives the collector-idle alert). |
| `soranet_privacy_collector_enabled` | Gauge that flips to `0` when the privacy collector is disabled or fails to start (drives the collector-disabled alert). |
| `soranet_privacy_poll_errors_total{provider}` | Polling failures grouped by relay alias (increments on decode errors, HTTP failures, or unexpected status codes). |

Buckets without observations stay silent, keeping dashboards tidy without
fabricating zero-filled windows.

## Operational Guidance

1. **Dashboards** – chart the metrics above grouped by `mode` and `window_start`.
   Highlight missing windows to surface collector or relay issues. Use
   `soranet_privacy_suppression_total{reason}` to distinguish contributor
   shortfalls from collector-driven suppression when triaging gaps. The Grafana
   asset now ships a dedicated **“Suppression Reasons (5m)”** panel fed by those
   counters plus a **“Suppressed Bucket %”** stat that computes
   `sum(soranet_privacy_bucket_suppressed) / count(...)` per selection so
   operators can spot budget breaches at a glance. The **Collector Share
   Backlog** series (`soranet_privacy_pending_collectors`) and the **Snapshot
   Suppression Ratio** stat highlight stuck collectors and budget drift during
   automated runs.
2. **Alerting** – drive alarms from privacy-safe counters: PoW reject spikes,
   cooldown frequency, RTT drift, and capacity rejects. Because counters are
   monotonic within each bucket, straightforward rate-based rules work well.
3. **Incident response** – rely on aggregated data first. When deeper debugging
   is necessary, request relays to replay bucket snapshots or inspect blinded
   measurement proofs instead of harvesting raw traffic logs.
4. **Retention** – scrape often enough to avoid exceeding
   `max_completed_buckets`. Exporters should treat the Prometheus output as the
   canonical source and drop local buckets once forwarded.

## Suppression Analytics & Automated Runs

SNNet-8 acceptance hinges on demonstrating that automated collectors stay
healthy and that suppression stays within policy bounds (≤10% of buckets per
relay over any 30 minute window). The tooling needed to satisfy that gate now
ships with the tree; operators must wire it into their weekly rituals. The new
Grafana suppression panels mirror the PromQL snippets below, giving on-call
teams live visibility before they need to fall back to manual queries.

### PromQL recipes for suppression review

Operators should keep the following PromQL helpers handy; both are referenced
in the shared Grafana dashboard (`dashboards/grafana/soranet_privacy_metrics.json`)
and Alertmanager rules:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

Use the ratio output to confirm the **“Suppressed Bucket %”** stat remains below
the policy budget; wire the spike detector into Alertmanager for fast feedback
when contributor counts dip unexpectedly.

### Offline bucket report CLI

The workspace exposes `cargo xtask soranet-privacy-report` for one-off NDJSON
captures. Point it at one or more relay admin exports:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

The helper streams the capture through `SoranetSecureAggregator`, prints a
suppression summary to stdout, and optionally writes a structured JSON report
via `--json-out <path|->`. It honours the same knobs as the live collector
(`--bucket-secs`, `--min-contributors`, `--expected-shares`, etc.), letting
operators replay historical captures under different thresholds when triaging
an issue. Attach the JSON alongside Grafana screenshots so the SNNet-8
suppression analytics gate remains auditable.

### First automated run checklist

Governance still requires proving that the first automation run met the
suppression budget. The helper now accepts `--max-suppression-ratio <0-1>` so
CI or operators can fail fast whenever suppressed buckets exceed the allowed
window (default 10%) or when no buckets are present yet. Recommended flow:

1. Export NDJSON from the relay admin endpoint(s) plus the orchestrator’s
   `/v1/soranet/privacy/event|share` stream into
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Run the helper with the policy budget:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   The command prints the observed ratio and exits non-zero when the budget is
   exceeded **or** when no buckets are ready, signalling that telemetry has not
   yet been produced for the run. Live metrics should show
   `soranet_privacy_pending_collectors` draining toward zero and
   `soranet_privacy_snapshot_suppression_ratio` staying under the same budget
   while the run executes.
3. Archive the JSON output and CLI log with the SNNet-8 evidence bundle before
   flipping the transport default so reviewers can replay the exact artefacts.

## Next Steps (SNNet-8a)

- Integrate the dual Prio collectors, wiring their share ingestion into the
  runtime so relays and collectors emit consistent `SoranetPrivacyBucketMetricsV1`
  payloads. *(Done — see `ingest_privacy_payload` in
  `crates/sorafs_orchestrator/src/lib.rs` and accompanying tests.)*
- Publish the shared Prometheus dashboard JSON and alert rules covering
  suppression gaps, collector health, and anonymity brownouts. *(Done — see
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml`, and validation fixtures.)*
- Produce the differential-privacy calibration artefacts described in
  `privacy_metrics_dp.md`, including reproducible notebooks and governance
  digests. *(Done — notebook + artefacts generated by
  `scripts/telemetry/run_privacy_dp.py`; CI wrapper
  `scripts/telemetry/run_privacy_dp_notebook.sh` executes the notebook via the
  `.github/workflows/release-pipeline.yml` workflow; governance digest filed in
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

The current release delivers the SNNet-8 foundation: deterministic,
privacy-safe telemetry that slots directly into existing Prometheus scrapers
and dashboards. Differential privacy calibration artefacts are in place, the
release pipeline workflow keeps the notebook outputs fresh, and the remaining
work focuses on monitoring the first automated run plus extending suppression
alert analytics.
