---
lang: ka
direction: ltr
source: docs/source/sdk/js/torii_retry_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 78cbe54aa306d854bc9c21db8ffc74bd288acb61a0ff8a50b7858e969037ddf4
source_last_modified: "2026-01-06T13:12:33.162926+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Torii retry & error policy

This note documents the retry/error contract that the JS SDK must follow for
the JS4/JS7 roadmap gates (see `roadmap.md` “JavaScript Next Checkpoints”).
Android reuses the same profile names and telemetry hooks, so keeping this file
up to date gives governance a single reference when comparing SDK behaviours.

## Endpoint mapping (shared across SDKs)

- **Default** — metadata/read-only routes (status, telemetry, explorer) fall
  back to the `default` profile.
- **Pipeline** — transaction submission/status helpers, governance trigger
  helpers, ISO bridge routes, and Connect registration calls use
  `retryProfile: "pipeline"` so POST retries stay bounded and observable.
- **Streaming** — SoraFS/Norito streaming requests, explorer replay, and SSE
  helpers use `retryProfile: "streaming"` to give long-lived flows a wider
  window without overwhelming Torii.

The profile names and mappings mirror the Android transport
(`docs/source/sdk/android/networking.md`). The JS defaults are exported for
lint/tests and cross-SDK parity checks:
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`.

## Default retry profiles

`ToriiClient` bootstraps three deterministic profiles. API methods select the
profile automatically, but the knobs can also be set explicitly via
`retryProfile` options (see below).

| Profile | Typical call sites | Max retries | Initial delay | Multiplier | Max backoff | Methods | Status codes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `default` | Metadata/read-only requests | 3 | 500 ms | ×2 | 5 s | `GET`, `HEAD`, `OPTIONS` | 429, 502, 503, 504 |
| `pipeline` | `/v2/pipeline/transactions{,/status}` | 5 | 250 ms | ×1.8 | 8 s | `GET`, `POST`, `HEAD` | 408, 425, 429, 500, 502, 503, 504 |
| `streaming` | Event/SSE/WebSocket helpers | 6 | 500 ms | ×1.5 | 12 s | `GET` | 408, 425, 429, 500, 502, 503, 504 |

Behavioural notes:

- All profiles obey the global `timeoutMs` (30 s by default); long-lived
  callers should pass an `AbortSignal`.
- `timeoutMs`, `maxRetries`, `backoffInitialMs`, `maxBackoffMs`, and retry status
  lists must be non-negative integers.
- SSE helpers and `openConnectWebSocket` pass `retryProfile: "streaming"` so
  relays reconnect without custom glue.
- Transaction submissions and status polling use the pipeline profile so POST
  retries stay bounded and loggable. The hash makes submissions idempotent, so
  Torii can safely deduplicate.
- Requests that do not opt into a named profile fall back to the `default`
  envelope (GET-only retries).

## Customising policies

`resolveToriiClientConfig()` merges `toriiClient` config blocks,
environment overrides, and inline overrides. Every profile is just a patch
object applied on top of the default envelope. The same API drives Android’s
retry plan so operators can feed the same values to every SDK.

```ts
import {ToriiClient, resolveToriiClientConfig} from "iroha-js";

const config = resolveToriiClientConfig({
  config: {
    toriiClient: {
      timeoutMs: 20000,
      retryProfiles: {
        pipeline: { maxRetries: 7, maxBackoffMs: 10_000 },
      },
    },
  },
  overrides: {
    retryProfiles: {
      metrics: { retryMethods: ["GET"], maxRetries: 4 },
    },
  },
});

const client = new ToriiClient("https://torii.dev", {
  ...config,
  retryProfiles: {
    ...config.retryProfiles,
    metrics: { ...config.retryProfiles.metrics },
  },
});

// Force the streaming profile for a custom SSE call.
await client._request("GET", "/v2/custom/events", {retryProfile: "streaming"});
```

### Environment overrides

During tests/CI it is often easier to tweak the global envelope via
environment variables. All values accept comma-separated lists.

| Variable | Description |
| --- | --- |
| `IROHA_TORII_TIMEOUT_MS` | Request timeout applied before retry logic |
| `IROHA_TORII_MAX_RETRIES` | Default profile retry budget |
| `IROHA_TORII_BACKOFF_INITIAL_MS` / `IROHA_TORII_BACKOFF_MULTIPLIER` / `IROHA_TORII_MAX_BACKOFF_MS` | Default profile backoff params |
| `IROHA_TORII_RETRY_STATUSES` | HTTP statuses that are safe to retry |
| `IROHA_TORII_RETRY_METHODS` | HTTP verbs that are retryable (upper-case) |
| `IROHA_TORII_API_TOKEN` / `IROHA_TORII_AUTH_TOKEN` | Default credential headers |

### Per-request overrides

Most public methods already apply a profile, but any call that ends up in
`ToriiClient._request` accepts `retryProfile`. Custom services (e.g., bespoke
pipelines or partner-only endpoints) should pick the closest built-in profile
so observability can classify retries correctly. Avoid inventing profile names
unless the Android/Swift/JS teams agree on the telemetry labels.

## Telemetry & alerting hooks

Pass a `retryTelemetryHook` when constructing the client to record retry
activity. The hook runs in a try/catch so it cannot break I/O.

```ts
const client = new ToriiClient(baseUrl, {
  ...resolveToriiClientConfig(),
  retryTelemetryHook(event) {
    metrics.counter(`torii.retry.${event.profile}`).inc({
      phase: event.phase,
      method: event.method,
      status: event.status ?? event.errorName ?? "unknown",
    });
  },
});
```

Payload fields include:

- `phase`: `"response"`, `"network"`, or `"timeout"`.
- `attempt` / `nextAttempt` / `maxRetries`.
- `method`, `url`, active `profile`.
- Either the HTTP `status` or `{errorName,errorMessage}` for network faults.
- `timedOut` for abort-triggered retries.

SREs expect retry spikes to be correlated with these metrics; attach the hook
output to incident reports alongside `iroha_config` diffs whenever a profile is
changed.

## Error surface

HTTP failures raise `ToriiHttpError`, which includes:

| Field | Meaning |
| --- | --- |
| `status` / `statusText` | Raw HTTP status returned by Torii |
| `expected` | List of status codes the SDK was waiting for |
| `code` | Torii error code (e.g., `PIPELINE_TX_REJECTED`) if present |
| `errorMessage` | Message extracted from the JSON payload |
| `bodyJson` / `bodyText` | Raw response body for troubleshooting |

Network failures surface whatever the underlying fetch implementation throws
(`AbortError`, `TypeError`, `ECONNRESET`, …). Pair the retry telemetry hook
with standard `try/catch` handling:

```ts
import {ToriiHttpError} from "iroha-js";

try {
  await client.submitTransaction(payload);
} catch (error) {
  if (error instanceof ToriiHttpError) {
    if (retryablePipelineCodes.has(error.code)) {
      // Ask the operator to requeue or inspect the hash.
    }
    throw error;
  }
  if (error.name === "AbortError") {
    // log timeout
  }
  throw error;
}
```

## References

- `javascript/iroha_js/src/config.js`
- `javascript/iroha_js/src/toriiClient.js`
- `docs/source/sdk/android/networking.md` (shared retry policy expectations)
