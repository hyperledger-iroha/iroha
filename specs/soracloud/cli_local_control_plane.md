# Soracloud CLI and Control Plane

Soracloud v1 ships an authoritative deterministic runtime:

- deterministic services run on `SoraContainerRuntimeV1::Ivm`
- hosted HTTP manifests use `execution_plane = HttpService` with
  `runtime = Inrou`; a node may host them only after explicit PortableVM V1
  enablement and successful runtime qualification

The control plane remains authoritative. Torii serves status and mutation
routes directly from committed world state plus the embedded Soracloud runtime
manager; the CLI does not keep a shadow control-plane mirror.

Soracloud's provenance-bearing mutation bodies use strict request schemas.
They do not contain `authority` or `private_key` fields, and JSON admission
rejects either field before constructing a request object, including inside
app-level nested service bundles. Clients authenticate the HTTP request with
`X-Iroha-Account` plus the canonical signature, timestamp, and nonce headers,
or with `X-Iroha-Witness`. The mutation provenance signer must be one of the
verified request signers. Torii then returns an unsigned instruction skeleton;
account private keys stay in the client wallet.

Aggregate, status, config, secret, health, training, model, host, and agent
GETs use the same exact-network canonical account boundary. The CLI signs the
final method, path, sorted query, and empty body with its configured NetworkId
and local account key before dispatch, does not follow redirects or retry an
ambiguous transport failure, and fails before network I/O when that signer is
unavailable. A listener API token may be sent in addition, but never replaces
the account proof.

Only the exact service discovery object, exact service-revision discovery
object, and current upload encryption-recipient object remain public. Each is
a single-object read with a 64 KiB encoded-JSON response cap; aggregate or
sensitive Soracloud state is not part of the public discovery surface.

## Runtime Scope

- Use deterministic IVM services for wallet auth, confidential vault state,
  governance-sensitive mutations, and other replay-safe handlers.
- `HttpService + Inrou` manifests and split-plane apps are shipping surfaces on
  an explicitly enabled host. The default-disabled or unqualified posture
  fails closed without advertising or materializing them.
- Hosted topology uses authoritative longest-prefix routing.
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
- Runtime health-report and generated-HF reconciliation cooldown histories
  expire with their cooldown window and have a hard process-wide entry bound.
  Identity churn cannot grow either tracker without limit; a full live window
  fails closed until an entry expires.
- The current local workflow validates both planes together with
  `iroha soracloud app plan`, resolves manifest-adjacent workspace
  scripts with `iroha soracloud app dev` and
  `iroha soracloud app build`, and verifies deterministic vault
  builds with `verify-build.sh`. This is a dev shim for the deterministic
  plane, not a full embedded IVM runtime.

## Runtime Provenance Signature Framing

Model-host heartbeats and Inrou host advertisements use one canonical V1
signature preimage. It is the canonical Norito tuple
`(domain_tag_bytes, version, purpose_wire_id, canonical_payload_bytes)`, where
the fixed domain is `iroha:soracloud:runtime-provenance:v1\0`, the version is
`1`, and the immutable purpose ids are `1` for model-host heartbeat and `2` for
Inrou host advertisement. The byte fields are length-delimited by Norito.

External signing adapters receive the expected purpose and framed preimage as
separate inputs and must reject any disagreement before signing. Ledger
admission independently rebuilds the operation-specific framed preimage, so a
raw semantic-payload signature or a signature from the other runtime purpose
fails verification.

## Vanity Alias Access Modes

- Soracloud deploys keep the registered vanity host stable. Releasing a new
  revision updates Soracloud route state, not per-release DNS records or
  per-service nginx host lists.
- The canonical runtime origin is always the registered alias itself, for
  example:
  - `https://docs.sora/`
  - `https://solswap-indexer.sora/api/indexer/v1/health`
- For clients that cannot resolve SoraDNS names directly yet, Taira exposes the
  owned Mon browser gateway:
  - `https://docs.sora.mon.taira.sora.net/`
  - `https://solswap-indexer.sora.mon.taira.sora.net/api/indexer/v1/health`
- Path-encoded aliases under `taira.sora.org` are rejected. App manifests,
  frontend env vars, and release notes point at the vanity host itself; public
  browser examples use the Mon gateway when native SoraDNS is unavailable.

## Offline Scaffolding Commands

- `iroha soracloud service init`
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
- `iroha soracloud app init`
  - scaffolds an app manifest plus one or more service pairs
  - templates now include:
    - `single-api`
    - `split-app`
  - `single-api` produces a root-bound frontend plus one deterministic API
    service under `/api`
  - `split-app` produces a SoraFS frontend plus one hosted live API and one
    deterministic vault API
- `iroha soracloud app plan`
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
  - reports the manifest-adjacent root script paths for `dev.sh`,
    `build-and-sync.sh`, `deploy.sh`, and `upgrade.sh`
- `iroha soracloud app dev`
  - resolves `dev.sh` adjacent to the app manifest
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app plan` reports,
    and the same child service plus route plan
  - without `--dry-run`, executes the local app entrypoint in place
- `iroha soracloud app build`
  - resolves `build-and-sync.sh` adjacent to the app manifest
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app plan` reports,
    and the same child service plus route plan
  - without `--dry-run`, executes the root rebuild + manifest-sync entrypoint in place
- `iroha soracloud app doctor`
  - validates the split-app release contract locally without Torii
  - fail-closes on CID-only frontend publication, same-origin `/api`, mixed
    hosted-live plus deterministic-vault planes, lease-backed live storage,
    vault-only auth/user bindings, and cross-service route collisions
- `iroha soracloud app doctor`
  - resolves `doctor.sh` adjacent to the app manifest
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app plan` reports,
    and the same child service plus route plan
  - without `--dry-run`, executes the root doctor entrypoint in place
- `iroha soracloud app release`
  - resolves `release.sh` adjacent to the app manifest
  - forwards `TORII_URL` and optional `API_TOKEN` into the generated root script
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app plan` reports,
    and the same child service plus route plan
  - without `--dry-run`, executes the root release entrypoint in place
- `iroha soracloud app deploy`
  - resolves `deploy.sh` adjacent to the app manifest
  - forwards `TORII_URL` and optional `API_TOKEN` into the generated root script
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app plan` reports,
    and the same child service plus route plan
- `iroha soracloud app upgrade`
  - resolves `upgrade.sh` adjacent to the app manifest
  - forwards `TORII_URL` and optional `API_TOKEN` into the generated root script
  - `--dry-run` prints the resolved working directory, script path, mixed-plane
    summary, the same root app identity fields that `app plan` reports,
    and the same child service plus route plan
- `iroha soracloud service sync-manifests`
  - recomputes `container.bundle_hash`, the service-side referenced container
    hash, and matching schema versions after local edits
  - supports:
    - one service pair via `--container`, `--service`, and optional
      `--bundle-file`
    - every service in an app manifest via `--app-manifest`
- `iroha soracloud service plan`
  - validates one service pair locally
  - prints the service execution plane, runtime, route ownership, handler routes,
    and manifest-adjacent root script paths
- `iroha soracloud service dev`
  - resolves `dev.sh` adjacent to one container/service manifest pair
  - `--dry-run` prints the resolved working directory, script path, and the
    same service plan that `plan` reports, including routes, counts, and
    workspace scripts
  - without `--dry-run`, executes the local service entrypoint in place
- `iroha soracloud service build`
  - resolves `build-and-sync.sh` adjacent to one container/service manifest pair
  - `--dry-run` prints the resolved working directory, script path, and the
    same service plan that `plan` reports, including routes, counts, and
    workspace scripts
  - without `--dry-run`, executes the root rebuild + manifest-sync entrypoint in place
- `iroha soracloud service deploy`
  - resolves `deploy.sh` adjacent to one container/service manifest pair
  - forwards `TORII_URL` and optional `API_TOKEN` into the generated root script
  - `--dry-run` prints the resolved working directory, script path, and the
    same service plan that `plan` reports, including routes, counts, and
    workspace scripts
- `iroha soracloud service upgrade`
  - resolves `upgrade.sh` adjacent to one container/service manifest pair
  - forwards `TORII_URL` and optional `API_TOKEN` into the generated root script
  - `--dry-run` prints the resolved working directory, script path, and the
    same service plan that `plan` reports, including routes, counts, and
    workspace scripts

## Network-Backed Commands

All deploy, upgrade, rollback, rollout, status, config, secret, HF lease,
training-job, model registry/status, and app mutation commands are
Torii-backed and require `--torii-url`.

- `iroha soracloud service deploy`
  - validates a single `SoraDeploymentBundleV1` locally and submits it to
    `POST /v1/soracloud/deploy`
  - returns the same local route and workspace-script projection that
    `plan` reports, plus the live mutation response
- `iroha soracloud service upgrade`
  - validates and submits one upgraded bundle to
    `POST /v1/soracloud/upgrade`
  - returns the same local route and workspace-script projection that
    `plan` reports, plus the live mutation response
- `iroha soracloud app deploy`
  - loads `app_manifest.json`
  - synchronizes every referenced service pair before submission
  - publishes the declared static site from `static_site.dist_dir`
  - deploys every referenced service in one pass
  - returns the root app `manifest_path`, root `workspace_dir`, root
    `workspace_scripts`, root `hostname`, the frontend publish projection, the
    top-level app `routes` split, and one manifest-derived service entry per
    app service alongside the mutation responses
- `iroha soracloud app upgrade`
  - follows the same app-wide flow, but uses upgrade semantics
  - returns the same app-scoped root manifest/hostname/workspace metadata,
    frontend, top-level `routes`, and service projection
- `iroha soracloud app release`
  - composes the manifest-adjacent `build-and-sync` path with deploy-then-upgrade-on-conflict semantics
  - is the recommended one-command mixed-app operator path for split apps
  - returns the same root app `manifest_path`, root `workspace_dir`, root
    `workspace_scripts`, root `hostname`, the frontend publish projection, the
    top-level app `routes` split, and one manifest-derived service entry per
    app service alongside the live mutation response
- `iroha soracloud service status`
  - accepts `--service-name` directly or resolves the filter from
    `--container` plus `--service`
  - queries authoritative service state from `GET /v1/soracloud/status`
  - projects typed `schema_version`, service counts, and service summaries while
    preserving the raw Torii payload
  - when driven by `--container` plus `--service`, also keeps the same local
    route and workspace-script projection that `plan` reports
- `iroha soracloud service config-*` and `iroha soracloud service secret-*`
  - accept `--service-name` directly or resolve the owning service from
    `--container` plus `--service`
  - keep service-scoped material operations aligned with manifest workspaces
  - when driven by `--container` plus `--service`, attach the same local
    `service_plan` projection that `plan` reports
- `iroha soracloud service rollback` and `iroha soracloud service rollout`
  - accept `--service-name` directly or resolve the target service from
    `--container` plus `--service`
  - require rollback callers to select an already-admitted revision with
    `--target-version`; no revision is inferred from audit history
  - keep rollback and rollout control aligned with manifest workspaces
  - when driven by `--container` plus `--service`, attach the same local
    `service_plan` projection that `plan` reports
- `iroha soracloud hf deploy`, `hf-status`, `hf-lease-renew`, and `hf-lease-leave`
  - accept `--service-name` directly or resolve the bound service from
    `--container` plus `--service` when a service name applies
  - keep HF shared-lease membership aligned with manifest workspaces
  - when driven by `--container` plus `--service`, attach the same local
    `service_plan` projection that `plan` reports
- `iroha soracloud model training-job-*`
  - accept `--service-name` directly or resolve the owning service from
    `--container` plus `--service`
  - keep training-job control aligned with manifest workspaces
  - when driven by `--container` plus `--service`, attach the same local
    `service_plan` projection that `plan` reports
- `iroha soracloud model artifact-*`, `model-weight-*`,
  `model-upload-encryption-recipient`, `model-upload-register`, and
  `model-upload-status`
  - accept `--service-name` directly or resolve the owning service from
    `--container` plus `--service` when a service name applies
  - keep model registry and uploaded-model status control aligned with
    manifest workspaces
  - when driven by `--container` plus `--service`, attach the same local
    `service_plan` projection that `plan` reports
- `iroha soracloud app status`
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

Inrou V1 has one runtime backend: one Linux KVM `PortableVm`. Backend,
accelerator, concurrency, and supplementary-group configuration selectors are
retired; an enabled host intrinsically runs at most one VM and
derives its sole supplementary gid from the validated `/dev/kvm` device.
`backends` and `max_concurrent_vms` are unknown Inrou configuration keys.
Firecracker and TCG labels are configuration and wire-schema errors; no mixed
backend smoke mode is exposed. `inrou.enabled` remains false by default.
Explicit enablement selects only PortableVM V1, and the manager advertises or
hosts nothing until its exact production preflight succeeds.

Persisted V1 runtime state is exact rather than migratory. The top-level
snapshot and every nested mailbox, artifact, lease-volume, apartment, HF-source,
hosted-HTTP aggregate, and hosted-HTTP replica record reject unknown or missing
keys. Nullable handler, error, listen, and pid values must be present as
explicit JSON `null`; no decoder default or predecessor layout is inferred.

The mandatory launcher reserves its configured uid/gid exclusively for the
supervised QEMU process. The uid and primary gid must be one equal canonical
slot pair: `70000` through `70003`; slot `i` deterministically requires the
locked local account and primary group name `iroha-inrou-i`. Provision all four
slots for a same-host four-validator Taira qualification. A single-validator
public host uses slot 0 (`iroha-inrou-0`, uid/gid `70000`). Each identity has password fields `x`, home
`/nonexistent`, a trusted literal nologin/false shell, locked shadow/gshadow
passwords, and no extra group membership or administrator entry. The host must use exactly
`passwd: files` and `group: files` in root-custodied `/etc/nsswitch.conf`;
SSS, systemd, LDAP, compat, and other identity sources are rejected. The
decimal identity names, duplicate records, and another account's use of the
primary gid are rejected. `subid`, when declared, must also use only `files`;
local `/etc/subuid` and `/etc/subgid` must not assign a range to the selected service
identity/numeric spelling or cover the child uid or any configured gid.
The runtime refuses disk
delegation while any process uses that uid or carries the primary gid as a
real, effective, saved, filesystem, or supplementary group.
The validated identity retains exactly the direct root-owned `/dev/kvm` device
group, and only when that character device is the
Linux KVM misc device (major 10, minor 232) with group read/write access and no
world access. Pinned bubblewrap and setpriv launch QEMU behind a token barrier
in private mount, network, IPC, UTS, PID, and cgroup namespaces. The
authenticated minimal root contains only the fixed QEMU runtime closure,
`/dev/kvm`, exact read-only inputs, exact writable disks, private proc/tmp/dev
state, anonymous QMP, and bounded stderr; it exposes no host `/run`, broad
`/sys`, Unix socket, or unrelated descriptor.
Reclaiming a writable disk requires an exclusive Linux write lease through
inode/custody revalidation; unsupported filesystems fail closed.

PortableVM also requires root-owned, non-writable `iptables` for a
defense-in-depth owner rule on the supervisor's public loopback listener. The
QEMU namespace has only loopback and no external interface. Each bounded bridge
session opens one socket through an attested network-namespace descriptor to
one concrete loopback backend, then re-attests the namespaces, minimal root,
cgroup membership, and endpoint before traffic. QMP and the broker receive no
traffic before attestation. Pre-existing firewall ownership state fails closed
for explicit operator cleanup.

The dedicated cgroup-v2 subtree applies exact CPU, memory, swap, pids, and I/O
limits including bounded QEMU overhead. Its launch barrier verifies a private
token and exact procfs/cgroup membership before exec. Startup, QMP, bridge,
shutdown, and cleanup all use bounded deadlines.

To qualify a deployment, prepare verified native-ISA Debian guest assets and
run the ignored privileged smoke on the target Linux/KVM host:

```bash
eval "$(python3 scripts/ci/prepare_inrou_portable_guest_assets.py --print-env)"
cargo xtask soracloud-inrou-smoke portable
```

The smoke-only `IROHA_INROU_PORTABLE_KERNEL_IMAGE`,
`IROHA_INROU_PORTABLE_ROOTFS_IMAGE`, and optional
`IROHA_INROU_PORTABLE_INITRD_IMAGE` variables supply local guest assets; they
do not select shipping behavior. Shipping enablement comes only from the exact
`soracloud_runtime.inrou` configuration.

PortableVm atomically copies the verified base image into a standalone mutable
raw root disk. The sandbox never needs the host base-image path, and startup
never gives root `qemu-img` a guest-mutated root image to parse. Shared lease
volumes are exact-size persistent raw block devices. For a new lease disk, the
supervisor runs root-custodied standard-path `mke2fs` against an exclusive
staging file with an empty `mke2fs.conf` and the exact V1 feature, 4-KiB block,
256-byte inode, inode-count, block-group, flex-group, 16-MiB journal, label, and
logical-volume-derived UUID profile. That UUID also binds the service revision,
volume kind, storage class, and authoritative generation, so a generation
rollover fails closed instead of inheriting old contents. Lease sizes are
positive multiples of one 128-MiB block group. The supervisor validates that
complete static superblock contract, syncs the file, and only then atomically
publishes `lease.raw`. An existing published disk must retain the exact size,
logical-volume UUID, geometry, and feature masks; the only additional
incompatible feature accepted is ext4's dynamic journal-recovery bit. The
supervisor never reformats it.
Guest cloud-init verifies the filesystem type before mounting it and never
runs a formatter. The subsequent read-write mount can replay the journal and
the health probe intentionally writes into the filesystem; a mismatch or
failure aborts startup without destructive reinitialization. The NoCloud seed
and application archive remain separate read-only devices.

Inrou V1 accepts only `Isolated` networking. `Open` and `Allowlist` are
rejected until kernel-owned counters can meter every guest byte; no unmetered
QEMU `guestfwd` path is built.

Focused validation includes:

- `inrou_v1_backend_and_capacity_are_unconditional_runtime_facts`
- `bubblewrap_surface_unshares_every_mandatory_namespace`
- `private_network_table_accepts_only_loopback`
- `launch_barrier_blocks_exec_and_rejects_cgroup_drift`
- `inrou_owner_firewall_setup_drains_preexisting_public_connections`
- `build_inrou_user_data_never_formats_existing_portable_block_mounts`
- `ensure_inrou_portable_lease_disks_create_reusable_raw_images`
- `ensure_inrou_portable_root_disk_is_a_standalone_authenticated_copy`

## Hosted-Service Workflow

The `http-service` scaffold describes one hosted HTTP service without an app
manifest wrapper. Non-dry-run deploy and upgrade require a validator whose
exact PortableVM V1 configuration and host preflight are qualified.

Build and refresh the single service pair:

```bash
cd .soracloud-live
iroha soracloud service plan --container ./container_manifest.json --service ./service_manifest.json
./build-and-sync.sh
iroha soracloud service build --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Run the local hosted-service dev entrypoint:

```bash
cd .soracloud-live
iroha soracloud service plan --container ./container_manifest.json --service ./service_manifest.json
./dev.sh
iroha soracloud service dev --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Run generated hosted `deploy.sh` or `upgrade.sh` wrappers only against an
explicitly enabled, qualified PortableVM V1 host; default-disabled nodes retain
no hosting capacity.

## Recommended Single-App Workflow

Use the single-api scaffold for apps that need:

- one root-bound static frontend on the public host
- one deterministic IVM API on the same host under `/api`
- no hosted `Inrou` plane and no split `/api` ownership

```bash
iroha soracloud app init \
  --template single-api \
  --app-name docs_portal \
  --app-version 1.0.0 \
  --output-dir .soracloud-docs-portal
```

Build the frontend and API bundle:

```bash
cd .soracloud-docs-portal
./build-and-sync.sh
iroha soracloud app build --manifest ./app_manifest.json --dry-run
```

For local development, the scaffold also includes:

- `services/api/dev.sh` to run a local `/api/healthz` HTTP shim
- `./dev.sh` to boot the frontend plus the local API shim
- `./build-and-sync.sh` to rebuild the frontend and deterministic bytecode and
  refresh manifest hashes
- `./deploy.sh` to run the full app-wide publish + deploy flow
- `./upgrade.sh` to rerun the same rebuild path and submit the app upgrade

Run the root-bound apply:

```bash
cd .soracloud-docs-portal
./dev.sh
iroha soracloud app dev --manifest ./app_manifest.json --dry-run
```

Deploy the root-bound frontend plus the API service in one step:

```bash
cd .soracloud-docs-portal
TORII_URL=http://127.0.0.1:8080 ./deploy.sh
iroha soracloud app deploy --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
```

Upgrade the root-bound frontend plus the API service in one step:

```bash
cd .soracloud-docs-portal
TORII_URL=http://127.0.0.1:8080 ./upgrade.sh
iroha soracloud app upgrade --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
```

This path keeps the frontend at `/` and the API at `/api/healthz` on the same
hostname.

## Mixed-App Workflow

The split-app scaffold models apps that need:

- a static frontend published to SoraFS
- a hosted live API on Inrou
- a deterministic IVM vault or auth plane

Non-dry-run deploy, release, and upgrade require an explicitly enabled,
qualified PortableVM V1 host for the hosted member.

```bash
iroha soracloud app init \
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
- `./dev.sh` to boot the frontend plus both local API processes
- `./build-and-sync.sh` to rebuild all artifacts and refresh manifest hashes
- `./doctor.sh` to rebuild and fail-close on the split-app release contract
- `./release.sh` to rebuild, validate, and then deploy-or-upgrade the full app
- `./deploy.sh` to rebuild and submit the exact create-deployment flow
- `./upgrade.sh` to rerun the same rebuild path, validate, and submit the app upgrade

The scaffolded Vite proxy strips the shared `/api` prefix before forwarding to
the live and vault child processes, which matches the hosted longest-prefix
routing behavior Torii applies in production.

Run the local mixed-app loop:

```bash
cd .soracloud-hayahi
./dev.sh
iroha soracloud app dev --manifest ./app_manifest.json --dry-run
```

Build the three app surfaces and refresh manifest hashes:

```bash
cd .soracloud-hayahi
./build-and-sync.sh
iroha soracloud app build --manifest ./app_manifest.json --dry-run
```

Inspect the local split-plane routing and frontend publish shape:

```bash
iroha soracloud app plan \
  --manifest .soracloud-hayahi/app_manifest.json
```

Validate the split-app release contract locally:

```bash
cd .soracloud-hayahi
iroha soracloud app doctor --manifest ./app_manifest.json --dry-run
```

Run `release.sh`, `deploy.sh`, `upgrade.sh`, or their non-dry-run CLI
counterparts only when the target validator advertises a currently qualified
PortableVM V1 capability.

The generated local root scripts resolve `IROHA_BIN`, then `iroha` from `PATH`,
then an explicit source checkout via `IROHA_SOURCE_DIR` or
`IROHA_MANIFEST_PATH`, so local app workspaces can target a nearby
`iroha_cli` checkout without requiring a globally installed source wrapper.
When you drive the fallback through `IROHA_SOURCE_DIR` or `IROHA_MANIFEST_PATH`,
set `IROHA_CARGO_HOME` and `IROHA_CARGO_TARGET_DIR` to keep Cargo package and
artifact state isolated from other local builds.

For Taira deployment of shipping deterministic apps, keep Torii root bound to
Torii itself and use the gateway host only as:

- the Torii/control-plane base URL
- the public non-SoraDNS browser form `https://<alias>.mon.taira.sora.net/...`
- the SoraFS CID gateway for intentionally CID-only frontend assets

Do not replace a stable Soracloud vanity host with a `taira.sora.org` path
just because a new revision, build, or CID was published.
