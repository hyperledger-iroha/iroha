# Metadata compilation boundary (2026-09-10)

`iroha_model_base::metadata` owns Metadata, Path, bounded decoding, canonical
JSON, streaming serialization and the complete owner tests. Ledger entities
retain `HasMetadata` in the aggregate. Consumers import the base owner directly;
the aggregate Metadata module, root/prelude exports and duplicate generated API
lists are removed. The crate entry point now owns the aggregate export inventory.

Metadata retains its declared Norito identity, schema identity and field order.
Its six inspected production implementation bodies are unchanged apart from
their owner imports. All 14 owner tests move with the implementation, including
the separate three-test allocation executable. Strict lint required moving the
unchanged inline test module after production items and a local type declaration
before statements; original runtime assertions remain intact.

The base FFI dispatcher owns Metadata at handle ID 3. Aggregate dispatchers retain
IDs 0, 1, 2, 4 and 5 and the single global deallocator. Four aggregate tests call
the real linked exports to exercise cloning, comparison, disposal, foreign-ID
rejection and combined ownership. Their execution passes against the rebuilt combined artifact below.

Development evidence lives under
`target/architecture-redesign/model-base-extraction-v1/`. These records do not
establish release readiness or a measured compilation-memory reduction.

| Check | Evidence and scope |
| --- | --- |
| Owner features | Default, transparent, FFI and combined selections each pass all 46 library tests and three allocation tests, with zero failures or ignored tests. Final reports are `metadata-base-{default,transparent,ffi,combined}-{build,runtime,allocations}-2`. |
| Source binding | Default/transparent selected inputs retain source `9e328438d5679c1933a5ef11690833c7219e591c088af8058c61305bff8cffb7`; FFI/combined inputs include the optional FFI crates and retain `8a62d1c87a79c22d0ddf49fa5a7142e9a8bc263aff1c624350c0739a22acb44a`. No selected inputs change during these builds or executions. |
| Aggregate model | `metadata-aggregate-build-2` and `metadata-aggregate-runtime-2` pass 3,694 library tests, including both complete captured wire-fixture checks. Six existing fixture-regeneration printers are ignored. Source `88f71f7fa4ae858ddd3b0d79d254007b8017cfc7ec299fcd3220431002be0aad` and executable `c4aea4597fac2871714082bb8054e22a2dfba64ae1775ad1e546264fb75bff2b` remain unchanged across build and execution. |
| Strict owner lint | `metadata-base-clippy-2` passes library and integration-test Clippy with combined FFI/transparent features and `-D warnings`, on the same FFI source above. The earlier ordering failures remain recorded. |
| Combined FFI | `metadata-aggregate-ffi-build-1` and `metadata-aggregate-ffi-runtime-1` pass 3,698 library tests, including all four actual-export regressions and both wire-fixture checks. Six existing fixture-regeneration printers are ignored. Source `ccee27003d3c704d941912185828b8a24cef8fdd5c13063e514106e2def2a71a` and executable `65fbb55e91fdc96c97560c3f9588e6cf68e407adeaf3869b82ba31929fe2d8a4` remain unchanged across build and execution. This host FFI run does not qualify JNI packaging or devices. |
| Dependency graph | All 20 feature-resolved normal/build boundaries pass. Cargo.lock changes only two local dependency lists for Mochi and the Python Rust extension. Two existing development dependencies become normal dependencies for their actual consumers. Exact edge budgets are refreshed to `sha256:d315669fd88ea3d6a71e4f6b075daa5bdab35777fa35e140297956e1f6538540`; no external package changes or budget headroom is added. |
| Tooling | The dependency, generated-artifact, source-budget, SoraFS reference-fixture and SCCP vendor tooling selection passes 220 tests. Six existing structural PoP fixtures now have their actual generator owner registered; the stale test for the previously removed CBSI generator is deleted. The canonical OpenAPI lock-pin generator updates the pin for the two local dependency additions; all 19 owner tests pass. Codec retirement, historical reconstruction and diff checks pass. |
| Consumer compilation | `metadata-consumers-check-5` passes all targets in the 37 selected packages with `xtask/dev-tools` enabled, including the feature-required developer binaries. Source `62f594b063332ec15ed9711021baa81560f807caea3afdef0fecdb8ccc76f9ef` remains unchanged across the 69.08-second check. Existing compiler warnings remain; this is not strict workspace lint or runtime qualification. |
| SDK and Musubi | Final build `metadata-sdk-musubi-build-2`, both `metadata-{sdk,musubi}-runtime-2` runs, `metadata-sdk-musubi-clippy-3` and `metadata-sdk-musubi-docs-1` retain source `c7c4b856f82bec9e157d1724fb69126e9b1b94181b55a0e8c10eaefbc3ad326e`. SDK runtime passes 802 tests and all ten doctests; Musubi passes 386, including all 35 resolver regressions on four ordinary workers, with its one subprocess-only ignored worker exercised by its parent. Strict library-and-test Clippy passes for both packages. Musubi has no doctests. |

Static import staging preserves original test and non-import tokens. Compilation
then exposed two inherited Metadata uses in the SoraFS instruction tests; a
direct owner import repairs their actual scope. `metadata-aggregate-build-1`
records that failed attempt, and the corrected aggregate run above passes.
An existing bridge test fragment has no module/include reference; its import is
migrated locally, but its execution is not claimed.

The first combined SDK/Musubi artifact also passed 802/386 tests on source
`1d2197ae842934367da972eba39f24a376eb7452f732b67df20bfa5db934ea9d`.
Expanding strict Clippy to SDK test targets then found 50 diagnostics. Test
helpers now have explicit lock lifetimes, coherent setup/assertion functions,
appropriate borrowing and private-module export visibility. Production
implementations, original test names, numeric fixtures and all assertion
macros remain preserved. The failed lint reports remain recorded. Final SDK
executable SHA-256 is `3de1324fd6f4d5432c428da0b4e246ab776b6ca67a1a58237b3e7804bee4d589`;
Musubi is `f5186641222f38b1faa44407d54abfb5deab7f5e88339855bb16b363b127c9aa`.

The third consumer check passed default-feature targets but skipped the
feature-required developer binaries. Its two introduced import warnings were
corrected. The fourth check enabled `xtask/dev-tools` and exposed test imports
previously inherited from the removed escrow glob, plus two developer tools
still calling retired Norito APIs. Escrow tests now import their actual owners
explicitly. Nexus uses the canonical framed decoder, and the streaming benchmark
uses the codec-owned framing helper with its existing payload and layout flags.
The fifth check above passes every selected target after these repairs. The
earlier failed check remains recorded. Developer-tool execution subsequently
passes as recorded below.

The SoraFS tooling split places every resulting source and test file
below its limit, removing one existing exception. Final build
`metadata-xtask-build-3` and the four `metadata-xtask-*-runtime-2` runs retain
source `f3004e7ef8de6a25f73911be134f801be86987a63260b526a7d27f6a8850adfd`:
90 SoraFS tests, ten Nexus tests, four streaming encode/decode integration tests
and the fetch-fixture integration test pass with no ignored tests. The first
SoraFS run passed 88 and failed two stale assertions. The corrected tests decode
canonical decimal XOR quantities and require the actual council chunk-digest
rejection for a corrupted payload; production validation remains unchanged.
The reserve test now uses Norito JSON. Its test executable is
`f139c25fcea12b454ae6dc214e05f417d6c8d549d844434b5de141984399ed62`;
the production tool is
`50d58b067fb577f8a801035429306c2d90fc0da360955aa43a928d3be1f76dae`.
The scoped driver distinguishes binary test and production profiles and verifies
both retained and invoked subprocess binaries before and after execution.
Its first ambiguous-artifact reporting failure remains recorded separately.

The 17 vendored-Go ownership
findings are resolved by three upstream owners and a bounded read-only verifier.
All 643 archive entries authenticate against the existing module and go.mod checksums before the selected 17 files match;
both network and offline checks pass. The generated-source guard passes for 281 outputs against a separate candidate
index containing the two new verifier files; the real Git index is unchanged. Native Go compilation and upstream
generator execution are not performed. Remaining
model extraction, full workspace/strict lint, mandatory four-validator execution,
native/device qualification and the pinned memory comparison remain open.

The first two 37-package consumer checks exhausted the filesystem while writing
compiler artifacts, before complete qualification reports could be produced.
Their available logs, input seals and explicit failure records remain preserved;
the recorded compiler errors in both attempts are filesystem-capacity errors.
Cleanup inventories 5–8 record removal of seven obsolete Core/model rlibs,
160 obsolete consumer rlibs and 571 obsolete metadata files from this task’s
inactive cache. Each removed artifact predates its owner’s recorded source
migration; source, current artifacts and retained executables/evidence remain
untouched. Inventory 9 then records removal of 412 obsolete incremental-cache
directories from those same migrated packages: every entry predates the Metadata
application, and an open-file check confirms the slot is inactive. Available
capacity rises from approximately 5 GiB to 125 GiB, permitting developer-tool
runtime compilation. The earlier disk failures remain failures.
