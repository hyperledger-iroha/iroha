---
lang: kk
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
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
| `TRYIT_PROXY_CLIENT_ID` | Identifier placed in `X-TryIt-Client` for every upstream request | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Default bearer token forwarded to Torii | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Allow end users to supply their own token via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Maximum request body size (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Upstream timeout in milliseconds | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Requests allowed per rate window per client IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Sliding window for rate limiting (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Optional listen address for the Prometheus-style metrics endpoint (`host:port` or `[ipv6]:port`) | _empty (disabled)_ |
| `TRYIT_PROXY_METRICS_PATH` | HTTP path served by the metrics endpoint | `/metrics` |

The proxy also exposes `GET /healthz`, returns structured JSON errors, and
redacts bearer tokens from log output.

Enable `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` when exposing the proxy to docs users so the Swagger and
RapiDoc panels can forward user-supplied bearer tokens. The proxy still enforces rate limits,
redacts credentials, and records whether a request used the default token or a per-request override.
Set `TRYIT_PROXY_CLIENT_ID` to the label you want sent as `X-TryIt-Client`
(defaults to `docs-portal`). The proxy trims and validates caller-supplied
`X-TryIt-Client` values, falling back to this default so staging gateways can
audit provenance without correlating browser metadata.

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

Before binding the socket the script validates that
`static/openapi/torii.json` matches the digest recorded in
`static/openapi/manifest.json`. If the files drift, the command exits with an
error and instructs you to run `npm run sync-openapi -- --latest`. Export
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` only for emergency overrides; the proxy will
log a warning and continue so you can recover during maintenance windows.

## Wire the portal widgets

When you build or serve the developer portal, set the URL that the widgets
should use for the proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

The following components read these values from `docusaurus.config.js`:

- **Swagger UI** — rendered at `/reference/torii-swagger`; pre-authorises the
  bearer scheme when a token is present, tags requests with `X-TryIt-Client`,
  injects `X-TryIt-Auth`, and rewrites calls through the proxy when
  `TRYIT_PROXY_PUBLIC_URL` is set.
- **RapiDoc** — rendered at `/reference/torii-rapidoc`; mirrors the token field,
  reuses the same headers as the Swagger panel, and targets the proxy
  automatically when the URL is configured.
- **Try it console** — embedded on the API overview page; lets you send custom
  requests, view headers, and inspect response bodies.

Both panels surface a **snapshot selector** that reads
`docs/portal/static/openapi/versions.json`. Populate that index with
`npm run sync-openapi -- --version=<label> --mirror=current --latest` so
reviewers can jump between historical specs, see the recorded SHA-256 digest,
and confirm whether a release snapshot carries a signed manifest before using
the interactive widgets.

Changing the token in any widget only affects the current browser session; the
proxy never persists or logs the supplied token.

## Short-lived OAuth tokens

To avoid distributing long-lived Torii tokens to reviewers, wire the Try it
console to your OAuth server. When the environment variables below are present
the portal renders a device-code login widget, mints short-lived bearer tokens,
and automatically injects them into the console form.

| Variable | Purpose | Default |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth Device Authorization endpoint (`/oauth/device/code`) | _empty (disabled)_ |
| `DOCS_OAUTH_TOKEN_URL` | Token endpoint that accepts `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth client identifier registered for the docs preview | _empty_ |
| `DOCS_OAUTH_SCOPE` | Space-delimited scopes requested during sign-in | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Optional API audience to bind the token to | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Minimum poll interval when waiting for approval (ms) | `5000` (values < 5000 ms are rejected) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Fallback device-code expiration window (seconds) | `600` (must remain between 300 s and 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Fallback access-token lifetime (seconds) | `900` (must remain between 300 s and 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Set to `1` for local previews that intentionally skip OAuth enforcement | _unset_ |

Example configuration:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

When you run `npm run start` or `npm run build`, the portal embeds these values
in `docusaurus.config.js`. During local preview the Try it card shows a
“Sign in with device code” button. Users enter the displayed code on your OAuth
verification page; once the device flow succeeds the widget:

- injects the issued bearer token into the Try it console field,
- tags requests with the existing `X-TryIt-Client` and `X-TryIt-Auth` headers,
- displays the remaining lifetime, and
- automatically clears the token when it expires.

The manual Bearer input remains available—omit the OAuth variables whenever you
want to force reviewers to paste a temporary token themselves, or export
`DOCS_OAUTH_ALLOW_INSECURE=1` for isolated local previews where anonymous access
is acceptable. Builds without OAuth configured now fail fast to satisfy the
DOCS-1b roadmap gate.

📌 Review the [Security hardening & pen-test checklist](./security-hardening.md)
before exposing the portal outside the lab; it documents the threat model,
CSP/Trusted Types profile, and the penetration-test steps that now gate DOCS-1b.

## Norito-RPC samples

Norito-RPC requests share the same proxy and OAuth plumbing as the JSON routes,
they simply set `Content-Type: application/x-norito` and send the
pre-encoded Norito payload described in the NRPC specification
(`docs/source/torii/nrpc_spec.md`).
The repository ships canonical payloads under `fixtures/norito_rpc/` so portal
authors, SDK owners, and reviewers can replay the exact bytes that CI uses.

### Send a Norito payload from the Try It console

1. Pick a fixture such as `fixtures/norito_rpc/transfer_asset.norito`. These
   files are raw Norito envelopes; do **not** base64-encode them.
2. In Swagger or RapiDoc, locate the NRPC endpoint (for example
   `POST /v1/pipeline/submit`) and switch the **Content-Type** selector to
   `application/x-norito`.
3. Toggle the request body editor to **binary** (Swagger's "File" mode or
   RapiDoc's "Binary/File" selector) and upload the `.norito` file. The widget
   streams the bytes through the proxy without alteration.
4. Submit the request. If Torii returns `X-Iroha-Error-Code: schema_mismatch`,
   verify that you are calling an endpoint that accepts binary payloads and
   confirm that the schema hash recorded in `fixtures/norito_rpc/schema_hashes.json`
   matches the Torii build you are hitting.

The console keeps the most recent file in memory so you can resubmit the same
payload while exercising different authorisation tokens or Torii hosts. Adding
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` to your workflow produces
the evidence bundle referenced in the NRPC-4 adoption plan (log + JSON summary),
which pairs nicely with screenshotting the Try It response during reviews.

### CLI example (curl)

The same fixtures can be replayed outside the portal via `curl`, which is useful
when validating the proxy or debugging gateway responses:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v1/pipeline/submit"
```

Swap the fixture for any entry listed in `transaction_fixtures.manifest.json`
or encode your own payload with `cargo xtask norito-rpc-fixtures`. When Torii
is in canary mode you can point `curl` at the try-it proxy
(`https://docs.sora.example/proxy/v1/pipeline/submit`) to exercise the same
infrastructure that the portal widgets use.

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
- `TRYIT_PROXY_PROBE_METRICS_URL` — optional metrics endpoint URL (for example, `http://localhost:9798/metrics`) that must respond successfully when `TRYIT_PROXY_METRICS_LISTEN` is enabled.

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

When `TRYIT_PROXY_METRICS_LISTEN` is configured, set
`TRYIT_PROXY_PROBE_METRICS_URL` to the metrics endpoint so the probe fails fast
if the scrape surface disappears (for example, misconfigured ingress or missing
firewall rules). A typical production setting is
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

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

### Metrics endpoint & dashboards

Set `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (or any host/port pair) before
starting the proxy to expose a Prometheus-formatted metrics endpoint. The path
defaults to `/metrics` but can be overridden via
`TRYIT_PROXY_METRICS_PATH=/custom`. Each scrape returns counters for per-method
request totals, rate-limit rejections, upstream errors/timeouts, proxy outcomes,
and latency summaries:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Point your Prometheus/OTLP collectors at the metrics endpoint and reuse the
existing `dashboards/grafana/docs_portal.json` panels so SRE can observe tail
latencies and rejection spikes without parsing logs. The proxy automatically
publishes `tryit_proxy_start_timestamp_ms` to help operators detect restarts.

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
