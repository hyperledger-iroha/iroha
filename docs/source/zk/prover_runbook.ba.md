---
lang: ba
direction: ltr
source: docs/source/zk/prover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7edf657f30c31d46a0a634c7028ec1478a78a75affa3bfe037b39ddfc79e72d8
source_last_modified: "2026-01-22T14:45:02.079306+00:00"
translation_last_reviewed: 2026-02-07
---

# Torii ZK Attachments & Prover Runbook

This runbook helps operators monitor, alert, and triage the Torii attachment
service and background ZK prover worker. It assumes telemetry is enabled with a
profile that exposes Prometheus metrics (`telemetry_profile = "extended"` or
`"full"`) and that operator dashboards ingest the `/metrics` endpoint.

## Components

- **Attachment API** – `POST /v1/zk/attachments` stores opaque payloads (proofs,
  transcripts). Stored entries are scanned by the background prover when
  enabled. Operators can list, download, and delete attachments through the API
  or `iroha_cli app zk attachments *`.
- **Background prover** – Controlled by
  `torii.zk_prover_enabled=true`. The worker drains the attachment queue,
  verifies `ProofAttachment` payloads, and produces JSON reports
  (`/v1/zk/prover/reports`). It enforces resource budgets:
  `torii.zk_prover_max_inflight`, `torii.zk_prover_max_scan_bytes`, and
  `torii.zk_prover_max_scan_millis`. Backend and circuit scope is governed by
  `torii.zk_prover_allowed_backends` and `torii.zk_prover_allowed_circuits`.
  When a registry entry omits inline VK bytes, the prover loads key bytes from
  `torii.zk_prover_keys_dir` using `<backend>__<name>.vk` naming.
- **Telemetry surface** – Metrics registered in
  `crates/iroha_telemetry::metrics` and exposed under `/metrics`.

## FASTPQ production prover

- Production binaries initialise the Stage 6 backend via `fastpq_prover::Prover::canonical`; the deterministic mock has been removed, so all builds ship the production prover by default.【crates/fastpq_prover/src/proof.rs:126】
- Execution mode overrides (`zk.fastpq.execution_mode`, `irohad --fastpq-execution-mode`) let operators pin CPU/GPU deterministically while the startup hook logs the resolved backend and increments `fastpq_execution_mode_total{requested,resolved,backend}` for fleet audits.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:2192】【crates/iroha_telemetry/src/metrics.rs:8887】
- Dashboards should ingest the counter above and alert on unexpected backend labels or mode downgrades to ensure cluster parity before enabling GPU acceleration globally.

## Quick checklist

| Scenario | Action |
|----------|--------|
| `torii_zk_prover_budget_exhausted_total` increases | Inspect budget reason (`bytes` or `time`). Raise limits in configuration or reduce attachment volume. |
| `torii_zk_prover_pending` stays above 0 for >10 minutes | Queue congestion: inspect scan metrics, consider scaling inflight cap or pruning attachments. |
| Attachments not scanned | Ensure worker enabled, check logs for `zk_prover` errors, verify budgets not at zero. |
| CLI/SDK uploads fail | Check Torii logs for HTTP errors and verify disk quotas. |
| Reports missing | Confirm TTL (`torii.zk_prover_reports_ttl_secs`) and GC messages in logs. |

## Metrics & PromQL snippets

| Metric | Description | PromQL / Alert Hint |
|--------|-------------|----------------------|
| `torii_zk_prover_inflight` gauge | Attachments currently being processed | `torii_zk_prover_inflight` – page if stuck at max for >10m. |
| `torii_zk_prover_pending` gauge | Queue length waiting for a worker permit | `avg_over_time(torii_zk_prover_pending[5m]) > 0` indicates backlog. |
| `torii_zk_prover_last_scan_bytes` / `torii_zk_prover_last_scan_ms` | Size/time of most recent scan cycle | Track with `increase` over time to see regressions. |
| `torii_zk_prover_budget_exhausted_total{reason}` counter | Budget hits (bytes/time) | `rate(torii_zk_prover_budget_exhausted_total[5m]) > 0` should trigger investigation. |
| `torii_zk_prover_attachment_bytes_bucket` histogram | Distribution of attachment sizes, labelled by `content_type` | `histogram_quantile(0.95, sum(rate(torii_zk_prover_attachment_bytes_bucket[5m])) by (le, content_type))` to size proof backlog. |
| `torii_zk_prover_latency_ms_bucket` histogram | Worker latency per attachment | Alert when p95 exceeds `torii.zk_prover_max_scan_millis`. |
| `zk_verify_proof_bytes_bucket` / `zk_verify_latency_ms_bucket` | End-to-end proof verification metrics (execution path) | Helps correlate ledger verification spikes with prover ingestion. |
| `torii_zk_prover_gc_total` counter | Attachments garbage-collected (TTL expirations) | Sudden spikes may indicate misconfigured TTL. |

## Sample Grafana dashboard

A ready-to-import dashboard is provided in
[`grafana_zk_prover.json`](../grafana_zk_prover.json). Panels include:

1. **Queue depth** – `torii_zk_prover_pending` and `torii_zk_prover_inflight`.
2. **Attachment throughput** – histogram quantiles for attachment sizes.
3. **Worker latency** – `torii_zk_prover_latency_ms` quantiles versus configured budgets.
4. **Budget exhaustion rate** – `increase(torii_zk_prover_budget_exhausted_total[15m])` by reason.
5. **Verification latency overlay** – compare `zk_verify_latency_ms` against
   prover latency to correlate ledger verification spikes.

Import the JSON via Grafana’s *Dashboards → Import*. Update the Prometheus data
source UID if necessary.

## Logs & observability

- Attachments API emits structured logs with the `torii::zk_attachments` target.
  Failed uploads return explicit HTTP status codes that surface in Torii logs.
- The background prover logs under `torii::zk_prover`. During each scan it
  reports: `scheduled`, `processed_bytes`, `elapsed_ms`, and budget status.
- Enable debug logs temporarily with `RUST_LOG=torii::zk_prover=debug` to inspect
  per-attachment flow. Do not leave debug logging on in production.
- The telemetry worker logs budget hits and queue resets via
  `torii::zk_prover::metrics`.

## Common incident playbooks

### 1. Budget exhaustion storms

1. Confirm metrics: `torii_zk_prover_budget_exhausted_total{reason}` rising.
2. Inspect logs for the reported reason (bytes vs time).
3. Check configuration:
   - `torii.zk_prover_max_scan_bytes`
   - `torii.zk_prover_max_scan_millis`
   - `torii.zk_prover_max_inflight`
4. If payloads are larger than expected, verify clients are not uploading
   compressed archives or multi-proof bundles. Consider raising the byte budget
   or splitting attachments upstream.
5. After raising limits, monitor `torii_zk_prover_pending` to ensure backlog
   drains. Document the new thresholds.

### 2. Growing attachment backlog

1. Examine `torii_zk_prover_pending` and `torii_zk_prover_inflight`.
2. Check `torii_zk_prover_last_scan_ms`: if close to the limit, the worker may be
   throttled by `torii.zk_prover_max_scan_millis`.
3. Confirm the worker is running: logs should show `scan_completed` lines every
   `torii.zk_prover_scan_period_secs`.
4. Consider increasing `torii.zk_prover_max_inflight` (parallelism) or reducing
   scan interval.
5. If attachments are stale, use `DELETE /v1/zk/attachments/:id` or the CLI
   equivalent to prune unused entries.

### 3. Reports missing or delayed

1. Verify retention: `torii.zk_prover_reports_ttl_secs`.
2. Inspect `torii_zk_prover_gc_total` to see if GC is deleting reports.
3. Check that clients poll `/v1/zk/prover/reports` before TTL expiry.
4. For urgent analysis, manually fetch raw attachments and reprocess offline.

## Integration with CLI & SDKs

- `iroha_cli app zk attachments` mirrors the Torii endpoints; ensure the CLI is built
  from the same commit as the node to keep DTOs in sync.
- Swift and Python SDK helpers under `ToriiClient` provide `upload_attachment`,
  `list_attachments`, `get_attachment`, and `delete_attachment` wrappers. Use the
  runbook metrics to validate SDK-driven flows.

## Change management

- Record configuration changes in the ops notebook (recommended format: timestamp,
  operator, old value, new value, reason).
- When changing budgets or scan cadence, update your Grafana dashboard and alert
  thresholds to reflect the new expectations.
- Run `cargo test -p iroha_torii zk_prover` before enabling new prover builds to
  catch regressions covered by unit tests.

## References

- [ZK App API](../zk_app_api.md)
- [ZK lifecycle (VK/proof flows)](lifecycle.md)
- [Torii CLI smoke tests](../../../crates/iroha_cli/tests/cli_smoke.rs)
- [Telemetry overview](../telemetry.md)
- [Roadmap – Milestone 4](../../../roadmap.md)
