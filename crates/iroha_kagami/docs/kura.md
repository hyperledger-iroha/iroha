# Kura Inspector

With Kura Inspector you can inspect blocks in disk storage regardless of the operating status of Iroha and print block contents in a human-readable format. The path must name one exact lane directory containing `blocks.index`, `blocks.data`, and `blocks.hashes`; the inspector never guesses a lane from a store root.

## Usage

Run Kura Inspector:

```bash
kagami advanced kura [OPTIONS] <PATH_TO_BLOCK_STORE> <SUBCOMMAND>
```

### Options

|     Option     |                      Description                      |    Default value     |       Type       |
| -------------- | ----------------------------------------------------- | -------------------- | ---------------- |
| `-f`, `--from` | The starting block height of the range for inspection | Current block height | Positive integer |

### Subcommands

|      Command        |                             Description                              |
| ------------------- | --------------------------------------------------------------------- |
| [`print`](#print)   | Print the contents of a specified number of blocks                     |
| [`sidecar`](#sidecar) | Print the pipeline recovery sidecar JSON for a given block height       |
| `help`              | Print the help message for the tool or a subcommand                    |

### Errors

An error in Kura Inspector occurs if one the following happens:

- `kura` fails to configure `kura::BlockStore`
- `kura` [fails](#print-errors) to run the `print` subcommand

## `print`

The `print` command reads data from the `block_store` and prints the results to the specified `output`.

|      Option      |                                      Description                                      | Default value |       Type       |
| ---------------- | ------------------------------------------------------------------------------------- | ------------- | ---------------- |
| `-n`, `--length` | The number of blocks to print. The excess is truncated.                               | 1             | Positive integer |
| `-o`, `--output` | Where to atomically write the inspection result. The path must be outside the block store. | stdout | file |

### `print` errors

An error in `print` occurs if one the following happens:
- `kura` fails to read `block_store`
- `kura` fails to print the `output`
- `kura` tries to print the latest block and there is none

## `sidecar`

The `sidecar` command reads the pipeline recovery sidecar for a given block height and prints the raw JSON to the specified `output` (or to stdout if omitted).

|      Option       |                          Description                          |  Default value  |  Type  |
| ----------------- | -------------------------------------------------------------- | --------------- | ------ |
| `-h`, `--height`  | The block height whose sidecar to print                        | required        | number |
| `-o`, `--output`  | Where to atomically write the sidecar JSON; it must be outside the block store | stdout | file |

Notes:
- Sidecars use the canonical indexed pair `<lane_dir>/pipeline/sidecars.{norito,index}`.
- A sidecar is returned only when its embedded height and block hash match the canonical block journals.
- Pass the exact lane directory. File paths and multi-lane store roots are rejected instead of being normalized or searched.

### Examples

- Print the sidecar for height 7 to stdout:

  ```bash
  kagami advanced kura <path> sidecar --height 7
  ```

- Save the sidecar for height 42 to a file:

  ```bash
  kagami advanced kura <path> sidecar -h 42 -o sidecar_42.json
  ```

### `sidecar` errors

An error in `sidecar` occurs if one the following happens:
- `kura` fails to read `block_store`
- sidecar file is not found for the requested height
- `kura` fails to write the `output`

## Examples

- Print the contents of the latest block:

  ```bash
  kagami advanced kura <path> print
  ```

- Print all blocks with a height between 100 and 104:

  ```bash
  kagami advanced kura -f 100 <path> print -n 5
  ```

- Print errors for all blocks with a height between 100 and 104:

  ```bash
  kagami advanced kura -f 100 <path> print -n 5 >/dev/null
  ```
