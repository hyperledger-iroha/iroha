---
title: Norito Try-It Console
description: Use the developer-portal proxy, Swagger, and RapiDoc widgets to send real Torii / Norito-RPC requests directly from the documentation site.
---

The portal bundles three interactive surfaces that relay traffic to Torii:

- **Swagger UI** at `/reference/torii-swagger` renders the signed OpenAPI spec and automatically rewrites requests through the proxy when `TRYIT_PROXY_PUBLIC_URL` is set.
- **RapiDoc** at `/reference/torii-rapidoc` exposes the same schema with file uploads and content-type selectors that work well for `application/x-norito`.
- **Try it sandbox** on the Norito overview page provides a lightweight form for ad-hoc REST requests and OAuth-device logins.

All three widgets send requests to the local **Try-It proxy** (`docs/portal/scripts/tryit-proxy.mjs`). The proxy verifies that `static/openapi/torii.json` matches the signed digest in `static/openapi/manifest.json`, enforces a rate limiter, redacts `X-TryIt-Auth` headers in logs, and tags every upstream call with `X-TryIt-Client` so Torii operators can audit traffic sources.

## Launch the proxy

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` is the Torii base URL you want to exercise.
- `TRYIT_PROXY_ALLOWED_ORIGINS` must include every portal origin (local dev server, production hostname, preview URL) that should embed the console.
- `TRYIT_PROXY_PUBLIC_URL` is consumed by `docusaurus.config.js` and injected into the widgets via `customFields.tryIt`.
- `TRYIT_PROXY_BEARER` only loads when `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`; otherwise users must supply their own token via the console or OAuth device flow.
- `TRYIT_PROXY_CLIENT_ID` sets the `X-TryIt-Client` tag carried on every request.
  Supplying `X-TryIt-Client` from the browser is allowed but values are trimmed
  and rejected if they contain control characters.

On startup the proxy runs `verifySpecDigest` and exits with a remediation hint if the manifest is stale. Run `npm run sync-openapi -- --latest` to download the newest Torii specification or pass `TRYIT_PROXY_ALLOW_STALE_SPEC=1` for emergency overrides.

To update or roll back the proxy target without editing environment files by hand, use the helper:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Wire the widgets

Serve the portal after the proxy is listening:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` exposes the following knobs:

| Variable | Purpose |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL injected into Swagger, RapiDoc, and the Try it sandbox. Leave unset to hide the widgets during unauthorised previews. |
| `TRYIT_PROXY_DEFAULT_BEARER` | Optional default token stored in memory. Requires `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` and the HTTPS-only CSP guard (DOCS-1b) unless you pass `DOCS_SECURITY_ALLOW_INSECURE=1` locally. |
| `DOCS_OAUTH_*` | Enable the OAuth device flow (`OAuthDeviceLogin` component) so reviewers can mint short-lived tokens without leaving the portal. |

When the OAuth variables are present the sandbox renders a **Sign in with device code** button that walks through the configured Auth server (see `config/security-helpers.js` for the exact shape). Tokens issued through the device flow are only cached in the browser session.

## Sending Norito-RPC payloads

1. Build a `.norito` payload with the CLI or snippets described in the [Norito quickstart](./quickstart.md). The proxy forwards `application/x-norito` bodies unchanged, so you can reuse the same artefact you would post with `curl`.
2. Open `/reference/torii-rapidoc` (preferred for binary payloads) or `/reference/torii-swagger`.
3. Select the desired Torii snapshot from the drop-down. Snapshots are signed; the panel shows the manifest digest recorded in `static/openapi/manifest.json`.
4. Choose the `application/x-norito` content type in the “Try it” drawer, click **Choose File**, and select your payload. The proxy rewrites the request to `/proxy/v1/pipeline/submit` and tags it with `X-TryIt-Client=docs-portal-rapidoc`.
5. To download Norito responses, set `Accept: application/x-norito`. Swagger/RapiDoc expose the header selector in the same drawer and stream the binary back through the proxy.

For JSON-only routes the embedded Try it sandbox is often faster: enter the path (for example, `/v1/accounts/ih58.../assets`), select the HTTP method, paste a JSON body when needed, and hit **Send request** to inspect headers, duration, and payloads inline.

## Troubleshooting

| Symptom | Likely cause | Remediation |
| --- | --- | --- |
| Browser console shows CORS errors or the sandbox warns that the proxy URL is missing. | Proxy is not running or the origin is not whitelisted. | Start the proxy, make sure `TRYIT_PROXY_ALLOWED_ORIGINS` covers your portal host, and relaunch `npm run start`. |
| `npm run tryit-proxy` exits with “digest mismatch”. | The Torii OpenAPI bundle changed upstream. | Run `npm run sync-openapi -- --latest` (or `--version=<tag>`) and retry. |
| Widgets return `401` or `403`. | Token missing, expired, or insufficient scopes. | Use the OAuth device flow or paste a valid bearer token into the sandbox. For static tokens you must export `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`. |
| `429 Too Many Requests` from the proxy. | Per-IP rate limit exceeded. | Raise `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` for trusted environments or throttle test scripts. All rate-limit rejections increment `tryit_proxy_rate_limited_total`. |

## Observability

- `npm run probe:tryit-proxy` (wrapper around `scripts/tryit-proxy-probe.mjs`) calls `/healthz`, optionally exercises a sample route, and emits Prometheus textfiles for `probe_success` / `probe_duration_seconds`. Configure `TRYIT_PROXY_PROBE_METRICS_FILE` to integrate with node_exporter.
- Set `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` to expose counters (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) and latency histograms. The `dashboards/grafana/docs_portal.json` board reads these metrics to enforce DOCS-SORA SLOs.
- Runtime logs live on stdout. Every entry includes the request id, upstream status, authentication source (`default`, `override`, or `client`), and duration; secrets are redacted before emission.

If you need to validate that `application/x-norito` payloads reach Torii unchanged, run the Jest suite (`npm test -- tryit-proxy`) or inspect the fixtures under `docs/portal/scripts/__tests__/tryit-proxy.test.mjs`. The regression tests cover compressed Norito binaries, signed OpenAPI manifests, and proxy downgrade paths so NRPC rollouts keep a permanent evidence trail.
