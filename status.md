# Status

Last updated: 2026-09-08.

Iroha 3 is under active first-release implementation and qualification. The
release is not qualified. Signed source checkpoints do not establish runtime readiness.
This page records current findings and bounded local evidence; it is not a
substitute for the release gates in [the roadmap](roadmap.md).

The previous dirty working copies are preserved in the
[dated historical archive](docs/history/2026-09-06/index.md). Its manifest binds
every source occurrence and reconstructs the original bytes. Historical pass
counts, plans, dates, and former policies do not attest the current candidate.
[Repository ownership](docs/repository_map.md) and the
[architecture record](specs/first_release_architecture_redesign.md) provide detail.

## Current implementation and local evidence

The merge resolves all 104 conflicted paths around the validated seven-field
Native AMX participant control, shared Torii configuration DTO, Kotlin-owned
Java API and explicit Norito identities. FASTPQ retains the exact-integer and
quotient/degree checks alongside the six-lane digest implementation; its current
raw transcript was regenerated and replayed successfully. Focused SDK, OpenAPI,
Norito and source-closure checks pass; the Rust workspace and Core/network
matrix have not been rerun. Swift execution still requires the ABI-23 bridge.
Receipt replay now disables bytecode writes into sealed inputs; the remaining
macOS Python framework-copy size/hash assertion fails in its isolated test.
The three OpenAPI schema copies agree, but release manifest provenance and
artifact digests require the clean-candidate replay before release. Those checks
do not qualify a release.

Kaigi's final scalar/identity/retained-participation model passes 33 tests, and
the authorization/usage circuit library passes 20 tests with real proofs and
every public-row mutation. Retired circuit/seed/artifact-hint interfaces are
removed. A fresh strict account-decoder run passes 42 cases across all ten
Norito layouts. Kotlin/Java pass 40 scoped tests including the shared 16-positive,
seven-negative full-controller fixture. Earlier Python wire results predate its
new native identity adapter; installed-wheel verification is pending. Mandatory host authorization,
leave/rejoin, usage and storage reservations await the shared Core verification
build; final SDK artifacts, relay transport and deployment remain unqualified.
See the [current privacy evidence](specs/privacy_first_release_closure.md).

Production multilane work has [six implementation milestones under an active goal](specs/sumeragi_v2_multilane_completion_goals.md).
No milestone or release gate is closed. The current source adds authenticated
terminal replay, bounded atomic cache retirement, a real second autonomous
application fixture and mandatory move-only authority for shipping Queue release.
G-UNIT registers 531 tests; production registers 881 across 43 modules. Those
runtime changes await the coordinated Core build; all original affected fixture
assertions remain. The combined source manifest covers 17 Rust files and five
model/configuration files under ignored `dist/multilane-validation-20260907/`.

A fresh formal audit reproduced direct FIFO release with active Kura custody,
then lane Commit and WSV application, in the former model and shared predicate.
The repaired relation acquires exact authenticated replica custody at Kura
activation and excludes Commit/application from every release disposition.
Exhaustive TLC passes all 20 configured invariants over 280,818 distinct states;
all 25 exact mutation controls produce their named counterexamples, including
the new 4/19/4-action failures. Four explicit replica/application and release
paths also pass. Current-model Apalache, Verus and Rust execution remain open.
The running 18-step check uses the earlier frozen model and cannot qualify this
repair. These model results do not establish live-network exploitability.

Fresh source evidence passes 69 terminal controls, 21 merge-cache semantic
negatives with copied positives, all eight cache owners, and the complete
QueuePlan contract with 14 release-authority negatives. The terminal baseline
also passes after the custody correction. The broader source diagnostic still
fails with 71 errors; ingress, lifecycle completion, worker ownership and
finalization contracts remain under review. Four inputs changed during that
run, so it is development evidence. Four retired-codec guards pass.

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
| Taira reset and startup | Release68 built and transferred four signed Linux binaries, passed native smoke checks, all four validator configurations and all five real host preflights. Its readiness-driven uploader completed the daemon upload. The next upload exposed a receipt namespace mismatch: canonical underscore-bearing artifact and action names were rejected by the shared validator. The correction admits those safe names and adds exhaustive action/role and unsafe-path regressions. Bounded subprocess rejection diagnostics now retain child stderr and exit status after stdin closure. | Release68 rolled back before any validator started; all five services are inactive and its native records remain retained. All 23 corrected native regressions pass in 176 seconds with unchanged source, and all 25 release-tool Python tests pass. The next signed build and public rollout are pending. Application connectivity, signed canaries, epoch-boundary roster publication, storage growth and reboot qualification remain open. |
| KAGEMUSHA release work | The default-stack production-services restart and inline-carrier size regressions pass after the retained-carrier allocation fix. The Linux build adapter passes ten tests and a real crypto-symbol link through the Cargo-Zig compiler path. | Full Linux release/runtime, remaining startup recovery, aggregate proofs and device resource limits, governed physical profiles and independent review remain unqualified. See [current readiness](specs/kagemusha_v1_production_readiness.md). |
| Privacy V1 admission and field carriers | Canonical 48-byte digest and 32-byte Fp4 carriers; complete signed synthetic qualification passes bounded decoding. Rust capability FFI passes 2 tests; corrected Exact12 fixtures pass JS 23, Python 94, Kotlin 16 and C# 13. SDK admission source checks pass 86 regressions; model privacy selection passes 127 tests and the final intent KAT is independently reproduced. Shared six-lane framing passes 39 primitive tests and the ACE selection passes 26; AXT binding passes 76 FASTPQ and 7 Core regressions. | Core native selection passes all 63 tests; the corrected complete proof crate passes 1,299 with zero failures and 13 ignored. Current SDK origin/network checks pass JS 37, Python 103, Kotlin 63, C# 17 and Swift source-only 35. Apple duplicate archive ownership is corrected and three target link controls pass; the previous full XCFramework build was lost during host restart and has no final artifact; complete GPU proof dispatch, AXT authoritative state/execution binding, independent review and four-validator qualification remain open in the [closure ledger](specs/privacy_first_release_closure.md). Synthetic signatures are validator tests only. |
| SCCP TON scoped audit | [Validated fixes and evidence](docs/source/sccp_ton_security_audit_2026_09.md): ordinary transfer funding, bounded replay work, native TL-B parsing, exact checkpoint identity, complete breaker readbacks, builder Git/verifier/attribute isolation, and canonical wire identifiers across Rust/SDKs, circuits and contracts. Earlier focused Rust/Core/model/production compile checks pass. Fresh validation: 459 Python tests with zero skips, 45 TON contract tests, authenticated StateInit write/check, and pinned EVM/TRON compiler plus EVM runtime smoke pass. Rust validator suites pass 22/27 tests, and the compiled Rust wire fixture passes. Policy/proof negatives have positive controls and precise rejection checks. All 8 R1CS identities are freshly measured, with a verified source closure and no pending profiles. | Full Core/workspace and Torii runtime tests are unclaimed. Production keys/proofs, trusted release signatures and authenticated deployment readbacks remain separate release artifacts. |
| Rust SDK dependency separation | Relay accounting moved to `soranet_incentives`; SoraNet policies and shared defaults have one `iroha_service_model` owner. Archive construction, filesystem persistence, orchestrated fetch and DA workflows now live in `iroha_storage_client`. The shipping SDK graph has 28 local packages, 87 external packages and 268 required edges; its boundary checks pass without node, CAR or orchestrator dependencies. Storage-client tests pass 41 cases. Protocol capability probes are isolated per context and shared by clones. Immutable account/operator transaction contexts, signed multisig submission and the explicit blocking runtime pass eight focused tests. CLI-owned queue/witness paths preserve source-relative resolution and scoped authority binding; 15 SDK and 13 CLI config/authentication tests pass. All development binaries and integration-library tests compile; the three development-bin suites pass 40 tests. AccountTransactionDraft and AccountClient::prepare_transaction/sign_transaction now replace all generic helpers and the quote-and-sign composite, with crate-level typed errors; ten focused tests preserve exact bytes, defaults, attachments and context isolation. SDK examples and all CLI/Musubi/Izanami/test-network and integration test targets compile. Four specialized SoraFS wrappers are removed; five focused tests preserve exact instructions, moderation TTL and invariant checks. | The base Client still has mutable fields; remaining specialized preparation APIs, non-transaction error shapes, synchronous capabilities, broader operator families and complete consumer migration remain unfinished. |
| Configuration and status HTTP contracts | Shared configuration DTOs have 22 wire tests, 3 node conversion tests and 14 Core runtime tests. Shared status preserves 32 captured named DTO frames/hashes/JSON; 93 telemetry tests pass. Core, SDK/CLI, test-network, schema generation, Mochi and grouped consumers compile. | Named records are qualified by focused fixtures; arbitrary generic-envelope schema identity is not yet cut over or fully qualified. |
| Core integration fixtures | The `core_api` harness compiles after all 21 identified fixture errors were repaired through canonical APIs. | The new four-validator configuration startup/restart/readback/isolation scenario has not run against rebuilt binaries. Compilation is not runtime evidence. |
| Torii evidence API | Grouped telemetry harness compiles. All 3 evidence runtime tests pass, including genuine four-validator BLS proof-of-possession/signature material and tamper rejection. | These tests do not qualify the full node consensus/release corridor. |
| Atomic private settlement | Retained Core functional validation passed 41 tests, SDK finality/timeout identity passed 22, and bounded BLS decode reuse passed 63. The selected Torii follow-up passed 152 tests with one unfinished stored account-query adapter failure; all fifteen patch regressions passed. Both optimized binaries rebuilt and all 29 ordinary integration preflights passed. The fresh sixteen-process diagnostic failed at activation admission with one of two required durable attestations. Telemetry now caps semantic block/transaction classification at one captured applied-State prefix; its first focused build failed during a concurrent advice-helper API change, and a retry awaits the active merge. | No completed private settlement yet. Five concurrent source changes predate the telemetry patch; both network executable bytes are retained unchanged, and the run records source drift. The new catch-up regression is applied; its production retry is prepared pending a negative control. Earlier failures remain retained. Current-source proof qualification, ten positive N=3 runs, fault/leakage/performance campaigns, independent audit and release evidence remain open. See [the protocol](specs/private_settlement.md). |
| Norito schema preparation | Canonical identity kernel, derive support, primitive/crypto declarations and strict UI tests pass. All 19 planned base-model declarations preserve 189 captured frames, signatures, JSON, storage keys and schema output. All 25 generated event sets preserve 225 captured frames and 75 JSON values; 12 enclosing event enums preserve their identities. Another 17 Musubi generated types preserve 196 frames across 49 values; 16 governance hash wrappers preserve 256 frames and 64 JSON values. Another 127 generated queries preserve 684 frames across 171 payloads, and 62 privacy/spentness carriers preserve 992 frames across 248 payloads. The complete model group_02 harness passes 178 tests (two fixture writers ignored), including all 28 query tests. The derive suites pass 48 tests, strict Clippy and the executable EventSet documentation example. | Active codecs remain unchanged. Complete declaration coverage, atomic cutover and model moves remain pending; see [the identity contract](specs/norito_schema_identity.md). |
| Norito migration capture | Eight probe-generator, 17 driver and 32 syntax/context-graph tests pass. Controlled crypto/model captures pass 206/6,112 probes. Another 108 crypto declarations pass nine module suites and seven existing wire/identity golden tests. The reviewed 1,771-declaration model batch preserves every pre-capture identity and its 110 owner-scoped fixture tests pass on the sealed post-declaration harness. Native AMX participant finality now uses a finite `NativeAmxParticipantSettlement` wire record; canonical Rust, Python, JavaScript, Kotlin and Swift fixtures use its domain-separated typed hash and reject the removed recursive field. Norito encoding also returns a typed nesting error before recursive user values can exhaust the native stack. | Generic, local, remaining generated families and other feature selections remain incomplete. The complete local model-library result is recorded below; other feature selections and release qualification remain open. |
| Instruction identity and registry | All 292 generated instruction declarations require captured identities. The original capture retains 357 values and 1,428 frames across 322 types. Five Kaigi private cases now correctly reject retired hash-shaped scalars; their public cases retain exact frames. New canonical private captures preserve root/vector/option/map coverage. Registry fixture updates account for 14 game and three NFT-market additions, with no removed or reassigned wire IDs. | All 409 focused tests and the complete 3,576-test model suite pass; six unrelated cases remain ignored. Other features and release qualification remain open. |
| Generic query identities | Five generic owners, four concrete query records and two typed-hash markers declare their captured identities. Immutable fixtures preserve 96 default and 116 ids-projection frames, marker composition and private decoder budgets. | Default queries pass 202 tests and ids-projection queries pass 203, with zero failures/ignored tests and all 5,109 selected inputs unchanged in each run. Atomic codec cutover, remaining declaration coverage and physical model moves remain open. |
| Generic model identities | Ten declarations preserve 68 captured frames and five FHE signing preimages. InstructionBox retains its wire-pair root projection and nominal generic argument; the borrowed FHE helper exposes encoding only. All five model and four query identity tests pass on the rebuilt artifact. | All 5,613 selected inputs remain unchanged; other feature selections, active codec cutover and physical model extraction remain open. |
| Concrete model wire identities | Seven owned records and two encoding-only adapters preserve 52 captured frames and three projections. DataEvent and DataEventFilter assign explicit tags, reserving disabled Governance variants. The frozen payload-boundary candidate passes 3,599 default/HTTP model tests and all nine governance-disabled normal-consumer tests, with all 19,170 recorded inputs unchanged. | Active identity cutover, model extraction and complete release qualification remain open. |
| AMX wire and recovery | All 35 merged-model AMX/framing/roster regressions pass on the default stack, including schema/codec/hash/drop for the maximum 4,096-source settlement and rejection of the removed recursive layout. The frozen payload-boundary default/HTTP model artifact passes all 3,599 library tests, with zero failures and six fixture generators ignored, including the maximum-size and removed-layout regressions without a stack override; all 19,170 recorded inputs remain unchanged. All ten retirement regressions pass after binding four-validator fixtures and lifecycle projections to their actual certificates. The lane-fixture and source-identity repairs pass 396 selected Core and seven Torii tests. The replica Queue witness wrapper and corrected recovery fixtures pass all 10 further Core regressions; 19 source tests and 35 subtests authenticate 28 actions and 29 runtime bindings, with 5,104 selected inputs unchanged. The mandatory shared reducer harness passes all 197 tests. The full pinned Verus harness verifies 221 project obligations with zero errors and proof escapes disabled. | Formal execution retains 40 unchanged selected inputs; it is not a complete release seal. Strict lint, workspace, four-validator and native SDK qualification remain open. See the [scoped implementation evidence](specs/first_release_architecture_redesign.md). |
| Storage identities and layout | Before the JSON key migration, the complete revised model library passed 3,446 tests on the default stack, with six fixture generators ignored and all 1,000 recorded inputs unchanged. Seven common-module tests preserve 432 storage frames. Three dedicated metadata tests prove scratch allocations are independent of payload size and preserve exact wire bytes. Block-signature wire/allocation checks pass. | Strict model Clippy reports 129 diagnostics; its dependency-inclusive attempt also fails on six dependency errors. Physical model extraction and release qualification remain open. |
| Nested encoding work | Object-safe SerializePayload owns bare serialization and sizing; typed frames retain NoritoSerialize. Bare containers and borrowed adapters use the payload contract. All 1,663 codec/derive/primitives tests pass (three ignored), and strict library Clippy passes. Exact bytes, checked lengths, writer failures and payload-only compile-fail cases are covered. | The frozen test inputs precede downstream bound/import corrections and eight blank-line cleanups. Complete target checks, source-size budgets, active schema cutover and pinned-runner qualification remain open; see the architecture record. |
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
The post-reboot manifest library passes **896 tests**, zero failures or ignored
tests, including canonical identities/signatures under alternate caller layouts.
The new local result and binary/source hashes are retained in ignored
`target/evidence/sorafs-v1/manifest-canonical-identity-result.json`.
The retention-request model selection also passes three tests. Provider/rollout
source contracts pass 410 checks, with two unfinished-source closure failures.
The review corrects canonical manifest/deal/audit/replication identities, retention
request digests, pin-accounting keys, and node billing/reputation/governance
checkpoint and publication framing. The fresh node retry passes **19 of 19**
regressions after correcting the two-slot encoder and fixtures; compression is
rejected before allocation. The full node suite reports **1,378 passed, 48 failed
and two Kubo cases ignored**; retained failure diagnostics are under review.
Canonical Core state/nonce keys and SDK reference frame admission are being
completed before the shared Core capture; broader qualification remains pending.
The host reboot cleared previous `/tmp` SoraFS logs and interrupted native
captures; those older observations cannot serve as retained current evidence.
The former Core run exposed 113 failures, and its fixture/security corrections
still require the coordinated rebuild. Matched daemon/harness four-validator
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
[profiler documentation](docs/profile_build.md). Remaining dependency-boundary
violations and 240 source-size findings remain visible. The test guard correctly
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
