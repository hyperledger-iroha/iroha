# Kagami

Kagami is the task-first operator toolbox shipped with Iroha. Use it for guided
peer setup, disposable local devnets, Docker Compose generation, genesis work,
validator key material, and lower-level inspection utilities.

## Build

From anywhere in the repository, run:

```bash
cargo build --bin kagami
```

This places `kagami` in `target/debug/` from the repository root.

Optional crypto features come from `iroha_crypto`:

- `--features gost` enables the TC26 GOST R 34.10-2012 parameter sets
- `--features ml-dsa` enables ML-DSA helpers
- `--features bls` enables BLS validator tooling

Example:

```bash
cargo build --bin kagami --features "bls,gost"
```

## Help

- Full generated CLI reference: [CommandLineHelp.md](CommandLineHelp.md)
- Regenerate the help snapshot: `cargo run -p iroha_kagami -- advanced markdown-help > crates/iroha_kagami/CommandLineHelp.md`

## Quickstart

New local devnet, guided:

```bash
kagami localnet-wizard
```

Existing Sora network / observer peer config, guided:

```bash
kagami wizard --profile nexus
```

Direct disposable localnet, permissioned by default:

```bash
kagami localnet --peers 4 --out-dir ./localnet
```

Direct NPoS localnet:

```bash
kagami localnet --consensus-mode npos --peers 4 --out-dir ./localnet-npos
```

Docker Compose from one authoritative prepared bundle:

```bash
kagami localnet \
  --fresh-random-keys \
  --peers 4 \
  --out-dir ./localnet
kagami docker \
  --peers 4 \
  --config-dir ./localnet \
  --image hyperledger/iroha:dev \
  --out-file docker-compose.yml
docker compose -f docker-compose.yml up
```

Ed25519 or BLS keys:

```bash
kagami keys --algorithm ed25519
kagami keys --out-dir ./key-custody
kagami keys --algorithm bls_normal --pop --json
```

`--out-dir` is the production-oriented form: it creates a mode-`0700`
directory containing newline-terminated `public.key` and owner-only
`private.key` files, refuses to reuse a non-empty directory, and never prints
the private key.

The generator commands print a concise summary with generated paths and the
next safe handoff. `localnet` and `wizard` also emit a generated `README.md`
into the output directory.

## Main Flows

`kagami localnet-wizard`
- Guided disposable devnet flow
- Prompts for peer count, profile, consensus mode, ports, sample assets, and
  output directory
- Defaults the output to the canonical OS temporary directory so owner-only
  custody checks do not traverse platform temporary-directory symlinks
- Writes genesis, signed genesis, its exact hash, per-peer configs,
  `client.toml`, `start.sh`, `stop.sh`, and a generated guide
- Generated stop scripts validate pidfiles against the expected peer config
  path before signalling a live process, so stale or reused pids are left alone.

`kagami wizard`
- Guided observer-onboarding flow for an existing Sora Nexus or Taira network;
  use `localnet-wizard` for a new generic network
- Supports interactive and fully flag-driven non-interactive use
- Requires the operator-authenticated full validator peer/PoP roster encoded by
  the network's signed genesis; the generated local peer is not promoted to validator
- Stages `config.toml`, a reference `genesis.json` manifest, and a generated
  guide that requires the network-authoritative signed genesis block and exact
  hash before showing the final `iroha3d` launch step

`kagami localnet`
- Bare-metal local network generator
- Requires at least four peers so generated networks use a representative
  revision-4 committee with mandatory RS16 data availability
- Protects validator/client configs and runtime signer/token sidecars with
  owner-only permissions and emits a bundle-wide `.gitignore`
- Emits `genesis.signed.nrt`, `genesis.public_key`, and
  `genesis.expected_hash` as a cross-checked runtime bundle. The latter contains
  one canonical checked `hash:<64 uppercase hex>#<CRC16>` NetworkId literal.
  An owner-only `genesis.private_key` is never mounted by generated Compose files
- Fresh-custody bundles keep directories and lifecycle scripts at `0700`, all
  other files at `0600`, and lifecycle scripts enforce `umask 077` for new
  logs, pidfiles, and runtime state
- Defaults to `permissioned` unless a Sora profile or perf preset requires
  `npos`
- `--sora-profile nexus` enforces public-dataspace rules and requires `npos`

`kagami docker`
- Docker Compose generator for an authoritative prepared bundle from
  `kagami localnet` (or equivalent peer configs plus signed genesis artifacts)
- Normal mode omits `--seed`: Kagami parses every `peerN.toml` without ambient
  environment overrides, rejects `extends`, and verifies the exact signed
  genesis, manifest, expected hash, verifier key, validator identities, trusted
  roster, and PoPs as one binding. It does not generate replacement validator
  identities.
- Kagami proves that each container-safe projection preserves the Sumeragi,
  execution-policy, and Nexus/AMX fingerprints, mounts the projected TOML as a
  file-backed Compose secret, and passes its BLAKE3 digest to `irohad` for a
  read-hash-parse startup check. Validator keys and private onboarding/faucet
  files are absent from Compose YAML and environment variables; the latter are
  mounted as separate Compose secrets.
- Byte-exact public policy assets are interned by digest as base64 Compose
  configs and decoded into `/config/runtime` before `iroha3d` starts. Prepared
  Compose accepts fresh state only, uses named validator storage volumes, never
  migrates live state, resolves relative source-state paths and omitted
  defaults against the prepared bundle directory for freshness checks, and
  fails closed on unsupported transport, CIDR-filter, or helper-service modes.
- `--seed` is an explicit deterministic development mode for relocatable sample
  manifests. That mode requires `IROHA_GENESIS_SIGNED_FILE`,
  `IROHA_GENESIS_PUBLIC_KEY_FILE`, and `IROHA_GENESIS_EXPECTED_HASH_FILE` when
  Compose is evaluated; those artifacts must match the seeded validator roster.

`kagami genesis`
- Power-user genesis generation, PoP embedding, validation, normalization, and
  signing helpers

`kagami verify`
- Profile-aware genesis verification for shipped Iroha 3 profiles

`kagami advanced`
- Low-level helpers that are not part of the main onboarding path:
  `client-configs`, `codec`, `kura`, `schema`, and `markdown-help`

## Iroha 3 Profiles

- Run `cargo xtask kagami-profiles` to emit sample bundles for
  `iroha3-dev` and `iroha3-nexus` under
  `defaults/kagami/<profile>/`
- Each bundle includes:
  - `genesis.json`
  - `verify.txt`
  - `config.toml`
  - `docker-compose.yml`
  - `README.md`
- For a disposable four-validator Taira deployment, use
  `python3 scripts/taira_devnet.py up`; use its `check` and `down` subcommands
  for inspection and teardown. The low-level `iroha3-taira` Kagami profile
  remains available for manifest generation and verification, targets the live
  Taira chain id, requires NPoS, and requires `--vrf-seed-hex`.

See [specs/kagami_profiles.md](../../specs/kagami_profiles.md) for
the profile-specific defaults.

## Validator PoP and Genesis Signing

Generate BLS validator keys and PoPs:

```bash
target/debug/kagami genesis pop --algorithm bls_normal \
  --seed-hex 5151515151515151515151515151515151515151515151515151515151515151 \
  --json > popA.json
target/debug/kagami genesis pop --algorithm bls_normal \
  --seed-hex 5252525252525252525252525252525252525252525252525252525252525252 \
  --json > popB.json
```

Generate a genesis JSON:

```bash
target/debug/kagami genesis generate \
  --ivm-dir ./ivm_libs \
  --genesis-public-key ed25519:...
```

Sign with topology and PoPs:

```bash
target/debug/kagami genesis sign \
  genesis.json \
  --topology "$TOPOLOGY_JSON" \
  --peer-pop "$PK_A=$POP_A" \
  --peer-pop "$PK_B=$POP_B" \
  --private-key-file "$GENESIS_PRIVATE_KEY_FILE" \
  --expected-public-key "$GENESIS_PUBLIC_KEY" \
  --algorithm ed25519 \
  --out-file genesis.signed.nrt \
  --expected-hash-out genesis.expected_hash
```

The one-line `genesis.expected_hash` output is the deployment trust root. It
carries the exact signed header hash as one canonical checked NetworkId literal.
Production templates select that same byte-exact file through validator
`genesis.expected_hash_file` and client `network_id_file`; do not copy the value
into independently rendered inline settings.

For seedless `kagami docker`, place that body and checked network identity beside the canonical
`genesis.public_key` and exact `peerN.toml` validator configs. Generation rejects
any signer, hash, identity, trusted-roster, or PoP disagreement. The generated
validator-only Compose projection rewrites operational paths to container
storage. Configured account-onboarding and faucet private-key files become
file-backed Compose secrets, while public binary policy inputs become
digest-interned Compose configs. The original bare-metal configs are unchanged.

## Streaming Identities

Iroha's streaming control plane always signs messages with an Ed25519 key. If a
validator uses another algorithm for its main identity, configure a dedicated
Ed25519 streaming identity:

```toml
[streaming]
identity_public_key  = "ed0120..."
identity_private_key = "802620..."
```

Use `kagami keys --algorithm ed25519` to generate that pair.

## Advanced Examples

- [Norito codec](docs/codec.md)
- [Kura block inspection](docs/kura.md)
- [Docker Compose generation](docs/swarm.md)
