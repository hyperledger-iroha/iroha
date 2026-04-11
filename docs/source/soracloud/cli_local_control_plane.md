# Soracloud CLI and Control Plane

Soracloud v1 is an authoritative mixed-plane runtime:

- deterministic services run on `SoraContainerRuntimeV1::Ivm`
- hosted HTTP services run on `execution_plane = HttpService` with
  `runtime = Inrou`
- `NativeProcess` remains a compatibility path, not the recommended production
  target

The control plane remains authoritative. Torii serves status and mutation
routes directly from committed world state plus the embedded Soracloud runtime
manager; the CLI does not keep a shadow control-plane mirror.

## Runtime Scope

- Use `HttpService + Inrou` for collector-heavy, SSE, cache-backed, or
  browser-assisted workloads that need a hosted HTTP plane.
- Use deterministic IVM services for wallet auth, confidential vault state,
  governance-sensitive mutations, and other replay-safe handlers.
- Public routing is longest-prefix authoritative, so one host can safely split
  `/api/v1/search*` to a hosted service while `/api/auth*` and
  `/api/v1/user*` stay on an IVM service.
- Hosted services declare persistent lease-backed storage through
  `lease_volumes`. At runtime, each mounted volume is exposed as:
  - `SORACLOUD_LEASE_VOLUME_<NAME>_DIR`
  - `SORACLOUD_LEASE_VOLUME_<NAME>_MOUNT_PATH`
- Config and secret materialization is still authoritative. Deploy, upgrade,
  and rollback fail closed when required config/secret bindings are missing or
  inconsistent with the active manifests.
- The local control plane can materialize both planes together, so mixed apps
  can be run locally as one deterministic IVM service plus one hosted Inrou
  service.

## Offline Scaffolding Commands

- `iroha app soracloud init`
  - scaffolds one service pair:
    - `container_manifest.json`
    - `service_manifest.json`
  - templates now include:
    - `baseline`
    - `http-service`
    - `site`
    - `webapp`
    - `pii-app`
    - `hayahi-app`
- `iroha app soracloud app init`
  - scaffolds an app manifest plus one or more service pairs
  - templates now include:
    - `single-api`
    - `split-app`
- `iroha app soracloud sync-manifests`
  - recomputes `container.bundle_hash`, the service-side referenced container
    hash, and matching schema versions after local edits
  - supports:
    - one service pair via `--container`, `--service`, and optional
      `--bundle-file`
    - every service in an app manifest via `--app-manifest`

## Network-Backed Commands

All deploy, upgrade, rollout, status, config, secret, and app mutation
commands are Torii-backed and require `--torii-url`.

- `iroha app soracloud deploy`
  - validates a single `SoraDeploymentBundleV1` locally and submits it to
    `POST /v1/soracloud/deploy`
- `iroha app soracloud upgrade`
  - validates and submits one upgraded bundle to
    `POST /v1/soracloud/upgrade`
- `iroha app soracloud app deploy`
  - loads `app_manifest.json`
  - synchronizes every referenced service pair before submission
  - publishes the declared static site from `static_site.dist_dir`
  - deploys every referenced service in one pass
- `iroha app soracloud app upgrade`
  - follows the same app-wide flow, but uses upgrade semantics
- `iroha app soracloud status`
  - queries authoritative service state from `GET /v1/soracloud/status`
- `iroha app soracloud app status`
  - scopes status output to the services declared in one app manifest

## Hosted Service Lease Volumes

Hosted HTTP services can persist mutable shared state without inventing their
own local directory contract.

Example manifest shape:

```json
{
  "lease_volumes": [
    { "volume_name": "shared_cache", "mount_path": "/var/lib/soracloud/shared-cache" },
    { "volume_name": "search_sessions", "mount_path": "/var/lib/soracloud/search-sessions" },
    { "volume_name": "collector_state", "mount_path": "/var/lib/soracloud/collector-state" }
  ]
}
```

At runtime the service receives, for each declared volume:

- `SORACLOUD_LEASE_VOLUME_SHARED_CACHE_DIR`
- `SORACLOUD_LEASE_VOLUME_SHARED_CACHE_MOUNT_PATH`
- `SORACLOUD_LEASE_VOLUME_SEARCH_SESSIONS_DIR`
- `SORACLOUD_LEASE_VOLUME_SEARCH_SESSIONS_MOUNT_PATH`
- `SORACLOUD_LEASE_VOLUME_COLLECTOR_STATE_DIR`
- `SORACLOUD_LEASE_VOLUME_COLLECTOR_STATE_MOUNT_PATH`

The `_DIR` value is the materialized runtime path. The `_MOUNT_PATH` value is
the logical mount path declared in the manifest.

## Recommended Mixed-App Workflow

Use the split-app scaffold for apps that need:

- a static frontend published to SoraFS
- a hosted live API on Inrou
- a deterministic IVM vault or auth plane

```bash
iroha app soracloud app init \
  --template split-app \
  --app-name hayahi \
  --app-version 1.0.0 \
  --output-dir .soracloud-hayahi
```

Build the three app surfaces:

```bash
cd .soracloud-hayahi/frontend
npm install
npm run build

cd ../services/live
./build.sh

cd ../vault
./build.sh
```

Refresh every manifest hash in one pass:

```bash
iroha app soracloud sync-manifests \
  --app-manifest .soracloud-hayahi/app_manifest.json
```

Deploy the static site plus every service without SSH or manual pinning:

```bash
iroha app soracloud app deploy \
  --manifest .soracloud-hayahi/app_manifest.json \
  --torii-url http://127.0.0.1:8080
```

For Taira-style deployments, keep Torii root bound to Torii itself and publish
the frontend only through SoraFS CID URLs under
`https://taira.sora.org/sorafs/cid/...`.
