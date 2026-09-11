# Native JSON value destruction

The [previous context checkpoint](json-context-primitives.md) left destruction
split between ordinary Rust ownership and explicit codec cleanup callbacks.
The correction puts iterative destruction in `Value::drop` and removes the
special public helper, callback contract and parser/vector cleanup guards.
It is qualified in the isolated real-source composition; external API consumers
must finish migration before the complete composition replaces the live API.

## Reproduced cause and correction

Two separately executed reproducers build 32,768-level trees iteratively on an
ordinary worker stack. Ordinary destruction of a single-child chain overflowed.
The previous explicit helper also overflowed for a chain with a leading null
sibling at every level: its overdepth fallback still recursed. Both baseline
processes exited with SIGABRT. Their exact compiler/library/source identities are
in `json-value-drop-root-baseline-v1/report.json` under the model extraction evidence.
These manually constructed values do not pass parser depth admission.

`Value::drop` takes child containers through mutable references, iterates them,
and retains only active ancestor iterators. Each temporary is empty before its
own destructor runs. Scalars and empty containers skip traversal. The inline
stack covers parser-admitted depth; deeper manually constructed branching trees
can spill to the heap with ordinary Rust allocation-failure abort behavior.
There is no recursive fallback or recoverable allocation-failure claim.

Both original shapes now pass ordinary destruction with the actual rebuilt
library (`json-value-drop-root-runtime-v2/report.json`). Normal Rust destruction
also covers partial vectors, maps, sets, parser frames and generated record locals;
these owners no longer require codec-specific cleanup dispatch. Generated flatten
code borrows maps or takes their contents through mutable references, preserving
container ownership and existing error order. No wire layout, schema identity,
parser bound, stack setting or release optimization level changes.

## Qualification

The complete final source fingerprint is
`c52ee7dd89886fbfc54e2c160bbbe2d4fbeb9248a2abee67164a1bd5975d8914`.
All 18 selected executables pass **1,862 tests**, with one existing ignored test,
including both complete compiler UI suites. Strict all-target Clippy passes for
Norito, Norito derive, MV and primitives. Separate primitive default and Rust
`ffi_export` selections each pass 327 library tests; the FFI selection also passes
strict all-target Clippy. Four-package doctests pass 17 tests with two existing
ignores, and strict Rustdoc, formatting and the retired-codec guard pass.

The new allocation target proves zero destruction allocations for admitted-depth
first/last-child arrays and objects and a 16,384-sibling array. Every measured
manually constructed buffer is freed. Parsed counterparts separately prove zero
destruction allocations and positive frees; parser allocation balance is not
claimed. Deep ordinary-drop and partial-failure tests use normal worker stacks.
The initial generated-flatten compile errors remain recorded before their emitter
correction; no failed run is relabeled as passing.

`norito-context-composed-v1/value-owner-closeout-v1.json` binds the exact reports,
source and artifacts (SHA-256
`794c0edf6d651a6808da6faef65a5c647797c1417dbe1eb302a3596e4e286d17`).
`source-value-owner-qualified-v1/manifest.json` freezes all 147 composed paths,
verifies every live before-image, and has SHA-256
`6f60e49253a2946f0f6932d67d7415d70885403274990f9b69f96fccd3de9b99`.
The earlier callback checkpoint remains historical evidence of the superseded
implementation. This checkpoint is not full workspace or release qualification.

## Remaining work

TODO: Migrate external context consumers and owned-Value extraction sites, then
qualify the complete source before API cutover. Crypto is the next compiled
consumer. Scalar backend and final MV tree allocation admission remain open.
Clone, Debug and equality still require a depth bound on manually constructed
values; this destructor correction does not make those operations iterative.

All 387 source files in the four packages were measured. Existing Norito
`core.rs` (8,244 lines) and `columnar.rs` (5,653) remain over budget; no exception
was expanded. Physical model extraction, measured memory reduction, complete
workspace/native/device tests and four-validator qualification remain open.
