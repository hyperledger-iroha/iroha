# Iroha Daemon (irohad)

The `irohad` crate contains the Iroha server (peer) binary. The binary is used to instantiate a peer and bootstrap an Iroha-based network. The capabilities of the network are determined by the feature flags used to compile the binary.

Pass the `--language <code>` flag to override automatic language detection for informational and error messages.

## Build

**Requirements:** a working [Rust toolchain](https://www.rust-lang.org/learn/get-started) (version 1.93.1), installed and configured.

Optionally, [Docker](https://www.docker.com/) can be used to build images containing any of the provided binaries. Using [Docker buildx](https://docs.docker.com/buildx/working-with-buildx/) is recommended, but not required.

### Build the default Iroha binary

Build the Iroha peer binary as well as every other supporting binary:

```bash
cargo build --release
```

The results of the compilation can be found in `<IROHA REPO ROOT>/target/release/`, where `<IROHA REPO ROOT>` is the path to where you cloned this repository (without the angle brackets).

### Add features

To add optional features, use ``--features``. For example, to add the support for _dev telemetry_, run:

```bash
cargo build --release --features dev-telemetry
```

A full list of features can be found in the [cargo manifest file](Cargo.toml) for this crate.

### Disable default features

By default, the Iroha binary is compiled with the `telemetry`, and `schema-endpoint` features. If you wish to remove those features, add `--no-default-features` to the command.

```bash
cargo build --release --no-default-features
```

This flag can be combined with the `--features` flag in order to precisely specify the feature set that you wish.

### Deployment runtime-provider launcher

`irohad` is also a library target. A deployment-owned binary can use the same
CLI/config/bootstrap path as the stock binaries while supplying HSM, KMS,
WebAuthn, authenticated transport, immutable-query, publication, and sealed
checkpoint adapters:

```rust
use irohad::{BuildLine, IrohaRuntimeProviderRegistryV1};

fn run(registry: &dyn IrohaRuntimeProviderRegistryV1) -> irohad::ReportResult<(), irohad::MainError> {
    irohad::run_with_runtime_provider_registry(BuildLine::Iroha3, registry)
}
```

The registry receives an `IrohaRuntimeProviderBindingsV1` containing only the
chain ID and an ordered set of public slot/handle/revision/policy-digest
bindings. It does not receive the full node configuration, validator key,
provider credentials, API tokens, or private evidence. Registry selection is a
compile-time/launcher decision; the standard launcher has no environment or
config selector that dynamically loads executable provider code.

The stock `iroha2d`, `iroha3d`, and `irohad` binaries do not embed deployment
providers. With the default empty binding catalog they start without external
adapters. With a non-empty catalog they use the stock local-broker client and
fail before subsystem startup if the broker or any exact requested role is
missing, substituted, stale, or unsupported.

There are two supported injection boundaries. A deployment-owned embedding
binary can statically link this crate, keep credentials inside reviewed
provider implementations, and call `run_with_runtime_provider_registry`.
Standard `irohad` startup instead projects the non-secret configured binding
catalog and, when that catalog is non-empty, creates the stock authenticated
local-broker client registry before starting Tokio or node-owned durable state.
An explicitly injected registry remains authoritative. There is no
process-global registry, plugin loader, environment selector, or executable
provider selector in configuration.

The source tree provides `serve_runtime_provider_broker_v1` as an injected
broker-server library boundary, but no checked-in executable calls it and no
HSM, KMS, sealed-store, credential loader, or production backend is packaged.
Deployments using the stock client must therefore supervise a separately
owned broker process that injects every requested backend. Client wiring alone
is not production-adapter or deployment qualification.

Registry resolution itself validates the sanitized binding catalog and rejects
missing or unrequested dependency objects. It cannot independently attest to a
trait object. The following service-owned startup boundaries provide the actual
qualification:

- Moderation quarantine, transparency PRF/release anchors, the Governance DAG
  signer and public-service adapters, appeal finance, moderation runtime,
  the evidence viewer's WebAuthn/grant/signer/erasure providers and
  authoritative checkpoint store, PoP, PoTR, gateway ACME/feed transport,
  reputation publication/delivery, hedging/billing, and the provider-ingest
  source pool, signer resolver, and checkpoint store compare configured or
  deterministically derived public identities and recheck them around use.

- The reputation finalized query is not an injectable registry object. The
  daemon opens the configured bounded archive, performs exact zero-gap
  reconciliation against Kura before Sumeragi starts, applies the configured
  live-lag barrier, and installs that same archive in the v2 apply corridor.
  Every fresh height is captured after Kura finality and the durable WSV
  checkpoint but before live State publication; an archive failure makes the
  committed transition restart-required.

- The provider-ingest completion signer accepts only the configured session
  chain and expected owner, an `Instructions` executable containing exactly one
  non-zero `CompleteReplicationOrder`, and the exact retained signer-policy
  lineage, assignment revision, and finalized anchor. Broker admission and
  request/result validation, plus the durable outbox, reject alternate
  executables, extra instructions, proof attachments, even-empty multisig
  sidecars, invalid signatures, and any context substitution.

- The central launcher converts the four configured proof-outcome, repair,
  reserve/rent, and orderbook identities into immutable Torii bindings, then
  wraps each raw registry provider in a role-specific qualified facade before
  any subsystem receives it. Missing, unexpected, role-confused, substituted,
  stale, test-marked, or drifting providers fail resolution.

- Stream-token signing checks the configured production handle and Ed25519
  public key and verifies every returned signature. Its configuration and
  provider trait do not yet expose an adapter revision or policy digest, so
  independent stale/revoked-provider detection remains a V1 production blocker.

- Provider-ingest source and resolver adapters check independently configured
  production handles, non-zero revisions and public-policy digests, the fixed
  source inventory, bounded readiness, and resolved signer identity. Startup,
  every worker probe, each fetch, and each signer resolution recheck the exact
  role-specific qualification before and after provider work.

Governance DAG IPFS/IPNS authentication, signed-head authentication, and sealed
monotonic checkpoint/publish-intent storage are separate registry slots. When
the service is enabled, irohad qualifies their exact stable handles, revisions,
and public-policy digests before Sumeragi startup, then prepares and supervises
the service from the already resolved config view. The same service boundary
rechecks identity around every authenticated request and sealed CAS operation.
The stock Governance DAG binary likewise has no built-in credential loader;
deployment launchers inject a
`GovernanceDagServiceRuntimeProviderRegistryV1` through the library entrypoint.

Moderation strict ingress and the stream-token identity gap above remain
production blockers; this registry wiring alone is not a claim that the
embedding launcher or a deployment is production-complete.

## Configuration

To run the Iroha peer binary, you must [generate the keys](#generating-keys) and provide a [configuration file](#configuration-file).

### Generating Keys

We highly recommend you to generate a new key pair for any non-testing deployment. We also recommend using the `Ed25519` algorithm. For convenience, you can use the provided [`kagami`](../iroha_kagami/README.md) tool to generate key pairs. For example,

```bash
cargo run --bin kagami -- crypto
```

<details> <summary>Expand to see the output</summary>

```bash
Public key (multihash): "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
Private key (ed25519): "0311152FAD9308482F51CA2832FDFAB18E1C74F36C6ADB198E3EF0213FE42FD8BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
```

</details>

To see the command-line options for `kagami`, you must first terminate the arguments passed to `cargo`. For example, run the `kagami` binary with JSON formatting:

```bash
cargo run --bin kagami -- crypto --json
```

**NOTE**: The `kagami` binary can be run without `cargo` using the `<IROHA REPO ROOT>/target/release/kagami` binary.
Refer to [generating key pairs with `kagami`](../iroha_kagami/CommandLineHelp.md#kagami-crypto) for more details.

### Configuration file

See the current [peer configuration reference](https://docs.iroha.tech/reference/peer-config/params.html)
for the complete parameter list and examples.

## Deployment

You may deploy Iroha as a [native binary](#native-binary) or by using [Docker](#docker).

### Native binary

1. **Build the binaries.**

    ```bash
    cargo build --release -p irohad
    cargo build --release -p iroha_kagami
    ```

2. **Stage a runtime directory.** Copy the release binary and the closest
   configuration template (the Sora Nexus profile ships under `defaults/nexus/`):

    ```bash
    mkdir -p deploy/peer
    cp target/release/irohad deploy/peer/
    cp defaults/nexus/config.toml deploy/peer/config.toml
    cp defaults/nexus/genesis.json deploy/peer/genesis.json
    ```

    Adjust the file layout if you prefer another location. `irohad` resolves
    relative paths from the directory that contains `config.toml`. The checked-in
    Nexus genesis is a schema-valid template, not a deployable public-chain
    identity: regenerate it with the operator-approved canonical Nexus XOR asset
    definition via `--xor-asset-definition-id` before signing. Do not substitute
    Taira's XOR asset ID.

3. **Provision keys and network settings.**

    - Generate a validator key pair with Kagami and capture it in JSON so the
      public/private pair can be pasted into the config and genesis manifests:

      ```bash
      cargo run --release -p iroha_kagami -- \
        crypto --json --algorithm ed25519 \
        --seed "$(uuidgen)" > deploy/peer/validator_keys.json
      ```

    - Update `config.toml` with the new `chain`, `public_key`, and
      `private_key` values, plus the `trusted_peers` you expect in your initial
      topology. Ensure each peer advertises a unique `network.address`/Torii
      port pair.

4. **Generate and sign the genesis block.**

    - Produce a template genesis manifest and tweak it as needed (additional
      accounts, assets, instructions, etc.):

      ```bash
      cargo run --release -p iroha_kagami -- \
        genesis generate default \
        --genesis-public-key <PEER_PUBLIC_KEY> \
        > deploy/peer/genesis.json
      ```

    - Sign the manifest to obtain the Norito block (`.nrt`) that the daemon
      expects:

      ```bash
      cargo run --release -p iroha_kagami -- \
        genesis sign deploy/peer/genesis.json \
        --public-key <PEER_PUBLIC_KEY> \
        --private-key <PEER_PRIVATE_KEY> \
        --out-file deploy/peer/genesis.signed.nrt
      ```

      Then edit `config.toml` so that the `[genesis]` section references the
      signed block:

      ```toml
      [genesis]
      file = "genesis.signed.nrt"
      public_key = "<PEER_PUBLIC_KEY>"
      ```

      See `crates/iroha_kagami/CommandLineHelp.md` and the
      [public genesis reference](https://docs.iroha.tech/reference/genesis.html)
      for additional subcommands such as `validate` and `embed-pop`.

5. **Start an Iroha peer.** Point the daemon at your staged configuration (add
   `--sora` when using the Nexus profile from `defaults/nexus/`):

    ```bash
    cd deploy/peer
    ./irohad --config ./config.toml
    # or, for the Nexus demo profile:
    ./irohad --sora --config ./config.toml
    ```

    Repeat the configuration/key/genesis steps for every peer. Remember that to
    tolerate _f_ Byzantine faults the network must contain at least _3f + 1_
    peers with mutually listed `trusted_peers` entries.

### Docker

We provide a development-only sample configuration in
[`docker-compose.yml`](../../defaults/docker-compose.yml). It contains no
genesis signing key. Create fresh owner-only custody and export only its paths
before evaluating the manifest:

```bash
cargo run --bin kagami -- keys --out-dir target/compose-genesis
export IROHA_GENESIS_PUBLIC_KEY_FILE="$PWD/target/compose-genesis/public.key"
export IROHA_GENESIS_PRIVATE_KEY_FILE="$PWD/target/compose-genesis/private.key"
docker compose -f defaults/docker-compose.yml up --build
```

Compose mounts the verifier key into every peer and the signing key into only
the genesis-submitting peer. Missing files and mismatched keys fail closed. For
a deployed network, generate validator identities and matching
`TRUSTED_PEERS_POP` entries with Kagami rather than inheriting the sample
validator credentials. To keep containers running after closing the terminal,
use the `-d` (*detached*) flag:

```bash
docker compose -f defaults/docker-compose.yml up --build -d
```

- Stop containers:

    ```bash
    docker compose -f defaults/docker-compose.yml stop
    ```

- Remove containers:

    ```bash
    docker compose -f defaults/docker-compose.yml down
    ```
