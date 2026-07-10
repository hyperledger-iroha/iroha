# Kotodama V1 language specification

This document is the single normative source-language specification for the
first Kotodama release. Translations, examples, editor grammars, generated
tables, and compiler behavior are subordinate to it; a disagreement in the
Rust compiler is an implementation bug.

The machine-readable documentation policy is
[`kotodama_v1_docs.json`](./kotodama_v1_docs.json). Pull-request CI extracts
every tracked `kotodama` or `ko` source fence, plus every documented
`cat > *.ko` heredoc, below its configured roots and checks the unique source
contents with the canonical Rust `koto` driver.

Kotodama compiles to deterministic Iroha Virtual Machine bytecode (`.to`). It is not a standalone RISC-V language. ABI version 1 is the only release ABI.

## Lexical grammar

Source is UTF-8 and V1 identifiers are ASCII. Keywords are case-sensitive.
Four branded declaration features each have a romanized Japanese spelling and
its exact Japanese-language equivalent: `seiyaku`/`誓約`,
`kotoage`/`言挙げ`, `hajimari`/`始まり`, and `kaizen`/`改善`.
Those eight spellings are first-class keywords, not compatibility aliases.
Other non-ASCII text is permitted only inside strings and comments; English
`contract`, `entry`, `init`, and `upgrade` are not Kotodama V1 keywords.

```ebnf
identifier      = (ASCII-letter | "_") (ASCII-letter | ASCII-digit | "_")* ;
integer-literal = decimal-literal | hexadecimal-literal | binary-literal ;
decimal-literal = ASCII-digit (ASCII-digit | "_")* ;
hexadecimal-literal = "0x" hex-digit (hex-digit | "_")* ;
binary-literal  = "0b" ("0" | "1" | "_")+ ;
string-literal  = '"' string-character* '"' ;
bytes-literal   = "b" string-literal ;
comment         = "//" non-newline-character* ;
```

String escapes are `\\`, `\"`, `\n`, `\r`, `\t`, `\0`, `\xNN`, and
`\u{...}`. Raw and raw-byte strings preserve their contents without escape
processing. Decimal fractions and exponent notation are not numeric literals in
V1.

The compiler applies the same mandatory frontend budgets in every driver: at
most 1 MiB (1,048,576 UTF-8 bytes) per source file, 250,000 significant tokens
including end-of-file, and 256 levels of syntactic nesting. The nesting budget
is shared by active delimiters, generic type arguments, unary-prefix chains,
and conditional-expression structure; combining individually shallow forms
cannot evade it. Inputs beyond a budget fail with stable `K0001`, `K0002`, or
`K0003` diagnostics before resolution or code generation. Parsing and cleanup
at the inclusive boundary use explicit work stacks and must not consume the
native call stack in proportion to source nesting.

## Source units

A deployable file contains exactly one named contract. A reusable file contains exactly one named module. A file cannot contain both, and source units cannot be nested.

```ebnf
source          = seiyaku | module ;
seiyaku         = seiyaku-keyword identifier "{" seiyaku-item* "}" ;
seiyaku-keyword = "seiyaku" | "誓約" ;
module          = "module" identifier "{" module-item* "}" ;

seiyaku-item    = struct | error-enum | constant | state | function | kotoage
                | view | hajimari | kaizen | trigger ;
module-item     = struct | error-enum | constant | function ;
```

Contract identity is the declared name; the compiler must preserve it through CST, AST, HIR, diagnostics, interfaces, and documentation. Modules are linked at typed HIR. Textual AST rewriting and wildcard imports are not part of V1.

The following spellings and forms are errors: English declaration words
`contract`, `entry`, `init`, and `upgrade`; implicit `main`; raw `call`
statements; source-level `messages`/`kotoba` localization tables; source macros
such as `account!` or `json!`; and multiple deployable units in one file.
Contract failures use declared numeric error enums; presentation-layer
localization is outside the deployable language. Constructors are ordinary
typed calls (`AccountId::parse("...")`, `Json::parse("{...}")`); bytes use
`b"..."`. There is no compatibility parser or edition switch.

## Declarations

```ebnf
struct          = "struct" identifier "{" field (("," | ";") field)* ("," | ";")? "}" ;
field           = identifier ":" type ;

error-enum      = "error" "enum" identifier "{"
                  error-variant (("," | ";") error-variant)* ("," | ";")?
                  "}" ;
error-variant   = identifier "=" integer-literal ;

constant        = "const" identifier ":" type "=" expression ";" ;
state           = "state" identifier ":" type ";" ;

function        = "fn" identifier parameters return-type? block ;
kotoage         = kotoage-keyword "fn" identifier parameters return-type?
                  "authorize" "(" string-literal ")" block ;
kotoage-keyword = "kotoage" | "言挙げ" ;
view            = "view" "fn" identifier parameters return-type?
                  authorization? block ;
hajimari        = hajimari-keyword parameters block ;
hajimari-keyword = "hajimari" | "始まり" ;
kaizen          = kaizen-keyword parameters block ;
kaizen-keyword  = "kaizen" | "改善" ;
authorization   = "authorize" "(" string-literal ")" ;
parameters      = "(" (parameter ("," parameter)*)? ")" ;
parameter       = identifier ":" type ;
return-type     = "->" type ;

trigger         = "trigger" identifier "->" trigger-call "{"
                  trigger-filter trigger-option* "}" ;
trigger-call    = identifier ("::" identifier)? ;
trigger-filter  = "on" (time-filter | execute-filter | data-filter | pipeline-filter) ";"? ;
trigger-option  = "repeats" ("indefinitely" | integer-literal) ";"
                | "authority" (identifier | string-literal) ";"
                | "metadata" "{" metadata-entry* "}" ";"? ;
metadata-entry  = (identifier | string-literal) ":" expression ";" ;
time-filter     = "time" ("pre_commit" | "schedule" "(" integer-literal
                  ("," integer-literal)? ")") ;
execute-filter  = "execute" "trigger" (identifier | string-literal) ;
data-filter     = "data" ("any" | identifier identifier "{" data-matcher* "}") ;
data-matcher    = identifier (identifier | string-literal) ";" ;
pipeline-filter = "pipeline" ("transaction" | "block") "approved"? ;
```

Every parameter, field, constant, and state declaration has an explicit type. Every error variant has an explicit, non-zero `u32` code, and names and codes are unique within the enum. Missing types, unknown types, duplicate declarations, reserved names, ambiguous resolution, recursive value types, duplicate parameters, and shadowing are compile errors.

A `kotoage fn`/`言挙げ fn` mutates or submits ledger state and always declares
caller authorization. Authorization is checked at runtime and is separate from
compiler-derived effects and operation-specific host authorization. Views are
public unless they add `authorize`. Lifecycle declarations never accept
source-level authorization: ABI V1 requires the runtime-defined
`CanRegisterSmartContractCode` permission for both `hajimari`/`始まり` and
`kaizen`/`改善`.

CLI and Torii callers may submit a JSON object keyed by parameter name. At that boundary, tooling validates it against the exact compiler-emitted `EntrypointArgumentSchemaV1` and converts it to a schema-bound `EntrypointArgumentRecordV1`. The VM receives only that canonical Norito record, decodes it once, and then reads a fixed table of typed ABI words; JSON is never the VM argument transport. Struct parameters use exact JSON objects, tuples use exact arrays, `Option<T>` uses exactly `{"some": value}` or `{"none": true}`, and `Result<T,E>` uses exactly `{"ok": value}` or `{"err": value}`. Unknown fields, duplicate schema names, alternate tags, non-canonical typed identifiers, schema-hash mismatches, and malformed typed atoms are rejected. A `Json` parameter is still a named field in the record and does not receive the whole outer boundary object.

The V1 function-call convention has a fixed 13-word argument window (`r10` through `r22`). Every scalar or pointer leaf consumes one word, `Option<T>` and `Result<T,E>` additionally consume one tag word, and structs and tuples recursively consume the sum of their fields. A function whose complete flattened parameter list exceeds 13 words is rejected with `K2007`; arguments are never truncated and V1 has no hidden stack-argument convention.

Self-describing IVM trigger actions select their callback explicitly with
`contract_entrypoint` action metadata. When an event fires, the runtime validates
that event's arguments against the selected callback schema and constructs the
same canonical Norito record before starting the VM. Trigger metadata may not
provide a fixed `contract_payload`, and host operations never interpret a null
pointer or numeric zero as a request to re-read JSON trigger arguments.

## Bindings and assignment

`let` creates an immutable local. `var` creates a mutable local. Assigning to a `let`, parameter, constant, or immutable field is an error. Redeclaring or shadowing a name in an enclosing scope is an error.

```ebnf
binding         = "let" identifier (":" type)? "=" expression ";"
                | "var" identifier (":" type)? "=" expression ";" ;
assignment      = place ("=" | "+=" | "-=" | "*=" | "/=" | "%=") expression ";" ;
```

Every local binding is initialized at its declaration; there is no
uninitialized-local state in V1.

## Types

The V1 type vocabulary is:

- `i64`
- `u128`
- `bool`
- `string`
- `bytes`
- `Amount`
- `Json`
- typed Iroha identifiers, including `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, `DomainId`, `DataSpaceId`, and `Name`
- declared structs and tuples
- `Option<T>`
- `Result<T, E>`
- `StateMap<K, V>`
- `Secret<T>` inside ZK contracts, subject to the information-flow rules below

`Opaque`, `int`, `number`, `fixed_u128`, `String`, `Blob`, `Bytes`, `Balance`, and in-memory `Map` are not types in V1.
Unit is an internal function-return state, not a source type: `()` and `(T)` are
errors in type position. Omit the return type for a Unit-returning function;
source tuple types always contain at least two elements.

```ebnf
type            = "i64" | "u128" | "bool" | "string" | "bytes"
                | "Amount" | "Json" | iroha-id-type | identifier
                | tuple-type | "Option" "<" type ">"
                | "Result" "<" type "," type ">"
                | "StateMap" "<" type "," type ">"
                | "Secret" "<" type ">" ;
tuple-type      = "(" type "," type ("," type)* ")" ;
iroha-id-type   = "AccountId" | "AssetDefinitionId" | "AssetId"
                | "NftId" | "DomainId" | "DataSpaceId" | "Name" ;
```

There are no implicit conversions. Numeric operations require identical operand
types. Boolean values are not integers, and `Amount` remains nominally distinct
from `u128` even though both use a canonical Norito numeric payload at the ABI.
Pointer-ABI constructors return their declared typed ID or data type and cannot
be substituted for one another.

An unsuffixed integer literal (or one suffixed with adjacent `i64`) has type
`i64`. A `u128` literal requires the adjacent `u128` suffix and may cover the
complete range `0..=340282366920938463463374607431768211455`. Values above that
range are lexical errors; a separated spelling such as `1 u128` is not a
suffix. Canonical explicit conversions are `u128::from_i64(value)`,
`Amount::from_i64(value)`, `Amount::from_u128(value)`, and
`numeric::to_i64(value)`. Converting a negative `i64` to an unsigned type
fails; there is no implicit literal, assignment, argument, return, comparison,
or arithmetic conversion.

## Control flow

V1 supports `if`/`else`, `return`, and compiler-proven bounded `for` loops. It rejects `while`, recursion, indirect source calls, and loops whose bound cannot be proven.

Collection iteration is deterministic and limited to 64 items. `StateMap`
iteration follows canonical Norito key order and must use `.take(end)` or
`.range(start, end)` with non-negative `i64` literals whose resulting span is
at most 64. The former `#[bounded(N)]` spelling is not V1 syntax; `#[test]` is
the only source attribute and is accepted only by test-mode tooling. `break`
and `continue` are valid only inside an accepted bounded loop.

`&&` and `||` short-circuit. The right operand is evaluated only when required.

```ebnf
block           = "{" statement* "}" ;
statement       = binding | assignment | expression ";" | return-statement
                | if-statement | for-statement | "break" ";" | "continue" ";" ;
return-statement = "return" expression? ";" ;
if-statement    = "if" expression block ("else" (block | if-statement))? ;
for-statement   = "for" identifier "in" "range" "(" expression ")" block
                | "for" "(" identifier "," identifier ")" "in"
                  bounded-collection block ;
bounded-collection = expression "." ("take" "(" i64-literal ")"
                   | "range" "(" i64-literal "," i64-literal ")") ;

expression      = conditional ;
conditional     = logical-or ("?" expression ":" expression)? ;
logical-or      = logical-and ("||" logical-and)* ;
logical-and     = comparison ("&&" comparison)* ;
comparison      = additive (("==" | "!=" | "<" | "<=" | ">" | ">=") additive)* ;
additive        = multiplicative (("+" | "-") multiplicative)* ;
multiplicative  = unary (("*" | "/" | "%") unary)* ;
unary           = ("!" | "-") unary | postfix ;
postfix         = primary (("." identifier) | ("[" expression "]")
                | call-arguments)* ;
primary         = i64-literal | u128-literal | string-literal | bytes-literal
                | "true" | "false" | qualified-name
                | qualified-name call-arguments | "(" expression ")"
                | tuple-expression ;
i64-literal     = integer ("i64")? ;
u128-literal    = integer "u128" ;
qualified-name  = identifier ("::" identifier)* ;
call-arguments  = "(" (expression ("," expression)*)? ")" ;
tuple-expression = "(" expression "," expression ("," expression)* ")" ;
```

`(expression)` is grouping and does not construct a one-element tuple. Bare
`()` is not a source expression; use `return;` when a function returns no
value. Tuple expressions, like tuple types, contain at least two elements.

The grammar above describes source control flow, not an escape hatch around the
bounded-loop rule. The compiler must prove the effective trip count and reject
any collection traversal that could exceed 64 items.

## Arithmetic

Arithmetic is checked by default. Overflow, underflow, division by zero, remainder by zero, and negating `i64::MIN` produce a deterministic contract error and revert effects. Compile-time folding follows the same rules.

Intentional modular arithmetic is written with explicit operations such as `math::wrapping_add`, `math::wrapping_sub`, `math::wrapping_mul`, and `math::wrapping_neg`. Ordinary operators never silently wrap.

Relational operators use signed ordering for `i64` and unsigned ordering for
`u128`. `numeric::neg` is not defined for `u128`. The ABI validates every
numeric arithmetic operand and result against the `u128` integer domain, so a
crafted arbitrary-precision `Numeric` payload cannot widen source-level
`u128`; subtraction underflow, addition or multiplication overflow, and zero
divisors fail deterministically.

## Durable state

Scalar contract state must be initialized by `hajimari`/`始まり` on every
successful initializer path before it can be observed. `StateMap` is
host-backed and does not require allocation in `hajimari`.

`StateMap.get` returns `Option<V>`; absence is not represented by a zero, empty string, or implicit default. Rvalue indexing such as `map[key]` and compound indexed assignment such as `map[key] += value` are errors because both would read a possibly absent value without handling `Option<V>`. Simple `map[key] = value` remains the canonical per-key write form. The flat spelling `get(map, key)` is not a StateMap operation; only the receiver form `map.get(key)` invokes the intrinsic, while an unrelated user-declared function named `get` resolves normally. `StateMap.remove(key)` returns the removed `Option<V>` and is not permitted in views. Every scalar or aggregate state root and every `StateMap` value is encoded once as one canonical, schema-bound record under one durable key. Its domain-separated schema hash covers the exact type and named-field layout; mismatched schemas, malformed typed leaves, null active pointers, and non-canonical inactive sum branches are rejected. Nested `StateMap` values and unsupported leaves are compile errors. V1 map keys may be `i64`, `u128`, `Amount`, `bool`, `string`, `bytes`, or a typed Iroha identifier; aggregate, `Json`, optional, result, secret, and nested-map keys are rejected. Physical V1 key paths use reversible lowercase hexadecimal canonical-Norito bytes, so path order is canonical key-byte order without hash-collision ambiguity. Keys and map bases are capped at 4 KiB, and iteration pages are canonical and limited to 64 items.

Compiler-derived access metadata is advisory until independently verified from bytecode. Unknown, dynamic, incomplete, or transitively unresolved access forces conservative scheduler serialization.

## Errors and requirements

Contracts declare error enums. A requirement has the form `require(condition, Error::Variant)`. Error variants compile to stable contract codes included in the public interface. Free-form failure strings are not part of the release contract.

```kotodama
seiyaku Vault {
    error enum VaultError {
        ZeroDeposit = 1,
        NotReady = 2,
    }

    state balance: i64;

    hajimari() {
        balance = 0;
    }

    kotoage fn deposit(amount: i64) authorize("CanDeposit") {
        require(amount > 0, VaultError::ZeroDeposit);
        balance = balance + amount;
    }

    view fn ready() -> bool {
        return balance > 0;
    }
}
```

Assertions intended only for local tests or diagnostics must not replace public contract errors.

## Local test mode

`#[test]` functions, `fixture` declarations, `koto_test` targets, and the
`test::` builtin namespace exist only when the compiler driver explicitly
selects test mode. A production `check` or `build` rejects these constructs
with `E_TEST_ONLY_PRODUCTION`; it never removes them silently before semantic
analysis or artifact hashing. Typed HIR records whether test capabilities were
enabled, and production code generation rejects test-capable HIR as well.

The `koto test` driver is the explicit test-mode boundary. It compiles the full
suite in test mode, then derives a test-free runtime contract for invocation;
that runner-only derivation is not available to ordinary production builds.

## Namespaced host API

Source code uses namespaced capabilities. Representative roots are:

- `context::authority`, `context::block_height`, and other immutable call context
- `ledger::asset::transfer`, `ledger::asset::mint`, and `ledger::asset::burn`
- `ledger::account::set_detail`
- `state::get`, `state::set`, and `state::delete`
- `crypto::sha256`, `crypto::sha3`, and signature/proof operations
- `math::wrapping_add`, `math::isqrt`, and other deterministic arithmetic helpers
- `debug::info` for diagnostics
- `test::assert` and `test::assert_eq` in test builds only

Flat aliases are errors. Allocation, heap growth, raw pointers, direct syscall variants, opaque instruction submission, and compiler `*_direct` helpers are not source APIs. The canonical builtin registry defines each capability's signature, effect, syscall, access behavior, gas class, and permitted execution modes.

## Secrets and ZK contracts

A ZK contract explicitly requests the ZK execution capability through its build configuration. Private input is represented only as `Secret<T>`. The V1 private-input syscall supplies one 64-bit word, so `Secret<i64>` is the only concrete secret payload in this release.

Secret values may flow only to approved commitment, proof, and cryptographic declassification operations. The V1 declassifiers are `crypto::poseidon2`, `crypto::poseidon6`, `crypto::pubkgen`, and `crypto::valcom`; when one scalar input is secret, every scalar input must be secret. Their result is public.

Secrets cannot influence public control flow, public returns, logs, error selection, state keys, state values, ledger writes, host queries, contract calls, ordinary arithmetic, comparisons, collection indices, or assertions. They cannot appear in public parameters or return types. A raw secret is never a nullifier or public commitment: `crypto::use_nullifier` accepts only an already-public commitment.

The compiler performs fail-closed information-flow analysis across the complete call graph.

## Resource limits

The compiler rejects inputs exceeding any V1 hard limit:

| Resource | Limit |
|---|---:|
| UTF-8 source | 1 MiB |
| Tokens, including EOF | 250,000 |
| Delimiter/parse nesting | 256 |
| Collection iteration | 64 items |
| Typed module graph | 512 sources / 16 MiB total |
| Default artifact cycle ceiling | 1,000,000 |

The node's configured admission ceiling is authoritative. The
`pipeline.ivm_max_cycles_upper_bound` setting is a mandatory positive integer
(default `1_000_000`); it is accepted only from the node configuration file,
configuration loading rejects zero, and neither environment variables nor
consensus custom parameters can override or disable it. The selected positive
cycle ceiling is embedded in the execution header and therefore covered by the
canonical artifact hash.

## Tooling and build configuration

`koto` is the only source-language command in V1:

```text
koto check contract.ko
koto build contract.ko --max-cycles 1000000
koto check --zk proof_contract.ko
koto build --zk proof_contract.ko
koto test contract.test.ko
koto fmt contract.ko
koto doc contract.ko
koto explain K0001
koto lsp
```

`--zk` is an explicit build capability, not source metadata. It is required for
`Secret<T>` and the approved proof/commitment operations; ordinary builds reject
those constructs. It does not make ABI, vector, or pointer policy selectable.

`koto fmt` and LSP formatting consume the compiler's lossless token stream.
They refuse syntactically invalid input, preserve comments and literal spelling,
and canonicalize four-space indentation, declaration spacing, operators, and
block layout. Formatting is idempotent and fails rather than producing a source
larger than the mandatory 1 MiB limit. `koto fmt --check` performs no writes.
LSP validation uses the check pipeline, so reusable `module` files are analyzed
without the deployable-contract-only artifact pass. A multi-source
`koto check --format json|sarif` invocation emits exactly one machine-readable
document with the combined, deterministically ordered diagnostic set. LSP
framing, individual documents, open-document count, and aggregate retained text
all have explicit bounds; rejected updates are not retained as stale formatter
input.

The test driver supports deterministic discovery and selection:

```text
koto test list contract.test.ko
koto test run --filter exact_test_name --exact --jobs 4 --seed 7 contract.test.ko
koto test run --format json contract.test.ko
koto test run --junit target/kotodama-tests.xml contract.test.ko
koto test run --zk zk_contract.test.ko
```

The Rust compiler library behind `koto` is canonical. `iroha contract dev` and
Musubi call that library in process. Content-addressed build authentication runs
before parsing or typed-HIR linking, so an unchanged project performs no
compiler work and rewrites no outputs. Node.js calls the compiler asynchronously
through `iroha_js_host`; browsers use an explicit compiler-service client. SDK
adapters enforce the same 1 MiB UTF-8 source limit before native or network
dispatch. There is no independent JavaScript compiler or offline browser
compiler.

ABI version, vector width, execution-mode bits, and compiler features are not source declarations or user-selectable language metadata. Build configuration may request a permitted execution capability such as ZK and may select a positive cycle ceiling no greater than node admission policy. Source-level `meta` blocks are errors.

## Contract artifact

The canonical `code_hash` is a domain-separated hash of the complete deployable `.to` image: every execution-header field, the embedded contract interface (CNTR), typed literals, and executable code.

Debug information and source maps are forbidden inside deployable artifacts. They are hash-keyed sidecars whose `artifact_hash` identifies the exact `.to` image.

Nodes validate direct control-flow targets, allowed ABI-v1 syscalls, pointer-ABI types, interface structure, code/ABI hashes, and signed manifest equality. Compiler fingerprints are informational and are not security claims.

## Example

```kotodama
seiyaku Counter {
    state value: i64;

    hajimari() {
        value = 0;
    }

    kotoage fn increment(delta: i64) -> i64 authorize("CanIncrementCounter") {
        let next = value + delta;
        value = next;
        return next;
    }

    view fn current() -> i64 {
        return value;
    }
}
```

The repository documentation check discovers and compiles every tracked
`kotodama` or `ko` code fence and documented `*.ko` heredoc below the roots in
`kotodama_v1_docs.json`. Grammar-derived keyword and operator tables feed
documentation, formatting, syntax highlighting, and LSP completion so those
surfaces cannot define independent dialects.
