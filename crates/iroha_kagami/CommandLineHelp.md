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
* [`kagami genesis pop`↴](#kagami-genesis-pop)
* [`kagami genesis embed-pop`↴](#kagami-genesis-embed-pop)
* [`kagami genesis normalize`↴](#kagami-genesis-normalize)
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
  kagami wizard --profile nexus
  kagami localnet --out-dir ./localnet
  kagami docker --peers 4 --config-dir ./localnet --image hyperledger/iroha:dev --out-file docker-compose.yml
  kagami keys --algorithm bls_normal --pop --json
  kagami advanced markdown-help


###### **Subcommands:**

* `wizard` — Guided node/bootstrap flow for configuring a peer against an existing network profile
* `localnet-wizard` — Guided disposable local devnet flow for generating peers, configs, genesis, and scripts
* `localnet` — Generate a bare-metal local network: genesis, per-peer configs, client config, and scripts
* `docker` — Generate Docker Compose deployment manifests from an existing config/genesis directory
* `keys` — Generate cryptographic key pairs and optional validator Proofs-of-Possession
* `genesis` — Commands related to genesis
* `verify` — Verify a genesis manifest against a preset profile
* `advanced` — Advanced low-level helpers for codec conversion, schema generation, block inspection, and docs

###### **Options:**

* `--ui-mode <MODE>` — Control how Kagami formats status messages (auto detects TTY by default)

  Default value: `auto`

  Possible values: `auto`, `plain`, `rich`




## `kagami wizard`

Guided node/bootstrap flow for configuring a peer against an existing network profile

**Usage:** `kagami wizard [OPTIONS]`

###### **Options:**

* `--profile <PROFILE>` — Optional preset profile; if omitted, the wizard prompts for one

  Possible values:
  - `iroha2`:
    Vanilla single-lane Iroha 2 style network (no Sora profile needed)
  - `nexus`:
    Sora Nexus (mainnet)
  - `taira`:
    Sora Taira (testnet)

* `--output-dir <PATH>` — Directory where generated config/genesis files will be written

  Default value: `wizard-output`
* `--non-interactive` — Run non-interactively, accepting defaults for prompts that are not supplied via flags
* `--chain-id <CHAIN>` — Override the default chain identifier
* `--p2p-host <HOST>` — Override the public P2P host/IP advertised for this peer
* `--p2p-port <PORT>` — Override the public P2P port for this peer
* `--torii-host <HOST>` — Override the Torii host/IP advertised for this peer
* `--torii-port <PORT>` — Override the Torii port for this peer
* `--relay-mode <RELAY_MODE>` — Override the relay mode instead of prompting interactively

  Possible values: `disabled`, `hub`, `spoke`, `assist`

* `--relay-hub-address <HOST:PORT>` — Relay hub addresses (`host:port`), repeat once per hub when relay mode uses them
* `--trusted-peers <PEERS>` — Override the bootstrap peer (`pubkey@host:port`). Comma-separated for multiple entries
* `--trusted-peers-pop <POPS>` — Comma-separated PoP entries for trusted peers (`pubkey=pop_hex`)



## `kagami localnet-wizard`

Guided disposable local devnet flow for generating peers, configs, genesis, and scripts

**Usage:** `kagami localnet-wizard`



## `kagami localnet`

Generate a bare-metal local network: genesis, per-peer configs, client config, and scripts

**Usage:** `kagami localnet [OPTIONS] --out-dir <DIR>`

###### **Options:**

* `-p`, `--peers <COUNT>` — Number of peers to generate

  Default value: `4`
* `-s`, `--seed <SEED>` — Optional UTF-8 seed for deterministic keys
* `--build-line <LINE>` — Select the build line (`iroha2` or `iroha3`) for DA/RBC defaults. Defaults to `iroha3`; consensus still defaults to `permissioned` unless a profile or perf preset requires `npos`

  Default value: `iroha3`

  Possible values: `iroha2`, `iroha3`

* `--sora-profile <PROFILE>` — Enable Sora profile defaults; `nexus` enforces public dataspace rules (NPoS). Requires `--build-line iroha3` and at least 4 peers

  Possible values: `dataspace`, `nexus`

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
* `--sample-asset` — Register the optional sample asset and mint to the default account. The built-in offline-note asset is always emitted

  Default value: `false`
* `--block-time-ms <MILLISECONDS>` — Override the consensus block time (milliseconds) in generated manifests/configs. Leave unset to use the fast localnet pipeline defaults. If only one of `--block-time-ms`/`--commit-time-ms` is supplied, Kagami mirrors it to the other
* `--commit-time-ms <MILLISECONDS>` — Override the consensus commit timeout (milliseconds) in generated manifests/configs. Leave unset to use the fast localnet pipeline defaults. If only one of `--block-time-ms`/`--commit-time-ms` is supplied, Kagami mirrors it to the other
* `--redundant-send-r <COUNT>` — Override redundant send fanout (r) for block payload dissemination
* `--consensus-mode <MODE>` — Consensus mode to emit in genesis/configs. Defaults to `permissioned` for generic localnets. Sora profile localnets and perf profiles require `npos`. Sora profile localnets require `npos` because the global merge ledger is NPoS

  Possible values: `permissioned`, `npos`

* `--next-consensus-mode <MODE>` — Optional staged consensus mode to activate at `mode_activation_height`

  Possible values: `permissioned`, `npos`

* `--mode-activation-height <HEIGHT>` — Optional activation height for switching to `next_consensus_mode`



## `kagami docker`

Generate Docker Compose deployment manifests from an existing config/genesis directory

**Usage:** `kagami docker [OPTIONS] --peers <COUNT> --config-dir <DIR> --image <NAME> --out-file <FILE>`

###### **Options:**

* `-p`, `--peers <COUNT>` — Number of peer services in the configuration
* `-s`, `--seed <SEED>` — UTF-8 seed for deterministic key-generation
* `-H`, `--healthcheck` — Includes a healthcheck for every service in the configuration.

   Healthchecks use predefined settings.

   For more details on healthcheck configuration in Docker Compose files, see: <https://docs.docker.com/compose/compose-file/compose-file-v3/#healthcheck>
* `-c`, `--config-dir <DIR>` — Directory with Iroha configuration. It will be mapped to a volume for each container.

   The directory should contain `genesis.json`. If you plan to upgrade the executor at genesis, include the executor bytecode file and reference it from `genesis.json`.
* `--peer-config <FILE>` — Optional TOML file describing peer names and port mappings.

   The file must contain an array named `peers`, for example:

   ```toml [[peers]] name = "alpha" p2p_port = 2000 api_port = 9000 [[peers]] name = "beta" p2p_port = 2001 api_port = 9001 ```
* `-i`, `--image <NAME>` — Docker image used by the peer services.

   By default, the image is pulled from Docker Hub if not cached. Pass the `--build` option to build the image from a Dockerfile instead.

   **Note**: Swarm only guarantees that the Docker Compose configuration it generates is compatible with the same Git revision it is built from itself. Therefore, if the specified image is not compatible with the version of Swarm you are running, the generated configuration might not work.
* `-b`, `--build <DIR>` — Build the image from the Dockerfile in the specified directory. Do not rebuild if the image has been cached.

   The provided path is resolved relative to the current working directory.
* `--no-cache` — Always pull or rebuild the image even if it is cached locally
* `-o`, `--out-file <FILE>` — Path to the target Compose configuration file.

   If the file exists, the app will prompt its overwriting. If the TTY is not interactive, the app will stop execution with a non-zero exit code. To overwrite the file anyway, pass the `--force` flag.
* `-P`, `--print` — Print the generated configuration to stdout instead of writing it to the target file.

   Note that the target path still needs to be provided, as it is used to resolve paths.
* `-F`, `--force` — Overwrite the target file if it already exists
* `--no-banner` — Do not include the banner with the generation notice in the file.

   The banner includes the seed to help with reproducibility.
* `--consensus-mode <MODE>` — Consensus mode to stamp into the generated genesis (optional)

  Possible values: `permissioned`, `npos`

* `--next-consensus-mode <MODE>` — Optional staged consensus mode to activate at `mode_activation_height`

  Possible values: `permissioned`, `npos`

* `--mode-activation-height <HEIGHT>` — Optional activation height for switching to `next_consensus_mode` (requires `--next-consensus-mode`)



## `kagami keys`

Generate cryptographic key pairs and optional validator Proofs-of-Possession

**Usage:** `kagami keys [OPTIONS]`

###### **Options:**

* `-a`, `--algorithm <ALGORITHM>` — An algorithm to use for the key-pair generation

  Default value: `ed25519`

  Possible values: `ed25519`, `secp256k1`, `ml-dsa`, `bls_normal`, `bls_small`

* `-p`, `--private-key <PRIVATE_KEY>` — A private key to generate the key-pair from

   `--private-key` specifies the payload of the private key, while `--algorithm` specifies its algorithm.
* `-s`, `--seed <SEED>` — The Unicode `seed` string to generate the key-pair from
* `-j`, `--json` — Output the key-pair in JSON format
* `--json-mh-prefixed` — Use algorithm-prefixed multihash strings in JSON (e.g., "ml-dsa:...")
* `-c`, `--compact` — Output the key-pair without additional text
* `--pop` — Also output a BLS Proof-of-Possession (PoP) for this key (BLS-normal only). Printed as hex in JSON or plain hex in compact mode



## `kagami genesis`

Commands related to genesis

**Usage:** `kagami genesis <COMMAND>`

###### **Subcommands:**

* `sign` — Sign the genesis block
* `generate` — Generate a genesis configuration and standard-output in JSON format
* `validate` — Validate a genesis JSON file and report invalid identifiers
* `pop` — Produce a BLS PoP (Proof-of-Possession) for a consensus key (BLS-normal)
* `embed-pop` — Embed one or more PoPs into a genesis JSON manifest (inline `topology` entries carrying `pop_hex`)
* `normalize` — Expand a genesis manifest and show the final ordered transactions



## `kagami genesis sign`

Sign the genesis block

**Usage:** `kagami genesis sign [OPTIONS] <GENESIS_FILE>`

###### **Arguments:**

* `<GENESIS_FILE>` — Path to genesis json file

###### **Options:**

* `-o`, `--out-file <PATH>` — Path to signed genesis output file in Norito format (stdout by default)
* `-t`, `--topology <TOPOLOGY>` — Use this topology instead of specified in genesis.json. JSON-serialized vector of `PeerId`. For use in `iroha_swarm`
* `--peer-pop <PEER_POPS>` — Embed one or more PoPs into the same transaction as `--topology`. Repeatable flag: `--peer-pop <public_key=pop_hex>`
* `--private-key <HEX>` — Private key hex (multihash payload, not prefixed) that matches the genesis public key
* `--seed <SEED>` — Seed string to derive the genesis key (testing convenience)
* `--algorithm <ALGORITHM>` — Algorithm of the genesis key (must match the genesis public key)

  Default value: `ed25519`
* `--config <PATH>` — Optional peer config TOML used to derive the DA proof-policy bundle embedded into genesis
* `--consensus-mode <MODE>` — Select the consensus mode to stamp into the manifest (optional override)

  Possible values: `permissioned`, `npos`

* `--next-consensus-mode <MODE>` — Optional future consensus mode to stage behind `--mode-activation-height`

  Possible values: `permissioned`, `npos`

* `--mode-activation-height <HEIGHT>` — Optional: set the block height at which `next_mode` should activate (requires `--next-consensus-mode`)



## `kagami genesis generate`

Generate a genesis configuration and standard-output in JSON format

**Usage:** `kagami genesis generate [OPTIONS] --ivm-dir <PATH> --genesis-public-key <MULTI_HASH> [COMMAND]`

###### **Subcommands:**

* `default` — Generate default genesis
* `synthetic` — Generate synthetic genesis with the specified number of domains, accounts and assets

###### **Options:**

* `--profile <PROFILE>` — Optional profile: picks Iroha3 defaults for dev/taira/nexus (sets chain id, DA/RBC, collector knobs)

  Possible values:
  - `iroha3-dev`:
    Local-only developer network
  - `iroha3-taira`:
    Public Sora test network
  - `iroha3-nexus`:
    Sora Nexus main network

* `--chain-id <CHAIN_ID>` — Optional explicit chain id (overrides profile default)
* `--vrf-seed-hex <HEX>` — Optional VRF seed (hex, 32 bytes). Required for `iroha3-taira`/`iroha3-nexus` when NPoS is selected; ignored for permissioned manifests
* `--executor <PATH>` — Optional path (relative to output) to the executor bytecode file (.to). If omitted, no executor upgrade is included in genesis
* `--ivm-dir <PATH>` — Relative path from the directory of output file to the directory that contains IVM bytecode libraries
* `--genesis-public-key <MULTI_HASH>`
* `--ivm-gas-limit-per-block <U64>` — Optional: set the custom parameter `ivm_gas_limit_per_block` (u64) in genesis so all peers agree on the block gas budget. If omitted, a sensible default (1,680,000) is applied
* `--consensus-mode <MODE>` — Select the consensus mode snapshot to seed in the genesis parameters (public dataspace requires NPoS; other Iroha3 dataspaces may use permissioned or NPoS; Iroha2 defaults to permissioned)

  Possible values: `permissioned`, `npos`

* `--next-consensus-mode <MODE>` — Optional future consensus mode to stage behind `--mode-activation-height` (Iroha2 only; Iroha3 disallows staged cutovers)

  Possible values: `permissioned`, `npos`

* `--mode-activation-height <HEIGHT>` — Optional: set the block height at which `next_mode` should activate (requires `--next-consensus-mode`)
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



## `kagami genesis pop`

Produce a BLS PoP (Proof-of-Possession) for a consensus key (BLS-normal)

**Usage:** `kagami genesis pop [OPTIONS]`

###### **Options:**

* `--algorithm <ALGORITHM>` — Algorithm to use; must be `bls_normal` for consensus PoP

  Default value: `bls_normal`
* `--private-key <PRIVATE_KEY>` — Private key hex (multihash payload, not prefixed)
* `--seed <SEED>` — Seed string to derive the key pair (for testing)
* `--json` — Output JSON instead of plain text
* `--expose-private-key` — Print the private key in plain-text output (disabled by default)



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
* `--domain <DOMAIN>` — Fully qualified account domain for generated client configs

  Default value: `acme.universal`
* `--seed-prefix <SEED>` — Seed prefix for deterministic key generation (`<prefix>-<name>`)

  Default value: `demo`
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
