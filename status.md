# Status

Reviewed 2026-09-30. Iroha 3 remains under implementation and qualification.
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
| Configuration and DPN | Private dataspace definitions are separate from validator settings. `iroha dataspace plan/apply/status` derives artifacts and retains once-only transactions under one budget. | Profile-based generator closure, four-daemon paid deployment/readback and physical isolation. Owner-node provisioning is outside this path. |
| Rust client | Immutable account contexts, owned async transport, explicit blocking capabilities and typed fee quoting are implemented. | Remaining capability/consumer migration, unified errors and network cancellation/finality/authorization coverage. |
| Kotlin/JVM | Kotlin owns the SDK, HTTP/SSE/WebSocket, attestation tools and JNI API; Java consumers exercise that API. Host coverage includes native/confidential operations. | Remaining Java/publication retirement, signed packages, CUDA hardware and Android/device qualification. |
| Other SDKs | Shared prepared-operation, signing, account and native checkpoint contracts are being migrated across Swift, JavaScript, Python and C#. | Same-source native artifacts, complete fixtures/consumers and release OS/architecture matrices. |
| Norito | Declared identities own canonical frames; payload serialization/reconstruction and explicit JSON key contracts are integrated. | Consumer/feature closure, fallible allocation ownership, physical model extraction and workspace lint/runtime coverage. |
| IVM/Kotodama | IVM is the sole VM with ABI V1. Compiler separation and state-free proof owners reduce normal dependency graphs. | Lifecycle/custody closure, native execution proofs, anchored private invocation/AXT, coherent SDK regeneration and hardware validation. |
| SoraFS | Software signing, canonical manifests and storage/billing/publication ownership are implemented and under repair. | Node failures, matched daemon/harness, provider resilience and L1/L2 promotion. |
| KAGEMUSHA | Bounded bootstrap/startup, request-bound native finality and signed top-up boundaries have component implementations. | Durable hardware authority, recursive monetary proofs, reserve settlement, provisioning and physical-device evidence. |

## Immediate blockers

The current Nexus happy-day workload exposed an execution-expanded proposal
publication defect. Checked projection and retained per-peer client context have
component coverage; telemetry/status wire retirement is applied. The successor
build, real workload and repeated accepted settlements remain pending. Restricted
native-lane gossip also needs fresh daemon/harness qualification with disjoint
global and participant committees.

Core/World acquisition and retained State ownership are being repaired without
oversized-stack workarounds. The combined test graph, complete resource funding
and original Validate-to-Apply custody remain open. Component repairs do not
establish one retained execution through finality and restart.

Remaining SoraFS Node failures concern fixture permissions, hedged-encryption
randomness assumptions and cumulative quarantine decode budgets. The combined
Core/Torii/daemon test graph compiles after the crate-extraction repairs. Full
Node runtime and production qualification remain open under the
[reliability goals](specs/sorafs/first_release_reliability_goals.md) and
[closure ledger](specs/sorafs/v1_closure_ledger.md).

The private-settlement five-second proof-through-settlement target is unmet.
Current-source repeated settlement, fault and leakage campaigns are unqualified;
prior component timings and network observations do not qualify changed source.
See the [protocol](specs/private_settlement.md).

## Deployment state

The latest pinned Taira observation on September 30 found four d431 validators
at height 10 with matching CommitQCs and empty queues. Health, liveness, faucet
policy and MCP responded, but every validator returned readiness HTTP 503. The
beacon install resolved to Applied; its proof and signer-provider activation
remained unfinished. The deployed recovery CLI loses the Canary operator key,
and its source-bound forward lease has expired. See the
[readiness incident](docs/incidents/2026-09-30-taira-readiness.md).

Current source fixes recovery arguments, checks readiness and initializes fresh
safety records. Authenticated Linux qualification and the authorized fresh
current-protocol cutover remain pending. Current codecs cannot authenticate the
retired runtime. The [reset runbook](specs/runbooks/sumeragi_taira_reset.md) requires
fresh four-validator readiness, write and restart evidence. On-chain governance
owns deployment policy; a fixed-duration fault soak is not a cutover prerequisite.

The retained height-3598 ledger remains a separate recovery obligation. Physical
DPN, paid `dpn`/`admin@dpn`, clean-client completion and production beacon custody
are not qualified by fresh bootstrap. Validators run in a Linux guest on
MacStadium in Dublin; use the approved deployment tooling.

BPNG retained-history qualification, validator catch-up, additive catalog
activation and API22/FE17 application commissioning remain open. Basic acceptance
does not close crash/concurrency or advanced guest work. Infrastructure changes
require the explicitly approved OVH target.

## Build and release qualification

Default daemon/CLI binaries have scoped compilation coverage after compiler and
runtime-owner extraction. Merged Core/test qualification, executable freshness,
timing and full workspace execution remain incomplete. Dependency ownership and
measured compiler memory are the active gates; code line-count gates are retired.
See the [architecture plan](specs/first_release_architecture_redesign.md) and
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
- **Privacy/crypto:** complete FASTPQ/X509 relations and bounded work, ZK-ACE/qROM
  and terminal-degree qualification, BFV/MKHE/Figure 9 design, independent review
  and actual CPU/Metal/CUDA conformance. Unsupported paths stay fail closed under
  the [ZK goals](specs/zk_first_release_goals.md).
- **Services:** Musubi publication/paid contracts, Parliament/standalone elections,
  SoraNet/Linux helpers, SCCP live corridors and Inrou Linux/AArch64/KVM isolation
  remain unqualified.
- **Offline money/devices:** recursive proofs, durable non-forking hardware,
  mint/redemption, adversarial recovery and signed physical profiles remain open.
  Secure-key signing and host JNI do not establish offline money.

First-release contracts remain canonical APIs, domainless `AccountId`, Norito
wire formats and deterministic ABI V1. Sumeragi uses exact `3f + 1` global
committees, exactly `n - f` votes, signed RS16 availability and work-driven blocks.
Ordinary signing supports authenticated software custody; KAGEMUSHA monetary
authority separately requires governed non-forking hardware.
