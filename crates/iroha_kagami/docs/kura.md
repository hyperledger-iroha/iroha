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

## Canonical scaling evidence

`kagami advanced kura scaling-evidence stopped-tip` observes the committed
height of one stopped store under the parent's independently pinned original
signed genesis and network identity. The explicit reader interval must be
`--first-height 1 --last-height 1`; finite limits still cover the complete
marker, index, hash/data lengths and merge-log census. The genesis carrier must
equal the retained original bytes. This observation authenticates no later
carrier QC or execution result; the collector and facts command must still
verify the complete interval from one through the observed committed height.

The original genesis lease and Core completion remain retained across the
actual stdout write, flush and final identity check. The six-scalar reply binds
`version`, `operation`, `invocation_id`, `genesis_sha256`, `genesis_bytes` and
`committed_height`. Require terminal success. This command creates no output
artifact. The launcher owns child shutdown and continuing custody of every
stopped-store file; it must retain a surviving peer for finality/query collection.

`kagami advanced kura scaling-evidence facts` authenticates the original final
manifest, signed genesis, four peer configurations, genesis context, complete
collector journal, and canonical finality/query vectors. Supply each absolute
path with an independently retained raw SHA-256 and byte reservation. The
parent also supplies the selected chain, genesis identity and signer, validator
order, account order, one/four-lane variant, public workload seed, exact schedule
and resource-clock geometry. None of these expectations comes from query rows
or the collector's summary.

The command requires four NPoS validators, registered universal workload
accounts, fixed account routes and disabled automatic lane
lifecycle. Peer configuration parsing must be self-contained; external hash or
key files, onboarding/faucet services, streaming codec overrides, manifest
dependencies and policy directories are rejected. The signed genesis and all
four effective configurations establish the actual context and lane authority.
The complete original journal is joined to full canonical execution verification
from genesis through the independently selected stopped-store tip. A committed
rejection or an omitted request fails the run.

Facts admission does not project account balances or permissions. The fixed
generator funds its workload accounts in genesis; the retained launcher still
owns the selected generator/configuration join and equality of operational
queue, storage and resource settings across the benchmark peers.

Only successful authentication can create a new canonical `PrepareFactsV1`
file. The publisher retains all originals, the completed Kura reader and the
facts descriptor through fsync, NOREPLACE publication, reply write and flush,
and the final identity check. The bounded reply contains exactly `version`,
`operation`, `invocation_id`, `facts_sha256` and `facts_bytes`. Require terminal
exit zero and an independent output census before using the file. Failed
publication or reply leaves surviving artifacts in place and returns failure.
Source/facts reservations and cumulative decode limits are explicit; they do
not by themselves establish a process memory or resource-measurement result.

`kagami advanced kura scaling-evidence prepare` consumes one independently pinned
canonical `PrepareFactsV1` file and publishes the canonical launcher request and
supplied evidence bundle. It validates the anchored finality chain, complete
ordered query bindings and signed workload schedule under explicit byte/work
bounds. Full canonical merge execution remains the export/replay owner's check.
The facts reservation plus both output reservations must fit the aggregate cap.
Both output files are staged and fsynced before either destination is published;
publication is two NOREPLACE operations. A failed second publication leaves the
first artifact intact and returns failure. Existing artifacts must be inspected
as evidence before choosing fresh destinations.

The preparation reply contains only `version`, `operation`, `invocation_id`,
and the raw SHA-256 and exact byte length of `facts`, `request`, and `bundle`.
Its complete byte bound includes the final newline. The original facts and both
output descriptors remain retained through reply flush and the final identity
check. This reply reports transport identities and provides no proof hash.

`kagami advanced kura scaling-evidence export` authenticates a retained launcher
request, exact supplied finality/query bundle, and immutable canonical Kura
interval. It publishes a new V1 Norito proof through the retained output owner.
`scaling-evidence replay` independently authenticates that proof under a retained
launcher request and emits the complete ordered row projection.

Export and replay require absolute input paths, separately supplied raw SHA-256 pins,
finite byte reservations, and an invocation identity. The complete reply cap
reserves framing first and bounds row projection allocation before materialization. Export additionally requires
all Core reader bounds and the expected Unix owner. Replay requires the separate
marked Iroha proof hash; raw SHA-256 is never converted into an Iroha hash.
Existing output destinations, symlinked inputs, altered retained files, partial
proofs, and noncanonical layouts fail. Refer to each command's `--help` for the
required fields.

Each export/replay stdout reply is one version-1 JSON object ending in a newline. It binds the
operation and invocation to request/input raw digests and the proof's independently
calculated raw SHA-256, marked Iroha hash, and exact byte length. Replay also emits
`rows`, the existing canonical ordered projection. The parent must require a clean
terminal exit as well as a complete bounded reply: original input handles remain
live and are rechecked after the output flush. Published output bytes must still
be independently censused and replayed by the parent.

The parent selects the independent launch expectations. These commands do not
qualify a scaling run by themselves. Pinned executable and runtime dependency
custody, child readiness, shutdown provenance, full trace/resource joins and
paired-run release checks remain required.

### Fixed scaling deployment inputs

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
pin this original context with the signed genesis and configurations before
collecting canonical scaling inputs; the generated artifact does not establish
runtime or process custody.

The actual CLI/signed-genesis test in `localnet/scaling/tests.rs` authenticates
the final genesis and checks every lane's committee and funded account. A
generated deployment does not establish process identity, readiness, graceful
shutdown, original Kura capture or a scaling result. The retained launcher and
complete facts/export/replay pipeline still own those checks.

The fixed localnet generator emits `genesis-anchors.json` and identical bounded JSON stdout. The receipt uses native typed genesis, context and network IDs and public original peer identities; it lists every genesis, peer and workload client input by raw SHA-256 and length. The retained caller pins this original receipt before starting any peer. `facts` consumes the canonical collector journal, finality proofs and committed-transaction queries under those original pins; preparation remains native. Runtime dependency admission, same-peer readiness and the complete four-peer lifecycle require the fixed runner.

Fixed scaling output has one mutable `storage/` tree with exactly four original role directories and initially empty `kura/` and `state/` children. Every resolved writable config path is bound to the role before final genesis authority is frozen. Snapshot and discovery replay paths are explicit; PoR VRF/drand files follow the canonical config-owned derivation from the role PoR state directory. The producer retains this initial namespace and seals the exact census through receipt flush. The caller independently owns mutable runtime verification after generation.

Fixed scaling anchor peer rows also require `primary_block_store` and `primary_merge_log`.
Kagami derives both bounded absolute paths with the final effective primary lane's native
`blocks_dir` and `merge_log_path` helpers, under that peer's original Kura root. They are
absent at generation and created by the daemon. Stopped-tip, canonical vector collection,
facts and proof export consume these retained native projections; the Kura runtime root
itself is not the canonical block-journal directory.
The same receipt requires typed public `genesis_public_key` and effective u16
`chain_discriminant`, derived and matched across all four final authenticated configs.
Callers retain these values directly without inferring omitted TOML defaults.
