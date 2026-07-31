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

These direct localnet commands are scaffolding examples. With mandatory offline
cash enabled, signing intentionally fails until the generated manifest and peer
configuration are provisioned with the externally reviewed release described
below. Kagami does not synthesize or silently substitute a local V4 catalog.

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
- Defaults the output to the canonical OS temporary directory so owner-only
  custody checks do not traverse platform temporary-directory symlinks
- Writes genesis, signed genesis, per-peer configs, `client.toml`, `start.sh`,
  `stop.sh`, and a generated guide
- Generated stop scripts validate pidfiles against the expected peer config
  path before signalling a live process, so stale or reused pids are left alone.

`kagami wizard`
- Guided peer/bootstrap flow for configuring a node against an existing profile
- Supports interactive and fully flag-driven non-interactive use
- Writes `config.toml`, `genesis.json`, and a generated guide with the exact
  `irohad` launch command

`kagami localnet`
- Bare-metal local network generator
- Requires at least four peers so generated networks use a representative
  DA/RBC topology
- Protects validator/client configs and runtime signer/token sidecars with
  owner-only permissions and emits a bundle-wide `.gitignore`
- Fresh-custody bundles keep directories and lifecycle scripts at `0700`, all
  other files at `0600`, and lifecycle scripts enforce `umask 077` for new
  logs, pidfiles, and runtime state
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
  --config peer0.toml \
  --topology "$TOPOLOGY_JSON" \
  --peer-pop "$PK_A=$POP_A" \
  --peer-pop "$PK_B=$POP_B" \
  --private-key "$GENESIS_PRIVATE_KEY_HEX" \
  --algorithm ed25519 \
  --out-file genesis.signed.nrt
```

### Mandatory offline release bootstrap

`kagami genesis sign` requires `--config`; it refuses to emit a deployment
genesis unless that peer configuration names at least one offline
asset-to-escrow binding, the funded Kagemusha command issuer, a positive decode
budget, and absolute paths to an authenticated ABI-21/V4 release policy and
artifact directory. Kagami loads and authenticates that catalog before staging
genesis, then calls Core's authoritative mandatory-offline evaluator over the
exact staged height-one block. Signing fails unless the issuer is registered,
permissioned, and funded for the effective fee asset; every escrow binding and
fixed-scale asset agrees; the governed hardware policy is active; and the
authenticated release exposes all five distinct verifier roles with the exact
ABI-21/V4 recursive proof backend. `irohad` calls the same evaluator after Kura
replay and before networking, so signing and runtime admission cannot drift.

No reviewed V4 catalog is checked into this repository, and localnet must not
substitute generated test keys. Produce the externally mounted release through
the sealed workflow:

1. Have the separately installed root provisioner publish the complete source,
   bootstrap, Rust sysroot/dynamic-library, linker, GPG/keyring, and tool
   closure from a root-only staging directory into its root-owned,
   content-addressed final path. The root supervisor then runs the admitted
   Python builder under the exclusive no-login `boi-build` UID with a target
   inaccessible to the operator UID. Root stable-hashes/copies the worker
   binary and report from descriptors and atomically publishes the exact
   three-file artifact set with independently pinned
   `root-published-candidate-build.json`. A mutable checkout, user-owned target
   or DMG, Rustup/Homebrew executable, or ambient keyring is not a production
   input or cross-stage artifact.
2. As the root supervisor, prove the receipt-named no-login `boi-build` UID is
   otherwise idle, then launch
   `scripts/run_kagemusha_v4_generation.py ... generate-candidate` under that
   exact UID through a sanitized `/usr/bin/env -i` environment. Supply the
   root-published candidate-build receipt and receipt SHA-256, exact
   receipt-named binary, reviewed chain, asset, scale, activation window,
   circuit parameters, and top-up finality roster. The launcher creates and
   exclusively uses one previously absent output parent containing direct
   `candidate` and `resource-report` children; it rejects a pre-existing or
   reusable parent.
3. Keep that candidate and its reports as
   `provisional_boi_generation_worker_output` only. Root must keep the worker
   UID quarantined, stable-hash and copy all bytes from `O_NOFOLLOW`
   descriptors, and atomically publish the exact normalized tree with
   `generation-worker-launch.json` and
   `root-published-generated-candidate.json`. The generated root contains
   exactly those two files plus the direct `candidate/` and
   `resource-report/` directories. The launch schema is
   `boi.taira.generation_worker_launch.v1`; the independently pinned generated
   receipt schema is
   `iroha.kagemusha.root_published_generated_candidate.v1`. Together they bind
   the canonical command digest, worker root/device/inode, storage admission
   and post-build reserve, candidate-build receipt, successful generation
   summary, tree digests, UID, and source/toolchain identity.
4. Run the strict Python finalization path with that generated-candidate receipt
   and its independent SHA-256 pin under `/usr/bin/env -i`, the receipt-named
   binary, and
   `--candidate-dir <root-published-generated>/candidate`. It admits the
   complete generated and launch receipts, performs a fail-closed loader scan,
   and passes both immutable receipt descriptors to `finalize-release`. Their
   SHA-256 identities are committed to the final manifest, signed attestation
   subject, and promotion record. A binary invocation without those admitted
   descriptors, or any provisional worker path, is rejected. Supply the
   canonical release policy, signed release attestation, benchmark evidence,
   and signed cryptographic review.
5. Configure the final policy and artifact paths in
   `[settlement.offline]`, and configure the issuer under
   `[torii.kagemusha_commands]`.
6. In genesis, register and fund that issuer with the exact
   `CanManageOfflineEscrow` permission; register the offline asset at fixed
   scale with `offline.enabled=true`; register the transfer, top-up-shield, and
   unshield verifier records and ZK bindings; grant the release activation and
   device-policy permissions; and append the catalog-built release activation
   at height 1.
6. Sign with `--config`, then run the validator's full offline readiness check.
   Do not admit any validator until every peer reports the same artifact
   identity and `ready:true`.

The complete guarded generator and finalization syntax is embedded in
[`kagemusha_recursive_spend_v4_bundle.rs`](../iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs).

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
