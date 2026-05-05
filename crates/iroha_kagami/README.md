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

Existing network / peer config, guided:

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

Docker Compose from an existing config/genesis directory:

```bash
kagami docker \
  --peers 4 \
  --config-dir ./localnet \
  --image hyperledger/iroha:dev \
  --out-file docker-compose.yml
```

Ed25519 or BLS keys:

```bash
kagami keys --algorithm ed25519
kagami keys --algorithm bls_normal --pop --json
```

The generator commands print a concise summary with generated paths, the primary
Torii URL, and exact next commands. `localnet` and `wizard` also emit a
generated `README.md` into the output directory.

## Main Flows

`kagami localnet-wizard`
- Guided disposable devnet flow
- Prompts for peer count, profile, consensus mode, ports, sample assets, and
  output directory
- Writes genesis, signed genesis, per-peer configs, `client.toml`, `start.sh`,
  `stop.sh`, and a generated guide

`kagami wizard`
- Guided peer/bootstrap flow for configuring a node against an existing profile
- Supports interactive and fully flag-driven non-interactive use
- Writes `config.toml`, `genesis.json`, and a generated guide with the exact
  `irohad` launch command

`kagami localnet`
- Bare-metal local network generator
- Defaults to `permissioned` unless a Sora profile or perf preset requires
  `npos`
- `--sora-profile nexus` enforces public-dataspace rules and requires `npos`

`kagami docker`
- Docker Compose generator for an existing config directory containing
  `genesis.json`
- Use this after `kagami localnet` or after preparing/signing genesis manually

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
  `iroha3-dev`, `iroha3-taira`, and `iroha3-nexus` under
  `defaults/kagami/<profile>/`
- Each bundle includes:
  - `genesis.json`
  - `verify.txt`
  - `config.toml`
  - `docker-compose.yml`
  - `README.md`
- `iroha3-taira` and `iroha3-nexus` require `--vrf-seed-hex` when generating
  or verifying NPoS manifests

See [docs/source/kagami_profiles.md](../../docs/source/kagami_profiles.md) for
the profile-specific defaults.

## Validator PoP and Genesis Signing

Generate BLS validator keys and PoPs:

```bash
target/debug/kagami genesis pop --algorithm bls_normal --seed seedA --json > popA.json
target/debug/kagami genesis pop --algorithm bls_normal --seed seedB --json > popB.json
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
  --private-key "$GENESIS_PRIVATE_KEY_HEX" \
  --algorithm ed25519 \
  --out-file genesis.signed.nrt
```

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
