# IVM Architecture Refactor Plan

This plan captures the short-term milestones for reshaping the Iroha Virtual Machine
(IVM) into clearer layers while preserving security and performance characteristics.
It focuses on isolating responsibilities, making host integrations safer, and
preparing the Kotodama language stack for extraction into a standalone crate.

## Goals

1. **Focused runtime API** – group construction and host-policy entry points while
   keeping `IVM` as the single concrete first-release engine.
2. **Host/syscall boundary  hardening** – route syscall dispatch through a
   dedicated adapter that enforces ABI policy and pointer validation before any host
   code executes.
3. **Language/tooling separation** – move Kotodama specific code to a new crate and
   keep only the bytecode execution surface in `ivm`.
4. **Configuration cohesion** – unify acceleration and feature toggles so they are
   driven through `iroha_config`, removing environment-based knobs in production
   paths.

## Phase Breakdown

### Phase 1 – Runtime API (complete)
- Group `IvmBuilder`, `IvmConfig`, acceleration policy, and stack policy in the
  `runtime` module.
- Keep lifecycle methods on the concrete `IVM`. The first release has no second
  engine, so an abstraction trait would add an unowned API without isolating any
  implementation.
- Keep syscall policy enforcement in the dispatcher boundary described below.

**Security / performance impact**: The concrete API avoids a bypass-prone or
speculative engine abstraction while the dispatcher centralizes host policy.

### Phase 2 – Syscall dispatcher
- Introduce a `SyscallDispatcher` component that wraps `IVMHost` and enforces ABI
  policy and pointer validation once, in one location.
- Migrate the default host and mock hosts to use the dispatcher, removing
  duplicated validation logic.
- Make dispatcher pluggable so hosts can supply custom instrumentation without
  bypassing safety checks.
- Provide a `SyscallDispatcher::shared(...)` helper so cloned VMs can forward
  syscalls through a shared `Arc<Mutex<..>>` host without each worker building
  bespoke wrappers.

**Security / performance impact**: Centralised gating protects against hosts that
forget to call `is_syscall_allowed`, and it allows future caching of pointer
validations for repeated syscalls.

### Phase 3 – Kotodama extraction
- Kotodama compiler extracted to `crates/kotodama_lang` (from `crates/ivm/src/kotodama`).
- Provide a minimal bytecode API that the VM consumes (`compile_to_ivm_bytecode`).

**Security / performance impact**: Decoupling lowers the attack surface of the VM
core and allows language innovation without risking interpreter regressions.

### Phase 4 – Configuration consolidation
- Thread acceleration options through `iroha_config` presets (e.g., enabling GPU backends) while keeping the existing environment overrides (`IVM_DISABLE_CUDA`, `IVM_DISABLE_METAL`) as runtime kill switches.
- Expose a `RuntimeConfig` object through the new façade so hosts select
  deterministic acceleration policies explicitly.

**Security / performance impact**: Eliminating env-based toggles avoids silent
configuration drift and ensures deterministic behaviour across deployments.

## Immediate next steps

- Finish Phase 1 by adding the façade trait and updating high-level call sites to
  depend on it.
- Audit public re-exports to ensure only the façade and deliberately public APIs
  leak out of the crate.
- Prototype the syscall dispatcher API in a separate module and migrate the
  default host once validated.

Progress on each phase will be tracked in `status.md` once the implementation is
underway.
