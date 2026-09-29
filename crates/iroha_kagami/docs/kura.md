# Kura Inspector

With Kura Inspector you can inspect blocks in disk storage regardless of the operating status of Iroha and print block contents in a human-readable format. The path must name one exact lane directory containing `blocks.index`, `blocks.data`, and `blocks.hashes`; the inspector never guesses a lane from a store root.

## Usage

Run Kura Inspector:

```bash
kagami advanced kura <SUBCOMMAND> [OPTIONS]
```

### Print options

|     Option     |                      Description                      |    Default value     |       Type       |
| -------------- | ----------------------------------------------------- | -------------------- | ---------------- |
| `-f`, `--from` | The starting block height of the range for inspection | Current block height | Positive integer |

### Subcommands

|      Command        |                             Description                              |
| ------------------- | --------------------------------------------------------------------- |
| [`print`](#print)   | Print the contents of a specified number of blocks                     |
| [`finality`](#finality) | Verify native certificates over a complete locally anchored prefix |
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

## `finality`

`kagami advanced kura finality <path> --chain-id <chain> --height <height>`
checks the signed genesis and every native certificate through the selected
height (1–4096), then emits the exact current `SumeragiFinalityProof`. Reads and
JSON output are bounded; verification failure emits no partial report. The
selected store supplies the genesis trust root, so `external_trust_anchor` is
false. A height-one report sets `genesis_execution_authenticated` to false:
only a verified successor authenticates the genesis execution result. Optional
`--output` uses atomic publication outside the store.

## `sidecar`

The `sidecar` command reads the pipeline recovery sidecar for a given block height and prints the raw JSON to the specified `output` (or to stdout if omitted).

|      Option       |                          Description                          |  Default value  |  Type  |
| ----------------- | -------------------------------------------------------------- | --------------- | ------ |
| `-H`, `--height`  | The block height whose sidecar to print                        | required        | number |
| `-o`, `--output`  | Where to atomically write the sidecar JSON; it must be outside the block store | stdout | file |

Notes:
- Sidecars use the canonical indexed pair `<lane_dir>/pipeline/sidecars.{norito,index}`.
- A sidecar is returned only when its embedded height and block hash match the canonical block journals.
- Pass the exact lane directory. File paths and multi-lane store roots are rejected instead of being normalized or searched.

### Examples

- Print the sidecar for height 7 to stdout:

  ```bash
  kagami advanced kura sidecar <path> --height 7
  ```

- Save the sidecar for height 42 to a file:

  ```bash
  kagami advanced kura sidecar <path> -H 42 -o sidecar_42.json
  ```

### `sidecar` errors

An error in `sidecar` occurs if one the following happens:
- `kura` fails to read `block_store`
- sidecar file is not found for the requested height
- `kura` fails to write the `output`

## Examples

- Print the contents of the latest block:

  ```bash
  kagami advanced kura print <path>
  ```

- Print all blocks with a height between 100 and 104:

  ```bash
  kagami advanced kura print <path> -f 100 -n 5
  ```

- Print errors for all blocks with a height between 100 and 104:

  ```bash
  kagami advanced kura print <path> -f 100 -n 5 >/dev/null
  ```

## Fixed scaling deployment inputs

`localnet --scaling-lanes 1|4` uses the existing NPoS genesis bootstrap with
exactly four validators. Public execution lanes share dataspace zero and its
four-member committee; automatic lane lifecycle is disabled in both variants.
The private generation seed is required and must match across a paired run.
It is separate from the public workload schedule seed. Named Sora/performance
profiles, extra accounts/assets and permissioned mode are rejected before the
output directory is created.

`--scaling-accounts` selects 4–64 accounts in complete groups of four (default
four). Bare universal accounts receive 100 XOR each and exact account routing
by `index % lane_count`; they receive no extra permissions. The generated fee
policy charges 0.0075 XOR per workload metadata update, covering the bounded
1,024 updates per account. Flat, owner-only `workload-account-00.toml` through
the final index retain explicit address discriminants and the same relative
`genesis.expected_hash` anchor. Pass these configs to `transaction load` in
index order with `--fee-payer authority`. Generation changes no queue, storage,
DA or consensus limits between the two layouts.

Fixed peer configs embed the exact final genesis hash and omit onboarding,
faucet, and streaming codec override tables. Config parsing therefore uses no
service-key, genesis-hash, or rANS side files; the codec uses compiled defaults.
Generated sidecars remain in the artifact inventory, and client configs still
use the public hash file. Runtime directories, codec resources, executable
identity, and process custody remain obligations of the retained launcher.
This boundary concerns peer config parsing, not runtime independence from files.

Fixed generation also publishes owner-only `genesis-context.nrt`, the canonical
height-one context from final signed-genesis staging against the final effective
peer configuration. The frame is bounded to one MiB. Retain and independently
pin this original context with the signed genesis and configurations; the generated artifact does not establish
runtime or process custody.

The actual CLI/signed-genesis test in `localnet/scaling/tests.rs` authenticates
the final genesis and checks every lane's committee and funded account. A
generated deployment does not establish process identity, readiness, graceful
shutdown, original Kura capture or a scaling result; the retained launcher owns
those checks.

The fixed localnet generator emits `genesis-anchors.json` and identical bounded JSON stdout. The receipt uses native typed genesis, context and network IDs and public original peer identities; it lists every genesis, peer and workload client input by raw SHA-256 and length. The retained caller pins this original receipt before starting any peer. Runtime dependency admission, same-peer readiness and the complete four-peer lifecycle require the fixed runner.

Fixed scaling output has one mutable `storage/` tree with exactly four original role directories and initially empty `kura/` and `state/` children. Every resolved writable config path is bound to the role before final genesis authority is frozen. Snapshot and discovery replay paths are explicit; PoR VRF/drand files follow the canonical config-owned derivation from the role PoR state directory. The producer retains this initial namespace and seals the exact census through receipt flush. The caller independently owns mutable runtime verification after generation.

Fixed scaling anchor peer rows require `primary_block_store`. Kagami derives this
bounded absolute path from the final effective primary lane under the original peer's
Kura root. It is absent at generation and created by the daemon, which writes its four native
journals there: block data, index, hashes and count. The Kura runtime root itself is not the canonical
block-journal directory.
The same receipt requires typed public `genesis_public_key` and effective u16
`chain_discriminant`, derived and matched across all four final authenticated configs.
Callers retain these values directly without inferring omitted TOML defaults.
