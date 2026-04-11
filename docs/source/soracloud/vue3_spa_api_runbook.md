# Soracloud Vue3 SPA and Split-App Runbook

This runbook covers three production-oriented frontend patterns:

- a static Vue3 site published to SoraFS with `--template site`
- a hosted HTTP API with `--template http-service`
- a mixed split app with `app init --template split-app`

The split-app path is the recommended production workflow for apps that need:

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

## 2. Build Artifacts

Static site:

```bash
cd .soracloud-docs/site
npm install
npm run build
```

Hosted HTTP service:

```bash
cd .soracloud-live/http-service
./build.sh
```

Split app:

```bash
cd .soracloud-hayahi/frontend
npm install
npm run build

cd ../services/live
./build.sh

cd ../vault
./build.sh

cd ../..
```

Expected outputs:

- static site: `site/dist/`
- hosted HTTP service: `http-service/build/http-service.tgz`
- split app frontend: `frontend/dist/`
- split app live API: `services/live/build/live-api.tgz`
- split app vault API: `services/vault/build/vault-api.to`

## 3. Recompute Manifest Hashes

Single hosted service:

```bash
iroha app soracloud sync-manifests \
  --container .soracloud-live/container_manifest.json \
  --service .soracloud-live/service_manifest.json \
  --bundle-file .soracloud-live/http-service/build/http-service.tgz
```

Whole split app:

```bash
iroha app soracloud sync-manifests \
  --app-manifest .soracloud-hayahi/app_manifest.json
```

App-wide sync is the recommended path for mixed deployments because each
service reference can declare its own `bundle_file`, letting one command
refresh every `bundle_hash` and referenced container manifest hash before
deployment.

## 4. Publish Frontend Assets

For static site packaging through SoraFS:

```bash
iroha app sorafs toolkit pack .soracloud-docs/site/dist \
  --manifest-out .soracloud-docs/sorafs/site_manifest.to \
  --car-out .soracloud-docs/sorafs/site_payload.car \
  --json-out .soracloud-docs/sorafs/site_pack_report.json
```

For a split app you normally do not need a separate manual packaging step.
`iroha app soracloud app deploy` publishes the declared `static_site.dist_dir`
from the app manifest as part of the app-wide deploy flow.

In production on Taira, keep `https://taira.sora.org/` bound to Torii and use
the SoraFS gateway path for frontend access:

- `https://taira.sora.org/sorafs/cid/<cid>/`

## 5. Deploy

Single hosted HTTP service:

```bash
iroha app soracloud deploy \
  --container .soracloud-live/container_manifest.json \
  --service .soracloud-live/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Split app:

```bash
iroha app soracloud app deploy \
  --manifest .soracloud-hayahi/app_manifest.json \
  --torii-url http://127.0.0.1:8080
```

The app deploy flow:

- republishes the static frontend from `frontend/dist`
- updates the app static-site binding
- deploys the hosted `services/live` API
- deploys the deterministic `services/vault` API

## 6. Validate Routing and Runtime State

Check service state:

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Check the app-scoped projection:

```bash
iroha app soracloud app status \
  --manifest .soracloud-hayahi/app_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Expected checks for a split app:

- `/api/v1/search*` resolves to the hosted live API
- `/api/auth*` resolves to the deterministic vault API
- `/api/v1/user*` resolves to the deterministic vault API
- the live service reports lease-backed volume mounts
- the frontend loads from a SoraFS CID URL and still targets `/api`

## 7. Upgrade and Redeploy

After rebuilding artifacts, rerun:

```bash
iroha app soracloud sync-manifests \
  --app-manifest .soracloud-hayahi/app_manifest.json

iroha app soracloud app upgrade \
  --manifest .soracloud-hayahi/app_manifest.json \
  --torii-url http://127.0.0.1:8080
```

That keeps the frontend publication plus both services on one documented
workflow, without SSH-only steps or manual CID pinning.

## 8. Operations Checklist

- Keep generated manifests under version control.
- Use `http-service` for hosted collectors, SSE, and shared caches.
- Keep wallet auth and confidential user state on the IVM plane.
- Persist hosted mutable state through declared `lease_volumes`, not ad hoc
  local directories.
- Fail frontend production builds if they point at demo/static data or the
  wrong API base.
- Treat Torii root as Torii-only; publish user-facing frontend assets to SoraFS
  CID paths instead.
