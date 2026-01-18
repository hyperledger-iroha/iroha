# Kotodama Gap Analysis

This document compares the current Kotodama syntax/grammar documentation with the implementation in `crates/ivm/src/kotodama` and outlines where the implementation should go to meet the design goals of clarity, safety, and ease of use.

Paths for reference:
- Lexer: `crates/ivm/src/kotodama/lexer.rs`
- AST: `crates/ivm/src/kotodama/ast.rs`
- Parser: `crates/ivm/src/kotodama/parser.rs`
- Semantic/type check: `crates/ivm/src/kotodama/semantic.rs`
- IR + lowering: `crates/ivm/src/kotodama/ir.rs`
- Codegen: `crates/ivm/src/kotodama/compiler.rs`
- Samples: `crates/ivm/src/kotodama/samples/*.ko`

## Summary
- The current implementation parses and lowers both free and contract functions (including `seiyaku`, `kotoage`, `hajimari`, and `kaizen` items), performs type checking for ints/bools/strings/pointer-ABI handles/structs/maps, and emits full multi-function IVM bytecode with durable `state` overlays when ABI v1 is selected. ✔
- Metadata and manifest wiring now surface `meta { features: ["zk","simd"] }` toggles plus per-entrypoint permission/read/write hints. Static ISI keys (including literal `create_trigger` specs), literal map keys (including hashed pointer keys), dynamic map paths (map-level `state:<name>` conflicts), explicit `#[access(read=..., write=...)]` annotations, and literal `execute_instruction` payloads plus `execute_query` payloads for supported queries (InstructionBox/QueryRequest decode, currently `FindAssetById`) are included in access hints; non-literal trigger specs and opaque host access patterns (including non-literal `execute_instruction`/`execute_query` payloads) emit lints, and non-literal host reads still skip hints so schedulers must fall back to dynamic analysis (entrypoint manifests report `access_hints_complete`/`access_hints_skipped` for coverage). ⚠
- The compiler scans emitted bytecode for ZK/vector opcodes, auto-enables header bits, and rejects `meta` feature requests that do not match actual opcode usage. ✔
- Numeric aliases (`fixed_u128`, `Amount`, `Balance`) are distinct numeric types with 64-bit scalar semantics; arithmetic preserves the alias and mixing aliases is rejected unless routed through an `int` binding. Trigger declarations (`register_trigger`) now parse time/execute filters and attach metadata to entrypoint manifests; data/pipeline filters and authority overrides remain pending. ⚠

Note: Kotodama compiles to Iroha Virtual Machine (IVM) bytecode (`.to`). It does not target “risc5”/RISC‑V as a standalone ISA. Any RISC‑V–like encodings mentioned in the compiler are IVM’s mixed instruction format and an implementation detail.

## Current Implementation vs. Grammar

### Lexing
- Implemented: identifiers, decimal/hex/binary integers with `_` separators, string literals with rich escapes (`\n`, `\t`, `\r`, `\0`, `\xNN`, `\u{…}`, `\"`, `\\`), raw strings, byte literals, booleans, logical operators, and keyword aliases (`seiyaku`/`誓約`, `kotoage`/`言挙げ`, etc.).

### Parsing and AST
- Implemented:
  - Full contract surface: `seiyaku`, `kotoage fn`, `hajimari`, `kaizen`, `struct`, and `state` items all produce AST nodes and flow into lowering.
  - Parameter grammar accepts `Type name`, `name: Type`, or bare identifiers everywhere; return types (`fn foo() -> Type`) are recorded; tuple destructuring, assignments, compound assignments, `return/break/continue`, ternary `cond ? a : b`, and `call foo()` sugar are available.
  - `permission(Role)` markers, `#[bounded(N)]` attributes, and `meta { key: value; features: ["zk","simd"] }` blocks are parsed and stored.
- Missing or partial:
  - Contract-level localization (`kotoba { ... }`) is accepted as a no-op stub.

### Semantic Analysis (Typing)
- Implemented:
  - Type checking for ints, bools (with implicit promotion to int when needed), strings, pointer-ABI handles (`AccountId`, `Name`, etc.), structs, tuples, and `Map<K,V>`.
  - Durable `state` bindings are injected into each function’s scope, so accessing `state Foo ledger;` compiles without extra boilerplate.
  - Primitive effect analysis guards privileged syscalls: public (`kotoage`) functions that call `transfer_asset`/`mint_asset` must declare `permission(...)` or compilation fails.
  - Numeric aliases (`fixed_u128`, `Amount`, `Balance`) are distinct 64-bit scalars; arithmetic preserves the alias and mixing alias types is rejected unless converted through `int`.
- Missing:
  - Permission annotations are validated at compile time, but runtime enforcement still relies on consuming manifest metadata.
  - Capability analysis for new syscalls (ZK, FASTPQ, trigger APIs) is not modeled, leaving large parts of the documented surface unimplemented.

### IR and Codegen
- Implemented:
  - All parsed functions lower to SSA IR and are emitted, with the entrypoint chosen by `main` > `hajimari` > first function.
  - Pointer literals propagate across calls, durable `state` accesses turn into `STATE_GET/SET/DEL` syscalls when ABI v1 is requested, string/data sections are deduplicated, and manifests supply code/ABI hashes.
  - Emitted bytecode is scanned for ZK/vector opcodes; header bits are auto-enabled and mismatched `meta` requests are rejected.
- Missing:
  - Trigger declarations are manifest-only metadata; cross-contract callback wiring (`call domain::fn`) is recorded but not yet consumed by runtime tooling.
- Access-set hints now include static ISI WSV keys (including literal `create_trigger` specs), literal map keys, dynamic map paths (map-level conflict keys), explicit `#[access]` annotations, and literal `execute_instruction` payloads plus `execute_query` payloads for supported queries (currently `FindAssetById`); non-literal trigger specs and opaque helper syscalls (including non-literal `execute_instruction`/`execute_query` payloads) emit lints, and non-literal host reads still skip hints.

## Samples vs. Implementation
Modern samples compile, but the following grammar-level expectations remain unmet:
- `permission(Role)` metadata now reaches manifests; end-to-end enforcement still depends on node admission wiring.
- Trigger registration works via `register_trigger`/`create_trigger`; DSL trigger declarations now emit manifest metadata but still require deploy-time tooling to register triggers.
- Cross-contract calls and dynamic entrypoint dispatch are only described conceptually; the compiler only knows about intra-program calls.

## Recommended Roadmap (Implementation Targets)
Short-to-mid term steps to align implementation with the designed grammar and safety goals:

1) Metadata + manifest parity
- Extend the current best-effort read/write hints beyond static ISI targets to cover dynamic map key patterns and host-driven reads so schedulers can do conflict analysis directly from manifests (dynamic map keys now emit map-level hints; literal `execute_instruction` payloads and supported `execute_query` payloads are decoded; non-literal host reads still skip hints with lints).
- Done: lints now report dynamic state paths and opaque host reads; opaque host reads still skip hints.
- Done: entrypoint manifests now report `access_hints_complete`/`access_hints_skipped` when coverage is incomplete.

2) Permission and trigger plumbing
- Extend trigger DSL support beyond time/execute filters (data/pipeline) and add explicit authority overrides.
- Wire manifest trigger descriptors into deployment tooling/runtime registration.

3) Type system extensions
- Replace numeric aliases (`fixed_u128`, `Amount`, `Balance`) with deterministic fixed-point/128-bit semantics where needed.
- Teach the type checker how to reason about Norito pointer wrappers (`Json`, `Blob`, `NoritoBytes`) beyond simple assignment so builders from the grammar work without manual casts.

4) Access hints and host integration
- Make read/write hints precise for dynamic maps (e.g., hashed keys, per-field cardinality) and include host-driven reads so schedulers can safely preplan execution.
- Surface when `create_trigger` specs are non-literal and access hints are skipped (lint now warns for non-literal trigger specs).

5) Tooling separation
- Extract the Kotodama compiler into `crates/kotodama_lang` once dependencies are untangled.

## Quick Wins (Low Risk, High Impact)
- Done: lint now reports dynamic state paths and opaque host reads (non-literal trigger specs and state-map keys were already covered).
- Emit a compiler hint when literal trigger specs cannot be decoded for access hints.

## Known Limitations to Call Out in Docs
- Access hints cover static ISI targets (including literal trigger specs), dynamic map paths via map-level keys, explicit `#[access]` annotations, and literal `execute_instruction` payloads plus supported `execute_query` payloads, but opaque helper syscalls and non-literal host reads still skip hints (with lints, including non-literal `execute_instruction`/`execute_query` payloads), so schedulers need fallback analysis for complex contracts.
- Entrypoint manifests report `access_hints_complete`/`access_hints_skipped` so coverage gaps can be surfaced without reading lints.
- Meta feature flags (`zk`, `vector`, `features`) are validated against emitted opcodes; requesting features that are unused now fails compilation.
- Numeric aliases (e.g., `fixed_u128`) are distinct types with 64-bit semantics; true 128-bit or fixed-point math still requires dedicated helpers.
- `permission(...)` annotations are enforced by compiler diagnostics and written into manifests; runtime enforcement depends on consuming the metadata.
- Trigger declarations currently support time/execute filters only; data/pipeline filters and explicit authority overrides remain pending.

Keeping these limitations explicit helps set expectations and aids contributors in targeting the most valuable next steps.
