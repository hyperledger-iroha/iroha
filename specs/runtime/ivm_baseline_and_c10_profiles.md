# IVM baseline and C10 host profiles

Status: owner decision recorded 2026-11-16; implementation in progress (phase 1). This record
couples to `specs/live_runtime_upgrades.md` (immutable definitions, selection-based dispatch,
governed retirement) and `specs/runtime_baseline_catalog.json`.

## Decision

The IVM ABI consolidation ("C10", commit `c5511ebfc4` and its follow-ups) changed contracts that
Taira's installed baseline selection depends on. Installed definitions never change meaning, so
the release:

1. restores the installed baseline exactly: selection `56c23286…`, HostProfile `abi.v1`
   `d50c38a8…`, ABI hash `7531b45dd9d79aabc3a0f64ab30a053af847dfe248078cff839ab2e14e3261a7`,
   gas schedule `af5a27f5fc57ebfb9eae3c4f8a25091ce78d97ff7797dbc3b304a0091069731f`, the frozen
   instruction layouts (`RegisterSmartContractCode` `dc449482…`, `iroha.register` `6a79ea4e…`)
   and the pre-C10 header, ISA, admission, fault, privacy and gas semantics for every contract
   that executes under the baseline selection; and
2. publishes C10 as new definitions: a distinct named HostProfile (still `abi_version = 1`)
   with its own descriptor and gas schedule, new public payload/result types and registration
   instructions with new wire IDs wherever C10 changed a public payload, packaged as the release
   addition `iroha.runtime.ivm_c10.v1`, and selected only through a governed change set.

Every difference is dispatched by the authenticated selection/profile, resolved before header
parsing. No path tries two decoders. Historical replay uses the historical selected or captured
profile. Kotodama's default target stays the baseline (`abi-v1`); C10 is an explicit target.

The source anchor for the baseline is `77600a4840` (`c5511ebfc4~1`). Its interpreter, decoder,
preparation, register file, errors, gas and proof helpers are reconstructed as a frozen baseline
engine behind the one `IVM` facade. Optimizations of that engine are allowed only after
differential evidence shows identical bytes, results, faults, gas, events and commitments.

Activating C10 on an existing network still needs an owner-authorized approval step (the
governed-activation bootstrap decision); this record does not grant one.

## What belongs where

| Area | Baseline profile (restored) | C10 profile (new definitions) | Shared |
|---|---|---|---|
| Header | 49 bytes, vector metadata/mode | 48 bytes, ZK-only `max_cycles` | |
| ISA | 256 registers, 8-bit operands, old immediates/branches/`SCALL`, vector/parallel/direct-crypto/ZK-helper opcodes, 128-bit memory ops | 32 registers, 14-bit immediates, 19-bit `JAL`, 24-bit `SCALL`, reserved bits refused | syscall-number and pointer-type allowlists (0xF2/0xF3 stay refused) |
| Admission/faults | original precedence; `AssertionFailed` for arithmetic | structured numeric and control-flow faults | |
| Gas | original formulas, shared nested cycles, header-to-gas conversion | caller-owned gas only | |
| Wire types | `IvmAdmissionError`, `IvmFaultKindV1`, `NumericFaultV1` with their original tags and `VectorLengthTooLargeInfo`/`MaxCyclesExceedsFuelInfo`, and every enclosing type of the frozen closures | `*C10V1` types for changed members and their rejection/status/filter/manifest/trigger parents; `RegisterSmartContractCodeC10V1` (`iroha.smart_contract.code.c10.register.v1`), `RegisterTriggerC10V1` (`iroha.trigger.c10.register.v1`) and, if raw C10 execution is supported, `ExecuteIvmC10V1` (`iroha.ivm.c10.execute.v1`) | unchanged leaves |
| Node-local | | | allocation custody, preparation reuse, metrics, deterministic acceleration, lazy Merkle allocation, after differential checks |

Internal VM faults are separate from wire errors. The originating profile decides fault
semantics and projection; an internal union never becomes the baseline wire enum.

## Phases

Each phase ends compiling and passing its focused gate. Known baseline failures stay open (never
re-pinned) until phase 4 closes them.

| Phase | Deliverable and gate | Writes `iroha_data_model`? |
|---|---|---|
| 1 | Frozen baseline fixtures from `77600a4840`: descriptor/schedule bytes, catalog, compiled artifact corpus and an execution oracle (outputs, faults, gas, events, commitments); fixture-integrity tests | no |
| 2 | Profile context (`ResolvedIvmProfile`), IVM facade over profile engines, internal/wire fault separation, profile-bound cache identity; current behaviour preserved | no |
| 3 | Frozen baseline engine, admission and Kotodama baseline backend beside C10; the baseline differential corpus passes | no |
| 4 | Atomic baseline cutover: original public layouts and consumers, `AbiV1` bound everywhere, frozen catalog constants (`catalog.rs` no longer derives the baseline from mutable globals), restored goldens; `baseline_instruction_layouts_stay_frozen`, both `taira_activation` reconstruction tests and the ABI golden pass | yes |
| 5 | C10 public families, result/event/storage/service dispatch, registrations, the `iroha.runtime.ivm_c10.v1` addition and its draft route; activation/refusal/roundtrip tests | yes |
| 6 | Cross-profile, four-validator activation/restart, copied-store `--check-replay`, hardware parity and release-feature qualification | no |

## Open owner questions

- The activation-bootstrap authority for existing networks.
- Whether the first C10 profile permits mixed-profile nested calls, and their budgeting/error contract.
- Whether C10 and the runtime-lifecycle profile ship as one composition.
- Which networks and obligations must drain before the baseline engine is retired and deleted.
- Approval of the successor result/event contract and any kernel/commitment scope it implies.

## Phase 1 record (2026-11-16)

Fixtures: `crates/ivm/tests/fixtures/baseline_v1` (layout and record format in its README).
Regeneration: `scripts/ivm_baseline_v1/capture.py` exports `77600a4840` with `git archive`, adds
two `#[doc(hidden)]` descriptor accessors, builds the capture program as an `ivm` example inside
the export and rewrites the directory. Gate: `cargo test -p ivm --test ivm_group_01
baseline_v1_fixtures`. No production source changed.

| Frozen value at the anchor | Captured | Expected |
|---|---|---|
| ABI descriptor, 131 466 bytes, `Hash::new` | `7531b45dd9d79aabc3a0f64ab30a053af847dfe248078cff839ab2e14e3261a7` | match |
| Gas schedule descriptor, 9 255 bytes, `Hash::new` | `af5a27f5fc57ebfb9eae3c4f8a25091ce78d97ff7797dbc3b304a0091069731f` | match |
| HostProfile `abi.v1` definition id | `d50c38a8d0d0a229f54a43f9f7568fd45561774cb58f40668eb703ae79611b99` | match |
| Baseline selection (anchor catalog) | `56c2328657d63a48e18c7d1bac5ae8f2406e6e71249a630ece7b504d0f9cd491` | match |
| Header size | 49 bytes | match |

- The anchor has one syscall policy, `AbiV1`; `AbiV1RuntimeLifecycle` does not exist there, so
  there is no second descriptor to freeze.
- HostProfile inputs. The anchor catalog publishes `DefinitionDescriptorV1::HostProfile {
  abi_version: 1, abi_hash: compute_abi_hash(AbiV1), metering: schedule_hash() }`; its id is
  `Hash::new(b"iroha:runtime:definition:v1\0" ‖ [format version 1, kind tag 6] ‖ le64(114) ‖
  canonical Norito bytes)`. Phase 4 rebuilds it from the two frozen hashes alone (the 114
  descriptor bytes are a fixture) and checks the baseline engine's own descriptors against
  `abi_v1.descriptor.bin` and `gas_schedule_v1.descriptor.bin`. Capturing it needed
  `iroha_data_model`, `ivm_abi` and `ivm` only; `iroha_core` was not built.
- Wire tags captured from the anchor codec: `IvmAdmissionError` 0–17 (`VectorLengthTooLarge` 5,
  `MaxCyclesExceedsUpperBound` 6, `MaxCyclesExceedsFuel` 7), `IvmFaultKindV1` 0–29
  (`InvalidVectorLength` 15, `MissingHalt` 16, `VectorExtensionDisabled` 17,
  `ZkExtensionDisabled` 18), `NumericFaultV1` 1–13, `PointerAbiFaultV1` 1–11.
- Corpus: 304 artifacts (179 hand-assembled, 125 compiled from 166 Kotodama sources by the
  anchor compiler, 4 of them with the ZK mode bit forced; the 45 sources it refuses are kept
  with their diagnostics) and 499 oracle cases. Every one of the 90 valid baseline opcodes
  executes in at least one case. Each case ran three times on the anchor interpreter, twice
  plain and once PC-traced, and a second capture process reproduced every file byte for byte.
- The oracle's Core admission verdicts apply the anchor's own `admission_reason_from_vm_error`,
  `validate_header_policy`, `validate_cycle_limits`, `validate_cycle_ceiling` and
  `gas_limit_for_cycles`, extracted verbatim from its source text
  (`descriptors/core_admission_anchor.rs.txt`).
- Fault kinds reached (24 values of 21 kinds): OutOfGas, MemoryLimitExceeded,
  MemoryAccessViolation, MisalignedAccess, DecodeError, InvalidOpcode, UnknownSyscall,
  UnsupportedSyscall, Numeric (DivisionByZero, MantissaOverflow, RepeatingDecimal,
  InexactConversion), PointerAbi (InvalidAddress), AssertionFailed, ExceededMaxCycles,
  InvalidMetadata, InvalidVectorLength, MissingHalt, VectorExtensionDisabled,
  ZkExtensionDisabled, PermissionDenied, PrivacyViolation, RegisterOutOfBounds, NoritoInvalid;
  plus `ContractAbort` rejections. Admission refusals reached: every `IvmAdmissionError` the
  interpreter and header policy can produce (tags 0–3, 5–7, 10, 16, 17).
- Not reached, for phase 3 to extend: nested contract calls (shared parent cycle allowance,
  ReentrantCall, CallDepthExceeded), MemoryOutOfBounds, GasCostOverflow, NullifierAlreadyUsed,
  AbiTypeNotAllowed, the host output and AMX budget faults, the manifest and decoded-size
  admission errors (tags 4, 8, 9, 11–15, which need Core state), the ZK backend availability
  check, positive ECDSA, ML-DSA and Ed25519-batch verifications, and proof-trace
  (`zk_trace_enabled`) logs.
- The syscall behaviour in the oracle is that of the anchor's `DefaultHost`, `CoreHost` and
  `WsvHost`; a replay needs hosts with the same behaviour, not today's.
- Repository gates treat every tracked `*.ko` and `*.to` as a current-toolchain source or
  artifact, so the fixtures use `.ko.txt` and `.to.bin`. The directory is not yet listed in
  `generated-files.toml`.

## Phase 2 record (2026-11-16)

### Design note (written before implementation)

Phase 2 changes no accepted byte, result, fault, gas value, event or commitment. It names the
seams and routes today's one engine through them. Line numbers are those of the working tree at
`f06c6ea1f8` plus the live overlay of 2026-11-16.

**Types**

| Type | Crate | Meaning |
|---|---|---|
| `IvmEngine { C10 }` | `ivm_abi::profile` | The instruction-set engine: header layout, ISA, admission order, fault semantics, metering. Exhaustive matches, no wildcard arm, so a second variant fails to compile at every dispatch site |
| `HeaderLayout` | `ivm_abi::profile` | Fixed-header geometry of one engine (`size`, budget, ABI-version and ABI-hash offsets). `advertised_abi_hash` reads the declared ABI hash at that engine's offset |
| `IvmProfileId { engine, policy }` | `ivm_abi::profile` | Compiled identity of one implemented profile. `IvmProfileId::of_policy` is the only binding of a named `SyscallPolicy` to an engine |
| `ProfileEngine` trait, `C10Engine` | `ivm` (`ivm/engine.rs`, private) | Operations an engine supplies behind the one `IVM` facade: program preparation, the run loop, fault projection, metering identity |
| `ResolvedIvmProfile { id, definition_id, metering_id }` | `iroha_core::smartcontracts::ivm::profile` | Result of the single resolver. `definition_id` is the HostProfile definition this release publishes for the profile |

`SyscallPolicy` keeps both variants and gains `Ord`/`Hash`. `IVM`, `IVMHost` and every
`&IVM`/`&mut IVM` host signature are unchanged. The public fields `registers`, `memory`, `pc` and
`gas_remaining` stay: about 1 900 uses of them live in crates outside this phase's ownership
(`crates/ivm/tests`, `iroha_core_privacy`, `iroha_crypto`, `fastpq_prover`, and others). Engine
state therefore stays inline in `IVM`; the boundary is the dispatch.

**Resolution rules (unchanged behaviour, one owner)**

1. Interface default: every first-release registration and execution interface names
   `SyscallPolicy::AbiV1` (`ResolvedIvmProfile::interface_default`).
2. Named profile of the same engine: an artifact advertises one by the ABI hash in its fixed
   header, read at the interface engine's `HeaderLayout`; it resolves only when the root's
   selection activates a HostProfile with that hash (`profile::resolve_artifact`). This replaces
   the global-offset lookup of `smartcontracts/ivm.rs:567`.
3. Authenticated ABI hash (manifest or deployed binding): `profile::resolve_abi_hash`.
4. Prepared object or warm cache entry: its bound profile, accepted only if the root selects it
   (`profile::resolve_selected`).
5. Captured context: `host_profile::captured_step` lends the captured definition only inside its
   native scope. Effective restrictions are rechecked on every invocation.

No rule parses a header to pick a profile and none tries a second decoder.

**Threading points**

| Path | Site | Change |
|---|---|---|
| Resolver | `iroha_core/src/smartcontracts/ivm.rs:567` `artifact_policy`, `:580` `named_policy_selected`, `:598` `known_selected_abi`, `:603` `verify_contract_in_world` | Bodies delegate to `profile::*`; signatures stay for `state.rs`, `tx.rs`, `isi/world.rs`, `smartcontracts/code.rs` and `iroha_torii/src/gov.rs` |
| Artifact verification | `ivm_artifact_admission/src/lib.rs:101` (declared profile), `:144` `verify_contract_artifact_owned` | Declared hash read through `HeaderLayout`; verifier dispatches on the engine |
| Header parsing | `ivm_abi/src/metadata.rs:1537` `parse_header_for_policy` | Dispatches on the engine of the caller's policy |
| Program load | `ivm/src/ivm/program_load.rs:12` `prepare_for_policy` | `ProfileEngine::prepare` |
| Run loop | `ivm/src/ivm.rs:3557` `run`, `:3573` `run_with_host`, `:3581` `run_with_host_diagnostic_steps` | `ProfileEngine::run`; `run_with_host_ref` keeps its name and body (the proof-coverage scan reads it) |
| Prepared execution policy | `iroha_core/src/executor.rs:3244`, `:3274` `validate_prepared_ivm_execution_policy`; callers `:7215`, `:8427`, `:8568`, `executor_initial_permission_authority.rs:3031`, `:3073`, `:3102` | Profile-keyed variant; in-scope callers pass the summary's profile |
| Overlay admission | `pipeline/overlay.rs:1003`, `:2474`, `:2613`, `:2794` (`*_in_world`), `:7158` `validate_header_policy` | Through the resolver; header policy keyed by engine |
| Cache identity | `smartcontracts/ivm/cache.rs:66` (`entries`, `order`), `:649` `SummaryKey`, `:658` `RuntimeKey`, `:3063`–`:3140` (`*_in_world`) | Keys carry `IvmProfileId`; summaries expose `profile()` |
| Nested calls | `smartcontracts/ivm/host.rs:2745` `prepare_contract_cache_hit`, `:2759` `prepare_contract_cache_miss`, `:2728` `ensure_host_profile` | No edit: they already resolve the callee from its own root through `get_in_world`, `get_or_prepare_in_world` and `host_profile`, which now use the resolver |
| Trigger registration | `smartcontracts/isi/triggers/mod.rs:480`; `isi/world.rs:762` (wrapper) | Explicit interface-default profile instead of the implicit baseline summarizer |
| Trigger execution | `state.rs:45450`–`:46070` | Unchanged call sites; the `*_in_world` helpers and `host_profile` resolve for them |
| Captured obligations | `smartcontracts/ivm/host_profile.rs:52` `ensure_new_capture`, `:70` `captured_step`, `:95` `ensure`; `runtime/obligations/registry.rs:393` | Selected definitions and the lifecycle ABI hash come from the resolver |
| Restoration and replay | none | Both re-execute through the paths above against the State of the replayed height; a VM snapshot keeps its policy, hence its engine |
| Fault projection | `ivm_abi/src/error.rs:428` `fault_kind`, `ivm/src/ivm/diagnostic.rs:152` `execution_fault`, `iroha_core/src/smartcontracts/ivm.rs:27` `admission_reason_from_vm_error`, `:207` `map_vm_error_with_context_to_validation` | `IvmEngine::fault_kind` and `projection::admission_error(engine, ..)`; the old names remain as wrappers |

**Constraints found while reading**

- `specs/state_table_inventory.json` pins `smartcontracts/ivm/host.rs:4724` and
  `pipeline/overlay.rs:1447` by line. Edits above those lines keep the line count.
- The State inventory scan matches a function with a World reader and a hash-bearing return type,
  and every versioned domain literal. Resolver functions return `ResolvedIvmProfile`; new Core
  code has no such literal.
- The proof-coverage inventory (`crates/ivm/docs/proof_coverage_inventory.json`) records which
  source file constructs each `VMError` variant and reads `fn run_with_host_ref(`. No construction
  moves between files and the interpreter loop is untouched.
- `crates/ivm/src/ivm_cache*` and `cache_memory*` are not edited.

**Test plan**

- `ivm_abi::profile`: policy-to-engine binding of every implemented profile; `HeaderLayout`
  equals the `HEADER_*` constants; the fault projection equals an independent table for one sample
  of every `VMError` variant and for every numeric and pointer-ABI fault tag; `VMError::fault_kind`
  equals the keyed projection.
- `ivm`: a loaded VM reports its profile and engine; the engine's metering identity equals
  `gas::schedule_hash()`.
- `iroha_core::smartcontracts::ivm::profile`: interface default; artifact, ABI-hash and
  selected-profile resolution in a baseline World and in a World that selects the lifecycle
  profile; equality with the retained wrappers; each `definition_id` is the HostProfile the
  release catalog publishes.
- `iroha_core::smartcontracts::ivm::cache`: a prepared object, summary and runtime key of one
  profile is never returned for another identity of the same code hash, including a forged entry.
- `iroha_core::smartcontracts::ivm::projection`: the admission projection equals an independent
  table for every admission-class `VMError`.
- Gates: the `ivm`, `ivm_abi` and `ivm_artifact_admission` suites, the listed `iroha_core`
  groups, strict Clippy on the touched crates.
