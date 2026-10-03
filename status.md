# Status

Reviewed 2026-10-02. Iroha 3 remains under implementation and qualification.
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
| Kagami/Mochi developer experience | Shared native localnet generation, process ownership and workspace contexts are implemented in `iroha_deploy`; Musubi exposes source/artifact/package deployment with exact retained recovery. Installed-runtime fixtures cover config-free startup, all three deployment inputs, restart recovery and four-parent/four-private attachment. | The current shared branch needs combined runtime requalification, including paid provisioning, anchoring and payload isolation. Cold registry dependencies, official Taira checkpoint publication, native desktop interaction, signed native OS matrices and reference-host latency remain open. See the [developer goals](specs/kagami_mochi_devex_goals.md). |
| Rust client | Immutable account contexts, owned async transport, explicit blocking capabilities and typed fee quoting are implemented. | Remaining capability/consumer migration, unified errors and network cancellation/finality/authorization coverage. |
| Kotlin/JVM | Kotlin owns the SDK, HTTP/SSE/WebSocket, attestation tools and JNI API; Java consumers exercise that API. Host coverage includes native/confidential operations. | Remaining Java/publication retirement, signed packages, CUDA hardware and Android/device qualification. |
| Other SDKs | Shared prepared-operation, signing, account and native checkpoint contracts are being migrated across Swift, JavaScript, Python and C#. | Same-source native artifacts, complete fixtures/consumers and release OS/architecture matrices. |
| Norito | Declared identities own canonical frames; payload serialization/reconstruction and explicit JSON key contracts are integrated. | Consumer/feature closure, fallible allocation ownership, physical model extraction and workspace lint/runtime coverage. |
| IVM/Kotodama | IVM is the sole VM with ABI V1. Compiler separation and state-free proof owners reduce normal dependency graphs. Source bundles support declaration includes and explicit module exports; authenticated error-message catalogs preserve nominal schemas. | Lifecycle/custody closure, native execution proofs, anchored private invocation/AXT, coherent SDK regeneration and hardware validation. |
| SoraFS | Software signing, canonical manifests and storage/billing/publication ownership are implemented and under repair. | Node failures, matched daemon/harness, provider resilience and L1/L2 promotion. |
| KAGEMUSHA | Ordinary app-owned hardware admission, Native clock/current-wallet reads and approval custody have component implementations; the current cash/Guard production library compiles. | Current-owner financial control, complete recursive monetary proofs, funded State transitions, settlement, recovery and physical-device evidence. |
| Petal Stream | `iroha_petal` implements the [Petal Stream](specs/petal_stream.md) animated optical transport (`天` orientation field, katakana, tile polarity and ring dots as three Reed–Solomon lanes under a rateless fountain) with decoder, renderer, camera simulator and `iroha offline petal`. A gain-free tile read keeps lanes `P` and `K` alive under over-exposure, veiling light and shadows. Swift, Kotlin/JVM, JavaScript, Python and C# ports reproduce the shared fixtures and decode the golden captures to the recorded lanes. Simulated reads complete the largest 7,552-byte payment in 8.6 s (34 s from lanes `P` and `D` alone). | All evidence is simulated: physical-camera reads on the governed Android/iOS device matrix, lane `K` at 480p or soft focus, field tuning (three-finger fallback, pose tracking) and the public guides in `iroha-docs`. |

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

Native stream-token gateway admission combines protected consensus quotas, leases and
callback history with challenged readbacks, daemon software custody and a final serving
publication fence. Broker-supplied gateway authority and the generic local token-reputation
producer are retired. Native admissions own exact reputation append intents; certified current
readbacks and committed terminal dispositions govern callback completion. The token issuer uses
one bounded private receipt journal over shared Unix/Windows filesystem custody;
native Windows execution and release qualification remain open. Combined candidate
validation, native service closure and multi-replica recovery remain open. Component
coverage does not qualify cold registry fetches, the complete 64 MiB fetch-process RSS
bound, signed native releases or reference-host p95 latency.

The private-settlement five-second proof-through-settlement target is unmet.
Current-source repeated settlement, fault and leakage campaigns are unqualified;
prior component timings and network observations do not qualify changed source.
See the [protocol](specs/private_settlement.md).

## Deployment state

Taira's four validators run the a2a98f02 daemon build with fresh keys and signed
genesis. The genesis beacon ceremony resolved to StateApplied at height 5.
All four completed sequential C2 restarts with their keys and ledger retained.
Latest direct checks verified each active process, current selector and C2
binary, with height 8, three peers and readiness HTTP 200. Public TLS,
readiness and status are healthy at a2a98f02, height 8, three peers, twenty-one
approved transactions, zero rejected transactions and an empty queue. The
same-revision native basic doctor passed all fifteen checks with no failures.
The previous live ledger and twenty obsolete validator releases were deleted.

Current source initializes fresh safety records before first startup and retires
completed execution after successful replay before strict native archive
attachment. Deployment preparation, transfer and the routine updater accept an
authenticated build-only candidate without a full regression gate. On-chain
governance owns deployment policy; no fixed 24-hour fault test is a prerequisite
for testnet or production. Release qualification remains open.

Fresh-account funding applied at height 6; ordinary paid public pings applied
at heights 7 and 8, with exactly one C2 ping submission. Native read-only
funding resume now returns Applied with exact committed-transaction readback
at height 6. The live endpoint objective is achieved; broader native release
qualification and original public-reset coordinator completion remain separate.
Mac outbound-port exhaustion was recovered, with the ephemeral-port setting
and scoped SYN guard persisted. The temporary artifact server was stopped.
Physical DPN, paid `dpn`/`admin@dpn` and clean-client completion remain open.
Validators run in a Linux guest on MacStadium in Dublin; use the approved
deployment tooling. Retained incident records describe the
[previous readiness failure](docs/incidents/2026-09-30-taira-readiness.md).


BPNG retained-history qualification, validator catch-up, additive catalog
activation and API22/FE17 application commissioning remain open. Basic acceptance
does not close crash/concurrency or advanced guest work. Infrastructure changes
require the explicitly approved OVH target.

## Build and release qualification

The `d05dc9c1` base and reviewed merge repairs build with stock Rust
1.93.1. On source17, normal privacy and optimized native test builds pass on
macOS and Linux, including all three authentic profile/proof pins. All 1,128
ordinary privacy controls pass on macOS; Linux passes those plus its required
full-domain CPU parity control. The 35 focused query/CPU/Metal controls also pass.

The joint MAIN/CA maximum proof and fresh verifier pass on that Linux candidate,
including wrong-genesis and tampering controls. The 9,412,912-byte proof fits its
cap, and the actual child stays under the kernel-enforced 32 GiB address-space
limit. Proving takes 2,275.987086 seconds against 300 seconds. Both live and
terminal RSS readings are below 12 GiB, but their 659,456-byte discrepancy fails
the observer's consistency check; RSS qualification remains unresolved. Query
transforms take 639.87 seconds and are the largest measured performance target.
The retained run is `dist/zk-remediation/2026-09-30/epoch19-linux-source17-maximum`.
Activation remains unavailable.

Fresh normal Rust compilation now emits all twelve selected Core/Kagami/IVM/
storage/FASTPQ test harnesses. The actual host bridge passes ABI-25, loaded-image
and required-symbol checks; genuine quantity and IVM captures match across paired
runs. Fresh issuer-key and quota-ownership regressions pass, including all 109
FASTPQ source-inventory controls. The broader selection remains incomplete.
Mint tests exposed a production mismatch between the 327-byte canonical envelope
and the 384-byte transport maximum; the model and recursive-consumer repairs are
applied. The remaining 66 groups execute 2,870 passes and five known fixture
failures with no skips. The fresh rerun passes the available CoreZk controls,
both private-export fee fixtures and mint geometry. It exposes two proof-helper
lookup configurations, an ineffective X25519 entropy mutation and an allocating
provider-validation length check; reviewed follow-up repairs are applied. Both
complete active/inactive mint binding controls pass, but the full mint relation
exceeds the unchanged production SHA row capacity at K=16. The Musubi original-State
source and final three semantic table readers are applied; native compilation
exposes a read-trait mismatch and test callback error, both repaired. The next
catalog enum-size lint is repaired; native rerunning waits for the separately
owned merge to finish. Full-State capture still rejects its unresolved verifier schema.
All nine crypto admission controls pass, including 44 fresh-process checks across
all 11 algorithms with no Rust allocation requests. Ongoing unrelated
edits mean these are artifact diagnostics,
not qualification of one fixed source candidate.

Fresh Linux privacy builds pass. All 54 private-dispatch controls and the four
full-domain X509 oracles pass. The obsolete composite-test expectation is repaired
and its rerun passes. The five-channel SPKI source repair passes all six native
controls. All eight TBS/CRL/signature controls and complete source-lookup replay
pass after the length-endianness correction. All six serial controls pass;
all five selected-disclosure controls pass. Its full RFC namespace has 119 passes
and four stale profile/descriptor/histogram failures. Complete Name OID census and
uniqueness are applied but await native validation; full value/string policy
remains open. Corrected private multiply fixtures now
pass all four controls. All eight public bit-count controls pass, including the
maximum proof; the run's late merge-state guard fails and that overall failure
is retained. Conditional moves await native validation. The zero-suffix FFT repair passes native parity, but its measured
large-domain gains do not establish the complete proof's time target.
Current complete proof, resource and cryptographic qualification remain unavailable.

The exact native export inventory is repaired and SDK source contracts pass.
The latest Apple retry stops after three compiled slices because Cargo.lock
changed; it publishes no package. Authentic Kotlin fixtures are refreshed; the
isolated core run passes all 1,766 cases. Exact report reconciliation and the
remaining diagnostic now pass: tooling, Android managed and actual host-JNI
consumers, plus the real redemption example. Four Android test files needed a
JUnit 5 import correction. Full Swift packaging and physical devices remain pending. Fifty local FASTPQ hardware controls pass, including
required Metal and NEON cleanup; the full hardware and side-channel matrix is open.
Vega timing requires an uncontended host and remains unqualified.

Fresh canonical fixture generation, native SDK consumers, Swift/Android packages,
workspace checks and four-validator tests still require one integrated candidate.
Fresh network artifacts compile, but all four selected scenarios fail Torii
startup on an alias-index authentication-marker mismatch. The correction preserves
the existing required signature policy and passes all ten native alias-route
controls. The next daemon build finds a missing type qualification in account
removal; its one-line repair is applied. Fresh daemon/harness and four-validator
reruns remain required.
Complete cryptographic/side-channel, physical-device and signed release evidence
remain open. The [ZK goals](specs/zk_first_release_goals.md) retain exact boundaries.

KAGEMUSHA component coverage includes journal recovery, issuer-key and
enrollment-floor controls, expired-preparation custody, Guard-generation and
reciprocal claim-carrier binding. Primary-owner Privacy library strict lint passes
with defaults, `privacy-release-evidence` and
`privacy-release-evidence,test-utils` in an unchanged source/Git interval.
The current debug-profile Privacy libtest build and all 22 selected controls
pass. Its registry contains 2,479 tests, including 61 ignored cases; independent
framing of all 29 native profile fields matches the current source pin. Genuine
IO, Projection and CA proof checks, the optimized constructor and maximum proof
with external time/RSS evidence, the full ordinary suite, upstream strict lint
and merged-source qualification remain open. The non-test real-proof frontend
remains under qualification. SDK Native custody and genuine production proving
are mandatory even with SDK defaults disabled; assembly tools remain explicit
`dev-tools` targets, and FASTPQ uses the existing STARK feature. Current exact
CoreZK/Halo2 SDK and downstream owner boundaries and feature hygiene pass.
Current source budgets pass with the updated developer-tool manifest fingerprint;
current native frontend and proof qualification remain open.

Ordinary recursive credential generation is blocked by the Eq circuit requiring
8,584 advice columns against the 1,024-column limit. The complete-circuit
preflight and compact authenticated carrier layout still need fresh compiler,
layout, parity and proof qualification.


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
- **Privacy/crypto:** the source17 joint X509 maximum proof passes native and fresh
  verification, proof size and enforced address space. The 300-second limit fails;
  RSS readings need observer reconciliation. Complete relation, adaptive transcript/hiding and
  side-channel qualification remain open. Historical q77 ordinary/AXT maximum
  proofs pass on their recorded source; finalized-source, hardware and network
  qualification remain. RAM-LFE secure encryption/full execution, IVM native
  execution/finalized-State binding and signed release evidence remain open under
  the [ZK goals](specs/zk_first_release_goals.md).

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
