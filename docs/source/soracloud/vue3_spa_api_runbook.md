# Soracloud Vue3 SPA and App Runbook

This runbook covers four production-oriented frontend patterns:

- a static Vue3 site published to SoraFS with `--template site`
- a hosted HTTP API with `--template http-service`
- a root-bound single-service app with `app init --template single-api`
- a mixed split app with `app init --template split-app`

The single-api path is the recommended workflow for apps that need:

- a static frontend served from the app root on the public host
- a deterministic IVM API on the same host under `/api`
- one app manifest that publishes the frontend and deploys one service

The split-app path is the recommended workflow for apps that need:

- a static frontend served from SoraFS CID URLs
- a hosted live API on `Inrou`
- a deterministic IVM vault/auth API
- one shared `/api` surface split by authoritative longest-prefix routing

## 1. Generate the Scaffold

Static site only:

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

Hosted HTTP service only:

```bash
iroha app soracloud init \
  --template http-service \
  --service-name live_search \
  --service-version 1.0.0 \
  --output-dir .soracloud-live
```

Root-bound single-api app:

```bash
iroha app soracloud app init \
  --template single-api \
  --app-name docs_portal \
  --app-version 1.0.0 \
  --output-dir .soracloud-docs-portal
```

Mixed split app:

```bash
iroha app soracloud app init \
  --template split-app \
  --app-name hayahi \
  --app-version 1.0.0 \
  --output-dir .soracloud-hayahi
```

The split-app scaffold produces:

- `app_manifest.json`
- `frontend/`
- `services/live/`
- `services/vault/`
- `local-dev.sh`
- `build-and-sync.sh`
- `deploy.sh`
- `upgrade.sh`
- `services/live/dev.sh`
- `services/vault/dev.sh`
- `services/vault/verify-build.sh`

The single-api scaffold produces:

- `app_manifest.json`
- `web/`
- `services/api/`
- `local-dev.sh`
- `build-and-sync.sh`
- `deploy.sh`
- `upgrade.sh`
- `services/api/dev.sh`

## 2. Build Artifacts

Static site:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

Hosted HTTP service:

```bash
cd .soracloud-live
./build-and-sync.sh
iroha app soracloud build-and-sync --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Hosted HTTP service local development:

```bash
cd .soracloud-live
./local-dev.sh
iroha app soracloud local-dev --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Root-bound single-api app:

```bash
cd .soracloud-docs-portal
./build-and-sync.sh
iroha app soracloud app build-and-sync --manifest ./app_manifest.json --dry-run
```

Root-bound single-api local development:

```bash
cd .soracloud-docs-portal
./local-dev.sh
iroha app soracloud app local-dev --manifest ./app_manifest.json --dry-run
```

Split app:

```bash
cd .soracloud-hayahi
./build-and-sync.sh
iroha app soracloud app build-and-sync --manifest ./app_manifest.json --dry-run
```

Split app local development:

```bash
cd .soracloud-hayahi
./local-dev.sh
iroha app soracloud app local-dev --manifest ./app_manifest.json --dry-run
```

Expected outputs:

- static site: `site/dist/`
- hosted HTTP service: `http-service/build/http-service.tgz`
- single-api frontend: `web/dist/`
- single-api API: `services/api/build/api-service.to`
- single-api API manifest: `services/api/build/api-service.contract_manifest.json`
- split app frontend: `frontend/dist/`
- split app live API: `services/live/build/live-api.tgz`
- split app vault API: `services/vault/build/vault-api.to`

## 3. Recompute Manifest Hashes

Single hosted service:

```bash
cd .soracloud-live
./build-and-sync.sh
iroha app soracloud build-and-sync --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Whole single-api app:

```bash
cd .soracloud-docs-portal
./build-and-sync.sh
iroha app soracloud app build-and-sync --manifest ./app_manifest.json --dry-run
```

Whole split app:

```bash
cd .soracloud-hayahi
./build-and-sync.sh
iroha app soracloud app build-and-sync --manifest ./app_manifest.json --dry-run
```

Inspect the split app locally before deploy:

```bash
iroha app soracloud app local-plan \
  --manifest .soracloud-hayahi/app_manifest.json
```

App-wide sync is the recommended path for mixed deployments because each
service reference can declare its own `bundle_file`, letting one command
refresh every `bundle_hash` and referenced container manifest hash before
deployment. The same path also works for single-api apps, where the manifest
tracks `services/api/build/api-service.to`.

## 4. Publish Frontend Assets

For static site packaging through SoraFS:

```bash
iroha app sorafs toolkit pack .soracloud-docs/site/dist \
  --manifest-out .soracloud-docs/sorafs/site_manifest.to \
  --car-out .soracloud-docs/sorafs/site_payload.car \
  --json-out .soracloud-docs/sorafs/site_pack_report.json
```

For a split app you normally do not need a separate manual packaging step.

For a single-api app you also normally do not need a separate manual packaging
step because `app deploy` publishes `web/dist` directly from the app manifest.
`iroha app soracloud app deploy` publishes the declared `static_site.dist_dir`
from the app manifest as part of the app-wide deploy flow.

For split apps in production on Taira, keep `https://taira.sora.org/` bound to
Torii and use the SoraFS gateway path for frontend access:

- `https://taira.sora.org/sorafs/cid/<cid>/`

## 5. Deploy

Single hosted HTTP service:

```bash
cd .soracloud-live
TORII_URL=http://127.0.0.1:8080 ./deploy.sh
```

Single-api app:

```bash
cd .soracloud-docs-portal
TORII_URL=http://127.0.0.1:8080 ./deploy.sh
```

Split app:

```bash
cd .soracloud-hayahi
TORII_URL=http://127.0.0.1:8080 ./deploy.sh
```

The app deploy flow:

- single-api: republishes the static frontend from `web/dist` and deploys
  `services/api`
- split-app: republishes the static frontend from `frontend/dist`
- split-app: returns the published `cid_gateway_url` for CID-only frontends
- split-app: deploys the hosted `services/live` API
- split-app: deploys the deterministic `services/vault` API

## 6. Validate Routing and Runtime State

Check service state:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Check the app-scoped projection for a single-api app:

```bash
iroha app soracloud app status \
  --manifest .soracloud-docs-portal/app_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Check the app-scoped projection for a split app:

```bash
iroha app soracloud app status \
  --manifest .soracloud-hayahi/app_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Expected checks for a single-api app:

- `/` serves the root-bound frontend from the published static site
- `/api/healthz` resolves to the deterministic API service
- `./local-dev.sh` serves the frontend locally with `/api` proxied to the local
  API shim
- `iroha app soracloud app local-dev --manifest ... --dry-run` resolves the same
  manifest-adjacent entrypoint from the app manifest
- the frontend uses same-host `/api` calls instead of a hard-coded external base

Expected checks for a split app:

- `/api/v1/search*` resolves to the hosted live API
- `/api/auth*` resolves to the deterministic vault API
- `/api/v1/user*` resolves to the deterministic vault API
- `iroha app soracloud app local-plan` reports the same route split from the app manifest
- the live service reports lease-backed volume mounts
- the frontend loads from a SoraFS CID URL and still targets `/api`
- `iroha app soracloud app local-dev --manifest ... --dry-run` resolves the
  same mixed-app local entrypoint before execution

## 7. Upgrade and Redeploy

After rebuilding artifacts, rerun:

```bash
cd .soracloud-live
TORII_URL=http://127.0.0.1:8080 ./upgrade.sh

cd .soracloud-docs-portal
TORII_URL=http://127.0.0.1:8080 ./upgrade.sh

cd .soracloud-hayahi
TORII_URL=http://127.0.0.1:8080 ./upgrade.sh
```

Each generated `upgrade.sh` reruns `./build-and-sync.sh` first, then submits
the matching hosted-service or app-wide upgrade command.

## 8. Operations Checklist

- Keep generated manifests under version control.
- Use `single-api` when one deterministic `/api` service is enough and the
  frontend should stay bound at `/` on the same host.
- Use `./local-dev.sh` for root-bound single-api iteration, which keeps `/api`
  same-host and proxies it to the local deterministic API shim.
- Use `iroha app soracloud app build-and-sync --manifest ... --dry-run` when
  you want the CLI to resolve the generated root rebuild path from the app manifest.
- Use `./upgrade.sh` after validating a new single-api build when you want the
  scaffolded app-wide upgrade path instead of raw CLI calls.
- Use `http-service` for hosted collectors, SSE, and shared caches.
- Keep wallet auth and confidential user state on the IVM plane.
- Persist hosted mutable state through declared `lease_volumes`, not ad hoc
  local directories.
- Use `./local-dev.sh` for local mixed-app iteration, which proxies `/api/v1/*`
  to the live service and `/api/auth*` plus `/api/v1/user*` to the vault dev
  shim while keeping `VITE_PUBLIC_API_BASE=/api`.
- Use `./build-and-sync.sh` before deploy or upgrade to rebuild the frontend,
  or call `./deploy.sh` or `./upgrade.sh` directly to let the scaffold rerun
  the rebuild and manifest sync path for you.
- Use `iroha app soracloud app local-dev --manifest ... --dry-run` or
  `iroha app soracloud app build-and-sync --manifest ... --dry-run` when you
  want the CLI to resolve the generated root scripts from the app manifest first.
  rebuild and verify the vault bytecode, and refresh app-wide manifest hashes.
- Fail frontend production builds if they point at demo/static data or the
  wrong API base.
- Treat Torii root as Torii-only; publish user-facing frontend assets to SoraFS
  CID paths instead.
