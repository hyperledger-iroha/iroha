# Command-Line Help for `kagami`

This document contains the help content for the `kagami` command-line program.

**Command Overview:**

* [`kagami`↴](#kagami)
* [`kagami wizard`↴](#kagami-wizard)
* [`kagami localnet-wizard`↴](#kagami-localnet-wizard)
* [`kagami localnet`↴](#kagami-localnet)
* [`kagami docker`↴](#kagami-docker)
* [`kagami keys`↴](#kagami-keys)
* [`kagami kagemusha`↴](#kagami-kagemusha)
* [`kagami kagemusha authenticate-release-v1`↴](#kagami-kagemusha-authenticate-release-v1)
* [`kagami kagemusha derive-mint-finality-next-epoch-v1`↴](#kagami-kagemusha-derive-mint-finality-next-epoch-v1)
* [`kagami genesis`↴](#kagami-genesis)
* [`kagami genesis sign`↴](#kagami-genesis-sign)
* [`kagami genesis generate`↴](#kagami-genesis-generate)
* [`kagami genesis generate default`↴](#kagami-genesis-generate-default)
* [`kagami genesis generate synthetic`↴](#kagami-genesis-generate-synthetic)
* [`kagami genesis materialize`↴](#kagami-genesis-materialize)
* [`kagami genesis validate`↴](#kagami-genesis-validate)
* [`kagami genesis validate-prepared`↴](#kagami-genesis-validate-prepared)
* [`kagami genesis embed-pop`↴](#kagami-genesis-embed-pop)
* [`kagami genesis normalize`↴](#kagami-genesis-normalize)
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
* [`kagami advanced kura scaling-evidence`↴](#kagami-advanced-kura-scaling-evidence)
* [`kagami advanced kura scaling-evidence stopped-tip`↴](#kagami-advanced-kura-scaling-evidence-stopped-tip)
* [`kagami advanced kura scaling-evidence facts`↴](#kagami-advanced-kura-scaling-evidence-facts)
* [`kagami advanced kura scaling-evidence prepare`↴](#kagami-advanced-kura-scaling-evidence-prepare)
* [`kagami advanced kura scaling-evidence export`↴](#kagami-advanced-kura-scaling-evidence-export)
* [`kagami advanced kura scaling-evidence replay`↴](#kagami-advanced-kura-scaling-evidence-replay)
* [`kagami advanced kura beacon-history`↴](#kagami-advanced-kura-beacon-history)
* [`kagami advanced kura print`↴](#kagami-advanced-kura-print)
* [`kagami advanced kura finality`↴](#kagami-advanced-kura-finality)
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
* `kagemusha` — Authenticate one complete KAGEMUSHA V1 release and its deployment evidence
* `genesis` — Commands related to genesis
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
* `--seed-fd <FD>` — Fixed scaling only: inherited read-only nonblocking pipe with exactly 64 lowercase hex bytes and EOF
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

* `--scaling-lanes <LANES>` — Generate a fixed execution-lane layout with four NPoS validators and autoscaling disabled. Use the same private development seed and options for both variants

  Possible values:
  - `1`:
    One execution lane
  - `4`:
    Four execution lanes sharing the same validator committee

* `--scaling-accounts <COUNT>` — Ordered workload accounts for a fixed scaling layout (4..=64, in groups of four). Defaults to four when --scaling-lanes is present
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
* `--sample-asset` — Register the optional sample asset and mint to the default account. The built-in KAGEMUSHA V1 asset is always emitted

  Default value: `false`
* `--asset-definition-id <ASSET_DEFINITION_ID>` — Register additional asset definition IDs owned by the generated client signer. Repeat the flag to register more than one asset definition. A localnet reserve is minted to the generated client signer for each requested asset definition
* `--block-cadence-ms <MILLISECONDS>` — Override the immutable signed block cadence in milliseconds. Leave unset to use the one-second localnet cadence
* `--consensus-mode <MODE>` — Consensus mode to emit in genesis/configs. Defaults to `permissioned` for generic localnets and `npos` for fixed scaling layouts. Sora profile localnets and perf profiles require `npos`

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

  Possible values: `ed25519`, `secp256k1`, `ml-dsa`, `gost3410-2012-256-paramset-a`, `gost3410-2012-256-paramset-b`, `gost3410-2012-256-paramset-c`, `gost3410-2012-512-paramset-a`, `gost3410-2012-512-paramset-b`, `bls_normal`, `bls_small`

* `--seed-hex <HEX>` — A 32-byte secret key-generation seed encoded as 64 hexadecimal characters.

   This is for reproducible fixtures. Omit it for OS-random production keys.
* `--out-dir <DIR>` — Write the key pair into a new owner-only custody directory.

   The directory must not contain any existing entries. Files are written as `public.key` and `private.key`; `--pop` also writes `pop.hex`. The private key never passes through standard output.
* `--pop` — Also output a BLS Proof-of-Possession (PoP) for this key (BLS-normal only). Written as `pop.hex` in the custody directory



## `kagami kagemusha`

Authenticate one complete KAGEMUSHA V1 release and its deployment evidence

**Usage:** `kagami kagemusha <COMMAND>`

###### **Subcommands:**

* `authenticate-release-v1` — Authenticate one complete KAGEMUSHA V1 release and its deployment evidence
* `derive-mint-finality-next-epoch-v1` — Derive one typed next-epoch parameter from four inherited private seed blocks
* `derive-mint-finality-epoch-schedule-v1` — Derive a bounded public epoch-maintenance schedule from one inherited seed pipe



## `kagami kagemusha authenticate-release-v1`

Authenticate one complete KAGEMUSHA V1 release and its deployment evidence

**Usage:** `kagami kagemusha authenticate-release-v1 --manifest <PATH> --validation-receipt <PATH> --authority-policy <PATH> --attestation <PATH> --recursive-profile <PATH> --artifact-root <PATH> --authority-review-projection <PATH> --authority-review-projection-sha256 <LOWER_HEX> --native-artifact-manifest <PATH> --native-artifact-manifest-sha256 <LOWER_HEX> --native-artifact <PATH>`

###### **Options:**

* `--manifest <PATH>` — Canonical Norito KAGEMUSHA V1 release manifest
* `--validation-receipt <PATH>` — Canonical Norito KAGEMUSHA V1 internal-validation receipt
* `--authority-policy <PATH>` — Canonical Norito locally trusted KAGEMUSHA V1 release-authority policy
* `--attestation <PATH>` — Canonical Norito KAGEMUSHA V1 threshold attestation
* `--recursive-profile <PATH>` — Canonical JSON recursive-verifier profile consumed by Core
* `--artifact-root <PATH>` — Absolute directory containing all 50 SHA-256-addressed release artifacts
* `--authority-review-projection <PATH>` — Canonical output from the separately pinned authority-review verifier
* `--authority-review-projection-sha256 <LOWER_HEX>` — SHA-256 pin for the exact authority-review projection bytes
* `--native-artifact-manifest <PATH>` — Canonical ABI23 c-jni native-artifact evidence manifest
* `--native-artifact-manifest-sha256 <LOWER_HEX>` — SHA-256 pin for the exact native-artifact manifest bytes
* `--native-artifact <PATH>` — Exact c-jni library whose bytes must match the native-artifact manifest



## `kagami kagemusha derive-mint-finality-next-epoch-v1`

Derive one typed public parameter without submitting a transaction.

**Usage:** `kagami kagemusha derive-mint-finality-next-epoch-v1 --network-id <NETWORK_ID> --epoch <EPOCH> --validator <PEER_ID> --seed-fd <FD>`

* `--network-id <NETWORK_ID>` — Exact canonical genesis-derived network identity
* `--epoch <EPOCH>` — Positive target epoch
* `--validator <PEER_ID>` — Repeat exactly four BLS-normal voters in strictly increasing PeerId order
* `--seed-fd <FD>` — Transferred read pipe descriptor at least 3; exactly four independent nonzero 32-byte seed blocks in voter order, followed by EOF

## `kagami kagemusha derive-mint-finality-epoch-schedule-v1`

Derive the public schedule consumed by `iroha taira epoch-maintenance`. Private
input uses the same owned pipe and is erased before public output. Required output
`genesis_roster` contains the epoch-zero public keys derived from those same seeds;
consumers compare it with their independently authenticated signed genesis.

**Usage:** `kagami kagemusha derive-mint-finality-epoch-schedule-v1 --network-id <NETWORK_ID> --epoch <EPOCH> --validator <PEER_ID> --seed-fd <FD> --epoch-count <EPOCH_COUNT> --payment-asset <PAYMENT_ASSET> --transaction-fee-maximum <TRANSACTION_FEE_MAXIMUM>`

* `--network-id <NETWORK_ID>` — Exact canonical genesis-derived network identity
* `--epoch <EPOCH>` — First positive target epoch
* `--validator <PEER_ID>` — Repeat exactly four BLS-normal voters in strictly increasing PeerId order
* `--seed-fd <FD>` — Transferred read pipe descriptor at least 3; exactly 128 seed bytes, followed by EOF
* `--epoch-count <EPOCH_COUNT>` — 1–256 consecutive epochs; overflow is rejected
* `--payment-asset <PAYMENT_ASSET>` — Sole asset authorized for maintenance fees
* `--transaction-fee-maximum <TRANSACTION_FEE_MAXIMUM>` — Positive maximum fee per transaction

Neither derivation command establishes election eligibility or submits a transaction.

## `kagami genesis`

Commands related to genesis

**Usage:** `kagami genesis <COMMAND>`

###### **Subcommands:**

* `sign` — Sign the genesis block
* `generate` — Generate a genesis configuration and standard-output in JSON format
* `materialize` — Materialize an incomplete source template with operator-provisioned public authority
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

**Usage:** `kagami genesis generate [OPTIONS] --ivm-dir <PATH> --genesis-public-key <MULTI_HASH> --kagemusha-mint-finality-parameters <PATH> [COMMAND]`

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
* `--kagemusha-mint-finality-parameters <PATH>` — Path to the explicitly provisioned public KAGEMUSHA mint-finality genesis parameters
* `--ivm-gas-limit-per-block <U64>` — Optional: set the custom parameter `ivm_gas_limit_per_block` (u64) in genesis so all peers agree on the block gas budget. If omitted, a sensible default (1,680,000) is applied
* `--consensus-mode <MODE>` — Select the consensus mode snapshot to seed in the genesis parameters (public dataspace requires NPoS; other dataspaces may use permissioned or NPoS)

  Possible values: `permissioned`, `npos`

* `--sm-openssl-preview <BOOL>` — Toggle the OpenSSL-backed SM preview helpers in the generated manifest

  Possible values: `true`, `false`

* `--default-hash <HASH>` — Override the default hash advertised in the manifest
* `--allowed-signing <ALGO>` — Replace the allowed signing algorithms (repeat flag to supply multiple values)

  Possible values: `ed25519`, `secp256k1`, `sm2`

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



## `kagami genesis materialize`

Materialize an incomplete source template with operator-provisioned public authority

**Usage:** `kagami genesis materialize --kagemusha-mint-finality-parameters <PATH> <TEMPLATE_FILE>`

###### **Arguments:**

* `<TEMPLATE_FILE>` — Incomplete genesis source file; the name must end in `.template.json`

###### **Options:**

* `--kagemusha-mint-finality-parameters <PATH>` — Explicitly provisioned public KAGEMUSHA mint-finality genesis parameters



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




## `kagami privacy-bootstrap`

Emit and validate fail-closed Taira exact-12 privacy bootstrap artifacts

**Usage:** `kagami privacy-bootstrap <COMMAND>`

###### **Subcommands:**

* `emit-taira-v1` — Emit one height-1 template of twelve ordered registration/activation pairs
* `validate-taira-v1` — Validate an emitted exact-12 instruction set and its digest inventory
* `validate-taira-nevo-review-v1` — Validate a reviewed Taira NEVO genesis source template without creating release artifacts
* `render-taira-release-v1` — Compose a secret-free Taira release plan, config, and non-signable genesis source template



## `kagami privacy-bootstrap emit-taira-v1`

Emit one height-1 template of twelve ordered registration/activation pairs

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

Validate a reviewed Taira NEVO genesis source template without creating release artifacts

**Usage:** `kagami privacy-bootstrap validate-taira-nevo-review-v1 --unsigned-genesis <UNSIGNED_GENESIS> --review <REVIEW>`

###### **Options:**

* `--unsigned-genesis <UNSIGNED_GENESIS>` — Exact non-signable NEVO genesis source template bound by the review manifest
* `--review <REVIEW>` — Deterministic public NEVO review manifest binding the genesis source template



## `kagami privacy-bootstrap render-taira-release-v1`

Compose a secret-free Taira release plan, config, and non-signable genesis source template

**Usage:** `kagami privacy-bootstrap render-taira-release-v1 --activation-instructions <ACTIVATION_INSTRUCTIONS> --activation-report <ACTIVATION_REPORT> --broker-public-export <BROKER_PUBLIC_EXPORT> --plan-template <PLAN_TEMPLATE> --config-template <CONFIG_TEMPLATE> --genesis-template <GENESIS_TEMPLATE> --nevo-review <NEVO_REVIEW> --plan-output <PLAN_OUTPUT> --config-output <CONFIG_OUTPUT> --genesis-output <GENESIS_OUTPUT> --broker-public-output <BROKER_PUBLIC_OUTPUT>`

###### **Options:**

* `--activation-instructions <ACTIVATION_INSTRUCTIONS>` — Exact-12 instruction JSON emitted by `emit-taira-v1`
* `--activation-report <ACTIVATION_REPORT>` — Digest report emitted together with the exact-12 instructions
* `--broker-public-export <BROKER_PUBLIC_EXPORT>` — Canonical public JSON emitted by the qualified peer-1 broker
* `--plan-template <PLAN_TEMPLATE>` — Canonical disabled Taira privacy plan template
* `--config-template <CONFIG_TEMPLATE>` — Canonical disabled peer-1 Taira config template
* `--genesis-template <GENESIS_TEMPLATE>` — Canonical non-signable Taira genesis source template without privacy bootstrap instructions
* `--nevo-review <NEVO_REVIEW>` — Deterministic public NEVO review manifest binding the genesis template
* `--plan-output <PLAN_OUTPUT>` — Fresh output path for the complete public release plan
* `--config-output <CONFIG_OUTPUT>` — Fresh output path for the complete peer-1 release config
* `--genesis-output <GENESIS_OUTPUT>` — Fresh `.template.json` output path for the overlaid release genesis source template
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

**Usage:** `kagami advanced kura <COMMAND>`

###### **Subcommands:**

* `scaling-evidence` — Prepare, export or independently replay canonical scaling evidence
* `beacon-history` — Project bounded typed public beacon candidates, with explicit coverage limits
* `print` — Print contents of a certain length of the blocks
* `finality` — Verify a locally anchored retained prefix and export its exact finality proof
* `sidecar` — Print the pipeline recovery sidecar JSON for a given height



## `kagami advanced kura scaling-evidence`

Prepare, export or independently replay canonical scaling evidence

**Usage:** `kagami advanced kura scaling-evidence <COMMAND>`

###### **Subcommands:**

* `stopped-tip` — Observe a stopped store's durable height under retained original genesis
* `facts` — Authenticate original launch inputs and publish canonical preparation facts
* `prepare` — Prepare two canonical transports from independently retained launch facts
* `export` — Authenticate an immutable Kura interval and publish one canonical proof
* `replay` — Reauthenticate a canonical proof and emit its complete ordered rows



## `kagami advanced kura scaling-evidence stopped-tip`

Observe a stopped store's durable height under retained original genesis

**Usage:** `kagami advanced kura scaling-evidence stopped-tip --invocation-id <INVOCATION_ID> --signed-genesis <SIGNED_GENESIS> --signed-genesis-sha256 <SIGNED_GENESIS_SHA256> --signed-genesis-max-bytes <SIGNED_GENESIS_MAX_BYTES> --network-id <NETWORK_ID> --block-store <BLOCK_STORE> --merge-log <MERGE_LOG> --reply-max-bytes <REPLY_MAX_BYTES> --first-height <FIRST_HEIGHT> --last-height <LAST_HEIGHT> --max-committed-blocks <MAX_COMMITTED_BLOCKS> --max-store-data-bytes <MAX_STORE_DATA_BYTES> --max-carrier-bytes <MAX_CARRIER_BYTES> --max-merge-log-bytes <MAX_MERGE_LOG_BYTES> --max-merge-frames <MAX_MERGE_FRAMES> --reader-max-output-bytes <READER_MAX_OUTPUT_BYTES> --max-decode-allocation-bytes <MAX_DECODE_ALLOCATION_BYTES> --owner-uid <OWNER_UID>`

###### **Options:**

* `--invocation-id <INVOCATION_ID>` — Independently selected lowercase SHA-256 invocation identity
* `--signed-genesis <SIGNED_GENESIS>` — Absolute path to the independently retained original canonical signed genesis
* `--signed-genesis-sha256 <SIGNED_GENESIS_SHA256>` — Independently pinned raw SHA-256 of the original signed genesis
* `--signed-genesis-max-bytes <SIGNED_GENESIS_MAX_BYTES>` — Maximum original genesis bytes, between 1 and 33554432
* `--network-id <NETWORK_ID>` — Expected genesis-header NetworkId in its canonical checked hash literal form
* `--block-store <BLOCK_STORE>` — Exact absolute stopped lane directory containing the canonical block journals
* `--merge-log <MERGE_LOG>` — Exact absolute stopped canonical merge-log file
* `--reply-max-bytes <REPLY_MAX_BYTES>` — Reserved complete JSON reply bytes, including its final newline
* `--first-height <FIRST_HEIGHT>` — First required carrier height, inclusive
* `--last-height <LAST_HEIGHT>` — Last required carrier height, inclusive
* `--max-committed-blocks <MAX_COMMITTED_BLOCKS>` — Maximum complete journal height admitted before reading
* `--max-store-data-bytes <MAX_STORE_DATA_BYTES>` — Maximum underlying blocks.data bytes
* `--max-carrier-bytes <MAX_CARRIER_BYTES>` — Maximum canonical wire bytes for one carrier
* `--max-merge-log-bytes <MAX_MERGE_LOG_BYTES>` — Maximum complete merge-log bytes
* `--max-merge-frames <MAX_MERGE_FRAMES>` — Maximum frames in the complete merge log
* `--reader-max-output-bytes <READER_MAX_OUTPUT_BYTES>` — Maximum cumulative carrier and merge-entry bytes returned by the reader
* `--max-decode-allocation-bytes <MAX_DECODE_ALLOCATION_BYTES>` — Maximum cumulative owned allocation per decoder invocation
* `--owner-uid <OWNER_UID>` — Independently expected Unix owner of the store directories and files



## `kagami advanced kura scaling-evidence facts`

Authenticate original launch inputs and publish canonical preparation facts

**Usage:** `kagami advanced kura scaling-evidence facts --invocation-id <INVOCATION_ID> --manifest <MANIFEST> --manifest-sha256 <MANIFEST_SHA256> --manifest-max-bytes <MANIFEST_MAX_BYTES> --signed-genesis <SIGNED_GENESIS> --signed-genesis-sha256 <SIGNED_GENESIS_SHA256> --signed-genesis-max-bytes <SIGNED_GENESIS_MAX_BYTES> --peer-config <PEER_CONFIG> <PEER_CONFIG> <PEER_CONFIG> <PEER_CONFIG> --peer-config-sha256 <PEER_CONFIG_SHA256> <PEER_CONFIG_SHA256> <PEER_CONFIG_SHA256> <PEER_CONFIG_SHA256> --peer-config-max-bytes <PEER_CONFIG_MAX_BYTES> --context <CONTEXT> --context-sha256 <CONTEXT_SHA256> --context-max-bytes <CONTEXT_MAX_BYTES> --journal <JOURNAL> --journal-sha256 <JOURNAL_SHA256> --journal-max-bytes <JOURNAL_MAX_BYTES> --finality <FINALITY> --finality-sha256 <FINALITY_SHA256> --finality-max-bytes <FINALITY_MAX_BYTES> --queries <QUERIES> --queries-sha256 <QUERIES_SHA256> --queries-max-bytes <QUERIES_MAX_BYTES> --chain-id <CHAIN_ID> --network-id <NETWORK_ID> --chain-discriminant <CHAIN_DISCRIMINANT> --genesis-public-key <GENESIS_PUBLIC_KEY> --validator <VALIDATOR> <VALIDATOR> <VALIDATOR> <VALIDATOR> --lanes <LANES> --workload-seed <WORKLOAD_SEED> --pair-index <PAIR_INDEX> --account <ACCOUNT> <ACCOUNT> <ACCOUNT> <ACCOUNT>... --rate-numerator <RATE_NUMERATOR> --rate-denominator <RATE_DENOMINATOR> --warmup-ns <WARMUP_NS> --measurement-ns <MEASUREMENT_NS> --drain-ns <DRAIN_NS> --submission-lag-bound-ns <SUBMISSION_LAG_BOUND_NS> --preparation-lookahead <PREPARATION_LOOKAHEAD> --preparation-concurrency <PREPARATION_CONCURRENCY> --preparation-ahead-ns <PREPARATION_AHEAD_NS> --max-submissions <MAX_SUBMISSIONS> --max-in-flight <MAX_IN_FLIGHT> --max-status-requests <MAX_STATUS_REQUESTS> --poll-interval-ns <POLL_INTERVAL_NS> --journal-max-requests <JOURNAL_MAX_REQUESTS> --resource-interval-ns <RESOURCE_INTERVAL_NS> --resource-response-deadline-ns <RESOURCE_RESPONSE_DEADLINE_NS> --resource-max-start-lag-ns <RESOURCE_MAX_START_LAG_NS> --proof-max-bytes <PROOF_MAX_BYTES> --verification-input-max-bytes <VERIFICATION_INPUT_MAX_BYTES> --verification-output-max-bytes <VERIFICATION_OUTPUT_MAX_BYTES> --max-heights <MAX_HEIGHTS> --max-requests <MAX_REQUESTS> --max-leaves-per-carrier <MAX_LEAVES_PER_CARRIER> --first-height <FIRST_HEIGHT> --last-height <LAST_HEIGHT> --max-committed-blocks <MAX_COMMITTED_BLOCKS> --max-store-data-bytes <MAX_STORE_DATA_BYTES> --max-carrier-bytes <MAX_CARRIER_BYTES> --max-merge-log-bytes <MAX_MERGE_LOG_BYTES> --max-merge-frames <MAX_MERGE_FRAMES> --reader-max-output-bytes <READER_MAX_OUTPUT_BYTES> --max-decode-allocation-bytes <MAX_DECODE_ALLOCATION_BYTES> --owner-uid <OWNER_UID> --block-store <BLOCK_STORE> --merge-log <MERGE_LOG> --facts-output <FACTS_OUTPUT> --source-max-bytes <SOURCE_MAX_BYTES> --facts-max-bytes <FACTS_MAX_BYTES> --total-max-bytes <TOTAL_MAX_BYTES> --assembly-decode-max-bytes <ASSEMBLY_DECODE_MAX_BYTES> --reply-max-bytes <REPLY_MAX_BYTES>`

###### **Options:**

* `--invocation-id <INVOCATION_ID>` — Independently selected lowercase SHA-256 invocation identity
* `--manifest <MANIFEST>` — Original final manifest absolute path; external artifact references are rejected
* `--manifest-sha256 <MANIFEST_SHA256>` — Independently pinned raw SHA-256 of the final manifest
* `--manifest-max-bytes <MANIFEST_MAX_BYTES>` — Maximum original final manifest bytes
* `--signed-genesis <SIGNED_GENESIS>` — Original canonical signed genesis absolute path
* `--signed-genesis-sha256 <SIGNED_GENESIS_SHA256>` — Independently pinned raw SHA-256 of canonical signed genesis
* `--signed-genesis-max-bytes <SIGNED_GENESIS_MAX_BYTES>` — Maximum original signed genesis bytes
* `--peer-config <PEER_CONFIG>` — Four original final peer config absolute paths, in independently selected validator order
* `--peer-config-sha256 <PEER_CONFIG_SHA256>` — Four independently pinned raw config SHA-256 values, in the same order
* `--peer-config-max-bytes <PEER_CONFIG_MAX_BYTES>` — Maximum bytes for each of the four original peer configs
* `--context <CONTEXT>` — Original independent canonical genesis HeightContext absolute path
* `--context-sha256 <CONTEXT_SHA256>` — Independently pinned raw SHA-256 of the original context
* `--context-max-bytes <CONTEXT_MAX_BYTES>` — Maximum original context bytes, at most 8388608
* `--journal <JOURNAL>` — Original complete signed-request collector journal absolute path
* `--journal-sha256 <JOURNAL_SHA256>` — Independently pinned raw SHA-256 of the complete original journal
* `--journal-max-bytes <JOURNAL_MAX_BYTES>` — Maximum complete original journal bytes
* `--finality <FINALITY>` — Original canonical Vec<FinalizedNativeContextV1> absolute path
* `--finality-sha256 <FINALITY_SHA256>` — Independently pinned raw SHA-256 of the complete finality vector
* `--finality-max-bytes <FINALITY_MAX_BYTES>` — Maximum complete finality vector bytes
* `--queries <QUERIES>` — Original canonical Vec<CommittedTransaction> absolute path, preserving every query
* `--queries-sha256 <QUERIES_SHA256>` — Independently pinned raw SHA-256 of the complete query vector
* `--queries-max-bytes <QUERIES_MAX_BYTES>` — Maximum complete query vector bytes
* `--chain-id <CHAIN_ID>` — Independently selected canonical chain label
* `--network-id <NETWORK_ID>` — Exact expected genesis-header NetworkId in canonical checked hash literal form
* `--chain-discriminant <CHAIN_DISCRIMINANT>` — Independently selected I105 chain discriminant
* `--genesis-public-key <GENESIS_PUBLIC_KEY>` — Independently selected public genesis signer; no private signing input is accepted
* `--validator <VALIDATOR>` — Four selected validator public keys in the same original role order as peer configs
* `--lanes <LANES>` — Fixed execution-lane count, either 1 or 4

  Possible values:
  - `1`:
    One fixed active execution lane
  - `4`:
    Four fixed active execution lanes

* `--workload-seed <WORKLOAD_SEED>` — Public workload seed as exactly 64 lowercase hex characters; never the development key seed
* `--pair-index <PAIR_INDEX>` — Independent paired-run index from 1 through 5
* `--account <ACCOUNT>` — Ordered canonical I105 account pool, 4 through 64 accounts in complete groups of four
* `--rate-numerator <RATE_NUMERATOR>` — Exact positive rational offered-rate numerator, in requests per second
* `--rate-denominator <RATE_DENOMINATOR>` — Exact positive rational offered-rate denominator
* `--warmup-ns <WARMUP_NS>` — Independent warmup duration in nanoseconds
* `--measurement-ns <MEASUREMENT_NS>` — Independent measurement duration in nanoseconds
* `--drain-ns <DRAIN_NS>` — Independent drain duration in nanoseconds
* `--submission-lag-bound-ns <SUBMISSION_LAG_BOUND_NS>` — Maximum allowed submission lag in nanoseconds
* `--preparation-lookahead <PREPARATION_LOOKAHEAD>` — Independently selected signed-request preparation lookahead
* `--preparation-concurrency <PREPARATION_CONCURRENCY>` — Independently selected preparation concurrency
* `--preparation-ahead-ns <PREPARATION_AHEAD_NS>` — Maximum preparation lead time in nanoseconds
* `--max-submissions <MAX_SUBMISSIONS>` — Maximum concurrent submissions selected before collection
* `--max-in-flight <MAX_IN_FLIGHT>` — Maximum in-flight requests selected before collection
* `--max-status-requests <MAX_STATUS_REQUESTS>` — Maximum concurrent status requests selected before collection
* `--poll-interval-ns <POLL_INTERVAL_NS>` — Independent status poll interval in nanoseconds
* `--journal-max-requests <JOURNAL_MAX_REQUESTS>` — Maximum complete journal schedule requests
* `--resource-interval-ns <RESOURCE_INTERVAL_NS>` — Independent resource sampling interval in nanoseconds
* `--resource-response-deadline-ns <RESOURCE_RESPONSE_DEADLINE_NS>` — Independent resource response deadline in nanoseconds
* `--resource-max-start-lag-ns <RESOURCE_MAX_START_LAG_NS>` — Maximum resource sampling start lag in nanoseconds
* `--proof-max-bytes <PROOF_MAX_BYTES>` — Independent admitted canonical proof byte allocation
* `--verification-input-max-bytes <VERIFICATION_INPUT_MAX_BYTES>` — Maximum cumulative verifier input bytes including the signed schedule
* `--verification-output-max-bytes <VERIFICATION_OUTPUT_MAX_BYTES>` — Reserved canonical proof and complete result-row output bytes
* `--max-heights <MAX_HEIGHTS>` — Maximum contiguous global heights, at most 65536
* `--max-requests <MAX_REQUESTS>` — Maximum complete scheduled request count, at most 1000000
* `--max-leaves-per-carrier <MAX_LEAVES_PER_CARRIER>` — Maximum ordinary and merged leaves in one carrier, at most 1000000
* `--first-height <FIRST_HEIGHT>` — First required carrier height, inclusive
* `--last-height <LAST_HEIGHT>` — Last required carrier height, inclusive
* `--max-committed-blocks <MAX_COMMITTED_BLOCKS>` — Maximum complete journal height admitted before reading
* `--max-store-data-bytes <MAX_STORE_DATA_BYTES>` — Maximum underlying blocks.data bytes
* `--max-carrier-bytes <MAX_CARRIER_BYTES>` — Maximum canonical wire bytes for one carrier
* `--max-merge-log-bytes <MAX_MERGE_LOG_BYTES>` — Maximum complete merge-log bytes
* `--max-merge-frames <MAX_MERGE_FRAMES>` — Maximum frames in the complete merge log
* `--reader-max-output-bytes <READER_MAX_OUTPUT_BYTES>` — Maximum cumulative carrier and merge-entry bytes returned by the reader
* `--max-decode-allocation-bytes <MAX_DECODE_ALLOCATION_BYTES>` — Maximum cumulative owned allocation per decoder invocation
* `--owner-uid <OWNER_UID>` — Independently expected Unix owner of the store directories and files
* `--block-store <BLOCK_STORE>` — Exact absolute stopped validator directory containing canonical block journals
* `--merge-log <MERGE_LOG>` — Exact absolute original canonical merge log
* `--facts-output <FACTS_OUTPUT>` — New absolute canonical facts output; existing stage or destination fails
* `--source-max-bytes <SOURCE_MAX_BYTES>` — Reserved cumulative original-file bytes, at most 268435456
* `--facts-max-bytes <FACTS_MAX_BYTES>` — Reserved canonical facts output bytes, at most 268435456
* `--total-max-bytes <TOTAL_MAX_BYTES>` — Aggregate source and facts reservations, at most 268435456
* `--assembly-decode-max-bytes <ASSEMBLY_DECODE_MAX_BYTES>` — Cumulative Norito allocation budget for the entire assembly, at most 536870912
* `--reply-max-bytes <REPLY_MAX_BYTES>` — Maximum complete five-field JSON reply including its final newline



## `kagami advanced kura scaling-evidence prepare`

Prepare two canonical transports from independently retained launch facts

**Usage:** `kagami advanced kura scaling-evidence prepare --invocation-id <INVOCATION_ID> --facts <FACTS> --facts-sha256 <FACTS_SHA256> --facts-max-bytes <FACTS_MAX_BYTES> --request-output <REQUEST_OUTPUT> --bundle-output <BUNDLE_OUTPUT> --request-max-bytes <REQUEST_MAX_BYTES> --bundle-max-bytes <BUNDLE_MAX_BYTES> --total-max-bytes <TOTAL_MAX_BYTES> --reply-max-bytes <REPLY_MAX_BYTES>`

###### **Options:**

* `--invocation-id <INVOCATION_ID>` — Independently selected lowercase SHA-256 invocation identity
* `--facts <FACTS>` — Absolute path to independently retained canonical preparation facts
* `--facts-sha256 <FACTS_SHA256>` — Independently pinned raw SHA-256 of the facts file
* `--facts-max-bytes <FACTS_MAX_BYTES>` — Reserved facts bytes, between 1 and 268435456
* `--request-output <REQUEST_OUTPUT>` — New absolute launcher request path; existing destinations are rejected
* `--bundle-output <BUNDLE_OUTPUT>` — New absolute supplied evidence bundle path; existing destinations are rejected
* `--request-max-bytes <REQUEST_MAX_BYTES>` — Reserved request output bytes, between 1 and 268435456
* `--bundle-max-bytes <BUNDLE_MAX_BYTES>` — Reserved bundle output bytes, between 1 and 268435456
* `--total-max-bytes <TOTAL_MAX_BYTES>` — Aggregate facts and both output reservations, between 1 and 268435456
* `--reply-max-bytes <REPLY_MAX_BYTES>` — Reserved complete JSON reply bytes, including its final newline



## `kagami advanced kura scaling-evidence export`

Authenticate an immutable Kura interval and publish one canonical proof

**Usage:** `kagami advanced kura scaling-evidence export --invocation-id <INVOCATION_ID> --request <REQUEST> --request-sha256 <REQUEST_SHA256> --request-max-bytes <REQUEST_MAX_BYTES> --input <INPUT> --input-sha256 <INPUT_SHA256> --input-max-bytes <INPUT_MAX_BYTES> --reply-max-bytes <REPLY_MAX_BYTES> --block-store <BLOCK_STORE> --merge-log <MERGE_LOG> --output <OUTPUT> --output-max-bytes <OUTPUT_MAX_BYTES> --first-height <FIRST_HEIGHT> --last-height <LAST_HEIGHT> --max-committed-blocks <MAX_COMMITTED_BLOCKS> --max-store-data-bytes <MAX_STORE_DATA_BYTES> --max-carrier-bytes <MAX_CARRIER_BYTES> --max-merge-log-bytes <MAX_MERGE_LOG_BYTES> --max-merge-frames <MAX_MERGE_FRAMES> --reader-max-output-bytes <READER_MAX_OUTPUT_BYTES> --max-decode-allocation-bytes <MAX_DECODE_ALLOCATION_BYTES> --owner-uid <OWNER_UID>`

###### **Options:**

* `--invocation-id <INVOCATION_ID>` — Independently selected lowercase SHA-256 invocation identity
* `--request <REQUEST>` — Absolute path to the independently retained canonical launcher request
* `--request-sha256 <REQUEST_SHA256>` — Independently pinned raw SHA-256 of the request file
* `--request-max-bytes <REQUEST_MAX_BYTES>` — Reserved request bytes, between 1 and 268435456
* `--input <INPUT>` — Absolute path to the supplied evidence bundle or canonical replay proof
* `--input-sha256 <INPUT_SHA256>` — Independently pinned raw SHA-256 of the input file
* `--input-max-bytes <INPUT_MAX_BYTES>` — Reserved input bytes, between 1 and 268435456
* `--reply-max-bytes <REPLY_MAX_BYTES>` — Reserved complete JSON reply bytes, including its final newline
* `--block-store <BLOCK_STORE>` — Exact absolute lane directory containing the canonical block journals
* `--merge-log <MERGE_LOG>` — Exact absolute canonical merge-log file
* `--output <OUTPUT>` — New absolute proof path; existing destinations are rejected
* `--output-max-bytes <OUTPUT_MAX_BYTES>` — Reserved canonical output bytes, between 1 and 268435456
* `--first-height <FIRST_HEIGHT>` — First required carrier height, inclusive
* `--last-height <LAST_HEIGHT>` — Last required carrier height, inclusive
* `--max-committed-blocks <MAX_COMMITTED_BLOCKS>` — Maximum complete journal height admitted before reading
* `--max-store-data-bytes <MAX_STORE_DATA_BYTES>` — Maximum underlying blocks.data bytes
* `--max-carrier-bytes <MAX_CARRIER_BYTES>` — Maximum canonical wire bytes for one carrier
* `--max-merge-log-bytes <MAX_MERGE_LOG_BYTES>` — Maximum complete merge-log bytes
* `--max-merge-frames <MAX_MERGE_FRAMES>` — Maximum frames in the complete merge log
* `--reader-max-output-bytes <READER_MAX_OUTPUT_BYTES>` — Maximum cumulative carrier and merge-entry bytes returned by the reader
* `--max-decode-allocation-bytes <MAX_DECODE_ALLOCATION_BYTES>` — Maximum cumulative owned allocation per decoder invocation
* `--owner-uid <OWNER_UID>` — Independently expected Unix owner of the store directories and files



## `kagami advanced kura scaling-evidence replay`

Reauthenticate a canonical proof and emit its complete ordered rows

**Usage:** `kagami advanced kura scaling-evidence replay --invocation-id <INVOCATION_ID> --request <REQUEST> --request-sha256 <REQUEST_SHA256> --request-max-bytes <REQUEST_MAX_BYTES> --input <INPUT> --input-sha256 <INPUT_SHA256> --input-max-bytes <INPUT_MAX_BYTES> --reply-max-bytes <REPLY_MAX_BYTES> --proof-iroha-hash <PROOF_IROHA_HASH>`

###### **Options:**

* `--invocation-id <INVOCATION_ID>` — Independently selected lowercase SHA-256 invocation identity
* `--request <REQUEST>` — Absolute path to the independently retained canonical launcher request
* `--request-sha256 <REQUEST_SHA256>` — Independently pinned raw SHA-256 of the request file
* `--request-max-bytes <REQUEST_MAX_BYTES>` — Reserved request bytes, between 1 and 268435456
* `--input <INPUT>` — Absolute path to the supplied evidence bundle or canonical replay proof
* `--input-sha256 <INPUT_SHA256>` — Independently pinned raw SHA-256 of the input file
* `--input-max-bytes <INPUT_MAX_BYTES>` — Reserved input bytes, between 1 and 268435456
* `--reply-max-bytes <REPLY_MAX_BYTES>` — Reserved complete JSON reply bytes, including its final newline
* `--proof-iroha-hash <PROOF_IROHA_HASH>` — Independently pinned marked Iroha hash of the canonical proof bytes



## `kagami advanced kura beacon-history`

Project bounded typed public beacon candidates, with explicit coverage limits

**Usage:** `kagami advanced kura beacon-history [OPTIONS] --from <BLOCK_HEIGHT> --length <LENGTH> <PATH_TO_BLOCK_STORE>`

###### **Arguments:**

* `<PATH_TO_BLOCK_STORE>` — Exact lane directory containing the canonical block journals

###### **Options:**

* `-f`, `--from <BLOCK_HEIGHT>` — First block height in the exact inspection interval
* `--length <LENGTH>` — Exact number of blocks, from the --from height (1..=4096)
* `--merge-sidecar <FILE>` — Exact canonical public merge-entry file; repeat for referenced carriers only
* `-o`, `--output <OUTPUT>` — Write bounded JSON outside the inspected store; defaults to stdout



## `kagami advanced kura print`

Print contents of a certain length of the blocks

**Usage:** `kagami advanced kura print [OPTIONS] <PATH_TO_BLOCK_STORE>`

###### **Arguments:**

* `<PATH_TO_BLOCK_STORE>` — Exact lane directory containing the canonical block journals

###### **Options:**

* `-f`, `--from <BLOCK_HEIGHT>` — Height of the block from which start the inspection. Defaults to the latest block height
* `-n`, `--length <LENGTH>` — Number of the blocks to print. The excess will be truncated

  Default value: `1`
* `-o`, `--output <OUTPUT>` — Where to write the results of the inspection If omitted, writes to stdout



## `kagami advanced kura finality`

Verify a locally anchored retained prefix and export its exact finality proof

**Usage:** `kagami advanced kura finality [OPTIONS] --height <HEIGHT> <PATH_TO_BLOCK_STORE>`

###### **Arguments:**

* `<PATH_TO_BLOCK_STORE>` — Exact lane directory containing the canonical block journals

###### **Options:**

* `-H`, `--height <HEIGHT>` — Verify all heights from genesis through this height (1..=4096)
* `-o`, `--output <OUTPUT>` — Write the public JSON outside the inspected store (default: stdout)



## `kagami advanced kura sidecar`

Print the pipeline recovery sidecar JSON for a given height

**Usage:** `kagami advanced kura sidecar [OPTIONS] --height <HEIGHT> <PATH_TO_BLOCK_STORE>`

###### **Arguments:**

* `<PATH_TO_BLOCK_STORE>` — Exact lane directory containing the canonical block journals

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
