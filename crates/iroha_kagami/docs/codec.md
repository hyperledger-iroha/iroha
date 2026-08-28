# Norito Decoder

These commands help you decode **Iroha 3** data types from binaries using the Norito codec.

> Run them via `cargo run -p iroha_kagami -- advanced codec <SUBCOMMAND>` or use a built `kagami` binary directly.

### Subcommands

| Command                                             | Description                                                                                                                        |
|-----------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------|
| [`list-types`](#list-types)                         | List all available data types                                                                                                      |
| [`norito-to-json`](#norito-to-json-and-json-to-norito) | Decode the data type from Norito to JSON                                                                                            |
| [`json-to-norito`](#norito-to-json-and-json-to-norito) | Encode the data type from JSON to Norito                                                                                            |
| [`norito-to-rust`](#norito-to-rust)                   | Decode the data type from Norito binary file to Rust debug format.<br>Can be used to analyze binary input if data type is not known |
| `help`                                              | Print the help message for the tool or a subcommand                                                                                |

## `list-types`

To list all supported data types, run from the project main directory:

```bash
kagami advanced codec list-types
```

The command prints the current, deliberately small first-release converter
inventory in deterministic order. Treat that output as authoritative; Kagami
does not expose every schema type as a generic codec surface.

## `norito-to-json` and `json-to-norito`

Both commands by default read data from `stdin` and print result to `stdout`.
There are flags `--input` and `--output` which can be used to read/write from files instead.
Input and output must refer to different files. File output is rendered and
validated first, then published atomically, so a failed conversion leaves an
existing destination unchanged.

Codec input and rendered output are limited to 64 MiB, matching the
first-release signed-genesis artifact corridor (the largest registered codec
type). JSON input is lexically preflighted and typed decoding uses fixed
element, allocation, and nesting budgets before any converter is entered.
Inputs above these limits are rejected; split unrelated records instead of
combining them into one codec invocation. Type guessing evaluates converters
in deterministic order and applies the same 64 MiB ceiling to all retained
matches together.

These commands require `--type` argument. If data type is not known, [`norito-to-rust`](#norito-to-rust) can be used to detect it.

* Decode the specified data type from a binary:

  ```bash
  kagami advanced codec norito-to-json --input <path_to_binary> --type <type>
  ```

### `norito-to-json` and `json-to-norito` usage examples

* Decode the `NewAccount` data type from the `samples/account.bin` binary:

  ```bash
  kagami advanced codec norito-to-json --input crates/iroha_kagami/samples/codec/account.bin --type NewAccount
  ```

* Encode the `NewAccount` data type from the `samples/account.json`:

  ```bash
  kagami advanced codec json-to-norito --input crates/iroha_kagami/samples/codec/account.json --output result.bin --type NewAccount
  ```

## `norito-to-rust`

Decode the data type from a given binary.

|   Option   |                                                          Description                                                          |          Type          |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------- | ---------------------- |
| `--binary` | The path to the binary file with an encoded Iroha structure for the tool to decode.                                           | An owned, mutable path |
| `--type`   | The data type that is expected to be encoded in the provided binary.<br />If not specified, the tool tries to guess the type. | String                 |

* Decode the specified data type from a binary:

  ```bash
  kagami advanced codec norito-to-rust <path_to_binary> --type <type>
  ```

* If you are not sure which data type is encoded in the binary, run the tool without the `--type` option:

  ```bash
    kagami advanced codec norito-to-rust <path_to_binary>
  ```

### `norito-to-rust` usage examples

* Decode the `NewAccount` data type from the `samples/account.bin` binary:

  ```bash
  kagami advanced codec norito-to-rust crates/iroha_kagami/samples/codec/account.bin --type NewAccount
  ```

* Decode the `Domain` data type from the `samples/domain.bin` binary:

  ```bash
  kagami advanced codec norito-to-rust crates/iroha_kagami/samples/codec/domain.bin --type Domain
  ```
