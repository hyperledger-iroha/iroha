# Soracloud Vue3 SPA and App Runbook

This runbook covers four production-oriented frontend patterns:

- a static Vue3 site published to SoraFS with `--template site`
- a hosted HTTP API with `--template http-service`
- a root-bound single-service app with `app init --template single-api`
- a mixed split app with `app init --template split-app`
- `app init --template nexus-split-app` is an alias for the same split-app scaffold

The single-api path is the recommended workflow for apps that need:

- a static frontend served from the app root on the public host
- a deterministic IVM API on the same host under `/api`
- one app manifest that publishes the frontend and deploys one service

The split-app path is the recommended workflow for apps that need:

- a static frontend served from SoraFS CID URLs
- a hosted live API on `Inrou`
- a deterministic IVM vault/auth API
- one shared `/api` surface split by authoritative longest-prefix routing

## Access Model

Soracloud deploys must behave like IPFS-style publishing for runtime URLs:

- the registered vanity host stays fixed
- deploys update Soracloud route bindings, not DNS records on every release
- direct vanity-host access remains canonical
- Taira's owned public browser gateway is `mon.taira.sora.net`
- `/soradns/<alias>/...` is the legacy Torii compatibility fallback

Examples:

- direct frontend origin: `https://docs.sora/`
- Taira browser frontend origin: `https://docs.sora.mon.taira.sora.net/`
- legacy fallback frontend origin: `https://taira.sora.org/soradns/docs.sora/`
- direct API origin:
  `https://solswap-indexer.sora/api/indexer/v1/health`
- Taira browser API origin:
  `https://solswap-indexer.sora.mon.taira.sora.net/api/indexer/v1/health`
- legacy fallback API origin:
  `https://taira.sora.org/soradns/solswap-indexer.sora/api/indexer/v1/health`

The Mon gateway is the public browser URL for clients that need normal DNS/TLS
before native SoraDNS resolution is available. The `/soradns/<alias>/...` path
is not the canonical origin for app configs or release notes. Use it only as a
compatibility/debug gateway. Do not invent `https://taira.sora.org/<service>/...`
URLs.

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
iroha app soracloud local-plan --container ./container_manifest.json --service ./service_manifest.json
./build-and-sync.sh
iroha app soracloud build-and-sync --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Hosted HTTP service local development:

```bash
cd .soracloud-live
iroha app soracloud local-plan --container ./container_manifest.json --service ./service_manifest.json
./local-dev.sh
iroha app soracloud local-dev --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Hosted HTTP deploy and upgrade wrappers:

```bash
cd .soracloud-live
TORII_URL=http://127.0.0.1:8080 ./deploy.sh
iroha app soracloud deploy-workspace --container ./container_manifest.json --service ./service_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
iroha app soracloud deploy --container ./container_manifest.json --service ./service_manifest.json --torii-url http://127.0.0.1:8080
TORII_URL=http://127.0.0.1:8080 ./upgrade.sh
iroha app soracloud upgrade-workspace --container ./container_manifest.json --service ./service_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
iroha app soracloud upgrade --container ./container_manifest.json --service ./service_manifest.json --torii-url http://127.0.0.1:8080
iroha app soracloud status --container ./container_manifest.json --service ./service_manifest.json --torii-url http://127.0.0.1:8080
iroha app soracloud config-status --container ./container_manifest.json --service ./service_manifest.json --torii-url http://127.0.0.1:8080
iroha app soracloud secret-status --container ./container_manifest.json --service ./service_manifest.json --torii-url http://127.0.0.1:8080
iroha app soracloud rollback --container ./container_manifest.json --service ./service_manifest.json --torii-url http://127.0.0.1:8080
```

Before treating a hosted-HTTP release as complete, run both backend-specific
smokes and the mixed-host placement gate with the same guest asset class you
plan to publish:

```bash
cargo xtask soracloud-inrou-smoke portable
sudo cargo xtask soracloud-inrou-smoke firecracker
cargo xtask soracloud-inrou-smoke mixed-host --inventory ./fixtures/soracloud/inrou_mixed_host_inventory.example.toml
```

That validation path exercises the real `HttpService + Inrou` runtime, not the
local `dev.sh` shim. `PortableVm` attaches shared lease storage as persistent
block devices in unprivileged userspace, while the Linux/KVM fast path keeps
the Firecracker NFS transport adapter. The mixed gate is expected to cover one
Linux Firecracker host, one non-Linux PortableVm host, and one proxy-only
validator that publishes zero hosted capacity while still proxying routed
hosted-HTTP traffic correctly.

The proxy-only host command in the example inventory runs the focused
`proxy_only_inrou_host` runtime tests instead of a plain compile check, so the
gate proves that proxy-only nodes fail closed for local Inrou materialization.

For a public release, pass the real operator inventory and the operator
observability evidence to the readiness runner:

```bash
scripts/ci/run_soracloud_production_readiness.sh \
  --profile load \
  --mixed-host-inventory ./operator-inrou-inventory.toml \
  --observability-evidence ./operator-soracloud-observability.json
```

`scripts/ci/check_soracloud_observability_evidence.py` validates that the
evidence covers the required Soracloud metrics, status fields, alerts, runbooks,
and dashboards. The sample
`fixtures/soracloud/production_observability_evidence.example.json` documents
the expected shape; production runs must use deployment-specific sources.

Portable smoke uses `IROHA_INROU_PORTABLE_KERNEL_IMAGE`,
`IROHA_INROU_PORTABLE_ROOTFS_IMAGE`, and optional
`IROHA_INROU_PORTABLE_INITRD_IMAGE`, plus optional
`IROHA_INROU_PORTABLE_ACCEL=auto|tcg|kvm|hvf|whpx`. Firecracker smoke uses the
corresponding `IROHA_INROU_LINUX_KVM_*` environment variables.
For local PortableVm validation, prepare verified Debian genericcloud assets
with `eval "$(python3 scripts/ci/prepare_inrou_portable_guest_assets.py --print-env)"`
before running `cargo xtask soracloud-inrou-smoke portable`.

The same `--container` plus `--service` manifest pair also works for other
service-bound Soracloud commands such as `hf-deploy`, `hf-status`, `hf-lease-renew`,
`hf-lease-leave`, `training-job-*`, `model-artifact-*`, `model-weight-*`,
`model-upload-encryption-recipient`, `model-upload-init`,
`model-upload-chunk`, `model-upload-finalize`, `model-upload-status`,
`model-compile`, `model-compile-status`, `model-allow`,
`model-run-private`, `model-run-status`, `model-decrypt-output`, and
`model-publish-private`. For `status`, the
manifest-pair form also keeps the same local route and workspace-script
projection that `local-plan` reports. The direct `deploy` and `upgrade`
commands now keep that same local projection in their response as well, and
manifest-pair `config-*`, `secret-*`, `rollback`, `rollout`, HF, training-job,
and model registry/status responses attach it under `service_plan`.

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
iroha app soracloud local-plan --container ./container_manifest.json --service ./service_manifest.json
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
tracks `services/api/build/api-service.to`. The local-plan output also reports
the root `manifest_path`, root `hostname`, and the manifest-adjacent root
scripts that the generated workspace and CLI wrappers use for `local-dev`,
`build-and-sync`, `deploy`, and `upgrade`.

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
from the app manifest as part of the app-wide deploy flow, and now returns the
same root manifest/hostname/workspace metadata, frontend publish projection,
and per-service manifest metadata that `app status` reports.

For split apps in production on Taira, keep `https://taira.sora.org/` bound to
Torii and use it as:

- the Torii/control-plane base URL
- the legacy SoraDNS compatibility gateway:
  `https://taira.sora.org/soradns/<alias>/...`
- the SoraFS CID gateway for intentionally CID-only frontends:
  `https://taira.sora.org/sorafs/cid/<cid>/`

Use `https://<alias>.mon.taira.sora.net/...` as the Taira public browser URL
for Soracloud apps that already have a vanity alias host. Do not treat
`taira.sora.org` paths as canonical app origins.

## 5. Deploy

Single hosted HTTP service:

```bash
cd .soracloud-live
TORII_URL=http://127.0.0.1:8080 ./deploy.sh
iroha app soracloud deploy-workspace --container ./container_manifest.json --service ./service_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
```

Single-api app:

```bash
cd .soracloud-docs-portal
TORII_URL=http://127.0.0.1:8080 ./deploy.sh
iroha app soracloud app deploy-workspace --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
```

Split app:

```bash
cd .soracloud-hayahi
./doctor.sh
TORII_URL=http://127.0.0.1:8080 ./release.sh
TORII_URL=http://127.0.0.1:8080 ./deploy.sh
iroha app soracloud app doctor --manifest ./app_manifest.json
iroha app soracloud app doctor-workspace --manifest ./app_manifest.json --dry-run
iroha app soracloud app doctor-workspace --manifest ./app_manifest.json
iroha app soracloud app release --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
iroha app soracloud app release --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080
iroha app soracloud app release-workspace --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
iroha app soracloud app release-workspace --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080
```

`app doctor-workspace` and `app release-workspace` resolve and run the same
root `doctor.sh` and `release.sh` scripts adjacent to `app_manifest.json`, so
the split-app happy path stays on the manifest-driven CLI surface.

Those generated root scripts resolve `IROHA_CLI_BIN`, `IROHA_BIN`,
`IROHA_CARGO_TARGET_DIR/.../iroha`, `CARGO_TARGET_DIR/.../iroha`,
`IROHA_MANIFEST_PATH`, and finally `PATH` `iroha`, so local app workspaces
can target a nearby `iroha_cli` checkout without requiring a globally
installed wrapper. When you drive the fallback through `IROHA_MANIFEST_PATH`,
set `IROHA_CARGO_HOME` and `IROHA_CARGO_TARGET_DIR` to keep Cargo package and
artifact state isolated from other local builds.

In local dev, the scaffolded Vite proxy strips the shared `/api` prefix before
forwarding to the live and vault child processes so the dev loop matches the
same hosted-route semantics Torii uses in production.

The app deploy flow:

- single-api: republishes the static frontend from `web/dist` and deploys
  `services/api`
- split-app: republishes the static frontend from `frontend/dist`
- split-app: returns the published `cid_gateway_url` for CID-only frontends
- split-app: deploys the hosted `services/live` API
- split-app: deploys the deterministic `services/vault` API
- split-app: the recommended scaffolded path is `./doctor.sh` followed by `./release.sh`
- split-app: the equivalent manifest-driven CLI path is `app doctor-workspace`
  followed by `app release-workspace`
- both modes: return the root app `manifest_path`, root `workspace_dir`, root
  `workspace_scripts`, root `hostname`, the top-level app `routes` split, and
  one manifest-derived service entry per app service

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

`app status` keeps one entry per service declared in the app manifest and
reports each child `container_manifest_path`, `service_manifest_path`, the root
`manifest_path`, root `hostname`, root `workspace_dir`, root
`workspace_scripts`, plane/runtime, route prefix, the top-level app `routes`
split, the frontend publish projection, and the matched Torii status when
present.

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
- `iroha app soracloud app local-plan` reports the same route split from the app manifest,
  plus each child service `container_manifest_path`, `service_manifest_path`,
  `workspace_dir`, and discovered child scripts such as `dev.sh`, `build.sh`,
  and `verify-build.sh`
- `iroha app soracloud app status` keeps both manifest services visible even if
  only one of them currently appears in the Torii control-plane payload
- `iroha app soracloud app status` also reports the expected frontend
  `/sorafs/cid/<cid>/` URL template for CID-only apps or the root-binding URL
  for root-bound frontend apps
- the live service reports lease-backed volume mounts
- the frontend loads from a SoraFS CID URL and still targets `/api`
- `iroha app soracloud app local-dev --manifest ... --dry-run` resolves the
  same mixed-app local entrypoint before execution

## 7. Upgrade and Redeploy

After rebuilding artifacts, rerun:

```bash
cd .soracloud-live
TORII_URL=http://127.0.0.1:8080 ./upgrade.sh
iroha app soracloud upgrade-workspace --container ./container_manifest.json --service ./service_manifest.json --torii-url http://127.0.0.1:8080 --dry-run

cd .soracloud-docs-portal
TORII_URL=http://127.0.0.1:8080 ./upgrade.sh
iroha app soracloud app upgrade-workspace --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run

cd .soracloud-hayahi
TORII_URL=http://127.0.0.1:8080 ./upgrade.sh
iroha app soracloud app upgrade-workspace --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
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
- Use `iroha app soracloud local-dev --container ... --service ... --dry-run`
  or `iroha app soracloud build-and-sync --container ... --service ... --dry-run`
  when you want the CLI to resolve the generated hosted-service root scripts
  from the manifest pair first and return the same route and workspace-script
  projection that `local-plan` reports.
- Use `iroha app soracloud deploy-workspace --container ... --service ... --torii-url ... --dry-run`
  or `iroha app soracloud upgrade-workspace --container ... --service ... --torii-url ... --dry-run`
  when you want the CLI to resolve the hosted-service deploy or upgrade entrypoint
  before executing the generated root script, while keeping the same service
  plan metadata in the dry-run output.
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
  want the CLI to resolve the generated root scripts from the app manifest first;
  those dry-run outputs also carry the same child service and route plan that
  `app local-plan` reports.
- Use `iroha app soracloud app deploy-workspace --manifest ... --torii-url ... --dry-run`
  or `iroha app soracloud app upgrade-workspace --manifest ... --torii-url ... --dry-run`
  for the same manifest-resolved child service and route projection before the
  root deploy or upgrade script runs.
  when you want the CLI to resolve the generated deploy or upgrade entrypoint
  before the workspace rebuilds, verifies the vault bytecode, and refreshes
  app-wide manifest hashes.
- Fail frontend production builds if they point at demo/static data or the
  wrong API base.
- Treat `taira.sora.org` as Torii/control-plane first and as the legacy
  `/soradns/<alias>/...` compatibility gateway second.
- Use `https://<alias>.mon.taira.sora.net/...` for Taira browser examples that
  need ordinary public DNS/TLS.
- Keep the canonical runtime origin on the registered vanity alias host.
- Use SoraFS CID paths only for apps that intentionally publish CID-only
  frontend assets.
