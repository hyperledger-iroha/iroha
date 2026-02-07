---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1017858988f6bbc1c58029ca0476e2eee7b011c3c65ba5b33a80c049165600ca
source_last_modified: "2025-12-29T18:16:35.115752+00:00"
translation_last_reviewed: 2026-02-07
title: Norito-RPC Overview
---

# Norito-RPC Overview

Norito-RPC is the binary transport for Torii APIs. It reuses the same HTTP paths
as `/v1/pipeline` but exchanges Norito-framed payloads that include schema
hashes and checksums. Use it when you need deterministic, validated responses or
when pipeline JSON responses become a bottleneck.

## Why switch?
- Deterministic framing with CRC64 and schema hashes reduces decoding errors.
- Shared Norito helpers across SDKs let you reuse existing data-model types.
- Torii already tags Norito sessions in telemetry, so operators can monitor
adoption with the provided dashboards.

## Making a request

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v1/transactions/submit
```

1. Serialize your payload with the Norito codec (`iroha_client`, SDK helpers, or
   `norito::to_bytes`).
2. Send the request with `Content-Type: application/x-norito`.
3. Request a Norito response using `Accept: application/x-norito`.
4. Decode the response using the matching SDK helper.

SDK-specific guidance:
- **Rust**: `iroha_client::Client` negotiates Norito automatically when you set
  the `Accept` header.
- **Python**: use `NoritoRpcClient` from `iroha_python.norito_rpc`.
- **Android**: use `NoritoRpcClient` and `NoritoRpcRequestOptions` in the
  Android SDK.
- **JavaScript/Swift**: helpers are tracked in `docs/source/torii/norito_rpc_tracker.md`
  and will land as part of NRPC-3.

## Try It console sample

The developer portal ships a Try It proxy so reviewers can replay Norito
payloads without writing bespoke scripts.

1. [Start the proxy](./try-it.md#start-the-proxy-locally) and set
   `TRYIT_PROXY_PUBLIC_URL` so the widgets know where to send traffic.
2. Open the **Try it** card on this page or the `/reference/torii-swagger`
   panel and select an endpoint such as `POST /v1/pipeline/submit`.
3. Switch the **Content-Type** to `application/x-norito`, choose the **Binary**
   editor, and upload `fixtures/norito_rpc/transfer_asset.norito`
   (or any payload listed in
   `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Provide a bearer token via the OAuth device-code widget or the manual token
   field (the proxy accepts `X-TryIt-Auth` overrides when configured with
   `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Submit the request and verify that Torii echoes the `schema_hash` listed in
   `fixtures/norito_rpc/schema_hashes.json`. Matching hashes confirm that the
   Norito header survived the browser/proxy hop.

For roadmap evidence, pair the Try It screenshot with a run of
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. The script wraps
`cargo xtask norito-rpc-verify`, writes the JSON summary to
`artifacts/norito_rpc/<timestamp>/`, and captures the same fixtures that the
portal consumed.

## Troubleshooting

| Symptom | Where it appears | Likely cause | Fix |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii response | Missing or incorrect `Content-Type` header | Set `Content-Type: application/x-norito` before sending the payload. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii response body/headers | Fixture schema hash differs from the Torii build | Regenerate fixtures with `cargo xtask norito-rpc-fixtures` and confirm the hash in `fixtures/norito_rpc/schema_hashes.json`; fall back to JSON if the endpoint has not enabled Norito yet. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Try It proxy response | Request came from an origin that is not listed in `TRYIT_PROXY_ALLOWED_ORIGINS` | Add the portal origin (e.g., `https://docs.devnet.sora.example`) to the env var and restart the proxy. |
| `{"error":"rate_limited"}` (HTTP 429) | Try It proxy response | Per-IP quota exceeded the `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` budget | Increase the limit for internal load testing or wait until the window resets (see `retryAfterMs` in the JSON response). |
| `{"error":"upstream_timeout"}` (HTTP 504) or `{"error":"upstream_error"}` (HTTP 502) | Try It proxy response | Torii timed out or the proxy could not reach the configured backend | Verify that `TRYIT_PROXY_TARGET` is reachable, check Torii health, or retry with a larger `TRYIT_PROXY_TIMEOUT_MS`. |

More Try It diagnostics and OAuth tips live in
[`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Additional resources
- Transport RFC: `docs/source/torii/norito_rpc.md`
- Executive summary: `docs/source/torii/norito_rpc_brief.md`
- Action tracker: `docs/source/torii/norito_rpc_tracker.md`
- Try-It proxy instructions: `docs/portal/docs/devportal/try-it.md`
