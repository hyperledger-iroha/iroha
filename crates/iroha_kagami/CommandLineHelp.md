# Command-Line Help for `kagami`

This document contains the help content for the `kagami` command-line program.

**Command Overview:**

* [`kagami`↴](#kagami)
* [`kagami wizard`↴](#kagami-wizard)
* [`kagami localnet-wizard`↴](#kagami-localnet-wizard)
* [`kagami localnet`↴](#kagami-localnet)
* [`kagami docker`↴](#kagami-docker)
* [`kagami keys`↴](#kagami-keys)
* [`kagami genesis`↴](#kagami-genesis)
* [`kagami genesis sign`↴](#kagami-genesis-sign)
* [`kagami genesis generate`↴](#kagami-genesis-generate)
* [`kagami genesis generate default`↴](#kagami-genesis-generate-default)
* [`kagami genesis generate synthetic`↴](#kagami-genesis-generate-synthetic)
* [`kagami genesis validate`↴](#kagami-genesis-validate)
* [`kagami genesis validate-prepared`↴](#kagami-genesis-validate-prepared)
* [`kagami genesis embed-pop`↴](#kagami-genesis-embed-pop)
* [`kagami genesis normalize`↴](#kagami-genesis-normalize)
* [`kagami kagemusha`↴](#kagami-kagemusha)
* [`kagami kagemusha verify-release-v4`↴](#kagami-kagemusha-verify-release-v4)
* [`kagami kagemusha promote-release-v4`↴](#kagami-kagemusha-promote-release-v4)
* [`kagami kagemusha prepare-activation-v4`↴](#kagami-kagemusha-prepare-activation-v4)
* [`kagami kagemusha prepare-enable-issuance-v4`↴](#kagami-kagemusha-prepare-enable-issuance-v4)
* [`kagami kagemusha prepare-cancel-release-v4`↴](#kagami-kagemusha-prepare-cancel-release-v4)
* [`kagami kagemusha prepare-deactivate-issuance-v4`↴](#kagami-kagemusha-prepare-deactivate-issuance-v4)
* [`kagami kagemusha prepare-release-circuit-params-v4`↴](#kagami-kagemusha-prepare-release-circuit-params-v4)
* [`kagami kagemusha prepare-taira-release-roster-v4`↴](#kagami-kagemusha-prepare-taira-release-roster-v4)
* [`kagami kagemusha prepare-taira-testnet-base-genesis-v4`↴](#kagami-kagemusha-prepare-taira-testnet-base-genesis-v4)
* [`kagami privacy-bootstrap`↴](#kagami-privacy-bootstrap)
* [`kagami privacy-bootstrap emit-taira-v1`↴](#kagami-privacy-bootstrap-emit-taira-v1)
* [`kagami privacy-bootstrap validate-taira-v1`↴](#kagami-privacy-bootstrap-validate-taira-v1)
* [`kagami privacy-bootstrap validate-taira-nevo-review-v1`↴](#kagami-privacy-bootstrap-validate-taira-nevo-review-v1)
* [`kagami privacy-bootstrap render-taira-release-v1`↴](#kagami-privacy-bootstrap-render-taira-release-v1)
* [`kagami verify`↴](#kagami-verify)
* [`kagami advanced`↴](#kagami-advanced)
* [`kagami advanced client-configs`↴](#kagami-advanced-client-configs)
* [`kagami advanced codec`↴](#kagami-advanced-codec)
* [`kagami advanced codec list-types`↴](#kagami-advanced-codec-list-types)
* [`kagami advanced codec norito-to-rust`↴](#kagami-advanced-codec-norito-to-rust)
* [`kagami advanced codec norito-to-json`↴](#kagami-advanced-codec-norito-to-json)
* [`kagami advanced codec json-to-norito`↴](#kagami-advanced-codec-json-to-norito)
* [`kagami advanced kura`↴](#kagami-advanced-kura)
* [`kagami advanced kura print`↴](#kagami-advanced-kura-print)
* [`kagami advanced kura sidecar`↴](#kagami-advanced-kura-sidecar)
* [`kagami advanced markdown-help`↴](#kagami-advanced-markdown-help)
* [`kagami advanced schema`↴](#kagami-advanced-schema)

## `kagami`

Task-first Iroha operator tooling for guided setup, local devnets, genesis work, and diagnostics.

**Usage:** `kagami [OPTIONS] <COMMAND>`

Common tasks:
  kagami localnet-wizard
  kagami wizard
  kagami localnet --out-dir ./localnet
  kagami docker --peers 4 --config-dir ./localnet --image hyperledger/iroha:dev --out-file docker-compose.yml
  kagami keys --out-dir ./key-custody
  kagami keys --algorithm bls_normal --pop --out-dir ./validator-custody
  kagami advanced markdown-help


###### **Subcommands:**

* `wizard` — Guided onboarding flow for staging a Sora Nexus observer configuration
* `localnet-wizard` — Guided disposable local devnet flow for generating peers, configs, genesis, and scripts
* `localnet` — Generate a bare-metal local network: genesis, per-peer configs, client config, and scripts
* `docker` — Generate validator-only Docker Compose from a prepared bundle or explicit dev seed
* `keys` — Generate cryptographic key pairs and optional validator Proofs-of-Possession
* `genesis` — Commands related to genesis
* `kagemusha` — Verify and promote authenticated Kagemusha ABI-21/V4 artifact releases
* `privacy-bootstrap` — Emit and validate fail-closed Taira exact-12 privacy bootstrap artifacts
* `verify` — Verify a genesis manifest against a preset profile
* `advanced` — Advanced low-level helpers for codec conversion, schema generation, block inspection, and docs

###### **Options:**

* `--ui-mode <MODE>` — Control how Kagami formats status messages (auto detects TTY by default)

  Default value: `auto`

  Possible values: `auto`, `plain`, `rich`




## `kagami wizard`

Guided onboarding flow for staging a Sora Nexus observer configuration

**Usage:** `kagami wizard [OPTIONS]`

###### **Options:**

* `--output-dir <PATH>` — Directory where generated config/genesis files will be written

  Default value: `wizard-output`
* `--non-interactive` — Run non-interactively, accepting defaults for prompts that are not supplied via flags
* `--p2p-host <HOST>` — Override the public P2P host/IP advertised for this generated observer
* `--p2p-port <PORT>` — Override the public P2P port for this peer
* `--torii-port <PORT>` — Override the local Torii listener port for this peer
* `--relay-mode <RELAY_MODE>` — Override the relay mode instead of prompting interactively

  Possible values: `disabled`, `hub`, `spoke`, `assist`

* `--relay-hub-address <HOST:PORT>` — Relay hub addresses (`host:port`), repeat once per hub when relay mode uses them
* `--trusted-peers <PEERS>` — Trusted roster (`pubkey` or `pubkey@host:port`); include a reachable address without a relay
* `--trusted-peers-pop <POPS>` — Comma-separated PoP entries for trusted peers (`pubkey=pop_hex`)



## `kagami localnet-wizard`

Guided disposable local devnet flow for generating peers, configs, genesis, and scripts

**Usage:** `kagami localnet-wizard`



## `kagami localnet`

Generate a bare-metal local network: genesis, per-peer configs, client config, and scripts

**Usage:** `kagami localnet [OPTIONS] --out-dir <DIR>`

###### **Options:**

* `-p`, `--peers <COUNT>` — Number of peers to generate (minimum four)

  Default value: `4`
* `-s`, `--seed <SEED>` — Optional UTF-8 seed for deterministic development keys.

   Omit this option to generate independent keys from operating-system entropy.
* `--chain-id <CHAIN_ID>` — Canonical chain identifier written into genesis, peer configs, and the client config

  Default value: `00000000-0000-0000-0000-000000000000`
* `--sora-profile <PROFILE>` — Enable Sora profile defaults; `nexus` enforces public dataspace rules (NPoS). Requires at least 4 peers

  Possible values: `dataspace`, `nexus`

* `--private-dataspace <DATASPACE>` — Select an exact restricted dataspace preset for the `dataspace` Sora profile

  Possible values:
  - `sbp`:
    State Bank of Pakistan dataspace (id 10, lane 3)
  - `cbuae`:
    Central Bank of the UAE dataspace (id 12, lane 4)

* `--perf-profile <PROFILE>` — Apply a localnet performance profile (10k TPS / 1s finality presets)

  Possible values: `10k-permissioned`, `10k-npos`

* `--bind-host <HOST>` — Host to bind P2P and Torii listeners to (host/IP only, no port)

  Default value: `0.0.0.0`
* `--public-host <HOST>` — Host to advertise to peers and use for client Torii URL (host/IP only, no port)

  Default value: `127.0.0.1`
* `--base-api-port <BASE_API_PORT>` — Base Torii API port (per-peer increments by 1)

  Default value: `8080`
* `--base-p2p-port <BASE_P2P_PORT>` — Base P2P port (per-peer increments by 1)

  Default value: `1337`
* `-o`, `--out-dir <DIR>` — Output directory for configs/genesis/scripts
* `--extra-accounts <EXTRA_ACCOUNTS>` — Extra accounts to pre-register (in wonderland)

  Default value: `0`
* `--sample-asset` — Register the optional sample asset and mint to the default account. The built-in Kagemusha asset is always emitted

  Default value: `false`
* `--asset-definition-id <ASSET_DEFINITION_ID>` — Register additional asset definition IDs owned by the generated client signer. Repeat the flag to register more than one asset definition. A localnet reserve is minted to the generated client signer for each requested asset definition
* `--block-cadence-ms <MILLISECONDS>` — Override the immutable signed block cadence in milliseconds. Leave unset to use the one-second localnet cadence
* `--consensus-mode <MODE>` — Consensus mode to emit in genesis/configs. Defaults to `permissioned` for generic localnets. Sora profile localnets and perf profiles require `npos`

  Possible values: `permissioned`, `npos`




## `kagami docker`

Generate validator-only Docker Compose from a prepared bundle or explicit dev seed

**Usage:** `kagami docker [OPTIONS] --peers <COUNT> --config-dir <DIR> --image <NAME> --out-file <FILE>`

###### **Options:**

* `-p`, `--peers <COUNT>` — Number of peer services in the configuration.

   Must be an exact Sumeragi v2 `3f + 1` committee in the range 4..=31.
* `-s`, `--seed <SEED>` — Enable deterministic development mode with this UTF-8 validator seed.

   When omitted, `--config-dir` must be an authoritative prepared bundle containing `peerN.toml`, signed genesis, verifier-key, and exact-hash files. Production workflows should omit this option so Compose cannot generate identities that diverge from genesis.
* `-H`, `--healthcheck` — Includes a healthcheck for every service in the configuration.

   Healthchecks use predefined settings.

   For more details on healthcheck configuration in Docker Compose files, see: <https://docs.docker.com/compose/compose-file/compose-file-v3/#healthcheck>
* `-c`, `--config-dir <DIR>` — Authoritative prepared validator/genesis bundle, or development manifest directory.

   Normal mode requires `genesis.json`, `peer0.toml` through `peerN.toml`, `genesis.signed.nrt`, `genesis.public_key`, and `genesis.expected_hash`. Kagami validates their canonical wire, signer, semantic manifest binding, exact hash, validator roster, and PoPs together. With `--seed`, only `genesis.json` is read and runtime artifact paths are supplied explicitly through the generated manifest's `IROHA_GENESIS_*_FILE` variables.
* `--peer-config <FILE>` — Optional TOML file describing peer names and port mappings. Only available with deterministic development `--seed` mode.

   The file must contain an array named `peers`, for example:

   ```toml [[peers]] name = "alpha" p2p_port = 2000 api_port = 9000 [[peers]] name = "beta" p2p_port = 2001 api_port = 9001 ```
* `-i`, `--image <NAME>` — Docker image used by the peer services.

   By default, the image is pulled from Docker Hub if not cached. Pass the `--build` option to build the image from a Dockerfile instead.

   The image must be built from the same Git revision as Kagami.
* `-b`, `--build <DIR>` — Build the image from the Dockerfile in the specified directory. Do not rebuild if the image has been cached.

   The provided path is resolved relative to the current working directory.
* `--no-cache` — Always pull or rebuild the image even if it is cached locally
* `-o`, `--out-file <FILE>` — Path to the target Compose configuration file.

   The file must be outside `--config-dir` and is published atomically.

   If the file exists, the app will prompt its overwriting. If the TTY is not interactive, the app will stop execution with a non-zero exit code. To overwrite the file anyway, pass the `--force` flag.
* `-P`, `--print` — Print the generated configuration to stdout instead of writing it to the target file.

   Note that the target path still needs to be provided, as it is used to resolve paths.
* `-F`, `--force` — Overwrite the target file if it already exists
* `--no-banner` — Do not include the banner with the generation notice in the file



## `kagami keys`

Generate cryptographic key pairs and optional validator Proofs-of-Possession

**Usage:** `kagami keys [OPTIONS] --out-dir <DIR>`

###### **Options:**

* `-a`, `--algorithm <ALGORITHM>` — An algorithm to use for the key-pair generation

  Default value: `ed25519`

  Possible values: `ed25519`, `secp256k1`, `ml-dsa`, `bls_normal`, `bls_small`

* `--seed-hex <HEX>` — A 32-byte secret key-generation seed encoded as 64 hexadecimal characters.

   This is for reproducible fixtures. Omit it for OS-random production keys.
* `--out-dir <DIR>` — Write the key pair into a new owner-only custody directory.

   The directory must not contain any existing entries. Files are written as `public.key` and `private.key`; `--pop` also writes `pop.hex`. The private key never passes through standard output.
* `--pop` — Also output a BLS Proof-of-Possession (PoP) for this key (BLS-normal only). Written as `pop.hex` in the custody directory



## `kagami genesis`

Commands related to genesis

**Usage:** `kagami genesis <COMMAND>`

###### **Subcommands:**

* `sign` — Sign the genesis block
* `generate` — Generate a genesis configuration and standard-output in JSON format
* `validate` — Validate a genesis JSON file and report invalid identifiers
* `validate-prepared` — Verify one exact bound-manifest/signed-genesis/signer/hash bundle
* `embed-pop` — Embed one or more PoPs into a genesis JSON manifest (inline `topology` entries carrying `pop_hex`)
* `normalize` — Expand a genesis manifest and show the final ordered transactions



## `kagami genesis sign`

Sign the genesis block

**Usage:** `kagami genesis sign [OPTIONS] --private-key-file <PATH> <GENESIS_FILE>`

###### **Arguments:**

* `<GENESIS_FILE>` — Path to genesis json file

###### **Options:**

* `-o`, `--out-file <PATH>` — Path to signed genesis output file in canonical Norito wire format (stdout by default)
* `--bound-manifest-out <PATH>` — Persist the exact config-bound genesis manifest used to build the signed block. May point to `GENESIS_FILE` to replace the input only after binding succeeds
* `--expected-hash-out <PATH>` — Write the canonical checked NetworkId derived from the exact signed consensus-header hash as one line.

   Validators and clients must select this same file through `genesis.expected_hash_file` and `network_id_file`, respectively.
* `-t`, `--topology <TOPOLOGY>` — Use this topology instead of specified in genesis.json. JSON-serialized vector of `PeerId`. For use in `iroha_swarm`.

   The final unique topology must be an exact Sumeragi v2 `3f + 1` committee in the range 4..=31.
* `--peer-pop <PEER_POPS>` — Embed one or more PoPs into the same transaction as `--topology`. Repeatable flag: `--peer-pop <public_key=pop_hex>`
* `--private-key-file <PATH>` — Owner-held mode-0600 file containing one canonical private-key multihash
* `--expected-public-key <PUBLIC_KEY>` — Public key that the selected private key must derive.

   Use this when the verifier key is distributed separately from the owner-held signing key, such as through container secrets.
* `--creation-time-ms <MILLISECONDS>` — Deterministic genesis transaction creation-time base in Unix milliseconds.

   Omit this for a fresh wall-clock timestamp. Fixture generators should set it so repeated signing produces identical canonical wire bytes.
* `--config <PATH>` — Optional peer config TOML used to derive the DA proof-policy bundle embedded into genesis



## `kagami genesis generate`

Generate a genesis configuration and standard-output in JSON format

**Usage:** `kagami genesis generate [OPTIONS] --ivm-dir <PATH> --genesis-public-key <MULTI_HASH> [COMMAND]`

###### **Subcommands:**

* `default` — Generate default genesis
* `synthetic` — Generate synthetic genesis with the specified number of domains, accounts and assets

###### **Options:**

* `--profile <PROFILE>` — Optional profile: picks Iroha3 chain, cadence, consensus, and VRF defaults for dev/taira/nexus

  Possible values:
  - `iroha3-dev`:
    Local-only developer network
  - `iroha3-taira`:
    Public Sora test network
  - `iroha3-nexus`:
    Sora Nexus main network

* `--chain-id <CHAIN_ID>` — Optional explicit chain id. With a profile, it must equal that profile's pinned chain id
* `--vrf-seed-hex <HEX>` — Optional VRF seed (hex, 32 bytes). Required for the public `iroha3-taira`/`iroha3-nexus` profiles
* `--xor-asset-definition-id <BASE58>` — Canonical public XOR asset definition id (Base58). Required for `iroha3-nexus` NPoS manifests; `iroha3-taira` defaults to its live XOR id
* `--executor <PATH>` — Optional path (relative to output) to the executor bytecode file (.to). If omitted, no executor upgrade is included in genesis
* `--ivm-dir <PATH>` — Relative path from the directory of output file to the directory that contains IVM bytecode libraries
* `--genesis-public-key <MULTI_HASH>`
* `--ivm-gas-limit-per-block <U64>` — Optional: set the custom parameter `ivm_gas_limit_per_block` (u64) in genesis so all peers agree on the block gas budget. If omitted, a sensible default (1,680,000) is applied
* `--consensus-mode <MODE>` — Select the consensus mode snapshot to seed in the genesis parameters (public dataspace requires NPoS; other dataspaces may use permissioned or NPoS)

  Possible values: `permissioned`, `npos`

* `--sm-openssl-preview <BOOL>` — Toggle the OpenSSL-backed SM preview helpers in the generated manifest

  Possible values: `true`, `false`

* `--default-hash <HASH>` — Override the default hash advertised in the manifest
* `--allowed-signing <ALGO>` — Replace the allowed signing algorithms (repeat flag to supply multiple values)

  Possible values: `ed25519`, `secp256k1`

* `--sm2-distid-default <DISTID>` — Override the fallback SM2 distinguishing identifier
* `--allowed-curve-id <CURVE_ID>` — Override the allowed curve identifiers (repeat flag to supply multiple values)



## `kagami genesis generate default`

Generate default genesis

**Usage:** `kagami genesis generate default`



## `kagami genesis generate synthetic`

Generate synthetic genesis with the specified number of domains, accounts and assets.

Synthetic mode is useful when we need a semi-realistic genesis for stress-testing Iroha's startup times as well as being able to just start an Iroha network and have instructions that represent a typical blockchain after migration.

**Usage:** `kagami genesis generate synthetic [OPTIONS]`

###### **Options:**

* `--domains <DOMAINS>` — Number of domains in synthetic genesis

  Default value: `0`
* `--accounts-per-domain <ACCOUNTS_PER_DOMAIN>` — Number of accounts per domains in synthetic genesis. The total number of accounts would be `domains * accounts_per_domain`

  Default value: `0`
* `--asset-definitions-per-domain <ASSET_DEFINITIONS_PER_DOMAIN>` — Number of asset definitions per domain in synthetic genesis. The total number of asset definitions would be `domains * asset_definitions_per_domain`

  Default value: `0`



## `kagami genesis validate`

Validate a genesis JSON file and report invalid identifiers

**Usage:** `kagami genesis validate <GENESIS_FILE>`

###### **Arguments:**

* `<GENESIS_FILE>` — Path to genesis json file



## `kagami genesis validate-prepared`

Verify one exact bound-manifest/signed-genesis/signer/hash bundle

**Usage:** `kagami genesis validate-prepared [OPTIONS] --reviewed-manifest <PATH> --validator-roster <PATH> --bound-manifest <PATH> --pre-sign-manifest <PATH> --signed-genesis <PATH> --genesis-public-key <PUBLIC_KEY> --expected-hash <HASH>`

###### **Options:**

* `--reviewed-manifest <PATH>` — Exact reviewed NEVO genesis before validator rendering
* `--validator-roster <PATH>` — Exact public validator roster used by the renderer
* `--bound-manifest <PATH>` — Exact config-bound genesis manifest used by the external signer
* `--pre-sign-manifest <PATH>` — Exact renderer output accepted by the external signer before config binding
* `--signed-genesis <PATH>` — Exact signed genesis in canonical framed Norito form
* `--peer-config <PATH>` — Effective validator configs whose complete roster and policy must reproduce the signed context. Repeat exactly four times in `taira-validator-1` through `-4` order
* `--genesis-public-key <PUBLIC_KEY>` — Public key of the independently provisioned genesis signer
* `--expected-hash <HASH>` — Exact signed genesis block-header hash



## `kagami genesis embed-pop`

Embed one or more PoPs into a genesis JSON manifest (inline `topology` entries carrying `pop_hex`)

**Usage:** `kagami genesis embed-pop [OPTIONS] --manifest <MANIFEST> --out <OUT>`

###### **Options:**

* `--manifest <MANIFEST>` — Input genesis JSON file (RawGenesisTransaction)
* `--out <OUT>` — Output file path
* `--peer-pop <PEER_POPS>` — Peer PoP entries in the form `public_key=hex`



## `kagami genesis normalize`

Expand a genesis manifest and show the final ordered transactions

**Usage:** `kagami genesis normalize [OPTIONS] <GENESIS_FILE>`

###### **Arguments:**

* `<GENESIS_FILE>` — Path to genesis json file

###### **Options:**

* `--format <FORMAT>` — Output format (`json` for structured output, `text` for a compact summary)

  Default value: `json`

  Possible values: `json`, `text`




## `kagami kagemusha`

Verify and promote authenticated Kagemusha ABI-21/V4 artifact releases

**Usage:** `kagami kagemusha <COMMAND>`

###### **Subcommands:**

* `verify-release-v4` — Verify one complete authenticated ABI-21/V4 release directory
* `promote-release-v4` — Verify an ABI-21/V4 release and atomically write its typed promotion record
* `prepare-activation-v4` — Build one release-bound activation instruction from an authenticated V4 catalog
* `prepare-enable-issuance-v4` — Build one staged-to-enabled instruction from an exact canonical witness
* `prepare-cancel-release-v4` — Build one permanent staged-release cancellation instruction
* `prepare-deactivate-issuance-v4` — Build one permanent enabled-issuance deactivation instruction
* `prepare-release-circuit-params-v4` — Atomically publish the canonical reviewed Eq/Ep first-release circuit parameters
* `prepare-taira-release-roster-v4` — Build the actual rendered Taira validator roster for signed V4 release generation
* `prepare-taira-testnet-base-genesis-v4` — Append network-independent offline-cash prerequisites to a fresh Taira genesis



## `kagami kagemusha verify-release-v4`

Verify one complete authenticated ABI-21/V4 release directory

**Usage:** `kagami kagemusha verify-release-v4 [OPTIONS] --bundle-dir <BUNDLE_DIR> --release-policy <RELEASE_POLICY> --benchmark-evidence <BENCHMARK_EVIDENCE> --cryptographic-review <CRYPTOGRAPHIC_REVIEW>`

###### **Options:**

* `--bundle-dir <BUNDLE_DIR>` — Immutable directory containing the exact eighteen-file promoted ABI-21/V4 inventory
* `--release-policy <RELEASE_POLICY>` — Canonical release policy provisioned alongside the candidate release
* `--benchmark-evidence <BENCHMARK_EVIDENCE>` — Signed physical-device benchmark evidence file
* `--cryptographic-review <CRYPTOGRAPHIC_REVIEW>` — Canonical signed, candidate-bound cryptographic review Norito file
* `--memory-limit-bytes <MEMORY_LIMIT_BYTES>` — Optional nonzero byte ceiling that may only lower the built-in physical-memory limit



## `kagami kagemusha promote-release-v4`

Verify an ABI-21/V4 release and atomically write its typed promotion record

**Usage:** `kagami kagemusha promote-release-v4 [OPTIONS] --bundle-dir <BUNDLE_DIR> --release-policy <RELEASE_POLICY> --promotion-record <PROMOTION_RECORD> --benchmark-evidence <BENCHMARK_EVIDENCE> --cryptographic-review <CRYPTOGRAPHIC_REVIEW>`

###### **Options:**

* `--bundle-dir <BUNDLE_DIR>` — Directory containing the exact seventeen-file pre-promotion ABI-21/V4 candidate
* `--release-policy <RELEASE_POLICY>` — Canonical release policy provisioned alongside the candidate release
* `--promotion-record <PROMOTION_RECORD>` — Exact absent `<bundle-dir>/promotion-record-v4.norito` leaf; it is never overwritten
* `--benchmark-evidence <BENCHMARK_EVIDENCE>` — Signed physical-device benchmark evidence file
* `--cryptographic-review <CRYPTOGRAPHIC_REVIEW>` — Canonical signed, candidate-bound cryptographic review Norito file
* `--memory-limit-bytes <MEMORY_LIMIT_BYTES>` — Optional nonzero byte ceiling that may only lower the built-in physical-memory limit



## `kagami kagemusha prepare-activation-v4`

Build one release-bound activation instruction from an authenticated V4 catalog

**Usage:** `kagami kagemusha prepare-activation-v4 --promotion-id <PROMOTION_ID> --promotion-binding <PROMOTION_BINDING> --artifact-root <ARTIFACT_ROOT> --release-policy <RELEASE_POLICY> --manifest-sha256 <MANIFEST_SHA256> --runtime-effective-config-sha256 <RUNTIME_EFFECTIVE_CONFIG_SHA256> --verifier-version <VERIFIER_VERSION> --device-attestation-policy <DEVICE_ATTESTATION_POLICY> --policy-evaluation-time-ms <POLICY_EVALUATION_TIME_MS> --output <OUTPUT>`

###### **Options:**

* `--promotion-id <PROMOTION_ID>` — Unique nonzero promotion-run identity reserved before validator qualification
* `--promotion-binding <PROMOTION_BINDING>` — Exact canonical controller-signed promotion binding committed by activation
* `--artifact-root <ARTIFACT_ROOT>` — Root containing lowercase manifest-digest release directories
* `--release-policy <RELEASE_POLICY>` — Canonical release policy configured on every validator
* `--manifest-sha256 <MANIFEST_SHA256>` — Exact lowercase SHA-256 directory name of the release to activate
* `--runtime-effective-config-sha256 <RUNTIME_EFFECTIVE_CONFIG_SHA256>` — Domain-separated SHA-256 shared by all four validator runtime projections
* `--verifier-version <VERIFIER_VERSION>` — Next atomic Eq/Ep verifier version observed from live consensus state
* `--device-attestation-policy <DEVICE_ATTESTATION_POLICY>` — Exact governed verifier policy derived from authenticated physical-device evidence. The policy and release are embedded in one composite consensus instruction
* `--policy-evaluation-time-ms <POLICY_EVALUATION_TIME_MS>` — Explicit Unix timestamp used for the same certificate-validity checks as consensus. The activation is checked again against its actual block timestamp on every validator
* `--output <OUTPUT>` — New private file receiving exact instruction JSON for direct-lifecycle payload preparation



## `kagami kagemusha prepare-enable-issuance-v4`

Build one staged-to-enabled instruction from an exact canonical witness

**Usage:** `kagami kagemusha prepare-enable-issuance-v4 --enable-witness <ENABLE_WITNESS> --output <OUTPUT>`

###### **Options:**

* `--enable-witness <ENABLE_WITNESS>` — Exact canonical bounded staged-to-enabled witness
* `--output <OUTPUT>` — New private file receiving exact instruction JSON for direct-lifecycle payload preparation



## `kagami kagemusha prepare-cancel-release-v4`

Build one permanent staged-release cancellation instruction

**Usage:** `kagami kagemusha prepare-cancel-release-v4 --cancellation <CANCELLATION> --output <OUTPUT>`

###### **Options:**

* `--cancellation <CANCELLATION>` — Exact canonical predecessor-bound staged-release cancellation
* `--output <OUTPUT>` — New private file receiving exact instruction JSON for direct-lifecycle payload preparation



## `kagami kagemusha prepare-deactivate-issuance-v4`

Build one permanent enabled-issuance deactivation instruction

**Usage:** `kagami kagemusha prepare-deactivate-issuance-v4 --deactivation <DEACTIVATION> --output <OUTPUT>`

###### **Options:**

* `--deactivation <DEACTIVATION>` — Exact canonical predecessor-bound enabled-issuance deactivation
* `--output <OUTPUT>` — New private file receiving exact instruction JSON for direct-lifecycle payload preparation



## `kagami kagemusha prepare-release-circuit-params-v4`

Atomically publish the canonical reviewed Eq/Ep first-release circuit parameters

**Usage:** `kagami kagemusha prepare-release-circuit-params-v4 --output-dir <OUTPUT_DIR>`

###### **Options:**

* `--output-dir <OUTPUT_DIR>` — New owner-private directory atomically receiving the canonical Eq/Ep Norito files



## `kagami kagemusha prepare-taira-release-roster-v4`

Build the actual rendered Taira validator roster for signed V4 release generation

**Usage:** `kagami kagemusha prepare-taira-release-roster-v4 [OPTIONS] --validator-config <VALIDATOR_CONFIG> --network-id <NETWORK_ID> --output <OUTPUT>`

###### **Options:**

* `--validator-config <VALIDATOR_CONFIG>` — One rendered validator config containing the complete trusted-peers PoP roster
* `--network-id <NETWORK_ID>` — Exact genesis-derived network identity whose finality votes the roster authenticates
* `--withdrawal-height <WITHDRAWAL_HEIGHT>` — First excluded height for release issuance and roster authentication

  Default value: `1000000000`
* `--output <OUTPUT>` — New private file receiving the canonical Norito roster artifact



## `kagami kagemusha prepare-taira-testnet-base-genesis-v4`

Append network-independent offline-cash prerequisites to a fresh Taira genesis

**Usage:** `kagami kagemusha prepare-taira-testnet-base-genesis-v4 [OPTIONS] --genesis <GENESIS> --genesis-authority <GENESIS_AUTHORITY> --command-authority <COMMAND_AUTHORITY> --output <OUTPUT>`

###### **Options:**

* `--genesis <GENESIS>` — Fresh canonical Taira unsigned genesis manifest
* `--genesis-authority <GENESIS_AUTHORITY>` — I105 account used to sign and execute the genesis block
* `--command-authority <COMMAND_AUTHORITY>` — Runtime account whose private key signs Torii offline commands
* `--fee-mint <FEE_MINT>` — XOR amount minted to the command authority for transaction fees

  Default value: `1000000`
* `--output <OUTPUT>` — New private path receiving the unsigned Taira base genesis



## `kagami privacy-bootstrap`

Emit and validate fail-closed Taira exact-12 privacy bootstrap artifacts

**Usage:** `kagami privacy-bootstrap <COMMAND>`

###### **Subcommands:**

* `emit-taira-v1` — Emit all twelve compiled governance activation templates atomically
* `validate-taira-v1` — Validate an emitted exact-12 instruction set and its digest inventory
* `validate-taira-nevo-review-v1` — Validate a reviewed Taira NEVO unsigned genesis without creating release artifacts
* `render-taira-release-v1` — Compose a complete secret-free Taira release plan, config, and genesis



## `kagami privacy-bootstrap emit-taira-v1`

Emit all twelve compiled governance activation templates atomically

**Usage:** `kagami privacy-bootstrap emit-taira-v1 --instructions-output <INSTRUCTIONS_OUTPUT> --report-output <REPORT_OUTPUT>`

###### **Options:**

* `--instructions-output <INSTRUCTIONS_OUTPUT>` — New file receiving the canonical governance-template instruction array
* `--report-output <REPORT_OUTPUT>` — New file receiving base64 Norito instructions and deterministic digests



## `kagami privacy-bootstrap validate-taira-v1`

Validate an emitted exact-12 instruction set and its digest inventory

**Usage:** `kagami privacy-bootstrap validate-taira-v1 --instructions <INSTRUCTIONS> --report <REPORT>`

###### **Options:**

* `--instructions <INSTRUCTIONS>` — Canonical genesis instruction JSON array emitted by this command group
* `--report <REPORT>` — Canonical digest inventory emitted alongside the instruction array



## `kagami privacy-bootstrap validate-taira-nevo-review-v1`

Validate a reviewed Taira NEVO unsigned genesis without creating release artifacts

**Usage:** `kagami privacy-bootstrap validate-taira-nevo-review-v1 --unsigned-genesis <UNSIGNED_GENESIS> --review <REVIEW>`

###### **Options:**

* `--unsigned-genesis <UNSIGNED_GENESIS>` — Exact unsigned NEVO genesis bound by the review manifest
* `--review <REVIEW>` — Deterministic public NEVO review manifest binding the unsigned genesis



## `kagami privacy-bootstrap render-taira-release-v1`

Compose a complete secret-free Taira release plan, config, and genesis

**Usage:** `kagami privacy-bootstrap render-taira-release-v1 --activation-instructions <ACTIVATION_INSTRUCTIONS> --activation-report <ACTIVATION_REPORT> --broker-public-export <BROKER_PUBLIC_EXPORT> --plan-template <PLAN_TEMPLATE> --config-template <CONFIG_TEMPLATE> --genesis-template <GENESIS_TEMPLATE> --nevo-review <NEVO_REVIEW> --plan-output <PLAN_OUTPUT> --config-output <CONFIG_OUTPUT> --genesis-output <GENESIS_OUTPUT> --broker-public-output <BROKER_PUBLIC_OUTPUT>`

###### **Options:**

* `--activation-instructions <ACTIVATION_INSTRUCTIONS>` — Exact-12 instruction JSON emitted by `emit-taira-v1`
* `--activation-report <ACTIVATION_REPORT>` — Digest report emitted together with the exact-12 instructions
* `--broker-public-export <BROKER_PUBLIC_EXPORT>` — Canonical public JSON emitted by the qualified peer-1 broker
* `--plan-template <PLAN_TEMPLATE>` — Canonical disabled Taira privacy plan template
* `--config-template <CONFIG_TEMPLATE>` — Canonical disabled peer-1 Taira config template
* `--genesis-template <GENESIS_TEMPLATE>` — Canonical Taira genesis without privacy bootstrap instructions
* `--nevo-review <NEVO_REVIEW>` — Deterministic public NEVO review manifest binding the genesis template
* `--plan-output <PLAN_OUTPUT>` — Fresh output path for the complete public release plan
* `--config-output <CONFIG_OUTPUT>` — Fresh output path for the complete peer-1 release config
* `--genesis-output <GENESIS_OUTPUT>` — Fresh output path for the complete release genesis
* `--broker-public-output <BROKER_PUBLIC_OUTPUT>` — Fresh output path for the verified canonical public broker export



## `kagami verify`

Verify a genesis manifest against a preset profile

**Usage:** `kagami verify [OPTIONS] --profile <PROFILE> --genesis <PATH>`

###### **Options:**

* `--profile <PROFILE>` — Profile to verify against (`iroha3-dev`, `iroha3-taira`, `iroha3-nexus`)

  Possible values:
  - `iroha3-dev`:
    Local-only developer network
  - `iroha3-taira`:
    Public Sora test network
  - `iroha3-nexus`:
    Sora Nexus main network

* `--genesis <PATH>` — Path to the genesis manifest (JSON)
* `--vrf-seed-hex <HEX>` — Optional VRF seed (hex, 32 bytes). Required for NPoS taira/nexus manifests



## `kagami advanced`

Advanced low-level helpers for codec conversion, schema generation, block inspection, and docs

**Usage:** `kagami advanced <COMMAND>`

###### **Subcommands:**

* `client-configs` — Generate per-client CLI configs from a base client.toml
* `codec` — Commands related to Norito codec conversions
* `kura` — Commands related to block inspection
* `markdown-help` — Output CLI documentation in Markdown format
* `schema` — Generate the schema used for code generation in Iroha SDKs



## `kagami advanced client-configs`

Generate per-client CLI configs from a base client.toml

**Usage:** `kagami advanced client-configs [OPTIONS] --base-config <PATH> --names <NAME>`

###### **Options:**

* `--base-config <PATH>` — Base client config to copy `chain`, `torii_url`, and `basic_auth` from
* `--out-dir <DIR>` — Output directory for generated client configs (default: <base-config-dir>/clients)
* `--domain <SCOPE>` — Account scope for generated client configs (`dataspace` or `domain.dataspace`)

  Default value: `acme.universal`
* `--seed-hex <HEX>` — A 32-byte secret master seed encoded as 64 hexadecimal characters.

   Per-client keys are derived with an explicit domain and client name. Omit this option for independent operating-system-random keys.
* `--names <NAME>` — Comma-separated list of client names



## `kagami advanced codec`

Commands related to Norito codec conversions

**Usage:** `kagami advanced codec <COMMAND>`

###### **Subcommands:**

* `list-types` — Show all available types
* `norito-to-rust` — Decode Norito to Rust debug format from binary file
* `norito-to-json` — Decode Norito to JSON. By default uses stdin and stdout
* `json-to-norito` — Encode JSON as Norito. By default uses stdin and stdout



## `kagami advanced codec list-types`

Show all available types

**Usage:** `kagami advanced codec list-types`



## `kagami advanced codec norito-to-rust`

Decode Norito to Rust debug format from binary file

**Usage:** `kagami advanced codec norito-to-rust [OPTIONS] <BINARY>`

###### **Arguments:**

* `<BINARY>` — Path to the binary with encoded Iroha structure

###### **Options:**

* `-t`, `--type <TYPE_NAME>` — Type that is expected to be encoded in binary. If not specified then a guess will be attempted



## `kagami advanced codec norito-to-json`

Decode Norito to JSON. By default uses stdin and stdout

**Usage:** `kagami advanced codec norito-to-json [OPTIONS] --type <TYPE_NAME>`

###### **Options:**

* `-i`, `--input <INPUT>` — Path to the input file
* `-o`, `--output <OUTPUT>` — Path to the output file
* `-t`, `--type <TYPE_NAME>` — Type that is expected to be encoded in input



## `kagami advanced codec json-to-norito`

Encode JSON as Norito. By default uses stdin and stdout

**Usage:** `kagami advanced codec json-to-norito [OPTIONS] --type <TYPE_NAME>`

###### **Options:**

* `-i`, `--input <INPUT>` — Path to the input file
* `-o`, `--output <OUTPUT>` — Path to the output file
* `-t`, `--type <TYPE_NAME>` — Type that is expected to be encoded in input



## `kagami advanced kura`

Commands related to block inspection

**Usage:** `kagami advanced kura [OPTIONS] <PATH_TO_BLOCK_STORE> <COMMAND>`

###### **Subcommands:**

* `print` — Print contents of a certain length of the blocks
* `sidecar` — Print the pipeline recovery sidecar JSON for a given height

###### **Arguments:**

* `<PATH_TO_BLOCK_STORE>`

###### **Options:**

* `-f`, `--from <BLOCK_HEIGHT>` — Height of the block from which start the inspection. Defaults to the latest block height



## `kagami advanced kura print`

Print contents of a certain length of the blocks

**Usage:** `kagami advanced kura print [OPTIONS]`

###### **Options:**

* `-n`, `--length <LENGTH>` — Number of the blocks to print. The excess will be truncated

  Default value: `1`
* `-o`, `--output <OUTPUT>` — Where to write the results of the inspection If omitted, writes to stdout



## `kagami advanced kura sidecar`

Print the pipeline recovery sidecar JSON for a given height

**Usage:** `kagami advanced kura sidecar [OPTIONS] --height <HEIGHT>`

###### **Options:**

* `-H`, `--height <HEIGHT>` — The block height whose sidecar to print
* `-o`, `--output <OUTPUT>` — Where to write the sidecar JSON (default: stdout)



## `kagami advanced markdown-help`

Output CLI documentation in Markdown format

**Usage:** `kagami advanced markdown-help`



## `kagami advanced schema`

Generate the schema used for code generation in Iroha SDKs

**Usage:** `kagami advanced schema [OPTIONS]`

###### **Options:**

* `--genesis-out <GENESIS_OUT>` — Optional path to output genesis schema



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
