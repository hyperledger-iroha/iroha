# Status

Last updated: 2026-09-09.

Iroha 3 is under active first-release implementation and qualification. The release is not qualified. Signed source checkpoints do not establish runtime readiness. This page records current findings and bounded local evidence; it is not a substitute for the release gates in [the roadmap](roadmap.md).

The previous dirty working copies are preserved in the [dated historical archive](docs/history/2026-09-06/index.md). Its manifest binds every source occurrence and reconstructs the original bytes. Historical pass counts, plans, dates, and former policies do not attest the current candidate. [Repository ownership](docs/repository_map.md) and the [architecture record](specs/first_release_architecture_redesign.md) provide detail.

## Current implementation and local evidence

The current Taira rollout remains unqualified. The deployed candidate stalled at genesis with ordinary transactions queued; the source fixes ordinary multi-route candidate starvation and proof bounds. The native network gate then exposed source-writing fixture staging and port allocation before peers started. Fixtures now use Cargo OUT_DIR, and test ports use runtime OS leases. No public finality or application rollout is established.

A code-first consensus review found three scheduling ownership defects: worker backpressure closed admission during deferred Apply retry; a view change or unrelated merge cleanup could strand a QueuePlan handoff; and pending Native participant evidence could terminally suppress an owned reservation. The current source retains exact retry owners and uses an explicit handoff state machine. Its dependency-free production reducer suite passed 205 tests, including four-reducer loss/reordering/restart traces, and the release runner's 83 Python/fixture checks passed. The integrated run passes all 23 selected Core regressions, including both participant and coordinator repair with real reservations and production Kura repair. The planner preserves authenticated interrupted publications as pending, including coordinator and planning-race paths; corruption remains fatal. Native Core incremental rebuilds measured 80.8 and 55.7 seconds after the initial 440.9-second build. The four-validator gate exposed a fixture that executed custom genesis before receiving its final Nexus configuration; it now defers execution to the builder. Native build/test profiles share split-debug settings, and the focused network gate lives in `iroha_test_network` to avoid unrelated integration dependencies. The next native run built both binaries and the focused harness, then stopped before peer startup: repeated codec fixture copying inherited the sealed source's read-only permissions. The fixture now embeds canonical bytes, reuses matching read-only files and replaces divergent bytes atomically; remaining custom multi-lane genesis callbacks defer execution to the final-config builder. The subsequent warm build took 13.0 seconds for native binaries and 6.4 seconds for the network harness. Child startup then exposed an omitted test storage cap: production auto-sizing reserved 20% of the shared host disk. Test peers now use an explicit 1 GiB cap; the gate checks 8 GiB free before compilation and startup and retains peer logs. The combined source audit covers configuration, startup capabilities and durable paths, exact lane authority, fee quotation, Ordinary admission, execution and status resolution. Validator clients now ignore ambient identity and endpoint overrides; the gate requires exact local and global Applied state on all four peers at the same height, with bounded observation requests. The next runtime run applied genesis on all four validators, then rejected the fixture's explicit Ordinary public submission with HTTP409. Public ingress requires QueuePlanSynced; the fixture now uses that contract. The same audit found three real Inrou pin operations forcing Ordinary before generic public submission; their producer and retained-envelope validation now require QueuePlanSynced, with the sponsored-pin regression added to the mandatory CLI gate. Internal onboarding/faucet and dedicated SoraFS Ordinary paths retain their own validated admission contracts. The corrected four-validator test and remaining native gates are required before cross-compilation and deployment.

[Executable build identity](docs/build_identity.md) removes revision stamping from shared Core, Torii and telemetry libraries. The earlier controlled revision-only warm build completed in 25.073 seconds with six shared production/test artifacts cached. Diagnostic and release commands now use separate persistent Cargo targets and the same isolated registry paths; 61 helper tests and 65 CI workflow tests pass. The [retry command](docs/source/taira_retry.md) reclaims closed public payload copies before fresh deployment-capacity admission, reuses unchanged binaries, and accepts recovered native rollback history only at the completed boundary. Its 51 focused tests pass. These are scoped local checks; no new full-build timing or live release qualification is claimed.

The 110-path merge resolution retains the final StateScan/cursor ABI, validated seven-field Native AMX control, immutable asynchronous account contexts and shared signer custody. Current source bindings are checked before deferred-carrier classification. Fresh custody tests pass 23, snapshot resource tests pass 4, JavaScript selections pass 59 and all 55 consensus source contracts pass. Workspace formatting, scoped conflict-diff checks and codec retirement checks pass. Current IVM ABI/artifact/gas/pointer selections pass 17/58/2/4, including exact SDK fixture reproduction. SDK/Core/Torii/JS-host/CLI/Kagami/daemon production checks pass; workspace, native SDK and four-validator qualification remain open.

Earlier merge checkpoints are preserved in the
[dated merge record](docs/history/2026-09-08/architecture-merge-checkpoints.md).
Their remaining limits include the ABI-23 Swift bridge, the macOS Python
framework-copy assertion and clean-candidate artifact/provenance replay.

Core validation retains its staged state on the heap through event delivery;
the former stack-overflow regression and all 163 block-validation tests pass on
the default thread stack, including explicit fixture cadence enforcement.
Asset registration now uses the signed owning domain
for authorization, with positive and alias/foreign-domain negative coverage.
Admission and block fixtures use current signature policy, authenticated
four-validator contexts, genesis asset incarnations and explicit contract
lifecycle state. Removed test-only Soracloud callbacks no longer make block
execution depend on a local mailbox runtime; the shipping mailbox admission
gate still rejects execution without consensus reexecution. All 387 distinct
focused regressions pass, including admission, block execution, fees and proof
accounting. Daemon, Kagami and test-network consumer checks, formatting, codec
guards and historical-archive verification pass. Full Core/workspace and network
execution remain unverified by this scoped run.

Kaigi's final model passes 33 tests, its proof circuits pass 20, and the fresh
JS-host native proof suite passes 21, including every 31/25-row mutation. All
three real Core Kaigi integration tests pass; its unit selection passes 96/97,
with a typed capacity-error assertion corrected for the next capture. Strict
Rust account decoding passes 42 cases across all ten Norito layouts. Updated
Kotlin/Java sources compile with JDK 8 API enforcement; three decoder/absence
tests pass. Earlier managed full-controller passes predate mandatory native
key admission. Python's separate native-owner test-target check passes, but
installed SDK wheels/addon/JNI/XCFramework execution remains pending. Fresh
Metal digest/Merkle/runtime tests pass 68 cases; complete GPU proofs, relay
transport and four-validator deployment remain unqualified.
See the [current privacy evidence](specs/privacy_first_release_closure.md).

Production multilane work has [six implementation milestones under an active goal](specs/sumeragi_v2_multilane_completion_goals.md).
No milestone or release gate is closed. The current source adds authenticated
terminal replay, bounded atomic cache retirement, a real second autonomous
application fixture and mandatory move-only authority for shipping Queue release.
G-UNIT registers 531 tests; production registers 881 across 43 modules. The
shared Core build passed with all 22 captured multilane inputs unchanged.
Its first 89 direct runtime tests finished with 39 passes and 50 failures. The
production witness still minted an old model identity and rejected otherwise
valid transitions. That constant is corrected; the real wrapper test now covers
25 producer/committee combinations and rejects altered witnesses. Nonzero BLS
fixture seeds, matching network context and manifest-backed Queue initialization
also repair distinct setup failures. All original assertions remain. Four changed
files were captured for a second build, which failed on unrelated SoraFS/Torii
compile errors before producing a Core executable. No post-fix runtime pass is
claimed. The production trace audit also found an unwrapped 28th replica-release
action; its witness wrapper and exhaustive registration are being repaired for
the next batch. Evidence is under ignored `dist/multilane-validation-20260907/`.

A fresh formal audit reproduced direct FIFO release with active Kura custody,
then lane Commit and WSV application, in the former model and shared predicate.
The repaired relation acquires exact authenticated replica custody at Kura
activation and excludes Commit/application from every release disposition.
Exhaustive TLC passes all 20 configured invariants over 280,818 distinct states;
all 25 exact mutation controls produce their named counterexamples, including
the new 4/19/4-action failures. Four explicit replica/application and release
paths also pass. Verus reports zero proof errors (1,690 and 221 verified), but
its evidence driver rejects a changed input and the stale expected proof count.
The scope audit and clean rerun remain open, as does current-model Apalache.
The current model has its own 18-step Apalache run; the separate earlier-model
run cannot qualify this repair. These results do not establish a live exploit.

Fresh source evidence passes 69 terminal controls, 21 merge-cache semantic
negatives with copied positives, all eight cache owners, and the complete
QueuePlan contract with 14 release-authority negatives. The terminal baseline
also passes after the custody correction. The repaired model/source registration
selection passes all 54 checks, including full production acceptance. Ordinary
ingress passes 24 copied-source baselines, 24 rehashed negatives and its complete
owner contract. Configuration geometry passes 24 rehashed controls; decided-body
serving passes 72 copied baselines and 72 rehashed controls across 19 owners.
Lifecycle certified serving passes all 46 selected checks; the four lane-output
owners pass 43 rehashed controls before and after their seal updates. Ingress
effects and tombstones pass 81 copied positives and 81 rehashed negatives. The last
broader diagnostic reported 71 errors before these corrections; remaining worker
ownership and finalization contracts are under review.
Four inputs changed during that run, so it is development evidence. Four
retired-codec guards pass.

The strict scaling evidence contract now requires fixed-schedule transaction
identities, authoritative Applied results, drained warmup and complete-cohort
latency including the drain tail. Validator checks pass 56 tests/86 subcases;
runner integration passes 13 tests/nine subcases, preserving all old assertions.
The production collector, exact routing/resource observations and real paired
trials remain open.

The host restart deleted temporary raw artifacts and unfinished Core/Apple
builds. Earlier SDK/formal/source observations are recorded in the linked goals
and closure ledger but cannot supply current retained release artifacts. New
checks retain exact inputs, commands and logs in durable ignored `dist/`.
Full SwiftPM needs the real ABI-23 bridge; grouped fixtures need two fresh
Rust-owned regenerations. Source-contract reconciliation, current Rust/SDK
execution, four-peer suites, 10 corridor seeds, the two-hour soak, scaling and
workspace qualification remain open.

Torii's router regenerates the canonical OpenAPI bundle on the ordinary runtime
stack; four exact Rust authority/authentication checks, tracked metadata
verification and all 145 Node tooling tests are recorded as passing.

| Area | Observed evidence | Practical limit |
| --- | --- | --- |
| Build cache invalidation | Build-support watches the Git references needed to resolve HEAD, avoiding rebuilds from unrelated packed-ref updates. All 31 standalone Rust tests pass, including eight new reference-transition regressions. | Legitimate source and commit changes still invalidate affected build inputs. |
| Kotodama V1 redesign | Fresh compiler library tests pass 1,090/1,090, compiler integration 77/77 and Koto/LSP 37/37. ABI values pass 172/172; Core completion/recovery 107/107 includes both actual cold-restart worker completions. Scoped Core contract tests pass 60/60; VM runtime/lists 50/50, ABI/artifacts 72/72, values/scan/test-driver 57/57, numeric/pointers 40/40, data-model contracts 57/57 and Torii public-state 5/5 pass. Offline record consumers pass Python 220, JavaScript 18, Kotlin/Java 17 and C# 21 tests. | C9 remains open. Local debug passes 9/9, including strict NFC keys and exact duplicate-JSON rejection. All ten scaffold tests and the complete actual offline project workflow pass, including three standalone success/exact-failure tests. Tutorials build and link-check with 12 GiB; immutable docs HEAD also exhausts the default 6 GiB heap. Native SDKs, final artifact admission, normal-release four-validator execution/restart and workspace qualification remain open. The failed formal checker has static attribution for 242 diagnostics against HEAD and 34 against the earliest retained resumed snapshot; these distinct limits remain explicit in [the execution ledger](specs/kotodama_v1_redesign.md). |
| Taira reset and startup | Cold-start snapshot handling now treats an owned root with no CURRENT pointer as a fresh chain; all three regressions pass. Validator convergence and restart wait for HTTP readiness inside the existing authorization deadline; all 55 focused release checks pass. Cargo cache retirement preserves the opposite profile family; four focused checks pass. The same approved VM is now 80 GB, with 55.5 GB guest and 41.2 GB host space available after closed public-payload cleanup. | Public Taira remains unavailable pending deployment of these corrections. Release75 passed preseed and process startup, then rolled back completely after an early doctor check; the actual cold-start snapshot error is fixed in source. The complete capacity plan includes all four writable Inrou runtimes and passes 18 helper tests. Finality, signed canaries and test.inori.co.il connectivity remain pending; no release GO is claimed. |
| KAGEMUSHA release work | R5 passed 1,490 full JVM tests, 12 wallet and 108 client managed Android tests; separate host JNI, 13 Kotlin and 24 JS/TypeScript tests also pass. Vendor memory checks pass 138 executions; Base passes 14 tests and two map benchmarks. Core passes 92 tests plus two row benchmarks; journal/recovery passes 54 tests. The real-proof API check passes after a profiling-helper cfg fix. Row-generation RSS falls from 189.6 to 28.3 MB with identical output. | Full aggregate proofs/resource limits, canonical Swift/native release artifacts, governed physical profiles and independent review remain unqualified. Focused host benchmarks do not qualify full-proof/device RSS or strict Clippy. Swift parity source is restored; complete native artifact qualification remains pending. See [current readiness](specs/kagemusha_v1_production_readiness.md). |
| Privacy V1 admission and field carriers | Canonical 48-byte digest and 32-byte Fp4 carriers; complete signed synthetic qualification passes bounded decoding. Rust capability FFI passes 2 tests; corrected Exact12 fixtures pass JS 23, Python 94, Kotlin 16 and C# 13. SDK admission source checks pass 86 regressions; model privacy selection passes 127 tests and the final intent KAT is independently reproduced. Shared six-lane framing passes 39 primitive tests and the ACE selection passes 26; AXT binding passes 76 FASTPQ and 7 Core regressions. | Core native selection passes all 63 tests; the corrected complete proof crate passes 1,299 with zero failures and 13 ignored. Current SDK origin/network checks pass JS 37, Python 103, Kotlin 63, C# 17 and Swift source-only 35. Apple duplicate archive ownership is corrected and three target link controls pass; the previous full XCFramework build was lost during host restart and has no final artifact; complete GPU proof dispatch, AXT authoritative state/execution binding, independent review and four-validator qualification remain open in the [closure ledger](specs/privacy_first_release_closure.md). Synthetic signatures are validator tests only. |
| SCCP TON scoped audit | [Validated fixes and evidence](docs/source/sccp_ton_security_audit_2026_09.md): ordinary transfer funding, bounded replay work, native TL-B parsing, exact checkpoint identity, complete breaker readbacks, builder Git/verifier/attribute isolation, and canonical wire identifiers across Rust/SDKs, circuits and contracts. Earlier focused Rust/Core/model/production compile checks pass. Fresh validation: 459 Python tests with zero skips, 45 TON contract tests, authenticated StateInit write/check, and pinned EVM/TRON compiler plus EVM runtime smoke pass. Rust validator suites pass 22/27 tests, and the compiled Rust wire fixture passes. Policy/proof negatives have positive controls and precise rejection checks. All 8 R1CS identities are freshly measured, with a verified source closure and no pending profiles. | Full Core/workspace and Torii runtime tests are unclaimed. Production keys/proofs, trusted release signatures and authenticated deployment readbacks remain separate release artifacts. |
| Rust SDK dependency separation | Relay accounting moved to `soranet_incentives`; SoraNet policies and shared defaults have one `iroha_service_model` owner. Archive construction, filesystem persistence, orchestrated fetch and DA workflows now live in `iroha_storage_client`. The shipping SDK required graph has 29 local packages, 87 external packages and 272 edges; its boundary checks pass without node, CAR or orchestrator dependencies. Storage-client tests pass 41 cases. Protocol capability probes are isolated per context and shared by clones. Immutable account/operator transaction contexts, signed multisig submission and the explicit blocking runtime pass eight focused tests. CLI-owned queue/witness paths preserve source-relative resolution and scoped authority binding; 15 SDK and 13 CLI config/authentication tests pass. All development binaries and integration-library tests compile; the three development-bin suites pass 40 tests. AccountTransactionDraft and AccountClient::prepare_transaction/sign_transaction now replace all generic helpers and the quote-and-sign composite, with crate-level typed errors; ten focused tests preserve exact bytes, defaults, attachments and context isolation. SDK examples and all CLI/Musubi/Izanami/test-network and integration test targets compile. Four specialized SoraFS wrappers are removed; five focused tests preserve exact instructions, moderation TTL and invariant checks. | The base Client still has mutable fields; remaining specialized preparation APIs, non-transaction error shapes, synchronous capabilities, broader operator families and complete consumer migration remain unfinished. |
| Configuration and status HTTP contracts | Shared configuration DTOs have 22 wire tests, 3 node conversion tests and 14 Core runtime tests. Shared status preserves 32 captured named DTO frames/hashes/JSON; 93 telemetry tests pass. Core, SDK/CLI, test-network, schema generation, Mochi and grouped consumers compile. | Named records are qualified by focused fixtures; arbitrary generic-envelope schema identity is not yet cut over or fully qualified. |
| Core integration fixtures | The `core_api` harness compiles after all 21 identified fixture errors were repaired through canonical APIs. | The new four-validator configuration startup/restart/readback/isolation scenario has not run against rebuilt binaries. Compilation is not runtime evidence. |
| Torii evidence API | Grouped telemetry harness compiles. All 3 evidence runtime tests pass, including genuine four-validator BLS proof-of-possession/signature material and tamper rejection. | These tests do not qualify the full node consensus/release corridor. |
| Atomic private settlement | Historical Core/Torii functional suites pass 520/221 tests, protected-body recovery passes 228, optimized preflights pass 307, and exact-body rejection diagnostics pass 75 focused tests. Four pool-funding regressions and the corrected full Nexus compiler check pass; concurrent source drift prevents current-source qualification. | No completed private settlement. The latest sixteen-process diagnostic fails during activation after durable merge-candidate revalidation shuts down one global validator; all final runtime logs are archived. The preceding `DecidedBodyInvalid` reason remains unresolved. Revalidation now distinguishes byte/digest mismatch and retains the semantic reason; two regressions and existing signing/funding tests await validation after the active Git merge. The pool-before-proof harness execution attempt fails during concurrent IVM/data-model changes before tests run. Ten positive networks, full proofs, fault/leakage/performance, audit and release gates remain open. See [the protocol](specs/private_settlement.md). |
| Norito schema preparation | Canonical identity kernel, derive support, primitive/crypto declarations and strict UI tests pass. All 19 planned base-model declarations preserve 189 captured frames, signatures, JSON, storage keys and schema output. All 25 generated event sets preserve 225 captured frames and 75 JSON values; 12 enclosing event enums preserve their identities. Another 17 Musubi generated types preserve 196 frames across 49 values; 16 governance hash wrappers preserve 256 frames and 64 JSON values. Another 127 generated queries preserve 684 frames across 171 payloads, and 62 privacy/spentness carriers preserve 992 frames across 248 payloads. The complete model group_02 harness passes 178 tests (two fixture writers ignored), including all 28 query tests. The derive suites pass 48 tests, strict Clippy and the executable EventSet documentation example. | Active frame-identity selection remains unchanged. Complete declaration coverage, atomic cutover and model moves remain pending; see [the identity contract](specs/norito_schema_identity.md). |
| Norito migration capture | Eight probe-generator, 17 driver and 32 syntax/context-graph tests pass. Controlled crypto/model captures pass 206/6,112 probes. Another 108 crypto declarations pass nine module suites and seven existing wire/identity golden tests. The reviewed 1,771-declaration model batch preserves every pre-capture identity and its 110 owner-scoped fixture tests pass on the sealed post-declaration harness. Native AMX participant finality now uses a finite `NativeAmxParticipantSettlement` wire record; canonical Rust, Python, JavaScript, Kotlin and Swift fixtures use its domain-separated typed hash and reject the removed recursive field. Norito encoding also returns a typed nesting error before recursive user values can exhaust the native stack. | Generic, local, remaining generated families and other feature selections remain incomplete. The complete local model-library result is recorded below; other feature selections and release qualification remain open. |
| Instruction identity and registry | All 292 generated instruction declarations require captured identities. The original capture retains 357 values and 1,428 frames across 322 types. Five Kaigi private cases now correctly reject retired hash-shaped scalars; their public cases retain exact frames. New canonical private captures preserve root/vector/option/map coverage. Registry fixture updates account for 14 game and three NFT-market additions, with no removed or reassigned wire IDs. | All 409 focused tests and the complete 3,576-test model suite pass; six unrelated cases remain ignored. Other features and release qualification remain open. |
| Generic query identities | Five generic owners, four concrete query records and two typed-hash markers declare their captured identities. Immutable fixtures preserve 96 default and 116 ids-projection frames, marker composition and private decoder budgets. | Default queries pass 202 tests and ids-projection queries pass 203, with zero failures/ignored tests and all 5,109 selected inputs unchanged in each run. Atomic codec cutover, remaining declaration coverage and physical model moves remain open. |
| Generic model identities | Ten declarations preserve 68 captured frames and five FHE signing preimages. InstructionBox retains its wire-pair root projection and nominal generic argument; the borrowed FHE helper exposes encoding only. All five model and four query identity tests pass on the rebuilt artifact. | All 5,613 selected inputs remain unchanged; other feature selections, active codec cutover and physical model extraction remain open. |
| Concrete model wire identities | Seven owned records and two encoding-only adapters preserve 52 captured frames and three projections. DataEvent and DataEventFilter assign explicit tags, reserving disabled Governance variants. The frozen payload-boundary candidate passes 3,599 default/HTTP model tests and all nine governance-disabled normal-consumer tests, with all 19,170 recorded inputs unchanged. | Active identity cutover, model extraction and complete release qualification remain open. |
| AMX wire and recovery | The final corrected-source run passes all 32 Native AMX tests, with zero failures/ignored cases and no stack override, including maximum 4,096-source schema/codec/hash/drop and removed-layout rejection. All 19,297 final inputs remain unchanged and the live model/hash owners match. The earlier nine-case selection passes on its separate prior seal. All 35 merged-model AMX/framing/roster regressions pass on the default stack, including schema/codec/hash/drop for the maximum 4,096-source settlement and rejection of the removed recursive layout. The frozen payload-boundary default/HTTP model artifact passes all 3,599 library tests, with zero failures and six fixture generators ignored, including the maximum-size and removed-layout regressions without a stack override; all 19,170 recorded inputs remain unchanged. All ten retirement regressions pass after binding four-validator fixtures and lifecycle projections to their actual certificates. The lane-fixture and source-identity repairs pass 396 selected Core and seven Torii tests. The replica Queue witness wrapper and corrected recovery fixtures pass all 10 further Core regressions; 19 source tests and 35 subtests authenticate 28 actions and 29 runtime bindings, with 5,104 selected inputs unchanged. The mandatory shared reducer harness passes all 197 tests. The full pinned Verus harness verifies 221 project obligations with zero errors and proof escapes disabled. | Formal execution retains 40 unchanged selected inputs; it is not a complete release seal. Strict lint, workspace, four-validator and native SDK qualification remain open. See the [scoped implementation evidence](specs/first_release_architecture_redesign.md). |
| Storage identities and layout | Before the JSON key migration, the complete revised model library passed 3,446 tests on the default stack, with six fixture generators ignored and all 1,000 recorded inputs unchanged. Seven common-module tests preserve 432 storage frames. Three dedicated metadata tests prove scratch allocations are independent of payload size and preserve exact wire bytes. Block-signature wire/allocation checks pass. | Strict model Clippy reports 129 diagnostics; its dependency-inclusive attempt also fails on six dependency errors. Physical model extraction and release qualification remain open. |
| Nested encoding work | Object-safe SerializePayload owns bare serialization and sizing; typed frames retain NoritoSerialize. All 1,663 codec/derive/primitives tests pass (three ignored), with strict library Clippy. The complete SDK library passes 716 tests on the default stack with all 19,174 recorded inputs unchanged. All selected SDK/storage/Core/Torii/P2P targets compile. | Whole-workspace and strict-lint qualification, source-size budgets, active schema cutover and pinned-runner evidence remain open. Test-only network-fixture corrections have separate manifests; see the architecture record. |
| Payload reconstruction | DeserializePayload owns reconstruction; typed NoritoDeserialize retains framing. Codec/derive 1,382, base 457 and doctests 11 pass (one ignored in each); primitives 305, Native AMX 32 on the default stack, ABI 167 and artifact admission 15 pass. Consumer suites pass 1,651 cases plus one status-wire integration and two Nexus commitment cases. Fifteen production libraries, shipping CLI/Kagami, seven library-test targets, CLI/Kagami test binaries and the Nexus/streaming harness compile. Strict scoped Clippy passes; merged-root source checks pass 87 tests plus 91 subtests. | The 166-path reconstruction stage is integrated. Its initial model run retained 3,650 passes, one fixture failure and six ignored cases; the corrected complete model result is recorded in the public-frame row below. All 14 corrected Torii cases pass; the earlier nine-pass/five-failure fixture run is retained. Structural rANS, four Parliament anchors and 236 source-size findings remain open. See the [reconstruction checkpoint](specs/first_release_architecture_redesign.md#payload-reconstruction-checkpoint-2026-09-08). |
| Public model frame ownership | IdBox and BlockSignature preserve 24 captured frames; seven redundant block-adapter markers are removed. Direct fallible IdBox decoding rejects malformed tags. Before the subsequent manual declaration stage, the complete default/HTTP model library passed 3,655 tests, zero failures and six existing ignores on 19,306 unchanged inputs without a stack override. Its 89 focused cases, two signature allocation tests and three adapter compile-fail guards also passed. | That full-library result belongs to the preceding source seal. The current public-target qualification is recorded below; complete workspace and release qualification remain open. See the [frame contract](crates/iroha_data_model/tests/fixtures/public_frame_owner_identities.md). |
| Manual model frame declarations | Sixteen additional scalar, protocol and proof owners preserve 120 captured root/Option/Vec frames. One public integration target now owns their tests and all prior identifier/signature assertions. All seven tests pass, preserving 144 complete frames plus checked malformed-input and exact proof-list boundary controls, with 19,313 unchanged inputs and no stack override. The 29 source/fixture paths are integrated; codec guard passes and Cargo.lock is unchanged. | The subsequent query/time qualification is recorded below. Remaining generated/manual declarations, active identity cutover, model extraction and complete workspace/release qualification remain open. Source budgets still report 236 findings; KAGEMUSHA V1 remains oversized at 6,258 lines, with no widened exceptions. |
| Query, time and event frames | The latest event stage adds 196 declarations and 103 captured frames. All 384 focused tests pass on 19,341 unchanged inputs without a stack override: 21 public tests preserve 525 immutable frames, 329 model tests include all 32 Native AMX regressions, and 34 query/SM/allocation cases pass. Four event slice decoders preserve the complete owner envelope, caller layout and typed budget errors. Native AMX tests have a topic-owned module; every moved assertion is retained. | Formatting and codec checks pass. Source-size findings fall to 235 with 173 unchanged exceptions. Strict Clippy fails at the model library with 225 diagnostics; the public test target is not lint-qualified. Complete owner coverage, active identity cutover, physical model moves and workspace/release qualification remain open. See the [frame contract](crates/iroha_data_model/tests/fixtures/public_frame_owner_identities.md#event-owners-and-stream-reconstruction). |
| Version diagnostic candidate (V) | The frozen candidate preserves 26 captured frames and repairs RawVersioned slice reconstruction through its canonical owner. It passes 27 test executions (16 default version/derive, including one doctest; 11 minimal version), strict default/minimal all-target version Clippy, all 16 dependency boundaries, five unchanged dependency budgets and the codec guard on 19,343 identical inputs without a stack override. | This qualification belongs to V's isolated source seal; concurrent root changes need their own checks. Active identity cutover, model extraction and full workspace/release qualification remain open. See the [identity contract](specs/norito_schema_identity.md). |
| Transaction owner candidate (W) | Twenty-five transaction, executable and rejection owners have retained-capture declarations and three scoped identity tests in five paths; existing item bodies and signed transaction fixtures are preserved. The model build passes in 258.514 seconds. Its four selected runtimes pass 499 tests, with one unchanged operator-only KAT generator ignored, on 19,344 identical inputs without a stack override; codec checking passes. | Candidate scope only. Strict Clippy fails with 225 library errors matching the earlier source diagnostics; the public test target lint is unreached. Source-size checking still fails with the same 235 findings and 173 exceptions. Full workspace/release qualification, active identity cutover and model extraction remain open. |
| Foundation identity closure | Captured declarations now cover seven address, three crypto scalar, three manual model and 75 protocol owners in this follow-up. The complete default/HTTP model library passes 3,644 tests on the default stack. The subsequent dependency/module batch passes 211 focused model/ABI/schema tests; all 114 moved tests remain, and governance/AXT/private-settlement roots meet the 5,000-line limit. Model tests lose 34 resolved packages; 16 dependency boundaries, 45 guard tests and all five exact manifest-budget scopes pass. | The complete-library and subsequent focused runs have separate frozen source/artifact records. The final candidate has 237 other source-size findings; strict Clippy, active codec cutover, physical extraction and complete workspace/native/release qualification remain open. See the [scoped follow-up](specs/first_release_architecture_redesign.md#foundation-identity-follow-up-2026-09-07). |
| Bare decoding ownership | Bare Decode, canonical fields, containers and query registries now require payload serialization; actual framed consumers declare frame serialization explicitly. Option uses the shared bounded canonical decoder, and tuple/Result slice decoders reject unread child bytes in every profile. The frozen default/HTTP model and ABI test run passes 4,110 tests (13 ignored), including all 32 Native AMX cases; all 19,275 source inputs remain unchanged. The preceding codec/derive/primitives run passes 1,684 tests (three ignored). | The follow-up codec/schema/primitive suites pass 1,726 tests (three ignored), and 52 focused model tests pass on the default stack. Result nesting now enforces and restores depth limits; multisig owners decode through their validating constructors. Library Clippy passes. The earlier minimal-model JSON failure is retained; its mandatory ownership correction and renewed qualification are recorded below. The isolated candidate Core/Torii production graph and Kagami binary compile after reconciling the production Encode import; seven Core and two Kagami warnings remain. Runtime qualification, atomic identity cutover, model extraction and release qualification remain pending. |
| Mandatory model JSON | The isolated candidate provides protocol JSON through six required owners; the aggregate `json` feature and active caller selections are removed. Implementation-only validation modules preserve wire owners, layouts and fixtures. Before decomposition, 3,648 model library tests and existing integrations pass; afterward, 419 focused model tests pass (three ignored), followed by 217 derive/integration/ABI tests with zero failures or ignored cases. All 89 Python controls and 16 resolved dependency boundaries pass. Only the existing model→mv and model→norito_derive edges become required; the exact dependency budget accounts for that closure. Cargo.lock and captured/generated fixture bytes remain unchanged. | The focused model and derive/ABI runs retain all 19,290 candidate inputs unchanged. Final model checking without default features and all three source guards pass on the separately sealed final candidate; one existing governance dead-code warning remains. All 29 standalone restricted-visibility tests and strict derive-library Clippy pass; integrated source paths match the qualified candidate. The earlier failed minimal check and derive visibility failures remain retained. Atomic identity cutover, physical extraction, memory, native and full release qualification remain open. |
| Streaming wire identities | The integrated declaration and correction paths preserve all 84 identities and 396 captured complete/bare frames. Final combined tests pass 1,380 cases (Norito 1,309, derive 57, strict JSON 14; one ignored), base library passes 456 (one ignored), and six focused structural schema integration tests pass. Strict Norito tests and isolated derive-library Clippy pass on 19,297 unchanged inputs. Enum directions now select the same active hash; removing unused private Debug derives adds no dependency features and preserves all parser diagnostics. | The earlier structural compile failure is retained. The integrated reconstruction stage now runs both streaming structural guards successfully, but its full structural library selection still fails one signed rANS checksum (467 passed, one failed, one ignored). The earlier group_05 result remains 109 passed and ten failed. Atomic cutover, physical moves, measured memory reduction and full release qualification remain open. See the [streaming checkpoint](specs/first_release_architecture_redesign.md#streaming-wire-identity-closure-2026-09-08). |
| JSON object keys | Explicit scalar key traits own key text and decoding; maps quote and escape numeric keys correctly. All 1,258 Norito tests pass (one generator ignored), three compile-fail cases and strict library Clippy pass. Another 506 JSON tests pass across model/crypto/primitives, with 1,041 selected input hashes matching before/after. Six model contract tests include observed normalization allocations. Norito `base-codec` checks without default features. | The post-migration full model and selected Core results are recorded above; remaining Core/storage/SDK consumer qualification is open. Bare no-default-feature Norito checking still fails in unconditional JSON helpers. Physical model extraction and workspace/native release qualification remain open. |
| Account fee quoting | One typed async account operation handles direct signatures and complete multisig witnesses; old raw and witness-specific public quote methods are removed. Focused authorization, bounds, response-binding, blocking and fee-error suites pass. Every CLI and integration test target compiles. | Unified SDK errors, canonical local builders, remaining async capability migration and four-validator runtime qualification remain open. |
| CLI ownership | `iroha app sorafs toolkit compile` owns compiler behavior; archive packing owns its CLI module. Toolkit 10 tests and governance audit 7 tests pass. | Remaining standalone storage CLI/runtime extraction and all downstream release checks remain open. |
| Kotlin/JVM | Local `connect_norito_bridge` and `kotlin-fixture-gen` dev builds pass. The latest complete `core-jvm` checkpoint, before the CUDA JNI migration, passes **1,295 XML-counted tests, 0 failures, 0 errors, 0 skips**, with the explicit rebuilt JNI directory and fixture generator. Reflection guard passes. | This is local host execution, not Android, physical-device, signed native-artifact, or release-provenance qualification. |
| JVM transport | Kotlin owns OkHttp HTTP/SSE and Netty WebSockets, with explicit resource lifetimes, one-shot upgrades, bounded messages/queues and cancellation during handshake/read/reconnect. All 72 focused tests pass and are included in the full JVM run. | Java transport implementation retirement and Android/native packaging remain open. |
| JVM CUDA ownership | One Kotlin-owned batch API replaces the Java implementation and facade. Eight Java consumer tests pass; five CPU-reference hardware tests compile under JDK 8. All seven compiled native declarations match the canonical Rust source exports. | Rebuilt host JNI execution is pending the concurrent Core helper repair. CUDA hardware execution, Android and release provenance remain unverified. |
| Kotlin JNI boundary | 77 Kotlin declarations match 77 SDK source exports. Nine unowned JNI exports are removed with the 132 C ABI function bodies preserved. All 41 new core Java consumer cases compile; 27 managed checks pass. Guard/launcher/reflection tests pass 68 cases; Kagami/release script checks pass 34 cases and 5 subtests. | Fifteen new core native cases and one Android manager native case remain unexecuted. The existing binary is stale, and 55 Android duplicates still await consumer retirement. Export inspection does not qualify native argument types or execution. |
| JVM signing ownership | The Kotlin client uses opaque `RequestSigner` injection; 5 new Java consumer tests cover it. Private-key auth surface, all production consumers and 55 test calls were migrated. | Remaining capability gaps, Java implementation/publication retirement and native packaging are tracked in [the consolidation inventory](specs/jvm_consolidation_inventory.md). |
| JVM attestation command | Kotlin `tools` passes 25 tests (22 Java consumers and 3 bounded-reader tests). The repository launcher verifies a shared mock fixture, emits matching stdout/file JSON, and rejects a wrong challenge. Old Java command/tests are removed; pure verification moved byte-identically to `core-jvm`. | Requalified Android host suites pass 57 client + 2 wallet tests. Physical StrongBox execution and release packaging remain unverified. |
| Android JVM consumers | Debug unit suites pass 78 client and 2 wallet tests, zero failures/errors/skips, including 21 migrated Java key-manager/context/ZK cases. Four earlier Java attestation tests require explicit challenge, roots, revocation policy and evaluation time. | Host-native manager execution has its own required-artifact task. Physical StrongBox/radio execution, rebuilt Android JNI, release packaging and provenance remain unverified. |
| JVM metadata values | Five Java consumer tests and 41 existing JSON/transaction/codec tests pass. `JsonValue` factories are directly Java-callable; transaction metadata is copied, immutable and rejects Java null entries. Canonical wire roundtrips pass. | The complete JVM suite has not rerun after this API change. An attempted wider selection failed at an existing test's required-native-bridge assertion; native qualification remains open. |
| Immutable Nexus values | 27 focused Nexus tests pass, including 5 new Java consumer tests, and are included in the full JVM run. Owned bytes/collections and deep receipt JSON, payload-derived hashes, canonical Ed25519 and removal of public data-class copying are covered. | Remaining JVM transport, CLI/native and release-platform gaps stay open. |
| Verified Nearby messages | Kotlin session owns typed IPM1 seal/open, profile binding and temporary-byte lifecycle. Seven adversarial Kotlin tests and four Java consumer tests pass in the full JVM suite; both duplicate Java Nearby facades are removed. | Host transport/shape checks do not qualify physical Android radio/device behavior or offline-money proofs. |
| CI and evidence routing | Binary-free and binary-consuming Rust lanes are classified before builds; focused router tests pass. All 70 combined archive/release-contract tests pass. Archive reconstruction and 300-line current-view limits are now in the PR classification gate; 27 router tests and configured workflow lint pass. | The selected jobs and full release aggregation still need execution on CI infrastructure and the final candidate. |

Musubi service extraction now owns publication, the durable clock and replay
journal. Its focused suite passes 74 of 76 tests. Direct moved records retain
their captured frame identities; the two remaining golden tests expose nine
generic `Vec`, `Option`, `HashOf` and `SignatureOf` roots whose active identity
still contains the physical Rust path. Fixing those headers requires the
documented atomic Norito typed-identity cutover; no alternate hashes, path
registry or rewritten goldens are accepted.


These are scoped working-tree checkpoints. They do not imply that workspace
build/tests, strict all-target Clippy, all SDK platforms, or release workflows
have passed together against one immutable source tree.

## Build and architecture qualification

SoraFS goal execution is tracked in the [V1 implementation goals](specs/sorafs/v1_implementation_goals.md).
The post-reboot manifest library now passes **908 tests**, zero failures or ignored
tests, including canonical identities/signatures under alternate caller layouts.
The new local result and binary/source hashes are retained in ignored
`target/evidence/sorafs-v1/manifest-reference-04-result.json`.
The retention-request model selection also passes three tests. Provider/rollout
source contracts pass 410 checks, with two unfinished-source closure failures.
The review corrects canonical manifest/deal/audit/replication identities, retention
request digests, pin-accounting keys, and node billing/reputation/governance
checkpoint and publication framing. The earlier full Node suite reports
**1,378 passed, 48 failed and two Kubo cases ignored**. The next rebuilt security
selection now reports **74 passed and four failed**, with zero ignored; all 48
original failures still pass. The remaining failures identify two unsafe fixture
file modes, a fixture assumption about intentionally hedged encryption randomness,
and insufficient cumulative quarantine decode budget at its exact byte limit.
Bounded corrections and the next full-suite execution remain pending.
The rebuilt Core SoraFS selection now passes **406 tests**, zero failures or
ignored, in 58.198 seconds with unchanged scoped source/binary hashes. It covers
all 42 earlier failures, the reputation policy cutover fence, exact permission
tokens under zero allocation, and authenticated snapshot timing. The combined
build still fails on Torii test include paths; this pass qualifies only the
captured Core selection. Node focused/full validation remains in progress.
The host reboot cleared previous `/tmp` SoraFS logs and interrupted native
captures; those older observations cannot serve as retained current evidence.
Matched daemon/harness four-validator
execution, full workspace/SDK validation, source/bootstrap seals, genuine HSM
custody and all deployment evidence remain open. See the
[current closure checkpoint](specs/sorafs/v1_closure_ledger.md#2026-09-07-post-reboot-checkpoint).

The active acceptance policy uses enforced dependency ownership, the existing
5,000-line production and 3,000-line test-file limits, substantive duplication
removal, and measured compiler memory. There is no global Rust LOC objective.
Historical LOC measurements remain historical facts in the archive.

| Profile | Evidence | Qualification |
| --- | --- | --- |
| Frozen data-model baseline, corrected profiler | 338 cold units, 31 compiler probes, stable source/toolchain seals; 148 profiler tests pass. Model peak: 13,275,365,376 bytes. | Qualified baseline; required candidate ceiling is 9,956,524,032 bytes (25% lower). Candidate and remaining surface qualification are pending. |
| Frozen SDK baseline, corrected profiler | 498 cold units and 46 probes with stable seals. SDK peak: 1,576,681,472 bytes; selected model peak: 13,295,861,760 bytes. | Qualified baseline; remaining test/release surfaces and comparable candidates are pending. |
| Frozen Core test baseline, corrected profiler | Restored compiler probes let Core compile (24,238,915,584-byte dev/library peak); 533 cold units complete before grouped governance fixtures fail on eight removed ballot/lock/referendum API references. Source/toolchain seals remain stable. | Invalid baseline (`returncode=101`); migrate fixtures and requalify a complete test build. The earlier Halo2 instrumentation failure remains historical evidence. |
| Frozen Torii test baseline, corrected profiler | 608 cold units and 50 probes; Torii dev/library peak 10,369,712,128 bytes. Source/toolchain seals are stable, but the library-test target fails on 82 stale caller/fixture compile errors. | Invalid baseline (`returncode=101`); complete the fixture migration and requalify the test build. |
| Frozen daemon release baseline, corrected profiler | 606 cold units, 54 probes, stable seals; model/Core/Torii peaks are 32,699,990,016 / 38,454,771,712 / 18,646,286,336 bytes. The frozen daemon manifest lacks the shared Torii dependency already added in the working tree. | Invalid baseline (`returncode=101`). All three completed peaks exceed the retained 13-GiB release ceiling; no release budget is qualified. |
| Frozen JNI bridge release baseline, corrected profiler | 562 cold units, 52 probes, stable seals; bridge/Core/model peaks are 1,195,311,104 / 35,761,225,728 / 32,736,935,936 bytes. The complete release build succeeds. | Qualified measurement baseline; Core/model exceed the retained 13-GiB ceiling. This does not qualify current-source native execution. |
| Frozen JavaScript native release baseline, corrected profiler | 595 cold units, 54 probes, stable seals; host/Core/model peaks are 1,412,186,112 / 33,764,720,640 / 32,704,380,928 bytes. The complete release build succeeds. | Qualified measurement baseline; Core/model exceed the 13-GiB ceiling. Current-source native execution remains unverified. |
| Measured memory budgets | Four baseline report/input/measurement identities now define concrete model byte limits and retained-unit caps. All 43 comparator tests pass; presenting the four real baselines as candidates correctly fails. | Complete suite checking rejects pending/missing surfaces and mixed candidate revisions. Pinned-runner CI execution and real candidate reduction remain unqualified. |
| Remaining surfaces | Core/Torii test units, repaired daemon release, lower-memory macOS and Linux cgroup runs, then comparable candidate profiles. | Qualification remains pending. |

Exact profile digests and methodology are in the
[architecture record](specs/first_release_architecture_redesign.md) and
[profiler documentation](docs/profile_build.md). Dependency-boundary
checks pass for all 16 selected configurations. The qualified streaming
candidate reports 235 remaining source-size findings across the measured
languages after the Native AMX test split; 173 existing exceptions remain after
removing the obsolete streaming exception. Production/test limits remain 5,000/3,000 lines. The test guard correctly
applies the 3,000-line limit to pytests, Swift Tests and split Rust test directories;
51 guard tests pass, including a real Git-ignore regression that keeps nested
security tests visible to source selection and budgets. Codec-owner extraction
lowers two existing file ceilings; 77 applied budget/provider checks pass.
Corridor and provider extraction preserves all 5,877
collected test IDs; the latest 54 applied checks pass. The main test module is
33,974 lines and its existing exception is ratcheted downward. No budget,
source seal, artifact identity, or 13 GiB release limit is waived by local passes.

## Release blockers

- **Architecture:** finish model and service ownership, storage/Musubi extraction,
  canonical SDK API and transport, Core state decomposition and Torii capability
  routing. Close the dependency and source-file checks without new exceptions.
- **Candidate validation:** settle the source and approved manifest/lock changes;
  run the workspace, strict lint/format, canonical codec/ABI, generated-artifact,
  native/SDK and release-feature matrices. Regenerate signed OpenAPI/artifact
  provenance from the actual clean candidate. Mutable-tree checks are development
  evidence only; unresolved and timed-out commands remain non-passes.
- **Consensus and topology:** qualify serialized Sumeragi v2 wire revision 4 with
  mandatory signed RS16 availability, exact committees, restart/replay and
  authenticated loss/hold/heal. Same-candidate formal proofs, production trace
  mapping, multilane/fault matrices, long soaks, additive SNS cold replay and
  physical dataspace isolation remain required.
- **Crypto and privacy:** independent arithmetic, soundness, side-channel and
  custody review; real CPU/Metal/CUDA conformance; unresolved FASTPQ verification,
  ZK-ACE six-lane/qROM qualification, native-STARK degree, BFV-RNS, MKHE and Figure 9 gates remain open.
  Unsupported proof paths stay fail closed.
- **KAGEMUSHA and mobile:** recursive aggregate proof, durable hardware coordinator,
  mint/redemption, long-history and adversarial recovery, same-source native
  artifacts and governed physical-device profiles remain unqualified. Host JNI,
  software models and ordinary secure-key signing do not establish offline money.
- **Deployment:** SoraFS L1/L2 and dual Governance DAG evidence, SoraNet transport
  and privileged Linux helper qualification, SCCP audited production proof/live
  corridors, Inrou real Linux/AArch64/KVM guests, and authorized Taira onboarding
  and runtime evidence remain open. Current public endpoint health is not asserted.

## Non-negotiable release contracts

This is the first release: one canonical API, wire/model owner and implementation
per capability; no legacy aliases, migration shims or compatibility modes.
`AccountId` is domainless; aliases and routing context are separate. IVM has one
V1 ABI and identical observable behavior across scalar and accelerated hardware.

Consensus uses the mandatory revision-4 Prepare/Commit protocol and signed RS16
layout. Four-validator integration networks are the minimum representative
corridor; exact `3f + 1` committees require `2f + 1` validator votes. Observers,
empty-block production and retired global-RBC bypasses do not establish liveness.

Ordinary node/client/consensus signing and custody are provider-neutral; software
custody is valid. Runtime secret handling, authentication, rotation/revocation,
durable recovery and independent review remain requirements. Optional KAGEMUSHA
offline spending separately requires governed non-forking hardware with no
software fallback.
