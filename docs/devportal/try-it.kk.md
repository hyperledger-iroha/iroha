---
lang: kk
direction: ltr
source: docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2025-12-29T18:16:35.067551+00:00"
translation_last_reviewed: 2026-02-07
title: Try It Sandbox Guide
summary: How to run the Torii staging proxy and developer portal sandbox.
---

The developer portal ships a “Try it” console for the Torii REST API. This guide
explains how to launch the supporting proxy and connect the console to a staging
gateway without exposing credentials.

## Prerequisites

- Iroha repository checkout (workspace root).
- Node.js 18.18+ (matches the portal baseline).
- Torii endpoint reachable from your workstation (staging or local).

## 1. Generate the OpenAPI snapshot (optional)

The console reuses the same OpenAPI payload as the portal reference pages. If
you have changed Torii routes, regenerate the snapshot:

```bash
cargo xtask openapi
```

The task writes `docs/portal/static/openapi/torii.json`.

## 2. Start the Try It proxy

From the repository root:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### Environment variables

| Variable | Description |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | Torii base URL (required). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Comma-separated list of origins allowed to use the proxy (defaults to `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | Optional default bearer token applied to all proxied requests. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Set to `1` to forward the caller’s `Authorization` header verbatim. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | In-memory rate limiter settings (defaults: 60 requests per 60 s). |
| `TRYIT_PROXY_MAX_BODY` | Maximum request payload accepted (bytes, default 1 MiB). |
| `TRYIT_PROXY_TIMEOUT_MS` | Upstream timeout for Torii requests (default 10 000 ms). |

The proxy exposes:

- `GET /healthz` — readiness check.
- `/proxy/*` — proxied requests, preserving the path and query string.

## 3. Launch the portal

In a separate terminal:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

Visit `http://localhost:3000/api/overview` and use the Try It console. The same
environment variables configure the Swagger UI and RapiDoc embeds.

## 4. Running unit tests

The proxy exposes a fast Node-based test suite:

```bash
npm run test:tryit-proxy
```

The tests cover address parsing, origin handling, rate limiting, and bearer
injection.

## 5. Probe automation & metrics

Use the bundled probe to verify `/healthz` and a sample endpoint:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Environment knobs:

- `TRYIT_PROXY_SAMPLE_PATH` — optional Torii route (without `/proxy`) to exercise.
- `TRYIT_PROXY_SAMPLE_METHOD` — defaults to `GET`; set to `POST` for write routes.
- `TRYIT_PROXY_PROBE_TOKEN` — injects a temporary bearer token for the sample call.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — overrides the default 5 s timeout.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — Prometheus textfile destination for `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` — comma-separated `key=value` pairs appended to the metrics (defaults to `job=tryit-proxy` and `instance=<proxy URL>`).

When `TRYIT_PROXY_PROBE_METRICS_FILE` is set, the script rewrites the file
atomically so your node_exporter/textfile collector always sees a complete
payload. Example:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

Forward the resulting metrics to Prometheus and reuse the sample alert in the
developer-portal docs to page when `probe_success` drops to `0`.

## 6. Production hardening checklist

Before publishing the proxy beyond local development:

- Terminate TLS ahead of the proxy (reverse proxy or managed gateway).
- Configure structured logging and forward to observability pipelines.
- Rotate bearer tokens and store them in your secrets manager.
- Monitor the proxy’s `/healthz` endpoint and aggregate latency metrics.
- Align rate limits with your Torii staging quotas; adjust the `Retry-After`
  behaviour to communicate throttling to clients.
