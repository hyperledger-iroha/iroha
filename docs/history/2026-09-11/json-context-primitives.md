# JSON context and primitive ownership checkpoint

This continues the [preceding codec/MV checkpoint](../2026-09-10/json-context-implementation.md).
The changes remain in the isolated, real-source composition under
`target/architecture-redesign/model-base-extraction-v1/norito-context-composed-v1`.
They are not applied to the live API while dependent callers still require migration.
The original dirty source, manifests, lockfile, binary identities and historical failures
remain preserved. No compiler stack override or release-profile change is introduced.

## Canonical primitive API

Stored `Json` has one borrowed, fallible typed constructor:
`Json::try_new(&payload, context) -> Result<Json, json::Error>`.
Typed decoding uses `try_into_any(context)` and preserves machine-readable context,
serialization and decoding errors. Raw canonical text uses `FromStr`; binary Json
remains an opaque canonical string and does not acquire a network-context parameter.
Infallible constructors/conversions and duplicate raw/native/typed constructor paths
are removed. The fixed byte/depth limits, strict lexical equality, duplicate rejection,
iterative semantic-value disposal and binary/schema identities remain.

The nine primitive owners implement the checked writer directly and propagate the
caller context through their decoding routes. Address, integer and numeric native
values use their actual validators. Numeric JSON implementations now have the
cohesive `numeric/json_codec.rs` owner, keeping `numeric.rs` below 5,000 lines.
ConstVec, SmallVec and UniqueVec reuse the admitted vector allocation; UniqueVec
preserves the original duplicate comparison order. SmallVec uses the real dependency's
allocation-free `from_vec` transfer into inline or spilled storage.

The six-owner collection stage preserves all 150 original tests, 322 assertion
expressions and their order, 56 binary/schema bodies and 13 checked writer bodies.
Its frozen source manifest is `primitives-context-collections-v1/manifest.json`,
SHA-256 `0e13766edc24b839c13d9930799a08cc1fa683bdfc5e299bcad783f73660d434`.
The parent numeric/Json implementation and all later test corrections have separate
before images and manifests; failed compiler/runtime/lint reports are retained.

## Actual qualification before the cleanup follow-up

On source `3d8e625fc084b540064914b92780af09b8ab84db29c236b8efa2be00ff87fc1f`,
the actual primitive library passes **320 tests** with four ordinary workers,
strict all-target Clippy, and four doctests (one existing ignored example).
These reports are `primitives-context-build-5.json`, `primitives-context-runtime-2.json`,
`primitives-context-clippy-2.json` and `primitives-context-docs-1.json`.
The existing nominal-frame goldens and 128-KiB JSON depth regression execute unchanged
in their invariants. New tests cover context isolation, typed missing-context errors,
canonical numeric validation, exact output bounds and collection allocation transfer.

The separate Rust `ffi_export` source selection also passes 320 library tests and
strict all-target Clippy, recorded in `primitives-context-ffi-*.json`. This is Rust
FFI qualification, not rebuilt JVM JNI or physical-device execution.

The Norito/derive standard all-target check and strict lint pass on source
`c00c6e2c3405c2c7f96ac68bc02100d7a70163101cd1370c4bc0e1ea04787b04`.
The actual full compiler UI harness now passes all four tests, including its 13 positive
and 22 negative fixtures; the earlier disk-exhausted run remains a separate failure.
The migrated telemetry example passes all seven tests on that same source.
Reports are `codec-all-targets-*-1.json`, `codec-full-ui-runtime-2.json` and
`codec-telemetry-example-runtime-1.json`.

Enabling `bench-internal` exposed three older frame owners missing schema declarations.
Their exact nominal names were captured with the pinned compiler before declarations
were added. The subsequent actual all-target strict Clippy selection passes, and
all nine migrated benchmark executables pass **62 cases** in Criterion's `--test` mode.
These are execution/correctness checks, not timing or build-memory measurements.
Reports are `codec-bench-internal-check-2.json` and `codec-migrated-benches-runtime-1.json`.

## Shared collection error cleanup

The shared streaming and native vector decoders now retain completed elements in
a cleanup guard. A later failure forwards each element to its codec's cleanup method;
success transfers the original vector allocation. Option, Box, ConstVec, SmallVec and
UniqueVec forward cleanup through their owned elements. Duplicate UniqueVec rejection
preserves comparison order while cleaning every completed element. Ordinary owner
destruction, parser admission, wire layout and schema identities remain unchanged.

Eighteen new regressions cover partial failures, trailing input, nullable and nested
owners, successful ownership transfer and duplicate rejection. A 32,768-level manually
constructed public Value chain reaches iterative cleanup on an ordinary worker stack;
the test does not bypass parser depth admission or use recursive equality. The five
existing source files retain all 92 original tests and 184 ordered assertions.

On source `29c5ad42de75b508f27e00645fc161ce4831d5b3c5c7bc3127fd414d828f6d89`,
all 17 actual test executables pass **1,854 tests**, with one existing ignored test:

| Package | Passed tests | Scope |
| --- | ---: | --- |
| Norito | 1,385 | Library, six grouped targets and allocation regressions |
| Norito derive | 87 | Library, strict JSON and complete compiler UI harness |
| MV | 49 | Library |
| Primitives | 333 | 327 library tests, five address/numeric tests and compiler UI |

`coherent-codec-build-3.json`, `coherent-codec-runtime-2.json` and
`coherent-codec-clippy-1.json` bind that source to the actual compiler artifacts,
runtime results and strict all-target Clippy selection. Separate primitive default
and Rust `ffi_export` selections each pass 327 library tests and strict all-target
Clippy (`primitives-cleanup-{base,ffi}-*.json`). The four-package doctest run passes
17 tests with two existing ignores (`coherent-codec-docs-1.json`). Formatting and
the unchanged retired-codec guard also pass against the actual candidate.

The earlier `coherent-codec-runtime-1.json` retains one failure in a newly added test:
it incorrectly expected malformed `[1,2,]` to fail before decoding any elements.
The correction asserts the existing `ExpectedDigits` position and exactly-once
cleanup of the two completed elements; no production behavior or original test changed.

## Frozen source and documentation

`source-primitives-qualified-v1/manifest.json` freezes all 145 composed paths and
verifies their complete live before-images. Its SHA-256 is
`41b4c14fa15ee5af9efd78ae061d9412a87a99208a49646d2e5464229a018b85`.
The final documentation supplement updates the Norito/MV READMEs, two codec
contributor notes and two stale parser Rustdoc links. The parser's non-comment
tokens are unchanged, as are all other Rust sources from the qualified selection.
All five complete README examples compile with `-D warnings` and execute against
the exact qualified Norito library (`readme-snippets-runtime-2/report.json`).
Strict documentation generation for all four packages passes with
`RUSTDOCFLAGS='-D warnings'` (`coherent-codec-rustdoc-2.json`); the preceding failed
link check and intermediate freeze remain recorded separately.
The runtime results above retain their precise source identity preceding this
documentation-only supplement; they are not whole-repository qualification.

## Remaining ownership and release gates

Map keys, retained/displaced map values, set elements and generated record-field
locals still need their own cleanup contracts. The vector correction does not make
ordinary local-variable destruction dispatch a codec cleanup method.

Address hostname and BigInt backend allocations are not yet fully represented by
Norito's cooperative decode counters. Final concread tree nodes, cursor buffers,
ownership wrappers and separator-key cloning also still require admission by their
actual owners. No arbitrary size estimate is treated as proof of these boundaries.

All 386 source files under the four candidate packages were measured using the
authoritative file-budget classifier. Two preexisting owners remain above 5,000
lines: Norito `columnar.rs` (5,653) and `core.rs` (8,244). No exception was expanded;
this measurement is not a passing architecture-budget result.

TODO: Finish cleanup/allocator ownership, migrate all external context consumers and
qualify one complete source before replacing the API or moving AccountId ownership.
Physical model extraction, measured memory reduction, workspace/native/device and
four-validator release gates remain open.
