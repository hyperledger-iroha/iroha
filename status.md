# Status

Last updated: 2026-09-06.

Iroha 3 is under active first-release implementation and qualification. The
working tree is not a signed release candidate, and the release is not qualified.
This page records current findings and bounded local evidence; it is not a
substitute for the release gates in [the roadmap](roadmap.md).

The previous dirty working copies are preserved in the
[dated historical archive](docs/history/2026-09-06/index.md). Its manifest binds
every source occurrence and reconstructs the original bytes. Historical pass
counts, plans, dates, and former policies do not attest the current candidate.
[Repository ownership](docs/repository_map.md) and the
[architecture record](specs/first_release_architecture_redesign.md) provide detail.

## Current implementation and local evidence

Production multilane work has [six implementation milestones under an active goal](specs/sumeragi_v2_multilane_completion_goals.md).
Source inspection confirms substantial implementation beyond the original plan;
fresh qualification remains open. The unmerged index entries have been resolved;
source and receipt inventories are being reconciled. No milestone or release gate
is closed by this audit. The focused JVM consensus/HTTP selection passes 123 tests,
zero failures/errors/skips, including immutable evidence ownership, bounded wire vectors and Java
consumers of Kotlin. Both migrated Java release runners pass (59 diagnostics and
6 grouped tests); 10 affected legacy settlement/onboarding checks also pass. Four private-invocation layout tests and 55 formal-launcher/artifact-retention
tests pass. QueuePlan's source ledger/startup controls pass 85 cases plus five
semantic checks. Six finite TLC models, 106 multilane mutations and 22 in-flight
mutations pass with fresh retained raw traces; five Apalache bounds pass and the
in-flight 18-step run is pending.
JS/Python settlement ownership and exact TWAP checks pass 63/65 grouped tests
and 152 related Python tests; Kotlin retains signed Numeric and bounded Quantity
semantics. Rust completion cleanup and crash/classifier regressions await the
shared build queue. Current cohort source-closure and receipt checks pass 23/7.
Public-certificate source controls pass 19 cases. A subsequent recovery fix stages
the complete bounded proposal set before eviction and prevents completed historical
source resurrection; its four new cache tests and updated hydration regressions
await Rust execution. G-UNIT now registers 526 tests; production remains 881 across
43 modules. New recovery source controls and full integration remain in progress.
All 17 Swift wire fixture tests pass in
an isolated source build after bounding vector allocations; the complete SDK
gate still needs its real bridge artifact. The broad receipt/bootstrap selection
finishes with 555 passes and one cache parent-change rejection; that case passes
on isolated retry. Source-contract reconciliation, a settled full receipt run,
fixture regeneration and full release qualification remain open.

| Area | Observed evidence | Practical limit |
| --- | --- | --- |
| Privacy V1 admission and field carriers | Canonical 48-byte digest and 32-byte Fp4 carriers; complete signed synthetic qualification passes bounded decoding. Rust capability FFI passes 2 tests; corrected Exact12 fixtures pass JS 23, Python 94, Kotlin 16 and C# 13. SDK admission source checks pass 86 regressions; model privacy selection passes 127 tests and the final intent KAT is independently reproduced. Shared six-lane framing passes 39 primitive tests and the ACE selection passes 26; AXT binding passes 76 FASTPQ and 7 Core regressions. | Core native selection passes all 63 tests; the corrected complete proof crate passes 1,299 with zero failures and 13 ignored. Current SDK origin/network checks pass JS 37, Python 103, Kotlin 63, C# 17 and Swift source-only 35. Apple duplicate archive ownership is corrected and three target link controls pass; the fresh full XCFramework build is running; complete GPU proof dispatch, AXT authoritative state/execution binding, independent review and four-validator qualification remain open in the [closure ledger](specs/privacy_first_release_closure.md). Synthetic signatures are validator tests only. |
| SCCP TON scoped audit | [Validated fixes and evidence](docs/source/sccp_ton_security_audit_2026_09.md): ordinary transfer funding, bounded replay work, native TL-B parsing, exact circuit checkpoint identity, complete breaker transaction readbacks, and SCCP builder Git/verifier/attribute isolation. All 191 SCCP Rust tests (35 TON native), 9 Core TON breaker tests, the new model regression, 42 TON contract tests, focused Go tests/compile checks, and Core/Torii production library compilation pass; authenticated StateInit regenerated. Combined builder/golden/release/corridor tests: 377 passed, 28 external-evidence/validator-dependent skips. All 8 R1CS identities are current and bound to checked definition/dependency sources. | Full Core/workspace and Torii runtime tests are unclaimed. Dependent release artifacts require regeneration and independent qualification against refreshed circuit identities. |
| Rust SDK dependency separation | Relay accounting moved to `soranet_incentives`; SoraNet policies and shared defaults have one `iroha_service_model` owner. SDK/config/CAR/orchestrator/CLI production library and binary checks pass after the policy and logging migration. | Storage orchestration and telemetry remain SDK dependencies; Musubi service ownership, immutable client contexts, asynchronous transport and consumer migration are incomplete. |
| Configuration and status HTTP contracts | Shared configuration DTOs have 22 wire tests, 3 node conversion tests and 14 Core runtime tests. Shared status preserves 32 captured named DTO frames/hashes/JSON; 93 telemetry tests pass. Core, SDK/CLI, test-network, schema generation, Mochi and grouped consumers compile. | Named records are qualified by focused fixtures; arbitrary generic-envelope schema identity is not yet cut over or fully qualified. |
| Core integration fixtures | The `core_api` harness compiles after all 21 identified fixture errors were repaired through canonical APIs. | The new four-validator configuration startup/restart/readback/isolation scenario has not run against rebuilt binaries. Compilation is not runtime evidence. |
| Torii evidence API | Grouped telemetry harness compiles. All 3 evidence runtime tests pass, including genuine four-validator BLS proof-of-possession/signature material and tamper rejection. | These tests do not qualify the full node consensus/release corridor. |
| Atomic private settlement | Audited-input Core selection: 140 pass, three fail, two proofs ignored; its native-owner full three-output proof passes. The exact model binary passes 28 tests. Two stale test expectations are corrected; continuation exposed a raw SHA/hash-wrapper mismatch corrected with an exact 32-byte field. Earlier daemon and 28 integration preflights pass. | Source changed during compilation. Exact-byte and continuation corrections need a fresh combined run; ten-run network, leakage, benchmark, independent audit and release qualification remain open. See [the protocol](specs/private_settlement.md). |
| Norito schema preparation | Canonical identity kernel, derive support, primitive/crypto declarations and strict UI tests pass. All 19 planned base-model declarations preserve 189 captured frames, signatures, JSON, storage keys and schema output in two tests passing before/after declarations. | Active codecs remain unchanged. Complete declaration coverage, atomic cutover and model moves remain pending; see [the identity contract](specs/norito_schema_identity.md). |
| Norito migration capture | Eight probe-generator, 17 driver and 32 syntax/context-graph tests pass. Controlled crypto/model captures pass 206/6,112 probes. Another 108 crypto declarations pass nine module suites and seven existing wire/identity golden tests. | The reviewed 1,771-declaration model batch is applied; its fixture suites await qualification. Generic, local, generated and other feature selections remain incomplete. Shipping codecs are unchanged. |
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

Musubi ownership preparation passes 4 filesystem and 31 dependency-guard tests. Consolidated CAR validation passes 20 tests; all 76 SDK Musubi tests preserve 102 complete golden frames and exact signed transcripts. The repaired complete SDK library suite passes **809 tests, zero failures or ignored tests**; its raw DA digest correction also passes eight owning-model tests. Service extraction remains incomplete.

These are scoped working-tree checkpoints. They do not imply that workspace
build/tests, strict all-target Clippy, all SDK platforms, or release workflows
have passed together against one immutable source tree.

## Build and architecture qualification

SoraFS goal execution is tracked in the [V1 implementation goals](specs/sorafs/v1_implementation_goals.md).
The initial release-hardening slice fixes signing-output directory substitution,
checks the source seal before Cargo metadata, and restores native mobile CI with
mandatory per-task execution evidence. The combined script/contract selection
passes 714 tests. The complete manifest library rerun passed 891 tests with no
failures or ignored tests. The focused Core security run passed 11 of 15
tests; registry-label and bootstrap-state fixture corrections await a fresh
Core binary. The canonical account binding passes its model regression and three
node authorization tests; Torii passes three binding, three OpenAPI and three
query-filter tests. The validator CLI passes 56 tests, fixture generator 12 and
Python fixture checker 30. Both Kotlin/Java-source PoP tests pass with a fresh
native bridge and no skipped tests. Signed-manifest qualification requires independently
authenticated source files; signed-state/receipt verification passes 11 tests,
and seven actual CLI process checks pass over simulated signed inputs. Python
gate integration remains pending. The four-peer repair target
compiles; all 23 custody-verifier, 26 operation/control/recovery and three bounded
credential-reader tests pass. Network execution and production hardware/state
adapter integration remain pending. Native CI execution,
the source seal, HSM custody and production qualification remain open. See the
[current closure checkpoint](specs/sorafs/v1_closure_ledger.md#2026-09-06-execution-checkpoint).

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
violations and oversized source findings are intentionally visible. No budget,
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
