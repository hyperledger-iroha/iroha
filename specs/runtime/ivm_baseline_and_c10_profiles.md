# IVM baseline and C10 host profiles

Status: owner decision recorded 2026-11-16; implementation in progress (phases 1–4 done: `AbiV1` runs the frozen baseline engine and Kotodama targets it by default; C10 public families are phase 5). This record
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

### As built

Phase 2 changes no accepted byte, result, fault, gas value, event or commitment: every resolution
returns a profile of the one engine, and every keyed function's only arm is today's code moved
behind it. The design above held with the deviations listed at the end.

**Where a profile comes from**

| Context | Resolver | Today's result |
|---|---|---|
| Registration and execution interfaces (contract code, raw IVM, trigger registration, executor upgrade, stateless overlay) | `ResolvedIvmProfile::interface_default()` | `AbiV1` |
| Artifact bytes through an interface, with a root | `profile::resolve_artifact(world, bytes)` (wrapped by `artifact_policy`) | `AbiV1RuntimeLifecycle` only when the header advertises its hash at the interface engine's `HeaderLayout` and the root selects it; otherwise `AbiV1` |
| Authenticated ABI hash (manifest, deployed binding) | `profile::resolve_abi_hash` (wrapped by `known_selected_abi`) | the exact implemented profile, if selected |
| Prepared object or warm cache entry | `profile::resolve_selected(world, id)` | its bound profile, if selected |
| Captured obligation | `host_profile::{ensure_new_capture, ensure}` through `profile::activates` | unchanged lending and restriction checks |

`ResolvedIvmProfile { id: IvmProfileId, definition_id, metering_id }` is compiled once per
policy; `definition_id` is the HostProfile id this release's catalog publishes (a test checks it
against `release_catalog()` and the baseline selection). Selection still matches on ABI version
and ABI hash, exactly as before; the metering hash of a selected definition is not compared.
`AbiV1` stays implicit (`is_implicit`): no root looks it up in its selection.

**Threading as built**

| Path | Files | Change |
|---|---|---|
| Header parsing | `ivm_abi/src/metadata.rs` | `parse_header_for_policy` dispatches on `IvmProfileId::of_policy(policy).engine()` |
| Artifact verification | `ivm_artifact_admission/src/lib.rs` | `verify_contract_artifact_owned` dispatches to `verify_c10_contract_artifact`; the declared-profile helper reads the hash through `IvmProfileId::declared_in(engine, ..)` |
| Program load and run | `ivm/src/ivm/engine.rs`, `program_load.rs`, `ivm.rs` | `ProfileEngine` (`prepare`, `run`, `fault_kind`, `metering_schedule_hash`), `C10Engine`; `run`, `run_with_host` and `run_with_host_diagnostic_steps` dispatch on `vm.engine()`; `run_with_host_ref` is unchanged |
| Facade | `ivm/src/ivm/engine.rs`, `prepared.rs`, `gas.rs` | `IVM::{profile_id, engine, set_heap_max_limit, code_len}`, `PreparedContract::profile_id`, `gas::schedule_hash_for(engine)`; Core callers use the facade methods instead of `vm.memory` for the heap ceiling and code length |
| Prepared execution policy | `iroha_core/src/executor.rs`, `executor_initial_permission_authority.rs` | `validate_prepared_ivm_execution_policy_for_profile(state, summary.profile(), ..)` and the governed proved variant at every in-scope caller (8 sites); the executor upgrade loads with the interface profile's policy |
| Overlay admission | `pipeline/overlay.rs` | `validate_header_policy_for(summary.profile().engine(), ..)` at the 9 summary sites; the stateless raw-IVM path resolves the interface profile before parsing and loading |
| Cache identity | `smartcontracts/ivm/cache.rs` | `SummaryKey { code_hash, profile }` and `RuntimeKey { .., profile }` key the prepared-contract store, its order and eviction, nested runtimes, local summaries, generic summaries, analyses and runtime templates; `GenericProgramSummary` carries its profile and both summary kinds expose `profile()`; a lookup that knows only a content address takes the resident entry's own profile and the root's selection decides; a request for another named policy of the same engine reports that policy's header mismatch, as a cold cache does; entries of another engine are never consulted |
| Triggers | `smartcontracts/isi/triggers/mod.rs` | registration names the interface profile explicitly |
| Captured obligations | `smartcontracts/ivm/host_profile.rs`, `runtime/obligations/registry.rs` | definitions activating a profile come from `profile::activates`; the lifecycle ABI hash comes from the resolved profile |
| Fault projection | `ivm_abi/src/profile.rs`, `ivm/src/ivm/diagnostic.rs`, `iroha_core/src/smartcontracts/ivm/projection.rs`, `smartcontracts/ivm.rs` | `IvmEngine::fault_kind` (with `c10_numeric_fault`, the identity) owns `VMError` → `IvmFaultKindV1`/`NumericFaultV1`; `IVM::execution_fault` projects with the VM's engine; `projection::admission_error(engine, ..)` owns `VMError` → `IvmAdmissionError`; `map_vm_error_with_context_to_validation` uses the engine of the VM that ran for both |

Unchanged on purpose: nested calls (`host.rs`, line-pinned) already resolve the callee through
`get_in_world`, `get_or_prepare_in_world` and `host_profile`; trigger execution, restoration and
replay re-execute through the same helpers against the State of their height.

**Retained unkeyed entry points** (each applies the interface profile and carries a `TODO:`):
`VMError::fault_kind`, `admission_reason_from_vm_error`, `program_admission_error`,
`validate_prepared_ivm_execution_policy`, `validate_header_policy`, the policy-less
`PreparedContractCache`/`IvmCache` methods, and `IVM::{load_program, validate_program}`. Callers
outside this phase's edit scope keep them: `state.rs` (trigger and contract execution, 5 sites),
`tx.rs` (2), `smartcontracts/code.rs`, `isi/world.rs`, and `map_program_summary_error` above the
line-pinned region of `overlay.rs`.

**What phase 3 adds**

1. `IvmEngine::Baseline` and its arms: `IvmEngine::{header_layout, fault_kind}`,
   `IvmProfileId::of_policy` (binding `AbiV1` to it once C10 has its own policy),
   `parse_header_for_policy`, `verify_contract_artifact_owned`, `engine::{prepare, run,
   fault_kind, metering_schedule_hash}`, `projection::admission_error` and
   `validate_header_policy_for`. The compiler lists every site.
2. Baseline machine state. `IVM` stores C10's state inline because about 1 900 uses of
   `registers`, `memory`, `pc` and `gas_remaining` live outside this phase's crates. The 256-register
   baseline needs either its own state behind the facade with those consumers moved to accessors,
   or a register file sized by engine; that decision belongs to phase 3.
3. Callers listed above move from the unkeyed entry points to the keyed ones.
4. Cross-engine cache tests: keys already carry the engine, and same-engine mismatch reporting
   never consults another engine's entries.

**Tests added**: `ivm_abi::profile` (4), `ivm::ivm::engine` (3),
`iroha_core::smartcontracts::ivm::profile` (4), `::projection` (3), `::cache::profile_key_tests`
(5). The second profile identity they use is the shipped `AbiV1RuntimeLifecycle`; no test-only
identity was needed.

**Deviations from the design note**: none in behaviour. `IvmEngine::fault_kind` lives in `ivm_abi`
(the owner of `VMError`), the `ProfileEngine` trait is private to `ivm`, and the State and ZK
inventories needed no regeneration (no pinned line moved, no counted token changed).

**Gates** (frozen copy `r-phase2`: `f06c6ea1f8` + the live overlay of 2026-11-16 + `c10-land.patch`; the landed files are byte-identical to it). `iroha_core` lib tests (`iroha_core-p2a`): captured 9/0, profile/projection/host_profile/cache 81/0, code 32/0, triggers 274/0, remaining `smartcontracts::ivm::` 16/0, runtime 272/2, executor 504/0, pipeline 169/0, ivm host 474/1, inventory 266/1. The failures are pre-existing and identical on build-8 (the same tree without phase 2): both `runtime::taira_activation` tests (phase 4) and `runtime_obligation_host_gas_refusal_publishes_no_effect_or_return` (stale under C10); the inventory failure was the copy's older test file, already fixed in the tree. `ivm` lib 1554/0, `ivm_group_01`–`10` all pass, custody tests 1 + 9, `ivm_abi` 279 + 12, `ivm_artifact_admission` 58 + 16 + 3. Strict Clippy (`-p ivm_abi -p ivm -p ivm_artifact_admission -p iroha_core --all-targets -D warnings`): exit 0.


Coordinator changes after the agent's run, both found by per-crate strict Clippy (workspace Clippy unifies the `iroha-core-tests` feature on, which hid them): the `named_policy_selected` wrapper was deleted (its only remaining use was a test asserting it equals `profile::is_selected`), and `IvmCache::summarize_program_in_world`, called only by the `test`/`iroha-core-tests` overlay builders, is gated like them.

Re-gate on `875cb62d5e` + phase 2 (copy `r-phase2-head`, identical to the landed live tree; its only Rust differences
from `r-phase2` are the queue partition fix and the State inventory test): strict Clippy of the four crates exit 0;
`iroha_core` lib tests `queue::` 516/0, `parliament` 301/0, `inventory` 267/0, `smartcontracts::ivm::{profile,
projection, host_profile, cache}` 81/0.

## Phase 3 record (2026-11-16)

Phase 3 adds the frozen baseline engine beside C10 behind the one `IVM` facade. The baseline
engine is reachable only through the explicit identity `IvmProfileId::BASELINE_ABI_V1`; no named
policy binds to it (`IvmProfileId::of_policy` still binds `AbiV1` and `AbiV1RuntimeLifecycle` to
C10), so every live path, accepted byte, fault, gas value, event and commitment is unchanged.
`iroha_data_model`, `kotlin/`, `IrohaSwift/` and `specs/runtime_baseline_catalog.json` are not
touched.

### Machine-state decision

Option (b), a register file sized for the widest engine. `Registers` stores 256 values and 256
privacy tags; the C10 engine addresses the first 32. The retained Merkle commitment,
`Registers::snapshot`, every authentication path and the register logs still cover exactly the 32
C10 registers, so a C10 root, path and log are byte-identical. A baseline image commits all 256
registers through `Registers::baseline_merkle_root` (rebuilt from the values on each request) and
`IVM::register_root` dispatches on the engine. The few values only the baseline reads (vector
enable and length, the header vector length, the constraint log, the shared cycle allowance) live
in one `BaselineMachine` field of `IVM`, inert for C10 images.

Why not (a), separate baseline state behind the facade: hosts receive `&mut IVM` and read and write
`vm.registers`, `vm.memory`, `vm.pc` and `vm.gas_remaining` directly; about 1 900 such uses live in
crates outside this phase. Moving them all to accessors is a workspace-wide refactor, and the
alternative, copying r0..r31 between two register files around each host callback, is a parallel
state that can diverge. Option (b) keeps one register file, one memory and one gas counter for both
engines, costs 2 KiB more per `Registers`, and changes no C10 value.

### As built

| Area | Files | What |
|---|---|---|
| Engine identity | `ivm_abi/src/profile.rs` | `IvmEngine::Baseline` (in `ALL`); `HeaderLayout` 49/8/16/17; `IvmProfileId::BASELINE_ABI_V1`, `of_engine`; `abi_hash` and `for_abi_hash` per engine; operand decoders shared by static analyses (`syscall_number`, `direct_transfer_offset`, `destination_register`, `is_indirect_transfer`); `fault_kind` Baseline arm |
| Frozen definitions | `ivm_abi/src/baseline.rs`, `baseline/{instruction,encoding}.rs` | Anchor instruction and encoding tables verbatim; 49-byte header constants and encoder; VECTOR mode and CNTR feature bits; frozen wire mirrors `FaultKindV1`, `FaultV1`, `AdmissionErrorV1` (+ `VectorLengthTooLargeInfo`, `MaxCyclesExceedsFuelInfo`) with the original tags and Norito layout; the anchor fault projection, `admission_reason_from_vm_error`, Core header policy and cycle checks |
| Internal faults | `ivm_abi/src/error.rs` | `VMError::{ProgramVectorLengthTooLarge, InvalidVectorLength, VectorExtensionDisabled, ZkExtensionDisabled}` restored with the anchor texts; `InvalidOpcode(u16)` (C10 reports its opcode byte, so its value, text and debug form are unchanged); `VmTrapKind::{ProgramVectorLengthTooLarge, InvalidVectorLength}` |
| Header | `ivm_abi/src/metadata.rs`, `metadata/program_header.rs` | `parse_header_for_profile`/`parse_for_profile` dispatch on the engine; `parse_baseline` keeps the anchor layout and precedence (version, mode bits, ABI version, vector length, ABI hash); parsed headers and metadata carry `profile`, `header_len` and `vector_length` |
| ABI descriptor | `ivm_abi/src/syscalls.rs` | `baseline_abi_surface_descriptor`/`compute_baseline_abi_hash`: today's ABI V1 surface with descriptor format 10, the 49-byte header text, the original fault layout text, no `register_integer_overflow` numeric fault and no ISA record. Hashes to `7531b45d…`; bytes equal the fixture |
| Gas | `ivm/src/gas/baseline.rs`, `gas.rs` | Anchor opcode table (90 opcodes, max 5 000), vector scaling, signature-opcode and Ed25519-batch formulas; the baseline schedule descriptor shares the syscall registry and staged phases and re-inserts the 13 baseline parameters at their original positions. Hashes to `af5a27f5…`; bytes equal the fixture |
| Register file | `ivm/src/registers.rs` | Option (b) above |
| Interpreter | `ivm/src/ivm/baseline.rs`, `baseline/field.rs` | The anchor `run_with_host_ref` verbatim except: anchor field extractors (`bwide`), baseline-only state through `BaselineMachine`, call-frame helpers shared with C10 mapped back to `AssertionFailed` (`baseline_call_fault`), refusal of the proof-trace producers it does not reconstruct. `VmCycleBudget`, `run_with_host_and_cycle_budget` and `run_with_host_and_parent_cycle_budget` restored (a C10 image refuses an allowance with `HostUnavailable`); ZK padding to the header bound; the anchor Goldilocks field module; baseline execution summary with the constraint digest |
| Facade and preparation | `ivm/src/ivm.rs`, `ivm/engine.rs`, `ivm/program_load.rs`, `prepared.rs`, `prepared/{control_flow,entrypoints}.rs`, `contract_artifact.rs`, `ivm/diagnostic.rs` | `BaselineEngine` (prepare, run, fault projection, metering identity) beside `C10Engine`; `IVM::{load_program_for_profile, validate_program_for_profile, baseline_execution_fault, baseline_execution_summary, vector_length}`; the installed profile is stored, not derived from the policy; prepared streams are validated and priced by the image's engine and the code-keyed prepared cache is keyed `(engine, sha256)`; control-flow and private-input reachability indexes decode in the image's engine layout; `prepare_contract_for_profile` and the budgeted variant |
| Hosts | `ivm/src/host.rs`, `mock_wsv/contract_calls.rs` | `SYSCALL_EXECUTION_SUMMARY` prices and returns the summary layout of the VM's engine; `WsvHost::install_contract_fixture_for_profile` |
| Admission | `ivm_artifact_admission/src/{lib,policy,admitted_program}.rs` | `verify_contract_artifact_owned` dispatches on the profile's engine; the same admission sequence reads the baseline header and applies the baseline mode bits, ABI hash, CNTR VECTOR feature rule, opcode validity, POSEIDON6 canonical form, `JR` refusal, `JALR` form, branch offsets and syscall forms; the admitted-program seal binds the profile and vector length |
| Core | `iroha_core/src/smartcontracts/ivm/projection.rs`, `pipeline/overlay.rs`, `smartcontracts/ivm/cache/profile_key_tests.rs` | Baseline arms of `admission_error` and `validate_header_policy_for` (the anchor rules, carried on today's wire enum; see deviations); a cross-engine cache-key test |
| Other crates | `irohad/src/soracloud_runtime.rs` | Labels of the four restored `VMError` variants (the anchor's labels) |
| Inventories | `ivm/src/proof_coverage/{traps,tests}.rs`, `ivm/docs/proof_coverage_inventory.json` | The restored variants and trap kinds, the baseline interpreter's producer record; regenerated inventory |
| Differential | `ivm/tests/baseline_v1_differential.rs` (in `ivm_group_01`) | See below |

### Differential evidence

`baseline_engine_reproduces_every_oracle_case_exactly` rebuilds every one of the 499 oracle
records on the baseline engine exactly as the capture program did (same VM builder, hosts, setup
steps, preludes, shared cycle allowance, recording host) and compares every recorded field:
artifact hash, VM admission (result, debug and display text), Core admission (effective cycles,
header-to-gas limit, or the exact original `IvmAdmissionError` variant, tag, bare bytes, JSON,
debug and display), the header-derived gas limit, the run result (error debug/display, unmetered
form, metered gas, deferral, original fault kind with tag/bare/JSON/debug, the `IvmFaultV1`
projection with JSON, bare bytes, the canonical frame and display), PC, cycles, gas, vector
length, all 256 registers, private tags, register root, output, the 26-field execution summary,
the return record, prelude runs, the shared allowance, the syscall-event digest and the first 16
events, Core host state, contract events, and for the 464 traced cases the PC-trace count, digest
and opcode histogram. All 499 cases are equal. A mutation check edited three values in a scratch
copy of `oracle.jsonl` (a gas value, a constraint digest, a fault wire tag); each was reported and
the file was restored. `baseline_engine_descriptors_are_the_frozen_fixture_bytes` compares the
engine's ABI and gas-schedule descriptors with the fixture bytes and their hashes with
`7531b45d…`/`af5a27f5…`.

### Deviations

- Wire values the data model no longer carries (`IvmFaultKindV1`, `IvmFaultV1`,
  `IvmAdmissionError` with their original tags) are frozen mirrors in `ivm_abi::baseline`
  (`FaultKindV1`, `FaultV1`, `AdmissionErrorV1`), not data-model types: phase 3 must not write
  `iroha_data_model`. `IvmEngine::fault_kind(Baseline)`, `projection::admission_error(Baseline)` and
  `validate_header_policy_for(Baseline)` return today's wire enums; the kinds and rejections those
  enums cannot name (`InvalidVectorLength`, `VectorExtensionDisabled`, `ZkExtensionDisabled`;
  `VectorLengthTooLarge`, `MaxCyclesExceedsFuel`) return `None` or a `BytecodeDecodingFailed`
  carrying the baseline text. No live profile reaches these arms in phase 3.
- `VMError::InvalidOpcode` widened from `u8` to `u16` so the baseline can report its original value
  (the low 16 bits of the word). C10 values, texts and debug forms are unchanged; the diagnostic
  context keeps its `u8` slot and drops a baseline value that does not fit.
- The baseline's proof-trace producers are not reconstructed: a baseline run with ZK trace
  collection, an active scoped register logger, `TraceMode::DeltaRegisters`, native execution
  packets or a diagnostic step recorder is refused locally with
  `ExecutionDeferred(TraceOwnerUnavailable)`. The corpus covers none of them; consensus execution
  requests none of them.
- The same artifact bytes prepared under two engines are distinct prepared-cache entries.
- Raw code (`IVM::load_code`, no header) always installs the C10 interface profile; the baseline
  engine runs only artifacts with a baseline header.

### Gates

Run in the frozen copy on 2026-11-16, every cargo step as `scripts/cargo_fast.sh --no-incremental
--stable-local-metadata --jobs 4 --target-dir <copy>/target-ivm -- <args>` (Clippy:
`--target-dir <copy>/target-clippy`), and every test binary alone from its package directory.

| Gate | Command | Result |
|---|---|---|
| Build | `test -p ivm -p ivm_abi -p ivm_artifact_admission --tests --locked --offline --no-run` | exit 0 |
| `ivm` lib | test binary, `--test-threads 6` | 1572 passed, 0 failed, 6 ignored |
| `ivm_group_01` … `ivm_group_10` | each binary | 136, 79, 85, 152, 244, 108, 88, 129, 66, 29 passed; 0 failed |
| `ivm` custody tests | `poseidon_parameter_custody`, `shared_owner_custody` | 1 and 9 passed; 0 failed |
| Differential | `ivm_group_01` (`baseline_v1_differential`) | 499 of 499 oracle cases equal; descriptors equal the fixture bytes |
| `ivm_abi` | lib; `axt_policy_vectors`, `call_schema_allocations`, `norito_pointer_abi_roundtrip`, `runtime_upgrade_abi_fixture` | 298; 6 (1 ignored), 3, 2, 1 passed; 0 failed |
| `ivm_artifact_admission` | `test -p ivm_artifact_admission --lib --locked --offline --no-run`, then lib; `current_artifact`, `preparation_error_custody` | 60; 16, 3 passed; 0 failed |
| Core build | `test -p iroha_core --lib --locked --offline --no-run` | exit 0 |
| Core `smartcontracts::ivm::{profile,projection,host_profile,cache}::` | lib binary, `--exact` on the listed names | 83 passed, 0 failed |
| Core `smartcontracts::code::` | same | 32 passed, 0 failed, 2 ignored |
| Core `pipeline::overlay::baseline_header_policy_tests::` | same | 1 passed, 0 failed |
| Core other `smartcontracts::ivm::` modules | same | 16 passed, 0 failed |
| Core `smartcontracts::ivm::{host,return_value}` | same | 474 passed, 1 failed, 1 ignored |
| Strict Clippy | `clippy -p ivm_abi -p ivm -p ivm_artifact_admission -p iroha_core --all-targets --locked --offline -- -D warnings` | exit 0 |
| `irohad` | `clippy -p irohad --bins --locked --offline` (no lib target) | exit 0; warnings only in untouched `zk-spartan`, `halo2curves`, `ark-ff` |
| Format | `rustfmt --edition 2024 --check` over the 50 touched Rust files | clean |

The one failure, `smartcontracts::ivm::host::tests::runtime_obligation_host_gas_refusal_publishes_no_effect_or_return`
(panic at `host_tests.rs:11416`), is one of the known failures of the base: it fails alone twice on
this build and alone on the phase-2 test binary. The Core `host` group ran on the first Core build;
the later source changes were a removed cast in `profile.rs`, a removed stray doc comment in
`ivm/baseline.rs` and test-only edits in `ivm` and `ivm_artifact_admission`, and the four shorter
Core groups were rerun on the final Core build with the same counts. The Clippy, `ivm`, `ivm_abi` and
admission results are of the final source. Not run in phase 3: the full `iroha_core` lib suite and
its integration tests, `integration_tests`, the workspace suite, and the Kotlin and Swift SDK
suites. `kotodama_lang` was not touched by this part; its gates are those of the backend below.

### Kotodama baseline backend

`CompilerOptions::engine` (an `IvmEngine`) selects the encoding a build emits;
`CompilerOptions::target_profile()` resolves it with `host_policy` to one `IvmProfileId`
(`IvmProfileId::BASELINE_ABI_V1` for the baseline, which implements only `AbiV1`; asking for the
baseline with the lifecycle policy is refused with `K4001`). The front end (parser, resolution,
semantic analysis, IR, SSA, register allocation) is shared: the anchor compiler differed from
today's only in code generation and in three compiler-internal builtins it already refused in
source. Code generation plans the same instruction sequence for both engines; what the engine
decides lives in one table, `kotodama_lang::compiler::isa::Isa`, threaded explicitly through every
emitter:

| Decision | C10 | Baseline (anchor) |
|---|---|---|
| Word layout | `ivm_abi::{encoding, instruction}` | `ivm_abi::baseline::{encoding, instruction}` |
| Immediates, displacements, branch word offsets | signed 14 bits | signed 8 bits: more chunked `ADDI`s, literal loads, stack-table and saved-register windows and branch relaxation |
| `JAL` word offset | signed 19 bits | signed 16 bits |
| System calls | one 24-bit `SCALL` | 8-bit `SCALL` up to `0xFF`, 24-bit `SYSTEM` above |
| Return | `JALR` register-immediate form | `JALR` register form |
| Relocation, relaxation and numeric-zero scans | C10 decoders | anchor decoders (including `SYSTEM`) |
| Header | 48 bytes; `max_cycles` only for ZK (`zk_trace_cycles`) | 49 bytes, vector length 0; `max_cycles` = `baseline_max_cycles` (default 1 000 000, the anchor's `DEFAULT_MAX_CYCLES`) for every artifact; `ZK`/`VECTOR` mode bits also from the anchor's opcode scans |
| CNTR | ZK feature bit; C10 ABI hash | ZK and `VECTOR` feature bits; baseline ABI hash `7531b45d…` |
| Budget report `jump_range_risk` | above the 19-bit `JAL` range | above the 16-bit range |

The manifest assembly, `BuildDriver` freshness and publication checks and the session
fingerprint follow the target profile; a baseline build has its own fingerprint (including its
cycle ceiling), so it never shares a build-cache entry with a C10 build.

**Default target.** The default stays C10 in phase 3: flipping it changes the bytes of every
build that leaves `engine` at its default, and those builds deploy under the live `AbiV1`
binding, which executes C10. The phase-4 cutover flips `CompilerOptions::default().engine` to
the baseline (C10 explicit) together with `IvmProfileId::of_policy`. The flip affects:

- every non-test caller of the default compiler: `iroha_cli` (contract commands in `contracts.rs`),
  `kotodama_toolchain`, `musubi` (build, deploy, test runner, bindgen, LSP), `izanami`,
  `iroha_js_host`, `xtask`, and `iroha_torii`'s contract-source verification (it recompiles a
  recorded source and compares code hashes, so recorded sources must name their engine);
- test callers in `ivm` (`src/ivm.rs`, `contract_artifact.rs`, trace/register-logging and
  execution-packet tests and the 57 `tests/kotodama*.rs` files), `iroha_core`,
  `iroha_deploy`, `ivm_artifact_admission`, `iroha_cli/bins`, `iroha_contract_deploy`, the
  Python bindings and `integration_tests`;
- the 58 Kotodama rows of `scripts/ivm_artifacts.tsv` (53 standard, 5 ZK: samples, IVM docs
  examples, IVM test data, `demo`, `examples`, `integration_tests` fixtures and the client's
  contract-code read-back fixture), which `repository_ivm_artifacts` recompiles and requires to
  carry the C10 header and ABI hash.

**Evidence.** `tests/baseline_v1_corpus.rs` (in `kotodama_integration`) compiles every one of
the 166 corpus sources with the baseline target, as the capture did (`compile_source`, plus the
`force_zk` variants), and requires the anchor's outcome exactly: 125 artifacts byte for byte (121
plain and 4 `force_zk`) and 45 refusals with identical rendered diagnostics. All 170 outcomes
are equal. A mutation check flipped one byte of a plain and of a `force_zk` artifact and one
character of a diagnostic in the copy's fixtures; each was reported, and the fixtures were
restored (`diff` against the base is empty). Unit tests cover the `Isa` table against the
anchor encoders, the target-profile resolution, a baseline build's header, CNTR, admission
under `BASELINE_ABI_V1` and refusal under the C10 interface profile, the session fingerprint and
a `BuildDriver` baseline build (built, then fresh, distinct from C10, ABI-hash substitution
refused). The C10 output is unchanged: every existing `kotodama_lang` byte-level test and the
`ivm` suites below pass, including `repository_ivm_artifacts`, which recompiles the checked-in
Kotodama artifacts with the default target.

**Deviations and limits.**

- The front end is today's for both engines. The anchor refused the compiler-internal
  `poseidon2`, `poseidon6` and `setvl` in source with `E_INTERNAL_BUILTIN`; today they are
  unknown (`K2002`), so the refusal stands but its diagnostic differs, and their lowering to the
  direct `POSEIDON2`/`POSEIDON6`/`SETVL` opcodes (unreachable from source at the anchor) is not
  reconstructed. Builtins added after the anchor (runtime-obligation registration and
  completion) parse for both engines; the baseline target refuses them at lowering because they
  need the lifecycle profile, where the anchor reported an unknown function. No corpus source is
  affected.
- `linker/source_bundle.rs` admits imported contract artifacts under the declared C10 profiles;
  a baseline build that imports another contract's artifact needs the session's target profile
  threaded there (TODO in place; no corpus source imports a contract).
- The local test harness (`CompilerMode::Test`) emits a baseline header for the baseline target;
  the corpus has no test-mode build, so it is unverified against the anchor.

#### Gates (backend)

Run in the frozen copy on 2026-11-16 with the same `scripts/cargo_fast.sh` flags as above; every
test binary alone from its package directory.

| Gate | Command | Result |
|---|---|---|
| Build | `test -p kotodama_lang --lib --test kotodama_integration --locked --offline --no-run` | exit 0 |
| `kotodama_lang` lib | test binary, `--test-threads 6` | 1575 passed, 0 failed, 6 ignored |
| `kotodama_integration` (includes `baseline_v1_corpus`) | test binary | 104 passed, 0 failed |
| Corpus | `baseline_v1_corpus` (3 tests) | 170 of 170 outcomes equal: 125 artifacts, 45 diagnostics |
| Doc tests | `test -p kotodama_lang --doc --locked --offline` | 1 passed |
| `ivm`-family rebuild against the new compiler | `test -p ivm -p ivm_abi -p ivm_artifact_admission --tests --locked --offline --no-run` | exit 0 |
| `ivm` lib, `ivm_group_01` … `ivm_group_10`, custody tests | each binary | 1572 (6 ignored); 136, 79, 85, 152, 244, 108, 88, 129, 66, 29; 1 and 9 passed; 0 failed |
| `ivm_abi`, its four test binaries; `ivm_artifact_admission` and its two | each binary | 298; 6 (1 ignored), 3, 2, 1; 60; 16, 3 passed; 0 failed |
| Strict Clippy | `clippy -p ivm_abi -p ivm -p ivm_artifact_admission -p iroha_core -p kotodama_lang --all-targets --locked --offline -- -D warnings` | exit 0 |
| Format | `rustfmt --edition 2024 --check` over the 18 touched Rust files | clean |

No failure. `ivm_group_04` includes `repository_ivm_artifacts`, which recompiled the 58 checked-in
Kotodama artifacts with the (C10) default target and found them fresh. The only non-Kotodama
source change of this part is a doc comment in `ivm_abi::baseline`; the `iroha_core` test groups
were not rerun for it (Clippy rebuilt `iroha_core` against it).

### Not done in phase 3, and what remains

- Kotodama: the default target flips to the baseline in phase 4 (callers above); contract
  imports in a baseline source bundle (TODO in `linker/source_bundle.rs`).
- Phase-2 item 3 (moving the unkeyed Core callers in `state.rs`, `tx.rs`, `smartcontracts/code.rs`,
  `isi/world.rs` and the line-pinned part of `overlay.rs` to keyed entry points): not done. They
  apply the interface profile, which is still C10.
- Not covered by the corpus and not separately tested: nested baseline contract calls (shared
  parent allowance, `ReentrantCall`, `CallDepthExceeded` from hosts), proof-trace logs,
  Core-state admission errors (tags 4, 8, 9, 11–15), the ZK backend availability check.
- Phase 4 (atomic cutover): bind `AbiV1` to the baseline engine in `IvmProfileId::of_policy` and
  give C10 its own policy; restore the original `IvmFaultKindV1`, `IvmFaultV1`,
  `IvmAdmissionError` (with `VectorLengthTooLargeInfo`, `MaxCyclesExceedsFuelInfo`) and every
  enclosing frozen type in the data model, replace the `ivm_abi::baseline` mirrors and the lossy
  arms; restore the baseline Core admission order (`validate_cycle_limits`,
  `validate_cycle_ceiling`, header-to-gas `gas_limit_for_cycles` with
  `gas::baseline::max_instruction_cost`) and the shared cycle allowance wiring the anchor Core had
  (`run_with_host_and_cycle_budget` for quarantine-lane sources in `executor.rs`,
  `run_with_host_and_parent_cycle_budget` for nested calls in `host/contract_calls.rs`); frozen catalog
  constants; restored goldens; the known failures listed above close here.
- Phase 5: C10 public families and registrations under the `iroha.runtime.ivm_c10.v1` addition.
- Phase 6: cross-profile and four-validator qualification, replay, hardware parity.

## Phase 4 record (2026-11-16)

The atomic cutover. `IvmProfileId::of_policy(SyscallPolicy::AbiV1)` is
`IvmProfileId::BASELINE_ABI_V1`: every live `AbiV1` path (transaction admission and execution,
triggers, the executor and its upgrade, contract registration and calls, Torii readers of stored
artifacts) parses, admits, prices and runs the frozen baseline engine. `AbiV1RuntimeLifecycle`
stays bound to C10 (`IvmProfileId::C10_RUNTIME_LIFECYCLE`); C10's own ABI V1 surface is reachable
only as the explicit `IvmProfileId::C10_ABI_V1`, which no root selects until phase 5.

### As built

| Area | Change |
|---|---|
| Profile binding | `ivm_abi/src/profile.rs`: `of_policy(AbiV1)` is the baseline; `C10_ABI_V1`, `C10_RUNTIME_LIFECYCLE`, `of_engine_policy`, `of_abi_hash`, `declared` (each implemented profile's hash read at its own engine's header position). `compute_abi_hash(policy)` is the bound profile's hash (`AbiV1` → `7531b45d…`); `compute_c10_abi_hash` is C10's surface (`AbiV1` → `bdd5188f…`). The C10 header parser now reports the C10 profile of its policy (it reported `of_policy(policy)`, which became the baseline). |
| Wire types | `iroha_data_model/src/executor.rs` and `executor/fault.rs` are the anchor's: `IvmAdmissionError` (tags 0–17 with `VectorLengthTooLargeInfo`, `MaxCyclesExceedsFuelInfo`), `IvmFaultKindV1` (0–29), `NumericFaultV1` (1–13). The `ivm_abi::baseline` mirrors are deleted; the baseline projections return the data-model values directly, so no baseline arm is lossy. C10's `RegisterIntegerOverflow` is an internal `VMError` (tag constant 14, never a wire value); `ControlFlowViolation`, `RegisterIntegerOverflow` and `MaxCyclesWithoutZk` have no baseline wire value, so the C10 projection returns `None` or carries the VM text (TODO phase 5: `*C10V1` families). |
| Catalog | `runtime/catalog.rs` builds the baseline HostProfile from the frozen ABI hash and gas schedule constants (`d50c38a8…`, selection `56c23286…`); the lifecycle HostProfile is metered by C10's schedule (`gas::c10_schedule_hash`, `46aa31d8…`). `specs/runtime_baseline_catalog.json` is the anchor's. `gas::schedule_hash()` is the baseline schedule (`af5a27f5…`). |
| Core cycle budgets | The anchor Core's baseline rules, keyed by the resolved engine: header `max_cycles` required and bounded by fuel then the committed ceiling (`execution_cycle_budget_for`, `validate_cycle_ceiling_for`), header-to-gas `gas_limit_for_cycles` (× 5 000) in the executor, the shared parent cycle allowance for nested calls (`host/contract_calls.rs`), and the cycle budget installed on every executor run. C10 checks only its ZK trace bound. Unkeyed Core callers (`state.rs`, `tx.rs`, `smartcontracts/code.rs`, `isi/world.rs`, `pipeline/overlay.rs`, `pipeline/access.rs`, triggers) use the resolved profile; the unkeyed `validate_header_policy` is deleted. |
| Analyses | `ivm::analysis` (AMX budget, syscall scans, static-state keys, direct-call edges) decodes in the program's engine; the baseline rules are the anchor's (256 registers, `LOAD128`/`STORE128`, 8-bit `SCALL` plus `SYSTEM`). |
| Unkeyed conveniences | `ProgramMetadata::{parse, parse_header, encode}`, `IVM::{load_program, validate_program, load_code}`, `prepare_contract`, `verify_contract_artifact`, `MockWorldStateView::install_contract_fixture` and `analysis::{analyze_program, program_syscall_numbers}` are explicitly the C10 profile and are not called on a live path (a temporary `#[deprecated]` audit over every workspace target found the production callers; all were moved). Offline tools (CLI manifest generation and `sorafs` toolkit, `musubi`, `koto`, contract deploy) read the profile an artifact declares (`verify_contract_artifact_with_declared_profile`, `IvmProfileId::declared`); Torii readers of stored artifacts use the root's resolution (`smartcontracts::ivm::artifact_policy`, now public); the Soracloud runtime prepares under the interface profile. |
| Kotodama | `CompilerOptions::default().engine` is the baseline (`CompilerOptions::c10()`, `Compiler::new_c10()`, `CompilerSession::c10()` select C10). `koto`'s `abi-v1` host profile builds the baseline; `iroha.ivm.runtime_lifecycle.v1` builds C10. Torii's verified-source recompilation records and replays the engine. |
| Cache identity | `IvmCache`'s `ProgramSummary::meta_hash` and `GenericProgramSummary::meta_hash` hash the authenticated fixed-header bytes of the program (the anchor's value for a baseline program); they had hashed a C10-layout re-encoding, which drops a baseline header's `vector_length`. The hash is local to the cache. The two implemented profiles now run different engines, so neither has a same-engine foreign key: a request for the runtime lifecycle policy of a baseline-resident artifact is refused by the C10 header check, exactly as a cold cache refuses it. |
| Documentation | `crates/ivm/docs/opcodes.md` is the anchor's baseline reference plus a C10 section; `specs/gas_model.md` is the anchor's baseline schedule plus a C10 schedule section; `crates/ivm/docs/syscalls.md`, `architecture_spec.md`, `calling_convention.md`, `conformance.md`, `gpu_offloading.md`, `performance_tasks.md`, `iso20022.md`, `crates/ivm/README.md`, `crates/ivm/AGENTS.md`, `specs/ivm_isi_kotodama_alignment.md` and `specs/new_pipeline.md` state which engine each ISA, header and budget claim belongs to. |
| Raw trigger fixture | `ivm::prebuilt_fixtures::build_raw_trigger_program` is the anchor's raw trigger artifact byte for byte; Izanami deploys it instead of a C10 predecoder fixture. |
| Goldens | Restored, not re-pinned: `abi_v1_hash.rs` (`7531b45d…`), `gas_schedule_hash.rs` (`af5a27f5…`, plus a C10 schedule pin), `axt_golden.rs`, `specs/samples/runtime_abi_hash.md`, the Android executor-hash fixture, `defaults/executor.to` and the Ironwood change-set ids (`runtime/ironwood/tests.rs`) are byte-identical to `77600a4840`. `release_identity.rs` and `specs/ironwood_runtime_transition.md` state `7531b45d…`. `specs/ivm_header.md` documents both engines' headers (generator `gen_header_doc`). |
| Generated artifacts | The 58 Kotodama rows of `scripts/ivm_artifacts.tsv` and `.kotodama-v1-owner-manifest.json` regenerated by `scripts/regenerate_kotodama_goldens.py`: every artifact is byte-identical to the anchor's. The JS current-artifact fixture's artifact and semantics equal the anchor's; its source-closure provenance is the copy's and must be regenerated after landing (it hashes the live source closure). `regenerate_kotodama_goldens.py`, `regenerate_current_rust_contract_artifact.py` and the JS SDK (`ivmArtifact.js`, `kotodamaCompiler/normalize.js`, `contractManifestNormalizer.js`, `index.d.ts`) read the 49-byte baseline header and the `ZK`/`VECTOR` feature bits again. |
| SDKs | The JS SDK (`ivmArtifact.js`, `kotodamaCompiler/normalize.js`, `contractManifestNormalizer.js`, `index.d.ts`), Swift (`ToriiClient.swift`), C# (`ToriiContractManifestJson.cs`), Kotlin (`ContractManifestModels.kt`) and Python (`ContractManifest.from_payload` in `client.py`) accept the baseline `ZK`/`VECTOR` feature bits and, where they read programs, the 49-byte header again; the JS, Python and C# fault mirrors are the anchor's `IvmFaultKindV1` and `NumericFaultV1`. |
| History guard | `specs/first_release_history_cutover.{json,md}` declare `7531b45d…`; the guard reads the hash at bytes 17..49. The C10 fixtures (the five Kotodama emission captures, whose tests select `CompilerSession::c10()`, and the `predecoder` rows of `ivm_artifacts.tsv`) carry the C10 hash: the captures stay regeneration targets without `carries`, and the artifact map names its `carrier_owners` (`default`, `kotodama-standard`, `kotodama-zk`). Phase 5 declares the C10 hash with these as its carriers. |
| Tests | C10-specific unit and integration tests in `ivm`, `kotodama_lang`, `ivm_artifact_admission` select C10 explicitly; Core tests that drive live paths build baseline fixtures (`ivm_test_support::{baseline_header, baseline_program, baseline_syscall, baseline_unit_return}`) and read fixtures under their declared profile (`declared_profile`, `parse_declared`, `prepare_declared`). Anchor tests restored where C10 had rewritten them: `register_code_rejects_zero_cycle_ceiling`, `trigger_registration_rejects_zero_and_over_ceiling_ivm_headers`, the cycle-ceiling and warm-policy overlay tests, the world and cache contract fixtures (`max_cycles` headers), the 1021-byte admission limit in `tx.rs`, Torii's subscription trigger helpers. The CoreHost harness probes, which run a local VM, stay C10 programs and bind C10's surface hash; fixtures that a live path registers or calls (the nested-dispatch caller, the private-witness callee) are baseline contracts (`build_authenticated_baseline_test_contract_program`). |

### Findings

- Both reconstruction tests pass without edits: `this_release_reconstructs_the_selection_taira_runs`
  (`f4f73c12…`) and `the_rehearsal_activates_in_a_discarded_overlay_and_changes_nothing`. No
  non-C10 difference blocks them, so nothing outside the C10 regression was restored for them.
- `runtime::ironwood::tests::only_the_taira_activation_unbinds_a_wire` computes exactly the
  anchor's change-set ids (`1e4c77b2…`, `d33f4c93…`, `9f6b930d…`). `c5511ebfc4` and `14e5a5e1f4`
  had re-pinned them to ids derived from the C10 baseline; the anchor pins are restored.
- `release_identity::wire_schema_hash` binds the ABI V1 hash, so it moves from `b2d6d37f…`
  to `9d96f872…` (not the anchor's `072924c3…`: covered wire roots changed after the anchor,
  outside C10). Taira runs `8c6e5380…`; the cutover does not make this release's frames
  readable by Taira's binary, which the decoder-rollout gate of
  `ironwood_runtime_transition.md` already requires.
- Frozen pins that C10 had re-derived from its own baseline are restored to the anchor values
  rather than re-pinned: `specs/runtime_ironwood_catalog.json` (predecessor `56c23286…`,
  selection `001a52cd…`) and the Ironwood change-set ids above.
- `execution_policy_digest_v1` is unchanged: the pinned first-release history test reports the
  same signed-genesis execution digest mismatch (`67502f37…` ≠ `ed32e924…`) as the live tree's own
  Core run before this phase. `e562d91f81` renamed
  `pipeline.quarantine_tx_max_cycles` to `quarantine_tx_max_gas` in its preimage before this
  phase; the anchor's quarantine-lane cycle budget is therefore not restored (restoring it needs
  the digest term, an owner decision); both engines keep the gas cap.
- World import (`runtime_amx_captures`, `parliament_anonymous_attempts`, `runtime_retirements`)
  is untouched; phase 4 does not change that picture.
- Core lib failures that are not this phase's, each failing identically in the live tree's own
  Core runs of 2026-11-15/16 (`artifacts/continuation-20261114/core-repair`): the seven
  `snapshot` portable-transfer tests, `sumeragi::node::portable_resume_tests::…`, the four
  `sumeragi::certified_chain` portable tests (the portable restore cannot read the native lane
  evidence source),
  `sumeragi::node::replay::check::tests::whole_store_copy_replays_from_an_authenticated_snapshot_without_touching_the_source`
  (`NativeExecutionReplayRequired`),
  `sumeragi::node::tests::history_cutover_tests::pinned_history_of_an_earlier_build_replays_to_its_pinned_state`
  (the execution-digest mismatch above),
  `smartcontracts::ivm::host::tests::runtime_obligation_host_gas_refusal_publishes_no_effect_or_return`
  (`DecodeError`), and
  `state::trigger_lifecycle::tests::lifecycle_compact_wire_identity_and_unchanged_ivm_abi`, which
  pins Taira's wire identity `8c6e5380…` (it was failing on `b2d6d37f…`; its ABI half,
  `7531b45d…`, now passes). None is re-pinned.
- `iroha_core_privacy`'s step AIR models C10 only; a baseline run that requests proof-trace
  collection is refused locally (phase 3), and consensus requests none.

### Gates

Run in the frozen copy on 2026-11-16. Every cargo step went through `scripts/cargo_fast.sh --jobs 4`
under `nice -n 5` after the shared-host resource gate: `ivm` builds and Clippy with
`--no-incremental --stable-local-metadata` (`--target-dir <copy>/target-ivm`, `<copy>/target-clippy`),
the `iroha_core` lib-test build with the default flags (`--target-dir <copy>/target`). Every test
binary ran alone from its package directory.

| Gate | Command | Result |
|---|---|---|
| Build | `test --locked --offline --no-run -p iroha_data_model --lib --test iroha_data_model_group_01 --test iroha_data_model_group_02 -p ivm_abi -p ivm -p ivm_artifact_admission -p kotodama_lang --tests` | exit 0 |
| `iroha_data_model` lib | test binary | 6223 passed, 0 failed, 145 ignored (includes `isi::registry::tests::baseline_instruction_layouts_stay_frozen` and the 545 roundtrip tests) |
| `iroha_data_model` groups | `iroha_data_model_group_01`, `_02` | 88; 214 passed (8 ignored); 0 failed |
| `ivm_abi` | lib; `axt_policy_vectors`, `call_schema_allocations`, `norito_pointer_abi_roundtrip`, `runtime_upgrade_abi_fixture` | 300; 6 (1 ignored), 3, 2, 1 passed; 0 failed |
| `ivm` lib | test binary | 1574 passed, 0 failed, 6 ignored |
| `ivm_group_01` … `ivm_group_10` | each binary (`group_01` holds `baseline_v1_differential`, `group_04` holds `repository_ivm_artifacts`) | 136, 79, 86, 152, 244, 108, 88, 129, 66, 29 passed; 0 failed |
| `ivm` custody | `poseidon_parameter_custody`, `shared_owner_custody` | 1, 9 passed; 0 failed |
| `ivm_artifact_admission` | lib; `current_artifact`, `preparation_error_custody` | 60; 16, 3 passed; 0 failed |
| `kotodama_lang` | lib; `kotodama_integration` (includes the baseline V1 corpus) | 1575 passed (6 ignored); 104 passed; 0 failed |
| Core build | `test --locked --offline --no-run -p iroha_core --lib` | exit 0 |
| Core lib | every one of the 15 853 lib tests, in three passes (each test binary alone, 8–10 threads) | An earlier binary ran the modules up to `smartcontracts::isi::staking` (7791 passed, 6 failed, 18 ignored; stopped there). A later binary ran the 6552 tests after `smartcontracts` (6512 passed, 17 failed, 23 ignored) and 2473 head tests: those not yet reported, every earlier failure and the modules edited after the first binary (2361 passed, 93 failed, 19 ignored). The final binary reran every failure plus `smartcontracts::ivm::`, `smartcontracts::isi::world::`, `runtime::taira_activation::` and `tx::tests::` (1165 tests): 1147 passed, 16 failed, 2 ignored. The 16 are the earlier failures listed under Findings. The only non-test source edited after the first two binaries is the cache `meta_hash`, which only `smartcontracts::ivm` tests read. |
| Taira reconstruction | `runtime::taira_activation::tests::` on the final binary, including `this_release_reconstructs_the_selection_taira_runs` and `the_rehearsal_activates_in_a_discarded_overlay_and_changes_nothing` | 12 of 12 passed, expectations unedited |
| Strict Clippy | `clippy --locked --offline --all-targets -p iroha_data_model -p ivm_abi -p ivm -p ivm_artifact_admission -p kotodama_lang -p kotodama_toolchain -p iroha_core -p iroha_torii -p irohad_lib -p irohad -p iroha_cli_lib -p iroha_cli -p izanami -p musubi -p iroha_js_host -p iroha_contract_deploy -p iroha_core_privacy -p integration_tests --keep-going -- -D warnings`, then `clippy --locked --offline --all-targets -p iroha_core -- -D warnings` after the last Core edits (the only crate edited since) | exit 0 both; warnings only in `experiments/kagemusha-spartan` and vendored crates |
| Format | `rustfmt --edition 2024 --check` over every changed Rust file | clean |
| Script tests | `pytest pytests/scripts/first_release_history_guard_test.py pytests/scripts/regenerate_kotodama_goldens_test.py scripts/tests/regenerate_current_rust_contract_artifact_test.py scripts/tests/kotodama_fixture_asset_source_test.py scripts/tests/regenerate_kotodama_goldens_test.py` | 426 passed, 147 subtests passed |
| History guard | `python3 scripts/check_first_release_history.py` | matches the source |
| Generated artifacts | `python3 scripts/regenerate_kotodama_goldens.py --check`; `python3 scripts/regenerate_current_rust_contract_artifact.py --check` | 58 mappings, no change; fixture verified |
| JS SDK | `node --test` over the 13 IVM/manifest/fault suites and `toriiClient.test.js`, in a scratch copy with `dist/` built by `scripts/build-dist.mjs` | 324 of 330 and 710 of 714 passed; every failure is the stale prebuilt native binding (`unknown field enum_types`, multisig occurrence constructor) |
| Kotlin SDK | `./gradlew --offline :core-jvm:test --tests 'org.hyperledger.iroha.sdk.client.ContractManifestTest'` | 23 of 25; the 2 failures need the ABI-33 native address validator |

### Not done in phase 4, and what remains

- Owner decisions, unchanged: `AbiV1RuntimeLifecycle` stays on C10 (both reconstruction tests
  pass with it there); the quarantine-lane cycle budget needs the `execution_policy_digest_v1`
  term `e562d91f81` renamed; mixed-profile nested calls; retirement and the activation
  bootstrap authority.
- Phase 5: `*C10V1` wire families for the C10 faults and admission errors that the baseline
  enums cannot name (`ControlFlowViolation`, `RegisterIntegerOverflow`, `MaxCyclesWithoutZk`;
  today `None` or the VM's text), the `iroha.runtime.ivm_c10.v1` addition, a `koto` host
  profile for C10's own ABI V1, and a declared C10 ABI hash in the first-release history
  inventory with the C10 captures and predecoder rows as its carriers.
- Test suites outside this phase's gates still build C10 programs for live paths and were not
  run: `iroha_core`'s grouped integration tests (`contract_code_bytes`,
  `contract_execution_header_binding`, `contract_manifest_triggers`,
  `executor_migration_introspect`, `ivm_admission_unknown_syscall`, `ivm_manifest_abi_reject`,
  `runtime_change_sets` and the CoreHost fixtures), `integration_tests` (`subscriptions`,
  `support/parliament_submission`), Torii and `irohad` Soracloud test fixtures. Their parse,
  load and verify calls read each fixture's declared profile; fixtures that a live path admits
  need baseline headers and encodings, as the Core lib tests now use.
- Source guards that already failed before this phase (`check_zk_resource_contract`,
  `check_zk_runtime_validity`, `check_zk_primitive_workloads`,
  `check_state_commitment_candidates`) still fail on their earlier findings; this phase adds one
  (`ivm/src/analysis/aggregate.rs` reads the baseline `POSEIDON6` operands, unlisted in
  `specs/zk_primitive_workloads.json`) and removes one (`check_zk_resource_contract` 14 → 13).
- The JS current-artifact fixture records the source closure it was generated from; after the
  change lands it is regenerated against the live tree
  (`scripts/regenerate_current_rust_contract_artifact.py --write`, then `--check`).
- SDK suites not run to completion: the Python SDK tests need the built `iroha_native`
  extension, the C# suite needs the .NET 10 SDK (the host has 8), the Swift suite needs the
  authenticated native artifact, the complete Kotlin JVM suite needs the ABI-33 bridge, and the
  JS suites ran against a stale prebuilt native binding. The Swift, C#, Kotlin and Python
  manifest feature bounds and fault mirrors are the anchor's.
- Documentation outside the files listed under As built (for example `specs/kotodama_grammar.md`,
  `specs/runtime_upgrades.md`, `crates/ivm/docs/kotodama_gap_analysis.md`) was scanned for header,
  register, syscall-encoding and budget claims but not reviewed line by line.
