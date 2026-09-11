# Explicit JSON context implementation checkpoint

This checkpoint records an **unapplied candidate** for the AccountId extraction
prerequisite. Current live Norito source and all existing uncommitted work are
preserved. AccountId has not moved. The composed library qualification below
does not qualify the remaining codec consumers or repository-wide cutover.

The candidate passes immutable context through parsers, prepared tapes,
readers, visitors, typed keys and checked writers. It removes the unchecked
writer fallback, context-free FromStr key bypass, duplicate JSON entry points
and RawValue serialization. Compact and pretty output use fallible sinks;
context failures remain typed. The macro requires context and returns Result.
Ordinary parser, structural-index, tape, reader, native-value and schema modules
replace the inline ownership in the candidate. Their staged files meet the
production/test size limits; this does not close live repository findings.

Evidence is under `target/architecture-redesign/model-base-extraction-v1/`.
The initial stage records retain the qualification available at their freeze:

| Stage | Manifest SHA-256 | Qualification |
| --- | --- | --- |
| `norito-json-context-decoder-v1` | `fac698beae7ab2b2f8fc54cf104e537bf3b4ff9eb7947cce8a450d5a95978d85` | 14 source paths, 30 sealed artifacts. Twelve changed/new parent files pass syntax/format checking; two carrier files retain their earlier exact bytes. All 24 lexical test names remain; 11 new actual-codec tests are staged. Parent compilation and test execution are pending. |
| `norito-checked-writer-context-v1` | `e6d857564d19d9681d1aeccf6e4d67530b12146ba2be89010843f8c5a7845486` | Three actual production modules compile with rustc and pass seven focused runtime tests using their recorded existing core/native artifact and frozen parent trait signatures. Full inline suites, parser/derive composition and strict lint are pending. |
| `norito-parent-decoder-review-v2` | `1e4b0e16846e8980e16d044cbd9af3339dfa4ef0ccd6154c50177500407b661f` | Bounded independent review confirms repairs to trait-method visibility, typed-key context bypasses, Parser/Reader aliases, checked pretty output and telemetry error propagation. This is not a compiler result. |

The parent verified all 31 sealed writer artifacts and unchanged live
before-images. The structural-index and accelerator implementation retains its
original body after a private visibility adjustment and formatting. Review
findings and earlier intermediate images remain in their original records.

The writer inventory accounts for 17 runtime assertions about APIs that are
removed by design. These require real compile-fail coverage, not replacement
runtime Unsupported results. Its remaining golden, allocation and two-pass
assertions stay with their owners. These obligations were pending at the
initial writer freeze; their later qualification is recorded below.

The actual parent, checked writer, derive and Core/streaming/YAML owners now
compile together in an inode-independent snapshot of the actual dirty workspace.
Original workspace manifests, lockfile, profiles and real dependencies remain;
no source stubs or enlarged stacks are used. Core's structural schema hash
helpers are fallible diagnostics, separate from canonical frame selection.
They feed canonical checked JSON directly into the unchanged domain-separated
hash and propagate failures without a panic or substitute digest. YAML retains
its existing scalar newline behavior and propagates context and I/O errors.

The 38-path composed source is frozen at
`norito-context-composed-v1/source-v1/manifest.json`, SHA-256
`9c5b322558dce06d7ab9a29adc275adbae6c3ca326c54241d4649e76b4d720a6`.
On one unchanged full-source fingerprint,
`faabeb22ce8110f0d2198b64ee127d7c247789787696f0a32f2386c6cae37136`,
the actual Norito library passes **523 structural-feature tests**, including
all context, checked-writer, allocation, captured streaming-frame and 128-KiB
writer regressions. The default library passes **519 tests**. Each has one
existing ignored diagnostic. The derive library passes **64 tests** in both
configurations. Strict production-library Clippy with structural features
passes for both crates with zero diagnostics; two-package formatting passes.
The reports are `lib-final-build-1`, `lib-final-runtime-1`,
`lib-default-final-build-1`, `lib-default-final-runtime-1` and `lib-clippy-2`
under `norito-context-composed-v1/`. All runs use four ordinary test workers.

Two actual integration files additionally pass **seven schema/frame tests and
four cross-language fixture tests**, linked against the recorded pre-lint
structural library. Their source and artifact identities remain in
`owner-runtime-1.json`; they do not claim the later attribute-cleaned artifact.
Earlier failed compilation, a new YAML test's incorrect newline expectation,
and the direct proc-macro test loader-path failure remain recorded. The loader
now uses the compiler's recorded libstd path; no thread stack setting changed.

The derive's prepared-tape path now honors a field's declared custom helper
and passes the same context. Its frozen 29-path fixture supplement is
`norito-derive-context-v1/fixture-supplement-v1/manifest.json`, SHA-256
`04014909c82c8dd6de7f3a28a60affe3585baa3389558a1203eb2c7e76aa7720`.
Nineteen runtime tests, seven changed UI pass programs and nine compile-fail
contracts pass against the final composed structural artifacts. Their frozen
result manifest is `norito-derive-context-v1/final-composed-fixture-results-v1/manifest.json`,
SHA-256 `06737111715f53ae493522f245d05f383c36ab024562678851c9a4b0b83db4a7`.
Linked library hashes and fixture source remain unchanged throughout execution.
The negative cases account for all 17 retired writer assertions. Every original
test name, assertion count and malformed runtime literal is retained. This
separate fixture source was not yet overlaid at that checkpoint.

The subsequent composition includes that derive supplement, all 37 migrated
Norito integration-test files and the three-path MV context implementation.
The integration stage is `norito-integration-context-v1/manifest.json`, SHA-256
`e7cd5c21b6089c10f88b4fdf4f6d03ba3b9766777fd6a83477dee38c186dec71`.
All 191 original integration test names and 420 assertion sites are retained;
all 61 non-Rust fixture files are byte-identical. The MV context stage is
`mv-json-context-v1/manifest.json`, SHA-256
`d96ef516e75e5d795c0e6f9261810ba9ffc42129c9e01e0786ecb6a93af101d1`.
Its state writers preserve revert/blocks ordering and contextual key/value
decoding. Non-final tuple components reject the separator that previously
allowed distinct typed keys to encode identically; valid key bytes remain.

On full-source fingerprint
`11e2b055f3d1966553861bf7f15c965020a438ed231584b6fdc04a4607b8002c`,
actual Cargo builds all 14 selected test executables. Thirteen targets pass
**1,499 runtime tests** with four ordinary workers and one existing ignored
Norito diagnostic: 523 Norito library, 843 grouped integration, eight allocation,
64 derive library, 19 strict JSON and 42 MV tests. Strict Clippy passes for all
three packages' library/test targets with structural and trybuild features,
with zero diagnostics. See `full-codec-tests-build-1.json`,
`full-codec-runtime-1.json` and `full-codec-clippy-1.json` in the composed stage.

The full trybuild UI executable fails during linking with `ENOSPC`; its two
ordinary path tests pass. This is not a passing harness run. All **13 positive
UI programs** subsequently compile and execute directly against that source's
actual Cargo-selected libraries, and all **22 negative cases** reject with
their exact normalized diagnostic snapshots. Those independent results are
sealed by `norito-context-composed-v1/full-ui-direct-1/manifest.json`, SHA-256
`a8482ee1fe148e75a2d94ce5aea705307968cfafe383629e2a55689489431890`.
The disk failure log, generated manifests and artifact identities are retained;
only that terminal run's newly generated isolated trybuild cache was removed.

The subsequent MV repair removes unconditional key copies, admits temporary
B-tree nodes before insertion, admits restored epoch-cell boxes and charges
the exact temporary text used by the native Value decoder. Existing lexical
admission still charges one metadata byte per entry before typed decoding.
The initial new tests omitted those metadata bytes and used the wrong counter
type; their compilation/runtime failures remain recorded. Corrected tests
exercise metadata-only rejection, exact/short budgets, typed duplicate errors,
context precedence and native restoration without changing production limits.

The complete **108-path unapplied source** is frozen at
`norito-context-composed-v1/source-v2/manifest.json`, SHA-256
`a48453ca0ec0251e530f97b7c74a8d98d01e3adf9934237e4e1f4a045a1bd108`.
Every live before-image remains unchanged. On full-source fingerprint
`bfbbb6d656259988d0a2455a3f4accd24475f960b7cdccaac8eaa070da90c1bc`,
**49 MV tests** and strict MV library/test Clippy pass with zero diagnostics.
The source-bound reports are `mv-admission-build-3.json`,
`mv-admission-runtime-2.json` and `mv-admission-clippy-1.json`. These later MV
results retain their separate source identity from the full codec run above.

Final concread tree allocation admission remains unresolved: its private
nodes, cursor buffers, ownership wrappers and allocating separator-key clones
need a fallible allocation interface in the tree owner. A standard B-tree
estimate cannot certify that implementation. The independent review is
`mv-context-admission-review-v1/manifest.json`, SHA-256
`ba495365e86145432c4bab83ad889c80eff5710ebcc0d78d848d8c3a46cbf4fe`.

TODO: Complete final MV tree admission, codec examples/benchmarks and external
context callers, then account ownership. The complete candidate must pass all
consumer, feature, serialization and strict test-lint checks before replacement.
Workspace, memory, native/device and four-validator release gates remain open.
No build-memory improvement is claimed by this checkpoint.
