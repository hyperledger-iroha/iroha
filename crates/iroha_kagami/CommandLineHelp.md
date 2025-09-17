# Command-Line Help for `kagami`

This document contains the help content for the `kagami` command-line program.

**Command Overview:**

* [`kagami`↴](#kagami)
* [`kagami crypto`↴](#kagami-crypto)
* [`kagami schema`↴](#kagami-schema)
* [`kagami genesis`↴](#kagami-genesis)
* [`kagami genesis default`↴](#kagami-genesis-default)
* [`kagami genesis synthetic`↴](#kagami-genesis-synthetic)
* [`kagami codec`↴](#kagami-codec)
* [`kagami codec list-types`↴](#kagami-codec-list-types)
* [`kagami codec scale-to-rust`↴](#kagami-codec-scale-to-rust)
* [`kagami codec scale-to-json`↴](#kagami-codec-scale-to-json)
* [`kagami codec json-to-scale`↴](#kagami-codec-json-to-scale)
* [`kagami kura`↴](#kagami-kura)
* [`kagami kura print`↴](#kagami-kura-print)
* [`kagami swarm`↴](#kagami-swarm)
* [`kagami wasm`↴](#kagami-wasm)
* [`kagami wasm check`↴](#kagami-wasm-check)
* [`kagami wasm build`↴](#kagami-wasm-build)
* [`kagami markdown-help`↴](#kagami-markdown-help)

## `kagami`

Kagami is a tool used to generate and validate automatically generated data files that are shipped with Iroha

**Usage:** `kagami <COMMAND>`

###### **Subcommands:**

* `crypto` — Generate cryptographic key pairs using the given algorithm and either private key or seed
* `schema` — Generate the schema used for code generation in Iroha SDKs
* `genesis` — Commands related to genesis
* `codec` — Commands related to codec
* `kura` — Commands related to block inspection
* `swarm` — Commands related to Docker Compose configuration generation
* `wasm` — Commands related to building wasm smartcontracts
* `markdown-help` — Output CLI documentation in Markdown format



## `kagami crypto`

Generate cryptographic key pairs using the given algorithm and either private key or seed

**Usage:** `kagami crypto [OPTIONS]`

###### **Options:**

* `-a`, `--algorithm <ALGORITHM>` — An algorithm to use for the key-pair generation

  Default value: `ed25519`

  Possible values: `ed25519`, `secp256k1`, `bls_normal`, `bls_small`

* `-p`, `--private-key <PRIVATE_KEY>` — A private key to generate the key-pair from

   `--private-key` specifies the payload of the private key, while `--algorithm` specifies its algorithm.
* `-s`, `--seed <SEED>` — The Unicode `seed` string to generate the key-pair from
* `-j`, `--json` — Output the key-pair in JSON format
* `-c`, `--compact` — Output the key-pair without additional text



## `kagami schema`

Generate the schema used for code generation in Iroha SDKs

**Usage:** `kagami schema`



## `kagami genesis`

Commands related to genesis

**Usage:** `kagami genesis --creation-time <CREATION_TIME> --executor <PATH> --wasm-dir <PATH> [COMMAND]`

###### **Subcommands:**

* `default` — Generate default genesis
* `synthetic` — Generate synthetic genesis with the specified number of domains, accounts and assets

###### **Options:**

* `--creation-time <CREATION_TIME>` — Creation time of the genesis block in RFC 3339 format (e.g. "2018-02-16T00:31:37Z")
* `--executor <PATH>` — Relative path from the directory of output file to the executor.wasm file
* `--wasm-dir <PATH>` — Relative path from the directory of output file to the directory that contains *.wasm libraries



## `kagami genesis default`

Generate default genesis

**Usage:** `kagami genesis default`



## `kagami genesis synthetic`

Generate synthetic genesis with the specified number of domains, accounts and assets.

Synthetic mode is useful when we need a semi-realistic genesis for stress-testing Iroha's startup times as well as being able to just start an Iroha network and have instructions that represent a typical blockchain after migration.

**Usage:** `kagami genesis synthetic [OPTIONS]`

###### **Options:**

* `--domains <DOMAINS>` — Number of domains in synthetic genesis

  Default value: `0`
* `--accounts-per-domain <ACCOUNTS_PER_DOMAIN>` — Number of accounts per domains in synthetic genesis. The total number of accounts would be `domains * assets_per_domain`

  Default value: `0`
* `--assets-per-domain <ASSETS_PER_DOMAIN>` — Number of assets per domains in synthetic genesis. The total number of assets would be `domains * assets_per_domain`

  Default value: `0`



## `kagami codec`

Commands related to codec

**Usage:** `kagami codec <COMMAND>`

###### **Subcommands:**

* `list-types` — Show all available types
* `scale-to-rust` — Decode SCALE to Rust debug format from binary file
* `scale-to-json` — Decode SCALE to JSON. By default uses stdin and stdout
* `json-to-scale` — Encode JSON as SCALE. By default uses stdin and stdout



## `kagami codec list-types`

Show all available types

**Usage:** `kagami codec list-types`



## `kagami codec scale-to-rust`

Decode SCALE to Rust debug format from binary file

**Usage:** `kagami codec scale-to-rust [OPTIONS] <BINARY>`

###### **Arguments:**

* `<BINARY>` — Path to the binary with encoded Iroha structure

###### **Options:**

* `-t`, `--type <TYPE_NAME>` — Type that is expected to be encoded in binary. If not specified then a guess will be attempted



## `kagami codec scale-to-json`

Decode SCALE to JSON. By default uses stdin and stdout

**Usage:** `kagami codec scale-to-json [OPTIONS] --type <TYPE_NAME>`

###### **Options:**

* `-i`, `--input <INPUT>` — Path to the input file
* `-o`, `--output <OUTPUT>` — Path to the output file
* `-t`, `--type <TYPE_NAME>` — Type that is expected to be encoded in input



## `kagami codec json-to-scale`

Encode JSON as SCALE. By default uses stdin and stdout

**Usage:** `kagami codec json-to-scale [OPTIONS] --type <TYPE_NAME>`

###### **Options:**

* `-i`, `--input <INPUT>` — Path to the input file
* `-o`, `--output <OUTPUT>` — Path to the output file
* `-t`, `--type <TYPE_NAME>` — Type that is expected to be encoded in input



## `kagami kura`

Commands related to block inspection

**Usage:** `kagami kura [OPTIONS] <PATH_TO_BLOCK_STORE> <COMMAND>`

###### **Subcommands:**

* `print` — Print contents of a certain length of the blocks

###### **Arguments:**

* `<PATH_TO_BLOCK_STORE>`

###### **Options:**

* `-f`, `--from <BLOCK_HEIGHT>` — Height of the block from which start the inspection. Defaults to the latest block height



## `kagami kura print`

Print contents of a certain length of the blocks

**Usage:** `kagami kura print [OPTIONS]`

###### **Options:**

* `-n`, `--length <LENGTH>` — Number of the blocks to print. The excess will be truncated

  Default value: `1`



## `kagami swarm`

Commands related to Docker Compose configuration generation

**Usage:** `kagami swarm [OPTIONS] --peers <COUNT> --config-dir <DIR> --image <NAME> --out-file <FILE>`

###### **Options:**

* `-p`, `--peers <COUNT>` — Number of peer services in the configuration
* `-s`, `--seed <SEED>` — UTF-8 seed for deterministic key-generation
* `-H`, `--healthcheck` — Includes a healthcheck for every service in the configuration.

   Healthchecks use predefined settings.

   For more details on healthcheck configuration in Docker Compose files, see: <https://docs.docker.com/compose/compose-file/compose-file-v3/#healthcheck>
* `-c`, `--config-dir <DIR>` — Directory with Iroha configuration. It will be mapped to a volume for each container.

   The directory should contain `genesis.json` and the executor.
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



## `kagami wasm`

Commands related to building wasm smartcontracts

**Usage:** `kagami wasm <COMMAND>`

###### **Subcommands:**

* `check` — Apply `cargo check` to the smartcontract
* `build` — Build the smartcontract



## `kagami wasm check`

Apply `cargo check` to the smartcontract

**Usage:** `kagami wasm check [OPTIONS] <PATH>`

###### **Arguments:**

* `<PATH>` — Path to the smartcontract

###### **Options:**

* `--cargo-args <CARGO_ARGS>` — Extra arguments to pass to `cargo`, e.g. `--locked`

  Default value: ``
* `--profile <PROFILE>`

  Default value: `release`



## `kagami wasm build`

Build the smartcontract

**Usage:** `kagami wasm build [OPTIONS] --out-file <OUT_FILE> <PATH>`

###### **Arguments:**

* `<PATH>` — Path to the smartcontract

###### **Options:**

* `--cargo-args <CARGO_ARGS>` — Extra arguments to pass to `cargo`, e.g. `--locked`

  Default value: ``
* `--profile <PROFILE>` — Build profile

  Default value: `release`
* `--out-file <OUT_FILE>` — Where to store the output WASM. If the file exists, it will be overwritten



## `kagami markdown-help`

Output CLI documentation in Markdown format

**Usage:** `kagami markdown-help`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
