# Iroha Daemon (irohad)

The `irohad` crate contains the Iroha server (peer) binary. The binary is used to instantiate a peer and bootstrap an Iroha-based network. The capabilities of the network are determined by the feature flags used to compile the binary.

Pass the `--language <code>` flag to override automatic language detection for informational and error messages.

## Build

**Requirements:** a working [Rust toolchain](https://www.rust-lang.org/learn/get-started) (version 1.62.1), installed and configured.

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

**Note:** this section is under development. You can track it in the [issue](https://github.com/hyperledger-iroha/iroha-2-docs/issues/392).

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
    relative paths from the directory that contains `config.toml`.

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

      See `crates/iroha_kagami/CommandLineHelp.md` and `docs/genesis*.md` for
      additional subcommands such as `validate` and `embed-pop`.

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

We provide a sample configuration for Docker in [`docker-compose.yml`](../../defaults/docker-compose.yml). We highly recommend that you adjust the `config.json` to include a set of new key pairs.

[Generate the keys](#generating-keys) and put them into `services.*.environment` in `docker-compose.yml`. Update `TRUSTED_PEERS` **and** provide matching `TRUSTED_PEERS_POP` entries (PoPs for every validator key, including the local one).

- Build images:

    ```bash
    docker-compose build
    ```

- Run containers:

    ```bash
    docker-compose up
    ```

  To keep containers up and running after closing the terminal, use the `-d` (*detached*) flag:

    ```bash
    docker-compose up -d
    ```

- Stop containers:

    ```bash
    docker-compose stop
    ```

- Remove containers:

    ```bash
    docker-compose down
    ```
