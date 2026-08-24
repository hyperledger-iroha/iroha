# Taira

Taira's disposable testnet path is one command:

```bash
python3 scripts/taira_devnet.py up
```

It builds the current `kagami`, `iroha3d_taira`, and `iroha` binaries, replaces
the previous script-owned bundle under `/var/lib/iroha-taira-devnet/` by
default, generates exactly four fresh-key NPoS validators for the canonical
Taira chain, validates every
peer configuration, and overlays each peer with the sole first-release Inrou
backend: one PortableVM with exact CPU, memory, storage, and egress budgets. It
starts the peers and waits for all four nodes to become ready, which also proves
that each daemon passed the artifact-free Inrou startup-boundary probe. That
probe exercises the production machine type and host CPU under KVM, private
namespaces, configured cgroup limits, anonymous QMP, QEMU user networking, the
private loopback connector, and the owner firewall; it does not boot a guest or
launch a workload. It then
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

The default devnet is an Inrou startup-boundary qualification command. It fails before building
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
`/usr/bin/qemu-img`, one root-custodied `iptables` executable at
`/usr/sbin/iptables`, `/sbin/iptables`, `/usr/bin/iptables`, or
`/bin/iptables`, `/dev/kvm` with API version 12, and unified cgroup v2 with the
`cpu`, `io`, `memory`, and `pids` controllers available. Kernel namespace,
QEMU user-network listener/private-connector, QMP, firewall owner-match, and
cgroup controls are exercised by the bounded startup probe; `up` fails closed
if any is unavailable. This artifact-free probe does not boot a guest or verify
the workload loopback bridge.

To prove a real guest launch, four placements, and the public route, prepare
verified AArch64 assets, generate the exact deploy workspace with the
same-revision compiled CLI, and pass that workspace to the devnet:

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

This opt-in path conditionally builds `sorafs-node`, invokes the compiled
`iroha taira inrou-stage --mode deploy`, and verifies its exact owner-only
stage. Before starting a validator, it preseeds both the service bundle and
guest directory commitments into each of the four disjoint generated SoraFS
roots. After signed finality and the four MCP checks, compiled
`iroha taira inrou-canary --mode deploy` requires exactly four active host
adverts, four hosted replicas, the canonical authoritative route, and four
distinct routed replica identities. The final JSON reports a redacted
`inrou_canary` outcome; it never reports the stage path or copies credentials
into repository files. Without this option, that field is
`{"status":"not_requested"}`. The default report uses
`configured_inrou_vm_capacity_per_peer` and
`inrou_startup_boundary_qualified_peers`; those fields do not claim guest boot,
the workload bridge, placements, or the public route.

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
loopback ports, and the four exact PID/config pairs; unrelated services on the
same ports cannot satisfy it. It reads the Torii base port from the generated
`client.toml`, so an `up` started with a custom `--base-api-port` needs no
repeated port argument. Its JSON reports configured peer/capacity values and
current read-only health only: it does not requalify KVM, source or executable
identity, signed writes, or the Inrou canary route.

Stop it while retaining generated configs and logs:

```bash
python3 scripts/taira_devnet.py down
```

Teardown returns success only after every managed PID file and matching peer
process is gone. If that cannot be proved, the bundle is retained for diagnosis
and the command fails instead of deleting its ownership evidence.

Optionally run the broader read-only public-product route diagnostic after the
standard signed smoke and four-peer MCP checks:

```bash
python3 scripts/taira_devnet.py up --full-doctor
```

`--full-doctor` runs the same-revision `iroha taira doctor` against the
generated local endpoint. It is independent of `--inrou-canary-dir`; combine
the two flags only when both broad route diagnostics and the real Inrou canary
are part of the same run.

The dedicated daemon's config validation, help, and version commands are
offline introspection surfaces: they never open or consume the inherited
runtime-signer descriptor. Every node-starting invocation still requires the
exact descriptor and compiled Taira profile.

The output directory is owner-only and contains private keys and runtime
tokens. Never commit, print, upload, or archive it. On failure the command
attempts bounded teardown, keeps the bounded peer logs in place, and exits
non-zero. If peer ownership or termination cannot be proved, it warns and
retains the bundle for operator diagnosis instead of claiming cleanup.

## Public reset inventory handoff

`scripts/taira_public_reset.py` is a local, strictly read-only inventory and
operator-handoff validator. It validates an owner-private exact four-host
inventory, source and artifact hashes, validator preflight attestations, and the
edge-authority attestation, then emits redacted JSON. `preflight` and `confirm`
do not grant deployment authority; `apply` always refuses after validation.

The script opens no network connection, launches no transport or subprocess,
writes or deletes no file, installs no artifact, stops no service, and does not
reload edge ingress. It is not a deployment controller. Public Taira deployment
remains outstanding until an external exact four-host Linux/AArch64/KVM
inventory, runtime-only credentials, and a compiled authenticated replay-safe
executor are available and the same artifact closure passes the disposable
four-validator canary, public convergence/write/route checks, four distinct
Inrou replica receipts, restart proof, and controlled edge cutover.

## Public Taira endpoint checks

The compiled CLI owns the current public API contract. Build it from the same
revision being deployed, copy the example config to an owner-only runtime path,
and replace its key placeholders before use:

```bash
cargo build --locked --profile local-release -p iroha_cli --bin iroha
target/local-release/iroha -c /private/runtime/client.toml \
  taira doctor --public-root https://taira.sora.org --json
```

For an explicitly authorized public write canary, copy the example client
configuration to a runtime-only location and supply the onboarding token from
an owner-only runtime file:

```bash
target/local-release/iroha -c /private/runtime/client.toml \
  --fee-payer authority \
  taira write-canary \
  --public-root https://taira.sora.org \
  --onboarding-token-file /private/runtime/onboarding.token \
  --write-config /private/runtime/canary-client.toml \
  --json
```

Do not persist signing keys, onboarding tokens, bearer tokens, or forwarded
authorization headers in this repository.

## Retained source-coupled assets

- `config.toml` and `genesis.json` are canonical profile fixtures consumed by
  compiled Kagami/config/genesis tests. They are not inputs to the disposable
  generator.
- `privacy_bootstrap_plan.json` and `privacy_rollout_plan_v1.json` remain
  coupled to Kagami's compiled privacy bootstrap feature. The V1 rollout keeps
  all twelve protocols retained-required, but records ZK-ACE, ZK-AMS, Vega, and
  ZK-X509 as unavailable; `retained-protocol-unavailable` therefore halts the
  exact-12 rollout until their independent release gates close.
- `dns_records.json`, `explorer.runtime-config.json`, `sorafs_sites.json`, and
  `taira-canary-client.example.toml` describe the live public profile.
- `validator_roster.example.toml`, the edge renderer, nginx template, and edge
  installer remain the public-ingress configuration surface.

The retired privileged reset, release, evidence, host-supervision, and soak
controllers are intentionally gone. `scripts/taira_public_reset.py` is retained
only as the read-only inventory/handoff boundary described above. New mutating
deployment behavior belongs in a compiled authenticated Kagami, daemon, or CLI
surface; keep `scripts/taira_devnet.py` limited to disposable process
orchestration and end-to-end smoke verification.
