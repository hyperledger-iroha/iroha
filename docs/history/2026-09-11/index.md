# Subsystem evidence recorded on 2026-09-11

These results apply to their recorded sources and artifacts, not the complete release.

| Subsystem | Evidence |
| --- | --- |
| [JSON context and primitive ownership](json-context-primitives.md) | Explicit primitive codecs and collection error cleanup; the coherent four-package selection passes 1,854 tests, both compiler UI suites and strict all-target lint. |
| [Native JSON value destruction](json-value-destruction.md) | Ordinary owner destruction replaces cleanup callbacks; two actual overflow reproducers are fixed, with 1,862 tests and strict all-target lint passing in the isolated candidate. |
| [Crypto JSON context and sequence ownership](crypto-json-context.md) | Canonical checked crypto consumers; expanded selection passes 432 tests and strict lint. The preceding shared codec checkpoint passes 1,868 tests; native metadata budget corrections have separate follow-up qualification. |
| [Foundational JSON contexts and native metadata ownership](base-json-context.md) | Coherent codec/model suite passes 1,962 tests, including both compiler UI suites; five-package strict Clippy passes. Base FFI/transparent variants each pass 93 tests; final reconciled crypto build and 430 selected tests pass after test-only lint and formatting corrections. |
| [Asset identity ownership and reconciled callers](asset-foundation.md) | Base FFI and transparent variants each pass 110 tests; the combined Norito/base run passes 1,508 tests, strict Clippy and five base documentation cases. Twenty dependency boundaries and 797 Python contracts pass at their recorded checkpoints. Aggregate consumer qualification remains open. |
| [Versioned JSON ownership](version-json-context.md) | Borrowed envelope decoding, direct checked writing and admitted diagnostic copies; 22 tests, all eight compiler UI cases, strict Clippy and the documentation example pass. |
| [SoraFS contextual JSON and cleanup](sorafs-json-context.md) | 1,083 tests pass, including byte-identical fixture regeneration and deep ordinary cleanup. Strict Clippy retains proof/signer findings; aggregate migration remains open. |
| [Aggregate contextual JSON migration](aggregate-json-context.md) | Primitive selection passes 328 tests; embedded JSON passes 548 Norito tests and subsequent strict Norito lint. Crypto scalar ownership passes three tests and strict lint; the updated base owner passes 112 tests and strict lint; aggregate library check 24 passes with zero warnings; the canonical-length codec passes 558 library and four allocation/drop tests plus strict all-target lint; aggregate test migration remains open. Source-bound diagnostics and pending consumers are recorded. |
| [Native JSON ownership and nested resource errors](json-native-ownership.md) | Bounded native retention replaces recursive cloning; 556 library tests, four actual allocator/destructor tests and strict Norito lint pass on ordinary stacks. |
