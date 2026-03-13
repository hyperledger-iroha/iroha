---
lang: zh-hans
direction: ltr
source: docs/source/torii/norito_rpc_pipeline_sse_bridge.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f31f35208444404483f5983cf7f48080128bbba580dc0f14b7a2b0e45ea5c8ee
source_last_modified: "2026-01-15T04:33:02.402973+00:00"
translation_last_reviewed: 2026-02-07
---

# Torii Norito RPC ↔ `/v2/pipeline` Parity & SSE Bridge (AND4)

Status: Drafted for AND4 readiness – aligns with the “Design Torii Norito RPC parity & SSE bridge” roadmap item.【roadmap.md:1003】

## 1. Goals & Scope


Key outcomes:

1. Canonical mapping between `/v2/pipeline/*` routes and their Norito RPC equivalents for submissions, status polling, and recovery helpers.
2. Documented retry/queue semantics so clients react identically to backpressure and duplicate submissions.

## 2. Endpoint Parity Matrix

| Route (alias) | Method | Handler & DTO | Norito / JSON contract & notes |
|---------------|--------|---------------|--------------------------------|
| `/transaction` (alias `/v2/pipeline/transactions`) | POST | `handler_post_transaction` consumes `NoritoVersioned<SignedTransaction>` before queuing via `handle_transaction_with_metrics`.【crates/iroha_torii/src/lib.rs:6297】【crates/iroha_torii/src/routing.rs:3443】 | Norito RPC clients set `Content-Type: application/x-norito` and receive the same empty 202-style acknowledgement as JSON callers. The alias is documented in the transport RFC table so API gateways may rewrite `/v2/pipeline/transactions` to `/transaction` transparently.【docs/source/torii/norito_rpc.md:126】 |
| `/v2/pipeline/transactions/status` | GET | Exposed through the app-API router and backed by the pipeline status query helpers (same DTOs consumed by the Android/Swift/JS clients). Swift picks the path via `pipelineEndpoints(for:)`, reinforcing the alias surface.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3412】 | Returns the canonical pipeline status envelope (`kind`, `content.hash`, `content.status`) regardless of transport. Norito RPC callers request the same DTO by sending `Accept: application/x-norito`; JSON callers fall back to Norito-backed JSON. |
| `/v2/pipeline/recovery/{height}` | GET | Registered alongside the server policy routes in `add_server_policy_and_pipeline_routes`, backed by `handler_pipeline_recovery`.【crates/iroha_torii/src/lib.rs:7763】【crates/iroha_torii/src/lib.rs:6362】 | Provides deterministic recovery metadata (chain id, settled block hash) so SDKs can prove which batches reached Torii during brownouts. The response is Norito JSON; NRPC users rely on the same DTO via `Accept: application/x-norito`. |
| `/v2/events/sse` | GET | `handle_v1_events_sse` streams `EventBox` payloads filtered through `EventsSseParams` and `event_to_json_value`.【crates/iroha_torii/src/routing.rs:15558】【crates/iroha_torii/src/routing.rs:18247】 | Until NRPC streaming lands, every SDK must bridge this SSE feed into typed pipeline observers. The JSON schema is derived from `PipelineEventBox` (hash, lane_id, dataspace_id, status, kind). |

## 3. Deterministic Retry & Queue Semantics

1. **Admission & throttling** – Submissions honour API-token gating and per-authority rate limiting before queuing the transaction.【crates/iroha_torii/src/lib.rs:6297】 Every SDK must surface the same limiter rejection by bubbling up the HTTP status (`429` unless `require_api_token` is enabled).

2. **Queue error envelope** – Queue failures are mapped to `QueueErrorEnvelope { code, message, queue { state, queued, capacity, saturated }, retry_after_seconds }` via `Error::queue_error_envelope`. Codes include `queue_full`, `per_user_queue_limit`, `already_enqueued`, `transaction_expired`, etc.【crates/iroha_torii/src/lib.rs:11076】【crates/iroha_torii/src/lib.rs:11090】 SDKs must parse the machine-readable `code` first and only retry when the envelope advertises `retry_after_seconds`.

3. **Status polling & retries** – Clients that call `/v2/pipeline/transactions/status` keep polling until the `kind` transitions out of `Queued`. A `404` response means Torii has no cached status yet (for example after a restart), so clients should treat it as pending and continue polling. Swift (`submitAndWait` + `PipelineStatusPollOptions`) and Java (`HttpClientTransport#waitForTransactionStatus`) already ship exponential backoff helpers; the same retry tables must be shared with JS to keep Android/JS/Swift cadence identical.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:287】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java:60】【javascript/iroha_js/src/toriiClient.js:1207】 Recommended defaults: 5 attempts, base delay 250 ms, jittered exponential up to 4 s, and a hard timeout supplied by the caller.

4. **Duplicate submissions** – When Torii responds with `already_enqueued` or `already_committed`, clients should treat the submission as successful and transition into status polling. This behaviour keeps eventual consistency with the queue’s dedupe semantics and mirrors the JSON guidance under “Torii Contracts API”.【docs/source/torii_contracts_api.md:21】

5. **Fallback order** – Norito RPC callers SHOULD attempt binary first and downgrade to JSON only when Torii returns `415 unsupported_layout`, `NRPC_DISABLED`, or a TLS failure. The downgrade must be logged alongside the queue envelope so operators know that `/v2/pipeline` carried the request.

### Retry Algorithm Reference Implementation

1. Submit transaction (binary or JSON).
2. If the response is success (`200` empty body) or `already_enqueued`, start polling.
3. If the response is `queue_full`/`per_user_queue_limit`, sleep for `retry_after_seconds` (defaults to 1) before re-submitting. Cap retries at the shared maximum.
4. Poll `/v2/pipeline/transactions/status?hash=<tx_hash>` using exponential backoff. Abort when:
   - Status kind ∈ {`Approved`, `Rejected`, `Expired`} → return final result.
   - Attempts exceed `maxAttempts` → raise `PipelineStatusTimeout`.
5. Record every retry/queue error in telemetry so Alertmanager rules can distinguish genuine load from client bugs.

## 4. SSE Bridge Contract


1. **Subscription** – Issue `GET /v2/events/sse` with `Accept: text/event-stream`. Optional `filter` query parameters accept the same JSON filter expressions handled in `routing.rs`. Proof-specific filters (`proof_backend`, `proof_call_hash`, `proof_envelope_hash`) mirror the extra selectors in the handler docstring.【crates/iroha_torii/src/routing.rs:15547】 Invalid or unsupported filters return `400 Bad Request`, and non-matching events are dropped without a `filtered` comment.
2. **Event payload** – Each `data:` chunk is the JSON produced by `event_to_json_value`. Pipeline transactions include `category`, `event`, `hash`, `lane_id`, `dataspace_id`, optional `block_height`, and `status` (stringified `TransactionStatus`). Blocks, warnings, merges, and witness events include their type-specific fields.【crates/iroha_torii/src/routing.rs:18247】
   Note: `Committed` is emitted after Kura persistence (before WSV apply). Clients that need apply semantics should wait for `Applied`. Pipeline events are emitted from the commit worker thread, so ordering with data events is not single-threaded; within a block the logical order remains `Committed` -> `Applied`.
3. **Lag handling** – When the SSE channel drops events, the handler emits `comment: lagged`; bridges must treat this as a signal to resubscribe and optionally fast-forward via `/v2/pipeline/recovery/{height}` or `/query` snapshots.
4. **Observer API** – SDKs should expose a shared `SseObserver` (or equivalent) with callbacks for `onEvent(PipelineEvent)`, `onLagged()`, and `onClosed(cause)`. Android reuses the Torii mock harness to exercise these callbacks, Swift pipes the JSON into structured `PipelineEvent` enums, and JS wraps `EventSource` to emit `ConnectEvent`s that match the Norito Connect telemetry schema.【java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/HttpClientTransportHarnessTests.java:74】【IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:335】【javascript/iroha_js/src/toriiClient.js:1252】
5. **Resubscribe semantics** – Bridges must retry SSE connections with capped exponential backoff (e.g., 250 ms base, capped at 2 s). When reconnecting, include the last observed `block_height` in diagnostics so operators can verify catch-up speed via `dashboards/grafana/torii_norito_rpc_observability.json`.

### Reference Pseudocode

```kotlin
interface PipelineEventObserver {
    fun onTransaction(hash: String, status: String, laneId: Long, dataspaceId: Long, blockHeight: Long?)
    fun onBlock(status: String)
    fun onLagged()
    fun onClosed(cause: Throwable?)
}
```

All SDKs should provide a default implementation that:

1. Wraps `EventSource`/`URLSession`/`java.net.http.HttpClient` SSE clients.
2. Parses each `data` payload with the Norito JSON helper.
3. Emits metrics (`connect.queue_depth`, `pipeline.status_latency_ms`) so dashboards can align Norito RPC and SSE observability.

## 5. SDK Implementation Checklist

| Item | Android | JavaScript | Swift |
|------|---------|------------|-------|
| Status polling helpers | `PipelineStatusOptions` + `PipelineStatusExtractor` define retry intervals and timeout enforcement to align with queue semantics.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransportStatusTests.java:38】 | `extractPipelineStatusKind` normalises the Norito JSON payload before surfacing typed statuses, ensuring parity with Android/Swift logic.【javascript/iroha_js/src/toriiClient.js:5039】 | `PipelineStatusPollOptions` encodes the same success/failure sets and retry windows; `submitAndWait` uses it by default.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:287】 |
| SSE bridge | `ToriiMockServer` and harness tests assert SSE observations and lag handling so Android parity evidence is reproducible during AND4 rehearsals.【java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/mock/ToriiMockServer.java:57】 | JS exposes `subscribePipelineEvents(filter)` built on `EventSource`, enforcing the same observer contract described above (bridge lives next to `toriiClient`).【javascript/iroha_js/src/toriiClient.js:1252】 | Swift’s `ToriiEventStream` wrapper uses `URLSession` SSE tasks and emits typed callbacks to match Android/JS observer signatures (tested under `IrohaSwiftTests`).【IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:335】 |

By adhering to this specification, AND4 deliverables can prove that every SDK hits identical HTTP routes, honours the same retry envelopes, and consumes the same SSE event structure regardless of whether they speak JSON or Norito RPC today.
