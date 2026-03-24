# Kotodama Language: Syntax and Grammar

This document specifies the current Kotodama language syntax as implemented in the repository and outlines planned extensions. The goal is clarity and safety: readable contracts, explicit control flow, and predictable, deterministic execution.

Bytecode Target
---------------
Kotodama compiles to Iroha Virtual Machine (IVM) bytecode (`.to`) for execution by the IVM. It does not target “risc5”/RISC‑V as a standalone architecture. Where RISC‑V–like encodings are referenced in implementation notes, they are part of IVM’s mixed instruction format and remain an implementation detail; observable behavior and outputs are defined by IVM.

Status tags used below:
- [Implemented]: parsed and used in lowering/codegen today
- [Parsed]: tokens/constructs recognized by the lexer/parser but not yet fully compiled
- [Planned]: part of the language design but not implemented yet

## Current Feature Matrix

Legend: ✅ Implemented, 🟨 Parsed, 💤 Planned

| Construct                                   | Status | Notes |
|---------------------------------------------|:------:|-------|
| Free function `fn`                          |  ✅    | fully compiled |
| Contract container `seiyaku`/`誓約`         |  ✅    | parsed; `meta {}` block sets header fields |
| Initializer `hajimari`/`始まり`             |  ✅    | compiled as function; runtime may call on deploy |
| Public function `kotoage fn`/`言挙げ fn`     |  ✅    | compiled as function; “public” enforced by runtime |
| Upgrade hook `kaizen`/`改善` + `permission` |  🟨    | parsed; governance/dispatch enforced by runtime |
| `kotoba {…}` translations                    |  🟩    | parsed; emitted in manifest |
| Access hints `#[access(read=..., write=...)]`|  ✅    | collected into manifest/entrypoint hints |
| Trigger declarations `register_trigger {…}`  |  ✅    | time/execute/data/pipeline trigger DSL with metadata and explicit authority; manifest triggers auto-register on activation |
| `state Type name;`                           |  ✅    | host-backed durable overlays (Norito TLV persistence + checkpoint/restore rollback) |
| `struct Name { … }`                          |  ✅    | lowered to tuple layout; field access compiled |
| `let name[: Type] = expr;`                   |  ✅    | optional type annotation |
| Tuple pattern `let (a,b) = expr;`            |  ✅    | binds both to expr; no field extract yet |
| `if/else`, `while`, `for` (C‑style)          |  ✅    | lowered to branches/jumps |
| Ternary conditional `cond ? then : else`     |  ✅    | lowered to branches/jumps |
| Range sugar `for i in range(N)`              |  ✅    | lowered to C‑style init/cond/step |
| For‑each `for (k, v) in map`                 |  ✅    | deterministic 2‑iteration lowering |
| `Map::new()`                                 |  ✅    | heap alloc via SYSCALL_ALLOC (zero-initializes key/value) |
| Map indexing `m[i]` get/set                  |  ✅    | minimal layout at offsets 0/8 |
| Field access (tuple index)                   |  ✅    | `t.0`/`t.1` lowered |
| Field access (named)                         |  ✅    | basic: struct literal or bound var (1-level) |
| Return values                                |  ✅    | single value; `r10` convention |
| Tuple return `-> (T1,T2)`                    |  ✅    | multi-return lowering → `Return2/ReturnN` + CallMulti codegen |
| Builtins → syscalls (assets/NFT/detail)      |  ✅    | pointer‑ABI mapping |
| Domain builtins (register/unregister/transfer) |  ✅    | literal args; typed Norito TLVs |
| Pointer constructors (AccountId/Name/Json/…) |  ✅    | literal args; typed Norito TLVs (includes DataSpaceId/AxtDescriptor/AssetHandle/ProofBlob; Blob args decode via `pointer_from_norito`) |
| Contract‑style `call transfer_asset(...)`    |  ✅    | normalized to builtin |
| Contract‑style `call set_account_detail(...)`|  ✅    | normalized to builtin |
| Boolean literals `true`/`false`              |  ✅    | typed as `bool` |
| Logical ops `&&` `||`                        |  ✅    | precedence implemented |
| Modulo operator `%`                          |  ✅    | lowered via M-extension REM |
| User‑defined fn calls (resolver/codegen)     |  ✅    | compiled with JAL/JALR; r10 return convention |

See examples under `crates/ivm/docs/examples`.

## Lexical Elements [Implemented]
- Identifiers: ASCII letters, digits, underscore; must not start with a digit.
- Numbers: decimal/hex/binary integers with `_` separators; decimal fractions (e.g., `1.25`) are accepted for numeric aliases (no exponent).
- Strings: double-quoted with escapes (`\\n`, `\\r`, `\\t`, `\\0`, `\\xNN`, `\\u{...}`, `\\"`, `\\\\`).
- Raw strings: `r"..."` or `r#"..."#` (no escapes, multiline allowed).
- Byte strings: `b"..."` with escapes, or raw `br"..."` / `rb"..."`; yield `bytes` literals.
- Comments: `//` to end of line.
- Whitespace: ignored except for tracking line/column in diagnostics.
- Keywords: `fn`, `let`, `if`, `else`, `while`, `for`, `seiyaku`/`誓約`, `hajimari`/`始まり`, `kaizen`/`改善`, `kotoage`/`言挙げ`.
- Punctuation: `+`, `++`, `-`, `*`, `/`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `?`, `:`, `(`, `)`, `{`, `}`, `;`, `,`.

Notes:
- Boolean literals `true`/`false` are recognized and type‑checked as `bool`. [Implemented]
- `++` is only usable in `for` headers as increment sugar. [Implemented]
- Decimal fractions (e.g., `1.25`) are parsed but rejected in v1; numeric aliases (`fixed_u128`/`Amount`/`Balance`) accept unsigned integer literals only (scale = 0).

## Program Structure

### Top Level [Implemented/Parsed]
```
Program   = { Item } ;
Item      = FreeFunction               // [Implemented]
          | Contract                   // [Parsed]
          | PublicFunctionShortcut ;   // [Parsed]

FreeFunction = "fn" Ident "(" [ NameList ] ")" Block ;          // params are names only
NameList     = Ident { "," Ident } ;

AccessAttr = "#[" "access" "(" AccessList ")" "]" ;
AccessList = AccessEntry { "," AccessEntry } ;
AccessEntry = ("read" | "write") "=" (String | "[" StringList "]") ;

PublicFunctionShortcut = [AccessAttr] "kotoage" "fn" FunctionSignature Block ; // outside any contract

Contract   = "seiyaku" Ident "{" { ContractItem } "}" ;
ContractItem = StructDef                                            // [Parsed]
             | StateDecl                                            // [Parsed]
             | MetaBlock                                            // [Implemented]
             | TriggerDecl                                          // [Implemented]
             | KotobaBlock                                          // [Parsed]
             | [AccessAttr] ( "fn" | "kotoage" "fn" ) FunctionSignature [ Permission ] Block // [Parsed]
             | "hajimari" "(" ")" Block                            // [Parsed]
             | "kaizen"   "(" [ ParamList ] ")" [ Permission ] Block ; // [Parsed]

StructDef  = "struct" Ident "{" { FieldDecl } "}" ;
FieldDecl  = Type Ident [ "," ] ;
StateDecl  = "state" Type Ident ";" ;
KotobaBlock = "kotoba" "{" { /* translation entries */ } "}" ;
Permission = "permission" "(" Ident ")" ;
MetaBlock  = "meta" "{" { MetaEntry ";" } "}" ;                // [Implemented]
MetaEntry  = ( "abi_version" | "abi" ) ":" Number
           | ( "vector_length" | "vl" ) ":" Number
           | ( "max_cycles" | "cycles" ) ":" Number
           | "zk" ":" ( "true" | "false" )
           | "vector" ":" ( "true" | "false" ) ;

TriggerDecl = ("register_trigger" | "trigger") Ident "{" { TriggerField } "}" ;
TriggerField = "call" CallTarget ";"                         // callback target
             | "on" TriggerFilter [ ";" ]                    // event filter
             | "repeats" Repeats ";"                         // repetition policy
             | "authority" (Ident | String) ";"             // explicit trigger authority
             | "metadata" "{" { MetadataEntry ";" } "}" [ ";" ] ;
CallTarget = Ident [ "::" Ident ] ;
TriggerFilter = "time" ("pre_commit" | "schedule" "(" Number [ "," Number ] ")")
              | "execute" "trigger" (Ident | String)
              | "data" "any"
              | "data" DataFamily DataEventKind DataMatcherBlock
              | "pipeline" ("transaction" | "block" | "merge" | "witness") ;
DataFamily = "peer"
           | "domain"
           | "account"
           | "asset"
           | "asset_definition"
           | "nft"
           | "trigger"
           | "role"
           | "configuration"
           | "executor" ;
DataEventKind = Ident | "any" ;
DataMatcherBlock = "{" { DataMatcher ";" } "}" ;
DataMatcher = "peer" (Ident | String)
            | "domain" (Ident | String)
            | "account" (Ident | String)
            | "asset" (Ident | String)
            | "asset_definition" (Ident | String)
            | "nft" (Ident | String)
            | "trigger" (Ident | String)
            | "role" (Ident | String) ;
Repeats = "indefinitely" | Number ;
MetadataEntry = (Ident | String) ":" Literal ;               // JSON literal (string/number/bool/null/json!)

FunctionSignature = Ident "(" [ ParamList ] ")" ;                  // see Parameters
```

Contract-level forms and `kotoage fn` are parsed. Contract metadata declared via `meta {}` influences IVM header fields (ABI version, vector length, max cycles) and mode bits (ZK/VECTOR). Trigger declarations attach filters, metadata, and optional explicit authority to entrypoint manifests and are auto-registered when a contract instance is activated (removed on deactivation); cross-contract callbacks are currently rejected. Free functions are fully compiled end-to-end today. See Gap Analysis for details.

Structured data-trigger example:
```kotodama
register_trigger cbuae_aed_to_pkr_asset_added {
  call run;
  on data asset added {
    asset_definition "66owaQmAQMuHxPzxUN3bqZ6FJfDa";
  }
}
```

Notes:
- `asset` data filters may combine both `asset` and `asset_definition` matchers; both apply with logical AND semantics.
- `configuration` and `executor` filters currently expose event-kind selection only and do not accept matcher fields.
- The DSL covers the core ledger data families above. Specialized product-specific event families still use the lower-level filter APIs when needed.

Accepted data event kinds by family:
- `peer`: `any`, `added`, `removed`
- `domain`: `any`, `created`, `deleted`, `asset_definition`, `nft`, `account`, `account_linked`, `account_unlinked`, `metadata_inserted`, `metadata_removed`, `owner_changed`, `kaigi_roster_summary`, `kaigi_relay_registered`, `kaigi_relay_manifest_updated`, `kaigi_usage_summary`, `kaigi_relay_health_updated`, `streaming_ticket_ready`, `streaming_ticket_revoked`
- `account`: `any`, `created`, `deleted`, `asset`, `permission_added`, `permission_removed`, `role_granted`, `role_revoked`, `metadata_inserted`, `metadata_removed`, `repo`
- `asset`: `any`, `created`, `deleted`, `added`, `removed`, `metadata_inserted`, `metadata_removed`
- `asset_definition`: `any`, `created`, `deleted`, `metadata_inserted`, `metadata_removed`, `mintability_changed`, `mintability_changed_detailed`, `total_quantity_changed`, `owner_changed`
- `nft`: `any`, `created`, `deleted`, `metadata_inserted`, `metadata_removed`, `owner_changed`
- `trigger`: `any`, `created`, `deleted`, `extended`, `shortened`, `metadata_inserted`, `metadata_removed`
- `role`: `any`, `created`, `deleted`, `permission_added`, `permission_removed`
- `configuration`: `any`, `changed`
- `executor`: `any`, `upgraded`

### Parameters & Returns [Parsed/Enforced]
```
ParamList  = Param { "," Param } ;
Param      = [ Type Ident ] | Ident ;
Type       = Ident ; // placeholder, not validated or used yet
ReturnTy   = "->" Type ;
```

- The parser accepts `Type Name` pairs and an optional return type `-> T` everywhere and records them on the AST. [Parsed]
- Bare parameter names are still accepted and treated as typeless. [Implemented]
- If a function declares a non-`unit` return type, all control‑flow paths must return a value. [Enforced]
- If a function omits a return type, `return expr;` is rejected — declaring an explicit return type is required to return a value. [Enforced]
- Parameter typing:
  - Recognized primitives: `int`/`i64`/`number`, `bool`, `string`, and numeric aliases (`fixed_u128`, `Amount`, `Balance`) are enforced in expressions. Numeric aliases are distinct `Numeric`-backed scalars restricted to unsigned, scale‑0 values; decimal literals are rejected in v1. Arithmetic preserves the alias, and mixing alias types requires converting through an `int` binding. Conversions to/from `int` are checked at runtime (range‑limited, non‑negative).
  - Unknown identifiers (e.g., `AccountId`, `Asset`) are treated as opaque handle types; they cannot be used in arithmetic but can be compared for equality. [Enforced]

## Blocks and Statements [Implemented]
```
Block     = "{" { Statement } "}" ;

Statement = LetStmt
          | AssignStmt
          | IfStmt
          | WhileStmt
          | ForStmt
          | BreakStmt
          | ContinueStmt
          | Expr ";" ;

LetStmt   = "let" Pattern [ ":" Type ] "=" Expr ";" ;
Pattern   = Ident | "(" Ident "," Ident ")" ; // tuple pattern (2‑tuple supported)
AssignStmt = Ident "=" Expr ";" ;
BreakStmt = "break" ";" ;
ContinueStmt = "continue" ";" ;

IfStmt    = "if" Expr Block [ "else" Block ] ;

WhileStmt = "while" Expr Block ;

ForStmt   = "for" ForHeader Block ;
ForHeader = // C‑style header
            [ ForInit ] ";" [ Expr ] ";" [ ForStep ]
          | // range sugar (0..N)
            Ident "in" "range" "(" Expr ")" ;

ForInit   = LetStmt | Expr ;
ForStep   = Ident "++" | "++" Ident | Expr ;
```

Notes and limitations:
- `x = y;`, `return`, `break`, and `continue` parse and lower; codegen for control-flow is implemented with regression coverage across `while` and `for range` loops.

### Statements & Expressions (additions)
- Let with optional type annotation: `let name[: Type] = Expr;` and tuple destructuring of length 2. [Implemented]
- Calls: `call name(args...) ;` — explicit external call statement. First‑slice normalization maps known names to builtins (e.g., `transfer_asset`, `set_account_detail`). [Implemented]
- Events/logging: `info(message_expr);` — builtin. [Implemented]
- Map/Indexing: `expr[index]` — map indexing is typed and lowered; general indexing is restricted. [Implemented]
- Field access: `expr.field` — tuple index and basic named struct fields lowered (1‑level). [Implemented]
- For‑each: `for (a, b) in map_expr { ... }` — lowered to a deterministic two‑iteration expansion. [Implemented]
- Modulo: `%` and `%=` lower to REM with regression coverage. [Implemented]
- Tuples: return types may be tuples `-> (T1, T2)` and `return (e1, e2);` — parsed; typing/codegen WIP. [Parsed]

## Expressions [Implemented]
```
Expr    = Conditional ;
Conditional = Or [ "?" Conditional ":" Conditional ] ;
Or      = And  { "||" And } ;
And     = Cmp  { "&&" Cmp } ;
Cmp     = Sum  { ("==" | "!=" | "<" | "<=" | ">" | ">=") Sum } ;
Sum     = Prod { ("+" | "-") Prod } ;
Prod    = Unary{ ("*" | "/") Unary } ;
Unary   = [ "-" | "!" ] Primary ;
Primary = Number
        | String
        | "true" | "false"
        | Ident "(" [ ArgList ] ")"   // call to builtin or future user fn
        | Ident
        | "(" Expr ")" ;
ArgList = Expr { "," Expr } ;
```

Built-in calls recognized by the semantic layer (arity and types enforced):
- ZK/crypto: `poseidon2(a, b)`, `poseidon6(a,b,c,d,e,f)`, `pubkgen(s)`, `valcom(v, r)`, `assert_eq(x, y)`.
- Vector helpers: `setvl(n)` (compile-time int `0..=255`).
- Iroha syscalls: `mint_asset(acc, asset, amount)`, `burn_asset(acc, asset, amount)`, `transfer_asset(from, to, asset, amount)`, `register_asset(name, symbol, quantity, mintable)`, `create_new_asset(name, symbol, quantity, account, mintable)`, `nft_mint_asset(id, owner)`, `nft_transfer_asset(from, id, to)`, `nft_set_metadata(id, json)`, `nft_burn_asset(id)`.
- Trigger syscalls: `create_trigger(json)`, `register_trigger(json)` (alias), `remove_trigger(name)`/`unregister_trigger(name)`, `set_trigger_enabled(name, enabled)`.
 - Iroha helpers (samples/dev): `create_nfts_for_all_users()`, `set_execution_depth(value)`, `set_account_detail(account, key, value)`.
 - Durable state helpers (host): `host::state_get(name_path) -> Blob`, `host::state_set(name_path, norito_bytes_value)`, `host::state_del(name_path)`.
 - Encoding helpers (WIP): `encode_int(int) -> Blob`, `decode_int(Blob) -> int`.
- Inline ZK ISI builders (literal-only): `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk)`, `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk)`. All arguments must be compile-time literals (string literals or pointer constructors from literals). `nullifier32` and `inputs32` must be exactly 32 bytes (raw string or `0x` hex), and `amount` must be non-negative.
- Path builder (host): `host::path_map_key(base: Name, key: int) -> Name` builds canonical `"<base>/<key>"`.
 - VRF helpers: `vrf_verify(input, pk, proof, variant) -> Blob`, `vrf_verify_batch(batch: Blob) -> Blob` (returns 0 on failure). [Implemented]
 - Pointer utilities: `schema_info(schema: Name) -> Json {id,version}`, `pointer_to_norito(ptr)` (wrap any pointer-ABI TLV into NoritoBytes for state storage or replay). [Implemented]
 - Typed constructors for pointer-ABI values: `account_id("6cmzPVPX...")`, `asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`, `asset_id("norito:<hex-asset-id>")`, `nft_id("n0$wonderland")`, `name("cursor")`, `json("{\"k\":\"v\"}")`, `blob("...")`, `norito_bytes("...")`, plus AXT/Nexus types `dataspace_id("0x…")`, `axt_descriptor("0x…")`, `asset_handle("0x…")`, and `proof_blob("0x…")`. String literals (plain or `0x`-hex) emit typed Norito TLVs into the data section; passing a `Blob`/`NoritoBytes` decodes the TLV at runtime via `pointer_from_norito`.
 - AXT helpers: `axt_begin(AxtDescriptor)`, `axt_touch(DataSpaceId[, manifest: Blob])`, `verify_ds_proof(DataSpaceId[, ProofBlob])`, `use_asset_handle(AssetHandle, Blob intent[, ProofBlob])`, `axt_commit()`. [Implemented]
 - Contextual value: `authority()` returns the current `AccountId` under which the contract executes.

User‑defined function calls are compiled: arguments move into `ARG_REGS`, calls use `JAL ra,<target>`, and returns use `r10` (and `r11` for tuple of two) with `JALR x0, x1, 0` from non‑entry functions. [Implemented]

Notes on helpers:
- `create_nfts_for_all_users()` creates one NFT per known account using a host-provided snapshot (used by tests).
- `set_execution_depth(value)` sets SmartContract execution depth parameter (host development helper).
- `setvl(n)` emits a `SETVL` opcode with an 8-bit immediate; `n` must be a compile-time int in `0..=255` (0 maps to 1 in the VM).
- `set_account_detail(account, key, value)` uses a pointer-ABI. Prefer passing typed values using `account_id(..)`, `name(..)`, and `json(..)`. The `authority()` builtin supplies the current account id without relying on numeric sentinels. NFT syscalls use typed pointers: pass a `nft_id("name$domain")` and an `account_id("...")`.

## Safety Model (in progress)
- Determinism: execution order and gas accounting are deterministic; parallelism preserves program order for observable state commits.
- Total resource usage: loops and recursion must be bounded by gas; there is no unmetered IO.

## State (Ephemeral + Durable)
- Syntax: contract-level `state Type name;` inside `seiyaku {}`.
- Current lowering: on function entry, the compiler allocates ephemeral storage:
  - `Map<K,V>`: heap-allocates a minimal fixed-layout map and binds `name`.
  - `struct` fields: recursively allocate per-field storage and bind `name#idx` for field access via `name.field`.
  - scalar/other: initialize to 0 and bind `name`.
- Durable host overlay: ABI v1 programs issue `STATE_GET/SET/DEL`. CoreHost stages writes/deletes per transaction (read‑your‑writes), then flushes the TLV payloads into WSV‑backed `smart_contract_state` after execution; prepass access logging uses the same overlay without persistence.
- Capability gating: Iroha operations are exposed as explicit syscalls; permission modeling at the language level (e.g., `permission(...)`) is planned.
- No raw pointers or references: the language intentionally avoids `*`/`&` semantics and aliasing rules to reduce foot‑guns in financial contracts. Memory is managed implicitly by the runtime.

## Examples
Minimal free function that compiles today:
```
fn add(a, b) {
    let c = a + b;
}
```

Contract skeleton (parsed today; bodies not yet compiled):
```
seiyaku MyDex {
    state LiquidityPool pool;
    struct LiquidityPool { Asset asset_a, Asset asset_b, fixed_u128 reserve_a, fixed_u128 reserve_b }
    kotoba { "invalid_assets": { en: "Invalid assets" } }
    hajimari() { /* init code */ }
    kaizen(address new_impl) permission(GovernanceSudo) { /* upgrade code */ }
    kotoage fn deposit(AccountId who, Amount a) { /* body */ }
    fn internal_helper() { /* body */ }
}
```

## Notes on the Compiler/IR
- Source is type-checked into a small set of types (int, bool, string, unit, plus numeric aliases `fixed_u128`/`Amount`/`Balance` that behave as distinct `Numeric` scalars) and lowered to a simple three-address-code IR with basic blocks.
- Code generation currently covers arithmetic (incl. `%`), control-flow (`if`/`while`/`for` with `break`/`continue`), structs/tuples, and the host-backed durable state/syscall surface. Remaining gaps are tracked in `status.md`.

See also: docs/kotodama_gap_analysis.md for an up-to-date gap analysis and roadmap items.
