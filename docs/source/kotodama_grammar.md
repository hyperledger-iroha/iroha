# Kotodama Language Grammar and Semantics

This document specifies the Kotodama language syntax (lexing, grammar), typing rules, deterministic semantics, and how programs lower to IVM bytecode (.to) with Norito pointer-ABI conventions. Kotodama sources use the .ko extension. The compiler emits IVM bytecode (.to) and can optionally return a manifest.

Contents
- Overview and Goals
- Lexical Structure
- Types and Literals
- Declarations and Modules
- Contract Container and Metadata
- Functions and Parameters
- Statements
- Expressions
- Builtins and Pointer-ABI Constructors
- Collections and Maps
- Deterministic Iteration and Bounds
- Errors and Diagnostics
- Codegen Mapping to IVM
- ABI, Header, and Manifest
- Roadmap

## Overview and Goals

- Deterministic: Programs must produce identical results across hardware; no floating point or nondeterministic sources. All host interactions happen through syscalls with Norito-encoded arguments.
- Portable: Targets Iroha Virtual Machine (IVM) bytecode, not a physical ISA. RISC‑V–like encodings visible in the repository are implementation details of IVM decoding and must not change observable behavior.
- Auditable: Small, explicit semantics; clear mapping of syntax to IVM opcodes and to host syscalls.
- Boundedness: Loops over unbounded data must carry explicit bounds. Map iteration has strict rules to guarantee determinism.

## Lexical Structure

Whitespace and comments
- Whitespace separates tokens and is otherwise insignificant.
- Line comments start with `//` and run to end-of-line.
- Block comments `/* ... */` do not nest.

Identifiers
- Start: `[A-Za-z_]` then continue `[A-Za-z0-9_]*`.
- Case-sensitive; `_` is a valid identifier but discouraged.

Keywords (reserved)
- `seiyaku`, `hajimari`, `kotoage`, `kaizen`, `state`, `struct`, `fn`, `let`, `const`, `return`, `if`, `else`, `while`, `for`, `in`, `break`, `continue`, `true`, `false`, `permission`, `kotoba`.

Operators and punctuation
- Arithmetic: `+ - * / %`
- Bitwise: `& | ^ ~`, shifts `<< >>`
- Compare: `== != < <= > >=`
- Logical: `&& || !`
- Assign: `= += -= *= /= %= &= |= ^= <<= >>=`
- Misc: `: , ; . :: ->`
- Brackets: `() [] {}`

Literals
- Integer: decimal (`123`), hex (`0x2A`), binary (`0b1010`). All integers are signed 64-bit at runtime; literals without suffix are typed via inference or as `int` by default.
- String: double-quoted with escapes (`\n`, `\r`, `\t`, `\0`, `\xNN`, `\u{...}`, `\"`, `\\`); UTF‑8. Raw strings `r"..."` or `r#"..."#` disable escapes and allow newlines.
- Bytes: `b"..."` with escapes, or raw `br"..."` / `rb"..."`; yields a `bytes` literal.
- Boolean: `true`, `false`.

## Types and Literals

Scalar types
- `int`: 64-bit two’s-complement; arithmetic wraps modulo 2^64 for add/sub/mul; division has defined signed/unsigned variants in IVM; the compiler chooses the appropriate op for semantics.
- `fixed_u128`, `Amount`, `Balance`: numeric aliases backed by Norito `Numeric` (signed decimal with up to 512-bit mantissa and scale). Kotodama treats these aliases as non-negative quantities; arithmetic is checked, preserves the alias, and traps on overflow or division by zero. Values created from `int` use scale 0; conversions to/from `int` are range-checked at runtime (non-negative, integral, fits in i64).
- `bool`: logical truth value; lowered to `0`/`1`.
- `string`: immutable UTF‑8 string; represented as Norito TLV when passed to syscalls; in-VM operations use byte slices and length.
- `bytes`: raw Norito payload; aliases the pointer-ABI `Blob` type for hashing/crypto/proof inputs and durable overlays.

Composite types
- `struct Name { field: Type, ... }` user-defined product types. Constructors use call syntax `Name(a, b, ...)` in expressions. Field access `obj.field` is supported and lowers to tuple-style positional fields internally. Durable state ABI on-chain is Norito-encoded; the compiler emits overlays that mirror the struct order and recent tests (`crates/iroha_core/tests/kotodama_struct_overlay.rs`) keep the layout locked in across releases.
- `Map<K, V>`: deterministic associative map; semantics restrict iteration and mutations during iteration (see below).
- `Tuple (T1, T2, ...)`: anonymous product type with positional fields; used for multi-return.

Special pointer-ABI types (host-facing)
- `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`, `Blob`, and similar are not first-class runtime types. They are constructors that yield typed, immutable pointers into the INPUT region (Norito TLV envelopes) and can only be used as syscall arguments or moved between variables without mutation.

Type inference
- Local `let` bindings infer type from initializer. Function parameters must be explicitly typed. Return types may be inferred unless ambiguous.

## Declarations and Modules

Top-level items
- Contracts: `seiyaku Name { ... }` contain functions, state, structs, and metadata.
- Multiple contracts per file are allowed but discouraged; one primary `seiyaku` is used as default entry in manifests.
- `struct` declarations define user types within a contract.

Visibility
- `kotoage fn` denotes a public entrypoint; visibility affects dispatcher permissions, not codegen.
- Optional access hints: `#[access(read=..., write=...)]` can precede `fn`/`kotoage fn` to supply manifest read/write keys. The compiler also emits advisory hints automatically; opaque host calls fall back to conservative wildcard keys (`*`) and surface a diagnostic unless explicit access hints are provided, so schedulers can opt into a dynamic prepass for finer-grained keys.

## Contract Container and Metadata

Syntax
```
seiyaku Name {
  meta {
    abi_version: 1,
    vector_length: 0,
    max_cycles: 0,
    features: ["zk", "simd"],
  }

  state int counter;

  hajimari() { counter = 0; }

  kotoage fn inc() { counter = counter + 1; }
}
```

Semantics
- `meta { ... }` fields override compiler defaults for the emitted IVM header: `abi_version`, `vector_length` (0 means unset), `max_cycles` (0 means compiler default), `features` toggles header feature bits (ZK tracing, vector announce). The compiler treats `max_cycles: 0` as “use default” and emits the configured non‑zero default to satisfy admission requirements. Unsupported features are ignored with a warning. When `meta {}` is omitted, the compiler emits `abi_version = 1` and uses the option defaults for the remaining header fields.
- `features: ["zk", "simd"]` (aliases: `"vector"`) explicitly requests the corresponding header bits. Unknown feature strings now produce a parser error instead of being ignored.
- `state` declares durable contract variables. The compiler lowers accesses into `STATE_GET/STATE_SET/STATE_DEL` syscalls and the host stages them in a per-transaction overlay (checkpoint/restore rollback, flush-on-commit into WSV). Access hints are emitted for literal state paths; dynamic keys fall back to map-level conflict keys. For explicit host-backed reads/writes, use the `state_get/state_set/state_del` helpers and the `get_or_insert_default` map helpers; these route through Norito TLVs and keep names/field order stable.
- State identifiers are reserved; shadowing a `state` name in parameters or `let` bindings is rejected (`E_STATE_SHADOWED`).
- State map values are not first-class: use the state identifier directly for map operations and iteration. Binding or passing state maps to user-defined functions is rejected (`E_STATE_MAP_ALIAS`).
- Durable state maps currently support `int` and pointer-ABI key types only; other key types are rejected at compile time.
- Durable state fields must be `int`, `bool`, `Json`, `Blob`/`bytes`, or pointer-ABI types (including structs/tuples composed of these fields); `string` is not supported for durable state.

### Kotoba localization
Syntax
```
kotoba {
  "E_UNBOUNDED_ITERATION": { en: "Loop over map lacks a bound." }
}
```

Semantics
- `kotoba` entries attach translation tables to the contract manifest (`kotoba` field).
- Message IDs and language tags accept identifiers or string literals; entries must be non-empty.
- Duplicate `msg_id` + language tag pairs are rejected at compile time.

## Trigger Declarations

Trigger declarations attach scheduling metadata to entrypoint manifests and are auto-registered
when a contract instance is activated (removed on deactivation). They are parsed inside a
`seiyaku` block.

Syntax
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  authority "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland";
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

Notes
- `call` must reference a public `kotoage fn` entrypoint in the same contract; an optional
  `namespace::entrypoint` is recorded in the manifest but cross-contract callbacks are rejected
  by the runtime for now (local callbacks only).
- Supported filters: `time pre_commit` and `time schedule(start_ms, period_ms?)`, plus
  `execute trigger <name>` for by-call triggers, `data any`, and pipeline filters
  (`pipeline transaction`, `pipeline block`, `pipeline merge`, `pipeline witness`).
- `authority` optionally overrides the trigger authority (AccountId string literal). If omitted,
  the runtime uses the contract-activation authority.
- Metadata values must be JSON literals (`string`, `number`, `bool`, `null`) or `json!(...)`.
- Runtime-injected trigger metadata keys: `contract_namespace`, `contract_id`,
  `contract_entrypoint`, `contract_code_hash`, `contract_trigger_id`.

## Functions and Parameters

Syntax
- Declaration: `fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- Public: `kotoage fn name(...) { ... }`
- Initializer: `hajimari() { ... }` (invoked on deploy by the runtime, not by the VM itself).
- Upgrade hook: `kaizen(args...) permission(Role) { ... }`.

Parameters and returns
- Arguments are passed in registers `r10..r22` as values or INPUT pointers (Norito TLV) per ABI; additional args spill to stack.
- Functions return zero or one scalar or tuple. Primary return value is in `r10` for scalar; tuples are materialized in stack/OUTPUT by convention.

## Statements

- Variable bindings: `let x = expr;`, `let mut x = expr;` (mutability is a compile-time check; runtime mutation is allowed for locals only).
- Assignment: `x = expr;` and compound forms `x += 1;` etc. Targets must be variables or map indices; tuple/struct fields are immutable.
- Numeric aliases (`fixed_u128`, `Amount`, `Balance`) are distinct `Numeric`-backed types; arithmetic preserves the alias and mixing aliases requires converting through an `int` binding. Conversions to/from `int` are checked at runtime (non-negative, integral, range-limited).
- Control: `if (cond) { ... } else { ... }`, `while (cond) { ... }`, C-style `for (init; cond; step) { ... }`.
  - `for` initializers and steps must be simple `let name = expr` or expression statements; complex destructuring is rejected (`E0005`, `E0006`).
  - `for` scoping: bindings from the init clause are visible in the loop and after it; bindings created in the body or step do not escape the loop.
- Equality (`==`, `!=`) is supported for `int`, `bool`, `string`, pointer-ABI scalars (e.g., `AccountId`, `Name`, `Blob`/`bytes`, `Json`); tuples, structs, and maps are not comparable.
- Map loop: `for (k, v) in map { ... }` (deterministic; see below).
- Flow: `return expr;`, `break;`, `continue;`.
- Call: `name(args...);` or `call name(args...);` (both accepted; compiler normalizes to call statements).
- Assertions: `assert(cond);`, `assert_eq(a, b);` map to IVM `ASSERT*` in non-ZK builds or ZK constraints in ZK mode.

## Expressions

Precedence (high → low)
1. Member/index: `a.b`, `a[b]`
2. Unary: `! ~ -`
3. Multiplicative: `* / %`
4. Additive: `+ -`
5. Shifts: `<< >>`
6. Relational: `< <= > >=`
7. Equality: `== !=`
8. Bitwise AND/XOR/OR: `& ^ |`
9. Logical AND/OR: `&& ||`
10. Ternary: `cond ? a : b`

Calls and tuples
- Calls use positional arguments: `f(a, b, c)`.
- Tuple literal: `(a, b, c)` and destructure: `let (x, y) = pair;`.
- Tuple destructuring requires tuple/struct types with matching arity; mismatches are rejected.

Strings and bytes
- Strings are UTF‑8; raw string and byte literal forms are accepted in source.
- Byte literals (`b"..."`, `br"..."`, `rb"..."`) lower to `bytes` (Blob) pointers; wrap with `norito_bytes(...)` when a syscall expects NoritoBytes TLV payloads.

## Builtins and Pointer-ABI Constructors

Pointer constructors (emit Norito TLV into INPUT and return a typed pointer)
- `account_id(string) -> AccountId*`
- `asset_definition(string) -> AssetDefinitionId*`
- `asset_id(string) -> AssetId*`
- `domain(string) | domain_id(string) -> DomainId*`
- `name(string) -> Name*`
- `json(string) -> Json*`
- `nft_id(string) -> NftId*`
- `blob(bytes|string) -> Blob*`
- `norito_bytes(bytes|string) -> NoritoBytes*`
- `dataspace_id(string|0xhex) -> DataSpaceId*`
- `axt_descriptor(string|0xhex) -> AxtDescriptor*`
- `asset_handle(string|0xhex) -> AssetHandle*`
- `proof_blob(string|0xhex) -> ProofBlob*`

Prelude macros provide shorter aliases and inline validation for these constructors:
- `account!("ih58...")`, `account_id!("ih58...")`
- `asset_definition!("rose#wonderland")`, `asset_id!("rose#wonderland")`
- `domain!("wonderland")`, `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` or structured literals such as `json!{ hello: "world" }`
- `nft_id!("dragon$demo")`, `blob!("bytes")`, `norito_bytes!("...")`

The macros expand to the constructors above and reject invalid literals at compile time.

Implementation status
- Implemented: constructors above accept string literal arguments and lower to typed Norito TLV envelopes placed in the INPUT region. They return immutable typed pointers usable as syscall arguments. Non-literal string expressions are rejected; use `Blob`/`bytes` for dynamic inputs. `blob`/`norito_bytes` also accept `bytes`-typed values at runtime without macro shims.
- Extended forms:
  - `json(Blob[NoritoBytes]) -> Json*` via `JSON_DECODE` syscall.
  - `name(Blob[NoritoBytes]) -> Name*` via `NAME_DECODE` syscall.
  - Pointer decode from Blob/NoritoBytes: any pointer constructor (including AXT types) accepts a `Blob`/`NoritoBytes` payload and lowers to `POINTER_FROM_NORITO` with the expected type id.
  - Pass-through for pointer forms: `name(Name) -> Name*`, `blob(Blob) -> Blob*`, `norito_bytes(Blob) -> Blob*`.
  - Method sugar is supported: `s.name()`, `s.json()`, `b.blob()`, `b.norito_bytes()`.

Host/syscall builtins (map to SCALL; exact numbers in ivm.md)
- `mint_asset(AccountId*, AssetDefinitionId*, numeric)`
- `burn_asset(AccountId*, AssetDefinitionId*, numeric)`
- `transfer_asset(AccountId*, AccountId*, AssetDefinitionId*, numeric)`
- `set_account_detail(AccountId*, Name*, Json*)`
- `execute_instruction(Blob[NoritoBytes])`
- `execute_query(Blob[NoritoBytes]) -> Blob`
- `subscription_bill()`
- `subscription_record_usage()`
- `nft_mint_asset(NftId*, AccountId*)`
- `nft_transfer_asset(AccountId*, NftId*, AccountId*)`
- `nft_set_metadata(NftId*, Json*)`
- `nft_burn_asset(NftId*)`
- `authority() -> AccountId*`
- `trigger_event() -> Json*` (current trigger-event payload; data/by-call trigger context)
- `register_domain(DomainId*)`
- `unregister_domain(DomainId*)`
- `transfer_domain(AccountId*, DomainId*, AccountId*)`
- `vrf_verify(Blob, Blob, Blob, int variant) -> Blob`
- `vrf_verify_batch(Blob) -> Blob`
- `axt_begin(AxtDescriptor*)`
- `axt_touch(DataSpaceId*, Blob[NoritoBytes]? manifest)`
- `verify_ds_proof(DataSpaceId*, ProofBlob?)`
- `use_asset_handle(AssetHandle*, Blob[NoritoBytes], ProofBlob?)`
- `axt_commit()`
- `contains(Map<K,V>, K) -> bool`

Utility builtins
- `info(string|int)`: emits a structured event/message via OUTPUT.
- `hash(blob) -> Blob*`: returns a Norito-encoded hash as Blob.
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk) -> Blob*` and `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk) -> Blob*`: inline ISI builders; all arguments must be compile-time literals (string literals or pointer constructors from literals). `nullifier32` and `inputs32` must be exactly 32 bytes (raw string or `0x` hex), and `amount` must be non-negative.
- `schema_info(Name*) -> Json* { "id": "<hex>", "version": N }`
- `encode_schema(Name*, Json*) -> Blob`: encodes JSON using the host schema registry (DefaultRegistry supports `QueryRequest` and `QueryResponse` in addition to Order/Trade samples).
- `decode_schema(Name*, Blob|bytes) -> Json*`: decodes Norito bytes using the host schema registry.
- `pointer_to_norito(ptr) -> NoritoBytes*`: wraps an existing pointer-ABI TLV as NoritoBytes for storage or transport.
- `isqrt(int) -> int`: integer square root (`floor(sqrt(x))`) implemented as an IVM opcode.
- `min(int, int) -> int`, `max(int, int) -> int`, `abs(int) -> int`, `div_ceil(int, int) -> int`, `gcd(int, int) -> int`, `mean(int, int) -> int` — fused arithmetic helpers backed by native IVM opcodes (ceil division traps on divide-by-zero).

Notes
- Builtins are thin shims; the compiler lowers them to register moves and a `SCALL`.
- Pointer constructors are pure: the VM ensures the Norito TLV in INPUT is immutable for the call duration.
 - Structs with pointer-ABI fields (e.g., `DomainId`, `AccountId`) can be used to group syscall arguments ergonomically. The compiler maps `obj.field` to the correct register/value without extra allocations.

## Collections and Maps

Type: `Map<K, V>`
- In-memory maps (heap-allocated via `Map::new()` or passed as parameters) store a single key/value pair; keys and values must be word-sized types: `int`, `bool`, `string`, `Blob`, `bytes`, `Json`, or pointer types (e.g., `AccountId`, `Name`).
- Durable state maps (`state Map<...>`) use Norito-encoded keys/values. Supported keys: `int` or pointer types. Supported values: `int`, `bool`, `Json`, `Blob`/`bytes`, or pointer types.
- `Map::new()` allocates and zero-initializes the single in-memory entry (key/value = 0); for non-`Map<int,int>` maps, provide an explicit type annotation or return type.
- State maps are not first-class values: you cannot reassign them (e.g., `M = Map::new()`); update entries via indexing (`M[key] = value`).
- Operations:
  - Indexing: `map[key]` get/set value (set performed via host syscall; see runtime API mapping).
  - Existence: `contains(map, key) -> bool` (lowered helper; may be an intrinsic syscall).
  - Iteration: `for (k, v) in map { ... }` with deterministic order and mutation rules.

Deterministic iteration rules
- The iteration set is the snapshot of keys at loop entry.
- Order is strictly ascending byte-lexicographic order of Norito-encoded keys.
- Structural modifications (insert/remove/clear) to the iterated map during the loop cause a deterministic `E_ITER_MUTATION` trap.
- Boundedness is required: either a declared max (`@max_len`) on the map, an explicit attribute `#[bounded(n)]`, or an explicit bound using `.take(n)`/`.range(..)`; otherwise the compiler emits `E_UNBOUNDED_ITERATION`.

Bounds helpers
- `#[bounded(n)]`: optional attribute on the map expression, e.g. `for (k, v) in my_map #[bounded(2)] { ... }`.
- `.take(n)`: iterate the first `n` entries from the start.
- `.range(start, end)`: iterate entries in the half-open interval `[start, end)`. Semantics are equivalent to `start` and `n = end - start`.

Notes on dynamic bounds
- Literal bounds: `n`, `start`, and `end` as integer literals are fully supported and compile to a fixed number of iterations.
- Non-literal bounds: when the `kotodama_dynamic_bounds` feature is enabled in the `ivm` crate, the compiler accepts dynamic `n`, `start`, and `end` expressions and inserts runtime assertions for safety (non-negative, `end >= start`). Lowering emits up to K guarded iterations with `if (i < n)` checks to avoid extra body executions (default K=2). You can tune K programmatically via `CompilerOptions { dynamic_iter_cap, .. }`.
- Run `koto_lint` to inspect Kotodama lint warnings prior to compilation; the main compiler always proceeds with lowering after parsing and type-checking.
- Error codes are documented in [Kotodama Compiler Error Codes](./kotodama_error_codes.md); use `koto_compile --explain <code>` for quick explanations.

## Errors and Diagnostics

Compile-time diagnostics (examples)
- `E_UNBOUNDED_ITERATION`: loop over map lacks a bound.
- `E_MUT_DURING_ITER`: structural mutation of iterated map in loop body.
- `E_STATE_SHADOWED`: local bindings cannot shadow `state` declarations.
- `E_BREAK_OUTSIDE_LOOP`: `break` used outside a loop.
- `E_CONTINUE_OUTSIDE_LOOP`: `continue` used outside a loop.
- `E0005`: for-loop initializer is more complex than supported.
- `E0006`: for-loop step clause is more complex than supported.
- `E_BAD_POINTER_USE`: using a pointer-ABI constructor result where a first-class type is required.
- `E_UNRESOLVED_NAME`, `E_TYPE_MISMATCH`, `E_ARITY_MISMATCH`, `E_DUP_SYMBOL`.
- Tooling: `koto_compile` runs the lint pass before emitting bytecode; use `--no-lint` to skip or `--deny-lint-warnings` to fail the build on lint output.

Runtime VM errors (selected; full list in ivm.md)
- `E_NORITO_INVALID`, `E_OOB`, `E_UNALIGNED`, `E_SCALL_UNKNOWN`, `E_ASSERT`, `E_ASSERT_EQ`, `E_ITER_MUTATION`.

Error messages
- Diagnostics carry stable `msg_id`s that map to entries in `kotoba {}` translation tables when available.

## Codegen Mapping to IVM

Pipeline
1. Lexer/Parser produce AST.
2. Semantic analysis resolves names, checks types, and populates symbol tables.
3. IR lowering to a simple SSA-like form.
4. Register allocation to IVM GPRs (`r10+` for args/ret per calling convention); spills to stack.
5. Bytecode emission: mix of IVM-native and RV-compat encodings as allowed; metadata header emitted with `abi_version`, features, vector length, and `max_cycles`.

Mapping highlights
- Arithmetic and logic map to IVM ALU ops.
- Branching and control map to conditional branches and jumps; the compiler uses compressed forms where profitable.
- Memory for locals spills to the VM stack; alignment is enforced.
- Builtins lower to register moves and `SCALL` with 8-bit number.
- Pointer constructors place Norito TLVs into the INPUT region and produce their addresses.
- Assertions map to `ASSERT`/`ASSERT_EQ` which trap in non-ZK execution and emit constraints in ZK builds.

Determinism constraints
- No FP; no nondeterministic syscalls.
- SIMD/GPU acceleration is invisible to bytecode and must be bit-identical; compiler does not emit hardware-specific ops.

## ABI, Header, and Manifest

IVM header fields set by the compiler
- `version`: IVM bytecode format version (major.minor).
- `abi_version`: syscall table and pointer-ABI schema version.
- `feature_bits`: feature flags (e.g., `ZK`, `VECTOR`).
- `vector_len`: logical vector length (0 → unset).
- `max_cycles`: admission bound and ZK padding hint.

Manifest (optional sidecar)
- `code_hash`, `abi_hash`, metadata from `meta {}` block, compiler version, and build hints for reproducibility.

## Roadmap

- **KD-231 (Apr 2026):** add compile-time range analysis for iteration bounds so loops expose bounded access sets to the scheduler.
- **KD-235 (May 2026):** introduce a first-class `bytes` scalar distinct from `string` for pointer constructors and ABI clarity.
- **KD-242 (Jun 2026):** expand the builtin opcode set (hash / signature verification) behind feature flags with deterministic fallbacks.
- **KD-247 (Jun 2026):** stabilize error `msg_id`s and maintain the mapping in `kotoba {}` tables for localized diagnostics.
### Manifest Emission

- The Kotodama compiler API can return a `ContractManifest` alongside the compiled `.to` via `ivm::kotodama::compiler::Compiler::compile_source_with_manifest`.
- Fields:
  - `code_hash`: hash of the code bytes (excluding the IVM header and literals) computed by the compiler to bind the artifact.
  - `abi_hash`: stable digest of the allowed syscall surface for the program's `abi_version` (see `ivm.md` and `ivm::syscalls::compute_abi_hash`).
- Optional `compiler_fingerprint` and `features_bitmap` are reserved for toolchains.
- `entrypoints`: ordered list of exported entrypoints (public, `hajimari`, `kaizen`) including their required `permission(...)` strings and the compiler’s best-effort read/write key hints so admission logic and schedulers can reason about expected WSV access.
- The manifest is intended for admission-time checks and for registries; see `docs/source/new_pipeline.md` for lifecycle.
