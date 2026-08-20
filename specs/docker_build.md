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
`BUILD_PROFILE=deploy bash scripts/build_canonical_binaries.sh`. Deterministic release bundles
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
  `/opt/iroha/configs/soranexus/taira` and expect an operator-owned validator
  config to be mounted at `/config/config.toml`

The build always uses the tracked workspace lockfile with `cargo --locked`.
Feature selection is explicit; `CONFIG_PROFILE=taira` no longer changes the
feature graph, binary list, source tree, or build profile behind the caller's
back. For a hosted Soracloud runtime image, request the real runtime feature:

```bash
docker build \
  --build-arg CONFIG_PROFILE=taira \
  --build-arg FEATURES=embedded-soracloud-runtime,external-software-signer-bin \
  -t hyperledger/iroha:taira-local .
```

The Taira-aware entrypoint starts:

```bash
iroha3d --sora --config /config/config.toml --genesis-manifest-json /opt/iroha/configs/soranexus/taira/genesis.json
```

Keep validator-specific runtime material out of the image. Mount the exact
operator-owned `/config/config.toml` read-only; the entrypoint copies it to
`/storage/runtime-config.toml` before starting `iroha3d`.
The runtime image also carries the bundled rANS tables under
`/opt/iroha/codec/rans/tables`, matching the default
`streaming.codec.rans_tables_path`.

For disconnected or one-node smoke starts, mount both a manifest JSON and a
signed genesis payload. The entrypoint accepts `IROHA_TAIRA_SIGNED_GENESIS` and
rewrites the copied runtime config so `genesis.file` points at the mounted
payload path before `iroha3d` starts.

For a disposable local four-validator Taira network, do not build a custom
container corridor. Use the same-revision bare-metal smoke:

```bash
python3 scripts/taira_devnet.py up
python3 scripts/taira_devnet.py down
```
