# Kotodama Gap Analysis

This document compares the current Kotodama syntax/grammar documentation with the implementation in `crates/kotodama_lang/src` and outlines where the implementation should go to meet the design goals of clarity, safety, and ease of use.

Paths for reference:
- Lexer: `crates/kotodama_lang/src/lexer.rs`
- AST: `crates/kotodama_lang/src/ast.rs`
- Parser: `crates/kotodama_lang/src/parser.rs`
- Semantic/type check: `crates/kotodama_lang/src/semantic.rs`
- IR + lowering: `crates/kotodama_lang/src/ir.rs`
- Codegen: `crates/kotodama_lang/src/compiler.rs`
- Samples: `crates/kotodama_lang/src/samples/*.ko`

## Summary
- The current implementation parses and lowers both free and contract functions (including `seiyaku`, `kotoage`, `hajimari`, and `kaizen` items), performs type checking for ints/bools/strings/pointer-ABI handles/structs/maps, and emits full multi-function IVM bytecode with durable `state` overlays when ABI v1 is selected. ✔
- Contract-level localization (`kotoba { ... }`) is parsed, validated for duplicates/empties, and emitted into manifest translation tables for tooling. ✔
- Metadata and manifest wiring now surface `meta { features: ["zk","simd"] }` toggles plus per-entrypoint permission/read/write hints. Static ISI keys (including literal `create_trigger` specs and `transfer_domain`), literal map keys (including hashed pointer keys), dynamic map paths (map-level `state:<name>` conflicts), explicit `#[access(read=..., write=...)]` annotations, and literal `execute_instruction` payloads plus `execute_query` payloads for supported queries (InstructionBox/QueryRequest decode, currently `FindAssetById`) are included in access hints; non-literal trigger specs and opaque host access patterns (including non-literal `execute_instruction`/`execute_query` payloads) emit lints, and opaque host reads now fall back to conservative wildcard hints (`*`) with diagnostics so schedulers can opt into a dynamic prepass when finer-grained keys are needed (entrypoint manifests emit hints when enabled; `access_hints_skipped` is empty, and fallback counts are reported via diagnostics). ⚠
- The compiler scans emitted bytecode for ZK/vector opcodes, auto-enables header bits, and rejects `meta` feature requests that do not match actual opcode usage. ✔
- Numeric aliases (`fixed_u128`, `Amount`, `Balance`) are distinct `Numeric`-backed scalar types (mantissa+scale) restricted to unsigned, scale‑0 values. Decimal literals are rejected in v1; arithmetic preserves the alias and mixing aliases is rejected unless routed through an `int` binding. Conversions to/from `int` are checked at runtime (range‑limited, non‑negative). Trigger declarations (`register_trigger`) now parse time/execute filters, attach metadata to entrypoint manifests, and are auto-registered when a contract instance is activated (removed on deactivation); data/pipeline filters and authority overrides remain pending, and cross-contract callbacks are rejected. ⚠

Note: Kotodama compiles to Iroha Virtual Machine (IVM) bytecode (`.to`). It does not target “risc5”/RISC‑V as a standalone ISA. Any RISC‑V–like encodings mentioned in the compiler are IVM’s mixed instruction format and an implementation detail.

## Current Implementation vs. Grammar

### Lexing
- Implemented: identifiers, decimal/hex/binary integers (plus decimal fractions) with `_` separators, string literals with rich escapes (`\n`, `\t`, `\r`, `\0`, `\xNN`, `\u{…}`, `\"`, `\\`), raw strings, byte literals, booleans, logical operators, and keyword aliases (`seiyaku`/`誓約`, `kotoage`/`言挙げ`, etc.).

### Parsing and AST
- Implemented:
  - Full contract surface: `seiyaku`, `kotoage fn`, `hajimari`, `kaizen`, `struct`, and `state` items all produce AST nodes and flow into lowering.
  - Parameter grammar accepts `Type name`, `name: Type`, or bare identifiers everywhere; return types (`fn foo() -> Type`) are recorded; tuple destructuring, assignments, compound assignments, `return/break/continue`, ternary `cond ? a : b`, and `call foo()` sugar are available.
  - `permission(Role)` markers, `#[bounded(N)]` attributes, and `meta { key: value; features: ["zk","simd"] }` blocks are parsed and stored.
  - Contract-level localization (`kotoba { ... }`) is parsed, validated, and emitted into manifest translations.

### Semantic Analysis (Typing)
- Implemented:
  - Type checking for ints, bools (with implicit promotion to int when needed), strings, pointer-ABI handles (`AccountId`, `Name`, etc.), structs, tuples, and `Map<K,V>`.
  - Durable `state` bindings are injected into each function’s scope, so accessing `state Foo ledger;` compiles without extra boilerplate.
  - Primitive effect analysis guards privileged syscalls: public (`kotoage`) functions that call `transfer_asset`/`mint_asset` must declare `permission(...)` or compilation fails.
  - Numeric aliases (`fixed_u128`, `Amount`, `Balance`) are distinct `Numeric`-backed scalars; arithmetic preserves the alias and mixing alias types is rejected unless converted through `int`.
- Missing:
  - Permission annotations are validated at compile time, but runtime enforcement still relies on consuming manifest metadata.
  - Capability analysis for new syscalls (ZK, FASTPQ, trigger APIs) is not modeled, leaving large parts of the documented surface unimplemented.

### IR and Codegen
- Implemented:
  - All parsed functions lower to SSA IR and are emitted, with the entrypoint chosen by `main` > `hajimari` > first function.
  - Pointer literals propagate across calls, durable `state` accesses turn into `STATE_GET/SET/DEL` syscalls when ABI v1 is requested, string/data sections are deduplicated, and manifests supply code/ABI hashes.
  - Emitted bytecode is scanned for ZK/vector opcodes; header bits are auto-enabled and mismatched `meta` requests are rejected.
- Missing:
  - Trigger declarations are registered from manifests during contract instance activation; cross-contract callback wiring (`call domain::fn`) is recorded but currently rejected by runtime tooling.
- Access-set hints now include static ISI WSV keys (including literal `create_trigger` specs and `transfer_domain`), literal map keys, dynamic map paths (map-level conflict keys), explicit `#[access]` annotations, and literal `execute_instruction` payloads plus `execute_query` payloads for supported queries (currently `FindAssetById`); non-literal trigger specs and opaque helper syscalls (including non-literal `execute_instruction`/`execute_query` payloads) emit lints, and opaque host reads now fall back to conservative wildcard hints (`*`).

## Samples vs. Implementation
Modern samples compile, but the following grammar-level expectations remain unmet:
- `permission(Role)` metadata now reaches manifests; end-to-end enforcement still depends on node admission wiring.
- Trigger registration works via `register_trigger`/`create_trigger`; DSL trigger declarations now emit manifest metadata and are auto-registered on contract instance activation.
- Cross-contract calls and dynamic entrypoint dispatch are only described conceptually; the compiler only knows about intra-program calls.

## Recommended Roadmap (Implementation Targets)
Short-to-mid term steps to align implementation with the designed grammar and safety goals:

1) Metadata + manifest parity
- Extend the current best-effort read/write hints beyond static ISI targets to cover dynamic map key patterns and host-driven reads so schedulers can do conflict analysis directly from manifests (dynamic map keys now emit map-level hints; literal `execute_instruction` payloads and supported `execute_query` payloads are decoded; opaque host reads now fall back to conservative wildcard hints with lints).
- Done: lints now report dynamic state paths and opaque host reads; opaque host reads now fall back to wildcard hints.
- Done: entrypoint manifests emit hints when enabled; wildcard keys cover opaque access, and diagnostics counters report fallback usage.

2) Permission and trigger plumbing
- Extend trigger DSL support beyond time/execute filters (data/pipeline) and add explicit authority overrides.
- Done: wire manifest trigger descriptors into runtime registration on activation/deactivation (local callbacks only).

3) Type system extensions
- Done: numeric aliases (`fixed_u128`, `Amount`, `Balance`) now use deterministic `Numeric` syscalls with unsigned, scale‑0 values; decimal literals are rejected in v1.
- Teach the type checker how to reason about Norito pointer wrappers (`Json`, `Blob`, `NoritoBytes`) beyond simple assignment so builders from the grammar work without manual casts.

4) Access hints and host integration
- Done: opaque host-driven reads now emit conservative wildcard hints (`*`) instead of skipping access hints entirely.
- Next: refine wildcard access hints into precise per-key coverage for dynamic maps and host-driven reads so schedulers can safely preplan execution.
- Keep warning when `create_trigger` specs cannot be decoded for access hints (lint already covers non-literal trigger specs).

5) Tooling separation
- Extract the Kotodama compiler into `crates/kotodama_lang` once dependencies are untangled.

## Quick Wins (Low Risk, High Impact)
- Done: lint now reports dynamic state paths and opaque host reads (non-literal trigger specs and state-map keys were already covered).
- Emit a compiler hint when literal trigger specs cannot be decoded for access hints.

## Known Limitations to Call Out in Docs
- Access hints cover static ISI targets (including literal trigger specs and `transfer_domain`), dynamic map paths via map-level keys, explicit `#[access]` annotations, and literal `execute_instruction` payloads plus supported `execute_query` payloads; opaque helper syscalls and non-literal host reads now emit conservative wildcard hints (`*`) with lints, so schedulers should still prefer the dynamic prepass for higher precision.
- Entrypoint manifests emit hints when enabled; fallback usage is surfaced via diagnostics rather than skip reasons.
- Meta feature flags (`zk`, `vector`, `features`) are validated against emitted opcodes; requesting features that are unused now fails compilation.
- Numeric aliases (e.g., `fixed_u128`) are distinct `Numeric` types; v1 restricts them to unsigned integers (scale = 0), rejecting fractional values and decimal literals.
- `permission(...)` annotations are enforced by compiler diagnostics and written into manifests; runtime enforcement depends on consuming the metadata.
- Trigger declarations currently support time/execute filters only; data/pipeline filters and explicit authority overrides remain pending, and cross-contract callbacks are rejected (local only).

Keeping these limitations explicit helps set expectations and aids contributors in targeting the most valuable next steps.
