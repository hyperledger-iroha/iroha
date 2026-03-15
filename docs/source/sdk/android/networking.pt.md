---
lang: pt
direction: ltr
source: docs/source/sdk/android/networking.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3c642c57a16af69cacd696f58e53268c41cd2c43d40f7d0e4afbba73f178bec
source_last_modified: "2026-01-22T15:41:34.084125+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Networking & Telemetry Guide (AND4 / AND7)

This guide documents the Torii networking stack that ships with the Android SDK
(`java/iroha_android`). It connects roadmap items **AND4** (networking parity &
mock harness) and **AND7** (observability) by showing how `ClientConfig`,
`HttpClientTransport`, retry/queue helpers, Norito RPC, and telemetry observers
fit together. Pair it with the key-management (`key_management.md`), offline
signing (`offline_signing.md`), and telemetry policy (`telemetry_redaction.md`)
guides when preparing operator playbooks.

> **Sample walkthroughs:**  
> - [Operator console walkthrough](samples/operator_console.md#9-step-by-step-walkthrough) – Step 3 covers `/v2/pipeline` submissions, queue drain, and telemetry reconciliation using the `ClientConfig` + retry stack documented below.  
> - [Retail wallet design notes](samples/retail_wallet.md#5-telemetry--observability) – the offline sync + telemetry sections show how consumer apps reuse the Norito RPC helpers, queue diagnostics, and OTEL hooks described in this guide.

## 1. `ClientConfig` and manifest ingestion

`ClientConfig` (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`)
is the single source of truth for Torii endpoints, headers, retry/queue knobs,
and telemetry observers. Production builds construct it from the generated
`iroha_config` manifest (see `docs/source/references/configuration.md`) and reload
it through the config watcher described in `docs/source/android_runbook.md`.
Reload successes/failures emit the `android.telemetry.config.reload` signal so
SRE can audit manifest drift.

| `ClientConfig` field | Purpose | `iroha_config` source |
|----------------------|---------|-----------------------|
| `baseUri()` | Torii `/v2/pipeline` root used by the HTTP client and Norito RPC helper. | `[torii]` public API URL written into the Android manifest. |
| `sorafsGatewayUri()` | Gateway host for SoraFS fetches. | `[torii.sorafs]` (mirrors `[sorafs.discovery]`); defaults to `baseUri` when omitted. |
| `requestTimeout()` | Deterministic per-request timeout. | `[torii.client.timeout_ms]` (or the platform default when absent). |
| `defaultHeaders()` | Authorization, correlation IDs, SDK version. | `[telemetry.headers]` and operator-provided overrides. |
| `retryPolicy()` | Submission retry behaviour. | `[torii.retry.*]` (attempt cap, base delay, retryable status codes). |
| `pendingQueue()` | Offline replay buffer (see Section 2). | `[client.pending_queue]` storage path + quota. |
| `exportOptions()` | Deterministic key export + attestation bundling for queued entries. | `[client.pending_queue.export]` / key-policy rules. |
| `observers()` | Hooks that emit the telemetry signals catalogued in `telemetry_redaction.md`. | `[telemetry.observers]` manifest entries plus SDK defaults. |

> **Tip:** Store the manifest hash and salt epoch in `ClientConfig` logs so
> incident responders can prove which inputs were active. The runbook
> (`docs/source/android_runbook.md` §§1–2) captures the required log lines.

Typical ingestion flow:

```java
ClientConfig config =
    ClientConfig.builder()
        .setBaseUri(URI.create(manifest.torii().baseUrl()))
        .setSorafsGatewayUri(URI.create(manifest.torii().sorafsGateway()))
        .setRequestTimeout(Duration.ofSeconds(manifest.torii().timeoutSeconds()))
        .setRetryPolicy(RetryPolicy.builder()
            .setMaxAttempts(manifest.retry().maxAttempts())
            .setBaseDelay(Duration.ofMillis(manifest.retry().baseDelayMs()))
            .build())
        .setPendingQueue(FilePendingTransactionQueue.from(manifest.pendingQueue().path()))
        .setExportOptions(manifest.export().toExportOptions(keyManager))
        .setObservers(manifest.telemetry().observers())
        .setDefaultHeaders(manifest.http().headers())
        .build();
```

Telemetry redaction data from the manifest feeds into `TelemetryOptions`, which is then threaded
through `ClientConfig`. Supplying a `TelemetrySink` automatically registers
`TelemetryObserver` so every Torii request emits hashed-authority signals:

```java
TelemetryOptions telemetry =
    TelemetryOptions.builder()
        .setTelemetryRedaction(
            TelemetryOptions.Redaction.builder()
                .setSaltHex(manifest.telemetry().redactionSaltHex())
                .setSaltVersion(manifest.telemetry().saltVersion())
                .build())
        .build();

ClientConfig config =
    ClientConfig.builder()
        .setBaseUri(URI.create(manifest.torii().baseUrl()))
        .setTelemetryOptions(telemetry)
        .setTelemetrySink(new OtelTelemetrySink(openTelemetry))
        .build();
```


## 2. Deterministic retries & offline queueing

`RetryPolicy` (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/RetryPolicy.java`)
enforces deterministic backoff: attempt counts, base delay, and retryable
statuses come directly from the manifest so device fleets share identical timing.
Pair it with a `PendingTransactionQueue` to persist signed transactions whenever
the device is offline:

```java
PendingTransactionQueue queue =
    new FilePendingTransactionQueue(context.getFileStreamPath("pending.queue").toPath());

ClientConfig config = ClientConfig.builder()
    .setBaseUri(baseUri)
    .setRetryPolicy(RetryPolicy.builder()
        .setMaxAttempts(5)
        .setBaseDelay(Duration.ofMillis(250))
        .build())
    .setPendingQueue(queue)
    .build();
```

- `FilePendingTransactionQueue`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/queue/FilePendingTransactionQueue.java`)
stores envelopes in insertion order and survives process restarts. The
`PendingQueueInspector` CLI (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/tools/PendingQueueInspector.java`)
decodes queue files for AND7 chaos drills and incident timelines.
- `DirectoryPendingTransactionQueue`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/queue/DirectoryPendingTransactionQueue.java`)
  persists one envelope per file under a caller-provided directory so OEM/MDM storage policies can
  manage queue artefacts without parsing a shared log. Wire it via
  `ClientConfig.Builder.enableDirectoryPendingQueue(path)` or
  `HttpClientTransport.withDirectoryPendingQueue(config, path)`; entries share the Norito envelope
  format consumed by `PendingQueueInspector`.
- `HttpClientTransport` automatically drains the queue before submitting new
transactions. Failures re-enqueue the remaining entries without re-ordering.

Declare the queue in your manifest so config reloads pick it up automatically:

```json
"pending_queue": {
  "kind": "file",
  "path": "queues/pending.queue"
}
```

Paths are resolved relative to the manifest location. Use `"kind": "offline_journal"`
with `key_seed_b64`/`key_seed_hex`/`passphrase` when you need the authenticated
WAL-format queue shared with Rust/Swift.

## 3. Torii HTTP + SoraFS surfaces

`HttpClientTransport`
(`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java`)
implements `/transaction` (alias `/v2/pipeline/transactions`), `/v2/pipeline/transactions/status`, and
SoraFS gateway helpers on top of `java.net.http.HttpClient`. Core entry points:

```java
HttpClientTransport transport =
    new HttpClientTransport(HttpClient.newHttpClient(), config);

// Submit and wait for commit.
SignedTransaction tx = builder.encodeAndSign(payload, alias, preference);
transport.submitTransaction(tx)
    .thenCompose(resp -> transport.waitForTransactionStatus(
        resp.hashHex(), PipelineStatusOptions.builder().setTimeoutMillis(60_000).build()))
    .join();

// Fetch content from the configured SoraFS gateway.
GatewayFetchSummary summary =
    transport.newSorafsGatewayClient()
        .fetchSummary(GatewayFetchRequest.forAlias("docs-portal"))
        .join();
```

- `waitForTransactionStatus` polls `/v2/pipeline/transactions/status` using the
  `PipelineStatusExtractor` so callers receive the structured Torii payload. A
  `404` response means Torii has no cached status yet (for example after a
  restart), so the client keeps polling until a terminal state arrives.
- Torii returns a Norito-encoded submission receipt in the response body. Use
  `ClientResponse.body()` to access the raw receipt bytes or rely on
  `ClientResponse.hashHex()` for deterministic status polling.
- `newSorafsGatewayClient()` reuses the same executor, timeout, headers, and
observers while targeting the `sorafsGatewayUri`. See
`docs/source/sorafs/developer/sdk/index.md` for gateway request details.

## 4. Norito RPC & binary payloads

AND4 also requires parity with the Norito RPC transport. `ClientConfig` exposes
`toNoritoRpcClient()` so callers can reuse the same base URI, timeout, and
observers when emitting `application/x-norito` frames:

```java
NoritoRpcClient rpcClient = config.toNoritoRpcClient();

byte[] response = rpcClient.call(
    "/norito/v2/contracts/invoke",
    myNoritoPayload,
    NoritoRpcRequestOptions.builder()
        .putHeader("X-Iroha-Trace", traceId)
        .setTimeout(Duration.ofSeconds(5))
        .build());
```

Requests inherit the default headers from `ClientConfig` (auth tokens, tracing
IDs) and honour per-request overrides defined through
`NoritoRpcRequestOptions`. Telemetry observers (Section 5) fire for RPC calls as
well so the same dashboards track REST and binary traffic.

Norito RPC calls emit `android.norito_rpc.call` with hashed authority, route, method, status code,
latency, outcome (`success`/`failure`/`fallback`), and a `fallback` flag so dashboards can separate
binary-transport health from REST telemetry even when the fallback handler is invoked.

When you already have a custom `HttpTransportExecutor` (for example the one
backing `HttpClientTransport` or a mocked executor in tests), call
`ClientConfig.toNoritoRpcClient(executor)` to reuse the exact same interceptor
stack and connection pool for binary calls. `HttpClientTransport#newNoritoRpcClient()`
delegates to this helper automatically, so REST submissions, Norito RPC payloads,
and offline queue flushes all emit identical telemetry.

`ClientConfig.Builder#setNoritoRpcMaxConcurrentRequests` (or
`setNoritoRpcFlowController`) caps the number of in-flight RPC calls so mobile
queues do not overwhelm Torii when background retries spike:

```java
ClientConfig config =
    ClientConfig.builder()
        .setBaseUri(baseUri)
        .setNoritoRpcMaxConcurrentRequests(4)
        .setNoritoRpcFallbackHandler(
            (context, error) -> {
              // Optionally degrade to the JSON pipeline when the Norito
              // endpoint is unavailable.
              return httpTransport.callLegacyPipeline(context.path(), context.payload());
            })
        .build();
```

Fallbacks emit `ClientObserver#onFailure` for the original error and then a
synthetic `ClientResponse` with status code `299` to signal the degraded
response; dashboards can chart both signals to distinguish primary failures from
fallback recoveries.

For transaction-centric flows, call `NoritoRpcClient#callTransaction` to reuse
the shared Norito codecs shipped in `norito-java`:

```java
TransactionPayload payload = TransactionPayload.builder()
    .setAuthority("i105...")
    .setInstructionBytes(KotodamaCompiler.compile(contract))
    .build();

TransactionPayload decoded =
    rpcClient.callTransaction(
        "/norito/v2/transactions/submit",
        payload,
        new NoritoJavaCodecAdapter(),
        NoritoRpcRequestOptions.defaultOptions());
```

The helper encodes and decodes via the supplied `NoritoCodecAdapter`, keeping
schema hashes aligned with the Rust server while sparing app code from manual
byte handling.

## 5. Telemetry observers & redaction

`ClientObserver`
(`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientObserver.java`)
provides request/response/failure hooks. Register an observer via
`ClientConfig.Builder#addObserver` to emit the signals listed in
`docs/source/sdk/android/readiness/signal_inventory_worksheet.md`:

`TelemetryObserver` (backed by the manifest-derived `TelemetryOptions`) now ships in
`java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryObserver.java`.
When paired with a `TelemetrySink` implementation the observer emits the signals documented in
`docs/source/sdk/android/readiness/signal_inventory_worksheet.md`, including the hashed-authority
metric required by AND7. Applications no longer need to hand-roll observers—setting
`setTelemetryOptions(...)` + `setTelemetrySink(...)` on the builder wires the default observer
alongside any custom observers already registered.

### 5.1 OpenTelemetry contract

`TelemetryObserver` exposes the same span/metric contract described in the NRPC spec
(`docs/source/torii/nrpc_spec.md`) so HTTP and Norito transports land on the shared dashboards:

| Signal | Type | Source | Notes |
|--------|------|--------|-------|
| `torii.request.duration` / `torii.request.bytes` | OTEL span + histogram | Emitted for **every** REST + Norito RPC call. Attributes capture `method`, `path`, `transport` (`http`/`norito_rpc`), TLS version/cipher, Torii/Norito status (`X-Iroha-Error-Code`), retry attempt, queue depth, and manifest/SKD version so regressions can be bisected in Grafana. |
| `torii_rpc_retry_total` | Counter | Incremented whenever `RetryPolicy` schedules another attempt. Labels: `endpoint`, `reason`, `transport`. Keeps the AND4 retry budget visible when storms force fallback to JSON submissions. |
| `android_sdk_submission_latency_seconds_bucket` | Histogram | End-to-end latency (device → Torii ack). Dashboards render it as `android_sdk_submission_latency_p95` with the thresholds captured in `docs/source/android_support_playbook.md`. |
| `android_sdk_pending_queue_depth` | Gauge | Samples queue length before/after every drain so outage drills and `PendingQueueInspector` reports can prove deterministic replay behaviour. |
| `android_sdk_offline_replay_errors_total` | Counter | Counts permanent queue failures (labels: `reason`, `transport`). AND7 chaos scenarios (see `docs/source/sdk/android/telemetry_chaos_checklist.md`) assert this stays zero during brownouts. |
Queue replays also annotate the `torii.request` spans with `queue_replay=true` so Grafana can plot
how long it took to drain a backlog during outages.

All failure paths emit a `torii.request.failure` OTEL event with the hashed authority, HTTP status,
canonical Torii/Norito status code, and TLS diagnostic fields. These events back the incident
timeline exports gathered via `scripts/telemetry/check_redaction_status.py` and must stay in sync
with the dashboards/alerts referenced in the support playbook. When new transports, TLS policies,
or queue strategies land, keep this table updated so AND4 (networking) and AND7 (telemetry) reviews
share the same observability contract.

Redaction requirements (hashed authorities, bucketed device profiles, removal
of carrier names) are defined in `telemetry_redaction.md` and validated via the
schema diff artefacts under
`docs/source/sdk/android/readiness/schema_diffs/`. Run
`scripts/telemetry/check_redaction_status.py` to confirm exporter health before
publishing builds; AND7 chaos drills (`telemetry_chaos_checklist.md`) rely on
those counters.

## 6. Diagnostics & runbooks

- Follow `docs/source/android_runbook.md` for Torii/network timeout, StrongBox,
and manifest-drift incidents. Every troubleshooting section references the
helpers above (queue inspector, schema diff tool, telemetry scripts).
- The `scripts/telemetry/run_schema_diff.sh` and
`scripts/telemetry/test_torii_norito_rpc_alerts.sh` helpers verify alert parity
against the Rust baseline before shipping new observers or retry policies.
- Chaos exercises (`docs/source/sdk/android/readiness/labs/telemetry_lab_01.md`)
inject queue growth, exporter brownouts, and override tokens; archive the
resulting artefacts under `docs/source/sdk/android/readiness/archive/<date>/`.

## 7. Torii mock harness parity (AND4)

Roadmap item **AND4** requires every SDK (Android first) to exercise the shared
Torii mock harness so networking changes prove parity before hitting staging.
The harness is distributed in `tools/torii_mock_harness/` and wraps the
existing Android JUnit suite
(`java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/HttpClientTransportHarnessTests.java`)
behind a CLI that other SDKs can reuse.

### Local smoke workflow

1. Regenerate fixtures on the current commit so the harness consumes the same
   bundle referenced by CI:

   ```bash
   ./scripts/android_fixture_regen.sh
   ```

2. Run the CLI with the desired scenario (see
   `tools/torii_mock_harness/config/default.toml`). The `--sdk` label feeds the
   Prometheus metrics and the repo-root flag keeps editor shells working outside
   of CI:

   ```bash
   cargo run -p xtask --bin torii-mock-harness -- \
     --sdk android \
     --scenario submit \
     --metrics-path artifacts/torii_mock/android_submit.prom \
     --repo-root "$(git rev-parse --show-toplevel)"
   ```

   The shim at `tools/torii_mock_harness/bin/torii-mock-harness` performs the
   same work but falls back to `cargo run` automatically when the binary is not
   built yet.

3. Inspect the generated `.prom` file (defaults to
   `mock-harness-metrics-android.prom`) and upload it together with the harness
   log whenever you attach evidence to a change request or roadmap review.

### CI & telemetry expectations

- `.github/workflows/torii-mock-harness.yml` already runs the Android harness on
  every push, but developers can trigger it manually (GitHub → *Actions →
  torii-mock-harness smoke*). The workflow publishes the CLI output plus the
  Prometheus snippet under the run’s artefacts.
- Every run must export the three canonical metrics described in
  `tools/torii_mock_harness/README.md`: `torii_mock_harness_retry_total`,
  `torii_mock_harness_duration_ms`, and `torii_mock_harness_fixture_version`.
  Failing to emit a metric blocks the AND4 acceptance check in
  `dashboards/grafana/torii_mock_harness.json`.
- When replaying scenarios locally, push the `.prom` file via
  `curl -XPOST <pushgateway> --data-binary @mock-harness-metrics-android.prom`
  so the dashboard mirrors the CI signal. Annotate the upload with the
  `torii-mock-harness` label and include the fixture hash printed by the CLI.

Following this workflow keeps the Android guide aligned with the shared harness
contract documented in `docs/source/torii/norito_rpc_brief.md` and satisfies the
roadmap requirement to prove mock-harness parity before AND4 preview builds.

## 8. References

- `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
- `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java`
- `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/queue/FilePendingTransactionQueue.java`
- `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/NoritoRpcClient.java`
- `java/iroha_android/src/main/java/org/hyperledger/iroha/android/tools/PendingQueueInspector.java`
- `docs/source/android_runbook.md`
- `docs/source/sdk/android/telemetry_redaction.md`
- `docs/source/sdk/android/readiness/`
