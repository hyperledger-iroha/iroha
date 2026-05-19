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
- Metadata and manifest wiring now surface `meta { features: ["zk","simd"] }` toggles plus compiler-generated per-entrypoint permission/read/write hints. Static ISI keys, literal map keys, dynamic map paths, and bounded dynamic state-map iteration are represented without production wildcard hints. Manual `#[access(...)]` annotations are rejected, and production compilation fails when opaque host access would require a wildcard fallback. ✔
- The compiler scans emitted bytecode for ZK/vector opcodes, auto-enables header bits, and rejects `meta` feature requests that do not match actual opcode usage. ✔
- Numeric aliases (`fixed_u128`, `Amount`, `Balance`) are distinct `Numeric`-backed scalar types (mantissa+scale) restricted to unsigned, scale‑0 values. Decimal literals are rejected in v1; arithmetic preserves the alias and mixing aliases is rejected unless routed through an `int` binding. Conversions to/from `int` are checked at runtime (range‑limited, non‑negative). Trigger declarations (`register_trigger`) now parse time/execute/data/pipeline filters, lower structured data-trigger blocks into manifest `EventFilterBox` values, support explicit trigger authority overrides, attach metadata to entrypoint manifests, and are auto-registered when a contract instance is activated (removed on deactivation); cross-contract callbacks are rejected. ✔

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
  - Cross-contract callback wiring (`call domain::fn`) is recorded but currently rejected by runtime tooling.
- Access-set hints now include static ISI WSV keys, literal map keys, dynamic map paths via map-level conflict keys, and bounded dynamic state-map iteration descriptors. Manual access annotations and production wildcard fallbacks are no longer part of the release language.

## Samples vs. Implementation
Modern samples compile, but the following grammar-level expectations remain unmet:
- `permission(Role)` metadata now reaches manifests; end-to-end enforcement still depends on node admission wiring.
- Trigger registration works via `register_trigger`/`create_trigger`; DSL trigger declarations now emit manifest metadata and are auto-registered on contract instance activation.
- Cross-contract calls and dynamic entrypoint dispatch are only described conceptually; the compiler only knows about intra-program calls.

## Recommended Roadmap (Implementation Targets)
Short-to-mid term steps to align implementation with the designed grammar and safety goals:

1) Metadata + manifest parity
- Done: compiler-generated hints cover static ISI targets, literal state paths, dynamic map-level state keys, and bounded dynamic state-map iteration descriptors.
- Done: production compilation rejects incomplete access derivation instead of emitting wildcard manifests.

2) Permission and trigger plumbing
- Done: extend trigger DSL support to data/pipeline filters and explicit authority overrides.
- Done: wire manifest trigger descriptors into runtime registration on activation/deactivation (local callbacks only).

3) Type system extensions
- Done: numeric aliases (`fixed_u128`, `Amount`, `Balance`) now use deterministic `Numeric` syscalls with unsigned, scale‑0 values; decimal literals are rejected in v1.
- Teach the type checker how to reason about Norito pointer wrappers (`Json`, `Blob`, `NoritoBytes`) beyond simple assignment so builders from the grammar work without manual casts.

4) Access hints and host integration
- Done: production artifacts must carry complete compiler-generated access metadata.
- Next: add precise descriptors for remaining opaque helper syscalls so they can compile in production without falling back to test-mode wildcard diagnostics.
- Keep warning when `create_trigger` specs cannot be decoded for access hints (lint already covers non-literal trigger specs).

5) Tooling separation
- Extract the Kotodama compiler into `crates/kotodama_lang` once dependencies are untangled.

## Quick Wins (Low Risk, High Impact)
- Done: lint now reports dynamic state paths and opaque host reads (non-literal trigger specs and state-map keys were already covered).
- Emit a compiler hint when literal trigger specs cannot be decoded for access hints.

## Known Limitations to Call Out in Docs
- Access hints cover static ISI targets, dynamic map paths via map-level keys, and bounded dynamic state-map iteration. Opaque helper syscalls remain test-mode-only until they have precise compiler-derived access descriptors.
- Entrypoint manifests emit complete hints for production artifacts.
- Meta feature flags (`zk`, `vector`, `features`) are validated against emitted opcodes; requesting features that are unused now fails compilation.
- Numeric aliases (e.g., `fixed_u128`) are distinct `Numeric` types; v1 restricts them to unsigned integers (scale = 0), rejecting fractional values and decimal literals.
- `permission(...)` annotations are enforced by compiler diagnostics and written into manifests; runtime enforcement depends on consuming the metadata.
- Trigger declarations support time/execute/data/pipeline filters plus explicit authority overrides; cross-contract callbacks are still rejected (local only).

Keeping these limitations explicit helps set expectations and aids contributors in targeting the most valuable next steps.
