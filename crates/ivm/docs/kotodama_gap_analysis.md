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
- Metadata and manifest wiring now surface `meta { features: ["zk","simd"] }` toggles plus compiler-generated per-entrypoint permission/read/write hints. Static ISI keys, literal map keys, dynamic map paths, bounded dynamic state-map iteration, and native/anonymous escrow helpers with literal names or decodable Norito request payloads are represented precisely when possible. Manual `#[access(...)]` annotations are rejected; opaque dynamic ledger access may emit compiler-owned wildcard hints so the scheduler can use its dynamic prepass or conservative serialization path. ✔
- The compiler scans emitted bytecode for ZK/vector opcodes, auto-enables header bits, and rejects `meta` feature requests that do not match actual opcode usage. ✔
- Numeric aliases (`fixed_u128`, `Amount`, `Balance`) are distinct `Numeric`-backed scalar types (mantissa+scale) restricted to unsigned, scale‑0 values. Decimal literals are rejected in v1; arithmetic preserves the alias and mixing aliases is rejected unless routed through an `int` binding. Conversions to/from `int` are checked at runtime (range‑limited, non‑negative). Trigger declarations (`register_trigger`) now parse time/execute/data filters plus deterministic approved block/transaction pipeline filters, lower structured data-trigger blocks into manifest `EventFilterBox` values, support explicit trigger authority overrides, attach metadata to entrypoint manifests, and are auto-registered when a contract instance is activated (removed on deactivation); cross-contract callbacks are rejected. Inline ZK unshield builders now encode one or more input nullifier chunks plus optional private change output chunks. ✔

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
  - Norito pointer-wrapper helpers (`json`, `name`, `blob`, `norito_bytes`, and the other pointer constructors) accept string bindings, matching pointer values, and Blob/bytes payloads where appropriate; method-call sugar such as `payload.json()` and `payload.name()` lowers through the same typed constructor paths.
  - Durable `state` bindings are injected into each function’s scope, so accessing `state Foo ledger;` compiles without extra boilerplate.
  - Primitive effect analysis guards privileged syscalls: public (`kotoage`) functions that call mutating ledger helpers or ZK verify latch helpers must declare `permission(...)` or compilation fails, and `view` functions cannot call them transitively.
  - Numeric aliases (`fixed_u128`, `Amount`, `Balance`) are distinct `Numeric`-backed scalars; arithmetic preserves the alias and mixing alias types is rejected unless converted through `int`.
- Missing:
  - Permission annotations are validated at compile time, but runtime enforcement still relies on consuming manifest metadata.
  - Broader capability analysis for remaining non-latch syscall families is still incomplete, so new helper surfaces should add effect/hint tests when they are introduced.

### IR and Codegen
- Implemented:
  - All parsed functions lower to SSA IR and are emitted, with the entrypoint chosen by `main` > `hajimari` > first function.
  - Pointer literals propagate across calls, durable `state` accesses turn into `STATE_GET/SET/DEL` syscalls when ABI v1 is requested, string/data sections are deduplicated, and manifests supply code/ABI hashes.
  - Emitted bytecode is scanned for ZK/vector opcodes; header bits are auto-enabled and mismatched `meta` requests are rejected.
  - Inline ZK ISI builders emit canonical Norito `InstructionBox` payloads; `build_unshield_inline` supports multiple 32-byte input chunks and optional 32-byte private change output chunks.
- Missing:
  - Cross-contract callback wiring (`call domain::fn`) is recorded but currently rejected by runtime tooling.
- Access-set hints now include static ISI WSV keys, native/anonymous escrow record keys, literal map keys, dynamic map paths via map-level conflict keys, and bounded dynamic state-map iteration descriptors. Manual access annotations are no longer part of the release language; compiler-owned wildcard fallbacks remain available for dynamic ledger helper calls that cannot be described precisely yet.

## Samples vs. Implementation
Modern samples compile, but the following grammar-level expectations remain unmet:
- `permission(Role)` metadata now reaches manifests; end-to-end enforcement still depends on node admission wiring.
- Trigger registration works via `register_trigger`/`create_trigger`; DSL trigger declarations now emit manifest metadata and are auto-registered on contract instance activation.
- Cross-contract calls and dynamic entrypoint dispatch are only described conceptually; the compiler only knows about intra-program calls.

## Recommended Roadmap (Implementation Targets)
Short-to-mid term steps to align implementation with the designed grammar and safety goals:

1) Metadata + manifest parity
- Done: compiler-generated hints cover static ISI targets, literal state paths, dynamic map-level state keys, and bounded dynamic state-map iteration descriptors.
- Done: production artifacts carry compiler-generated access metadata, using compiler-owned wildcard manifests for dynamic ledger access that cannot be derived precisely yet.

2) Permission and trigger plumbing
- Done: extend trigger DSL support to data filters, deterministic approved block/transaction pipeline filters, and explicit authority overrides.
- Done: wire manifest trigger descriptors into runtime registration on activation/deactivation (local callbacks only).

3) Type system extensions
- Done: numeric aliases (`fixed_u128`, `Amount`, `Balance`) now use deterministic `Numeric` syscalls with unsigned, scale‑0 values; decimal literals are rejected in v1.
- Done: Norito pointer-wrapper constructors and method-call sugar accept string bindings, matching pointer types, and Blob/bytes payloads so grammar-level builders compile without manual casts.

4) Access hints and host integration
- Done: production artifacts must carry compiler-generated access metadata, with wildcard fallbacks limited to compiler-diagnosed dynamic ledger access.
- Done: native and anonymous escrow helper syscalls now emit stable escrow record access keys when Kotodama names are literal, and anonymous escrow request-backed helpers decode literal Norito payloads for precise keys.
- Next: add precise descriptors for remaining opaque helper syscalls to reduce wildcard fallback use.
- Done: literal `create_trigger(json(...))` specs that cannot be decoded now emit a dedicated access-hint diagnostic and manifest skip reason; lint still covers non-literal trigger specs.

5) Tooling separation
- Done: the Kotodama compiler, parser, semantic analysis, IR, linting, and tooling support now live in `crates/kotodama_lang`.

## Quick Wins (Low Risk, High Impact)
- Done: lint now reports dynamic state paths and opaque host reads, while staying silent for compiler-hintable escrow helpers (non-literal trigger specs and state-map keys were already covered).
- Done: compiler diagnostics now count literal trigger spec decode failures and production rejections include the trigger-specific access-hint reason.

## Known Limitations to Call Out in Docs
- Access hints cover static ISI targets, native/anonymous escrow helpers with literal or decodable request inputs, dynamic map paths via map-level keys, and bounded dynamic state-map iteration. Opaque dynamic ledger helper syscalls may still use compiler-owned wildcard hints until they have precise compiler-derived access descriptors.
- Entrypoint manifests emit complete hints for production artifacts.
- Meta feature flags (`zk`, `vector`, `features`) are validated against emitted opcodes; requesting features that are unused now fails compilation.
- Numeric aliases (e.g., `fixed_u128`) are distinct `Numeric` types; v1 restricts them to unsigned integers (scale = 0), rejecting fractional values and decimal literals.
- `permission(...)` annotations are enforced by compiler diagnostics and written into manifests; runtime enforcement depends on consuming the metadata.
- Trigger declarations support time/execute/data filters plus deterministic approved block/transaction pipeline filters and explicit authority overrides; cross-contract callbacks are still rejected (local only).

Keeping these limitations explicit helps set expectations and aids contributors in targeting the most valuable next steps.
