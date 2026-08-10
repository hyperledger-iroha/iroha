# Docker Builder Image

This container is defined in `Dockerfile.build` and bundles all toolchain
dependencies required for CI and local release builds. The image now runs as a
non-root user by default, so Git operations continue to work with Arch Linux’s
`libgit2` package without resorting to the global `safe.directory` workaround.

## Build arguments

- `BUILDER_USER` – login name created inside the container (default: `iroha`).
- `BUILDER_UID` – numeric user id (default: `1000`).
- `BUILDER_GID` – primary group id (default: `1000`).

When you mount the workspace from your host, pass matching UID/GID values so
generated artifacts remain writable:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

The toolchain directories (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`)
are owned by the configured user so Cargo, rustup, and Poetry commands remain fully
functional once the container drops root privileges.

## Running builds

Attach your workspace to `/workspace` (the container `WORKDIR`) when invoking the
image. Example:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

The image keeps the `docker` group membership so nested Docker commands (e.g.
`docker buildx bake`) remain available for CI workflows that mount the host PID
and socket. Adjust group mappings as needed for your environment.

## Canonical Iroha artefacts

The first-release workspace emits the canonical `iroha` client, `irohad`
validator daemon, and standalone `sorafs_governance_dag` service. Run
`make build`, or set a deployment profile explicitly with
`BUILD_PROFILE=deploy bash scripts/build_line.sh`. Deterministic release bundles
and generic runtime images include all three; the Governance service still
requires its exact deployment-owned runtime-provider broker and public config.

Generators, probes, fixture refreshers, and evidence programs are excluded from
ordinary workspace builds. Invoke those targets explicitly with their documented
features, including `dev-tools` where required.

## Runtime Images

The repository-root `Dockerfile` builds the runtime image used for published
`irohad` containers. The supported `CONFIG_PROFILE` values are:

- `single` — embed the default single-node bundle under `/config`
- `nexus` — embed the Nexus sample bundle under `/config`
- `taira` — ship the public Taira static bundle under
  `/opt/iroha/configs/soranexus/taira` and expect a rendered validator config
  to be mounted at `/config/config.toml`

Local Taira image build example:

```bash
cd /path/to/dpn-api-rust
IROHA_DIR=/path/to/iroha ops/taira/build-validator-image.sh
```

Taira image builds must enter through the DPN release wrapper so the ignored
Iroha lockfile and a dirty developer checkout cannot silently become release
inputs. The wrapper reconstructs and verifies the policy-pinned source tree,
installs the reviewed full Cargo lock, and passes both digests to Docker. The
equivalent low-level inputs are shown below for reference, not as a release
procedure:

```bash
docker build \
  --build-arg CONFIG_PROFILE=taira \
  --build-arg FEATURES=embedded-soracloud-runtime \
  --build-arg CARGO_BUILD_JOBS=1 \
  --build-arg BINARIES=iroha3d \
  --build-arg VALIDATOR_LOCK_SHA256=<reviewed-lock-sha256> \
  --build-arg VALIDATOR_SOURCE_TREE_SHA256=<attested-source-tree-sha256> \
  -t hyperledger/iroha:taira-local .
```

The Taira image automatically includes `embedded-soracloud-runtime` and uses a
Taira-aware entrypoint. With no command override it starts:

```bash
iroha3d --sora --config /config/config.toml --genesis-manifest-json /opt/iroha/configs/soranexus/taira/genesis.json
```

Keep validator-specific runtime material out of the image. Generate
`/config/config.toml` with a read-only bind mount; the image entrypoint copies
it to `/storage/runtime-config.toml` before starting `iroha3d`.
`python3 scripts/render_taira_validator_bundle.py --roster ... --secrets ...`
and mount it into the container together with persistent `/storage`.
The runtime image also carries the bundled rANS tables under
`/opt/iroha/codec/rans/tables`, matching the default
`streaming.codec.rans_tables_path`.

For disconnected or one-node smoke starts, mount both a manifest JSON and a
signed genesis payload. The entrypoint accepts `IROHA_TAIRA_SIGNED_GENESIS` and
rewrites the copied runtime config so `genesis.file` points at the mounted
payload path before `iroha3d` starts.

For a local 4-validator container rollout proof, first render a fresh
`kagami localnet` bundle into bridge-friendly configs/env files:

```bash
python3 scripts/render_taira_localnet_container_bundle.py \
  --bundle-dir dist/taira-localnet-smoke \
  --output-dir dist/taira-localnet-cluster
```

Those generated env files set `TAIRA_DOCKER_NETWORK=taira-localnet` so the
existing `taira-validator-container.sh` wrapper can launch all four peers on a
shared Docker bridge with canonical internal `addr:...#CRC16` literals.

For a host-side Taira validator deployment, use the checked-in examples under
`configs/soranexus/taira/`:

- `taira-validator-container.sh`
- `docker-compose.validator.yml`
- `taira-validator-container.compose.env.example`
- `taira-validator-container.service`

Prefer `taira-validator-container.sh` on hosts that only have the base Docker
CLI. Use `docker-compose.validator.yml` only when the Compose plugin is
installed and verified.
