---
lang: he
direction: rtl
source: docs/source/sdk/js/index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 237a4db4cff5ee86efe8729bcf8ddba47ec1b3f27290802b7d86d1a7c20ed63f
source_last_modified: "2026-01-23T19:38:28.950325+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Iroha JS SDK Guides

```{toctree}
:maxdepth: 1

quickstart
torii_retry_policy
connect
offline
governance_iso_examples
publishing
publishing_ci_plan
release_playbook
fixture_cadence
support_playbook
validation
```

## SoraFS gateway helpers

The `sorafsGatewayFetch()` helper (see `javascript/iroha_js/src/sorafs.js`) now
raises a structured `SorafsGatewayFetchError` whenever the Rust orchestrator
returns a `MultiSourceError`. The error exposes the canonical code, retryable
flag, last provider failure, and capability mismatches so SDK consumers can
surface actionable diagnostics without parsing CLI output. Example:

```ts
import { sorafsGatewayFetch, SorafsGatewayFetchError } from "iroha-js";

try {
  await sorafsGatewayFetch(manifestHex, chunker, planJson, providers, opts);
} catch (error) {
  if (error instanceof SorafsGatewayFetchError) {
    console.warn("fetch failed", error.code, {
      chunk: error.chunkIndex,
      lastError: error.lastError,
      providers: error.providers,
    });
    if (!error.retryable) {
      throw error;
    }
  }
  throw error;
}
```

The native binding encodes the error payload as JSON, which the wrapper turns
into `SorafsGatewayFetchError`. Non-gateway failures continue to raise the
original exception.

## Torii retry telemetry

`ToriiClient` accepts an optional `retryTelemetryHook` callback that fires
every time the built-in HTTP client schedules a retry. The hook receives the
HTTP method, URL, attempt counters, retry cause, and the delay that will be
applied before the next request. The payload also exposes the selected retry
`profile` (`default`, `pipeline`, `streaming`, or any custom entry) so you can
distinguish between idempotent transaction submissions and long-lived stream
reconnections. This allows callers to feed retry events into
custom loggers or metrics pipelines without reimplementing the HTTP client:

```ts
const client = new ToriiClient(baseUrl, {
  fetchImpl,
  maxRetries: 3,
  retryTelemetryHook: (event) => {
    console.info("retrying Torii request", event);
  },
});
```

The hook runs inside a try/catch block so exceptions never interfere with the
retry loop.

Pipeline submissions (`/v2/pipeline/transactions` + `/v2/pipeline/transactions/status`)
automatically use the `pipeline` profile (POST retries enabled, 250 ms base backoff, 5 attempts),
while SSE endpoints use the `streaming` profile (longer retry window, 6 attempts). Override the
profiles via `resolveToriiClientConfig({ overrides: { retryProfiles: { … } } })` or by passing
`retryProfiles` directly to the `ToriiClient` constructor when you need different budgets.
If `/v2/pipeline/transactions/status` returns `404`, the JS client treats it as "pending" and
returns `null` so polling can continue after Torii restarts or cache eviction.
`ToriiClient.submitTransaction` validates `data_model_version` from `/v2/node/capabilities` and
throws `ToriiDataModelCompatibilityError` when it differs from the SDK's built-in value.
See {doc}`torii_retry_policy` for the full table of defaults, override knobs,
and error-handling expectations that governance audits during JS4/JS7 reviews.
