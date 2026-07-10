# Kotodama V1 implementation alignment

This is a non-normative guide for contributors aligning the implementation
with the first release. The sole source-language specification is
[`docs/source/kotodama_grammar.md`](../../../docs/source/kotodama_grammar.md).
`status.md` records verified work and `roadmap.md` records only outstanding
work.

Kotodama targets deterministic Iroha Virtual Machine bytecode (`.to`). It does
not target RISC-V as a standalone architecture.

## Release surface

The V1 parser accepts one branded source unit:

- one deployable `seiyaku Name { ... }`/`誓約 Name { ... }`, or
- one reusable `module Name { ... }` linked before code generation.

Contracts use private `fn`, authorized mutating `kotoage fn`/`言挙げ fn`,
read-only `view fn`, and the dedicated `hajimari`/`始まり`, `kaizen`/`改善`,
and `trigger` declarations. English `contract`, `entry`, `init`, and `upgrade`
are not source keywords. Parameters, fields, constants, and state use
`name: Type`. Locals use immutable `let` or mutable `var`; duplicate names and
shadowing are errors.

There is no compatibility grammar, source edition, implicit entrypoint,
source-order dispatch, raw-call sugar, typeless parameter, wildcard import, or
source `meta` block. ABI v1 is unconditional. Execution capabilities and vector
metadata are derived by the compiler or supplied through trusted build
configuration, never selected by contract source.

## Compiler pipeline

The implementation is organized around these boundaries:

1. lossless CST and spanned AST preserve source identity and locations;
2. resolution produces a named HIR and rejects unknown, duplicate, reserved,
   cyclic, and ambiguous symbols;
3. type and effect analysis produces typed HIR and computes transitive effects
   and access through the complete call graph;
4. SSA MIR is optimized before register allocation and bytecode emission;
5. the assembler relaxes branches and calls and emits the canonical V1
   artifact plus hash-keyed debug sidecars.

`CompilerSession` owns reusable compiler state. Public driver APIs return either
a `CompileOutput` or a `DiagnosticBundle`; diagnostics have stable codes,
phases, severities, spans, labels, notes, help, and optional fixes. Human, JSON,
and SARIF renderings carry the same semantic fields.

Hard parser budgets are part of the release contract: 1 MiB of UTF-8 source,
250,000 tokens including EOF, and 256 levels of delimiter/parse nesting.
Recursive value types, recursive calls, `while`, and unproved loop bounds fail
closed.

Every artifact carries a positive `max_cycles` value in its hash-covered
execution header. The node's `pipeline.ivm_max_cycles_upper_bound` is a
mandatory non-zero configuration value (default `1_000_000`) used by admission
and every execution path. The value is file-configured only: a zero
configuration value is rejected, and neither environment variables nor
on-chain custom parameters can override the node policy.

## Security boundaries

The canonical artifact hash covers every deployable byte: all execution-header
fields, the embedded CNTR interface, typed literals, and executable code.
Source maps and debug data are sidecars keyed by that hash.

The compiler uses one exhaustive builtin registry for signature, effect,
syscall, scheduler access, gas class, and allowed execution modes. Source code
uses namespaces such as `context`, `ledger`, `state`, and `crypto`; allocation,
raw pointers, direct syscall variants, and opaque instruction submission are
not source capabilities.

CNTR is an interface, not a trust root. Admission validates control-flow
targets and ABI-v1 operations, then derives transitive effects and access from
bytecode. Dynamic or incomplete access forces conservative serialization.
Compiler fingerprints and access summaries remain informational until they are
independently verified.

Host operations follow prepare/quote, gas debit, then execute. Query,
allocation, nested invocation, and state effects must not happen before the
caller can afford the quoted cost.

## Language safety

Arithmetic is checked by default; explicit `math::wrapping_*` operations are the only
modular arithmetic. Comparisons preserve signed `i64` ordering at the complete
integer boundary. `&&` and `||` short-circuit and therefore do not execute an
unneeded right-hand side.

Durable scalar state is initialized in `hajimari`. `StateMap.get` and the mutating
`StateMap.remove` return `Option<V>`, iteration is in canonical Norito key
byte order, and collection iteration is capped at 64 items. Top-level structs,
tuples, `Option`, and `Result` values use one schema-bound canonical Norito
record and therefore one host read or write per state/StateMap entry. Nested
`StateMap` values and schema mismatches are rejected.

Public failure behavior uses explicitly numbered `error enum` variants and
`require(condition, Error::Variant)`. Stable error codes are exported in the
seiyaku interface; free-form strings are not a public error protocol.

Private input exists only as `Secret<T>` for a ZK-enabled build. Secret values
can flow only into approved proof or commitment operations. They cannot affect
public returns, logs, error selection, control flow, state, ledger writes, host
queries, or contract calls.

## Runtime and tooling

Public calls carry one canonical Norito argument record. The wrapper decodes it
once and then reads typed ABI words; JSON is confined to Torii and CLI
boundaries.

Validated bytecode is cached as an immutable `PreparedContract` containing the
interface, metadata, predecode, and CFG. Warm execution reuses prepared state
and resets only dirty memory instead of cloning the full VM memory and Merkle
tree.

`koto check|build|test|fmt|doc|explain|lsp` is the single command surface. The
Rust compiler library is canonical for `koto`, `iroha contract dev`,
Musubi, and the Node native bridge. Browsers use a compiler service; there is
no independent JavaScript or offline browser compiler.

Generated keyword and operator tables are consumed by formatting, syntax
highlighting, documentation, and LSP completion. CI compiles every current
`kotodama` documentation fence so examples cannot silently define a second
dialect.

The formatter is a canonical lossless-token consumer: it preserves comments
and literal spelling, emits deterministic four-space block layout, is
idempotent, and refuses invalid or post-format sources larger than 1 MiB.
The reusable module driver caps one graph at 512 source units/16 MiB and keeps
only a 64-entry/4 MiB exact-source LRU of parsed modules, so long-lived compiler
services cannot accumulate attacker-controlled ASTs without bound.

## Performance gate

`crates/ivm/benches/bench_kotodama.rs` measures parsing, semantic analysis, IR
lowering, end-to-end code generation, cold execution, and warm prepared
execution separately. The runtime samples explicitly select the embedded CNTR
entrypoint, use the canonical Norito argument record, and validate the result
before Criterion starts sampling. The warm sample prepares and loads its
immutable contract once, builds its reset template once, and measures only
dirty-state reset plus invocation.

The canonical golden pipeline also applies deterministic artifact gates before
publishing anything. Every compiler-generated instruction region must contain
strictly less than 1% unresolved relocation NOPs. Representative padding-heavy
checked-in samples must be at least 50% smaller than the audited pre-reset code
regions recorded in `scripts/kotodama_v1_size_baseline.json`. These checks run
for both `--check` and `--write`:

```console
make kotodama-goldens-check
```

An authenticated second build in the same pipeline must report every source as
`fresh` and preserve every generated file's modification time, proving that a
no-op graph performs zero compilation and zero rewrites.

Capture a reference on the same controlled runner, then compare a candidate:

```console
cargo bench -p ivm --bench bench_kotodama
python3 scripts/check_kotodama_perf.py \
  --write-baseline target/kotodama-perf-baseline.json
# Build and benchmark the candidate on the same runner.
cargo bench -p ivm --bench bench_kotodama
python3 scripts/check_kotodama_perf.py \
  --baseline target/kotodama-perf-baseline.json
```

The checker fails closed on missing/malformed samples or benchmark coverage
changes. Its threshold cannot be loosened above 5%; a stable release runner may
set a tighter threshold with `--threshold`. The
`.github/workflows/kotodama_perf.yml` gate checks out the pull request base and
candidate, measures both on the same runner with Criterion's named baseline,
and applies this checker to every representative median. Timing baselines are
deliberately runner-local; they are not portable across CPU models or loaded
hosts. The reset's initial pull request cannot compare against the retired
compiler because that base has no equivalent phase suite or language input; CI
detects that one bootstrap case and checks a candidate self-baseline. Once the
suite lands, absence of any representative base or candidate sample fails
closed and every later change is subject to the 5% ceiling.
