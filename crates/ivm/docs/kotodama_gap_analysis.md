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
- Metadata and manifest wiring now surface `meta { features: ["zk","simd"] }` toggles plus compiler-generated per-entrypoint permission/read/write hints. Core direct dispatch and nested `CALL_CONTRACT` dispatch consume entrypoint permission metadata and require the caller to hold the named permission directly or through a role before invocation. Static ISI keys, literal map keys, dynamic map paths, bounded dynamic state-map iteration, literal nullifiers, decodable transfer batches, native/anonymous escrow helpers, and static smart-contract lifecycle requests with decodable Norito payloads are represented precisely when possible. Manual `#[access(...)]` annotations are rejected; opaque dynamic ledger access may emit compiler-owned wildcard hints so the scheduler can use its dynamic prepass or conservative serialization path. ✔
- The compiler scans emitted bytecode for ZK/vector opcodes, auto-enables header bits, and rejects `meta` feature requests that do not match actual opcode usage. ✔
- Numeric aliases (`fixed_u128`, `Amount`, `Balance`) are distinct `Numeric`-backed scalar types (mantissa+scale) restricted to unsigned, scale‑0 values. Decimal literals are rejected in v1; arithmetic preserves the alias and mixing aliases is rejected unless routed through an `int` binding. Conversions to/from `int` are checked at runtime (range‑limited, non‑negative). Trigger declarations (`register_trigger`) now parse time/execute/data filters plus deterministic approved block/transaction pipeline filters, lower structured data-trigger blocks into manifest `EventFilterBox` values, support explicit trigger authority overrides, attach metadata to entrypoint manifests, and are auto-registered when a contract instance is activated (removed on deactivation). Namespaced trigger callbacks resolve at activation time to an already active contract address or alias and fail closed when the target or entrypoint is unavailable. Inline ZK unshield builders now encode one or more input nullifier chunks plus optional private change output chunks. ✔

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
  - Internal helper functions can accept durable `state` parameters for scalar, map, struct, and tuple state handles. Callers must pass a durable state binding or durable member expression, and aggregate handles use deterministic flattened child handles.
  - Primitive effect analysis guards privileged syscalls: public (`kotoage`) functions that call mutating ledger helpers or ZK verify latch helpers must declare `permission(...)` or compilation fails, and `view` functions cannot call them transitively.
  - Runtime contract dispatch enforces entrypoint `permission(...)` manifest metadata against direct and role-derived account permissions before invoking a VM, including nested `CALL_CONTRACT` child VMs.
  - Numeric aliases (`fixed_u128`, `Amount`, `Balance`) are distinct `Numeric`-backed scalars; arithmetic preserves the alias and mixing alias types is rejected unless converted through `int`.
- Ongoing policy:
  - New helper surfaces should add capability/effect and access-hint tests when they are introduced, so mutating or externally visible behavior does not bypass first-release manifest metadata.

### IR and Codegen
- Implemented:
  - All parsed functions lower to SSA IR and are emitted, with the entrypoint chosen by `main` > `hajimari` > first function.
  - Pointer literals propagate across calls, durable `state` accesses turn into `STATE_GET/SET/DEL` syscalls when ABI v1 is requested, string/data sections are deduplicated, and manifests supply code/ABI hashes.
  - Emitted bytecode is scanned for ZK/vector opcodes; header bits are auto-enabled and mismatched `meta` requests are rejected.
  - Inline ZK ISI builders emit canonical Norito `InstructionBox` payloads; `build_unshield_inline` supports multiple 32-byte input chunks and optional 32-byte private change output chunks.
  - Cross-contract trigger callbacks (`call alias::fn`) are recorded in manifests and resolved by core activation against active contract addresses or aliases. Bare namespaces resolve as `<name>::universal`; fully qualified contract aliases and contract-address literals are also accepted.
  - Aggregate `state` parameters lower to hidden root plus flattened child handle arguments, so helpers can read `entry.counter` and `entries[key].amount` through the same deterministic durable paths as direct state access.
- Access-set hints now include static ISI WSV keys, native/anonymous escrow record keys, static smart-contract manifest/code/instance lifecycle keys, literal nullifier keys, decoded transfer-batch asset keys, literal map keys, dynamic map paths via map-level conflict keys, and bounded dynamic state-map iteration descriptors. Manual access annotations are no longer part of the release language; compiler-owned wildcard fallbacks remain available for dynamic ledger helper calls that cannot be described precisely yet.

## Samples vs. Implementation
Modern samples compile, and the following grammar-level expectations are now covered:
- `permission(Role)` metadata reaches manifests and core dispatch enforces the named permission for direct contract calls, metadata-dispatched IVM calls, and nested `CALL_CONTRACT` calls.
- Trigger registration works via `register_trigger`/`create_trigger`; DSL trigger declarations now emit manifest metadata and are auto-registered on contract instance activation.
- Cross-contract trigger callbacks resolve during contract activation. General
  deployed-contract calls are available through the `call_contract(...)` helper,
  with CoreHost enforcing the callee entrypoint permission metadata.

## Completed Implementation Checkpoints
The first-release implementation checkpoints aligned with the designed grammar
and safety goals are:

1) Metadata + manifest parity
- Done: compiler-generated hints cover static ISI targets, literal state paths, dynamic map-level state keys, and bounded dynamic state-map iteration descriptors.
- Done: production artifacts carry compiler-generated access metadata, using compiler-owned wildcard manifests for dynamic ledger access that cannot be derived precisely yet.

2) Permission and trigger plumbing
- Done: runtime direct and nested contract dispatch consume manifest entrypoint `permission(...)` metadata and reject callers missing the named direct or role-derived permission.
- Done: extend trigger DSL support to data filters, deterministic approved block/transaction pipeline filters, and explicit authority overrides.
- Done: wire manifest trigger descriptors into runtime registration on activation/deactivation, including activation-time resolution for cross-contract callbacks.

3) Type system extensions
- Done: numeric aliases (`fixed_u128`, `Amount`, `Balance`) now use deterministic `Numeric` syscalls with unsigned, scale‑0 values; decimal literals are rejected in v1.
- Done: Norito pointer-wrapper constructors and method-call sugar accept string bindings, matching pointer types, and Blob/bytes payloads so grammar-level builders compile without manual casts.
- Done: durable `state` helper parameters now support aggregate struct/tuple state handles and maps with aggregate values through flattened child handles.

4) Access hints and host integration
- Done: production artifacts must carry compiler-generated access metadata, with wildcard fallbacks limited to compiler-diagnosed dynamic ledger access.
- Done: native and anonymous escrow helper syscalls now emit stable escrow record access keys when Kotodama names are literal, and anonymous escrow request-backed helpers decode literal Norito payloads for precise keys.
- Done: static smart-contract lifecycle helper requests for manifest registration, bytecode registration, instance activation, and bytecode removal now decode literal Norito payloads into contract manifest/code/instance access keys.
- Done: literal `use_nullifier(...)` calls and decodable `transfer_v1_batch_apply(...)` payloads now emit exact nullifier and asset access keys.
- Conservative access-hint cases are intentionally dynamic or test-only:
  unresolved smart-contract lifecycle payloads, malformed request bytes, and
  actor-helper intrinsics produce incomplete access metadata instead of
  guessing.
- Done: literal `create_trigger(json(...))` specs that cannot be decoded now emit a dedicated access-hint diagnostic and manifest skip reason; lint still covers non-literal trigger specs.

5) Tooling separation
- Done: the Kotodama compiler, parser, semantic analysis, IR, linting, and tooling support now live in `crates/kotodama_lang`.

## Quick Wins (Low Risk, High Impact)
- Done: lint now reports dynamic state paths and opaque host reads, while staying silent for compiler-hintable asset registration, literal transfer-domain routing, subscription context, inline ZK builders, and escrow helpers (non-literal trigger specs and state-map keys were already covered).
- Done: compiler diagnostics now count literal trigger spec decode failures and production rejections include the trigger-specific access-hint reason.

## Deterministic Boundaries
- Access hints cover static ISI targets, native/anonymous escrow helpers with literal or decodable request inputs, static smart-contract lifecycle requests, literal nullifier use, decodable transfer batches, dynamic map paths via map-level keys, and bounded dynamic state-map iteration. Opaque dynamic ledger helper syscalls may still use compiler-owned wildcard hints until they have precise compiler-derived access descriptors.
- Entrypoint manifests emit complete hints for production artifacts.
- Meta feature flags (`zk`, `vector`, `features`) are validated against emitted opcodes; requesting features that are unused now fails compilation.
- Numeric aliases (e.g., `fixed_u128`) are distinct `Numeric` types; v1 restricts them to unsigned integers (scale = 0), rejecting fractional values and decimal literals.
- `permission(...)` annotations are enforced by compiler diagnostics, written into manifests, and consumed by core direct and nested contract dispatch before VM invocation.
- Trigger declarations support time/execute/data filters plus deterministic approved block/transaction pipeline filters and explicit authority overrides; cross-contract callbacks must resolve to an already active contract address or alias during activation.

Keeping these limitations explicit helps set expectations and aids contributors in targeting the most valuable next steps.
