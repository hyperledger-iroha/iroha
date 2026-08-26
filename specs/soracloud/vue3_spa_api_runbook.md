# Soracloud Vue3 SPA and App Runbook

This runbook covers four shipping frontend patterns:

- a static Vue3 site published to SoraFS with `--template site`
- a root-bound single-service app with `app init --template single-api`
- a hosted HTTP API with `--template http-service`
- a mixed split app with `app init --template split-app`

The single-api path is the recommended workflow for apps that need:

- a static frontend served from the app root on the public host
- a deterministic IVM API on the same host under `/api`
- one app manifest whose release publishes the frontend and submits one service

The split-app path is for apps that need:

- a static frontend served from SoraFS CID URLs
- a hosted live API on `Inrou`
- a deterministic IVM vault/auth API
- one shared `/api` surface split by authoritative longest-prefix routing

Hosted-service and split-app release requires an explicitly enabled
PortableVM V1 validator whose exact production preflight succeeds. The default
configuration remains disabled and advertises no hosting capacity.

## Access Model

Soracloud releases behave like IPFS-style publishing for runtime URLs:

- the registered vanity host stays fixed
- releases update Soracloud route bindings, not DNS records
- direct vanity-host access remains canonical
- Taira's owned public browser gateway is `mon.taira.sora.net`

Examples:

- direct frontend origin: `https://docs.sora/`
- Taira browser frontend origin: `https://docs.sora.mon.taira.sora.net/`
- direct API origin:
  `https://solswap-indexer.sora/api/indexer/v1/health`
- Taira browser API origin:
  `https://solswap-indexer.sora.mon.taira.sora.net/api/indexer/v1/health`

The Mon gateway is the public browser URL for clients that need normal DNS/TLS
before native SoraDNS resolution is available. Requests must use the alias host;
the retired path-encoded alias gateway is rejected. Do not invent
`https://taira.sora.org/<service>/...` URLs.

## 1. Generate the Scaffold

Static site only:

```bash
iroha soracloud service init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

Hosted HTTP service only:

```bash
iroha soracloud service init \
  --template http-service \
  --service-name live_search \
  --service-version 1.0.0 \
  --output-dir .soracloud-live
```

Root-bound single-api app:

```bash
iroha soracloud app init \
  --template single-api \
  --app-name docs_portal \
  --app-version 1.0.0 \
  --output-dir .soracloud-docs-portal
```

Mixed split app:

```bash
iroha soracloud app init \
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
- `dev.sh`
- `build-and-sync.sh`
- `release.sh`
- `services/live/dev.sh`
- `services/vault/dev.sh`
- `services/vault/verify-build.sh`

The single-api scaffold produces:

- `app_manifest.json`
- `web/`
- `services/api/`
- `dev.sh`
- `build-and-sync.sh`
- `release.sh`
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
iroha soracloud service plan --container ./container_manifest.json --service ./service_manifest.json
./build-and-sync.sh
iroha soracloud service build --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Hosted HTTP service local development:

```bash
cd .soracloud-live
iroha soracloud service plan --container ./container_manifest.json --service ./service_manifest.json
./dev.sh
iroha soracloud service dev --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Run a generated hosted `release.sh` only against an explicitly enabled,
currently qualified PortableVM V1 validator.

For deployment qualification, run the ignored privileged Inrou V1 smoke
against a verified guest asset class on the target Linux/KVM host:

The preparer requires `gpgv` or `gpg`, a trusted Debian keyring, the detached
`SHA512SUMS.sign`, and agreement between both the authenticated Debian digest
and the repository-pinned SHA512. Select a keyring with `--debian-keyring` or
`DEBIAN_ARCHIVE_KEYRING` when no system Debian keyring is installed.

```bash
eval "$(python3 scripts/ci/prepare_inrou_portable_guest_assets.py --print-env)"
cargo xtask soracloud-inrou-smoke portable
```

`PortableVm` is the sole first-release backend and intrinsically means one
Linux KVM VM. Backend, accelerator, concurrency, and supplementary-group
selectors are retired; the runtime rejects Firecracker and TCG labels rather
than falling back. In particular, `backends` and `max_concurrent_vms` are
unknown Inrou configuration keys. `inrou.enabled` is false by default; explicit enablement
selects only PortableVM V1, and no capability or local workload activates
until the full production preflight succeeds.

The locked local host identity is exclusive to the one supervised QEMU
process. `/etc/nsswitch.conf` must use exactly
`passwd: files` and `group: files`; SSS, systemd, LDAP, compat, and other
sources are rejected. The uid and primary gid must be one equal canonical
slot pair: `70000` through `70003`. Slot `i` is reserved by exact
`iroha-inrou-i` passwd/group rows; a single-validator public host uses slot 0,
while a same-host four-validator qualification provisions all four. Each row uses
password field `x`, nonexistent home `/nonexistent`, a trusted literal
nologin/false shell, locked shadow/gshadow passwords, and no extra group
membership or administrator entry. Decimal identity names, primary-gid reuse,
and duplicate records are rejected. `subid`, when present, must also use only
`files`; no subordinate range may belong to the selected `iroha-inrou-i`/its numeric spelling
or cover the child uid or a configured gid. Startup checks also fail closed if
any process already uses that uid or primary gid.
The runtime derives exactly the group of a direct, root-owned Linux KVM
character device at `/dev/kvm` (misc major 10, minor 232) whose group grants
read/write access and whose world access is disabled; any additional group
aborts startup. Pinned bubblewrap and setpriv place QEMU behind a token barrier
in private mount, network, IPC, UTS, PID, and cgroup namespaces. Its
authenticated minimal root exposes only the fixed runtime closure, `/dev/kvm`,
exact inputs and disks, anonymous QMP, and bounded stderr; host `/run`, broad
`/sys`, Unix sockets, and unrelated inherited descriptors are absent.
Reused writable disks additionally require a successful exclusive Linux write
lease while root custody and inode identity are revalidated; filesystems that
do not support leases are rejected.

The Linux host must provide root-owned, non-writable `iptables` for the
defense-in-depth owner rule on the supervisor's public loopback listener. The
private network namespace exposes only loopback. Each bounded bridge session
opens one socket through an attested namespace descriptor to one exact backend,
then re-attests namespace, mount, cgroup, and endpoint identity before traffic.
QMP and the bridge receive no traffic before attestation. The dedicated
cgroup-v2 subtree applies exact CPU, memory, swap, pids, and I/O limits,
including bounded QEMU overhead; startup and cleanup are deadline-bounded.

PortableVm materializes every root and non-root lease volume as a separate
replica-private disk; it never shares or multi-attaches a disk between replica
slots. Mandatory PID-1 `.mount` units are followed by exact
device/filesystem/UUID/options/custody attestation. The tenant unit is bound to
each mount and cannot start on the underlying root directory. Admission permits
at most 32 non-root volumes per replica. Root-private binding sidecars cover
the admitted revision and replica-private disk contract before publication;
initialization is committed only after guest health. Existing volumes are never
reformatted after an unexpected signature or identity failure; interrupted
initialization is accepted only for a blank disk or the exact deterministic
ext4 UUID. Preparation has no
`CAP_SYS_ADMIN`, tenant output and serial capture are disabled, and undeclared
scratch state is ephemeral. The generated scaffold is network-isolated and has
no SSH keys.
PortableVM V1 rejects `Open` and `Allowlist` networking until kernel-owned
counters can meter all guest traffic.

Hosted HTTP responses forwarded over the Torii P2P proxy path are capped by
`torii.soracloud_public_max_response_bytes` before buffering. The default is
64 MiB; over-limit responses fail closed with `502 Bad Gateway`.

The same `--container` plus `--service` manifest pair also works for other
service-bound Soracloud commands such as `hf-deploy`, `hf-status`, `hf-lease-renew`,
`hf-lease-leave`, `training-job-*`, `model-artifact-*`, `model-weight-*`,
`model-upload-register`, and `model-upload-status`. For `status`, the
manifest-pair form also keeps the same local route and workspace-script
projection that `plan` reports. The direct `deploy` and `upgrade`
commands now keep that same local projection in their response as well, and
manifest-pair `config-*`, `secret-*`, `rollback`, `rollout`, HF, training-job,
and model registry/status responses attach it under `service_plan`.

Root-bound single-api app:

```bash
cd .soracloud-docs-portal
./build-and-sync.sh
iroha soracloud app build --manifest ./app_manifest.json --dry-run
```

Root-bound single-api local development:

```bash
cd .soracloud-docs-portal
./dev.sh
iroha soracloud app dev --manifest ./app_manifest.json --dry-run
```

Split app:

```bash
cd .soracloud-hayahi
./build-and-sync.sh
iroha soracloud app build --manifest ./app_manifest.json --dry-run
```

Split app development:

```bash
cd .soracloud-hayahi
./dev.sh
iroha soracloud app dev --manifest ./app_manifest.json --dry-run
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
iroha soracloud service plan --container ./container_manifest.json --service ./service_manifest.json
./build-and-sync.sh
iroha soracloud service build --container ./container_manifest.json --service ./service_manifest.json --dry-run
```

Whole single-api app:

```bash
cd .soracloud-docs-portal
./build-and-sync.sh
iroha soracloud app build --manifest ./app_manifest.json --dry-run
```

Whole split app:

```bash
cd .soracloud-hayahi
./build-and-sync.sh
iroha soracloud app build --manifest ./app_manifest.json --dry-run
```

Inspect the split topology locally:

```bash
iroha soracloud app plan \
  --manifest .soracloud-hayahi/app_manifest.json
```

App-wide sync handles mixed manifests because each
service reference can declare its own `bundle_file`, letting one command
refresh every `bundle_hash` and referenced container manifest hash before
release. The same path also works for single-api apps, where the manifest
tracks `services/api/build/api-service.to`. The plan output also reports
the root `manifest_path`, root `hostname`, and the manifest-adjacent root
scripts that the generated workspace and CLI wrappers use for `dev`,
`build-and-sync`, and `release`.

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
step. `iroha soracloud app release` publishes the declared
`static_site.dist_dir` from the app manifest as part of the single app-wide
release flow. Its response carries the same root
manifest/hostname/workspace metadata, frontend publish projection, and
per-service manifest metadata that `app status` reports.

For shipping deterministic apps on Taira, keep `https://taira.sora.org/` bound
to Torii and use it as:

- the Torii/control-plane base URL
- the SoraFS CID gateway for intentionally CID-only frontends:
  `https://taira.sora.org/sorafs/cid/<cid>`

Use `https://<alias>.mon.taira.sora.net/...` as the Taira public browser URL
for Soracloud apps that already have a vanity alias host. Do not treat
`taira.sora.org` paths as canonical app origins.

## 5. Release

Hosted HTTP service: release only to an explicitly enabled validator with a
currently qualified PortableVM V1 capability.

Single-api app:

```bash
cd .soracloud-docs-portal
export IROHA_BIN=/absolute/path/to/same-revision/iroha
export IROHA_BIN_SHA256="$(sha256sum "$IROHA_BIN" | awk '{print $1}')"
TORII_URL=http://127.0.0.1:8080 ./release.sh
"$IROHA_BIN" soracloud app release --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
```

Split app: its Inrou member requires an explicitly enabled validator with a
currently qualified PortableVM V1 capability. `app plan`, `app build --dry-run`,
and `app dev --dry-run` remain available without a qualified remote host.

Generated root scripts require `IROHA_BIN` to name an absolute, executable,
non-symlinked regular file built from the same revision as the workspace.
`IROHA_BIN_SHA256` must be the exact 64-character lowercase SHA-256 of that
file. The scripts do not search `PATH` or build a source checkout.

In local dev, the scaffolded Vite proxy strips the shared `/api` prefix before
forwarding to the live and vault child processes so the topology can be
exercised locally before release.

The shipping app release flow:

- single-api: rebuilds and republishes the static frontend from `web/dist`, then
  releases `services/api`
- returns the root app `manifest_path`, root `workspace_dir`, root
  `workspace_scripts`, root `hostname`, the top-level app `routes` split, and
  one manifest-derived service entry per app service

## 6. Validate Routing and Runtime State

Check service state:

```bash
iroha soracloud service status --torii-url http://127.0.0.1:8080
```

Check the app-scoped projection for a single-api app:

```bash
iroha soracloud app status \
  --manifest .soracloud-docs-portal/app_manifest.json \
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
- `./dev.sh` serves the frontend locally with `/api` proxied to the local
  API shim
- `iroha soracloud app dev --manifest ... --dry-run` resolves the same
  manifest-adjacent entrypoint from the app manifest
- the frontend uses same-host `/api` calls instead of a hard-coded external base

Expected local planning checks for a split app:

- the dev proxy maps `/api/v1/search*` to the local live API
- the dev proxy maps `/api/auth*` and `/api/v1/user*` to the local vault API
- `iroha soracloud app plan` reports the same route split from the app manifest,
  plus each child service `container_manifest_path`, `service_manifest_path`,
  `workspace_dir`, and discovered child scripts such as `dev.sh`, `build.sh`,
  and `verify-build.sh`
- the live service reports lease-backed volume mounts
- the frontend loads from a SoraFS CID URL and still targets `/api`
- `iroha soracloud app dev --manifest ... --dry-run` resolves the
  same mixed-app entrypoint before execution

## 7. Release a New Revision

After incrementing the app and service versions, run the same mandatory release
path again:

```bash
cd .soracloud-docs-portal
export IROHA_BIN=/absolute/path/to/same-revision/iroha
export IROHA_BIN_SHA256="$(sha256sum "$IROHA_BIN" | awk '{print $1}')"
TORII_URL=http://127.0.0.1:8080 ./release.sh
"$IROHA_BIN" soracloud app release --manifest ./app_manifest.json --torii-url http://127.0.0.1:8080 --dry-run
```

The generated `release.sh` invokes the only first-release app mutation command.
It performs the mandatory build, validation, publication, submission,
authoritative-status reconciliation, and live verification sequence without a
build bypass. Hosted-service and split-app releases require a currently
qualified PortableVM V1 host.

## 8. Operations Checklist

- Keep generated manifests under version control.
- Use `single-api` when one deterministic `/api` service is enough and the
  frontend should stay bound at `/` on the same host.
- Use `./dev.sh` for root-bound single-api iteration, which keeps `/api`
  same-host and proxies it to the local deterministic API shim.
- Use `iroha soracloud app build --manifest ... --dry-run` when
  you want the CLI to resolve the generated root rebuild path from the app manifest.
- Use `./release.sh` for every single-api revision; it runs the complete
  first-release app release path instead of a separate upgrade command.
- Use `http-service` for hosted collectors, SSE, and shared caches, and require
  qualified PortableVM V1 capacity before deployment.
- Use `iroha soracloud service dev --container ... --service ... --dry-run`
  or `iroha soracloud service build --container ... --service ... --dry-run`
  when you want the CLI to resolve the generated hosted-service root scripts
  from the manifest pair first and return the same route and workspace-script
  projection that `plan` reports.
- Keep wallet auth and confidential user state on the IVM plane.
- Persist hosted mutable state through declared `lease_volumes`, not ad hoc
  local directories.
- Use `./dev.sh` for local mixed-app iteration, which proxies `/api/v1/*`
  to the live service and `/api/auth*` plus `/api/v1/user*` to the vault dev
  shim while keeping `VITE_PUBLIC_API_BASE=/api`.
- Use `./build-and-sync.sh` to rebuild a split frontend and inspect its manifest
  hashes before running `./release.sh` against a qualified host.
- Use `iroha soracloud app dev --manifest ... --dry-run` or
  `iroha soracloud app build --manifest ... --dry-run` when you
  want the CLI to resolve the generated root scripts from the app manifest first;
  those dry-run outputs also carry the same child service and route plan that
  `app plan` reports.
- Fail frontend production builds if they point at demo/static data or the
  wrong API base.
- Treat `taira.sora.org` as the Torii/control-plane and CID-gateway host, not
  as an app-alias path router.
- Use `https://<alias>.mon.taira.sora.net/...` for Taira browser examples that
  need ordinary public DNS/TLS.
- Keep the canonical runtime origin on the registered vanity alias host.
- Use SoraFS CID paths only for apps that intentionally publish CID-only
  frontend assets.
