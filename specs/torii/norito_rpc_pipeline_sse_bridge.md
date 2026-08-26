# Torii Norito RPC ↔ `/v1/pipeline` Parity & SSE Bridge (AND4)

Status: Drafted for AND4 readiness – aligns with the “Design Torii Norito RPC parity & SSE bridge” roadmap item.【roadmap.md:1003】

## 1. Goals & Scope


Key outcomes:

1. Canonical mapping between `/v1/pipeline/*` routes and their Norito RPC equivalents for submissions, status polling, and recovery helpers.
2. One-shot admission and authoritative status semantics shared by every SDK, with no wire-format downgrade or signed-byte replay.

## 2. Endpoint Parity Matrix

| Route | Method | Handler & DTO | Norito / JSON contract & notes |
|---------------|--------|---------------|--------------------------------|
| `/v1/pipeline/transactions` | POST | `handler_post_transaction` consumes `NoritoVersioned<SignedTransaction>` before queuing via `handle_transaction_with_metrics`.【crates/iroha_torii/src/lib.rs:6297】【crates/iroha_torii/src/routing.rs:3443】 | Binary callers send exact canonical V1 bytes with `Content-Type: application/x-norito`. HTTP `202` is the sole admission success. JSON is a separate current ingress representation, never an automatic fallback.【specs/torii/norito_rpc.md:126】 |
| `/v1/pipeline/transactions/status` | GET | Exposed through the app-API router and backed by the pipeline status query helpers (same DTOs consumed by the Android/Swift/JS clients). Swift picks the path via `pipelineEndpoints(for:)`, reinforcing the canonical surface.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3412】 | Returns the canonical pipeline status envelope (`kind`, `content.hash`, `content.status`) regardless of transport. Norito RPC callers request the same DTO by sending `Accept: application/x-norito`; JSON callers fall back to Norito-backed JSON. |
| `/v1/pipeline/recovery/{height}` | GET | Registered alongside the server policy routes in `add_server_policy_and_pipeline_routes`, backed by `handler_pipeline_recovery`. The mount authenticates an exact-network `OperatorSignature` before handler work. | Provides node-local recovery metadata (chain id, settled block hash) so operators can prove which batches reached Torii during brownouts. The fresh signature covers `GET`, the substituted path, query, and empty body; redirects, retries, and token fallback are not permitted. The response is Norito JSON. |
| `/v1/events/sse` | GET | `handle_v1_events_sse` streams `EventBox` payloads filtered through `EventsSseParams` and `event_to_json_value`.【crates/iroha_torii/src/routing.rs:15558】【crates/iroha_torii/src/routing.rs:18247】 | Until NRPC streaming lands, every SDK must bridge this SSE feed into typed pipeline observers. The JSON schema is derived from `PipelineEventBox` (hash, lane_id, dataspace_id, status, kind). |

## 3. Deterministic Retry & Queue Semantics

1. **Admission & throttling** – Submissions honour API-token gating and per-authority rate limiting before queuing the transaction.【crates/iroha_torii/src/lib.rs:6297】 Every SDK must surface the same limiter rejection by bubbling up the HTTP status (`429` unless `require_api_token` is enabled).

2. **Queue error envelope** – Queue failures are mapped to `ErrorEnvelope { code, message, details { queue, retry_after_seconds, reject_code, entrypoint_hash?, tx_hash? } }` via `Error::queue_error_envelope`. Indeterminate durability outcomes identify the canonical entrypoint and include the signed-transaction hash only when one exists. Codes include `queue_full`, `per_user_queue_limit`, `already_enqueued`, and `transaction_expired`.【crates/iroha_torii/src/lib.rs:11076】【crates/iroha_torii/src/lib.rs:11090】 SDKs surface the machine-readable code but never replay the same signed bytes. Callers reconcile by hash and, only after authoritative rejection, may construct and sign a new transaction.

3. **Status polling & retries** – Status GETs are safe to retry. A `404` means no authoritative status is available yet, so clients keep polling. Success requires the state-resolved `Applied` terminal outcome with global scope and a positive block context; `Rejected` and `Expired` fail, and queue/cache observations are not finality. Swift (`submitAndWait` + `PipelineStatusPollOptions`) and Java (`HttpClientTransport#waitForTransactionStatus`) ship exponential-backoff helpers; JS uses the same bounded cadence.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:287】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java:60】【javascript/iroha_js/src/toriiClient.js:1207】 Recommended defaults: 5 attempts, 250 ms base delay, jittered exponential backoff capped at 4 s, and a caller-supplied hard timeout.

4. **Duplicate submissions** – `already_enqueued` and `already_committed` are not admission-success substitutes. They prove only that Torii recognized the hash. Clients fail the POST contract, do not redispatch, and resolve the exact hash through the authoritative status endpoint.

5. **No fallback order** – A binary submission never downgrades to JSON after `415`, a disabled route, TLS failure, redirect, timeout, or disconnect. Changing representation would create a second dispatch with an ambiguous first outcome. The caller chooses one current ingress representation before signing/dispatch and keeps it fixed.

### Retry Algorithm Reference Implementation

1. Validate one exact V1 transaction envelope and compute its canonical hash before network I/O.
2. Dispatch it once. Only HTTP `202` acknowledges admission; every other status or transport failure ends submission without replay.
3. Poll `/v1/pipeline/transactions/status?hash=<tx_hash>` using bounded exponential backoff. A `404` remains pending.
4. Return success only for authoritative global `Applied` evidence with a positive block context. Return failure for `Rejected` or `Expired`; time out without inventing finality.
5. Record status-read retries and submission ambiguity in telemetry without logging signing material or replaying the transaction.

## 4. SSE Bridge Contract


1. **Subscription** – Issue `GET /v1/events/sse` with `Accept: text/event-stream`. Optional `filter` query parameters accept the same JSON filter expressions handled in `routing.rs`. Proof-specific filters (`proof_backend`, `proof_call_hash`, `proof_envelope_hash`) mirror the extra selectors in the handler docstring.【crates/iroha_torii/src/routing.rs:15547】 Invalid or unsupported filters return `400 Bad Request`, and non-matching events are dropped without a `filtered` comment.
2. **Event payload** – Each `data:` chunk is the JSON produced by `event_to_json_value`. Pipeline transactions include `category`, `event`, `hash`, `lane_id`, `dataspace_id`, optional `block_height`, and `status` (stringified `TransactionStatus`). Blocks, warnings, merges, and witness events include their type-specific fields.【crates/iroha_torii/src/routing.rs:18247】
   Note: `Committed` is emitted after Kura persistence (before WSV apply). It is not client finality. After an `Applied` event, clients that require finality re-resolve the hash through the authoritative status endpoint and require global positive-block evidence. Pipeline events are emitted from the commit worker thread, so ordering with data events is not single-threaded; within a block the logical order remains `Committed` -> `Applied`.
3. **Lag handling** – When the SSE channel drops events, the handler emits `comment: lagged`; bridges must treat this as a signal to resubscribe and optionally fast-forward via `/v1/pipeline/recovery/{height}` or `/v1/query` snapshots.
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

By adhering to this specification, SDKs use one current ingress representation per dispatch, never replay ambiguous signed bytes, and derive finality from the same authoritative state rather than queue acknowledgements or SSE observations.
