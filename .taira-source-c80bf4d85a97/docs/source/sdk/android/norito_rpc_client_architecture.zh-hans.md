---
lang: zh-hans
direction: ltr
source: docs/source/sdk/android/norito_rpc_client_architecture.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 539700ec0ded45fc4b94018cf06c4338ebaa1b50811d5c4408fe297d001d4576
source_last_modified: "2026-01-05T09:28:12.058022+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Norito RPC Android Client Architecture (NRPC-3 / AND4)

This document closes roadmap item **NRPC-3** by describing the Android Norito RPC
client design, its flow-control and fallback hooks, and how it reuses the shared
Norito codecs. Pair it with the networking guide (`networking.md`) for higher
level usage examples and the telemetry redaction plan
(`telemetry_redaction.md`) for signal governance.

## 1. Goals & Scope

- Provide a transport abstraction that emits `application/x-norito` payloads
  while sharing configuration, telemetry, and retry semantics with the REST
  pipeline.
- Offer deterministic backpressure controls and fallbacks so mobile apps can
  degrade to the JSON pipeline without forking business logic.
- Integrate directly with the Norito codec stack so SDK users never need to
  hand-roll binary encoders.
- Document observability expectations, mock-harness coverage, and readiness
  checks so AND4/AND7 owners can prove parity with Rust clients.

## 2. Component Map

| Component | Responsibility | Source |
|-----------|----------------|--------|
| `NoritoRpcClient` | Core transport that builds `java.net.http.HttpRequest`s, applies defaults, fires observers, and returns raw byte responses (or codec-decoded payloads). | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/NoritoRpcClient.java` |
| `NoritoRpcRequestOptions` | Per-call overrides for headers, method, timeout, Accept handling, and query parameters. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/NoritoRpcRequestOptions.java` |
| `NoritoRpcFlowController` | Limits concurrent RPC calls (semaphore-backed or unlimited) to protect Torii and device resources. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/NoritoRpcFlowController.java` |
| `NoritoRpcFallbackHandler` + `NoritoRpcCallContext` | Optional hook that can replay the request via the JSON pipeline or another transport when Norito RPC fails, reusing the captured context. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/NoritoRpcFallbackHandler.java` / `NoritoRpcCallContext.java` |
| `ClientObserver` | Emits telemetry (`android.torii.*`) for every RPC call, mirroring the HTTP client hooks. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientObserver.java` |
| `ClientConfig` | Single source of truth for Torii URIs, default headers, observers, flow-control, and fallback wiring. `toNoritoRpcClient()` / `toNoritoRpcClient(HttpTransportExecutor)` produce ready-to-use RPC instances that reuse the same instrumentation as the REST stack (also exposed via `HttpClientTransport#newNoritoRpcClient()`). | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` / `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java` |
| `NoritoCodecAdapter` + `TransactionBuilder` | Encode/decode Norito payloads (`callTransaction`) so applications keep schema hashes aligned with Rust nodes. | `java/norito_java/src/main/java/org/hyperledger/iroha/android/norito/NoritoCodecAdapter.java`, `java/iroha_android/src/main/java/org/hyperledger/iroha/android/tx/TransactionBuilder.java` |

## 3. Request Lifecycle

1. **Configuration** — Apps build `ClientConfig` from the manifest, populating
   base URIs, default headers, observers, `RetryPolicy`, and the optional
   `NoritoRpcFlowController`/`NoritoRpcFallbackHandler`.
2. **Client construction** — `ClientConfig#toNoritoRpcClient(HttpClient)` (or
   `toNoritoRpcClient(HttpTransportExecutor)` when sharing a custom executor)
   hands the configuration to `NoritoRpcClient.Builder`, carrying over
   timeouts, headers, telemetry observers, and the chosen transport. When
   invoked via `HttpClientTransport#newNoritoRpcClient()` the REST transport’s
   executor gets reused so RPC and JSON submissions share identical telemetry
   and retry plumbing.
3. **Request assembly** — `NoritoRpcClient#call(path, payload, options)` merges
   default headers with per-call overrides, applies query parameters, and
   assigns `Content-Type`/`Accept` defaults when callers omit them.
4. **Flow control** — Before dispatching, `NoritoRpcFlowController#acquire`
   guards concurrency. Semaphore controllers keep fairness by design.
5. **Dispatch & observation** — The client notifies observers (`onRequest`)
   before `HttpClient#send`, then triggers `onResponse` or `onFailure`
   depending on the outcome. Successful calls surface the raw bytes and status
   code; failures wrap the cause in `NoritoRpcException`.
6. **Fallback (optional)** — When the primary call fails, `NoritoRpcClient`
   invokes the configured `NoritoRpcFallbackHandler`, passing a snapshot
   (`NoritoRpcCallContext`) so the handler can replay the request via the JSON
   pipeline or another circuit. Returned bytes short-circuit the exception path
   and emit a synthetic `ClientResponse` with status `299`.
7. **Release** — The flow controller releases its permit and the response bytes
   propagate back to the caller. Helper `callTransaction` encodes/decodes via
   the supplied codec adapter before returning typed payloads.

## 4. Flow Control & Backpressure

- `ClientConfig.Builder#setNoritoRpcMaxConcurrentRequests(n)` wraps the helper
  in a semaphore so mobile apps cap outstanding RPCs per process or per queue.
- Use unlimited controllers for foreground/interactive flows and semaphore
  controllers when replaying queues, performing background sync, or hitting
  high-fanout APIs (e.g., ISO settlement batches).
- Observers can emit queue-depth metrics by inspecting `NoritoRpcCallContext`
  metadata or the global pending queue; AND4 dashboards consume the
  `android.torii.http.request`/`android.torii.http.retry` signals documented in
  `telemetry_redaction.md`.

## 6. Codec Integration & Typed Helpers

- `NoritoRpcClient#callTransaction` bridges the RPC transport with
  `NoritoCodecAdapter`. It encodes the supplied `TransactionPayload`, issues the
  RPC call, and decodes the binary response back into a typed payload. This
  keeps schema hashes aligned with Rust nodes and reuses the same codec as
  `TransactionBuilder`.
- For non-transaction payloads, callers can supply any codec implementation
  (e.g., builders generated via the AND3 codegen pipeline).
- The helper is covered by `NoritoRpcClientTests.callTransactionUsesCodecAdapter`
  so regressions surface in `make android-tests`.

## 7. Observability & Telemetry

- `ClientObserver` hooks fire for every Norito RPC call. The default observers
  emit the signals catalogued in
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`, the
  redaction plan, and the AND7 readiness outline.
- Failures trigger both `onFailure` (with the original `NoritoRpcException`)
  and, when fallback succeeds, `onResponse` with the synthetic `299` response.
  Dashboards can therefore plot primary failure rates and fallback success
  rates independently.
- Observers also carry tracing headers (e.g., `X-Iroha-Trace`) without mutating
  the transport. Instrumentation guidance lives in `networking.md` §5 and the
  telemetry plan.

## 8. Configuration Surface

- **From manifests:** `ClientConfig` consumes `iroha_config` to set base URIs,
  headers, retry policy, queue locations, and telemetry observers. Operators
  can extend the manifest schema with `norito_rpc_max_concurrent_requests` or
  fallback toggles if tighter control is needed; the builder already exposes
  the hooks.
- **Per-request overrides:** `NoritoRpcRequestOptions` controls method overrides
  (default `POST`), Accept removal (`accept(null)`), timeout adjustments, and
  query parameters so apps can set `?schema=` selectors or replay tokens
  without string concatenation.
- **Shared HTTP client:** Pass an existing `java.net.http.HttpClient` into
  `toNoritoRpcClient` to reuse connection pools and TLS caches; otherwise the
  helper builds a new client with the platform defaults.

## 9. Testing & Mock Harness

- `NoritoRpcClientTests` spins up an embedded HTTP server (or `ToriiMockServer`
  when available) to exercise header propagation, method overrides, flow
  control, fallback semantics, and codec helpers. Run via `make android-tests`
  or `ci/run_android_tests.sh`.
- `ClientConfigNoritoRpcTests` validates that manifest-derived configs carry
  observers, flow controllers, and fallback handlers into the RPC helper.
- Cross-SDK parity relies on the shared Torii mock harness documented in
  `tools/torii_mock_harness/README.md`. Android’s AND4 preview lane and the
  JS/Python parity suites replay the same scenarios to prove transport and
  telemetry behaviour matches the RFC.

## 10. Deployment Checklist

1. **Wire manifests** — Ensure `ClientConfig` pulls base URIs, headers, retry
   budget, observer list, flow controller, and fallback handler from the same
   configuration loader that feeds the REST client.
2. **Set concurrency caps** — Use `setNoritoRpcMaxConcurrentRequests` for
   background queues; keep unlimited controllers only for foreground UX.
3. **Register telemetry observers** — Reuse the observers described in the
   telemetry plan so Norito RPC calls share the same redaction and retention
   controls as HTTP traffic.
4. **Decide fallback policy** — For environments that still require JSON,
   implement a fallback handler that calls `HttpClientTransport` and emits
   observability breadcrumbs for `299` responses.
5. **Run mock-harness + unit tests** — Execute `make android-tests` and the
   shared mock harness to capture evidence for AND4/NRPC gate reviews.

Following this architecture keeps the Android SDK aligned with the Torii
Norito-RPC rollout plan and satisfies the NRPC-3 deliverables: deterministic
flow control, codec reuse, fallback governance, and documented readiness hooks.
