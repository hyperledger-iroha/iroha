# Taira

Taira is SORA's persistent public testnet. The public Torii MCP endpoint is
`https://taira.sora.org/v1/mcp`; public validators and observers join that
shared network rather than creating a replacement cohort. The public-node join
contract must consume one published, signed Taira bootstrap bundle (network
identity, genesis anchor, seed peers, permissionless observer policy, on-chain
validator activation policy, and upgrade policy) plus locally generated node
keys. Runtime credentials and signing inputs stay outside the repository.
Until that bundle and the single supported node-init command are shipped, do
not present the local harness below as a public-network join procedure.

The repository's disposable four-validator harness is a local qualification
network. It is not the public Taira network and is not part of ordinary public
node onboarding. Its path is one command with an explicit prepared Inrou guest
workspace:

```bash
python3 scripts/taira_devnet.py up \
  --inrou-canary-dir /private/runtime/taira-inrou-canary
```

It builds the current `kagami`, `iroha3d_taira`, `iroha`, and `sorafs-node`
binaries, replaces
the previous script-owned bundle under `/var/lib/iroha-taira-devnet/` by
default, generates exactly four fresh-key NPoS validators for the canonical
Taira chain, with Kagami directly owning the exact storage and closed egress
profile. It validates every base configuration, then the compiled
`iroha taira inrou-stage` command stages the trusted guest and, through the
required `--bind-validator-config-dir`, atomically binds each peer to the
complete first-release Inrou backend: one
PortableVM with exact CPU, memory, writable-storage, and egress budgets plus a
separate 10 GiB immutable guest-image materialization bound. It
starts the peers and waits for all four nodes to become ready, which also proves
that each daemon passed the artifact-free Inrou startup-boundary probe. That
probe exercises the production machine type and host CPU under KVM, private
namespaces, configured cgroup limits, anonymous QMP, QEMU user networking, the
private loopback connector, and the owner firewall. The command then stages and
preseeds the required guest, boots four isolated workload replicas, and proves
their authoritative public route. It also
submits one signed `iroha tx ping`, waits for its typed `Applied` status,
requires all four committed heights to advance and converge, and checks that
every generated MCP endpoint can initialize and list tools. The fixed
`local-release` build explicitly targets the native Rust host triple, clears
ambient target-dir, target, compiler/wrapper, incremental, and build-identity
environment overrides, and selects only
`<target-dir>/<triple>/local-release` outputs from a direct owner-controlled
tree disjoint from the disposable network. It never accepts prebuilt binaries.
It records the exact `optimizations` HEAD plus a collision-safe pre/post
observation of the tracked diff and non-ignored untracked files. That observation
is a race detector, not proof of which source Cargo consumed. It requires all
four live validator build identities to match the observed HEAD and target, the
CLI to report the same HEAD, hashes every selected executable before the cohort
is replaced and after qualification, and fails if the observation or binary
evidence changes.

The JSON names this record `source_observation`, scopes it to the Git HEAD,
tracked diff, and non-ignored untracked entries, and sets
`cargo_source_consumption` to `not_proven`. Ignored files, Cargo configuration,
build-script inputs outside the worktree, the toolchain, and dependency caches
are outside that observation. Exact source provenance belongs to the separate
signed immutable release corridor.

`up` is an Inrou startup-boundary and guest-workload qualification command. It
fails before building
or replacing a cohort unless the host is Linux AArch64, the command starts as
uid 0, and `/dev/kvm` exposes KVM API version 12. The daemon then remains the
authority for the root-custodied runtime closure, locked service identities,
private namespaces, cgroup-v2 limits, anonymous QMP, and firewall posture.
Provision the four canonical same-host identity slots before running it:

- `iroha-inrou-0`, uid/gid `70000`
- `iroha-inrou-1`, uid/gid `70001`
- `iroha-inrou-2`, uid/gid `70002`
- `iroha-inrou-3`, uid/gid `70003`

Public validators run on separate hosts and use slot 0. These accounts are
locked execution identities only; the command does not provision accounts or
persist deployment credentials.

### Prepare the fixed Inrou host runtime

On each native AArch64 Linux validator, install packages that provide direct,
root-owned, single-link executables at these exact paths:

- `/usr/bin/qemu-system-aarch64`
- `/usr/bin/setpriv`
- `/usr/bin/ldd`
- `/usr/bin/bwrap`
- `/usr/bin/nsenter`
- `/usr/bin/socat`

The QEMU and `setpriv` ELF interpreters and dynamic-library closure must also be
root-custodied and non-writable by group/other. Create the fixed parent once,
then run the packager from the `optimizations` checkout as root:

```bash
sudo install -d -o root -g root -m 0755 /opt/iroha
sudo -- python3 scripts/ci/package_inrou_runtime_v1.py
```

The packager has no destination option. It atomically creates the previously
absent `/opt/iroha/inrou-runtime-v1/` with `root/` and `manifest.sha256`, and
fails if that destination already exists. Its only source overrides are
canonical absolute `--qemu`, `--setpriv`, and `--ldd` paths; this Taira AArch64
posture uses the defaults.

The daemon startup boundary additionally requires direct root-custodied
`/usr/bin/qemu-img`, root-custodied `mke2fs` at `/usr/sbin/mke2fs` or
`/sbin/mke2fs`, one root-custodied `iptables` executable at
`/usr/sbin/iptables`, `/sbin/iptables`, `/usr/bin/iptables`, or `/bin/iptables`,
`/dev/kvm` with API version 12, and unified cgroup v2 with the `cpu`, `io`,
`memory`, and `pids` controllers available. Kernel namespace, QEMU user-network
listener/private-connector, QMP, firewall owner-match, and cgroup controls are
exercised by the bounded startup probe; `up` fails closed if any is unavailable.
This artifact-free probe does not boot a guest or verify the workload loopback
bridge.

New non-root Inrou lease volumes use the first-release canonical ext4 profile:
their byte budgets must be positive multiples of 128 MiB. The daemon ignores
host `mke2fs.conf` policy, supplies the complete format geometry and feature
set explicitly, derives a stable UUID from the service revision, volume kind,
storage class, and authoritative generation, and validates that exact
superblock contract before publishing or reusing a disk.

Every successful run must prove a real guest launch, four placements, and the
public route. Prepare verified AArch64 assets, generate the exact deploy
workspace with the same-revision compiled CLI, and pass that workspace to the
devnet:

The asset preparer requires `gpgv` or `gpg` plus a trusted Debian archive or
cloud-image keyring. Install `debian-archive-keyring`, set
`DEBIAN_ARCHIVE_KEYRING`, or pass `--debian-keyring`; a missing
`SHA512SUMS.sign` is fatal, and the archive must match both the authenticated
Debian sums and the repository-pinned SHA512.

```bash
TAIRA_RUST_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
cargo build --locked --profile local-release --target "$TAIRA_RUST_TARGET" \
  -p iroha_cli --bin iroha

eval "$(python3 scripts/ci/prepare_inrou_portable_guest_assets.py \
  --output-dir /private/runtime/taira-inrou-assets \
  --print-env)"

target/"$TAIRA_RUST_TARGET"/local-release/iroha taira inrou-workspace \
  --kernel "$IROHA_INROU_PORTABLE_KERNEL_IMAGE" \
  --rootfs "$IROHA_INROU_PORTABLE_ROOTFS_IMAGE" \
  --initrd "$IROHA_INROU_PORTABLE_INITRD_IMAGE" \
  --output-dir /private/runtime/taira-inrou-canary \
  --json

python3 scripts/taira_devnet.py up \
  --inrou-canary-dir /private/runtime/taira-inrou-canary
```

The `--output-dir` must not exist: `inrou-workspace` creates one direct,
effective-user-owned mode `0700` directory and never reuses it. It emits only
the exact deploy-mode `container_manifest.json`, `service_manifest.json`, and
deterministic embedded-Python `bundle.tgz`, plus mode `0700`
`inrou/aarch64/` directories containing direct, single-link mode `0600`
`vmlinux`, `rootfs.ext4`, and `initrd.img` copies. Every emitted file is
effective-user-owned mode `0600`. The compiled generator validates the final
bundle with the canonical Taira canary validator before reporting success.

Keep both asset and canary directories runtime-only, outside the repository
and disjoint from the disposable `--dir` tree and qualification Cargo target.
Every canary-path ancestor must be direct, owned by root, and non-writable by
group/other. Do not substitute generated fixtures, fallback filenames, or
placeholder guest images. The devnet rejects symlinks, empty files, permissive
modes, extra or missing tree members, oversized assets, and workspace overlap
before it mutates its managed tree. It pins every file identity and SHA-256,
revalidates the workspace before replacing the cohort, copies it through
no-follow descriptors into an owner-only network-local snapshot, and makes the
compiled stager consume only that snapshot. The final JSON reports the
aggregate `inrou_canary_input_content_sha256` without exposing input paths.

The mandatory path builds `sorafs-node`, invokes the compiled
`iroha taira inrou-stage --mode deploy --bind-validator-config-dir ...`, and
verifies both its exact owner-only stage and the four typed daemon configs it
rewrote. The command rejects any pre-existing Inrou table; there is no Python
TOML writer, idempotent reuse, or compatibility binding path. Before starting
a validator, it preseeds the service bundle, guest
directory, and public discovery commitments into each of the four disjoint
generated SoraFS roots. After signed finality and the four
MCP checks, the coordinator executes four prepared Inrou children in order:
`bundle-pin` (`inrou_bundle_pin`), `guest-pin` (`inrou_guest_pin`),
`discovery-pin` (`inrou_discovery_pin`), then `service-mutation`
(`inrou_canary`). Each invocation selects exactly one child and one of
prepare, retained-envelope submit, or read-only recovery. The coordinator
atomically persists the canonical authorization-bound envelope before one
submit, never replaces first-wins bytes, and requires exact Applied predecessor
evidence before preparing the next child. The final service mutation proves
exactly four active host adverts, four hosted replicas, the canonical
authoritative route, and four distinct routed replica identities. The final
JSON reports a redacted `inrou_canary` outcome; it never reports the stage path
or copies credentials into repository files. A successful report always sets
`inrou_guest_workload_qualification` to `verified`; there is no startup-only
success shape. It atomically publishes an owner-only exact-schema
`inrou_guest_qualification.json` record inside the disposable network for
subsequent read-only checks. That record binds the exact qualifying CLI path,
digest and byte length plus the source revision and native target triple. The
report also uses
`configured_inrou_vm_capacity_per_peer` and
`inrou_startup_boundary_qualified_peers` for the separately proven startup
boundary.

Each of those four hosted replicas receives its own root and non-root lease
disks. The canary does not share or multi-attach a disk between replica slots,
and common filenames or matching guest paths are not evidence of shared
storage.

There is no external signed release ceremony, evidence archive, promotion
state, 24-hour soak, host service installation, or predecessor rollback in
this disposable path. `up` records a stable in-run worktree observation and
binds the exact binaries it executes, but reports Cargo source consumption as
`not_proven`.

## Daily commands

Inspect the running cohort without writing to it:

```bash
python3 scripts/taira_devnet.py check
```

`check` binds the listeners to the generated Taira chain, genesis hash,
loopback ports, and four exact owner-only `peerN.process.json` V1 identities.
Each identity pins the Linux boot, process start time, executable path/device/inode,
exact argv/config, UID/GID, session, and process group; unrelated services or a
reused numeric PID cannot satisfy it. It reads the Torii base port from the generated
`client.toml`, so an `up` started with a custom `--base-api-port` needs no
repeated port argument. It also requires and strictly validates the owner-only
V1 guest qualification record, including the canonical four-replica canary
receipt and input digest. It rehashes the retained input snapshot, requires the
recorded `optimizations` revision and Linux/AArch64 target on every validator,
rehashes and executes only the recorded qualifying CLI, revalidates the exact
retained stage, and invokes one `iroha taira inrou-check --mode deploy`. The
compiled check performs an account-signed status read, compares the live
container and service manifest hashes with the stage, and observes all four
route identities. The report labels the historical mutation result
`inrou_stored_deploy_receipt` and the current result `inrou_live_check`; it
never presents the stored receipt as fresh evidence. It remains read-only: it
does not repeat KVM qualification, submit a ping, register an artifact, or
submit a canary deployment.

Stop it and destroy the complete generated network:

```bash
python3 scripts/taira_devnet.py down
```

Every `up`, `check`, and `down` holds one exclusive lock on the managed marker.
Taira lifecycle control is Linux-only and requires native `pidfd_open`,
`pidfd_send_signal`, pollable pidfds, and procfs. Startup, restart, inspection,
and teardown reopen and hold a pidfd before observing or signaling a process;
signals and exit waits use only that pidfd. There is no `ps`, PID signal, or
shell-kill fallback. Bare `peerN.pid` files are retired and rejected without
migration. Teardown returns success only after every exact process record and
matching process is gone and the pinned cleanup-directory identity is unchanged. It
atomically moves that exact inode to a private cleanup name, proves the identity
again, then removes configs, logs, state, runtime signers, and onboarding
material together. If either proof fails, the bundle (or quarantined racing
replacement) is retained for diagnosis and the command fails instead of
deleting unproven ownership evidence.

Optionally run the broader read-only public-product route diagnostic after the
standard signed smoke and four-peer MCP checks:

```bash
python3 scripts/taira_devnet.py up \
  --inrou-canary-dir /private/runtime/taira-inrou-canary \
  --full-doctor
```

`--full-doctor` runs the same-revision `iroha taira doctor` against the
generated local endpoint after the mandatory real Inrou canary. It adds the
broad public-product route diagnostic; it does not replace any guest workload
qualification step.

The optional local diagnostic is not public-ingress qualification and is never
a default devnet gate. Run the same-revision `iroha taira doctor` directly
against a public ingress when qualifying that deployment.

The dedicated daemon's config validation, help, and version commands are
offline introspection surfaces: they never open or consume the inherited
runtime-signer descriptor. Every node-starting invocation still requires the
exact descriptor and compiled Taira profile.

The output directory is owner-only and contains private keys and runtime
tokens. Never commit, print, upload, or archive it. On failure the command
prints bounded peer log tails, attempts bounded teardown, and destroys the
bundle after proving shutdown and directory identity. If either proof fails, it
warns and retains the complete bundle for operator diagnosis instead of
claiming cleanup.

## Public reset

The same-revision compiled CLI is the single public-reset path. Build the
evidence binary with the release profile and admit the complete input closure
locally before any host is contacted:

```bash
cargo build --locked --profile release -p iroha_cli --bin iroha
target/release/iroha taira public-reset preflight \
  --inventory /private/runtime/taira-public-reset/inventory.json \
  --authorization /private/runtime/taira-public-reset/authorization.json \
  --trusted-public-key /private/runtime/taira-public-reset/trusted-public-key.json \
  --ssh-identity /private/runtime/taira-public-reset/id_ed25519 \
  --known-hosts /private/runtime/taira-public-reset/known_hosts
```

`InventoryV1` must contain `canary_onboarding_request`; it is not optional and
has no derived-at-runtime fallback. The value must be the exact canonical
`AccountOnboardingPlanRequestV1`: version 1, the canonical domainless
single-signatory canary account, its deterministically derived rollout alias in
the `taira.universal` scope, and an empty `permissions` array. The inventory
SHA-256 covered by the signed authorization binds this complete request before
admission, so neither an operator nor a resumed controller can substitute the
account, alias, or permissions during prepare. Preflight rejects a missing,
noncanonical, mismatched, or permission-bearing request.

The inventory must also contain `faucet_policy` with the exact canonical
single-signatory faucet `authority`, resolved Base58 `asset_definition_id`, and
positive fixed `amount` from the rendered Taira configuration. The signed
authorization repeats and binds this policy. Prepared faucet envelopes are
accepted only when their signer, transfer asset, amount, fee closure, and
instruction bytes all match these independently admitted values; no value is
learned from the envelope being authenticated.

`iroha taira public-reset preflight` performs local fail-closed admission;
`iroha taira public-reset apply` is the live mutating operation. Apply requires
explicit owner-private, runtime-only authorization, SSH, and canary inputs. It
is permitted only after the identical artifact closure passes the disposable
four-validator corridor and each admitted host already has the trusted compiled
dispatcher and reset guard provisioned independently of the candidate. Never
persist those inputs in the repository, let the candidate bootstrap its own
host authority, or introduce a Python alias or parallel V1 schema.

The rendered validator configuration must replace the dedicated
`REPLACE_WITH_TAIRA_CANARY_ONBOARDING_*` fields with one credential scoped to
the `universal` dataspace. Its token digest must match the owner-only token
admitted by the reset closure; the raw token never enters the release bundle or
repository.

## Public Taira endpoint checks

The compiled CLI owns the current public API contract. Build it from the same
revision being deployed. The read-only doctor deliberately does not load a
client config or signing identity:

```bash
cargo build --locked --profile release -p iroha_cli --bin iroha
target/release/iroha \
  taira doctor --public-root https://taira.sora.org --json
```

The public doctor remains non-mutating and does not impersonate an operator.
It requires the exact two-field operator-signature `401` from
`/v1/sumeragi/status` and the exact two-field canonical-account `401` from
`/v1/soracloud/status`; arbitrary gateway challenges fail closed. Exact runtime
topology and four-replica Inrou convergence belong to the signed Inrou canary,
not the public route-posture probe.

Maintained clients may perform one bounded, credential-free
`GET /v1/kagemusha/readiness` and must reject redirects. A ready deployment
advertises only the sole `KagemushaV1` aggregate-balance protocol and its
authenticated proof and hardware profiles. The readiness schema has no hop,
origin, ancestry, input-count, note-count, or proof-depth capability field.

`ready=true` describes the universal KAGEMUSHA peer-cash protocol surface; it does not
assert that a particular asset has a promoted proof release or operational
command authority. Use the signed KAGEMUSHA V1 rollout evidence before attempting
top-up or redemption. Override the probe origin only with the credential-free
HTTPS origin in `IROHA_TAIRA_PUBLIC_ROOT`.
The Taira rollout asset is Digital Shekel `7ZepsJTHCVLKsrFFNZGSRGZgvBhv`
(`ds#boi.is`, scale 2); XOR `6TEAJqbb8oEPmLncoNiMRbLEK6tw` (scale 9) remains
the transaction-fee asset.

The offline `taira inrou-stage` command assigns the canary an immutable
`artifact-<digest>` service version derived from the complete canonical bundle
with only the version field cleared. Final guest publication references are
therefore part of the revision identity. A staged directory and receipt bind
the service manifest, container manifest, materialized bundle, and both SoraFS
manifests; changing any input produces a different revision. Its required
`--bind-validator-config-dir` must name the owner-only directory containing
exactly `peer0.toml` through `peer3.toml`; all four base configs must be fresh,
must match the staged placement set, and must not already contain an Inrou
table.
`--sorafs-retention-epoch` is a required nonzero absolute Unix-second boundary;
the receipt and both manifests bind it exactly, and a retry must reuse the same
value to reproduce the original manifest digests. Every manifest carries that
same value in both the pin policy and its sole metadata entry,
`soracloud.retention_epoch=<epoch>`; extra metadata is rejected. A retry reuses
an exact `Approved` record, waits for an exact `Pending` record, and registers
only a `Missing` record.

The signed `taira inrou-canary` mutation path checks authoritative SoraCloud
state before publishing either staged SoraFS manifest. `deploy` requires the
service to be absent. `upgrade` requires it to exist at a different immutable
revision; replaying the current staged revision fails before upload. That
preflight produces a mandatory signed compare-and-set condition: deploy binds
service absence, while upgrade binds the exact current version, service and
container manifest hashes, positive process generation, and the current config
and secret generations. The ledger checks the condition atomically in the same
transaction that admits the new revision, so revision or material drift after
preflight cannot become a lost update. Process, config, and secret generations
use checked monotonic increments; an exhausted counter fails closed and never
wraps or saturates into a replayable token.

An Inrou upgrade is an atomic 100% revision replacement. It cannot supersede an
active rollout or change the service execution plane, container runtime, or
route identity. The admitted revision becomes the sole current revision; no
baseline, split-traffic, rollback-serving, or other compatibility revision is
kept active.

After the explicit mutation, convergence requires the exact current/latest
version, both staged manifest hashes, four placements, and a positive process
generation on every status poll. A failed or changed status generation discards
all collected route evidence. Each accepted health response must also carry Torii-owned
served-service, served-version, replica-slot, process-generation, and
materialized-bundle headers. Torii only stamps those headers for an
authoritatively healthy placement whose host capability is valid, unexpired,
and matches the validator, peer, backend, and guest ISA, and whose exact bundle,
generation, and snapshot peer identity match the node-local process. Local-only
health or an expired host advert never substitutes for authoritative state.
Both local and remote ingress overwrite upstream values, so guest self-reporting
or a stale process cannot satisfy the proof.

An explicitly authorized public write canary is an ordered durable protocol,
not a one-shot command. `iroha taira public-reset apply` prepares, privately
persists, submits, and recovers the `onboarding`, `faucet`, and `final-canary`
children in that order. The low-level `iroha taira write-canary` command accepts
one child and one of `--prepare-envelope`, `--submit-prepared-envelope-fd`, or
`--recover-prepared-envelope-fd`; later preparation also requires the exact
Applied predecessor envelope. Do not invoke it manually unless implementing or
auditing that coordinator protocol. Keep the populated example client config
and owner-only onboarding-token file in the admitted runtime workspace.
The faucet child additionally requires `--faucet-authority`,
`--faucet-asset-id`, and `--faucet-amount`; these must come from the signed
inventory and never from a prepared response.

Do not persist signing keys, onboarding tokens, bearer tokens, or forwarded
authorization headers in this repository.

## Retained source-coupled assets

- `config.toml` and `genesis.template.json` are canonical profile sources
  consumed by compiled Kagami/config/genesis tests. The genesis source omits
  operator-owned mint-finality authority, is not a raw or signable manifest,
  and is not an input to the disposable generator.
- `privacy_bootstrap_plan.json` and `privacy_rollout_plan_v1.json` remain
  coupled to Kagami's compiled privacy bootstrap feature. The V1 rollout does
  not carry caller-authored assurance or availability claims. It admits a wave
  only when the authenticated committed Exact12 manifest reports every one of
  the twelve rows as `production-qualified`; missing release, audit, security,
  or deployment evidence therefore halts rollout.
- `dns_records.json`, `explorer.runtime-config.json`, `sorafs_sites.json`, and
  `taira-canary-client.example.toml` describe the live public profile. The
  Explorer runtime config carries the exact genesis-derived `NETWORK_ID`, the
  fixed public Torii origin, and `toriiForceBaseUrl: true`; retired feature
  flags are not accepted by the first-release Explorer.
- `validator_roster.example.toml`, the edge renderer, nginx template, and edge
  installer remain the public-ingress configuration surface. The production
  `taira-explorer.sora.org` TLS vhost serves only the Explorer release symlink
  at `/Users/administrator/dev/iroha2-block-explorer-web/dist`; it does not
  proxy `/status` or `/v1`. Both Torii CORS and the public-edge CORS map admit
  the exact `https://taira-explorer.sora.org` browser origin.

The edge installer validates with the fixed production executable
`/usr/sbin/nginx`. Dry runs may run unprivileged; installation and reload must
run as root into a root-owned include directory. Installed configuration is
published atomically as root-owned mode `0644`. Install and reload runs hold an
exclusive owner-only `.taira-edge-install.lock` in that directory for the full
render, validation, publication, reload, and rollback transaction. The installer
validates a private snapshot, requires the published bytes to match that exact
fingerprint, and rechecks content and metadata before and after live validation
and reload. Rollback never overwrites a changed target; it retains the
owner-only recovery copy for explicit operator handling. Executable overrides
and Homebrew-specific target discovery are intentionally not supported.

The retired Python reset, release, evidence, host-supervision, and soak
controllers are intentionally gone. The compiled `iroha taira public-reset`
preflight/apply pair is the sole reset surface; there is no compatibility alias
or parallel schema. Keep `scripts/taira_devnet.py` limited to disposable
process orchestration and end-to-end smoke verification.
