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

## Iroha 2 vs Iroha 3 artefacts

The workspace now emits separate binaries per release line to avoid collisions:
`iroha3`/`iroha3d` (default) and `iroha2`/`iroha2d` (Iroha 2). Use the helpers to
produce the desired pair:

- `make build` (or `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) for Iroha 3
- `make build-i2` (or `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`) for Iroha 2

The selector pins the feature sets (`telemetry` + `schema-endpoint` plus the
line-specific `build-i{2,3}` flag) so Iroha 2 builds cannot accidentally pick up
Iroha 3-only defaults.

Release bundles built via `scripts/build_release_bundle.sh` pick the correct binary
names automatically when `--profile` is set to `iroha2` or `iroha3`.

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
scripts/build_release_image.sh --profile iroha3 --config taira
```

The Taira helper now defaults to `CARGO_BUILD_JOBS=1` and `BINARIES=irohad`
so validator-image builds do not depend on unrelated `iroha_cli` health. For a
direct Docker build, pass those explicitly:

```bash
docker build \
  --build-arg CONFIG_PROFILE=taira \
  --build-arg FEATURES=embedded-soracloud-runtime \
  --build-arg CARGO_BUILD_JOBS=1 \
  --build-arg BINARIES=irohad \
  -t hyperledger/iroha:taira-local .
```

The Taira image automatically includes `embedded-soracloud-runtime` and uses a
Taira-aware entrypoint. With no command override it starts:

```bash
irohad --sora --config /config/config.toml --genesis /opt/iroha/configs/soranexus/taira/genesis.json
```

Keep validator-specific runtime material out of the image. Generate
`/config/config.toml` with
`python3 scripts/render_taira_validator_bundle.py --roster ... --secrets ...`
and mount it into the container together with persistent `/storage`.

For a host-side Taira validator deployment, use the checked-in examples under
`configs/soranexus/taira/`:

- `taira-validator-container.sh`
- `docker-compose.validator.yml`
- `taira-validator-container.compose.env.example`
- `taira-validator-container.service`

Prefer `taira-validator-container.sh` on hosts that only have the base Docker
CLI. Use `docker-compose.validator.yml` only when the Compose plugin is
installed and verified.
