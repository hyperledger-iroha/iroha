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

After extracting `Name`, `StatePath` and `ParseError` into `iroha_model_base`,
the rebuilt Musubi resolver again passes all 34 original tests without a stack
override. The iterative search and original tests remain byte-identical;
only their canonical `Name` import changes. A new production-boundary regression
then covers depth-64 success, depth-65 rejection, exact conflict-chain ownership
and deterministic results with reversed registry rows. It retains every original
test and exercises continuation/conflict-chain destruction on ordinary workers.
The complete library passes **386 tests, zero failures and one ignored
subprocess-only crash worker exercised by its parent**, on four workers.
The 5,655 selected inputs remain unchanged during build and execution:
source `52bdb44a540ea0d84146b28646e98582751d1eedd90bf31c3640a0f03121413a`,
executable `99be236c956c98724d202f39d2e30ec85604dd2cf546fa7875b3834dae9724f6`.
The records are `musubi-depth-build-1` and `musubi-depth-runtime-1` under
`target/architecture-redesign/model-base-extraction-v1/`; the preceding unchanged
resolver run is `musubi-current-resolver-1`. These results qualify this selected
model-consumer source; the earlier SDK/lint results retain their separate scope.

Strict library-and-test Clippy subsequently found three loads of the same
persistence-fixture module and a needlessly owned test-fixture URL. The three
consumers now share one crate-private test module; every frame assertion remains.
The rebase fixture owns its fixed offline endpoint once, preserving the exact
URL previously repeated at all 33 call sites. Final build, full runtime and
strict library-and-test Clippy pass on the same 5,655 unchanged inputs:
source `0dcd9468a898319a4f14ff6700fb53922b7145bbbb595d3a904b348a71363934`,
executable `d833b7def560feb427fbf2c89d2d8f9cc4329c143e288c1dee34ac3a649fc93b`.
The full run again reports **386 passed, zero failed and one subprocess-only
ignored worker exercised by its parent**, including all 35 resolver regressions.
Reports are `musubi-final-build-1`, `musubi-final-runtime-1` and
`musubi-depth-clippy-4`. Failed lint attempts, including an exhausted-disk run,
remain recorded separately. Workspace formatting and codec guards also pass.
The final 34-package all-target consumer check passes with compiler warnings;
its larger input closure is recorded in the [model checkpoint](model-base-name-state-path.md).

After the Metadata move and SDK test-helper cleanup, both packages build and
execute from the same 5,658 unchanged selected inputs, source
`c7c4b856f82bec9e157d1724fb69126e9b1b94181b55a0e8c10eaefbc3ad326e`.
SDK passes **802 tests**; Musubi passes **386**, including all 35 resolver
regressions on four ordinary workers and its subprocess-only crash-worker
contract. Strict library-and-test Clippy passes for both packages, and all ten
SDK doctests pass on that same source. Musubi has no doctests. The original
overflowing 512-edge case and the depth-64/65 regression both pass. SDK executable
SHA-256 is `3de1324fd6f4d5432c428da0b4e246ab776b6ca67a1a58237b3e7804bee4d589`;
Musubi is `f5186641222f38b1faa44407d54abfb5deab7f5e88339855bb16b363b127c9aa`.
Reports are `metadata-sdk-musubi-build-2`, `metadata-sdk-runtime-2`,
`metadata-musubi-runtime-2`, `metadata-sdk-musubi-clippy-3` and
`metadata-sdk-musubi-docs-1`. The
[Metadata checkpoint](model-base-metadata.md) records the broader consumer and
tooling scope; these results do not close workspace, memory or release gates.

After the ChainId owner move, `chain-model-sdk-build-1` and
`chain-{sdk,musubi}-runtime-1` pass **802 SDK and 386 Musubi tests** on one
unchanged source, `139dcaac2274565b4d267a6287d1c0afa324662da99a6a22a427dd3d84657bea`.
Both use four ordinary workers; the 35 resolver regressions and subprocess
crash-worker contract pass. `chain-sdk-musubi-clippy-1` and
`chain-sdk-musubi-docs-1` also pass strict library/test lint and all ten SDK
doctests on that same source. See the [ChainId checkpoint](model-base-chain.md) for
aggregate and consumer qualification.

After the DomainId move, one composed source again passes **802 SDK tests and
386 Musubi tests**, including all 35 resolver regressions on four ordinary
workers. Source is
`69cf733d700c512abe5e769cead45fd3105452f3e2aa66623ddfbbf2c545c8c2`
with all 5,663 selected inputs unchanged. SDK executable SHA-256 is
`8ad129385a0e48a1da89fc6cc502bfde5f19ef0b5dead869d04b6cb9fdb5f16e`;
Musubi is `3461e5f41737de59048a6e650e07e9f55c4f159af96717f4b005c653927991a7`.
The search implementation retains SHA-256
`a548f9e155819e50b56b89197bb67d2fe66855721c29c8f8d0846094f0786e3d`.
Musubi's one ignored subprocess-only crash worker remains exercised by its
parent. The reports are `domain-sdk-runtime-1`, `domain-musubi-runtime-1` and
`domain-resolver-root-cause-checkpoint-1` in the model extraction evidence root.
The [Domain checkpoint](model-base-domain.md) records the simultaneously passing
base/aggregate suites and separates the remaining qualification.

After the numeric topology extraction, the composed source
`09d3aff4126085899eeef7b305bc0a58a8e351c4e0269627ef456e371033db78`
passes 77 base, 3,675 aggregate, 802 SDK and 386 Musubi tests. All 35 resolver
regressions retain their passing outcomes on four ordinary workers. The
iterative search source is unchanged. Musubi executable
`9f94be93d540193e1cd96441a6e7f57793d3535309d0cf20627a364d4c366d3a`
and SDK executable
`7f19681cba2a68937ef16ad9ed4e029d8df71c50263c2afa06d5f4e4b10b22d0`
are bound by `topology-resolver-root-cause-checkpoint-1.json`. Broader release
qualification remains outstanding.
