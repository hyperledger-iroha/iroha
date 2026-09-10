# Musubi SDK and resolver checkpoint (2026-09-10)

The resolver's recursive search added one native call frame per processed edge,
including edges in shallow graphs. Its unchanged 512-edge backtracking regression
overflowed an ordinary test-worker stack. The search now stores branch
continuations explicitly in a `Vec` and drives descent and backtracking in one
loop. Candidate order, locked-choice priority, duplicate-version minimization,
conflict selection, global attempt accounting and graph limits remain intact.
No stack override, reduced graph limit or optimization change was introduced.

Retained ARM64 debug disassembly gives the original recursive search a
6,864-byte static frame per call. The iterative search frame is 3,168 bytes;
its largest direct branch helper is 4,208 bytes. These are function-entry frame
sizes, not complete call-chain or cross-target peak measurements. The change
removes edge-count growth in native call depth. Exact prologues and artifact
identities are retained in `resolver-stack-root-v1/artifact-frame-comparison-v1/`.

The original solver passes 33 resolver cases when the crashing case is excluded;
the corrected solver passes all **34**, including that unchanged regression.
The entire original resolver test module is retained byte-for-byte. Independent
source review found no semantic difference in the eight reviewed search
invariants. The passing build source fingerprint is
`055990ed3366926e8ca1e5067531f79db5cc704915d804b22eadfe1741af27bd`,
and its executable SHA-256 is
`158e4dd6c18a7dfda82db0829922df633a76ae7ab3f35d389ab312cffa0ab031`.
The full consumer run on that artifact reports **352 passed, 20 failed and one
ignored**, without an overflow. This result does not qualify later edits.

The Rust SDK now exposes twelve typed Musubi reads only through the authenticated
account capability and its explicit blocking facade. Requests retain fixed route
selection, exact signing, context-owned transports/deadlines, bounded responses,
explicit address formatting and request/response binding. The arbitrary-route
free function and overlapping public wrappers are removed. Registry consumers
retain one owned account runtime. See the [SDK inventory](../../sdk_inventory.md)
for operation and authority details.

The pre-follow-up SDK candidate passes **800 library tests, ten doctests and
strict library Clippy**, with no failures or ignored library tests. Its source is
`db886cb064f5912571fc47610336d190098a6292750fface4fe2228e55e8bd6a`,
and executable SHA-256 is
`4c63f42b02fed2b975a7dd88091253bc72cfcdb31c030ca3b130350cf544f4f1`.
These results precede the transaction-status binding and model follow-ups below.

Thirteen Musubi persistence roots now declare their captured canonical frame
identities. The retained reference corpus checks 39 root/container identities
in both codec directions and 32 complete frames. Complex publication workflow
assertions remain in their original suites; the bounded identity capture is not
a substitute for executing those workflows. All three persistence identity
regressions pass in the 352-pass consumer run.

The twenty consumer failures identified three adapter defects and stale fixtures.
Builds, packaged publication checks and workspace tests now share one canonical
compiler identity encoder over structural fields. Package paths explicitly use
the pinned NFC normalizer before strict Name parsing; wire parsing remains strict.
Transaction-status hash substitution now retains a typed response-binding error.
Fixture timeouts and request echoes follow the unchanged bounded reader/model
contracts. Three exact compiler-identity module tests pass against retained model
libraries. The next scoped build stopped
at a concurrent `ContractInstance` derive merge conflict before compiling Musubi.
That merge is now resolved. The fresh scoped build and runtime retain source
`e3703d202963f7f7b447dd8b50f1b469e2b516c02e916dee29aeb6469d8bb746`
and executable `71c8f36abfd8f7799e91cab0a846c87c55632f0972af64bed7089aaad4a6b91c`.
That run reports **370 passed, five failed and one ignored**. Its five failures
exposed a comment-only `musubi new` scaffold rejected by the canonical compiler.
The corrected scaffold emits a valid module and canonical function placeholders.
Existing source retains its actual type exports, including names reserved only
for functions; immutable installation rejects conflicting concurrent creation.
Actual compiler Check/Build cases and source-race regressions cover both paths.

The next full run reports **375 passed, three failed and one ignored**. All three
failures exposed eager signer construction during local publication recovery.
Recovery now validates the public address profile and exact cached graph first.
Only an online cache miss constructs the authenticated registry reader and storage
transport from the same bounded configuration image. Original invalid-key and
missing-provider-key fixtures remain unchanged. Added cases cover online cache
hits, required authentication after a miss, and strict public-profile projection.

The first full run with these fixes passes **385 tests**, with no failures and one
ignored abrupt-exit worker invoked by its parent. A subsequent production check
found the typed diagnostic accessor was test-gated; it is now available in the
production path. Private-module helper visibility also follows workspace lint
policy. Strict Musubi library Clippy passes on source
`ae9e141fe45ee3ababf738707c984f9e4ca983540746a43e8fabb0cd9a51221e`.
That rebuilt library again passes **385 tests with zero failures**, including
all 34 unchanged resolver regressions, on four ordinary-stack workers. The one
ignored subprocess-only crash worker is exercised by its parent. Its executable
SHA-256 is `7c74fc615abfc0622eb2a1db3c802e05b0aad1a94157054c1c686da6df6fadd4`;
the same 5,651 selected source/fixture inputs remain unchanged during build,
strict Clippy, runtime and all-target compilation checks.

After final formatting, the current candidate repeats those **385 passing tests**,
strict library Clippy and all-target checks on source
`721462511a31c83db74b61dd5128e7f1cd144883913e20b885f8d1f495da8307`.
Its retained executable SHA-256 is
`bddf4b11f176bdddf137a0a8d9f887f25f751ed4b846b8b63a1726e29252830a`.
The same selected input set remains unchanged in all four checks.

The merged SDK passes **802 library tests, ten doctests and strict library
Clippy** on source
`3f1d8412aa5437e8032a8b64f09e1fde696ee446c7bd5c357169fad0bcfef379`;
its executable SHA-256 is
`e5be0e985bfa9c62a9ff5df05b3dfc19ae287bf449a9598f613ede25447f8e3c`.
The current wait tests check the exact typed hash-binding error in both async
and blocking contexts and still require exactly two requests.

All **25 Name-model tests** pass on source
`c1f915e2c00abbe66f06d70001858a27574fd9848661ce60085b660fda32e9f2`,
covering pinned normalization, allocation bounds, strict parsing and codec
rejection. The retained executable SHA-256 is
`c8c23d2138991628e9db39fce361fe1e715cc84aba27aa7b4b52f117eb187822`.
Only the Name selection ran; the other 3,721 model tests were filtered out.

All 16 feature-resolved dependency boundaries, 70 dependency/codec guard tests,
the 665-route SDK inventory, workspace formatting, codec retirement and historical
archive verification pass. Numeric dependency budgets still hold, but the merged
manifest fingerprint needs an integrated review. The source-size guard still has
249 findings; no limits or exceptions were expanded. These guards do not establish
the required build-memory reduction or release-source provenance.

Raw source seals, artifacts, unchanged original tests, failed runs and independent
reviews are retained under `target/architecture-redesign/sdk-musubi-capability/`.
The resolver evidence is `resolver-stack-root-v1/`, `resolver-reference-1`,
`resolver-iterative-1` and `musubi-consumer-runtime-2`; the SDK evidence is
`sdk-build-6`, `sdk-runtime-final-4`, `sdk-docs-2` and `sdk-clippy-4`.
`consumer-root-fixes-v1/` records the follow-ups, and `musubi-build-4` retains
the failed merged-source build. `musubi-build-5` and `musubi-consumer-runtime-3`
record the resolved-merge run. `scaffolder-source-choice-v1/` and
`recovery-credential-boundary-v1/` retain independently reviewed repairs;
`musubi-build-7` and `musubi-consumer-runtime-5` retain the pre-lint runtime.
The pre-format declaration checks are `musubi-build-8`, `musubi-consumer-runtime-6`,
`musubi-clippy-4` and `musubi-all-targets-2`. Final formatted reports are
`musubi-build-9`, `musubi-consumer-runtime-7`, `musubi-clippy-5` and
`musubi-all-targets-3`; `model-name-1` records the focused model run.
Current SDK reports are `sdk-build-8`, `sdk-runtime-final-6`,
`sdk-docs-3` and `sdk-clippy-6`. Workspace, memory, native/device and
four-validator release qualification remain open.
