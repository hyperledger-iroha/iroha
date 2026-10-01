# Status

Reviewed 2026-10-01. Iroha 3 remains under implementation and qualification.
Component checks cover substantial portions of the system, but the combined
source has not passed the complete workspace, SDK, hardware and release gates.
The [roadmap](roadmap.md) lists outstanding outcomes; the linked specifications
hold detailed acceptance criteria. Routine repair receipts belong in PRs and CI.

## Current implementation

| Area | Current state | Remaining qualification |
| --- | --- | --- |
| Sumeragi | The sans-IO core, simulator and node driver are integrated; the previous consensus runtime is removed. Certified results bind World roots and ordered events. | Current-candidate simulator/mutation gates, consumers, network faults and authenticated accelerated restoration. |
| Lanes and dataspaces | Fixed/elastic lanes run as Sumeragi instances; the global chain merges certified lane blocks. Lifecycle/restart have component and node coverage. | Dataspace instances with their own State, cross-dataspace AMX, isolation and current-source network scale/restart. |
| Storage and execution | `lanes::LaneRunner` and `SumeragiLaneMerge` are the production path. Kura owns a shared fail-stop gate and authenticated native tips/journals. | Original funded execution custody through acquisition, certification, publication, replay and retained-generation reclamation. |
| Configuration and DPN | Private dataspace definitions are separate from validator settings. `iroha dataspace plan/apply/status` derives artifacts and retains once-only transactions under one budget. Kagami has an isolated BPNG catalog/paid-namespace genesis preset. | Profile-based generator closure, four-daemon paid deployment/readback and physical isolation. BPNG local contracts, fee authority and application provisioning remain incomplete. Owner-node provisioning is outside this path. |
| Kagami/Mochi developer experience | Shared native localnet generation, process ownership and workspace contexts are implemented in `iroha_deploy`; Musubi exposes source/artifact/package deployment with exact retained recovery. Native private-file custody and release-signed checkpoint authentication have component coverage. | Desktop cutover, matching installed-binary startup/deployment/restart, independent private dataspace execution and parent admission, official checkpoint publication, native OS matrices and latency targets remain under qualification. See the [developer goals](specs/kagami_mochi_devex_goals.md). |
| Rust client | Immutable account contexts, owned async transport, explicit blocking capabilities and typed fee quoting are implemented. | Remaining capability/consumer migration, unified errors and network cancellation/finality/authorization coverage. |
| Kotlin/JVM | Kotlin owns the SDK, HTTP/SSE/WebSocket, attestation tools and JNI API; Java consumers exercise that API. Host coverage includes native/confidential operations. | Remaining Java/publication retirement, signed packages, CUDA hardware and Android/device qualification. |
| Other SDKs | Shared prepared-operation, signing, account and native checkpoint contracts are being migrated across Swift, JavaScript, Python and C#. | Same-source native artifacts, complete fixtures/consumers and release OS/architecture matrices. |
| Norito | Declared identities own canonical frames; payload serialization/reconstruction and explicit JSON key contracts are integrated. | Consumer/feature closure, fallible allocation ownership, physical model extraction and workspace lint/runtime coverage. |
| IVM/Kotodama | IVM is the sole VM with ABI V1. Compiler separation and state-free proof owners reduce normal dependency graphs. Source bundles support declaration includes and explicit module exports; authenticated error-message catalogs preserve nominal schemas. | Lifecycle/custody closure, native execution proofs, anchored private invocation/AXT, coherent SDK regeneration and hardware validation. |
| SoraFS | Software signing, canonical manifests and storage/billing/publication ownership are implemented and under repair. | Node failures, matched daemon/harness, provider resilience and L1/L2 promotion. |
| KAGEMUSHA | Bounded bootstrap/startup, request-bound native finality and signed top-up boundaries have component implementations. | Durable hardware authority, recursive monetary proofs, reserve settlement, provisioning and physical-device evidence. |

## Immediate blockers

Exact finalized-carrier retries now authenticate the original execution and
acknowledge admission without requeueing or charging again. The embedded MCP
descriptor size drift that prevented daemon startup is corrected with a bounded
loader and compile-time guard. A fixed local daemon/harness candidate completes
the Nexus smoke workload's financial, signed RS16 finality and exact-retry checks
across all 16 peers and preserves the settlement through all 16 restarts. Its
settlement takes 59.17 seconds. The campaign validator now recognizes the exact
retry observer's canonical phase label and verifies the retained evidence.
The rebuilt disjoint-committee workload also preserves progress while one lane
stops and after every peer restarts, covering the applied-body pruning repair.
Ten fresh paid-settlement runs also pass on that fixed candidate, each with 16
signed RS16 observations, exact-retry checks and finality at height 9. Qualification
of the combined source and the settlement latency target remain open.

Core/World acquisition and retained State ownership are being repaired without
oversized-stack workarounds. The combined test graph, complete resource funding
and original Validate-to-Apply custody remain open. Component repairs do not
establish one retained execution through finality and restart.

Remaining SoraFS Node failures concern fixture permissions, hedged-encryption
randomness assumptions and cumulative quarantine decode budgets. The Core
selection has scoped coverage and Torii's unit-test target compiles. Full Node
and production qualification remain open under the
[reliability goals](specs/sorafs/first_release_reliability_goals.md) and
[closure ledger](specs/sorafs/v1_closure_ledger.md).

The private-settlement five-second proof-through-settlement target is unmet.
Current-source repeated settlement, fault and leakage campaigns are unqualified;
prior component timings and network observations do not qualify changed source.
See the [protocol](specs/private_settlement.md).

## Deployment state

Taira runs the signed e35c10ba release with fresh validator keys and signed genesis.
All four validators return readiness HTTP 200, have three peers, and converged
at height 3 after an ordinary signed transaction resolved to StateApplied.
The public endpoint at `https://taira.sora.org` has verified TLS, and the
same-revision basic doctor reports healthy public routes and curated MCP tools.
The previous live ledger and twenty obsolete validator releases were deleted.

Current source fixes deployment recovery and initializes fresh safety records
before first startup. Deployment preparation, transfer and the routine updater
accept an authenticated build-only candidate without a full regression gate.
On-chain governance owns deployment policy; no fixed 24-hour fault test is a
deployment prerequisite for testnet or production.

Beacon custody activation, physical DPN, paid `dpn`/`admin@dpn` and clean-client
completion remain open. Validators run in a Linux guest on MacStadium in Dublin;
use the approved deployment tooling. Retained incident records describe the
[previous readiness failure](docs/incidents/2026-09-30-taira-readiness.md).

BPNG retained-history qualification, validator catch-up, additive catalog
activation and API22/FE17 application commissioning remain open. Basic acceptance
does not close crash/concurrency or advanced guest work. Infrastructure changes
require the explicitly approved OVH target.

## Build and release qualification

Recorded `051df111` checks cover workspace all-targets, Core/Kagami,
Torii/bridge, CLI/daemon and both network-test targets. Separate owner-extraction
checks cover normal native/JS/Python frontends and dependency/codec guards.
These results qualify their recorded source. On merged base `222e30c4`, the
recorded privacy candidate passes normal builds, all three authentic pins and all
1,017 selected ordinary controls. Required-Metal coefficient parity and full RFC
key replay pass. Its maximum proof passes fresh verification and byte/RSS limits;
proving takes 2,368.35 seconds against 300 seconds, and observed virtual size
exceeds the literal 32 GiB limit.

The corrected normal Core/Kagami/SDK build and genuine 54-role producer pass.
Paired finalized-execution captures and consumers agree byte for byte. Authentic
canonical fixtures, managed verifier corrections, bounded retail-journal recovery,
IVM descriptor/copyback/partial-dispatch/publication controls and the new X509
private SHA/RFC bridge are integrated. Normal privacy builds and regenerated pins
pass, but the expanded selection has 1,039 passes and 19 failures; repairs require
native rerunning. Successive SDK producer builds expose IVM fixture imports and
JSON-array serialization errors; their corrections await a fresh build. The
actual release-evidence Python suite passes 130 tests. The last native
identity selection failed cold Recover/Recover before the repair. Full bridge,
Kotlin, Swift, workspace and four-validator checks on the combined candidate,
physical-device evidence and signed release artifacts remain incomplete.

Dependency ownership and pinned compiler-memory measurements remain gates; see
the [architecture plan](specs/first_release_architecture_redesign.md) and
[compile-bloat goals](specs/compile_bloat_optimization_goals.md).

Memory qualification retains a 25% model-baseline reduction, measured limits for
introduced units and a 13-GiB per-unit release ceiling. Core/Torii test and daemon
baselines need complete successful builds; native release baselines exceeded the
ceiling in Core/model. Comparable candidate measurements need one pinned runner
and toolchain. The [profiler guide](docs/profile_build.md) defines the method.

Release requires one immutable candidate with complete workspace build/tests,
applicable strict Clippy, formatting, canonical codec/ABI fixtures, generated API
and artifact provenance, native/SDK matrices and authenticated release workflows.
CI routing has local source coverage; actual CI/corridor execution and final
aggregation need qualification. Missing artifacts, blocks and timeouts are not
passes.

## Remaining production risks

- **Consensus/staking:** complete [Sumeragi S1–S9](specs/sumeragi_goals.md), signed
  RS16 availability at whole-node/network scope, DS-local State/AMX, E+2/beacon
  custody and [paid 4→7→4 transitions](specs/staking_validator_completion.md)
  with restart, rewards, exits and slashing.
- **Privacy/crypto:** the recorded maximum X509 proof and fresh verifier pass
  byte/RSS and cryptographic acceptance, but proving takes 2,368.35 seconds
  against 300 seconds and observed virtual address space exceeds 32 GiB.
  All 1,017 selected ordinary privacy controls, genuine pins and required-Metal
  parity pass on that recorded source. The new private SHA/RFC bridge removes
  112 further public values and needs native validation; 320 public intermediate
  values and additional byte-source joins remain. Historical q77 maximum
  ordinary/AXT proofs and fresh replay pass byte/RSS limits on their recorded
  source. RAM-LFE secure encryption/full execution, IVM execution/finalized-State
  binding, complete protocol/side-channel review, hardware/network evidence and
  final signing remain open under the [ZK goals](specs/zk_first_release_goals.md).

- **Services:** Musubi publication/paid contracts, Parliament/standalone elections,
  SoraNet/Linux helpers, SCCP live corridors and Inrou Linux/AArch64/KVM isolation
  remain unqualified.
- **Offline money/devices:** the ordinary app profile uses an attested persistent
  hardware key, genuine platform admission and independent Native financial
  custody. Recursive proofs, current-owner integration, mint/redemption,
  adversarial recovery and signed physical qualification remain open. Ordinary
  key signatures do not establish a non-forking journal or hardware clock.

First-release contracts remain canonical APIs, domainless `AccountId`, Norito
wire formats and deterministic ABI V1. Sumeragi uses exact `3f + 1` global
committees, exactly `n - f` votes, signed RS16 availability and work-driven blocks.
Ordinary signing supports authenticated software custody. The first production
KAGEMUSHA app profile requires governed app-key admission, genuine monetary
proofs and exact current-owner/replay authority; it has no custom applet or
one-use-key prerequisite.
