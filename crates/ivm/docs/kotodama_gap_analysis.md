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
- Metadata and manifest wiring now surface `meta { features: ["zk","simd"] }` toggles plus per-entrypoint permission/read/write hints, and the compiler now emits WSV access keys for static ISI syscalls. Dynamic map keys and opaque host-driven reads still require fallback analysis. ⚠
- The compiler now scans emitted bytecode for vector opcodes and auto-enables the header bit, yet there is still no author-facing diagnostic when SIMD is requested but unused (or vice versa), and access hints remain best-effort. ⚠
- Higher-level constructs mentioned in the docs (trigger registration, access-list extraction, richer numeric types such as `fixed_u128`) still compile as opaque placeholders and need end-to-end support. ⏳

Note: Kotodama compiles to Iroha Virtual Machine (IVM) bytecode (`.to`). It does not target “risc5”/RISC‑V as a standalone ISA. Any RISC‑V–like encodings mentioned in the compiler are IVM’s mixed instruction format and an implementation detail.

## Current Implementation vs. Grammar

### Lexing
- Implemented: identifiers, decimal/hex/binary integers with `_` separators, string literals with basic escapes (`\n`, `\t`, `\"`, `\\`), booleans, logical operators, and keyword aliases (`seiyaku`/`誓約`, `kotoage`/`言挙げ`, etc.).
- Missing: richer escapes (`\xNN`, `\u{…}`), raw strings, and verbatim byte literals that the grammar references for Norito blobs. These should be added so Norito payload samples can be copied verbatim.

### Parsing and AST
- Implemented:
  - Full contract surface: `seiyaku`, `kotoage fn`, `hajimari`, `kaizen`, `struct`, and `state` items all produce AST nodes and flow into lowering.
  - Parameter grammar accepts `Type name`, `name: Type`, or bare identifiers everywhere; return types (`fn foo() -> Type`) are recorded; tuple destructuring, assignments, compound assignments, `return/break/continue`, ternary `cond ? a : b`, and `call foo()` sugar are available.
  - `permission(Role)` markers, `#[bounded(N)]` attributes, and `meta { key: value; features: ["zk","simd"] }` blocks are parsed and stored.
- Missing or partial:
  - No syntax exists for trigger/attribute blocks such as `register_trigger` or `#[access(write = …)]`, even though the grammar advertises them.
  - Contract-level localization (`kotoba { ... }`) is explicitly rejected, but the grammar still mentions it; the docs should either drop it or the compiler should offer a no-op stub.

### Semantic Analysis (Typing)
- Implemented:
  - Type checking for ints, bools (with implicit promotion to int when needed), strings, pointer-ABI handles (`AccountId`, `Name`, etc.), structs, tuples, and `Map<K,V>`.
  - Durable `state` bindings are injected into each function’s scope, so accessing `state Foo ledger;` compiles without extra boilerplate.
  - Primitive effect analysis guards privileged syscalls: public (`kotoage`) functions that call `transfer_asset`/`mint_asset` must declare `permission(...)` or compilation fails.
- Missing:
  - Custom numeric types (`fixed_u128`, `Amount`, etc.) lower to opaque placeholders, so arithmetic on them is rejected; the grammar expects these to behave like first-class scalars.
  - Permission annotations are linted but not surfaced anywhere else, so enforcement relies entirely on the compiler diagnostic.
  - Capability analysis for new syscalls (ZK, FASTPQ, trigger APIs) is not modeled, leaving large parts of the documented surface unimplemented.

### IR and Codegen
- Implemented:
  - All parsed functions lower to SSA IR and are emitted, with the entrypoint chosen by `main` > `hajimari` > first function.
  - Pointer literals propagate across calls, durable `state` accesses turn into `STATE_GET/SET/DEL` syscalls when ABI v1 is requested, string/data sections are deduplicated, and manifests supply code/ABI hashes.
- Missing:
  - Vector/SIMD usage is only reported when authors manually request it; the compiler does not scan emitted IR/opcodes to auto-enable the `VECTOR` bit or detect mismatches when SIMD ops appear without a feature flag.
- Access-set hints now include static ISI WSV keys; dynamic map keys and opaque helper syscalls remain unhinted.
  - Trigger registration and scheduler callbacks referenced in the DSL (`register_trigger`, `call domain::fn`, etc.) do not exist in IR/codegen.

## Samples vs. Implementation
Modern samples compile, but the following grammar-level expectations remain unmet:
- `permission(Role)` metadata now reaches manifests; end-to-end enforcement still depends on node admission wiring.
- Trigger and scheduler examples (`register_trigger`, access-list attributes) referenced in the docs have no compiler support.
- Cross-contract calls and dynamic entrypoint dispatch are only described conceptually; the compiler only knows about intra-program calls.
- Trigger and scheduler examples (`register_trigger`, access-list attributes) referenced in the docs have no compiler support.
- Cross-contract calls and dynamic entrypoint dispatch are only described conceptually; the compiler only knows about intra-program calls.

## Recommended Roadmap (Implementation Targets)
Short-to-mid term steps to align implementation with the designed grammar and safety goals:

1) Metadata + manifest parity
- Extend the current best-effort read/write hints beyond static ISI targets to cover dynamic map key patterns and host-driven reads so schedulers can do conflict analysis directly from manifests.
- Emit diagnostics when vector/ZK opcodes are present but the corresponding mode bits are forced off (or vice versa) to catch accidental divergence early.

2) Permission and trigger plumbing
- Design `register_trigger`/scheduler intrinsics so the grammar’s trigger examples become runnable, even if the initial implementation limits the scope.
- Extend manifest entrypoint descriptors with trigger metadata once the runtime semantics are finalized.

3) Type system extensions
- Add first-class numeric aliases (`fixed_u128`, `Amount`, `Balance`) and wire them to deterministic arithmetic helpers (or provide compile errors that mention the current limitation).
- Teach the type checker how to reason about Norito pointer wrappers (`Json`, `Blob`, `NoritoBytes`) beyond simple assignment so builders from the grammar work without manual casts.

4) Access hints and host integration
- Make read/write hints precise for dynamic maps (e.g., hashed keys, per-field cardinality) and include host-driven reads so schedulers can safely preplan execution.
- When `meta` requests features such as `force_zk`/`force_vector`, validate that the program actually uses the matching syscalls/opcodes (or automatically infer them) to avoid silently lying to hosts.

## Quick Wins (Low Risk, High Impact)
- Promote the access-hint emitter to log when dynamic map keys or host-driven reads are encountered so authors understand the remaining blind spots.
- Introduce a compiler flag (and future default) that fails builds when SIMD/vector opcodes are used but the header bit would be incorrect; this keeps hosts safe while the rest of the feature detection lands.

## Known Limitations to Call Out in Docs
- Access hints cover static ISI targets but still miss dynamic map keys and opaque helper syscalls, so schedulers need fallback analysis for complex contracts.
- SIMD/vector requirements are detected automatically now, but there is no diagnostic when authors force bits unnecessarily; hosts may still want to enforce policy out-of-band.
- Custom numeric types (e.g., `fixed_u128`) behave as opaque tokens; arithmetic on them is rejected unless cast to `int`, which diverges from the published grammar.
- `permission(...)` annotations are only enforced via compiler linting; runtimes do not receive this metadata, so operators must enforce dispatcher checks out-of-band.

Keeping these limitations explicit helps set expectations and aids contributors in targeting the most valuable next steps.
