# Kagami (Teacher and Exemplar and/or Looking glass)

Kagami is a tool used to generate and validate automatically generated data files that are shipped with Iroha.

## Build

From anywhere in the repository, run:

```bash
cargo build --bin kagami
```

This will place `kagami` inside the `target/debug/` directory (from the root of the repository).

> **Enabling optional algorithms:**  
> Kagami inherits cryptographic features from `iroha_crypto`.  
> - `--features gost` exposes the TC26 GOST R 34.10-2012 parameter sets.  
> - `--features ml-dsa` enables ML‑DSA (Dilithium) helpers.  
> - `--features bls` enables BLS validator tooling (default).  
> Combine features as needed, for example:
>
> ```bash
> cargo build --bin kagami --features "bls,gost"
> ```

## Usage
See [Command-Line Help](CommandLineHelp.md).

Quick tip: set IVM block gas budget at genesis
- To pin a consistent per-block gas budget for IVM across peers, Kagami can embed a custom parameter in genesis:
  - Flag: `--ivm-gas-limit-per-block <u64>`
  - Rationale: the node enforces a per‑block gas cap when validating transactions. Baking the value into genesis ensures every peer enforces the same limit, even if compiled defaults differ.
  - Example:
    - `kagami genesis generate --ivm-dir ./ivm_libs --genesis-public-key <MULTIHASH> --ivm-gas-limit-per-block 1680000`
- If omitted, Kagami applies a sensible default: 1,680,000.

## Iroha 3 profile bundles

- Run `cargo xtask kagami-profiles` to emit sample bundles for the `iroha3-dev`, `iroha3-testus`, and `iroha3-nexus` profiles under `defaults/kagami/<profile>/`.
- Each bundle includes:
  - `genesis.json` generated with `kagami genesis --profile ...` and patched with deterministic PoPs/topology.
  - `verify.txt` — stdout from `kagami verify --profile ... --genesis genesis.json`.
  - `config.toml` + `docker-compose.yml` for a single-node smoke run (ports 8080/1337).
  - `README.md` describing the VRF seed and peer material for the profile.
- Flags: `--profile <slug>` limits generation to one profile; `--kagami <path>` points at a prebuilt Kagami binary; `--out <dir>` overrides the output root (default `defaults/kagami`).

## NPoS devnet quickstart
Sora Nexus public dataspaces require NPoS and do not support staged cutovers; omit `--next-consensus-mode`/`--mode-activation-height` when targeting a public dataspace. Other Iroha3 dataspaces may use permissioned or NPoS.
- Bare metal (Iroha3): `kagami localnet --consensus-mode npos --peers <N> --out-dir <dir>` renders peer configs/PoPs. Supply `--seed` to keep the validator roster and PoPs deterministic; edit `genesis.json` if you need to pin `sumeragi_npos_parameters.epoch_seed` for a fixed VRF seed.
- Sora Nexus localnet: `kagami localnet --sora-profile nexus --peers 4 --out-dir ./sora-nexus-local` generates a 4-node NPoS localnet and starts peers with `--sora` so Nexus dataspaces are enabled.
- Sora dataspace localnet: `kagami localnet --sora-profile dataspace --consensus-mode permissioned --peers 4 --out-dir ./sora-ds-local` keeps Sora multi-lane defaults while letting you pick permissioned/NPoS consensus.
- Bare metal (Iroha2 cutover): `kagami localnet --build-line iroha2 --consensus-mode permissioned --next-consensus-mode npos --mode-activation-height <H> --peers <N> --out-dir <dir>` stages an NPoS cutover in genesis and renders peer configs/PoPs.
- `--build-line <iroha2|iroha3>` pins DA/RBC defaults to the target line; use `iroha2` when generating staged cutovers with the `kagami` binary.
- Localnet defaults to a fast pipeline (1.0s total split ~1/3 block and ~2/3 commit), tunes the commit-inflight timeout to match, bumps redundant-send fanout for DA, raises `sumeragi.rbc.chunk_max_bytes` to 256 KiB, lifts queue capacity to 262,144, sets `nexus.fusion.exit_teu = 1,000,000` and `sumeragi.block.proposal_queue_scan_multiplier = 4` to keep proposal assembly bounded, relaxes Torii tx rate limiting (`torii.tx_rate_per_authority_per_sec = 1,000,000`, `torii.tx_burst_per_authority = 2,000,000`, `torii.api_high_load_tx_threshold = 262,144`), and disables Kura fsync (`kura.fsync_mode = "off"`) for local throughput. It also tunes DA availability timeouts for local runs (`sumeragi.da.quorum_timeout_multiplier = 1`, `sumeragi.da.availability_timeout_multiplier = 1`, `sumeragi.da.availability_timeout_floor_ms = 2000`) and writes per-peer `tiered_state` roots under `storage/peerN`. NPoS localnets now seed XOR stake for validators, activate validators in genesis, and raise `parameters.block.max_transactions` to 10,000. Override the pipeline with `--block-time-ms`/`--commit-time-ms` and adjust the config fields when you need slower timings; if you set only one of `--block-time-ms` or `--commit-time-ms`, Kagami mirrors it to the other so the pipeline stays balanced.
- Perf profiles: `--perf-profile 10k-permissioned` and `--perf-profile 10k-npos` pin 1s block/commit timing, set `collectors_k=3` and `redundant_send_r=2`, and apply the 10k tx-per-block cap. The NPoS profile also raises the bootstrap stake to match the configured `min_self_bond`. Use these profiles when running the localnet throughput harness.
- Localnet peer configs allow loopback CIDRs (`127.0.0.0/8`, `::1/128`) through Torii pre-auth and API rate-limit allowlists so demo clients don't hit temporary bans.
- `--bind-host`/`--public-host` accept host/IP only (no port); IPv6 literals are supported and will be bracketed in URLs.
- `start.sh` defaults to `target/debug/irohad`, falls back to `target/release/irohad`, and respects `CARGO_TARGET_DIR` via the generated absolute paths; set `IROHAD_BIN` to override.
- Docker (Iroha3): reuse the same `genesis.json`/configs and run `kagami swarm --consensus-mode npos --peers <N> --config-dir <dir> --image <img> --out-file <compose.yml>`.
- Docker (Iroha2 cutover): reuse the same `genesis.json`/configs and run `kagami swarm --consensus-mode permissioned --next-consensus-mode npos --mode-activation-height <H> --peers <N> --config-dir <dir> --image <img> --out-file <compose.yml>`. The Compose manifest forwards the cutover flags to the signing sidecar so `kagami genesis sign` stamps both `next_mode` and `mode_activation_height`.
- Topology/PoPs: use the “End-to-end: validator PoP and genesis signing” flow to attach `--topology`/`--peer-pop` at sign time; the same PoP/topology bundle can be reused for both localnet and swarm deployments.

## NPoS devnets (local + Docker)

- Bare-metal localnet (NPoS now): `kagami localnet --consensus-mode npos --out-dir ./localnet-npos --peers 4` generates BLS keys/PoPs, stamps NPoS parameters, and writes configs/genesis/scripts. Edit `genesis.json` to set a reproducible VRF seed via `parameters.custom.sumeragi_npos_parameters.epoch_seed` (32-byte array/hex) when you need deterministic epochs.
- Bare-metal localnet (Iroha2 cutover): `kagami localnet --build-line iroha2 --consensus-mode permissioned --next-consensus-mode npos --mode-activation-height 5 --out-dir ./localnet-npos --peers 4` stages `next_mode = npos` at height 5 while keeping the permissioned fingerprint until activation.
- Compose/Docker: generate an NPoS-ready `genesis.json` first (`kagami genesis generate --consensus-mode npos --ivm-dir ./ivm_libs --genesis-public-key <PK>`), then sign it with your roster + PoPs: `kagami genesis sign genesis.json --consensus-mode npos --topology "<peer ids>" --peer-pop "<pk=pop_hex>" --private-key <hex> --out-file genesis.signed.nrt`. For Iroha2 cutovers, add `--next-consensus-mode npos --mode-activation-height 5` (keep `--consensus-mode permissioned`).
- Swarm cutover (Iroha2): `kagami swarm --consensus-mode permissioned --next-consensus-mode npos --mode-activation-height 5 --config-dir ./cfg --image hyperledger/iroha:dev --out-file docker-compose.yml` now refuses to render when `genesis.json` is missing `sumeragi_npos_parameters`, so the compose run always carries the NPoS tuning baked into genesis.
- Topology/PoP reminders: when signing for NPoS, supply a full BLS roster in `--topology` and pair each entry with `--peer-pop` so validators carry Proofs-of-Possession in the final signed block; reuse the same roster when launching swarm/localnet to avoid mismatches.

## End-to-end: validator PoP and genesis signing

## Streaming identities vs. validator keys

Iroha's streaming control plane always signs messages with an Ed25519 key. If your validator identity uses another algorithm (e.g., TC26 GOST), provide a dedicated Ed25519 pair in the node config:

```toml
[streaming]
identity_public_key  = "ed0120..."
identity_private_key = "802620...""
```

`kagami crypto --algorithm ed25519` can generate the pair. When the override is absent, the node reuses its main key.

This section shows how to generate BLS validator keys, produce Proof‑of‑Possession (PoP), embed PoPs into a genesis JSON, and sign the final genesis file.

Prerequisites
- Build Kagami: `cargo build --bin kagami`
- Ensure you compile with `--features bls` for BLS support when needed.
- Keep the genesis private key handy: `kagami genesis sign` now requires `--private-key <hex>` (payload only, no multihash prefix) or `--seed <string> --algorithm <algo>` so the signed block uses the intended authority key.

### 1. Generate consensus keys and PoPs

Use either a seed or a private key. The example below uses seeds and emits JSON.
When using the plain-text mode, Kagami hides the private key unless you pass `--expose-private-key` (freshly generated keys are always shown once so you can store them).

```bash
# Validator A
target/debug/kagami genesis pop --algorithm bls_normal --seed seedA --json > popA.json
# Validator B
target/debug/kagami genesis pop --algorithm bls_normal --seed seedB --json > popB.json
```

The output looks like:

```json
{
  "public_key": "bls_normal:...",
  "pop_hex": "abcd..."
}
```

### 2. Prepare a genesis JSON

Generate a default genesis (or craft one by hand):

```bash
target/debug/kagami genesis generate \
  --ivm-dir ./ivm_libs \
  --genesis-public-key ed25519:... \
  > genesis.json
```

Note: the default template leaves `topology` empty and carries no PoPs. Override these at signing, or embed PoPs inline (`pop_hex` alongside each peer) before signing with `embed-pop`.

### 3A. Embed PoPs directly into the genesis JSON (optional pre‑sign step)

If you prefer to materialize PoPs in the manifest before signing:

```bash
# Extract public_key/pop from JSON
PK_A=$(jq -r .public_key popA.json)
POP_A=$(jq -r .pop_hex popA.json)
PK_B=$(jq -r .public_key popB.json)
POP_B=$(jq -r .pop_hex popB.json)

# Write a topology array (PeerIds) that references these public keys.
# Example topology.json content:
# ["$PK_A", "$PK_B"]

topology_json="[\"$PK_A\",\"$PK_B\"]"
echo "$topology_json" > topology.json

# Embed PoPs matching the topology into a new genesis file
target/debug/kagami genesis embed-pop \
  --manifest genesis.json \
  --out genesis_popped.json \
  --peer-pop "$PK_A=$POP_A" \
  --peer-pop "$PK_B=$POP_B"
```

Now `genesis_popped.json` has `topology` entries, each carrying a `pop_hex` aligned with its peer.

### 3B. Alternatively: pass topology + PoPs at sign time

When you sign, you can specify a topology and per‑peer PoPs in one go; Kagami will embed `pop_hex` alongside each topology entry automatically for the signing transaction.

```bash
TOPOLOGY=$(jq -c . topology.json)  # compact JSON ["pkA","pkB"]

target/debug/kagami genesis sign \
  genesis.json \
  --topology "$TOPOLOGY" \
  --peer-pop "$PK_A=$POP_A" \
  --peer-pop "$PK_B=$POP_B" \
  --private-key "$GENESIS_PRIVATE_KEY_HEX" \
  --algorithm ed25519 \
  --out-file genesis.scale
```

Either 3A or 3B yields the same final effect: the signed genesis encodes `pop_hex` alongside the chosen topology.

### 4. Verify and use

- Optionally run `kagami genesis validate genesis.json` to check identifiers.
- Distribute the signed `genesis.scale` to all peers.
- Ensure your `config.toml` lists validator BLS-Normal keys under `trusted_peers` **and** provides matching PoPs under `trusted_peers_pop`. The loader rejects `trusted_peers_bls` mappings and any validator without a PoP; genesis-embedded `pop_hex` entries can satisfy the same requirement if you prefer manifest-only PoPs.
- Using BLS validator keys does not change transaction admission defaults: `allowed_signing` still defaults to Ed25519/secp256k1 for accounts. Only add `bls_normal` to `allowed_signing`/`allowed_curve_ids` if you plan to accept BLS-signed transactions; consensus will function with BLS validators even when admission stays Ed25519-only.


## Examples
- [codec](docs/codec.md)
- [kura](docs/kura.md)
- [swarm](docs/swarm.md)
