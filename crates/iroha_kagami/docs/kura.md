# Kura Inspector

With Kura Inspector you can inspect blocks in disk storage regardless of the operating status of Iroha and print out block contents in a human-readabe format.

## Usage

Run Kura Inspector:

```bash
kagami kura [OPTIONS] <PATH_TO_BLOCK_STORE> <SUBCOMMAND>
```

### Options

|     Option     |                      Description                      |    Default value     |       Type       |
| -------------- | ----------------------------------------------------- | -------------------- | ---------------- |
| `-f`, `--from` | The starting block height of the range for inspection | Current block height | Positive integer |

### Subcommands

|      Command      |                     Description                     |
| ----------------- | --------------------------------------------------- |
| [`print`](#print) | Print the contents of a specified number of blocks  |
| `help`            | Print the help message for the tool or a subcommand |

### Errors

An error in Kura Inspector occurs if one the following happens:

- `kura` fails to configure `kura::BlockStore`
- `kura` [fails](#print-errors) to run the `print` subcommand

## `print`

The `print` command reads data from the `block_store` and prints the results to the specified `output`.

|      Option      |                                      Description                                      | Default value |       Type       |
| ---------------- | ------------------------------------------------------------------------------------- | ------------- | ---------------- |
| `-n`, `--length` | The number of blocks to print. The excess is truncated.                               | 1             | Positive integer |
| `-o`, `--output` | Where to write the results of the inspection: valid data and [errors](#print-errors). | `/dev/stdout` | file             |

### `print` errors

An error in `print` occurs if one the following happens:
- `kura` fails to read `block_store`
- `kura` fails to print the `output`
- `kura` tries to print the latest block and there is none

## Examples

- Print the contents of the latest block:

  ```bash
  kagami kura <path> print
  ```

- Print all blocks with a height between 100 and 104:

  ```bash
  kagami kura -f 100 <path> print -n 5
  ```

- Print errors for all blocks with a height between 100 and 104:

  ```bash
  kagami kura -f 100 <path> print -n 5 >/dev/null
  ```