# Roadmap

Last updated: 2026-09-08.

This is the outstanding-outcome view for Iroha 3's first release. Component owners
below are code responsibilities, not invented individual assignments. Local
results live in [status](status.md); completed and superseded observations live
in the [historical archive](docs/history/2026-09-06/index.md). The
[coverage map](docs/history/current-roadmap-coverage.json) maps every original
roadmap area to the outcomes below; archival does not mark an obligation complete.

First-release design is canonical-only: remove duplicate implementations and
obsolete surfaces rather than retaining aliases, compatibility dispatch or
migration shims. Approved focused crate additions and coherent manifest/lock
refreshes serve real ownership boundaries. Keep mandatory protocol capabilities
assembled in every node; do not feature-gate deterministic consensus semantics.

Taira rollout is blocked by failed four-validator startup. Qualify the bounded
authentication/cancellation and durable finalization handoff fixes, then prove
public transaction application on all four validators, canaries, restart proof,
public cutover and application connectivity. Reuse stable build lanes and verify
closed-attempt reclamation through the maintained retry path. Run shared lifecycle
source assertions before Cargo, qualify consistent Nexus descriptor defaults
and complete the combined native gate without relaxing the four-validator
or release checks.

## Architecture and build ownership

The [approved design](specs/first_release_architecture_redesign.md),
[repository map](docs/repository_map.md), [SDK route inventory](docs/sdk_inventory.md)
and [schema contract](specs/norito_schema_identity.md) define these boundaries.

| ID | Outstanding outcome | Component owner | Completion criteria |
| --- | --- | --- | --- |
| A1 | Foundational, privacy and service model boundaries | `iroha_data_model`, model crates, Norito/schema | Independent cohesive units with acyclic dependencies; preserve mandatory aggregate protocol JSON for admission and consensus parameters while qualifying supported feature graphs. Complete explicit generic schema identities, exact named/frame goldens and consumer migration before atomic codec cutover and physical model extraction. |
| A2 | SDK and service runtime separation | Rust SDK, storage client, Musubi service | Remove telemetry/CAR/orchestrator runtime edges; one service owner for journals, clocks and publication; one typed storage adapter; dependency-boundary gate passes for shipping normal/build graphs. |
| A3 | Immutable asynchronous Rust client | Rust SDK and all callers | Public/account/operator contexts enforce authority; one async transport and explicit blocking facade; cancellation, bounded responses, retry/finality and streaming regressions pass; remove mutable/global transports and migrate consumers. |
| A4 | Cohesive Core and Torii modules | Core state/storage/execution; Torii route capabilities | Separate World schema, restore, merge, execution and caches; preserve one atomic block overlay, rollback/replay and canonical storage definition; capability-owned route construction; no new facade-only layers. |
| A5 | Enforced architecture and compiler memory | Build tooling and component owners | Dependency checks and 5,000/3,000 source-file limits pass without expanded exceptions; substantive duplication removed. Repair Core/Torii test and daemon baselines; run the complete memory comparison on one pinned runner/candidate, preserving per-unit bounds, 25% model reduction and the 13-GiB release ceiling. Qualify lower-memory Mac and Linux cgroup hosts. |
| A6 | CI and generated-artifact ownership | CI router, generators, SDK consumers | Execute selected binary-free/binary-consuming lanes and accurate required aggregation; retain authenticated specialized daemon flows. One create-only Norito RPC fixture producer and verifier; generated manifests and SDK operation/signature inventory are complete and drift-checked. |
| A7 | One Kotlin/JVM implementation | Kotlin core, Android modules, Java consumer tests | Migrate supported capability gaps, fixtures, native exports and publication paths; then delete mirrored Java implementations/factories. Keep JDK 8 API enforcement, no reflection, Java-callable canonical APIs and Android isolation. |
| A8 | Current source-coupled documentation | Component maintainers and docs owners | Keep root views within 300 lines, exact historical reconstruction and working navigation; reconcile detailed specs with canonical implementation; public guides live in optional `iroha-docs`, never a build prerequisite. |

## Ledger, consensus, identity and configuration

Use the [Sumeragi v2 contract](specs/sumeragi_v2.md),
[closure ledger](specs/sumeragi_v2_multilane_closure_ledger.md),
[active implementation goals](specs/sumeragi_v2_multilane_completion_goals.md),
[soak matrix](specs/sumeragi_soak_matrix.md),
[liveness specification](specs/sumeragi_v2_liveness.md) and
[evidence API](specs/sumeragi_evidence_api.md).

| ID | Outstanding outcome | Component owner | Completion criteria |
| --- | --- | --- | --- |
| N1 | Revision-4 DA and runtime qualification | Core/Sumeragi, P2P, DA workers | Signed RS16 stays mandatory; exact 4- and 7-validator loss, parity reordering, view/lifecycle retirement, restart and certified-body fallback. Authenticate hold/drop acknowledgement before healing; profile malformed/adversarial sessions and qualify any sole systematic-first fanout policy before adopting it. |
| N2 | Formal, fault and multilane release evidence | Sumeragi reducer/WAL/runner and formal owners | Same clean signed candidate passes strict TLAPS, pinned Verus, cross-tool/production trace mapping and mandatory revision-4 TLC/mutation checks. Archive five four-validator scenarios at 32 seeds and 100,000-height permissioned/NPoS chaos; finite real-work inclusion then stable empty-queue heights. Historical obligation counts are not fresh proofs. |
| N3 | Dataspace-owned topology and additive SNS activation | Nexus/data model, Core, node deployment | Dataspace manifests own physical membership/privacy/DA/governance; lanes share the owning policy; namespace/dataspace routing is typed. Four-validator additive activation preserves Kura/CommitQC and cold replay. Archive distinct physical server/storage cohorts before claiming live private dataspaces; no history reset. |
| N4 | Bounded Core execution and queries | Core state, trigger, query registries | Consensus-configured trigger/work budgets and reverse permission index; one stored/ephemeral query registry; isolate invariant-bypassing test mutators; production reachability/lint checks; deterministic admission/query/state benchmarks and autonomous lane rollback/restart tests. |
| N5 | Canonical account, fee and transaction model | Data model, executors, SDKs | Remove inert provider-owner instructions, domain-selector/rekey and boxed initiation remnants; one executable carrier and signing-token registry. Typed indexes/borrowed proof checks; enact Hijiri base-fee policy, respect 120,960-block delay, qualify quote authority, stale/private/loss cases and native fee-bearing transactions on four peers. |
| N6 | Authenticated Kura startup and recovery | Kura and daemon startup | Emergency Fast remains read-only and cannot drain or produce durable work. Large-history benchmarks separate bounded authentication/startup costs and prove no historical-body/snapshot/World decode; Strict alone restores deferred write authority. |
| N7 | Network identity, routed lists and pending obligations | Core/Nexus, Torii and Connect | Exact chain/genesis/network identity throughout; one snapshot/global ordering contract for routed pagination; authenticated QueuePlan cancellation/rebind; Connect incarnation or durable tombstone and authenticated renewal/deadline policy; audited irreversible AXT family retirement and per-issuer quotas. |
| N8 | Configuration closure | `iroha_config`, runtime owners | Remaining ordinary-input errors use aggregated parsing; reject invalid bounds, aliases and silent clamps. Cohesive user/actual modules and single defaults; no production environment controls. Run four-validator startup/restart/readback/isolation and bounded TOML/projection benchmarks. |
| N9 | Safe observable runtime | Telemetry, logger, Torii | Producer/consumer reachability for metrics/routes; crash, disk-full, half-close and permission-failure durability with exact retries and exporter isolation; status/gather/filter/reconnect allocation and latency bounds. |
| N10 | Current governance and recovery fixtures | Core governance tests, integration owners | Migrate removed ballot/lock/referendum fixtures through canonical constructors, resolve remaining merged-candidate compile failures and qualify the complete production/test feature matrix without reintroducing old APIs. |
| N11 | Atomic private settlement qualification | Core/private settlement, Nexus, Torii, native proof and evidence owners | [Canonical protocol](specs/private_settlement.md): QC-bound historical authority and one immutable merge view; exact raw auditor-input SHA256 binding, virtual-zero continuation and unchanged-source adversarial/native/PQ full proofs; ten fresh 16-process N=3 successes, then N=2,3,4,8,16 faults at ten seeds each. Paired leakage, five warmups/thirty measured bundles and transparent controls; complete formal/independent audit, signed reproducible builds, SBOM, custody and validated release evidence. Remain disabled until all gates pass. |

## SDK, native and device delivery

The [JVM capability inventory](specs/jvm_consolidation_inventory.md),
[native bridge release contract](docs/norito_bridge_release.md),
[KAGEMUSHA readiness](specs/kagemusha_v1_production_readiness.md) and
[physical evidence specification](specs/kagemusha_v1_physical_evidence.md) remain authoritative detail.

| ID | Outstanding outcome | Component owner | Completion criteria |
| --- | --- | --- | --- |
| S1 | Python canonical transport and distribution | Python SDK/native owners | One typed credential and transport/session implementation; cohesive route modules and export inventory; cold-import/large-response/codec budgets; same-source native vectors, clean wheel/install/typing and four-validator corridor. |
| S2 | JavaScript client, codec and packaging | JavaScript SDK/native host | Cohesive route families, exact option/export allowlists and explicit codec context; single `dist/` artifact authority; hard eager/deferred/combined bundle and RSS budgets; portable/native test lanes, exact source closure and native/four-validator parity. |
| S3 | C# immutable API and release package | C# SDK/native bridge | Canonical immutable results and parse boundary, allocation benchmarks, reviewed API surface and public-member docs; pinned .NET formatting/build/sample/package checks plus native privacy/Hijiri/SoraFS tests and real Windows packaging execution. |
| S4 | Cross-SDK wire, signing and activity APIs | All maintained SDKs, Torii models | Shared signing tokens, one executable carrier, canonical CurveId and exact fixtures; typed multisignature witness construction; snapshot-bound participant activity feed and time-range outgoing-value aggregate with bounded opaque cursors and explicit expiry. |
| S5 | Remaining Kotlin capability closure | Kotlin/JVM and Android owners | Carry the completed Nexus, verified Nearby, bounded HTTP/SSE/WebSocket and Java-consumer evidence into release qualification. Carry completed Kotlin attestation CLI/verifier and Android host-consumer checks into release qualification. Complete remaining Java/JNI/publication retirement and rebuilt CUDA binding execution. Run the canonical CUDA CPU-reference hardware gate on qualified hardware; keep native/device evidence separate. |
| S6 | Same-candidate native and alias/SNS evidence | Native bridge and release-platform owners | Signed ABI-23 target inventory; rebuilt AAR/XCFramework/JNI/wheel/host packages; complete Swift/Kotlin/Java-consumer/JS/Python/C# fixture and alias/SNS replay on the exact release OS/architecture matrix. Local host passes do not qualify publication or devices. |
| S7 | Three-message recursive KAGEMUSHA product | Core, wallet coordinator, proof and SDK owners | Only Request/Payment/Acknowledgement; fixed Bootstrap/MintFold/SendSplit/ReceiveFold/RedeemSplit/Rotate over aggregate balance. Ordered recursive claim-fold binds mint/finality/replay authority; 1,024+ real handoffs and 1,000-funded-device merchant corridor with history-independent work and canonical size limits. |
| S8 | Durable hardware money and settlement | KAGEMUSHA coordinator, hardware/native and Core reserve owners | Authenticated history, exact-next hardware successor, trusted time, inbox/outbox and byte-identical recovery at every crash boundary; finalize pooled top-ups/redemptions with reserve/nullifier/concurrency protection. No host-only monetary authority or software fallback. |
| S9 | Android/iOS/nearby/NFC physical matrix | Android/Swift SDK, native services and device providers | ABI-23 arm64/x86_64 JNI plus complete device operations 1–22; qualify each model/OS/firmware/provider, including HarmonyOS separately. QR/Nearby/NFC cross-device directions, RF/power loss, replay, backgrounding, clock/restore/rollover, memory-pressure/busy retry, thermal/latency/value-conservation evidence. |
| S10 | Governed ZK-ACE and private-file SDK closure | JavaScript, privacy and native packaging owners | New typed governed two-pass intent-bound signed transaction with private nonserializable witnesses and erasure; no retired attachment/key/backend API. Execute Windows secure-storage tests and rebuild signed native artifacts from one authenticated source/lock/provenance set. |

## Cryptography, VM and proof systems

See [Norito](norito.md), [schema identity](specs/norito_schema_identity.md),
[IVM architecture](specs/ivm_architecture_plan.md),
[FASTPQ plan](specs/fastpq_plan.md), [privacy V1 closure ledger](specs/privacy_first_release_closure.md) and the detailed historical
[privacy/proof records](docs/history/2026-09-06/records/crypto-proofs/index.md).

| ID | Outstanding outcome | Component owner | Completion criteria |
| --- | --- | --- | --- |
| C1 | Canonical Norito archive and derive APIs | Norito, derive family and primitives | One fallible aligned-storage helper with allocation accounting; owning/borrowed archive context replaces global marker state; retire consumerless experimental adapters. Share generated-emitter implementation; production reachability, compile-time/token-size budgets and strict UI tests. |
| C2 | VM/compiler deterministic performance | IVM, Kotodama and host owners | One V1 interpreter/ABI and strict SSA optimizer; cohesive loader/execution/pointer/metering/proof modules. Identical gas/traps/state/proofs across threads and hardware; cache/retained-byte bounds; source/lock-authenticated compiler benchmarks and required M1 Ultra/Graviton3 Numeric calibration archives. |
| C3 | Signature and cryptographic boundary audit | Crypto and all signature consumers | Mixed-torsion Ed25519 with/without batching; canonical key/PoP roster admission; deferred offline/attestation consumers; independent threshold-BLS/timed-OVN arithmetic, side-channel, custody and erasure review; minimal/application/consensus/node/FFI feature closure and bounded allocation benchmarks. |
| C4 | FASTPQ proof architecture and soundness | FASTPQ prover/verifier and independent reviewers | Complete authenticated bounded-opening admission and independent qualification. Validate protocol adversary/qROM multi-artifact reduction and independent permutation implementation; bind reviewed parameters to regenerated final proof artifacts before activation. |
| C5 | Privacy proof authority and degree bounds | ZK-ACE, native-STARK, AXT and IVM admission | Bind AXT exact finalized source state and issuer intent/amount; independently qualify the implemented six-lane ZK-ACE commitments, qROM/multi-target reduction and regenerated AIR/FRI certificate; exact48/exact32 and six-symbol SDK admission parity; explicit terminal degree/geometry checks; unsupported public-padding proof paths stay disabled. |
| C6 | Full FHE/MKHE and Figure 9 | Crypto, privacy model and proof owners | Replace BFV-shaped scaffolding with full BFV-RNS. Complete atomic 40-limb MKHE source/materialization/packing/cross-field/padding verifier and full-size/eight-party KAT with measured resources. Install governed full-shape Figure 9 keys and independent proof vector; no receipts/readiness from unavailable stages. |
| C7 | Hardware acceleration qualification | Crypto/proof native backends | Native Metal/CUDA compilation, actual CPU/GPU/KAT/root parity, copy/launch/stream/timeout fault quarantine, side-channel review and implementation-derived RSS/throughput. T256/MKHE remain scalar until these conditions hold; no feature-build substitution for hardware execution. |
| C8 | Kaigi private-session model | Kaigi model, crypto, Core and SDKs | Qualify final authorization/usage circuits, mandatory host proof and retained original-account lifecycle/rekey/undo state. Exact regenerated keys/schema and shared SDK fixtures; suite-tagged HPKE, bounded usage/accounting, authenticated relay recovery/archive policy and measured signal-index startup. |
| C9 | Kotodama V1 syntax and usability closure | Compiler, ABI, Core/VM, CLI, SDK and editor owners | Complete the [approved redesign acceptance ledger](specs/kotodama_v1_redesign.md): exact labels, Unit/errors, checked/fallible lists, must-use, bounded live pagination, fused rounding, named patterns, semantic tools, exact rejection assertions and offline first-project workflow. Regenerate only final V1 artifacts and obtain every behavior test, four-validator integration and workspace evidence. Re-establish unavailable temporary evidence on the current source candidate; retain the passing compiler/Core/public-boundary, CLI and actual offline-project selections; finish final artifact checks, then qualify native SDKs, normal-release four-validator execution/restart and workspace commands. Distinguish proven baseline failures from unclassified failures and regressions. |

## Product services and deployment

| ID | Outstanding outcome | Component owner | Completion criteria |
| --- | --- | --- | --- |
| P1 | Parliament runtime and independent review | Governance model/reducer, crypto and Torii | Exact 18-event surface, atomic Policy→Confirmation, bounded redraws, finalized beacon, private ballot/deadline/retry, rollback and restore on four validators; independent timed-OVN/threshold-BLS review and source-bound public API/SDK artifacts. See [governance pipeline](specs/governance_pipeline.md). |
| P2 | SoraFS production promotion | SoraFS, operators and evidence owners | Complete the [V1 implementation goals](specs/sorafs/v1_implementation_goals.md) and [closure ledger](specs/sorafs/v1_closure_ledger.md), including the required HSM/PKCS#11/KMS custody contract; live four-voter/multi-provider/dual-gateway L1 with resilience/load/24-hour soak and exactly 17 fresh summaries; ordered L2 promotion legs and trusted foundational envelope. Synthetic aggregates cannot authorize promotion. |
| P3 | Governance DAG deployment | DAG service and deployment broker owners | Two concrete linked service instances, both Kubo/head ingress administrations, authenticated signing/custody and sealed CAS; failover/rotation/archive/rollback/corruption/outage/disaster recovery; five-target artifacts, SBOM and L1/L2 evidence. See [DAG plan](specs/sorafs_governance_dag_plan.md). |
| P4 | Secure QUIC, relay and VPN activation | P2P, SoraNet and privileged Linux helpers | Qualify a fixed QUIC resolution and abuse regressions before removing fail-closed gates; exact DATAGRAM accounting and deterministic fallback. Live pre-auth capacity/NAT tests; paid lease consistency; real TUN/pidfd/DNS rollback, hostile peer, rotation/loss/traffic-shape, fuzz and independent review. See [SoraNet handshake](specs/soranet_handshake.md). |
| P5 | Musubi registry and publication deployment | Musubi service, Core/Torii, cache and deploy owners | Complete long-detach/private HTTPS runner and authenticated recovery integration; descriptor-relative/no-follow atomic cache/publication and supported non-Unix guarantees; authoritative metrics, queue crash/restart, four-peer and namespace/two-week soak; supported-host whole-process 64 MiB target. See [Musubi contract](specs/musubi.md). |
| P6 | Transactional DA/Taikai publishing | Torii DA spool, Taikai publishers, CLI/CAR | Journal intended bytes/server time before side effects; recover or quarantine partial transactions; immutable lineage retries, shared checked V1 builders, file-backed bounded CAR and atomic summary publication; fix mutable-path races and run complete consumer matrix. See [ingest plan](specs/taikai_ingest_plan.md). |
| P7 | Audited SCCP production corridors | SCCP circuits/contracts, runtime/native and release operators | Taira↔Ethereum/BSC/TRON/TON mainnet only. Regenerate reviewed keys/witness/runtime artifacts against refreshed circuit identities and canonical sparse wire identifiers; real TRON TVM/TRE and exact pinned TON contracts/StateInit, including bounded replay runtime validation; approve builder policies for isolated Git source verification and archival; authenticated deployment readback, positive/negative real-value canaries and independently reproduced signed release bundle. No fixtures, synthetic proofs or proof-controlled rosters as authority. |
| P8 | Isolated Inrou and generated-HF execution boundary | Inrou guest/runtime and deployment owners | Real Linux/AArch64/KVM mount/network/IPC/UTS/PID/cgroup escape/resource evidence; fixed authenticated bridge, pinned lease/recovery and exact four-replica guest canary. Generated HF remains storage-only; any execution is a governed pinned guest, never host Python or a second proxy. |
| P9 | Persistent Taira joining and disposable networks | CLI/daemon and authorized network operators | One signed-bundle `iroha taira join` path initializes an owner-private permissionless observer, validates expiry/network/genesis and supports staking activation. Disposable four-validator `taira_devnet.py up` proves typed Applied, advancing convergence, MCP and guest workload; canonical source/artifact identity, no startup-only success or unmanaged cleanup. Complete the approved public reset with same-source Linux binaries, candidate convergence/canaries/restarts before cutover, preserved public Host routing, and test.inori.co.il application verification. Verify stopped-owner cgroup/firewall cleanup on the deployed native dispatcher. Replace per-release adapters with shared native read-only input/progress checks and immutable preparation reuse; run composed FD, inventory, timeout and stage-consumer gates before heavy builds, retaining one native apply recovery journal. Measure executable-only revision changes in the release build lane without invalidating shared libraries. |
| P10 | Native Torii MCP and capability ownership | Torii/shared route registry and SDK/CLI consumers | One in-process route/listener and canonical protocol; reviewed orthogonal authority/mutation/retry/sensitivity registry; bounded resources/read/prepare/inspect/external-signing flows. Simulation waits for side-effect-free bounded Core scratch execution; exact auth/cache/cancellation/response and four-validator release tests. |
| P11 | Mochi, Kagami and governed compute | Tool owners, Core/Torii runtime | Reachability and dependency gates, descriptor-relative private custody/atomic publication on shipping platforms and measured startup/snapshot/query costs. Qualify prepared prover-key mounts/restart; compute requires governed manifest/catalog/auth, bounded replay, real IVM metering and Kiso pricing before its config can advertise execution. See [Mochi plan](specs/mochi_architecture_plan.md). |
| P12 | SORA Economic Constitution | Economic design, oracle/governance and simulation owners | Specify purchasing-power basket, oracles/intervention/reserves before stability claims; capped term Phoenix certificates, one ring-fenced Producer Credit Facility, bounded governance lanes and reproducible run/default/capture/cartel stress simulations. Implementation is pending. |

## Candidate sealing, release and community

| ID | Outstanding outcome | Component owner | Completion criteria |
| --- | --- | --- | --- |
| R1 | Audited clean candidate and reproducible artifacts | Release engineering and every component owner | Resolve named audit dispositions and merged failures; full locked workspace build/tests, strict applicable feature/Clippy matrix, formatting, canonical ABI/wire fixtures and fresh native/SDK checks. Complete source/artifact/compiler identity and independent signed envelopes; blocked/timeouts never count as passes. See [audit matrix](docs/audit_closeout_matrix.md) and [release runbook](specs/release_runbook.md). |
| R2 | Generated API and authenticated operation approvals | OpenAPI/generator owners and release operators | Clean double regeneration, complete synchronized bundle/version manifests and generated-artifact inventory; explicit generated-client vs maintained-SDK ownership; external authorized signatures with `OPENAPI_REQUIRE_SIGNED=1`. Obtain the exact candidate-bound protected approvals; documentation never grants live mutation permission. |
| R3 | Community ownership and public evidence | Maintainers and public documentation owners | Clear contributor onboarding and repeat subsystem reviewers; official `@hl_iroha` Spaces/demos/Q&A, recaps and LFDT follow-up; distinguish public implementation updates from independent release evidence. |

Ordinary node/client/consensus custody remains provider-neutral, including
software custody; qualify runtime-only secrets, authentication, rotation,
revocation and durable recovery rather than vendor-specific product modes.
KAGEMUSHA offline monetary authority separately requires a governed non-forking
hardware profile and never permits software fallback.

The single first-release Sumeragi v2 revision-4 protocol uses mandatory signed
RS16 availability, dual count/power quorums and exact `3f + 1` committees with
`2f + 1` validator votes. Observers, global-RBC bypasses, empty-block liveness,
mixed protocol modes and compatibility paths cannot close any outcome above.
