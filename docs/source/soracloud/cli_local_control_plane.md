# Soracloud CLI and Control Plane

Soracloud v1 is an authoritative mixed-plane runtime:

- deterministic services run on `SoraContainerRuntimeV1::Ivm`
- hosted HTTP services run on `execution_plane = HttpService` with
  `runtime = Inrou`

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
  `lease_volumes`.
- `PersistentRootLeaseVolume` is required for `HttpService + Inrou`, must mount
  at `/`, and is materialized per replica.
- Non-root `ServiceLeaseVolume` and `ConfidentialLeaseVolume` attachments are
  shared across replicas of the same service revision by default.
- At runtime, each mounted volume is exposed as:
  - `SORACLOUD_LEASE_VOLUME_<NAME>_DIR`
  - `SORACLOUD_LEASE_VOLUME_<NAME>_MOUNT_PATH`
- Config and secret materialization is still authoritative. Deploy, upgrade,
  and rollback fail closed when required config/secret bindings are missing or
  inconsistent with the active manifests.
- The current local workflow validates both planes together with
  `iroha app soracloud app local-plan`, resolves manifest-adjacent workspace
  scripts with `iroha app soracloud app local-dev` and
  `iroha app soracloud app build-and-sync`, and verifies deterministic vault
  builds with `verify-build.sh`. This is a dev shim for the deterministic
  plane, not a full embedded IVM runtime.

## Vanity Alias Access Modes

- Soracloud deploys keep the registered vanity host stable. Releasing a new
  revision updates Soracloud route state, not per-release DNS records or
  per-service nginx host lists.
- The canonical runtime origin is always the registered alias itself, for
  example:
  - `https://docs.sora/`
  - `https://solswap-indexer.sora/api/indexer/v1/health`
- For clients that cannot resolve SoraDNS names directly yet, Torii exposes a
  generic fallback path:
  - `https://taira.sora.org/soradns/docs.sora/`
  - `https://taira.sora.org/soradns/solswap-indexer.sora/api/indexer/v1/health`
- On Taira, the shared public edge serves that fallback form generically for
  active aliases. Do not replace it with invented per-service paths such as
  `https://taira.sora.org/<service>/...`.
- The `/soradns/<alias>/...` path is a compatibility gateway, not the
  canonical app origin. App manifests, frontend env vars, and release notes
  should continue to point at the vanity host itself.
- The fallback accepts any active registered alias FQDN, not only `.sora`
  names. `.dao`, `.nexus`, and future suffixes follow the same rule.

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
    - `nexus-split-app` as an alias for `split-app`
  - `single-api` produces a root-bound frontend plus one deterministic API
    service under `/api`
  - `split-app` produces a SoraFS frontend plus one hosted live API and one
    deterministic vault API
- `iroha app soracloud app local-plan`
  - validates every service referenced by an app manifest locally
  - prints the mixed-plane runtime split:
    - hosted `/api/v1/*` ownership for `HttpService + Inrou`
    - deterministic handler ownership for `/api/auth*` and `/api/v1/user*`
  - prints the frontend CID gateway URL template when `publish_mode = CidOnly`
  - prints the root `manifest_path`, root `hostname`, then each service's
    resolved `container_manifest_path` and `service_manifest_path` for
    service-scoped Soracloud commands
  - prints each child service `workspace_dir` plus discovered child scripts such
    as `dev.sh`, `build.sh`, and `verify-build.sh` when present
  - reports the manifest-adjacent root script paths for `local-dev.sh`,
    `build-and-sync.sh`, `deploy.sh`, and `upgrade.sh`
- `iroha app soracloud app local-dev`
  - resolves `local-dev.sh` adjacent to the app manifest
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app local-plan` reports,
    and the same child service plus route plan
  - without `--dry-run`, executes the local app entrypoint in place
- `iroha app soracloud app build-and-sync`
  - resolves `build-and-sync.sh` adjacent to the app manifest
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app local-plan` reports,
    and the same child service plus route plan
  - without `--dry-run`, executes the root rebuild + manifest-sync entrypoint in place
- `iroha app soracloud app doctor`
  - validates the split-app release contract locally without Torii
  - fail-closes on CID-only frontend publication, same-origin `/api`, mixed
    hosted-live plus deterministic-vault planes, lease-backed live storage,
    vault-only auth/user bindings, and cross-service route collisions
- `iroha app soracloud app doctor-workspace`
  - resolves `doctor.sh` adjacent to the app manifest
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app local-plan` reports,
    and the same child service plus route plan
  - without `--dry-run`, executes the root doctor entrypoint in place
- `iroha app soracloud app release-workspace`
  - resolves `release.sh` adjacent to the app manifest
  - forwards `TORII_URL` and optional `API_TOKEN` into the generated root script
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app local-plan` reports,
    and the same child service plus route plan
  - without `--dry-run`, executes the root release entrypoint in place
- `iroha app soracloud app deploy-workspace`
  - resolves `deploy.sh` adjacent to the app manifest
  - forwards `TORII_URL` and optional `API_TOKEN` into the generated root script
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app local-plan` reports,
    and the same child service plus route plan
- `iroha app soracloud app upgrade-workspace`
  - resolves `upgrade.sh` adjacent to the app manifest
  - forwards `TORII_URL` and optional `API_TOKEN` into the generated root script
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app local-plan` reports,
    and the same child service plus route plan
- `iroha app soracloud sync-manifests`
  - recomputes `container.bundle_hash`, the service-side referenced container
    hash, and matching schema versions after local edits
  - supports:
    - one service pair via `--container`, `--service`, and optional
      `--bundle-file`
    - every service in an app manifest via `--app-manifest`
- `iroha app soracloud local-plan`
  - validates one service pair locally
  - prints the service execution plane, runtime, route ownership, handler routes,
    and manifest-adjacent root script paths
- `iroha app soracloud local-dev`
  - resolves `local-dev.sh` adjacent to one container/service manifest pair
  - `--dry-run` prints the resolved working directory, script path, and the
    same service plan that `local-plan` reports, including routes, counts, and
    workspace scripts
  - without `--dry-run`, executes the local service entrypoint in place
- `iroha app soracloud build-and-sync`
  - resolves `build-and-sync.sh` adjacent to one container/service manifest pair
  - `--dry-run` prints the resolved working directory, script path, and the
    same service plan that `local-plan` reports, including routes, counts, and
    workspace scripts
  - without `--dry-run`, executes the root rebuild + manifest-sync entrypoint in place
- `iroha app soracloud deploy-workspace`
  - resolves `deploy.sh` adjacent to one container/service manifest pair
  - forwards `TORII_URL` and optional `API_TOKEN` into the generated root script
  - `--dry-run` prints the resolved working directory, script path, and the
    same service plan that `local-plan` reports, including routes, counts, and
    workspace scripts
- `iroha app soracloud upgrade-workspace`
  - resolves `upgrade.sh` adjacent to one container/service manifest pair
  - forwards `TORII_URL` and optional `API_TOKEN` into the generated root script
  - `--dry-run` prints the resolved working directory, script path, and the
    same service plan that `local-plan` reports, including routes, counts, and
    workspace scripts

## Network-Backed Commands

All deploy, upgrade, rollback, rollout, status, config, secret, HF lease,
training-job, model registry/status, and app mutation commands are
Torii-backed and require `--torii-url`.

- `iroha app soracloud deploy`
  - validates a single `SoraDeploymentBundleV1` locally and submits it to
    `POST /v1/soracloud/deploy`
  - returns the same local route and workspace-script projection that
    `local-plan` reports, plus the live mutation response
- `iroha app soracloud upgrade`
  - validates and submits one upgraded bundle to
    `POST /v1/soracloud/upgrade`
  - returns the same local route and workspace-script projection that
    `local-plan` reports, plus the live mutation response
- `iroha app soracloud app deploy`
  - loads `app_manifest.json`
  - synchronizes every referenced service pair before submission
  - publishes the declared static site from `static_site.dist_dir`
  - deploys every referenced service in one pass
  - returns the root app `manifest_path`, root `workspace_dir`, root
    `workspace_scripts`, root `hostname`, the frontend publish projection, the
    top-level app `routes` split, and one manifest-derived service entry per
    app service alongside the mutation responses
- `iroha app soracloud app upgrade`
  - follows the same app-wide flow, but uses upgrade semantics
  - returns the same app-scoped root manifest/hostname/workspace metadata,
    frontend, top-level `routes`, and service projection
- `iroha app soracloud app release`
  - composes the manifest-adjacent `build-and-sync` path with deploy-then-upgrade-on-conflict semantics
  - is the recommended one-command mixed-app operator path for split apps
  - returns the same root app `manifest_path`, root `workspace_dir`, root
    `workspace_scripts`, root `hostname`, the frontend publish projection, the
    top-level app `routes` split, and one manifest-derived service entry per
    app service alongside the live mutation response
- `iroha app soracloud status`
  - accepts `--service-name` directly or resolves the filter from
    `--container` plus `--service`
  - queries authoritative service state from `GET /v1/soracloud/status`
  - projects typed `schema_version`, service counts, and service summaries while
    preserving the raw Torii payload
  - when driven by `--container` plus `--service`, also keeps the same local
    route and workspace-script projection that `local-plan` reports
- `iroha app soracloud config-*` and `iroha app soracloud secret-*`
  - accept `--service-name` directly or resolve the owning service from
    `--container` plus `--service`
  - keep service-scoped material operations aligned with manifest workspaces
  - when driven by `--container` plus `--service`, attach the same local
    `service_plan` projection that `local-plan` reports
- `iroha app soracloud rollback` and `iroha app soracloud rollout`
  - accept `--service-name` directly or resolve the target service from
    `--container` plus `--service`
  - keep rollback and rollout control aligned with manifest workspaces
  - when driven by `--container` plus `--service`, attach the same local
    `service_plan` projection that `local-plan` reports
- `iroha app soracloud hf-deploy`, `hf-status`, `hf-lease-renew`, and `hf-lease-leave`
  - accept `--service-name` directly or resolve the bound service from
    `--container` plus `--service` when a service name applies
  - keep HF shared-lease membership aligned with manifest workspaces
  - when driven by `--container` plus `--service`, attach the same local
    `service_plan` projection that `local-plan` reports
- `iroha app soracloud training-job-*`
  - accept `--service-name` directly or resolve the owning service from
    `--container` plus `--service`
  - keep training-job control aligned with manifest workspaces
  - when driven by `--container` plus `--service`, attach the same local
    `service_plan` projection that `local-plan` reports
- `iroha app soracloud model-artifact-*`, `model-weight-*`,
  `model-upload-encryption-recipient`, `model-upload-init`,
  `model-upload-chunk`, `model-upload-finalize`, `model-upload-status`,
  `model-compile`, `model-compile-status`, `model-allow`,
  `model-run-private`, `model-run-status`, `model-decrypt-output`, and
  `model-publish-private`
  - accept `--service-name` directly or resolve the owning service from
    `--container` plus `--service` when a service name applies
  - keep model registry and uploaded-model status control aligned with
    manifest workspaces
  - when driven by `--container` plus `--service`, attach the same local
    `service_plan` projection that `local-plan` reports
- `iroha app soracloud app status`
  - keeps one status entry per service declared in one app manifest
  - projects the root app `manifest_path`, root `workspace_dir`, root
    `workspace_scripts`, root `hostname`, the top-level app `routes` split, the
    frontend publish mode, and the expected CID-gateway or root-binding URL
    when a static site is configured
  - projects child manifest paths, `workspace_dir`, plane/runtime, route
    ownership, and the matched Torii control-plane status when present
  - keeps missing manifest services visible instead of dropping them from the
    app-scoped output

## Hosted Service Lease Volumes

Hosted HTTP services can persist mutable shared state without inventing their
own local directory contract.

Example manifest shape:

```json
{
  "lease_volumes": [
    { "volume_name": "root_disk", "mount_path": "/" },
    { "volume_name": "shared_cache", "mount_path": "/var/lib/soracloud/shared-cache" },
    { "volume_name": "search_sessions", "mount_path": "/var/lib/soracloud/search-sessions" },
    { "volume_name": "collector_state", "mount_path": "/var/lib/soracloud/collector-state" },
    { "volume_name": "runtime_cache", "mount_path": "/var/lib/soracloud/runtime-cache" }
  ]
}
```

For the example above, the service receives:

- `SORACLOUD_LEASE_VOLUME_ROOT_DISK_DIR`
- `SORACLOUD_LEASE_VOLUME_ROOT_DISK_MOUNT_PATH`
- `SORACLOUD_LEASE_VOLUME_SHARED_CACHE_DIR`
- `SORACLOUD_LEASE_VOLUME_SHARED_CACHE_MOUNT_PATH`
- `SORACLOUD_LEASE_VOLUME_SEARCH_SESSIONS_DIR`
- `SORACLOUD_LEASE_VOLUME_SEARCH_SESSIONS_MOUNT_PATH`
- `SORACLOUD_LEASE_VOLUME_COLLECTOR_STATE_DIR`
- `SORACLOUD_LEASE_VOLUME_COLLECTOR_STATE_MOUNT_PATH`
- `SORACLOUD_LEASE_VOLUME_RUNTIME_CACHE_DIR`
- `SORACLOUD_LEASE_VOLUME_RUNTIME_CACHE_MOUNT_PATH`

The `_DIR` value is the materialized runtime path. The `_MOUNT_PATH` value is
the logical mount path declared in the manifest.

## Inrou Smoke

Inrou now supports both local backend classes: `FirecrackerKvm` on Linux/KVM
and `PortableVm` everywhere else. Mixed-host validator fleets still proxy
hosted HTTP through authoritatively placed healthy replicas, but local
materialization is no longer restricted to Linux/KVM peers. The repo now ships
dedicated smoke entrypoints for both local backend classes plus a mixed-host
orchestrator:

```bash
cargo xtask soracloud-inrou-smoke portable
sudo cargo xtask soracloud-inrou-smoke firecracker
cargo xtask soracloud-inrou-smoke mixed-host --inventory ./fixtures/soracloud/inrou_mixed_host_inventory.example.toml
```

PortableVm stays unprivileged and requires only native-ISA QEMU, `qemu-img`,
and `tar`. Shared non-root lease volumes are attached as persistent block
devices that the guest formats and mounts on first boot. The Firecracker/KVM
path keeps the Linux host prerequisites for tap networking and the NFS
transport adapter:

- host OS is Linux
- the caller is root
- `/dev/kvm` exists
- `/dev/net/tun` exists
- `/proc/sys/net/ipv4/ip_forward = 1`
- `firecracker`, `ip`, `iptables`, `tar`, `exportfs`, `rpc.nfsd`, `mount`,
  `chown`, and `mke2fs` or `mkfs.ext4` are on `PATH`

Portable smoke expects:

- `IROHA_INROU_PORTABLE_KERNEL_IMAGE`
- `IROHA_INROU_PORTABLE_ROOTFS_IMAGE`
- optional `IROHA_INROU_PORTABLE_INITRD_IMAGE`
- optional `IROHA_INROU_PORTABLE_ACCEL` (`auto`, `tcg`, `kvm`, `hvf`, `whpx`)

Firecracker smoke expects:

- `IROHA_INROU_LINUX_KVM_KERNEL_IMAGE`
- `IROHA_INROU_LINUX_KVM_ROOTFS_IMAGE`
- optional `IROHA_INROU_LINUX_KVM_INITRD_IMAGE`

Focused validation now covers both shared-storage transports:

- `build_inrou_user_data_projects_portable_block_mounts_and_allowlist_overlay`
- `ensure_inrou_portable_lease_disks_create_reusable_raw_images`
- `ensure_inrou_portable_root_disk_uses_qcow2_overlay_with_backing_file`
- `build_inrou_user_data_projects_mounts_overlay_and_replica_env`
- `write_inrou_firecracker_config_serializes_boot_source_drives_and_network`
- `ensure_inrou_root_disk_copies_once_and_reuses_existing_rootfs`
- `planned_inrou_tap_firewall_rules_keep_isolated_policy_private`

The portable path mounts shared lease storage through attached virtio block
devices. The Firecracker fast path keeps the backend-private NFS adapter, but
both backends now expose the same guest-visible lease semantics. Mixed-host
acceptance is expected to cover one Linux/KVM Firecracker validator, one
non-Linux PortableVm validator, and one proxy-only validator that publishes
zero hosted capacity.

## Recommended Hosted-Service Workflow

Use the `http-service` scaffold for one hosted HTTP service that should run on
the Soracloud hosted plane without an app manifest wrapper.

Build and refresh the single service pair:

```bash
cd .soracloud-live
iroha app soracloud local-plan --container ./container_manifest.json --service ./service_manifest.json
./build-and-sync.sh
iroha app soracloud build-and-sync --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Run the local hosted-service dev entrypoint:

```bash
cd .soracloud-live
iroha app soracloud local-plan --container ./container_manifest.json --service ./service_manifest.json
./local-dev.sh
iroha app soracloud local-dev --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Deploy or upgrade through the same hosted-service root scripts:

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

The direct `deploy`, `upgrade`, and manifest-pair `status` outputs keep the
same local route and workspace-script projection that `local-plan` reports,
alongside the live control-plane data.

## Recommended Single-App Workflow

Use the single-api scaffold for apps that need:

- one root-bound static frontend on the public host
- one deterministic IVM API on the same host under `/api`
- no hosted `Inrou` plane and no split `/api` ownership

```bash
iroha app soracloud app init \
  --template single-api \
  --app-name docs_portal \
  --app-version 1.0.0 \
  --output-dir .soracloud-docs-portal
```

Build the frontend and API bundle:

```bash
cd .soracloud-docs-portal
./build-and-sync.sh
iroha app soracloud app build-and-sync --manifest ./app_manifest.json --dry-run
```

For local development, the scaffold also includes:

- `services/api/dev.sh` to run a local `/api/healthz` HTTP shim
- `./local-dev.sh` to boot the frontend plus the local API shim
- `./build-and-sync.sh` to rebuild the frontend and deterministic bytecode and
  refresh manifest hashes
- `./deploy.sh` to run the full app-wide publish + deploy flow
- `./upgrade.sh` to rerun the same rebuild path and submit the app upgrade

Run the root-bound app locally:

```bash
cd .soracloud-docs-portal
./local-dev.sh
iroha app soracloud app local-dev --manifest ./app_manifest.json --dry-run
```

Deploy the root-bound frontend plus the API service in one step:

```bash
cd .soracloud-docs-portal
TORII_URL=http://127.0.0.1:8080 ./deploy.sh
iroha app soracloud app deploy-workspace --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
```

Upgrade the root-bound frontend plus the API service in one step:

```bash
cd .soracloud-docs-portal
TORII_URL=http://127.0.0.1:8080 ./upgrade.sh
iroha app soracloud app upgrade-workspace --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
```

This path keeps the frontend at `/` and the API at `/api/healthz` on the same
hostname.

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

For local development, the scaffold includes:

- `frontend/` Vite dev mode for the same-host `/api` base with proxy rules for
  the live and vault planes
- `services/live/dev.sh` to run the hosted HTTP starter directly with local
  fallback lease directories
- `services/vault/dev.sh` to run the local vault HTTP shim for `/api/auth*` and
  `/api/v1/user*`
- `services/vault/verify-build.sh` to recompile the deterministic vault
  contract and verify that the committed build outputs still match
- `./local-dev.sh` to boot the frontend plus both local API processes
- `./build-and-sync.sh` to rebuild all artifacts and refresh manifest hashes
- `./doctor.sh` to rebuild and fail-close on the split-app release contract
- `./release.sh` to rebuild, validate, and then deploy-or-upgrade the full app
- `./deploy.sh` as the compatibility wrapper around `./release.sh`
- `./upgrade.sh` to rerun the same rebuild path, validate, and submit the app upgrade

The scaffolded Vite proxy strips the shared `/api` prefix before forwarding to
the live and vault child processes, which matches the hosted longest-prefix
routing behavior Torii applies in production.

Run the local mixed-app loop:

```bash
cd .soracloud-hayahi
./local-dev.sh
iroha app soracloud app local-dev --manifest ./app_manifest.json --dry-run
```

Build the three app surfaces and refresh manifest hashes:

```bash
cd .soracloud-hayahi
./build-and-sync.sh
iroha app soracloud app build-and-sync --manifest ./app_manifest.json --dry-run
```

Inspect the local split-plane routing and frontend publish shape before deploy:

```bash
iroha app soracloud app local-plan \
  --manifest .soracloud-hayahi/app_manifest.json
```

Deploy the static site plus every service without SSH or manual pinning:

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
operators can stay on the manifest-driven CLI path without shelling into the
workspace manually.

Those generated root scripts resolve `IROHA_CLI_BIN`, `IROHA_BIN`,
`IROHA_CARGO_TARGET_DIR/.../iroha`, `CARGO_TARGET_DIR/.../iroha`,
`IROHA_MANIFEST_PATH`, and finally `PATH` `iroha`, so local app workspaces
can target a nearby `iroha_cli` checkout without requiring a globally
installed wrapper. When you drive the fallback through `IROHA_MANIFEST_PATH`,
set `IROHA_CARGO_HOME` and `IROHA_CARGO_TARGET_DIR` to keep Cargo package and
artifact state isolated from other local builds.

Upgrade the static site plus every service without SSH or manual pinning:

```bash
cd .soracloud-hayahi
TORII_URL=http://127.0.0.1:8080 ./upgrade.sh
iroha app soracloud app upgrade-workspace --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
```

For Taira-style deployments, keep Torii root bound to Torii itself and use the
gateway host only as:

- the Torii/control-plane base URL
- the non-SoraDNS fallback form `https://taira.sora.org/soradns/<alias>/...`
- the SoraFS CID gateway for intentionally CID-only frontend assets

Do not replace a stable Soracloud vanity host with a `taira.sora.org` path
just because a new revision, build, or CID was published.
