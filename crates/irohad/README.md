# Iroha Daemon (irohad)

The `irohad` crate contains the `iroha3d` Iroha server (peer) binary. The binary is used to instantiate a peer and bootstrap an Iroha-based network. Portable, release-qualified production capabilities are compiled into the default daemon; runtime configuration controls deployment policy.

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

### Add specialized features

To add optional features, use ``--features``. For example, to add the support for _dev telemetry_, run:

```bash
cargo build --release --features dev-telemetry
```

A full list of features can be found in the [cargo manifest file](Cargo.toml) for this crate. Explicit features are reserved for platform accelerators, preview providers, profiling/developer tooling, release evidence, and test/fault-injection lanes that cannot form one portable production build.

### Disable default features

By default, the Iroha binary selects the `daemon` aggregate. It includes the portable Core and Torii production surfaces, full Halo2/STARK proof support, GOST and SM algorithms, event and metrics telemetry, schema endpoints, DAG recovery verification, HTTPS/WSS webhooks, and the bounded app/MCP API surface. To construct a deliberately reduced specialist library, disable the aggregate explicitly.

```bash
cargo build -p irohad --release --no-default-features --lib
```

This flag can be combined with the `--features` flag in order to precisely specify the feature set that you wish.

### Deployment runtime-provider launcher

`irohad` is also a library target. A deployment-owned binary can use the same
CLI/config/bootstrap path as the stock binary while supplying HSM, KMS,
WebAuthn, authenticated transport, immutable-query, publication, and sealed
checkpoint adapters:

```rust
use irohad::IrohaRuntimeProviderRegistryV1;

fn run(registry: &dyn IrohaRuntimeProviderRegistryV1) -> irohad::ReportResult<(), irohad::MainError> {
    irohad::run_with_runtime_provider_registry(registry)
}
```

The registry receives an `IrohaRuntimeProviderBindingsV1` containing only the
display/routing chain ID, the exact genesis-derived `NetworkId` for daemon
catalogs, and an ordered set of public slot/handle/revision/policy-digest
bindings. Standalone services that never sign or validate transactions may
omit `NetworkId`. It does not receive the full node
configuration, validator key, provider credentials, API tokens, or private
evidence. Registry selection is a compile-time/launcher decision; the standard
launcher has no environment or config selector that dynamically loads
executable provider code.

The stock `iroha3d` binary does not embed deployment providers. With the default
empty binding catalog it starts without external adapters. With a non-empty
catalog it uses the stock local-broker client and fails before subsystem
startup if the broker or any exact requested role is
missing, substituted, stale, or unsupported.

An enabled Musubi provider-attestation journal projects its combined durability
seal, approval-only signer, and authenticated coordinator inventory as three
independent public bindings in exact slots 57, 58, and 59. Slot 57 exposes
separate authenticated small-record namespaces for the monotonic UNIX-time
floor and checkpoint head, plus immutable content-addressed checkpoint blobs;
one qualification covers that complete durability contract. The binding
catalog and resolved dependency set are all-or-none and contain no endpoint,
credential, token, or private key. Registry resolution snapshots each exact
production handle/revision/policy digest before and after qualification, but it
does not call readiness or perform a durability, signing, or inventory effect.
The stock broker does not implement these roles.

These slots remain inert. Private daemon wrappers now pin the signer to its
configured adapter, chain/genesis/provider context, and finalized
`State::provider_owners()` value, and pin inventory calls and returned data to
their configured adapter and exact chain/genesis/archive/order scope, with
put/get restricted to the local provider. Neither wrapper is installed or
supervised. Because the stock broker does not support slots 57--59, ordinary
stock launch fails during pre-Tokio provider resolution. If an injected registry
resolves and qualifies the three roles, the shared `start_with_runtime_deps`
activation gate still rejects the journal before supervisor startup.

The inert capture foundation exposes request minting only through a doc-hidden
`NodeHandle` method. Its clone-shared marker identifies one process-local handle
incarnation with storage and an ingest outbox; restart creates a fresh marker.
The marker owns a non-resetting atomic take guard across handle clones. The
prepared daemon archive exposes its signed capture reader only as one movable
concrete value, and a private composer consumes it into a doc-hidden
non-generic coordinator retained on `Iroha`. Acquisition is reader-inert and
lazy binding retries that same reader/session after height-zero bootstrap; the
coordinator exposes no public operational surface and starts no child.
Same-head suppression is scanner-lifetime only. Reconciliation now performs a
qualified exact slot-59 read after fresh request verification and before
enqueue: an existing valid item with the exact request payload suppresses
admission, absence proceeds, and conflict or qualification failure fails closed.
Journal enqueue remains idempotent only while its key is retained because
delivered rows may be capacity-pruned. Concrete combined durability, signer,
and inventory adapters, broker readiness, effect-driver ownership, supervision,
and fault/chaos/platform qualification remain separate gates. Deployment must
also enforce one rooted
journal session for each exact external provider scope across machines, or an
equivalent authenticated provider-side session fence; the local OS lease
coordinates only processes sharing one state root.

The journal's raw checkpoint/CAS types, abstract store, transition engine, and
runtime constructor are crate-private, as is checkpoint-head orchestration. The
public root-fenced file store exposes no raw load/CAS operation. On Linux and
macOS its explicit initialization path proves the local cache empty and installs
the canonical empty external `H0`; ordinary open requires an existing `H0` or
later head, never initializes from local bytes, and consumes the store only
after matching chain/genesis/provider plus the exact retained journal-policy
digest. Canonical domain-separated hashes bind that scope and every
predecessor-linked head record.

A mutation exactly reads back an immutable content-addressed checkpoint blob,
then the external head CAS, before advancing the local two-slot cache. The
external head/blob remains authoritative after local rollback: only an exact
direct predecessor proved by the retained predecessor record and blob can be
repaired forward; deeper rollback, ahead/fork, missing, or substituted state
fails closed. The separate sealed time floor bounds the checkpoint timestamp.
The external-to-local window is protected by a nonblocking process and
cross-process lease on the two-slot initialization lock whose exact identity is
committed in the immutable slot headers. Contention returns unavailable,
cancellation releases the lease, and exact retry completes recovery. Other
platforms fail closed, and this runtime remains inert and unwired in stock
`irohad`.

There are two supported injection boundaries. A deployment-owned embedding
binary can statically link this crate, keep credentials inside reviewed
provider implementations, and call `run_with_runtime_provider_registry`.
Standard `irohad` startup instead projects the non-secret configured binding
catalog and, when that catalog is non-empty, creates the stock authenticated
local-broker client registry before starting Tokio or node-owned durable state.
An explicitly injected registry remains authoritative. There is no
process-global registry, plugin loader, environment selector, or executable
provider selector in configuration.

When finalized moderation is enabled, that same catalog projection also
qualifies the configured strict-ingress handle, revision, and public-policy
digest against Torii's fixed V1 ingress binding. This happens before the stock
broker client or an injected registry is invoked, and therefore before Tokio
or node-owned durable state exists. The in-process ingress is intentionally not
an external broker slot; missing, substituted, stale, zero-qualified, or
test-marked configuration fails the common launcher preflight, while the live
adapter remains requalified at Torii construction and around every operation.

The source tree provides `RuntimeProviderBrokerDeploymentV1` as the standard
deployment assembly around the injected `serve_runtime_provider_broker_v1`
server boundary. `RuntimeProviderBrokerExecutableV1` adds the common process
shell: a one-argument `RuntimeProviderBrokerExecutableArgsV1` CLI, secure
bounded canonical-catalog loading, redacted failures, supervisor-owned
readiness/lifecycle hooks, and SIGINT/SIGTERM shutdown. Its
`RuntimeProviderBrokerBackendRegistryV1` receives only the sanitized non-empty
public catalog, and the assembled launch performs exact live server
qualification before readiness.
The server accepts canonical non-empty client subsets of that catalog so the
stock daemon and packaged standalone services can share the fixed endpoint;
the handshake requires the same exact genesis-derived `NetworkId` and every
binding byte-for-byte, and a session cannot invoke a provider outside its
authenticated subset. The packaged `sorafs_governance_dag` launcher therefore
requires both `--chain-id` and `--network-id`. Deployment launchers can
handoff that projection without sharing `actual::Config` by calling
`IrohaRuntimeProviderBindingsV1::export_canonical_v1`; the broker side loads it
with `load_canonical_v1`, or uses
`load_runtime_provider_broker_catalog_file_v1` for the process shell's secure
absolute-path handoff. The explicitly versioned canonical Norito artifact is
bounded, non-empty, strictly ordered, and contains only the chain identity plus
the mandatory exact `NetworkId`, public handles, identities, revisions, bounds,
and policy digests already held by the sanitized projection. The common CLI has
no socket override, plugin, private-key, credential, or test-provider argument;
Linux and macOS use the platform-fixed authenticated endpoint, while Windows
and other platforms fail before catalog filesystem access because V1 has no
equivalent authenticated transport.

No checked-in binary or registry supplies vendor HSM, KMS, WebAuthn, sealed
store, network, or immutable-query implementations. Under the current static
injection architecture, a deployment must link its reviewed concrete registry
into a thin owned binary that parses `RuntimeProviderBrokerExecutableArgsV1`
and calls `RuntimeProviderBrokerExecutableV1`; credentials remain inside those
provider objects. A generic in-tree binary would first require an explicitly
approved and versioned authenticated provider-plugin/IPC ABI, which V1 does not
define. Client wiring, the common executable shell, or an empty/dummy registry
alone is not production-adapter or deployment qualification.

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

- Stream-token signing checks the configured production handle, Ed25519 public
  key, non-zero adapter revision, and public-policy digest.
  Both startup qualification probes are individually identity-fenced, and every signing
  operation rechecks the exact qualification and handle/key binding before and
  after the external call before verifying the returned signature.

- Provider-ingest source and resolver adapters check independently configured
  production handles, non-zero revisions and public-policy digests, the fixed
  source inventory, bounded readiness, and resolved signer identity. Startup,
  every worker probe, each fetch, and each signer resolution recheck the exact
  role-specific qualification before and after provider work.

Governance DAG Kubo request authentication, signed-HTTP head CAS
authentication, and sealed monotonic checkpoint/publish-intent storage are
separate required registry slots. When the service is enabled, irohad qualifies
their exact stable handles, revisions, and public-policy digests before Sumeragi
startup, then prepares and supervises the service from the already resolved
config view. The Kubo and head adapters must each return a live
`GovernanceDagRequestIngressQualificationV1` matching the exact configured
`GovernanceDagRequestIngressBindingV1`, exposed by
`ipfs_request_ingress_binding()` and `head_request_ingress_binding()`. The only
accepted ingress contract is an exclusive authenticated receiver backed by one
shared sealed atomic replay namespace for the complete replica set through
envelope expiry. The service rechecks identity around every authenticated
request and sealed CAS operation.

The publisher uses one fixed Kubo UnixFS profile, locally derives every expected
CID, and publishes the public head only through signed-HTTP strong-ETag CAS.
Its mirror retains the protocol-fixed suffix of at most 65,536 blocks and 512
MiB of canonical source bytes. A sealed intent owns the derived mirror candidate;
checkpoint/source recovery must reproduce its exact digest. All referenced Kubo
objects are verified or repaired before the public-head CAS, each checkpoint
generation receives a full first audit, and later polls rotate through the
retained objects. A missing post-CAS pin or object is restored from authenticated
bytes only when it reproduces the same deterministic CID.

Preparation also yields the service-owned authenticated mirror-read capability.
The launcher installs it into the embedded `NodeHandle` exactly once, before
spawning the service task or sharing the first node clone. Installation checks
the logical and retained physical producer root, the configured and retained
signer identity/qualification/peer/key, the sealed-checkpoint-store binding,
the reader-retained service-state root, and the existing typed mirror store.
Any mismatch is startup-fatal. Every mirror read authenticates the current typed
store and sealed checkpoint under the runner's readiness epoch; reconciliation
failure or runner exit withdraws all retained readers. Torii reads publication,
runtime, and mirror authority through these path-free typed snapshots.
The stock Governance DAG binary likewise has no built-in credential loader;
deployment launchers inject a
`GovernanceDagServiceRuntimeProviderRegistryV1` through the library entrypoint.

The stream-token source-side identity gap is closed. Production readiness still
requires a genuine deployment-owned signer matching that exact public binding
and multi-replica rotation/revocation/failover evidence. Likewise, moderation
strict-ingress preflight does not provide the real external moderation signer,
settlement, publication, notification, archive, or multi-replica deployment
evidence required for production readiness.

## Configuration

To run the Iroha peer binary, you must [generate the keys](#generating-keys) and provide a [configuration file](#configuration-file).

### Generating Keys

Generate a new key pair for every non-testing deployment. Validator consensus
identities use BLS-normal keys and a Proof-of-Possession; client and transport
identities typically use Ed25519. The provided
[`kagami`](../iroha_kagami/README.md) tool writes keys directly into an
owner-only custody directory. For a validator identity:

```bash
cargo run --bin kagami -- keys --algorithm bls_normal --pop \
  --out-dir ./validator-key-custody
```

Kagami creates a fresh owner-only directory containing `public.key` and
`private.key`, plus `pop.hex` for this command; it never prints the private key
to standard output. To see the available options, run
`cargo run --bin kagami -- keys --help`.

**NOTE**: The `kagami` binary can be run without `cargo` using the `<IROHA REPO ROOT>/target/release/kagami` binary.
Refer to [generating key pairs with `kagami`](../iroha_kagami/CommandLineHelp.md#kagami-keys) for more details.

### Configuration file

See the current [peer configuration reference](https://docs.iroha.tech/reference/peer-config/params.html)
for the complete parameter list and examples.

## Deployment

You may deploy Iroha as a [native binary](#native-binary) or by using [Docker](#docker).

### Native binary

1. **Build the binaries.**

    ```bash
    cargo build --release -p irohad --bin iroha3d
    cargo build --release -p iroha_kagami
    ```

2. **Stage a runtime directory.** Copy the release binary and the closest
   configuration template (the Sora Nexus profile ships under `defaults/nexus/`):

    ```bash
    mkdir -p deploy/peer
    cp target/release/iroha3d deploy/peer/
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

    - Generate a validator key pair into a fresh owner-only custody directory:

      ```bash
      cargo run --release -p iroha_kagami -- \
        keys --algorithm bls_normal --pop \
        --out-dir deploy/peer/validator-key-custody
      ```

    - Update `config.toml` with the new `chain` and the canonical records from
      `validator-key-custody/{public,private}.key`, plus the `trusted_peers` you
      expect in your initial topology and the matching `pop.hex` entry in
      `trusted_peers_pop`. Ensure each peer advertises a unique
      `network.address`/Torii port pair.

4. **Generate and sign the genesis block.**

    - Produce a template genesis manifest and tweak it as needed (additional
      accounts, assets, instructions, etc.):

      ```bash
      cargo run --release -p iroha_kagami -- \
        genesis generate default \
        --genesis-public-key <GENESIS_PUBLIC_KEY> \
        > deploy/peer/genesis.json
      ```

    - Sign the manifest to obtain the Norito block (`.nrt`) that the daemon
      expects:

      ```bash
      cargo run --release -p iroha_kagami -- \
        genesis sign deploy/peer/genesis.json \
        --private-key-file <MODE_0600_GENESIS_PRIVATE_KEY_FILE> \
        --expected-public-key <GENESIS_PUBLIC_KEY> \
        --bound-manifest-out deploy/peer/genesis.json \
        --out-file deploy/peer/genesis.signed.nrt \
        --expected-hash-out deploy/peer/genesis.expected_hash
      ```

      Then edit `config.toml` so that the `[genesis]` section references the
      signed block:

      ```toml
      [genesis]
      file = "genesis.signed.nrt"
      manifest_json = "genesis.json"
      public_key = "<GENESIS_PUBLIC_KEY>"
      expected_hash_file = "genesis.expected_hash"
      ```

      See `crates/iroha_kagami/CommandLineHelp.md` and the
      [public genesis reference](https://docs.iroha.tech/reference/genesis.html)
      for additional subcommands such as `validate` and `embed-pop`.

5. **Start an Iroha peer.** Point the daemon at your staged configuration (add
   `--sora` when using the Nexus profile from `defaults/nexus/`):

    ```bash
    cd deploy/peer
    ./iroha3d --config ./config.toml
    # or, for the Nexus demo profile:
    ./iroha3d --sora --config ./config.toml
    ```

    Repeat the validator configuration/key steps for every peer and provision
    the same signed genesis block plus exact bound manifest on every peer with
    empty storage. Genesis is a local startup trust artifact and is not fetched
    from another validator. To tolerate _f_ Byzantine faults the network must
    contain exactly _3f + 1_ validators with mutually listed `trusted_peers`
    entries.

### Docker

We provide an explicitly seeded development-only sample configuration in
[`docker-compose.yml`](../../defaults/docker-compose.yml). It contains no
genesis signing key or runtime signer. Provision the signed body, verifier key,
and independently approved exact hash for that exact sample roster before
evaluating the manifest:

```bash
cargo run --bin kagami -- localnet \
  --seed Iroha --peers 4 --sora-profile nexus --consensus-mode npos \
  --out-dir target/compose-genesis
export IROHA_GENESIS_SIGNED_FILE="$PWD/target/compose-genesis/genesis.signed.nrt"
export IROHA_GENESIS_PUBLIC_KEY_FILE="$PWD/target/compose-genesis/genesis.public_key"
export IROHA_GENESIS_EXPECTED_HASH_FILE="$PWD/target/compose-genesis/genesis.expected_hash"
docker compose -f defaults/docker-compose.yml up --build
```

The checked seeded Compose mounts all three runtime inputs read-only into every
validator. Prepared seedless Compose validates each exact `peerN.toml`, derives
a content-addressed container-safe projection, proves that its consensus and
deterministic execution fingerprints are unchanged, and mounts that projection
as `/config/peer.toml` through a file-backed Compose secret. Validator keys do
not appear in Compose YAML or environment variables. Neither mode mounts the
genesis signing key, client credentials, source manifest, or source peer config.
Host-only account-onboarding and faucet services are omitted from the
validator-only projection. Missing files and trust-root mismatches fail closed.
For a deployed network, generate one authoritative `kagami localnet` bundle and
run `kagami docker` without `--seed`; Kagami validates and reuses its identities,
PoPs, signed body, verifier key, hash, and policy-equivalent configs rather than
inheriting the sample validator credentials.
To keep containers running after closing the terminal,
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
