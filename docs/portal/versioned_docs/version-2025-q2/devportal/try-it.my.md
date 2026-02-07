---
lang: my
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
translation_last_reviewed: 2026-02-07
---

# Try It sandbox

The developer portal ships an optional “Try it” console so you can call Torii
endpoints without leaving the documentation. The console relays requests
through the bundled proxy so browsers can bypass CORS limits while still
enforcing rate limits and authentication.

## Prerequisites

- Node.js 18.18 or newer (matches the portal build requirements)
- Network access to a Torii staging environment
- A bearer token that can call the Torii routes you plan to exercise

All proxy configuration is done through environment variables. The table below
lists the most important knobs:

| Variable | Purpose | Default |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Base Torii URL that the proxy forwards requests to | **Required** |
| `TRYIT_PROXY_LISTEN` | Listen address for local development (format `host:port` or `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Comma-separated list of origins that may call the proxy | `http://localhost:3000` |
| `TRYIT_PROXY_BEARER` | Default bearer token forwarded to Torii | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Allow end users to supply their own token via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Maximum request body size (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Upstream timeout in milliseconds | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Requests allowed per rate window per client IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Sliding window for rate limiting (ms) | `60000` |

The proxy also exposes `GET /healthz`, returns structured JSON errors, and
redacts bearer tokens from log output.

## Start the proxy locally

Install dependencies the first time you set up the portal:

```bash
cd docs/portal
npm install
```

Run the proxy and point it at your Torii instance:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

The script logs the bound address and forwards requests from `/proxy/*` to the
configured Torii origin.

## Wire the portal widgets

When you build or serve the developer portal, set the URL that the widgets
should use for the proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

The following components read these values from `docusaurus.config.js`:

- **Swagger UI** — rendered at `/reference/torii-swagger`; uses a request
  interceptor to attach bearer tokens automatically.
- **RapiDoc** — rendered at `/reference/torii-rapidoc`; mirrors the token field
  and supports try-it requests against the proxy.
- **Try it console** — embedded on the API overview page; lets you send custom
  requests, view headers, and inspect response bodies.

Changing the token in any widget only affects the current browser session; the
proxy never persists or logs the supplied token.

## Observability & operations

Every request is logged once with method, path, origin, upstream status, and the
authentication source (`override`, `default`, or `client`). Tokens are never
stored—both bearer headers and `X-TryIt-Auth` values are redacted before
logging—so you can forward stdout to a central collector without worrying about
secrets leaking.

### Health probes & alerting

Run the bundled probe during deployments or on a schedule:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Environment knobs:

- `TRYIT_PROXY_SAMPLE_PATH` — optional Torii route (without `/proxy`) to exercise.
- `TRYIT_PROXY_SAMPLE_METHOD` — defaults to `GET`; set to `POST` for write routes.
- `TRYIT_PROXY_PROBE_TOKEN` — injects a temporary bearer token for the sample call.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — overrides the default 5 s timeout.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — optional Prometheus textfile destination for `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` — comma-separated `key=value` pairs appended to the metrics (defaults to `job=tryit-proxy` and `instance=<proxy URL>`).

Feed the results into a textfile collector by pointing the probe at a writable
path (for example, `/var/lib/node_exporter/textfile_collector/tryit.prom`) and
adding any custom labels:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

The script rewrites the metrics file atomically so your collector always reads a
complete payload.

For lightweight alerting, wire the probe into your monitoring stack. A Prometheus
example that pages after two consecutive failures:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### Rollback automation

Use the management helper to update or restore the target Torii URL. The script
stores the previous configuration in `.env.tryit-proxy.bak` so rollbacks are a
single command.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Override the env file path with `--env` or `TRYIT_PROXY_ENV` if your deployment
stores configuration elsewhere.
