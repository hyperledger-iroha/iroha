# IVM baseline and C10 host profiles

The November 18 owner decision authorizes a distinct release wire identity and coordinated decoder rollout for certified C10 outputs. The candidate implements the dormant C10 catalog addition, governed registration, captured storage and result-family dispatch. Combined-source qualification is in progress. ABI V1 and the default Kotodama target continue to use Taira’s frozen baseline engine. This record
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
The owner authorized a new release wire identity for the certified C10 output family on November 18. The decoder rollout precedes activation. The installed selections, baseline ABI and gas, and Ironwood fixtures stay pinned.

In the November 18 candidate the release catalog compiles the two C10 registration interfaces, the C10 host and event definitions, and the manifest, captured profile and trigger tables. The universal root can draft this selection through the existing runtime-upgrade route. Only a governed change effective at a certified height opens it. The native registration code checks the selected host, the artifact's advertised C10 ABI, its code hash, the authenticated compiler interface and the C10 manifest signing frame. Restore authenticates the stored registration in both the current and rollback cuts.

The signed contract-artifact GET serves a registered C10 manifest when the request explicitly accepts `application/vnd.iroha.c10-manifest.v1+json`. It returns the network, data-space-scoped artifact, captured host definition and signed C10 manifest. The Rust client checks those identities and verifies the signature under the caller's budget. Original artifact reads retain their previous representation.

The phase records below describe their named snapshots. Release qualification of the combined source requires the certified execution, activation, restore, consumer-decoder and frozen release gates.


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
  collection, an active scoped register logger, native execution packets or a diagnostic step
  recorder is refused locally with `ExecutionDeferred(TraceOwnerUnavailable)`. The corpus covers
  none of them; consensus execution requests none of them. (`TraceMode::DeltaRegisters` was
  refused too until phase 5; it now records the baseline's 256-register geometry, see the Phase 5
  record.)
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
  outside C10). Taira runs `072924c3…` (`a34b1e7a60`); the cutover does not make this release's frames
  readable by Taira's binary, which the decoder-rollout gate of
  `ironwood_runtime_transition.md` already requires.
- Frozen pins that C10 had re-derived from its own baseline are restored to the anchor values
  rather than re-pinned: `specs/runtime_ironwood_catalog.json` (predecessor `56c23286…`,
  selection `001a52cd…`) and the Ironwood change-set ids above.
- `execution_policy_digest_v1` keeps its signed V1 preimage (owner decision 2026-11-16: keep
  Taira's digest). `e562d91f81` had renamed the quarantine slot
  `pipeline.quarantine_tx_max_cycles` to `pipeline.quarantine_tx_max_gas`, which moved every
  signed genesis's execution digest (`67502f37…` ≠ `ed32e924…` in the pinned first-release
  history test, and Taira's genesis refused at `--check-config`). The slot is frozen under its
  V1 name again (`push_frozen_quarantine_tx_budget_v1`) and binds the gas cap's u64 value, so
  a signed genesis keeps its digest. The anchor's quarantine-lane cycle budget is not restored;
  both engines keep the gas cap. Restoring the anchor's aggregate cycle budget is a forward
  release addition: a new typed parameter definition and selected execution-services
  implementation that reinstates the source-bound cycle owner without changing the digest.
- World import (`runtime_amx_captures`, `parliament_anonymous_attempts`, `runtime_retirements`)
  is untouched; phase 4 does not change that picture.
- The Core lib failures that were not this phase's now pass on `optimizations` (2026-11-17):
  the seven `snapshot` portable-transfer tests, `sumeragi::node::portable_resume_tests::…`,
  the four `sumeragi::certified_chain` portable tests and the whole-store-copy replay check
  (portable checkpoint stores, `e3c0409106`);
  `history_cutover_tests::pinned_history_of_an_earlier_build_replays_to_its_pinned_state`
  (the original fixture replays with the frozen quarantine slot, `5f99f31cff`, `d57abe2b78`);
  `smartcontracts::ivm::host::tests::runtime_obligation_host_gas_refusal_publishes_no_effect_or_return`;
  and `state::trigger_lifecycle::tests::lifecycle_compact_wire_identity_and_unchanged_ivm_abi`,
  which pins the release wire identity `9d96f872…` and ABI V1 `7531b45d…`, the identities byte
  accounting pins.
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
  pass with it there); mixed-profile nested calls; retirement and the activation bootstrap
  authority. (The quarantine-lane cycle budget is a forward addition, not a digest term: see the
  corrected finding above.)
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

## Phase 5 record (2026-11-17)

The November 17 candidate contained the phase-4 test follow-through, profile threading,
C10 result families and projections, mixed-profile call refusal, the `koto` C10 target and
its declared ABI hash. At that revision the catalog omitted `IvmProfileId::C10_ABI_V1`,
so governance could select only the earlier host profiles. The November 18 integration
below records publication of the dormant addition and its governed activation tests.

### As built

| Area | Files | Change |
|---|---|---|
| Phase-4 follow-through | `iroha_core/tests/{contract_code_bytes, contract_execution_header_binding, contract_manifest_triggers, executor_migration_introspect, ivm_admission_unknown_syscall, ivm_codec_helpers, ivm_event_ordering, ivm_host_mapping, ivm_host_shadow_execute, ivm_manifest_abi_reject, ivm_pointer_abi_apply, ivm_syscall_policy, runtime_change_sets}.rs`; `integration_tests/tests/{subscriptions, support/parliament_submission}.rs`; `iroha_torii/src/{test_utils, routing}.rs` (validation-fee ingress tests); `irohad/src/soracloud_runtime/tests/{part_01, part_04}.rs` | Every fixture a live ABI V1 path admits (stateless admission, raw `Executable::Ivm`, contract registration, triggers, the executor upgrade, the Soracloud runtime) is a baseline artifact: the 49-byte header with a positive `max_cycles`, the baseline encodings, the register form of `JALR`, the 8-bit `SCALL` (24-bit `SYSTEM` above `0xFF`). Only fixture construction changed; no expectation. Raw-code tests that drive a C10 VM directly (`IVM::load_code`) stay C10. Soracloud fixtures use the canonical baseline opcodes and encoding helpers re-exported by `ivm::baseline`; the duplicate test-only encoder is deleted. |
| Warm and cold caches agree | `iroha_core/src/smartcontracts/ivm/cache.rs`, `state.rs`, `smartcontracts/ivm/host.rs` | `PreparedContractCache::get_in_world` and `IvmCache::cached_program_summary_in_world` no longer answer a root that does not select the resident entry's profile with that entry's ABI hash mismatch: they report a miss, so the caller prepares the stored bytes under the profile the root resolves and returns exactly the refusal a cold cache returns. Before, a warm node reported `ArtifactAbiHashMismatch` where a cold node's baseline parser reported, for example, `UnsupportedAbiVersion`: two different admission results for the same transaction depending on node-local cache state. Tested warm against cold for both lookups. |
| Profile threading | `iroha_core/src/smartcontracts/ivm.rs`, `ivm/{cache, profile, host_profile, host}.rs`, `ivm/cache/profile_key_tests.rs`, `executor.rs`, `executor_contract_dispatch_tests.rs`, `state.rs`, `pipeline/overlay.rs`, `smartcontracts/isi/{world, triggers/mod}.rs`; `iroha_torii/src/{routing, gov}.rs`; `iroha_cli/src/contracts.rs`; `ivm/src/{lib, contract_artifact}.rs` | No reader collapses a resolved, prepared or VM profile to its `SyscallPolicy` any more (C10's own ABI V1 surface shares `AbiV1` with the baseline). `artifact_profile` (returns `IvmProfileId`) replaces `artifact_policy`; `verify_contract_in_world` verifies under the resolved profile; the prepared-contract and summary caches take profiles (`*_for_profile`, `require_prepared_profile` compares the exact profile and reports the two profiles' ABI hashes); `host_profile::{ensure_at_height, ensure_in_transaction}` and `ensure_host_profile` check the exact profile, so a C10 ABI V1 object is not admitted as the implicit baseline; trigger registration, the stateless raw-IVM overlay, the executor upgrade, raw contract dispatch and trigger callback validation parse and load under the profile; Torii's contract-state schema, multisig contract-call and governed-contract readers and the CLI's program summary use the resolved or declared profile. `ivm` re-exports `verify_contract_artifact{,_with_memory_budget}_for_profile`. The interface is still bound before any header byte is parsed (`profile::resolve_artifact`). |
| C10 result families | `iroha_data_model/src/ivm_c10.rs` (new module `ivm_c10`) | New definitions with their own nominal schema names; every existing type, tag and layout is byte-identical. `NumericFaultC10V1` (the baseline tags 1–13 plus `RegisterIntegerOverflow` 14), `IvmFaultKindC10V1` (C10's tags 0–27, `ControlFlowViolation` 27, exactly the `fault_layout` C10's descriptor binds), `IvmFaultC10V1`, `IvmAdmissionErrorC10V1` (C10's tags 0–16, `MaxCyclesWithoutZk` 6), and the rejection parents `ValidationFailC10V1`, `TransactionRejectionReasonC10V1`, `TransactionStatusC10V1`. Unchanged leaves are shared. Framed and bare Norito roundtrips, JSON roundtrips and tag pins for each. |
| Projections | `ivm_abi/src/profile.rs` (`c10_wire_fault_kind`), `iroha_core/src/smartcontracts/ivm/projection.rs` (`c10_admission_error_v1`) | The complete C10 projections onto the C10 families: no C10 fault returns `None` or text (`ControlFlowViolation`, `RegisterIntegerOverflow`, `MaxCyclesWithoutZk` are structured). Tested against the independent projection table of every `VMError` variant. Not yet called by dispatch (TODO). |
| Mixed-profile calls | `iroha_core/src/smartcontracts/ivm/host/contract_calls.rs`, `host/nested_profile_tests.rs` | `CALL_CONTRACT` compares the exact `IvmProfileId` of the authenticated prepared callee with the caller VM's, immediately after callee preparation and the cache-independent boundary charge, before argument capture, runtime checkout, attachments, host-context mutation or child execution, and refuses a mismatch with a metered `VMError::PermissionDenied` that the caller's engine projects onto its own `PermissionDenied` fault. Same-profile calls are unchanged. Before this change a mixed call ran: Core installs no shared cycle allowance on any root run (`run_with_host_and_cycle_budget` has no Core caller), so the `HostUnavailable` refusal of a C10 child under an active baseline allowance was unreachable, and a baseline contract could call a runtime-lifecycle contract and the reverse (see Findings). Tests: baseline caller with a runtime-lifecycle (C10) callee, the reverse, both with a cold and a warm cache (identical metered refusal, no State, no return value), and same-profile calls of both engines. |
| `koto` | `kotodama_toolchain/src/bin/koto.rs`, `kotodama_toolchain/tests/cli_smoke.rs` | `--host-profile iroha.ivm.c10.v1` builds C10's own ABI V1 surface (`IvmProfileId::C10_ABI_V1`); `abi-v1` (the baseline) stays the default. The name-to-profile map is `host_profile_target`; a unit test builds and verifies each of the three targets. Three `koto` tests that phase 4's default flip broke now select their engine: the C10 header assertions of `koto_build_meta_header_smoke` build with `--host-profile iroha.ivm.c10.v1` (and the default baseline build is checked for its 1 000 000-cycle header), `offline_upgrade_verification_accepts_only_exact_compiled_host_profiles` builds each policy for the engine that implements it and corrupts the hash at that engine's header position, and `koto_build_named_runtime_profile_smoke` again asserts that C10's ABI V1 verifier refuses the lifecycle artifact (phase 4 had turned it into a self-contradicting pair of assertions). |
| History inventory | `specs/first_release_history_cutover.{json,md}`, `scripts/check_first_release_history.py`, `pytests/scripts/first_release_history_guard_test.py`, `ivm_abi/tests/runtime_upgrade_abi_fixture.rs` | `declared_profile_abi_hashes` declares C10's hash `bdd5188f…` (golden `C10_ABI_V1_HASH_GOLDEN`, checked against `IvmProfileId::C10_ABI_V1.abi_hash()`) with its carriers: the five Kotodama emission captures (`ivm_hex_rows`) and the `predecoder` rows of `scripts/ivm_artifacts.tsv`. The guard reads a profile's hash at its engine's header position (16..48 for C10), requires every artifact-map row to carry exactly one declared hash, and refuses a profile hash in the pinned history. |
| Pre-existing Clippy failure | `iroha_core/src/block.rs` | Upstream `e3c0409106` left an orphaned doc line and a second `#[expect(clippy::result_large_err)]` above `parent_creation_time`; the unfulfilled expectation failed strict Clippy for `iroha_core` and every crate that depends on it. Both removed (attributes and documentation only; no code changed). |
| Documentation | `crates/ivm/docs/syscalls.md`, `crates/ivm/README.md`, this record | The mixed-profile refusal, the `koto` target and the profile status; the phase-4 quarantine finding is corrected above. |

### Findings

- C10's ABI descriptor (`compute_c10_abi_hash`, `bdd5188f…`) binds its fault frame by the
  nominal name `IvmFaultV1` (`frame_hash` hashes the name) with C10's tag layout text. The new
  wire family has its own name, `IvmFaultC10V1`; the descriptor is unchanged here, because
  renaming its frame changes C10's hash, which every C10 artifact and capture carries and which
  the runtime-lifecycle descriptor embeds. This must be decided before the addition publishes
  C10's host profile.
- `ValidationFail::InternalError` carries `#[codec(skip)]`, which Norito does not read: the
  internal error text is encoded, contrary to its doc comment. The frozen type is unchanged; the
  C10 family mirrors the actual encoding (a test pins the identical bare bytes).
- The runtime-lifecycle profile runs C10 but still reports through the baseline families
  (`IvmEngine::fault_kind`, `projection::admission_error`), where `ControlFlowViolation`,
  `RegisterIntegerOverflow` and `MaxCyclesWithoutZk` have no structured value. Moving it to the
  C10 families changes the result contract of a published (dormant) definition.
- In this release the only C10 profile a root can select is the runtime-lifecycle profile, so the
  mixed-call refusal applies today to calls between baseline and lifecycle contracts, and those
  calls ran before it: the brief's premise that a C10 child under a baseline parent returns
  `HostUnavailable` holds only under an active shared cycle allowance, which Core never installs.
  The refusal is therefore a behaviour change for a root that selects the runtime-lifecycle
  profile (no installed selection does). Relatedly, the anchor's shared nested cycle allowance is
  restored in the engine but not wired by Core: baseline nested calls draw only on gas. Restoring
  it belongs with the quarantine cycle-budget addition (phase-4 correction above).
- A warm and a cold node could reject the same transaction with different admission errors
  when a cached artifact's profile was not selected by the executing root (phase 2's
  `require_prepared_policy` fallback). Fixed here (As built, "Warm and cold caches agree"); it was
  reachable only with the runtime-lifecycle profile selected in one root and not in another.
- Local `koto test trace` records the selected engine's register deltas, including all 256
  baseline registers. Baseline proof traces, native packets and diagnostic step records
  remain unimplemented.
- The guard's artifact-map rule is stricter: a row the ABI V1 carrier leaves out must carry a
  declared profile hash. `test_artifact_map_carrier_owners_exclude_other_engine_rows` now
  declares the other engine's profile instead of leaving its row undeclared.
- Step 0 behaviour notes: in `contract_execution_header_binding` the `max_cycles` mutation no
  longer fails header admission (the baseline admits a cycle bound without the ZK bit) and is
  refused later by the manifest signature check; the mutation list does not cover the baseline
  header's vector-length byte (offset 7), so the test's "every fixed-header field" claim has a
  gap (left for review: closing it changes the asserted mutation count).

### Owner decisions flagged

- The successor result/event contract and its kernel and commitment scope: the C10 families stop
  at `TransactionStatusC10V1`. Pipeline events, event filters, trigger descriptors, manifests and
  certified execution outputs (`TransactionResult` in block execution outputs) of C10 executions
  need new enclosing definitions whose commitment scope is the owner's to approve. Decided
  2026-11-17: C10 executions get new enclosing definitions for these, effective only where a
  governed change set activates `iroha.runtime.ivm_c10.v1`. The November 18 owner decision authorizes the distinct certified-output wire identity and coordinated decoder rollout.
- Whether C10's descriptor names its own fault frame (`IvmFaultC10V1`), which changes `bdd5188f…`.
- Whether the runtime-lifecycle profile moves to the C10 result families.
- Whether the mixed-call refusal also covers baseline and runtime-lifecycle contracts, which
  could call each other before this change (it does here, following settled default 3).
- Unchanged: permitting mixed-profile calls (refused by default here), bundled lifecycle and C10
  activation, the activation-bootstrap authority, baseline retirement.

### Gates

Run in the frozen copy `r-phase5` on 2026-11-17 (base: `4e121e77e9` with the merge-reconcile
patch). Every cargo step went through `artifacts/continuation-20261114/r-phase5/cargo_step.sh`
(`scripts/cargo_fast.sh --jobs 4` under `nice -n 5` after the shared-host resource gate): Core
builds with the default flags in `<copy>/target`, `ivm`-family builds and Clippy with
`--no-incremental --stable-local-metadata` in `<copy>/target-ivm` and `<copy>/target-clippy`.
Test binaries ran from their package directories. Logs: `artifacts/continuation-20261114/r-phase5/runs/`.

| Gate | Command | Result |
|---|---|---|
| Step 0, before | `test --locked --offline --no-run -p iroha_core --features iroha-core-tests --test iroha_core_group_01 --test iroha_core_group_03 --test iroha_core_group_04`, each binary `--test-threads 6` | 61/16, 117/17 (2 ignored), 62/1 passed/failed: the 34 failures were C10 fixtures on live ABI V1 paths (`runs/s0-failures.txt`) |
| Step 0, after | same, after the fixture conversion (build before the warm/cold cache fix and the `block.rs` attribute fix) | 77/0, 134/0 (2 ignored), 63/0 |
| Core lib, final binary | `test --locked --offline --no-run -p iroha_data_model -p ivm_abi -p iroha_core --lib`; `iroha_core-895d13dd3bd3f1df` (sha256 `e7490fa8…`), all 15 868 tests, `--test-threads 8` | 15 823 passed, 3 failed, 42 ignored (8 272 s). Failures: `lifecycle_compact_wire_identity_and_unchanged_ivm_abi` and `pinned_history_of_an_earlier_build_replays_to_its_pinned_state` (both known, unchanged) and `state::world_journals::tests::publication_tests::detached_world_activates_original_prepaid_control_on_publication_thread`, which passed alone twice on the same binary (a shared allocation-budget count under parallel load). The 16 portable-transfer and replay failures recorded in phase 4 pass. The binary was linked before the attribute-only `block.rs` fix. |
| Core focused (earlier binary) | same build before the warm/cold cache fix: `smartcontracts::ivm::`, `smartcontracts::isi::triggers`, `executor::`, `pipeline::overlay`, `runtime::taira_activation`, `runtime::ironwood`, `state::trigger_lifecycle`, `history_cutover_tests`, `snapshot_accounting_keeps_release_wire_and_abi_pins`, `smartcontracts::code::` | 1 744 passed, 2 failed (the two known), 7 ignored; `runtime::taira_activation` 12/12 with both reconstruction tests, `PINNED_CHANGE_SET_IDS` and the release wire and ABI pins (`9d96f872…`, `7531b45d…`) pass |
| `iroha_data_model` | `test --locked --offline --no-fail-fast -p iroha_data_model -p ivm_abi -p ivm -p ivm_artifact_admission -p kotodama_lang -p kotodama_toolchain --tests` (target-ivm) | lib 6 233 passed, 145 ignored (includes `baseline_instruction_layouts_stay_frozen`, every roundtrip and the 7 `ivm_c10` tests); groups 88 and 214 (8 ignored); the other 20 test binaries pass |
| `ivm_abi` | same run | lib 301; `axt_policy_vectors` 6 (1 ignored), `call_schema_allocations` 3, `norito_pointer_abi_roundtrip` 2, `runtime_upgrade_abi_fixture` 2 (with the C10 golden); 0 failed |
| `ivm` | same run | lib 1 574 (6 ignored); `ivm_group_01` … `_10`: 136, 79, 86, 152, 244, 108, 88, 129, 66, 29; custody 1, 9; 0 failed |
| `ivm_artifact_admission` | same run | lib 60, `current_artifact` 16, `preparation_error_custody` 3; 0 failed |
| `kotodama_lang` | same run | lib 1 575 (6 ignored), `kotodama_integration` 104; 0 failed |
| `kotodama_toolchain` | same run | lib 140/1, `koto` 39/1 (the new `host_profile_names_select_their_compiled_profiles` passes), `cli_smoke` 12/2. The library failure exposed the missing baseline delta trace, fixed in the Phase 5 continuation. The three fixture fixes later passed strict Clippy. The shared host's free-disk gate prevented a test rerun at the time. |
| Torii, `irohad`, CLI (earlier binaries) | `test --locked --offline --no-run -p iroha_torii -p irohad_lib -p iroha_cli_lib --lib` before the warm/cold cache fix; filters below | `irohad` `soracloud_runtime` 287 (1 ignored), CLI `contracts::` 58, Torii `validation_fee contract gov test_utils multisig` 637; 0 failed |
| Strict Clippy | `clippy --locked --offline --all-targets -p iroha_data_model -p ivm_abi -p ivm -p ivm_artifact_admission -p kotodama_lang -p kotodama_toolchain -p iroha_core -p iroha_torii -p irohad_lib -p iroha_cli_lib -p integration_tests --keep-going -- -D warnings`; then `clippy --locked --offline --all-targets -p iroha_core -- -D warnings` | the first pass stopped on the pre-existing `block.rs` expectation; after its fix both exit 0 (warnings only in vendored crates and `zk-spartan`) |
| Format | `rustfmt --edition 2024 --check` over the 45 changed Rust files | clean; the only reported diff is the unchanged, pre-existing `iroha_data_model/src/sumeragi_finality.rs`, reached through `lib.rs` |
| Script tests | `pytest -q -p no:cacheprovider -n 4 pytests/scripts/first_release_history_guard_test.py pytests/scripts/regenerate_kotodama_goldens_test.py scripts/tests/regenerate_current_rust_contract_artifact_test.py scripts/tests/kotodama_fixture_asset_source_test.py scripts/tests/regenerate_kotodama_goldens_test.py` | 441 passed, 147 subtests passed |
| History guard | `python3 scripts/check_first_release_history.py` | matches the source |
| Kotodama goldens | `python3 scripts/regenerate_kotodama_goldens.py --check --koto <target-ivm>/debug/koto --admission-tool <target-ivm>/debug/ivm_artifact_admit` | 58 mappings, no change. `koto` was built from the copy (its non-test source is final); `ivm_artifact_admit` is the phase-4 build inherited by the target, because the free-disk gate refused its rebuild (phase 5 changes no admission behaviour; only an unused function was deleted) |
| JS current artifact | `python3 scripts/regenerate_current_rust_contract_artifact.py --check --koto … --cache-root <owner-only>` | the artifact and its semantics are unchanged; only `source_provenance` differs (closure `c2d9e594…` over 4 588 files → `53cbd738…` over 4 593: the copy's added files). Not rewritten in the copy, because the closure is the landed tree's: regenerate after landing |
| After hand-back, final copy (2026-11-17 05:38–06:10) | `test --locked --offline --no-run -p iroha_core --features iroha-core-tests --test iroha_core_group_01` … `_05`; `test --locked --offline --no-run -p iroha_torii -p irohad_lib -p iroha_cli_lib --lib`; `test --locked --offline --no-run -p kotodama_toolchain` | Core groups 01–05: 77, 26, 134 (2 ignored), 63, 5, all 0 failed; `irohad` `soracloud_runtime` 287/0, CLI `contracts::` 58/0, Torii focused set 637/0; `kotodama_toolchain` lib 140/1 (the `--trace` regression), `koto` 40/0, `cli_smoke` 14/0 |
| After landing, live tree | the JS current artifact regenerated with `--write` from the final copy's `koto`, then `--check`; `regenerate_kotodama_goldens.py --check` with tools rebuilt from the final copy; `check_first_release_history.py` | fixture verified (only `source_provenance` changed: closure `53cbd738…`, 4 593 files); 58 mappings, no change; history guard matches |

### Continuation (2026-11-17, frozen copy `r-phase5b`)

Base: `68d313b18a`, which merged upstream into the phase-5 subset (`a8647cfd91`). The merge
needed no fixture repair: the step-0 gates below pass on the merged tree as it was copied.

#### As built

| Area | Files | Change |
|---|---|---|
| `koto test trace` on baseline builds | `ivm/src/zk/delta_rows.rs`, `ivm/src/ivm/trace_logging.rs`, `ivm/src/ivm/trace_logging/change_counts/baseline.rs`, `ivm/src/ivm/baseline.rs`, `ivm/src/proof_coverage/traps.rs` | Cause: phase 3 refused `TraceMode::DeltaRegisters` in the baseline run loop, because the delta log, its first-row image and its public destination spans had C10's 32-register geometry; phase 4 made the baseline the default Kotodama target, so every `koto test trace` of a default build deferred with `TraceOwnerUnavailable`. The delta log now records the geometry of the engine that runs. Baseline observations take the anchor's 256-register image (`prepare_baseline_batch`, `record_baseline_reserved`; the first observation of a geometry records the complete image, later rows only changes) and the anchor's change-count table (`change_counts::baseline`, verbatim against the baseline decoder: vector spans follow the current logical vector length, `SCALL`/`SYSTEM` admit 255). The invocation token carries the engine; a baseline completed-cycle proof trace is refused there as well as in `run_baseline`. C10's arms are the previous code, so its rows, spans and hashes are unchanged. Tracing only observes: under `Off`, `PcOnly` and `DeltaRegisters` a baseline run produces the same registers, tags, register root, gas, cycles and PC (tested). Register authentication logs, step recorders and native packets stay refused for the baseline. Nothing falls back to C10. |
| C10 public parents | `iroha_data_model/src/ivm_c10.rs`, `ivm_c10/{events, manifest, trigger, result}.rs` | New definitions, each under its own nominal schema identity and keeping the baseline parent's members, order and tags with the C10 member in place of the changed one: `TransactionEventC10V1`, `PipelineEventBoxC10V1`, `EventBoxC10V1` (pipeline events); `TransactionEventFilterC10V1` (with `matches`), `PipelineEventFilterBoxC10V1`, `EventFilterBoxC10V1` (event filters); `TriggerDescriptorC10V1`, `EntrypointDescriptorC10V1`, `ContractManifestC10V1` (manifests; their closure reaches a status through trigger filters); `ActionC10V1`, `TriggerC10V1` (the baseline `Action` invariants, enforced by the constructor and by decoding with the baseline decoder's rules and errors); `TransactionResultC10V1` (the certified result, with the baseline result's JSON object form); `RegisterSmartContractCodeC10V1` (`iroha.smart_contract.code.c10.register.v1`) and `RegisterTriggerC10V1` (`iroha.trigger.c10.register.v1`, member `object` as in `Register<Trigger>`), with the baseline registrations' canonical field decoders. `TransactionRejectionReasonC10V1` and `TransactionStatusC10V1` gain the base64 JSON form of their baseline counterparts. Unchanged leaves (block, warning and witness events, every other filter, data events, manifest tables, `Executable`, `Repeats`) are the baseline's types. No existing type, tag, layout or wire ID changed, and nothing new is reachable from a covered wire root, so the release wire identity is unchanged. Tests: framed, bare and JSON roundtrips of every family, layout parity with the baseline parent wherever the C10 member is absent or unchanged, filter matching, the action invariants and decode refusal, the registrations' wire IDs (new and not yet registered), and distinct schema identities. `ExecuteIvmC10V1` stays deferred; raw `Executable::Ivm` stays baseline-bound. |

#### Findings

- The baseline execution summary (`SYSCALL_EXECUTION_SUMMARY`) of a traced run hashes the delta
  log and counts its rows in the syscall's price, as at the anchor and as C10 does. Only hosts
  that enable tracing (`koto test --trace`, `koto test trace`, or local debugging) see this.
  Consensus execution never traces.
- A delta row of a baseline program names registers 0–255, a C10 row 0–31. `koto`'s reports keep
  each program's capture apart (test projection and runtime artifact), and a combined runtime
  capture joins calls into one artifact, so no report mixes geometries; a consumer that combines
  captures of different engines has to keep that distinction itself.
- **The retained phase 5b build stopped at certified C10 execution outputs.** A
  C10 transaction's certified outcome is `TransactionResultC10V1`, which a block must carry in its
  execution outputs (`ExecutionOutputV1` and its `*ExecutionOutputV*` rows hold the baseline
  `TransactionResult`). Every type a block reaches is part of the release wire identity
  (`release_identity::wire_schema_hash` hashes the closed schema of `SignedBlock`, the consensus
  messages and the execution result), so appending C10 output rows, as the trigger-lifecycle
  successor appended its `*WithLifecycle` rows, changes the release wire identity, which the
  original brief required the addition to preserve (`9d96f872…`, pinned by
  `lifecycle_compact_wire_identity_and_unchanged_ivm_abi` and
  `snapshot_accounting_keeps_release_wire_and_abi_pins`). Carrying C10 bytes inside an existing
  baseline field would be a second decoder for one field. Without certified C10 outputs, an
  activated C10 execution could only report through the baseline families, contrary to the
  2026-11-17 contract. The addition, its storage, dispatch and activation tests (steps 3–6) are
  therefore stopped at their data-model prerequisites on the retained copy.
- `TriggerC10V1` compares structurally; the baseline `Trigger` compares, orders and hashes by its
  id (`IdEqOrdHash`) for its storage maps. The C10 trigger storage decides its key when it is
  built.
- `ContractManifestC10V1` carries the baseline manifest's fields; its provenance signature payload
  and admission checks (entrypoint, event and permission tables) belong to the C10 registration
  handler.

#### November 18 owner decision

- On November 18 the owner authorized the new release wire identity for certified C10 outputs and a coordinated decoder rollout. The decoder-readiness gate in `ironwood_runtime_transition.md` applies before activation.
- Unchanged and kept as built: C10's descriptor names its fault frame `IvmFaultV1`
  (`bdd5188f…`); the runtime-lifecycle profile keeps its result types and its C10 engine
  binding; the mixed-profile nested-call refusal, including baseline and runtime-lifecycle
  contracts; bundled lifecycle and C10 activation, the activation-bootstrap authority and
  baseline retirement.

#### Gates

The retained copy is based on `68d313b18a`. A receipt applies to that base plus the
phase 5b files in the copy. The gate logs are under
`artifacts/continuation-20261114/r-phase5b/runs/`.

| Gate | Retained result |
|---|---|
| Data model, IVM, ABI, admission, Kotodama compiler and toolchain | 11,798 passed, 178 ignored across 50 executables. This includes the 141 toolchain library tests (the baseline trace regression passes), 40 `koto` tests and 14 CLI smoke tests. |
| Core integration groups 01–05 | 305 passed and two ignored. |
| Full Core library | 15,822 passed, four failed and 42 ignored in 10,832.39 s. Three State inventory tests independently reported the same two unclassified C10 wire names. The tiered snapshot worker exceeded its deadline and passed in 10.31 s alone on the same executable. The pinned-history replay passed. |
| Frozen wire and artifact checks | First-release history, the Parliament source check, strict Clippy on the two changed crates and the 58 Kotodama golden mappings passed. The JavaScript artifact comparison differed only in the source-closure digest and file count. |
| Service tests | The gate stopped before compilation at its disk threshold. The November 18 recovery log tracks the service rerun separately. |

### Acceptance items at the retained phase 5 cutoff

- Registering the two C10 registrations as instructions. A tolerant wire ID must be claimed by a
  catalog addition (`exactly_the_added_instructions_decode_tolerantly`), so they are registered
  (`built_in_wire_id!(.. tolerant)`, `seal::Instruction`) with the addition. `ExecuteIvmC10V1`
  stays deferred; raw `Executable::Ivm` stays baseline-bound.
- Step 3, storage: new storage definitions (World tables) for C10 manifests, C10 triggers and
  their profile bindings; the baseline `ContractManifest` storage is not widened.
- Step 4, selected dispatch: admission, execution, trigger scheduling and completion, metering,
  queries, event projection (`EventBoxC10V1`, matching `EventFilterBoxC10V1` against C10 events
  and the baseline events a C10 trigger observes), execution outputs, snapshots and replay by the
  selected or captured profile, through `ivm_abi::profile::c10_wire_fault_kind` and
  `projection::c10_admission_error_v1`. The baseline interfaces must keep resolving only their own
  profiles once C10's surface is selectable (`profile::resolve_artifact` iterates
  `IvmProfileId::IMPLEMENTED`). Host-syscall metering is keyed by the named policy
  (`require_host_syscall_metering_spec`); if C10's schedule prices a host call differently from
  the baseline's, that key must become the profile. Blocked on the owner decision above for the
  certified outputs.
- Step 5, the independent addition `iroha.runtime.ivm_c10.v1` in `runtime/catalog/additions.rs`
  and `catalog.rs`: the C10 host profile (`abi_version = 1`, `bdd5188f…`, gas schedule
  `46aa31d8…`; `C10_ABI_V1` joins `IvmProfileId::IMPLEMENTED`), its interfaces, implementation,
  storage, services and result-family definitions, explicit dependencies outside the ordered
  chain (like `CONTRACT_LIFECYCLE`) and a draft that activates its complete closure through the
  existing route (`iroha ops runtime upgrade draft --addition iroha.runtime.ivm_c10.v1`,
  `POST /v1/runtime/upgrades/draft`).
- Step 6: activation in a test-only governed change set, typed refusal of C10 registrations and
  artifacts before activation, a C10 contract registered and executed after activation reporting
  C10 results and events, baseline contracts unchanged, the mixed-call refusal, and the assertion
  that publishing the addition leaves the baseline, the Time, Ironwood and repaired selections,
  `PINNED_CHANGE_SET_IDS`, the frozen layouts, every `taira_activation` expectation, the ABI
  (`7531b45d…`), gas (`af5a27f5…`) and host-profile (`d50c38a8…`) goldens and the release wire
  identity unchanged under the original brief. The November 18 decision supersedes the release-wire constraint.
- Integration: the `integration_tests` targets that use the converted phase-5 fixtures
  (`events_and_triggers`' subscriptions, the Parliament smoke and committee-transition tests)
  pass strict Clippy but were neither built as test binaries nor run on a network.
- After landing: regenerate the JS current-artifact fixture against the live tree
  (`scripts/regenerate_current_rust_contract_artifact.py --write`, then `--check`).


### November 18 integration and qualification

The release catalog publishes `iroha.runtime.ivm_c10.v1` as a dormant addition. Its draft
binds the C10 host, registration interfaces, storage declarations, event family and certified
result service in one successor selection. A governed boundary can install this closure.
The installed baseline still selects its original host. Raw IVM transactions continue to use
their original interface. Explicit registration and deployed execution resolve the separate C10
ABI and check its selected definition and restrictions.

The approved certified block schema has wire identity
`f06b6d0e1ecc75fb87e31314c24061cc74f85405a19346068a05642aee322d3f`.
A C10 output carries its own typed result. Native proof consumers bind an observation to the
signed input and its certified output position. The coordinated rollout includes account-signed
C10 manifest reads, negotiated C10 event delivery and the mixed-output proof fixture in
`fixtures/sumeragi/native-finality/c10-mixed`. Decoder acceptance and governance decisions
follow the [release transition record](../ironwood_runtime_transition.md).

The certified Core tests in `runtime/catalog/ivm_c10/tests.rs` stage a scheduled fixture
record with all four test validators' support for the exact change. The ordinary executor
applies it at the certified boundary. Signed transactions then register, deploy and invoke a
C10 manifest, dispatch direct and deployed callbacks, reject malformed C10 metadata and
preserve the baseline result of an ordinary call. A World-only history restarts by replaying
the original certified suffix. With the complete-State commitment selected through its two
governed transitions, a successor authenticates the captured C10 manifests, host bindings
and trigger repeats on snapshot restore. A tip snapshot without a certified successor is
refused. Network governance and bootstrap authorization remain with each root's owners.

On signed `c4bbbe38602f5499d0d9bcd7973b4357ebb34255`, the 18 C10 Core cases passed in
27.469 seconds. The same library executable passed the four retained Core failures in
26.169 seconds and the frozen selection, Ironwood, wire and State inventory checks in
32.347 seconds (41 passed, two ignored). A separate data-model suite exercises the
binary and JSON round trips for C10 faults, admission errors, result rows, events and
filters in `ivm_c10.rs` and `ivm_c10/family_tests.rs`.

The CLI accepts `koto test --trace` and `koto test trace`. The
`trace_flag_and_subcommand_share_the_selected_baseline_execution` smoke test checks
both forms. The November 18 coffee fixture yielded the same 71 rows with both spellings.
The full Core, services, strict workspace and decoder release gates have separate
revision-specific receipts in `artifacts/continuation-20261114/qualification-20261118.md`.

### Node-local VM error ownership

The Rust engine returns `VMError::Trap(VmTrap)` for deterministic execution and admission
failures. A metered trap owns another `VmTrap` and its deterministic gas. An unfinished
invocation returns `VMError::Deferred(VmDeferral)`. The deferral owns the original allocation,
decoder, verifier or host cause and carries no charge. Core moves that cause through the
disposable transaction and keeps its local phase when present. The proof coverage inventory
records the owner family for every leaf. Baseline and C10 each project their deterministic
failures onto their governed result definitions.
