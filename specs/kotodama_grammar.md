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

The following lexical tables are generated from
`crates/kotodama_lang/grammar/v1.lex`; edits belong in that machine-readable
grammar rather than in this rendered copy or an editor grammar.

<!-- BEGIN GENERATED: kotodama-v1-keywords -->
| Spelling | Token |
| --- | --- |
| `authorize` | `Authorize` |
| `break` | `Break` |
| `const` | `Const` |
| `continue` | `Continue` |
| `else` | `Else` |
| `enum` | `Enum` |
| `error` | `Error` |
| `false` | `False` |
| `fn` | `Fn` |
| `for` | `For` |
| `hajimari` | `Hajimari` |
| `始まり` | `Hajimari` |
| `if` | `If` |
| `in` | `In` |
| `kaizen` | `Kaizen` |
| `改善` | `Kaizen` |
| `kotoage` | `Kotoage` |
| `言挙げ` | `Kotoage` |
| `let` | `Let` |
| `match` | `Match` |
| `module` | `Module` |
| `return` | `Return` |
| `seiyaku` | `Seiyaku` |
| `誓約` | `Seiyaku` |
| `state` | `State` |
| `struct` | `Struct` |
| `trigger` | `Trigger` |
| `true` | `True` |
| `var` | `Var` |
| `view` | `View` |
<!-- END GENERATED: kotodama-v1-keywords -->

<!-- BEGIN GENERATED: kotodama-v1-operators -->
| Spelling |
| --- |
| `+` |
| `-` |
| `*` |
| `/` |
| `%` |
| `==` |
| `!=` |
| `<` |
| `<=` |
| `>` |
| `>=` |
| `&&` |
| `\|\|` |
| `!` |
| `=` |
| `+=` |
| `-=` |
| `*=` |
| `/=` |
| `%=` |
| `->` |
| `=>` |
| `::` |
| `.` |
| `,` |
| `:` |
| `;` |
| `?` |
| `#` |
| `(` |
| `)` |
| `{` |
| `}` |
| `[` |
| `]` |
<!-- END GENERATED: kotodama-v1-operators -->

```ebnf
identifier      = (ASCII-letter | "_") (ASCII-letter | ASCII-digit | "_")* ;
integer-literal = decimal-literal | hexadecimal-literal | binary-literal ;
decimal-literal = ASCII-digit (ASCII-digit | "_")* ;
exact-decimal-literal = decimal-literal
                        (("." decimal-literal) exponent? | exponent) ;
exponent        = ("e" | "E") ("+" | "-")? decimal-literal ;
hexadecimal-literal = "0x" hex-digit (hex-digit | "_")* ;
binary-literal  = "0b" ("0" | "1" | "_")+ ;
string-literal  = '"' string-character* '"' ;
bytes-literal   = "b" string-literal ;
comment         = "//" non-newline-character* ;
```

String escapes are `\\`, `\"`, `\n`, `\r`, `\t`, `\0`, `\xNN`, and
`\u{...}`. Raw and raw-byte strings preserve their contents without escape
processing. Decimal fractions and decimal exponents are exact: they never
create a binary floating-point value. Separators are permitted only between
digits. Spellings such as `1.`, `.5`, `1__0`, and an exponent without digits
are invalid. Leading zeroes are valid in source numeric literals: `0007` is
base ten (never octal), and leading zeroes in decimal, hexadecimal, or binary
coefficients do not change the mathematical value or its domain check.
Likewise, `0001.2300` is normalized by the exact-decimal rules. Source
acceptance does not weaken canonical external encodings: typed numeric JSON
strings and numeric pointer payloads reject leading or otherwise redundant
zeroes. V1 has no numeric suffixes.

The compiler applies the same mandatory frontend budgets in every driver: at
most 1 MiB (1,048,576 UTF-8 bytes) per source file, 250,000 significant tokens
including end-of-file, and 256 levels of syntactic nesting. The nesting budget
is shared by active delimiters, generic type arguments, unary-prefix chains,
and conditional-expression structure; combining individually shallow forms
cannot evade it. Inputs beyond a budget fail with stable `K0001`, `K0002`, or
`K0003` diagnostics before resolution or code generation. Parsing and cleanup
at the inclusive boundary use explicit work stacks and must not consume the
native call stack in proportion to source nesting.

Named value types share the 256-level limit after resolution. Semantic
analysis measures each acyclic dependency DAG leaf-first, then resolves every
accepted named struct exactly once into an immutable shared product graph.
Parameters, returns, state declarations, constants, and expression checks
reuse that canonical graph; referring to a type never clones or recursively
re-expands its fields. A source is rejected with `K2008` if a conceptual
expanded shape exceeds 256 levels or if all conceptual expanded local struct
shapes exceed 250,000 type nodes. Branching DAGs and repeated references
therefore cannot amplify compact source into unbounded compiler work.

## Source units

A deployable file contains exactly one named `seiyaku`/`誓約`. A reusable file contains exactly one named module. A file cannot contain both, and source units cannot be nested.

```ebnf
source          = seiyaku | module ;
seiyaku         = seiyaku-keyword identifier "{" seiyaku-item* "}" ;
seiyaku-keyword = "seiyaku" | "誓約" ;
module          = "module" identifier "{" module-item* "}" ;

seiyaku-item    = struct | error-enum | constant | state | function | kotoage
                | view | hajimari | kaizen | trigger ;
module-item     = struct | error-enum | constant | function ;
```

Seiyaku identity is the declared name; the compiler must preserve it through CST, AST, HIR, diagnostics, interfaces, and documentation. Modules are linked at typed HIR. Textual AST rewriting and wildcard imports are not part of V1.

The following spellings and forms are errors: English declaration words
`contract`, `entry`, `init`, and `upgrade`; implicit `main`; raw `call`
statements; source-level `messages`/`kotoba` localization tables; source macros
such as `account!` or `json!`; and multiple deployable units in one file.
Seiyaku failures use declared numeric error enums; presentation-layer
localization is outside the deployable language. Constructors are ordinary
typed calls (`AccountId::parse("...")`, `Json::parse("{...}")`); bytes use
`b"..."`. There is no compatibility parser or edition switch.

## Declarations

```ebnf
struct          = "struct" identifier "{" (field (("," | ";") field)* ("," | ";")?)? "}" ;
field           = type identifier ;

error-enum      = "error" "enum" identifier "{"
                  error-variant (("," | ";") error-variant)* ("," | ";")?
                  "}" ;
error-variant   = identifier "=" integer-literal ;

constant        = "const" type identifier "=" expression ";" ;
state           = "state" type identifier ";" ;

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
parameter       = type identifier ;
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

Every parameter, field, constant, and state declaration has an explicit type.
Declaration types always precede names; the retired `name: Type` form is a
syntax error with a type-first diagnostic. Every error variant has an explicit,
non-zero code representable as `int`, and names and codes are unique within the
enum. Missing types, unknown types, duplicate declarations, reserved names,
ambiguous resolution, recursive value types, duplicate parameters, and
shadowing are compile errors.

A `kotoage fn`/`言挙げ fn` mutates or submits ledger state and always declares
caller authorization. Authorization is checked at runtime and is separate from
compiler-derived effects and operation-specific host authorization. Views are
public unless they add `authorize`. Lifecycle declarations never accept
source-level authorization: ABI V1 binds both `hajimari`/`始まり` and
`kaizen`/`改善` hook dispatch to the runtime-defined
`CanInvokeContractEntrypoint` permission. That hook permission does not grant
address lifecycle control: deployment creates the address atomically, while
later activation and deactivation require its account owner plus an exact
lifecycle revision or the certified Parliament corridor.

Lifecycle hooks are accepted only as top-level calls to a deployed seiyaku
instance. A hash-only governance stub can never become active: the complete
verified `.to` bytes and their exact signed manifest must exist before direct
or governance binding. Activating new code stages one consensus-owned
`hajimari`/`始まり`
transition when that declaration exists; rebinding an already-active address
to a different verified code hash stages one `kaizen`/`改善` transition when
the new artifact declares it. The exact pending transition and active code
binding are rechecked immediately before effects are applied, and a successful
hook consumes the transition atomically. While a transition is pending, all
other calls and views are rejected. Replaying a consumed hook, invoking
`kaizen` without an in-place code replacement, or selecting either lifecycle
hook from raw IVM, a trigger, or a nested seiyaku call is rejected. Seiyaku
units that omit the applicable declaration do not acquire a pending transition.

CLI and Torii callers may submit a JSON object keyed by parameter name. At that
boundary, tooling validates it against the exact compiler-emitted
`EntrypointArgumentSchemaV1` and converts it to a schema-bound
`EntrypointArgumentRecordV1`. The host retains that complete signed Norito
record. For prepared calls, it first validates the compiler-owned flat schema
and derives the schema's conservative maximum aggregate and pointer-allocation
bound. The signed wire lengths and that bound must be affordable before the
untrusted canonical record is decoded exactly once. The complete record stays
host-owned; the VM receives only a domain-separated binding plus a fixed table
of typed ABI words, and JSON is never the VM argument transport. The host
preflights the complete aligned allocation sequence before any allocation.
Pointer TLVs and the word table prefer INPUT and spill into owned HEAP; raw
`List` and sum storage is always owned HEAP. Raw decode-syscall quoting uses
only bounded record/schema envelope lengths and reserves the full HEAP before
authenticating either payload. The argument-record protocol cap is inclusive
at 1 MiB, while the selected artifact/node cycle ceiling still limits what an
invocation can afford. Struct
parameters use exact JSON objects, tuples use
exact arrays, `Option<T>` uses exactly `{"some": value}` or `{"none": true}`,
and `Result<T,E>` uses exactly `{"ok": value}` or `{"err": value}`. Decoding
either sum materializes one compiler-owned typed heap handle containing only
the active variant and payload; no inactive placeholder payload is constructed.
Unknown fields, duplicate schema names, alternate tags, non-canonical typed
identifiers, schema-hash mismatches, and malformed typed atoms are rejected. A
`Json` parameter is still a named field in the record and does not receive the
whole outer boundary object.

VM-to-VM invocation uses that same schema-bound record directly:
`CALL_CONTRACT` accepts `EntrypointArgumentRecordV1` Norito bytes, or a literal
zero only when the selected public or lifecycle declaration has no parameters. It never accepts or
reconstructs JSON. The host quotes authenticated envelope lengths and escrows
gas before copying or decoding the record, rejects private or tainted target,
selector, and argument registers, and decodes the canonical record exactly
once against the callee's signed schema.

Every non-unit public return has an exact flat-preorder
`EntrypointValueTypeV1` tape in CNTR and the signed manifest. Aggregate children
immediately follow their parent; a `List` node carries only its capacity and is
followed by exactly one element subtree. The valid V1 boundary is 256 nodes and
256 levels. Admission, decoding, materialization, and rendering use explicit
work stacks, so valid schemas never depend on the native call stack. Public
return handles are validated against that exact tape, including canonical sum
tags, list lengths and element schemas, typed pointer envelopes, UTF-8 and Norito
payloads, and ZK public-memory tags. Nested calls return one schema-hashed
`EntrypointReturnRecordV1`; JSON rendering happens only at Torii and CLI
boundaries. The complete returned `NoritoBytes` TLV is bounded to 1 MiB, so its
encoded record payload is at most 1,048,537 bytes after the exact 39-byte V1
TLV envelope. Cumulative pointer cloning is rejected before a repeated or
aliased large pointer can amplify memory use.

The V1 function-call convention has a fixed 13-word argument window (`r10`
through `r22`). Every scalar or pointer leaf consumes one word. `Option<T>`,
`Result<T,E>`, and `List<T,N>` each consume exactly one typed heap-handle word,
independent of payload width; sums do not occupy a separate tag register.
Structs and tuples consume the sum of their fields. A function
whose complete flattened parameter list exceeds 13 words is rejected with
`K2007`; arguments are never truncated and V1 has no hidden stack-argument
convention.

Self-describing IVM trigger actions select their callback explicitly with
`contract_entrypoint` action metadata. When an event fires, the runtime validates
that event's arguments against the selected callback schema and constructs the
same canonical Norito record before starting the VM. Trigger metadata may not
provide a fixed `contract_payload`, and host operations never interpret a null
pointer or numeric zero as a request to re-read JSON trigger arguments.

## Bindings and assignment

`let` creates an immutable local. `var` creates a mutable local. Assigning to a `let`, parameter, constant, or immutable field is an error. Redeclaring or shadowing a name in an enclosing scope is an error.

```ebnf
binding         = ("let" | "var") (type identifier | binding-pattern)
                  "=" expression ";" ;
binding-pattern = identifier | positional-destructure ;
positional-destructure = "(" identifier ("," identifier)* ")" ;
assignment      = place ("=" | "+=" | "-=" | "*=" | "/=" | "%=") expression ";" ;
```

Every local binding is initialized at its declaration; there is no
uninitialized-local state in V1. A positional destructure accepts a tuple or
a declared struct and must contain exactly one identifier for every element or
field. Tuple elements bind in tuple order; struct fields bind in declaration
order, not struct-literal source order. `_` discards that position. Duplicate
non-`_` names, a trailing comma, an arity mismatch, or a non-tuple/non-struct
initializer is an error. Destructuring is one level only: nested binding
patterns and named-field patterns such as `let Pair { left, right } = value;`
are not V1 syntax. `var` applies mutability independently to every non-`_`
binding produced by the pattern. Positional destructuring is inferred from its
initializer and has no separate type annotation in V1; ordinary single-name
annotations remain type-first.

## Types

The V1 type vocabulary is:

<!-- BEGIN GENERATED: kotodama-v1-source-policy -->
| Source policy | Canonical V1 values |
| --- | --- |
| Active type spellings | `int`, `decimal`, `quantity`, `bool`, `string`, `bytes`, `Json`, `AccountId`, `AssetDefinitionId`, `AssetId`, `DomainId`, `Name`, `NftId`, `DataSpaceId`, `Option`, `Result`, `List`, `StateMap`, `Secret`, `AccountView`, `AssetView`, `AssetDefinitionView`, `DomainView`, `NftView`, `QueryPage` |
| Forbidden in every source identifier position | `Amount` |
| Reserved retired numeric type spellings | `i8`, `i16`, `i32`, `i64`, `i128`, `isize`, `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `num`, `Int`, `Integer`, `float`, `f32`, `f64`, `Decimal`, `Fixed`, `FixedPoint`, `Amount`, `amount`, `money`, `Quantity`, `number` |
| Ordinary value/function identifier examples | `amount` |
| Retired literal suffixes with safe fix-its | `amt` (remove the suffix), `qty` (remove the suffix) |
| Durable `StateMap` key types (ordered) | `int`, `decimal`, `quantity`, `bool`, `string`, `bytes`, `DataSpaceId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, `DomainId`, `Name` |
| Dynamic-access bound kinds (ordered) | `range`, `take` |
| Dynamic-access key bound | `1..=64` |
| Dynamic-access base | One direct declared top-level `StateMap`, encoded as `state:<state_declaration_identifier>` |
| Dynamic-access scheduler semantics | Advisory only; never authorization or scheduler-authoritative evidence |

| Source type | Rust nominal type | Pointer ID | Schema name | Schema hash |
| --- | --- | --- | --- | --- |
| `int` | `Int` | `0x0011` | `iroha.numeric.IntValueV1` | `07c039457363b9e1d36bbd31d93dec4a` |
| `decimal` | `Decimal` | `0x0012` | `iroha.numeric.DecimalValueV1` | `ba2ffed52e4d8ee16f17efefe1828524` |
| `quantity` | `Quantity` | `0x0010` | `iroha.numeric.QuantityValueV1` | `e4769984c81ce0e8b678f2eb06274ee3` |

`0x0013` is unassigned and rejected as unknown; it is not an ABI tombstone.
<!-- END GENERATED: kotodama-v1-source-policy -->

- `int`
- `decimal`
- `quantity`
- `bool`
- `string`
- `bytes`
- `Json`
- typed Iroha identifiers, including `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, `DomainId`, `DataSpaceId`, and `Name`
- declared structs and tuples
- `Option<T>`
- `Result<T, E>`
- `List<T, N>`, where `N` is a compile-time capacity from 1 through 64
- `StateMap<K, V>`
- the compiler-declared `AccountView`, `AssetView`, `AssetDefinitionView`,
  `DomainView`, `NftView`, and `QueryPage<View>` query projections
- `Secret<T>` inside ZK seiyaku, subject to the information-flow rules below

`i64`, `u128`, `Int`, `Integer`, `Decimal`, `Fixed`, `FixedPoint`, `Amount`,
`Quantity`, `float`, `num`, `number`, `money`, `Opaque`, `fixed_u128`, `String`,
`Blob`, `Bytes`, `Balance`, and in-memory `Map` are not types in V1.
Retired type spellings are reserved only in the type namespace and in declared
type names. Except for exact `Amount`, which is forbidden in every identifier
position, the other retired type spellings remain available for ordinary
function, parameter, and local value names; for example,
`fn amount(quantity amount) -> quantity` is valid.
Unit is an internal function-return state, not a source type: `()` and `(T)` are
errors in type position. Omit the return type for a Unit-returning function;
source tuple types always contain at least two elements.

```ebnf
type            = "int" | "decimal" | "quantity" | "bool" | "string" | "bytes"
                | "Json" | iroha-id-type | identifier
                | tuple-type | "Option" "<" type ">"
                | "Result" "<" type "," type ">"
                | "List" "<" type "," capacity ">"
                | "QueryPage" "<" query-view-type ">"
                | "StateMap" "<" type "," type ">"
                | "Secret" "<" type ">" ;
capacity        = integer-literal ;
query-view-type = "AccountView" | "AssetView" | "AssetDefinitionView"
                | "DomainView" | "NftView" ;
tuple-type      = "(" type "," type ("," type)* ")" ;
iroha-id-type   = "AccountId" | "AssetDefinitionId" | "AssetId"
                | "NftId" | "DomainId" | "DataSpaceId" | "Name" ;
```

Boolean values are not integers. `quantity` is a nominal non-negative decimal
and cannot be substituted for `decimal`, even though both use exact base-10
arithmetic. Whole-number literals default to `int` and may be checked exactly
against an expected `decimal` or `quantity`; fractional and exponent literals
default to `decimal` and may be checked exactly against an expected `quantity`.
Negative contextual quantities are rejected at compile time.

Runtime `int` and `decimal` values never mix implicitly in arithmetic,
comparison, or compound assignment. Convert the `int` explicitly with
`decimal::from_int(value)` before operating in the decimal domain. Exact
numeric literals may still infer `decimal` or `quantity` from their expression
context because that compile-time choice inserts no runtime conversion. All
other cross-type conversions are also named and checked. In particular,
`quantity::try_from_decimal`, `decimal::from_quantity`, and exact, truncating,
or explicitly rounded decimal-to-int conversions make domain changes visible.
There is no implicit assignment, argument, return, arithmetic, comparison, or
ledger-boundary conversion between numeric runtime values. Pointer-ABI
constructors return their exact declared type and cannot be substituted for one
another.

## Control flow and expressions

V1 supports `if`/`else`, `return`, and compiler-proven bounded `for` loops. It rejects `while`, recursion, indirect source calls, and loops whose bound cannot be proven.

Collection iteration is deterministic and limited to 64 items. `StateMap`
iteration follows canonical Norito key order and must use `.take(end)` or
`.range(start, end)` with non-negative `int` literals whose resulting span is
at most 64. The former `#[bounded(N)]` spelling is not V1 syntax; `#[test]` is
the only source attribute and is accepted only by test-mode tooling. `break`
and `continue` are valid only inside an accepted bounded loop.

`&&` and `||` short-circuit. The right operand is evaluated only when required.

```ebnf
block           = "{" statement* tail-expression? "}" ;
tail-expression = expression ;
statement       = binding | assignment | expression ";" | return-statement
                | if-statement | if-let-statement | for-statement
                | "break" ";" | "continue" ";" ;
return-statement = "return" expression? ";" ;
if-statement    = "if" expression block ("else" (block | if-statement))? ;
if-let-statement = "if" "let" sum-pattern "=" expression block
                   ("else" block)? ;
for-statement   = "for" identifier "in" "range" "(" expression ")" block
                | "for" "(" identifier "," identifier ")" "in"
                  bounded-collection block ;
bounded-collection = expression "." ("take" "(" integer-literal ")"
                   | "range" "(" integer-literal "," integer-literal ")") ;

expression      = conditional ;
conditional     = logical-or ("?" expression ":" expression)? ;
logical-or      = logical-and ("||" logical-and)* ;
logical-and     = comparison ("&&" comparison)* ;
comparison      = additive (("==" | "!=" | "<" | "<=" | ">" | ">=") additive)* ;
additive        = multiplicative (("+" | "-") multiplicative)* ;
multiplicative  = unary (("*" | "/" | "%") unary)* ;
unary           = ("!" | "-") unary | postfix ;
postfix         = primary (("." identifier) | ("[" expression "]")
                | call-arguments | "?")* ;
primary         = integer-literal | exact-decimal-literal
                | string-literal | bytes-literal
                | "true" | "false" | qualified-name
                | qualified-name call-arguments | "(" expression ")"
                | tuple-expression | struct-literal | list-literal
                | list-comprehension | if-expression | if-let-expression
                | match-expression | sum-constructor | native-json ;
qualified-name  = identifier ("::" identifier)*
                | "state" "::" identifier
                | identifier "::" "trigger" "::" identifier
                | identifier "::" seiyaku-keyword "::" identifier
                | identifier "::" kotoage-keyword ;
call-arguments  = "(" (positional-arguments | named-arguments)? ")" ;
positional-arguments = expression ("," expression)* ","? ;
named-arguments = named-argument ("," named-argument)* ","? ;
named-argument  = (identifier | kotoage-keyword) ":" expression ;
tuple-expression = "(" expression "," expression ("," expression)* ")" ;
struct-literal  = identifier "{" (struct-field ("," struct-field)* ","?)? "}" ;
struct-field    = identifier (":" expression)? ;
list-literal    = "[" (expression ("," expression)* ","?)? "]" ;
list-comprehension = "[" expression "for" identifier "in" expression
                     ("if" expression)? "]" ;
if-expression   = "if" expression block "else" (block | if-expression) ;
if-let-expression = "if" "let" sum-pattern "=" expression block
                    "else" block ;
match-expression = "match" expression "{" match-arm ("," match-arm)* ","? "}" ;
match-arm       = sum-pattern "=>" (block | expression) ;
sum-pattern     = "Option::some" "(" (identifier | "_") ")"
                | "Option::none"
                | "Result::ok" "(" (identifier | "_") ")"
                | "Result::err" "(" (identifier | "_") ")" ;
sum-constructor = "Option::some" "(" expression ")" | "Option::none"
                | "Result::ok" "(" expression ")"
                | "Result::err" "(" expression ")" ;
native-json     = "json" (json-object | json-array) ;
json-object     = "{" (json-entry ("," json-entry)* ","?)? "}" ;
json-entry      = (identifier | string-literal) ":" expression ;
json-array      = "[" (expression ("," expression)* ","?)? "]" ;
```

`(expression)` is grouping and does not construct a one-element tuple. Bare
`()` is not a source expression; use `return;` when a function returns no
value. Tuple expressions, like tuple types, contain at least two elements.

A block's final expression has no semicolon and supplies the block value.
Functions, `if`/`if let`, and `match` all use the same tail rule; explicit
`return` remains available. `if` and `if let` require `else` when used as
values. Sum matches are exhaustive and use only the namespaced patterns above.
Postfix `?` propagates only the same `Option` family or the exact same `Result`
error type returned by the enclosing function; V1 performs no implicit error
conversion. The retired lowercase placeholder constructors are syntax errors
with active-only fix-its.

Calls never mix positional and named source arguments. Pagination is
named-only, as are privileged/effectful calls with at least three parameters
and signatures whose repeated parameter types are easy to transpose. Structs
are constructed only with named fields; `Type(a, b)` is retired.

The branded `seiyaku`/`誓約` and `kotoage`/`言挙げ` tokens are contextual in
canonical capability paths, and `kotoage`/`言挙げ` is contextual as the named
selector argument for those capabilities. They normalize to the romanized
registry spelling. These tokens remain reserved everywhere else: they cannot
be bindings, declarations, ordinary root namespaces, or compatibility aliases.

The grammar above describes source control flow, not an escape hatch around the
bounded-loop rule. The compiler must prove the effective trip count and reject
any collection traversal that could exceed 64 items.

## Arithmetic

Arithmetic is checked by default. Overflow, quantity underflow, division by
zero, remainder by zero, and negating the minimum `int` produce deterministic
seiyaku failures and revert effects. Compile-time folding calls the same exact
arithmetic implementation as runtime execution.

Intentional modular arithmetic is written with explicit operations such as `math::wrapping_add`, `math::wrapping_sub`, `math::wrapping_mul`, and `math::wrapping_neg`. Ordinary operators never silently wrap.

```text
math::wrapping_neg(int value) -> int
math::wrapping_add(int left, int right) -> int
math::wrapping_sub(int left, int right) -> int
math::wrapping_mul(int left, int right) -> int
```

The binary forms are named-only. These are the complete V1 modular-arithmetic
APIs; the corresponding flat names and all generic `numeric::*` helpers are
retired source spellings.

`int` is the signed range `-2^511..=2^511-1`; its compact encoding does not
change its semantic bounds. Division truncates toward zero, and remainder has
the dividend's sign. `min_int / -1` and the paired remainder operation fail
with overflow. Explicit wrapping helpers operate modulo `2^512`.

`decimal` and `quantity` use a signed 512-bit mantissa and canonical decimal
scale `0..=28`; `quantity` additionally rejects negative values. Trailing
fractional zeros are removed and zero always has scale zero. Ordinary
arithmetic computes the exact mathematical result with conceptual unbounded
intermediates, normalizes it, and then checks the final bounds. Plain decimal
division succeeds only for a canonical exact result representable through
scale 28; repeating results and terminating results needing more precision are
distinct failures. Rounded operations require an output scale and exactly one
of `Rounding::toward_zero`, `Rounding::away_from_zero`, `Rounding::floor`,
`Rounding::ceil`, `Rounding::nearest_even`, `Rounding::nearest_away`, or
`Rounding::nearest_toward_zero`, as documented in
[`kotodama_numeric_v1.md`](./kotodama_numeric_v1.md). Other rounding spellings
are rejected rather than treated as compatibility aliases.
Rounded operations never round implicitly. Invalid constant arithmetic is
diagnosed during compilation; runtime failures use the same stable numeric
faults.

The exact rounded source surface is:

```text
decimal.div_round(decimal divisor, int scale, rounding-mode mode) -> decimal
quantity.div_round(decimal divisor, int scale, rounding-mode mode) -> quantity
quantity.ratio_round(quantity divisor, int scale, rounding-mode mode) -> decimal
```

All three arguments are named-only. `rounding-mode` denotes one of the seven
`Rounding::*` paths listed above, not an integer tag or a user-declarable type.
The scale is checked in `0..=28`; `div_round` is not an `int` method, and
`ratio_round` is not a `decimal` method.

## Bounded lists

An uncontextualized non-empty `[a, b]` infers `List<T, 2>`. Context may provide
a larger capacity; `[]` requires a `List<T, N>` context. A comprehension's
proven maximum is its source capacity, even when it has an `if` filter, and it
is rejected if that maximum exceeds 64 or the contextual capacity. Lists may
nest and contain ordinary structured values, but never resource handles such
as `StateMap` or `Secret`. Every element schema must flatten to at least one
runtime word; zero-field and recursively zero-sized product elements are
rejected with `E_LIST_ZERO_SIZED_ELEMENT`.

The bounded API is `len`, `get(index) -> Option<T>`, `try_set`, `try_push`,
`pop() -> Option<T>`, `contains`, `take(constant_limit)`, and bounded
`enumerate`. Unchecked list reads and writes are errors. Failed `try_set` and
`try_push` leave the list unchanged. The mutating `try_set`, `try_push`, and
`pop` methods require a `var` receiver; temporaries and immutable `let`
bindings are rejected. `contains` is available when the element has canonical
equality; structs, tuples, `Option`, `Result`, and nested `List` values are
compared recursively by schema, tag, active payload, length, and element value.
The migration fix for a complete simple `list[index] = value;` statement is
`list.try_set(index: index, value: value);`: V1 defines that form as one attempted mutation,
with an out-of-range `false` safely ignored. Code that must distinguish that
case should bind or branch on the returned boolean. No automatic rewrite is
offered for compound writes, comments, or incomplete source ranges.
`take(limit)` accepts a compile-time constant from zero through the source
capacity. `take(0)` returns an empty list with the minimum valid static capacity
`List<T, 1>`; a positive limit `L` returns `List<T, L>`.

## Native JSON values

```kotodama
seiyaku NativeJsonExample {
    view fn build(AccountId account_id, string label) -> Json {
        json {
            owner: account_id,
            amount: 1.25,
            labels: json ["primary", label],
        }
    }
}
```

JSON object keys are identifiers or string literals, duplicates are errors,
and encoded keys are sorted canonically regardless of source order. Each
object or array node contains at most 64 entries or elements. JSON
construction recursively accepts booleans, `int`, `decimal`, `quantity`,
strings, canonical IDs, `Json`, `Option`, and `List`; bytes become lowercase
`0x` hex. `Result` and arbitrary structs require explicit handling. Typed
getters return `Option<T>` and use `.get_int(key)`, `.get_decimal(key)`, and
`.get_quantity(key)` for the three numeric domains. Retired numeric getter
spellings are errors.

`Json::parse` accepts exactly one direct string literal and validates that
literal at compile time. Parameters, locals, and constants are not accepted as
parser input and produce `E_JSON_LITERAL_REQUIRED`; construct dynamic typed JSON
with native `json { ... }` and `json [ ... ]` expressions instead.

## Typed core ledger queries

The five compiler-declared projections and the page wrapper have these exact
field names, declaration order, and types:

```text
AccountView {
    AccountId id,
    Json metadata,
}
AssetView {
    AssetId id,
    quantity amount,
}
AssetDefinitionView {
    AssetDefinitionId id,
    string name,
    Option<string> description,
    AccountId owned_by,
    quantity total_quantity,
    Json metadata,
}
DomainView {
    DomainId id,
    AccountId owned_by,
    Json metadata,
}
NftView {
    NftId id,
    AccountId owned_by,
    Json content,
}
QueryPage<T> {
    List<T, 64> items,
    Option<int> next_offset,
}
```

`ledger::query::account`, `asset`, `asset_definition`, `domain`, and `nft`
accept their exact typed ID and return `Option<View>`. Their plural forms
`accounts`, `assets`, `asset_definitions`, `domains`, and `nfts` require named
`int offset` and `int limit` arguments and return `QueryPage<View>` with
`List<View, 64> items` and `Option<int> next_offset`. Offset is in
`0..=i64::MAX`, limit is 1 through 64, and the `offset + limit` page window must
fit `i64`. Ordering is canonical ID order, and `next_offset` is present only
when another page exists. Other specialist query families remain explicit byte
APIs; the typed balance API is unchanged.

## Durable state

Scalar seiyaku state must be initialized by `hajimari`/`始まり` on every
successful `hajimari`/`始まり` path before it can be observed. `StateMap` is
host-backed and does not require allocation in `hajimari`.

`StateMap.get` returns `Option<V>`; absence is not represented by a zero, empty
string, or implicit default. Rvalue indexing such as `map[key]` and compound
indexed assignment such as `map[key] += value` are errors because both would
read a possibly absent value without handling `Option<V>`. Simple
`map[key] = value` remains the canonical per-key write form. The flat spelling
`get(map, key)` is not a StateMap operation; only the receiver form
`map.get(key)` invokes the intrinsic, while an unrelated user-declared function
named `get` resolves normally. `StateMap.remove(key)` returns the removed
`Option<V>` and is not permitted in views. Every scalar or aggregate state root
and every `StateMap` value is encoded once as one canonical, schema-bound record
under one durable key. Its domain-separated schema hash covers the exact type
and named-field layout; mismatched schemas, malformed typed leaves, invalid
active-only sums, and null active pointers are rejected. Nested `StateMap`
values and unsupported leaves are compile errors. V1 map keys may be `int`,
`decimal`, `quantity`, `bool`, `string`, `bytes`, or a typed Iroha identifier;
aggregate, `Json`, optional, result, secret, and nested-map keys are rejected.
Numeric keys are canonicalized before hashing and ordering, so equivalent
decimal spellings cannot create distinct keys. Durable paths are the distinct
nominal `StatePath` storage type, transported to IVM state syscalls as canonical
Norito bytes rather than as `Name` pointers. Physical V1 map paths retain the
form `Name-base/<reversible lowercase hexadecimal canonical-Norito key bytes>`,
so path order is canonical key-byte order without hash-collision ambiguity.
Map keys are capped at 4 KiB, map bases retain the 255-byte `Name` bound,
complete paths are capped at 16 KiB, and iteration pages are canonical
`Vec<StatePath>` values limited to 64 items. The source helper
`base.path(key)` therefore returns `bytes` containing a framed `StatePath`;
passing a `Name` directly to `state::get`, `set`, `delete`, `keys`, `has`,
`len`, or `count` is a type error.

Compiler-derived access metadata is advisory until independently verified from bytecode. Unknown, dynamic, incomplete, or transitively unresolved access forces conservative scheduler serialization.

## Errors and requirements

Seiyaku units declare error enums. A requirement has the form `require(condition, Error::Variant)`. Error variants compile to stable seiyaku codes included in the public interface. Free-form failure strings are not part of the release contract.

```kotodama
seiyaku Vault {
    error enum VaultError {
        ZeroDeposit = 1,
        NotReady = 2,
    }

    state int balance;

    hajimari() {
        balance = 0;
    }

    kotoage fn deposit(int amount) authorize("CanDeposit") {
        require(amount > 0, VaultError::ZeroDeposit);
        balance = balance + amount;
    }

    view fn ready() -> bool {
        return balance > 0;
    }
}
```

Assertions intended only for local tests or diagnostics must not replace public seiyaku errors.

## Local test mode

`#[test]` functions, `fixture` declarations, `koto_test` targets, and the
`test::` builtin namespace exist only when the compiler driver explicitly
selects test mode. A production `check` or `build` rejects these constructs
with `E_TEST_ONLY_PRODUCTION`; it never removes them silently before semantic
analysis or artifact hashing. Typed HIR records whether test capabilities were
enabled, and production code generation rejects test-capable HIR as well.

The `koto test` driver is the explicit test-mode boundary. It compiles the full
suite in test mode, then derives a test-free runtime seiyaku when the target has
an invocable public or lifecycle declaration. A pure unit-test target containing
only private helpers and `#[test]` functions needs no runtime artifact; its
tests, coverage, and profile data run from the test projection. This runner-only
derivation is not available to ordinary production builds. The two projections
retain separate immutable prepared artifacts, compiler reports, and code hashes.
The test projection is a generic IVM 1.0 harness without deployable `CNTR` or
`DBG1` sections. Its compiler-owned interface is carried beside the immutable
image, checked against the current ABI hash, and structurally validates the
terminal `HALT` through the reserved `__koto_test_return` descriptor. Production
admission accepts only IVM 1.1 contracts with an embedded interface, and rejects
both the generic test profile and that selector. Host-private
`0x00FE0001..=0x00FE0005` helpers require the crate-private test loader plus an
explicit host opt-in (the runner supplies `KotoTestHost`), remain outside ABI v1
and its hash, and cannot be enabled by public VM loaders or a permissive custom
host.
Typed fixture values use the same constructors as seiyaku code, including
`AccountId::parse`, `AssetDefinitionId::parse`, `DomainId::parse`,
`Name::parse`, and `Json::parse`; flat fixture-only constructor aliases are
errors.

## Namespaced host API

Source code uses namespaced capabilities. Representative roots are:

- `context::authority`, `context::block_height`, and other immutable call context
- `context::seiyaku_subject`, `context::seiyaku_address`, and
  `context::kotoage` for the branded execution identity and selected public or
  lifecycle declaration
- `ledger::asset::transfer`, `ledger::asset::mint`, and `ledger::asset::burn`
- `ledger::account::set_detail`
- `ledger::query::seiyaku_manifest` and `ledger::query::seiyaku_instance`
- `ledger::seiyaku::grant_kotoage` and
  `ledger::seiyaku::revoke_kotoage` for the current immutable seiyaku address
  and an exact kotoage selector
- `state::get`, `state::set`, and `state::delete`
- `bytes::len(value)` for the exact payload length of a first-class `bytes`
  value; it does not accept `Json`, IDs, strings, or generic pointer-ABI values
- `crypto::sha256`, `crypto::sha3`, and signature/proof operations
- `math::wrapping_add`, `math::wrapping_sub`, `math::wrapping_mul`, and
  `math::wrapping_neg` for explicitly modular 512-bit integer arithmetic
- `debug::info` for diagnostics
- `test::assert`, `test::assert_eq`, `test::invoke_kotoage`, and
  `test::invoke_kotoage_as` in test builds only

Flat aliases are errors. Allocation, heap growth, raw pointers, direct syscall variants, opaque instruction submission, and compiler `*_direct` helpers are not source APIs. In particular, `tlv_len` and `codec::tlv_len` remain internal; source uses only the typed `bytes::len`. The canonical builtin registry defines each capability's signature, effect, syscall, access behavior, gas class, and permitted execution modes.

The scalar IVM operations historically exposed as `math::isqrt`, `math::abs`,
`math::min`, `math::max`, `math::div_ceil`, `math::gcd`, and `math::mean` are not
Kotodama V1 source helpers. They remain internal until complete signed 512-bit
semantics and deterministic gas formulas are specified. Use ordinary checked
operators and comparisons; no implicit 64-bit narrowing is permitted.

Compiler-owned lifecycle and code-operation labels use the branded
`seiyaku::deactivate_instance`, `seiyaku::remove_code`,
`seiyaku::register_code`, `seiyaku::register_bytes`, and
`seiyaku::activate_instance` spellings. They remain compiler-internal and
cannot be called from source. The English `contract::` root is never a
Kotodama source namespace, and raw `contract::call`/`seiyaku::call` sugar does
not exist. English feature-concept builtin spellings such as
`context::contract_address`, `context::entrypoint`,
`ledger::query::contract_manifest`, and `test::invoke_entrypoint` are likewise
rejected rather than retained as compatibility aliases.

## Secrets and ZK seiyaku

A ZK seiyaku explicitly requests the ZK execution capability through its build
configuration. Private input is represented only as `Secret<int>`,
`Secret<decimal>`, or `Secret<quantity>`. A private input must initialize an
explicitly typed binding; its canonical Norito record carries the matching
nominal kind plus the complete schema-bound numeric frame. The compiler emits
that requested kind, and the host rejects a mismatch before allocating opaque
private VM memory.

The V1 source declassifier is `crypto::valcom`. Both operands must be typed
secrets. It binds the nominal kind and every byte of each canonical numeric TLV,
derives full-width BLS12-381 scalars without `u64` truncation, and returns the
complete compressed Pedersen point as a public `int`. The scalar `POSEIDON2`
and `POSEIDON6` opcodes are internal proof gadgets that reject private
operands. ABI V1 has no register-level BLS12-381 public-key, commitment, or
curve operations; full-width typed syscall boundaries provide those semantics.

Secrets cannot influence public control flow, public returns, logs, error
selection, state keys, state values, ledger writes, host queries, seiyaku calls,
ordinary arithmetic, comparisons, collection indices, or assertions. They
cannot appear in public parameters or return types. The legacy invocation-local
`u64` nullifier helper is not a durable V1 source capability.

`Secret<T>` and `GET_PRIVATE_INPUT` execute only in local compiler tests and in
explicitly provisioned prover/test hosts. Ordinary production consensus
dispatch rejects every seiyaku selector whose bytecode-reachable call graph
reads a private input, including a read hidden in a helper. This fail-closed
boundary is enforced again by consensus `CoreHost`, which rejects
`GET_PRIVATE_INPUT` during quote preparation and direct execution even if
selector resolution is bypassed. It remains until a proof-carrying invocation
statement binds the seiyaku address and code hash, seiyaku selector, public
arguments, authority and chain,
state root and exact read/write sets, outputs and events, gas schedule and
ceiling, and circuit and verifier-key versions.

Raw private witness bytes must never enter a signed transaction, `IvmProved`
payload, overlay, public argument record, or deterministic validator replay.
Validators receive only the complete public statement and its proof once that
production proof path exists.

The compiler performs fail-closed information-flow analysis across the complete call graph.

## Resource limits

The compiler rejects inputs exceeding any V1 hard limit:

| Resource | Limit |
|---|---:|
| UTF-8 source | 1 MiB |
| Tokens, including EOF | 250,000 |
| Delimiter/parse nesting | 256 |
| Resolved named-type nesting | 256 |
| Expanded local struct nodes | 250,000 |
| Collection iteration | 64 items |
| Signed argument record | 1 MiB |
| Complete nested-return TLV | 1 MiB (1,048,537-byte record + 39-byte envelope) |
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
koto check seiyaku.ko
koto check --project kotodama.project.json
koto build seiyaku.ko --max-cycles 1000000
koto build --project kotodama.project.json
koto build --format sarif seiyaku.ko
koto check --zk proof_seiyaku.ko
koto build --zk proof_seiyaku.ko
koto test seiyaku.test.ko
koto fmt seiyaku.ko
koto doc seiyaku.ko
koto explain K0001
koto lsp --project kotodama.project.json
koto lsp --zk --project zk.project.json
```

`--zk` is an explicit build capability, not source metadata. It is required for
`Secret<T>` and the approved proof/commitment operations; ordinary builds reject
those constructs. `koto check|build|doc|lsp`, `musubi build`, and
`iroha contract dev` pass this policy to the same in-process compiler session.
It does not make ABI, vector, or pointer policy selectable.

`koto fmt` and LSP formatting consume the compiler's lossless token stream.
They refuse syntactically invalid input, preserve comments and literal spelling,
and canonicalize four-space indentation, declaration spacing, operators, and
block layout with a 100-column target. A comma-delimited construct expanded
over multiple lines has a trailing comma. Formatting is deterministic and
idempotent, and fails rather than producing a source larger than the mandatory
1 MiB limit. `koto fmt --check` performs no writes.
LSP validation analyzes reusable `module Name` files without artifact
generation, but it never invents imports or exports from the set of open
documents. `koto lsp --project kotodama.project.json` loads the same exact
locked graph as `check` and `build`; open buffers overlay their matching
canonical project files while unopened files are read from that graph. Without
`--project`, open documents have positional-check semantics and cross-file
calls report `E_PROJECT_MANIFEST_REQUIRED` rather than appearing valid only in
the editor.
Positional `koto check` paths are independent sources: one seiyaku has
an empty import graph, reusable modules are checked without linking, multiple
seiyaku roots are rejected, and mixing a root with modules requires `--project`.
`koto check --project` and `koto build --project` consume the same exact locked
graph; neither command scans sibling files or treats source order as authority.
A multi-source `koto check --format json|sarif` invocation emits exactly one
machine-readable document with the combined, deterministically ordered
diagnostic set. LSP
framing, individual documents, open-document count, and aggregate retained text
all have explicit bounds; rejected updates are not retained as stale formatter
input.

The project graph is canonical Norito JSON. Every field is explicit, version 1
is the only accepted schema, source paths are relative to and contained by the
manifest directory, and package identities are exact locked strings:

```json
{
  "version": 1,
  "root": "contracts/app.ko",
  "imports": [{"alias": "Math", "package": "example/math@1.0.0"}],
  "packages": [{
    "identity": "example/math@1.0.0",
    "modules": ["modules/math.ko"],
    "exports": ["value"],
    "imports": []
  }]
}
```

Unknown or duplicate fields, duplicate sources/exports, path escapes, unknown
packages, import cycles, undeclared aliases, and unexported calls fail closed.
Diagnostic spans keep package identity separate from the logical source path,
so two locked packages may both own `src/lib.ko` without ambiguous JSON, SARIF,
or human output.

`koto build --format human|json|sarif` and `musubi build --format
human|json|sarif` use the same canonical diagnostic bundle. Typed-module link
failures retain all semantic fields rather than embedding a rendered error in a
wrapper string. Imported-call failures point at the exact resolver-owned call
name, and ambiguous exports label every conflicting function declaration.

The test driver supports deterministic discovery and selection:

```text
koto test list seiyaku.test.ko
koto test run --filter exact_test_name --exact --jobs 4 --seed 7 seiyaku.test.ko
koto test run --format json seiyaku.test.ko
koto test run --junit target/kotodama-tests.xml seiyaku.test.ko
koto test run --zk zk_seiyaku.test.ko
```

The Rust compiler library behind `koto` is canonical. `iroha contract dev` and
Musubi call that library in process. Their physical paths are normalized to
project-relative `/` names. In the absence of an explicit project manifest, the
deterministic V1 default is the selected root with no inferred imports,
wildcard exports, or sibling modules. Content-addressed build
authentication runs before parsing or typed-HIR linking, so an unchanged
project performs no compiler work and rewrites no outputs. Node.js calls the
compiler asynchronously through `iroha_js_host`; browsers use an explicit
compiler-service client. SDK adapters enforce the same 1 MiB UTF-8 source limit
before native or network dispatch. There is no independent JavaScript compiler
or offline browser compiler.

ABI version, vector width, execution-mode bits, and compiler features are not
source declarations or user-selectable language metadata. Build configuration
may request a permitted execution capability such as ZK and may select a
positive cycle ceiling no greater than node admission policy. Source-level
`meta` blocks are errors. The manifest's hash-covered `features_bitmap` is
derived from the execution header and currently mirrors only ZK and
deterministic VECTOR capability; it never advertises host SIMD, Metal, or CUDA
availability.

## Seiyaku artifact

The canonical `code_hash` is a domain-separated hash of the complete deployable `.to` image: every execution-header field, the embedded seiyaku interface (CNTR), typed literals, and executable code.

Debug information and source maps are forbidden inside deployable artifacts. They are hash-keyed sidecars whose `artifact_hash` identifies the exact `.to` image. Every native source segment carries its graph-stable `source_id`, logical source path, exact half-open UTF-8 byte range, and the corresponding one-based line and Unicode-scalar column; generated instructions without a source range are identified separately rather than borrowing a neighboring span.

Nodes validate direct control-flow targets, allowed ABI-v1 syscalls, pointer-ABI types, interface structure, code/ABI hashes, and signed manifest equality. Compiler fingerprints are informational and are not security claims.

## Example

```kotodama
seiyaku Counter {
    state int value;

    hajimari() {
        value = 0;
    }

    kotoage fn increment(int delta) -> int authorize("CanIncrementCounter") {
        let int next = value + delta;
        value = next;
        return next;
    }

    view fn current() -> int {
        return value;
    }
}
```

The repository documentation check discovers and compiles every tracked
`kotodama` or `ko` code fence and documented `*.ko` heredoc below the roots in
`kotodama_v1_docs.json`. Grammar-derived keyword and operator tables feed
documentation, formatting, syntax highlighting, and LSP completion so those
surfaces cannot define independent dialects.
