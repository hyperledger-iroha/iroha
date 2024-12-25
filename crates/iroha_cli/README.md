# Iroha CLI

Iroha cli is a multi-purpose tool for interactions with iroha components.

# Table of Contents
1. [Installation](#installation)
2. [Usage](#usage)
3. [Examples](#examples)

## Installation

**Requirements:** a working [Rust toolchain](https://www.rust-lang.org/learn/get-started) (version 1.62.1), installed and configured.

Build Iroha and its binaries:

```bash
cargo build
```

The above command will produce the `iroha` ELF executable file for Linux/BSD, the `iroha` executable for MacOS, and the `iroha.exe` executable for Windows, depending on your platform and configuration.

<!-- TODO: Update documentation -->
Alternatively, check out the [documentation](https://docs.iroha.tech/get-started/install-iroha-2.html) (**TBU**) for system-wide installation instructions.

## Usage

Run Iroha CLI:

```
iroha <SUBCOMMANDS> [OPTIONS]
```

### Subcommands
| Command | Description                                                                                                                                         |
|---------|-----------------------------------------------------------------------------------------------------------------------------------------------------|
| codec   | Execute commands related to [Parity Scale Codec](https://github.com/paritytech/parity-scale-codec): list available types, decode SCALE to iroha types, decode SCALE to json, encode SCALE from json. |
| wasm    | Execute commands related to smartcontracts: build and check source files, run wasm tests.                                                           |
| client  | Execute commands relatedto interactions with iroha peers Web API.                              |

<details> <summary>Codec subcommands</summary>

| Command                                             | Description                                                                                                                        |
|-----------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------|
| `list-types`                       | List all available data types.                                                                                                      |
| `scale-to-json` | Decode the data type from SCALE to JSON.                                                                                            |
| `json-to-scale` | Encode the data type from JSON to SCALE.                                                                                            |
| `scale-to-rust`                  | Decode the data type from SCALE binary file to Rust debug format.<br>Can be used to analyze binary input if data type is not known. |
| `help`                                              | Print the help message for the tool or a subcommand.                                                                                |
</details>

<details> <summary>Wasm subcommands</summary>

| Command                                             | Description                                                                                                                        |
|-----------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------|
| `check`                       | Check if smartcontract sources are valid (`cargo check`).                                                                                                      |
| `build` | Build smartcontracs from given sources (`cargo build`).                                                                                            |
| `test` | Run WebAssembly tests.                                                                                            |
| `help`                                              | Print the help message for the tool or a subcommand.                                                                                |
</details>
</details>

<details> <summary>Client subcommands</summary>

`client` commands require a valid configuration file, refer to `defaults/client.toml` as example.

|  Command  |                                                                 Description                                                                 |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `account` | Execute commands related to accounts: register a new one, list all accounts, grant a permission to an account, list all account permissions. |
| `asset`   | Execute commands related to assets: register a new one, mint or transfer assets, get info about an asset, list all assets.                   |
| `blocks`  | Get block stream from Iroha peer.                                                                                                            |
| `domain`  | Execute commands related to domains: register a new one, list all domains.                                                                   |
| `events`  | Get event stream from Iroha peer.                                                                                                            |
| `json`    | Submit multi-instructions or request query as JSON.                                                                                                           |
| `peer`    | Execute commands related to peer administration and networking.                                                                              |
| `wasm`    | Execute commands related to WASM.                                                                                                            |
| `help`    | Print the help message for `iroha` and/or the current subcommand other than `help` subcommand.                                    |

Refer to [Iroha Special Instructions](https://docs.iroha.tech/blockchain/instructions.html) for more information about Iroha instructions such as register, mint, grant, and so on.
</details>

## Examples
:grey_exclamation: All examples below are Unix-oriented. If you're working on Windows, we would highly encourage you to consider using WSL, as most documentation assumes a POSIX-like shell running on your system. Please be advised that the differences in the syntax may go beyond executing `iroha.exe` instead of `iroha`.

- [Codec](examples/codec.md) tutorial and examples
- [Wasm](examples/wasm.md) basic usage examples
- [Client](examples/client.md) basic usage examples