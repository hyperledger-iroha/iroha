# Status

Reviewed 2026-10-03. Iroha 3 remains under implementation and qualification.
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
| Kagami/Mochi developer experience | Shared native localnet generation, process ownership and workspace contexts are implemented in `iroha_deploy`; Musubi exposes source/artifact/package deployment with exact retained recovery. Installed-runtime fixtures cover config-free startup, all three deployment inputs, restart recovery and four-parent/four-private attachment. | The current shared branch needs combined runtime requalification, including paid provisioning, anchoring and payload isolation. Three-provider generation with shared network policy, distinct-key ingest, aggregate activation, bounded maintenance, independent enrollment restart and automatic custody renewal are implemented but await combined native qualification. Portable publication clock/journal/seed custody, daemon storage integration and authenticated provider-inventory reads are implemented and await native service qualification. Explicit finite startup authorization, bounded unsigned-request recovery and wallet cancellation are implemented but await native interruption qualification. Native pin/outbox submission, publication-service installation and three-provider cold registry publication, official Taira checkpoint publication, native desktop interaction, signed native OS matrices and reference-host latency remain open. See the [developer goals](specs/kagami_mochi_devex_goals.md). |
| Rust client | Immutable account contexts, owned async transport, explicit blocking capabilities and typed fee quoting are implemented. | Remaining capability/consumer migration, unified errors and network cancellation/finality/authorization coverage. |
| Kotlin/JVM | Kotlin owns the SDK, HTTP/SSE/WebSocket, attestation tools and JNI API; Java consumers exercise that API. Host coverage includes native/confidential operations. | Remaining Java/publication retirement, signed packages, CUDA hardware and Android/device qualification. |
| Other SDKs | Shared prepared-operation, signing, account and native checkpoint contracts are being migrated across Swift, JavaScript, Python and C#. | Same-source native artifacts, complete fixtures/consumers and release OS/architecture matrices. |
| Norito | Declared identities own canonical frames; payload serialization/reconstruction and explicit JSON key contracts are integrated. | Consumer/feature closure, fallible allocation ownership, physical model extraction and workspace lint/runtime coverage. |
| IVM/Kotodama | IVM is the sole VM with ABI V1 and program header 1.1; header 1.0 is rejected. Compiler separation and state-free proof owners reduce normal dependency graphs. Source bundles support declaration includes and explicit module exports; authenticated error-message catalogs preserve nominal schemas. | Lifecycle/custody closure, native execution proofs, anchored private invocation/AXT, coherent SDK regeneration and hardware validation. CUDA release builds lack the ten PTX artifacts and signed provenance; plain Cargo daemon defaults omit CUDA. |
| SoraFS | Software signing, canonical manifests and storage/billing/publication ownership are implemented and under repair. | Node failures, matched daemon/harness, provider resilience and L1/L2 promotion. |
| KAGEMUSHA | Single-design G1 canonical objects (`kagemusha_wallet_v1`) and shared Rust/Kotlin/Swift vectors are implemented and tested; no relation, provider or ledger consumes them yet. The empty `iroha_kagemusha_attested`/`iroha_kagemusha_issuer` crates are deleted; results are in the [checklist](specs/kagemusha_evidence_gate.md#8-recorded-results). | Owner decisions on proof size versus R9 and 2 s p95 (measured: one in-circuit P-256 check alone gives a 10,112-byte k=16 proof); stock-OS journal/marker Advance, the single recursive relation, ledger load/unload and device recovery measurements. |
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

The repaired sixteen-target Core/Kagami compilation passes with stock Rust
1.93.1 after fixing the typed proposal-ID fixture. Its immutable native artifacts
are retained; fresh selected controls and the lifetime doctest remain required.
Unrelated document edits fail the wrapper's source guard after Cargo succeeds;
that failure remains separate from the compiler outcome.
The preceding fixed candidate builds all sixteen selected Core/Kagami targets
with stock Rust 1.93.1 and passes the original lifetime compile-fail test. Its
3,520 planned executions end with 3,393 passes, 32 fixture failures and 70 controls
left unexecuted by a default-stack overflow. All 757 selected bridge controls
pass. The reviewed fixture and lifetime repairs are applied; fresh native
validation remains required. Complete Mint/Guard/receiver and genuine proof/export
qualification remain open; full-State capture still rejects its unresolved
runtime-verifier schema. Governed standby retirement has paired native public
fixtures and an exact current inventory of 105 codec owners and 1,555 nominal
identities. New ordinary and reduced-feature checks, SDK consumption and native
publication validation remain required.

An earlier fixed candidate passes all 78 private-dispatch controls, including signed
MEAN, and all seven shared public MEAN controls. It also passes 137 ordinary RFC
controls, both actual ignored RFC auxiliary oracles, and the current X509 profile,
geometry and degree checks. The preceding private namespace records 86 passes and
three fixture failures: two stale opcode inventories and a padding mutation that
left an existing one unchanged. Their repairs are applied. All 144 preceding RFC
controls, five inverse-window controls, two temporal controls and both ignored
RFC auxiliary oracles pass. The complete 210-record
inverse-window experiment preserves every column but runs 1.535 times slower
than the former scalar path, so it remains test-only; the fixed-work successor
needs a fresh comparison.
Two public MEAN controls initially fail resource observation despite native
success; exact repeats pass the unchanged observer, preserving the original failures.
The batch FFT experiment passes its native oracles and emits all 156 measurements;
its corrected parser preserves the original incomplete-inventory failure. A shared
coarse CPU scheduler is applied but still needs integrated native validation.
The first fixed-work inverse retains a compiled private-gate branch. Its reviewed
successor is applied and masks before validation; integrated AArch64 inspection
confirms the helper repair. Surrounding recurrences and whole-prover side-channel
qualification remain open. The repaired inverse-window collector passes all ten
native prerequisites and the complete 210-record parity/resource benchmark; the
sum of per-span medians improves by 1.958 times. Integrated AArch64 inspection
covers that collector. The bounded production route is now selected in source;
fresh production controls and the complete proof still need validation.
All 97 private-dispatch controls pass on the rebuilt artifact. Newly applied
successful LOAD64/STORE64 relations preserve original memory/control/history
ports and reject other running opcodes; their native controls remain pending.
The phase-aware X509 resource repair compiles. Its native producer records 22
passes, four stale assertion failures and one resource-observer failure despite
native success. The numeric and digest fixtures are updated from actual native
emissions, with all limits unchanged. Fresh complete controls and a bounded
maximum proof remain required.
Earlier immutable full-suite processes finish naturally; source changes are
recorded rather than inherited as current-candidate qualification. Complete IVM
execution/finalized-State binding and RAM-LFE encryption remain unavailable.

Maximum FASTPQ ordinary and AXT production plus fresh verifier replays pass on
the preceding fixed candidate, within unchanged proof/payload/work limits. The
available M1 Ultra passes all 52 selected native/SIMD/Metal controls and 13 entropy,
preflight and owned-erasure controls. Conditional adaptive-QROM hiding review now
has explicit public/conditional samplers and conflict accounting; concrete hash,
side-channel, remaining hardware and finalized-source network qualification stay
open.

The recorded script selection passes 718 tests and 163 subtests; typed workflows
pass 58 controls. The preceding Kotlin/JNI run records 1,824 passes and one stale
bootstrap fixture failure among 1,825 tests, including six passing maximum-tree
controls. Android client managed tests record 421 passes and one signing-vector
fixture failure; both corrections are applied. Actual Android client host JNI
passes 95 controls, wallet host JNI passes one, and wallet managed tests pass 91.
JVM tools pass 30 controls and the native example passes. The compiled JNI census
also rejects four Android-only methods absent from the Darwin library; correct
target-specific native coverage remains to be established. The preceding host
library loads as ABI-25 with all 120 required symbols; all ten genuine generator
modes pass twice with identical paired outputs. Paired Kagami, quantity, IVM and
public-codec producers and selected consumers pass on their recorded source.
The possession-journal directory and Swift test-endpoint conformance repairs are
applied. Fresh complete consumers, Apple packaging, full Swift host tests,
physical devices, the complete Cash-owner lifecycle and signed releases remain open.

The preceding whole-workspace all-target check ends with six Torii test API
errors; strict all-target Clippy finds 43 diagnostics. Their reviewed corrections
are applied and need native replay. The daemon, CLI and three integration
harnesses build. Four-validator transaction commitment passes. The lane restart
scenario stalls after lane height 1 commits while global height remains 3; an
unchanged-source repeat reproduces it. The assembler now includes a nonempty
merge section's time floor before its first build, avoiding duplicate parent
authentication while retaining canonical-time validation. Native wire-parity and
build-count controls plus the original network scenario remain required.
FASTPQ and wallet network scenarios fail
before their proof assertions on asset-domain and initial-executor admission;
reviewed repairs preserve proof checks and need fresh network validation.
The merge is committed as `ab6d83b7216f4e8ec00b29a31baeed0637929600` on
`optimizations`. It changed checkout identity during the producer's terminal
check without changing source bytes; that failed validation remains recorded.
The authored OpenAPI copies now include the same retirement schema and require
fresh source guards and native route checks. The [ZK goals](specs/zk_first_release_goals.md)
distinguish recorded outcomes from the follow-up repairs' pending validation.

KAGEMUSHA component coverage includes journal recovery, issuer-key and
enrollment-floor controls, expired-preparation custody, Guard-generation and
reciprocal claim-carrier binding. Primary-owner Privacy library strict lint passes
with defaults, `privacy-release-evidence` and
`privacy-release-evidence,test-utils` in an unchanged source/Git interval.
The recorded debug-profile Privacy libtest build and all 22 selected controls
pass. Its registry contains 2,479 tests, including 61 ignored cases; independent
framing of all 29 native profile fields matches the recorded source pin. Genuine
IO, Projection and CA component proof checks pass once each sequentially at the
closed merged revision, with all 475 retained inputs unchanged through the runtime
endpoints. The CA check binds a synthetic paired MAIN record. The optimized
constructor, full ordinary suite, upstream strict lint and merged-source qualification
remain open. Maximum-proof external time/RSS evidence and enforced address-space
limits remain separate cryptographic release gates. The non-test real-proof frontend
remains under qualification. Later Privacy source additions have a separate
development libtest build and registry discovery (2,541 tests, 65 ignored,
433 dep-info inputs). After the shared Norito derive correction, the current
debug-profile libtest compiles with all 8,374 captured source, literal and control
inputs unchanged at the closed merged revision. Its actual registry preserves
all 61 original ignores; the 2,476 ordinary cases are running serially after a
current resource review. The derive library, strict JSON and UI regressions pass;
full runtime validation remains open and retains its artifact's source epoch
across the separate test-fixture repair.
SDK Native custody and genuine production proving are mandatory even with SDK
defaults disabled; assembly tools remain explicit
`dev-tools` targets, and FASTPQ uses the existing STARK feature. Current exact
CoreZK/Halo2 SDK and downstream owner boundaries and feature hygiene pass.
Source budgets include the exact filesystem-lock, Petal, URL, wallet-manifest
and IVM test-owner costs without growth headroom or relaxed ownership denials;
current native frontend and proof qualification remain open.
The signed-clock type path, Core ZK URL dependency and shared request-codec
derive are corrected. The normal Core ZK library frontend passes with its default
features and an unchanged source/Git interval. The repair chat installed the
ordinary-mint submission-limit import and checkpoint genesis-error conversion
that blocked the fee-target build; a fresh build and fee runtime remain pending.
The current metadata sequence stops at its first build on three wallet custody
types missing required canonical Norito schema names. Its source/Git interval
is unchanged; the second build and strict freshness comparison have not run.

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
  side-channel qualification remain open. Maximum q77 ordinary/AXT proofs and local
  M1 Ultra controls pass on their recorded candidate; finalized-source, remaining
  hardware and network qualification remain. RAM-LFE secure encryption/full execution, IVM native
  execution/finalized-State binding and signed release evidence remain open under
  the [ZK goals](specs/zk_first_release_goals.md).

- **Services:** Musubi publication/paid contracts, Parliament/standalone elections,
  SoraNet/Linux helpers, SCCP live corridors and Inrou Linux/AArch64/KVM isolation
  remain unqualified.
- **Offline money/devices:** the [single KAGEMUSHA target](specs/kagemusha_single_design_proposal.md)
  uses a released wallet on a stock uncompromised OS, hardware-backed keys,
  a durable software provider and recursive proofs. Its Send is irreversible;
  recovery replays exact Payment bytes to the receiver, with no refund path.
  Existing ordinary code still has per-operation online-control dependencies.
  The G1 canonical objects exist as a data-model module with cross-language
  vectors. Measured Pasta IPA proof sizes (≈ 358 bytes per advice column at
  k=16) mean a 10,000-byte Payment needs a narrow large-k outer proof layer;
  that architecture and the 2 s target await owner decisions. Complete proofs,
  provider integration, mint/redemption and device recovery measurements remain open.
  The [checklist](specs/kagemusha_evidence_gate.md) records verification without
  an approval gate; software markers claim no protection against OS takeover.

First-release contracts remain canonical APIs, domainless `AccountId`, Norito
wire formats and deterministic ABI V1. Sumeragi uses exact `3f + 1` global
committees, exactly `n - f` votes, signed RS16 availability and work-driven blocks.
Ordinary signing supports authenticated software custody. The first production
KAGEMUSHA target requires platform enrollment, genuine monetary proofs and
durable local state/replay authority under its stock-OS assumption; it has no
custom applet or one-use-key prerequisite.
