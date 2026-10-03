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
| Kagami/Mochi developer experience | Shared native localnet generation, process ownership and workspace contexts are implemented in `iroha_deploy`; Musubi exposes source/artifact/package deployment with exact retained recovery. Installed-runtime fixtures cover config-free startup, all three deployment inputs, restart recovery and four-parent/four-private attachment. | The current shared branch needs combined runtime requalification, including paid provisioning, anchoring and payload isolation. Cold registry dependencies, official Taira checkpoint publication, native desktop interaction, signed native OS matrices and reference-host latency remain open. See the [developer goals](specs/kagami_mochi_devex_goals.md). |
| Rust client | Immutable account contexts, owned async transport, explicit blocking capabilities and typed fee quoting are implemented. | Remaining capability/consumer migration, unified errors and network cancellation/finality/authorization coverage. |
| Kotlin/JVM | Kotlin owns the SDK, HTTP/SSE/WebSocket, attestation tools and JNI API; Java consumers exercise that API. Host coverage includes native/confidential operations. | Remaining Java/publication retirement, signed packages, CUDA hardware and Android/device qualification. |
| Other SDKs | Shared prepared-operation, signing, account and native checkpoint contracts are being migrated across Swift, JavaScript, Python and C#. | Same-source native artifacts, complete fixtures/consumers and release OS/architecture matrices. |
| Norito | Declared identities own canonical frames; payload serialization/reconstruction and explicit JSON key contracts are integrated. | Consumer/feature closure, fallible allocation ownership, physical model extraction and workspace lint/runtime coverage. |
| IVM/Kotodama | IVM is the sole VM with ABI V1. Compiler separation and state-free proof owners reduce normal dependency graphs. Source bundles support declaration includes and explicit module exports; authenticated error-message catalogs preserve nominal schemas. | Lifecycle/custody closure, native execution proofs, anchored private invocation/AXT, coherent SDK regeneration and hardware validation. |
| SoraFS | Software signing, canonical manifests and storage/billing/publication ownership are implemented and under repair. | Node failures, matched daemon/harness, provider resilience and L1/L2 promotion. |
| KAGEMUSHA | Ordinary app-owned hardware admission, Native clock/current-wallet reads and approval custody have component implementations; the current cash/Guard production library compiles. | Stock-OS journal/marker Advance, complete recursive monetary proofs, funded State transitions, ledger load/unload and device recovery measurements. |
| Petal Stream | `iroha_petal` implements the [Petal Stream](specs/petal_stream.md) animated optical transport (`天` orientation field, katakana, tile polarity and ring dots as three Reed–Solomon lanes under a rateless fountain) with decoder, renderer, camera simulator and `iroha offline petal`. A gain-free tile read keeps lanes `P` and `K` alive under over-exposure, veiling light and shadows. Swift, Kotlin/JVM, JavaScript, Python and C# ports reproduce the shared fixtures and decode the golden captures to the recorded lanes. Three corner blossoms suffice (a thumb, glare or frame edge may hide the fourth) and sessions track the pose between frames (about 4× cheaper per frame). Simulated reads complete a 10,000-byte KAGEMUSHA message in 11.5 s (45 s from lanes `P` and `D` alone). | All evidence is simulated: physical-camera reads on the governed Android/iOS device matrix, lane `K` at 480p or soft focus, and the public guides in `iroha-docs`. |

## Immediate blockers

The proxied pipeline-status JSON accounting mismatch is repaired. The preceding
integrated candidate passed repeated paid settlements, sixteen-peer settlement with
restart, and disjoint lane isolation with restart. The latest paid 4→7→4 run certified
the return boundary at height 72 after all-peer restart, but the next paid transaction
remained queued and height 73 never applied. The transition, signed RS16
loss/withholding and release qualification remain open. Combined Core component
checks pass; the repaired Rust SDK passes all 954 unit controls and strict lint.
Core strict lint and fresh qualification of the merged source remain open. Exact
finalized-carrier retries retain original execution without requeueing or charging
again; changed source requires fresh evidence.

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

The current `optimizations` checkout has merged HEAD
`c0f2be96c6e15f773f23e916ab574a59c6806fb8`, subsequent repairs and concurrent
Petal changes. The seventeen-target Core/Kagami build and stock Rust 1.93.1
whole-workspace all-target check pass on their recorded inputs. They do not
establish one unchanged release candidate. Exact component evidence and remaining
criteria are maintained in the [ZK goals](specs/zk_first_release_goals.md).

The rebuilt Core artifact passes all five prior stack failures on the default
stack, all 72 selected certified-chain controls and all 246 still-existing controls
from the original 247-test request. Its retired control now has three passing
genuine producer/consumer replacements, including paired captures and bridge
verification. The original selection has no uncovered obligation. The complete rebuilt 66-control
group04 run now passes after seeding the initial fee-owning domain and regenerating
the canonical pin fixture with its genuine producer. The changed allocation-credit
control also passes. The repaired CoreZK libtest compiles and all eight bootstrap,
canonical-stream and shared-capacity controls pass. Its initial compiler-input
audit refusal is retained; a supplemental audit validates the actual IVM sample
inputs against every original compiler observation. The lifetime compile-fail test, Primitives 333, Norito derive
59 unit and 17 strict-JSON controls, and 32 compiler cases pass.
Complete Mint/Guard/receiver, genuine proof/export and full-State authority remain
open. The current inventory preserves 105 codec owners and 1,555 nominal
identities; publication and reduced-feature qualification remain required.

The fresh Linux and Mac optimized privacy artifacts pass all 133 private-dispatch
controls, including all eight direct-jump and eleven CALL descriptor/frame-work
controls, with zero failures or skips. Earlier RFC, FFT, nonce and auxiliary
oracle passes remain scoped to their recorded artifacts. The applied DER/RFC
canonical-zero inversion repairs and expanded public phase counters require fresh
native tests and compiled-caller review. Complete current ordinary privacy
coverage, IVM execution, initialization and finalized-State binding remain open.
RAM-LFE secure encryption, refresh and the full program relation remain unavailable.

The latest Linux maximum X509 proof is 9,412,944 bytes. Producer and independent
verification, wrong-genesis, corruption and all 32 nonce-byte substitutions pass;
a separate fresh verifier passes in 8.674104 seconds. Conservative reported RSS
is 7,147,257,856 bytes and sampled address space reaches 8,668,545,024 bytes,
within the unchanged 12 GiB RSS and enforced 32 GiB address-space caps. These
observations do not measure an exact physical or address-space peak. Proving
takes 1,494.890315 seconds against 300, so the complete run fails qualification.
The applied compact P-256, shared FFT, masked-coefficient and exact-capacity DEEP
changes complete this proof. The repaired Metal maximum also produces and verifies the complete proof,
including a fresh verifier and all nonce mutations. Its 1,061.052853-second
proving time still exceeds 300; Darwin cannot enforce the 32 GiB address-space
limit and its sampled literal address space exceeds it. The retained pre-CALL privacy artifact
has 2,028 of 2,588 ordinary controls pending; those counts do not describe the
current source's expanded census. Relation, adaptive transcript/hiding and
whole-prover side-channel review remain open. Busy-host timings are not a
quiet-host speedup claim; no cap or supported shape is relaxed.

Maximum FASTPQ ordinary and AXT production and fresh verifier replays pass on
their recorded candidate. Current hardware controls pass 57 tests, including all
nine mandatory Metal controls on the M1 Ultra. The four-validator finalized
transcript scenario passes after the exact authenticated query repair, preserving
contiguous genesis-rooted finality, source substitutions and restart checks.
This establishes transcript custody; complete q77/D7 source/spend admission,
concrete cryptographic qualification and other hardware remain open.

Recorded script, Kotlin/Android, Swift, JavaScript and native fixture controls
provide component coverage. Fresh canonical generation, complete SDK consumers,
strict lint, workspace tests and four-validator recovery still require one
integrated candidate. Physical-device, cryptographic/side-channel and signed
release qualification remain open. The [ZK goals](specs/zk_first_release_goals.md)
retain the current acceptance boundaries.

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
- **Privacy/crypto:** the current joint X509 maximum proof passes native and fresh
  verification, proof size, conservative RSS and enforced address space. The
  300-second limit fails. Complete relation, adaptive transcript/hiding and
  side-channel qualification remain open. Maximum q77 ordinary/AXT proofs, local
  M1 Ultra controls and four-validator transcript custody pass on their recorded
  candidates; complete source/spend admission and other hardware remain open.
  RAM-LFE secure encryption/full execution, IVM native
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
  Complete proofs, provider
  integration, mint/redemption and device recovery measurements remain open.
  The [checklist](specs/kagemusha_evidence_gate.md) records verification without
  an approval gate; software markers claim no protection against OS takeover.

First-release contracts remain canonical APIs, domainless `AccountId`, Norito
wire formats and deterministic ABI V1. Sumeragi uses exact `3f + 1` global
committees, exactly `n - f` votes, signed RS16 availability and work-driven blocks.
Ordinary signing supports authenticated software custody. The first production
KAGEMUSHA target requires platform enrollment, genuine monetary proofs and
durable local state/replay authority under its stock-OS assumption; it has no
custom applet or one-use-key prerequisite.
