---
lang: es
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/api/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ad6308564afe27bc7e483ff2356cacf3811f044c8938bed04cdeb507fd8042a1
source_last_modified: "2026-01-30T17:50:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/api/overview.mdx
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 293cedf8e24e3e711da383ae5152786614d1ea294b9232988dcf57784cf1a30d
source_last_modified: "2025-12-27T17:59:34+00:00"
translation_last_reviewed: 2026-01-30
---

import TryItConsole from '@site/src/components/TryItConsole.jsx';

# Torii API Overview

The Torii gateway exposes a REST + WebSocket surface for interacting with Iroha
nodes. The canonical OpenAPI document is generated via:

```bash
cargo xtask openapi
```

Running the command above writes `docs/portal/static/openapi/torii.json` and
feeds the `docusaurus-plugin-openapi-docs` integration so this section updates
automatically. The generator spins up an in-memory Torii router and attempts to
query its OpenAPI endpoint; if that fails (for example, router initialization
errors) a deterministic placeholder spec is emitted instead.

## Current status

- **Spec source:** `cargo xtask openapi` queries the Torii router directly; the
  fallback stub is only emitted when the router fails to expose an OpenAPI
  document.
- **Determinism:** JSON output is canonicalised to ensure stable diffing in CI.
- **Authentication:** The “Try it” sandbox now forwards requests through a
  constrained staging proxy. Operators must supply a Torii endpoint and bearer
  token before issuing calls.
- **Version selector:** Swagger UI, RapiDoc, and the full Torii OpenAPI page
  expose a dropdown that reads `openapi/versions.json` so you can compare
  historical snapshots. The `Latest (tracked)` option points to
  `/openapi/torii.json`, while all other entries load the corresponding
  `openapi/versions/<version>/torii.json` artifact.
- **Snapshot workflow:** After running `npm run docs:version -- <label>`, refresh
  the matching OpenAPI file with `npm run sync-openapi -- --version=<label> --latest`
  so the dropdown can load both the frozen snapshot and the rolling “Latest” build.

The router-backed extractor now emits the full Torii surface. If you see the
placeholder stub, treat it as a build-time failure and inspect the Torii
router startup logs.

## Try it sandbox (DOCS-3)

The developer portal embeds a small console that relays REST requests through a
Node-based proxy. The proxy enforces CORS, rate limits, and optional bearer
tokens so the widget can talk to staging gateways without exposing credentials.
See `docs/devportal/try-it.md` for the full operator checklist.

Start the proxy alongside the Docusaurus dev server:

```bash
cd docs/portal
# Proxy configuration
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_BEARER="sora-dev-token"
# Front-end configuration so the widget knows where the proxy lives
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"

# Run the proxy and the docs site (parallel terminals recommended)
npm run tryit-proxy
npm run start
```

- The proxy listens on `TRYIT_PROXY_LISTEN` (default `127.0.0.1:8787`) and
  forwards any `/proxy/...` requests to `TRYIT_PROXY_TARGET`.
- Supply `TRYIT_PROXY_BEARER` for static bearer tokens or let callers provide
  one per request via the “Bearer token” field.
- Rate limits default to 60 requests per minute and can be tuned through
  `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS`.

> ⚠️ The proxy is intended for staging/demo access. Production deployments
> should run behind hardened ingress and reuse the same rate limiting policy in
> front of Torii.

<TryItConsole />
