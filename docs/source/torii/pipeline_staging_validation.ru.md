---
lang: ru
direction: ltr
source: docs/source/torii/pipeline_staging_validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e271232f49afd9e7a5b55bac7fac4223341d0591b73b2243e6fc35b2dda716f
source_last_modified: "2026-01-18T05:31:56.993103+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Torii `/v2/pipeline` Staging Validation
summary: Checklist for Torii owners to prove the pipeline endpoints, telemetry, and SSE observers behave deterministically before IOS2-WB2 closes.
---

# 1. Scope & Owners

- **Roadmap items:** IOS2-WB2 (Swift `/v2/pipeline` adoption) and NRPC-2 (Torii rollout plan) both gate on Torii providing evidence that the staging cluster exercises the pipeline routes with the same behaviour exposed to SDKs.【roadmap.md:1268】【docs/source/torii/norito_rpc_rollout_plan.md:1】
- **Owners:** Torii Platform TL (primary), SDK Program Lead (witness), Swift/Android parity reps (consumers), Observability TL (telemetry capture).

# 2. Pre-flight Requirements

| Item | Details | Evidence |
|------|---------|----------|
| Staging cluster config | `torii.pipeline.enabled = true`, `/v2/pipeline` aliases exposed through the ingress, NRPC feature flag enabled so both transports can be exercised. Configuration is referenced from the same `iroha_config` bundle that Swift uses for parity tests.【iroha_config::parameters::actual::Torii#pipeline_enabled】 | Attach staged `config.toml` plus `GET /v2/configuration` output before running validation. |
| Test fixtures | Signed transaction envelopes encoded as Norito binaries (e.g., `fixtures/norito_rpc/transfer_asset.norito`) with known hashes. | Store the `.norito` payload + hash in `artifacts/torii/pipeline_validation/<timestamp>/`. |
| Authentication | API token or mTLS credentials that match staging access policy. | Include the token id / client cert fingerprint in the evidence log. |
| Observability | Access to Prometheus / OTLP exporters for Torii so `pipeline_*` counters in `docs/source/telemetry.md` can be sampled during the run (e.g., `pipeline_stage_ms`, `pipeline_layer_count`, `pipeline_total_ms`). | Export a Grafana snapshot or the raw Prometheus queries listed below. |

# 3. Validation Steps

## 3.1 Submit & Poll (`/v2/pipeline/transactions`)

1. Compute or reuse the transaction hash for the envelope under test (e.g., `transfer_asset.norito`). This is required because Torii responds with an empty body for successful submits. The transaction hash is canonical and matches the entrypoint hash for external transactions.
2. Submit the envelope over HTTPS with Norito content type:

```bash
curl -sS -X POST "$TORII_BASE/v2/pipeline/transactions" \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: Bearer $API_TOKEN" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  -o /dev/null -w "%{http_code}\n"
```

3. Poll the status endpoint until the transaction reaches a terminal state. The response mirrors the DTO consumed by Swift’s `ToriiClient.getTransactionStatus` helpers.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:5299】

```bash
curl -sS -G "$TORII_BASE/v2/pipeline/transactions/status" \
  -H "Authorization: Bearer $API_TOKEN" \
  --data-urlencode "hash=$TX_HASH" | jq .
```

✅ Acceptance: the submit request returns `202` / `204`, status transitions through `Queued → Approved/Committed/Applied`, and the timeline aligns with the retry budgets documented in the Swift adoption guide.【docs/source/sdk/swift/pipeline_adoption_guide.md:33】

## 3.2 Recovery Sidecar (`/v2/pipeline/recovery/{height}`)

1. Record the height that finalized while the test transaction was processed (from `/v2/pipeline/transactions/status`).
2. Fetch the recovery snapshot and verify it embeds the expected DAG fingerprint and footprint metadata:

```bash
curl -sS "$TORII_BASE/v2/pipeline/recovery/$HEIGHT" \
  -H "Authorization: Bearer $API_TOKEN" | jq .
```

3. The response must advertise `format = "pipeline.recovery.v1"` as enforced by `handler_pipeline_recovery`. Missing heights must return `404` exactly as covered by `crates/iroha_torii/tests/pipeline_recovery_endpoint.rs`. Capture both the populated and missing responses in the evidence bundle.

## 3.3 SSE Bridge (`/v2/events/sse`)


```bash
curl -sS -N "$TORII_BASE/v2/events/sse?topic=pipeline&status=queued,applied" \
  -H "Authorization: Bearer $API_TOKEN" \
  -H "Accept: text/event-stream" | tee artifacts/torii/pipeline_validation/sse.log
```

The stream must emit `event: pipeline` entries with a JSON body matching `PipelineEventBox` (`hash`, `status`, `lane_id`, `dataspace_id`). Compare at least one event against the transaction hash submitted earlier. This proves the SSE bridge defined in `docs/source/torii/norito_rpc_pipeline_sse_bridge.md` is active on staging.

## 3.4 Norito RPC Parity

Use the NRPC transport (`POST /transaction` with Norito payload wrapped in the RPC envelope) to submit the same transaction and ensure:

- The RPC response matches the HTTP status path (no additional error codes).
- `/v2/pipeline/transactions/status` reflects both submissions identically.

Document the RPC request/response in the evidence bundle and reference the schema anchored in `docs/source/torii/nrpc_spec.md`.

# 4. Telemetry & Evidence

Collect the following during the validation window (pull quotes from `docs/source/telemetry.md` for metric definitions):

| Metric / Log | Query / Source | Target |
|--------------|----------------|--------|
| Pipeline stage latency | `histogram_quantile(0.5, sum(rate(pipeline_stage_ms_bucket[5m])) by (le,stage))` | P50 latencies within the envelope declared in `status.md` for staging. |
| Scheduler shape | `avg_over_time(pipeline_layer_avg_width[5m])`, `avg_over_time(pipeline_layer_count[5m])` | No regressions relative to baseline, captured in Grafana snapshot. |
| Recovery write confirmation | `kura_pipeline_metadata_write_total{height="$HEIGHT"}` (if exported) or Torii logs tagged `pipeline_recovery_sidecar`. | Entry increments exactly once per finalized block. |
| SSE delivery | `increase(torii_events_sse_sent_total{topic="pipeline"}[5m])` plus the raw `sse.log` from §3.3. | Counter increments by ≥1 while log shows matching hashes. |
| Admission counters | `rate(torii_pipeline_admission_total{result="ok"}[5m])` and `{result="queue_full"}` (available through the Torii OTLP exporter). | `result="ok"` increments for every test submission, error buckets remain flat. |

Store artefacts under `artifacts/torii/pipeline_validation/<timestamp>/`:

- `pipeline_submit.json` — list of hashes, timestamps, HTTP codes.
- `pipeline_status_trace.json` — status transitions for each hash.
- `pipeline_recovery_existing.json` / `pipeline_recovery_missing.json`.
- `sse.log`.
- `telemetry.promql.txt` — PromQL queries + sampled values.
- Grafana snapshot (`pipeline_validation_dashboard.json`) if dashboards were used.

# 5. Reporting & Gating

1. Attach the artefact directory to the weekly status digest and link it from the IOS2 section in `status.md`.
2. Update the IOS2-WB2 row in `roadmap.md` with the validation date and link to this runbook entry so reviewers know staging coverage is complete.
3. File a short summary in `docs/source/sdk/swift/status/swift_weekly_digest.md` noting which hashes were exercised and whether any retries or SSE gaps were observed.
4. If validation fails, open an incident in the Torii tracker referencing the metric/log that breached expectations and rerun the checklist after remediation.

Completion of these steps constitutes “/v2/pipeline staging validation” for the IOS2 milestone; subsequent runs should repeat the same artefact layout so governance can diff evidence across releases.
