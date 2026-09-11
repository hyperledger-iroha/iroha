# Chain label compilation boundary (2026-09-10)

`iroha_model_base::chain::ChainId` owns the deployment-selected ASCII label,
bounded parsing and structural codecs. Private `ChainIdText` and `ChainIdWire`
remain private. The sole grammar and byte-limit definition remain in
`iroha_primitives::chain_id`. Genesis-derived `NetworkId`, `IdBox` and ledger
composition stay in the aggregate. Superseded aggregate root, prelude and module
exports are removed; callers import the canonical owner directly.

The owner move preserves implementation tokens, explicit nominal identities and
all six original validation tests. Two additional tests exercise every valid V1
layout, including invalid-label rejection and restoration of decode context.
The 14 private-helper fixture records now execute in the base owner. The
aggregate retains the other 175 records and complete-capture cardinality,
schema and key assertions. The original 155,592-byte fixture remains unchanged,
SHA-256 `9eaba83f63302101a16a324d07bd85bd316dfc1cea8eca7bbafb876e0b57342a`.

The coordinated source application changes 202 paths. Its explicit migration
changes 101 imports and 133 qualified paths in 142 files, preserving all 5,230
literal test names and 23,191 assertion macros. Scope review adds 49 imports in
47 files and checks included fragments and inherited bindings. Torii's base
ChainId import is unconditional because its state and constructors use the type
without `app_api`; other capability imports retain their original predicates.
The real Git index remains unchanged.

Evidence is retained under
`target/architecture-redesign/model-base-extraction-v1/`. These are selected
development checks, not release qualification or measured memory evidence.

| Check | Recorded result |
| --- | --- |
| Base features | `chain-base-{default,transparent,ffi,combined}-{build,runtime,allocations}-1` pass 55 library and three allocation tests per configuration, with no failures or ignored tests. Default/transparent source is `4867108a3c7375c7f7cbb4397ddf8ee8740f2c0d6b3f2df7e4364da57e0151c1`; FFI/combined source is `4351087bb4da057b4a74ddb2026d935430dc9c99b22a80f19be79b143847d98e`. |
| Strict lint | After fixing one documentation-backtick diagnostic, `chain-base-clippy-2` passes combined-feature library/test Clippy with `-D warnings`. Its source is `0590b3c1c82f962900d063085a678983e1707dccea100525089697c99069562c`; the preceding runtime records retain their own source scope. |
| Dependencies | All 20 feature-resolved normal/build boundaries pass. Cargo.lock changes exactly seven local dependency lists and no external package versions. Base adds only two test dependencies; six consumers add their direct base dependency in the actual normal or test scope. |
| Budgets | The manifest fingerprint is `sha256:e5f72c939324ba46a544297ce6d3efa7ad57bccb1ac4c721df308046d1f24ffe`. Workspace declared/required edges change from 1,711/1,579 to 1,719/1,587; external edges increase by one for the base hex test dependency. No headroom or source-size exceptions are added. |
| Tooling | Dependency, generated-artifact and source-budget tooling pass 120 tests. The generated registry passes for 281 outputs using a private index containing the 18 new task sources; the real index is unchanged. The canonical OpenAPI lock-pin generator and check pass for lock SHA-256 `1d26e7894baca72d6ebfe4c77c15316df694f629b9a78f0a50fa0f1300c8dfd2`. |
| Consumers | `chain-consumers-check-1` passes all targets in 43 packages with `xtask/dev-tools`, `mochi-integration/dev-tools` and `mochi-ui/gui`. All 9,271 selected source paths remain unchanged across the 610.39-second check, source `fc3ad427b95d723a6c7d33aa29a78aba870638f55ebed64ed7cd8b22145475ac`. Compiler warnings remain; this is not strict whole-workspace lint. |
| Current runtime checkpoint | `chain-model-sdk-build-1` and `chain-{base,aggregate,sdk,musubi}-runtime-1` pass from the same 5,660 unchanged selected inputs, source `139dcaac2274565b4d267a6287d1c0afa324662da99a6a22a427dd3d84657bea`. Base passes 55 tests, aggregate 3,692, SDK 802 and Musubi 386. The aggregate retains six ignored fixture printers; Musubi retains one subprocess-only worker exercised by its parent. All use four ordinary workers. The resolved model features include `transparent_api` and `test-fixtures`, without `ffi_export`. |
| Final consumer checkpoint | `chain-consumers-check-3` passes the same 43-package all-target selection after removing one unused Kagami test import and extracting Mochi CLI parsing. Source is `cf9fe919e353815fd653572e8e1071278d6c4576221b82aaa7dc9a8a3b62b91d`, 9,273 unchanged inputs. The intermediate check exposed missing private parser imports; its failure remains recorded. |
| Aggregate FFI | `chain-aggregate-ffi-build-1`, `chain-aggregate-ffi-runtime-1` and `chain-composed-base-ffi-runtime-1` pass with both `ffi_export` and `transparent_api`. Source is `4519bb0a10ce1f02d0ae33b53d92380f7b218845f513243f59d8c4ce8ce4e936`, 3,863 unchanged inputs. Base passes 55 tests; aggregate passes 3,692, including four actual linked-export/deallocator checks and both mixed golden-fixture checks, with six ignored fixture printers. This selection omits the four HTTP tests enabled in the SDK checkpoint. |
| SDK/Musubi strict lint | `chain-sdk-musubi-clippy-1` passes library and test Clippy with `-D warnings` on the same `139dcaac…` source as the complete SDK/Musubi runtime checkpoint. |
| Final owner matrix | `chain-base-final-{default,transparent,ffi,combined}-{build,runtime,allocations}-1` pass 55 library and three allocation tests in every configuration. The unchanged default/transparent source is `a276f8caee71e2d125b397a7291a52f37926b43071700a3fe954c266f3ecc65a`; FFI/combined is `0590b3c1c82f962900d063085a678983e1707dccea100525089697c99069562c`, matching the strict owner lint checkpoint. |
| SDK documentation | `chain-sdk-musubi-docs-1` passes all ten SDK doctests on the same `139dcaac…` source as its full runtime and strict lint. Musubi has no doctests. |

Wider workspace/native/network qualification and the pinned-runner memory comparison
remain pending.

The source-size gate initially reported 250 findings and the same 170
exceptions. The canonical ChainId import took Mochi GUI above its 11,900-line
no-growth baseline. Its subsequent [CLI owner extraction](mochi-cli-owner.md)
reduces the GUI to 11,200 lines and tightens its ceiling to that value. The
gate now reports 249 findings; no exception is added or expanded. Continued
decomposition to the 5,000-line production limit remains required. Workspace
formatting, retired-codec checks and the regenerated environment inventory pass
at their recorded checkpoints.
