# IVM baseline v1 fixtures

Frozen evidence of the IVM that the installed baseline selection (`56c23286…`) executes: the
pre-C10 anchor commit `77600a4840` (`c5511ebfc4~1`). Decision and phases:
`specs/runtime/ivm_baseline_and_c10_profiles.md`. Nothing here is generated from, or checked
against, today's interpreter; the restored baseline engine must reproduce it.

Do not edit these files by hand. Regenerate with `scripts/ivm_baseline_v1/capture.py` (it exports
the anchor with `git archive`, builds a capture program inside that export and rewrites this
directory); after editing only this README, run it with `--manifest-only`.

## Layout

| Path | Content |
|---|---|
| `manifest.json` | Anchor commit, toolchain, frozen identities and the SHA-256 of every other file |
| `descriptors/abi_v1.descriptor.bin` | ABI surface descriptor; `Hash::new` of it is the ABI hash `7531b45d…` |
| `descriptors/gas_schedule_v1.descriptor.bin` | Gas schedule descriptor; `Hash::new` of it is `af5a27f5…` |
| `descriptors/host_profile_abi_v1.descriptor.bin` | Canonical Norito bytes of the HostProfile `abi.v1` definition (`d50c38a8…`) |
| `descriptors/runtime_baseline_catalog.json` | The anchor's `specs/runtime_baseline_catalog.json` |
| `descriptors/frozen_values.json` | Hashes, the definition-id preimage, header and memory layout, opcode table with gas costs, syscall and pointer-type lists, wire tags of the frozen payload enums |
| `descriptors/core_admission_anchor.rs.txt` | The anchor's Core admission functions, verbatim; the oracle's `admission.core` applies them |
| `artifacts/asm/*.to.bin` | Hand-assembled programs (anchor `ivm::encoding` helpers) |
| `artifacts/kotodama/**/*.to.bin` | Kotodama programs compiled by the anchor compiler |
| `sources/kotodama/**/*.ko.txt` | Their sources, including the ones the anchor compiler refuses |
| `oracle/oracle.jsonl` | One Norito JSON record per case: setup, admission, final state |
| `oracle/corpus.json` | Case counts, coverage and the source index |

Artifacts and sources carry `.bin`/`.txt` suffixes because repository gates format, compile and
inventory every tracked `*.ko` and `*.to` file with the current toolchain.

`Hash::new` is `iroha_crypto::Hash::new`: BLAKE2b-256 with the least significant bit of the last
byte set. The HostProfile definition id is
`Hash::new(b"iroha:runtime:definition:v1\0" ‖ [1, kind tag 6] ‖ le64(len) ‖ descriptor)`, and the
descriptor is `DefinitionDescriptorV1::HostProfile { abi_version: 1, abi_hash, metering }` with
`metering` the gas schedule hash.

## Oracle records

Each line of `oracle.jsonl` has:

- `id`, `artifact`, `artifact_sha256`, `covers`.
- `setup`: `gas_limit` (and whether it came from the header through the anchor Core's
  `gas_limit_for_cycles`), `host`, `public_inputs`, `prelude`, `steps`, the Core `policy`, an
  optional shared `cycle_budget`.
- `admission.vm`: `IVM::validate_program`. `admission.core`: the anchor Core's verdict, either
  the effective cycle limit or the exact `IvmAdmissionError` (wire tag, bare Norito bytes,
  JSON). The state-dependent ZK backend check is not modelled.
- `run`: `not_run` with the loader error, or the result (`VMError`, `IvmFaultKindV1` with its
  wire tag, the `IvmFaultV1` the anchor projects), `pc`, `cycles`, `gas_remaining`, non-zero
  registers, private register tags, register root, output bytes, the full `ExecutionSummary`
  (register and memory roots, gas, cycles), the return record, host state or contract events,
  prelude results, the cycle-budget account, and the syscalls the interpreter dispatched (quote,
  arguments, pointer payloads, results, gas before and after). `syscall_trace` counts and hashes
  all syscall events; `syscalls` keeps the first 16. Byte strings are `{len, hex}` up to 96
  bytes and `{len, sha256}` above.
- `trace`: fetched-instruction count, SHA-256 of the little-endian PC sequence and opcode
  histogram from a further run with `TraceMode::PcOnly`; `null` with a `trace_note` for a
  program that reads its own trace logs (`SYSCALL_EXECUTION_SUMMARY`).

To replay a case: build the VM with `IVM::deterministic_builder(gas_limit)
.suppress_startup_banner().build()`; `load_program(artifact)` (for `wsv_contract`:
`install_contract_fixture`, `load_prepared`, `bind_contract_runtime_context`); for each prelude
entrypoint `select_entrypoint`, run, `reset`, `set_gas_limit`; apply `steps` in order; run with
the host (`run_with_host`, or `run_with_host_and_cycle_budget` with one `VmCycleBudget` shared
by the prelude and the run when `cycle_budget` is set). Hosts are the anchor's
`DefaultHost::new()`, `CoreHost::new()` and
`WsvHost::new_with_subject(MockWorldStateView::new(), account)`, where `account` is the key
`ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774`; their behaviour is part
of the oracle.

Every case ran three times on the anchor (twice plain, once traced) with identical results; the
one case that reads its own trace logs ran twice. A second capture process reproduced every
generated file byte for byte.

## Coverage

304 artifacts and 499 cases. All 90 valid baseline opcodes execute in at least one case,
including the families C10 removed or re-encoded: vector and `SETVL`, parallel markers,
direct-crypto and ZK-helper opcodes, 128-bit memory operations, registers above 31, 8-bit
immediates and branch offsets, 16-bit `JAL`, 8-bit `SCALL`. `oracle/corpus.json` lists the
executed-opcode counts, fault kinds, admission refusals, dispatched syscalls and the Kotodama
source index. Not covered: nested contract calls, Core-state admission errors, proof-trace logs.
