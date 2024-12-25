# Examples for `iroha codec`

In this section we will show you how to use Iroha CLI Codec to do the following:
  - [List availible types](#list-availible-types)
  - [Decode SCALE ⇔ JSON](#decode-scale-⇔-json)
  - [Decode SCALE to supported type](#decode-scale-to-supported-type)

## List availible types

To list all supported data types, run from the project main directory:

```bash
./iroha codec list-types
```

<details> <summary> Expand to see expected output</summary>

```
Account
AccountEvent
AccountEventFilter
AccountEventSet
AccountId
AccountMintBox
AccountPermissionChanged
AccountRoleChanged
Action
Algorithm
...

344 types are supported
```

</details>


## Decode SCALE ⇔ JSON

Commands: `scale-to-json` and `json-to-scale`

Both commands by default read data from `stdin` and print result to `stdout`.
There are flags `--input` and `--output` which can be used to read/write from files instead.

These commands require `--type` argument. If data type is not known, [`scale-to-rust`](#scale-to-rust) can be used to detect it.

* Decode the specified data type from a binary:

  ```bash
  ./iroha codec scale-to-json --input <path_to_binary> --type <type>
  ```

### `scale-to-json` and `json-to-scale` usage examples

* Decode the `NewAccount` data type from the `samples/account.bin` binary:

  ```bash
  ./iroha codec scale-to-json --input iroha_codec/samples/account.bin --type NewAccount
  ```

* Encode the `NewAccount` data type from the `samples/account.json`:

  ```bash
  ./iroha codec json-to-scale --input iroha_codec/samples/account.json --output result.bin --type NewAccount
  ```


## Decode SCALE to supported type
Command: `scale-to-rust`

Decode the data type from a given binary.

|   Option   |                                                          Description                                                          |          Type          |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------- | ---------------------- |
| `--binary` | The path to the binary file with an encoded Iroha structure for the tool to decode.                                           | An owned, mutable path |
| `--type`   | The data type that is expected to be encoded in the provided binary.<br />If not specified, the tool tries to guess the type. | String                 |

* Decode the specified data type from a binary:

  ```bash
  ./iroha codec scale-to-rust <path_to_binary> --type <type>
  ```

* If you are not sure which data type is encoded in the binary, run the tool without the `--type` option:

  ```bash
    ./iroha codec scale-to-rust <path_to_binary>
  ```

### `scale-to-rust` usage examples

* Decode the `NewAccount` data type from the `samples/account.bin` binary:

  ```bash
  ./iroha codec scale-to-rust iroha_codec/samples/account.bin --type NewAccount
  ```

* Decode the `NewDomain` data type from the `samples/domain.bin` binary:

  ```bash
  ./iroha codec scale-to-rust iroha_codec/samples/domain.bin --type NewDomain
  ```
