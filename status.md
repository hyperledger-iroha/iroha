# Status

Reviewed 2026-10-04. Iroha 3 remains under implementation and qualification.
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
| Kagami/Mochi developer experience | Shared native localnet generation, process ownership and workspace contexts are implemented in `iroha_deploy`; Musubi exposes source/artifact/package deployment with exact retained recovery. Installed-runtime fixtures cover config-free startup, all three deployment inputs, restart recovery and four-parent/four-private attachment. | The current shared branch needs combined runtime requalification, including paid provisioning, anchoring and payload isolation. Three-provider generation with shared network policy, distinct-key ingest, aggregate activation, bounded maintenance, independent enrollment restart and automatic custody renewal are implemented but await combined native qualification. Portable publication clock/journal/seed custody, daemon storage integration and authenticated provider-inventory reads are implemented and await native service qualification. Explicit finite startup authorization, bounded unsigned-request recovery and wallet cancellation are implemented but await native interruption qualification. Native paid pin coordination, publisher acquisition of complete signed provider inventories and generated publication TLS selection are implemented and await combined native qualification. Stock publication-service selection, generated runtime configuration, the explicit generated publication API, Kagami publication command and Mochi Packages view are implemented; their current native serving path and frontend handoff await qualification. Three-provider cold registry publication, the approved committed Taira installation profile and recurring signed checkpoint publication, native desktop interaction, signed native OS matrices and reference-host latency remain open. Release bundling refuses without that approved public profile. See the [developer goals](specs/kagami_mochi_devex_goals.md). |
| Rust client | Immutable account contexts, owned async transport, explicit blocking capabilities and typed fee quoting are implemented. | Remaining capability/consumer migration, unified errors and network cancellation/finality/authorization coverage. |
| Kotlin/JVM | Kotlin owns the SDK, HTTP/SSE/WebSocket, attestation tools and JNI API; Java consumers exercise that API. Host coverage includes native/confidential operations. Android Keystore alias existence is decided only by keystore2 `getKey` (API 31+); a Keystore error is never read as absence. | Remaining Java/publication retirement, signed packages, CUDA hardware and Android/device qualification. |
| Other SDKs | Shared prepared-operation, signing, account and native checkpoint contracts are being migrated across Swift, JavaScript, Python and C#. | Same-source native artifacts, complete fixtures/consumers and release OS/architecture matrices. |
| Torii collection queries | Domains, accounts, asset definitions, NFTs, RWA lots, balances, holders, transactions, account transactions and repo agreements share one [query language](specs/torii/collection_queries.md) and engine: text/JSON filters, sort, select, aggregates, keyset or block-coordinate cursors and optional totals, merged across dataspace routes. The Rust, Kotlin/Java, Swift, JavaScript, Python and C# SDKs and the CLI implement it against shared golden vectors. | Aggregates spanning several dataspace routes are rejected; explorer and history feeds keep their own paging; no live multi-peer qualification yet. |
| Norito | Declared identities own canonical frames; payload serialization/reconstruction and explicit JSON key contracts are integrated. | Consumer/feature closure, fallible allocation ownership, physical model extraction and workspace lint/runtime coverage. |
| IVM/Kotodama | IVM is the sole VM with ABI V1 and program header 1.1; header 1.0 is rejected. Compiler separation and state-free proof owners reduce normal dependency graphs. Source bundles support declaration includes and explicit module exports; authenticated error-message catalogs preserve nominal schemas. FASTPQ Metal uses one embedded-bundle admission owner; runtime source/path loading is removed. Ordinary Linux/Windows daemon dependencies include driver-loaded IVM CUDA; genuine bundle absence permits CPU build/startup, and supplied unapproved material is rejected. | Lifecycle/custody closure, native execution proofs, anchored private invocation/AXT, coherent SDK regeneration and hardware validation. FASTPQ has no approved Metal bundle. IVM CUDA approval is `None`; genuine signed ten-family artifacts, driverless runtime checks and physical qualification remain open. |
| SoraFS | Software signing, canonical manifests and storage/billing/publication ownership are implemented; the ordinary Node library tests pass. | Matched daemon/harness, provider resilience and L1/L2 promotion. |
| KAGEMUSHA | Single-design G1 canonical objects (`kagemusha_wallet_v1`) and shared Rust/Kotlin/Swift vectors are implemented and tested; no relation, provider or ledger consumes them yet. The empty `iroha_kagemusha_attested`/`iroha_kagemusha_issuer` crates are deleted; results are in the [checklist](specs/kagemusha_evidence_gate.md#8-recorded-results). | Owner decisions on proof size versus R9 and 2 s p95 (measured: one in-circuit P-256 check alone gives a 10,112-byte k=16 proof); stock-OS journal/marker Advance, the single recursive relation, ledger load/unload and device recovery measurements. |
| Petal Stream | `iroha_petal` implements the [Petal Stream](specs/petal_stream.md) animated optical transport (`天` orientation field, katakana, tile polarity and ring dots as three Reed–Solomon lanes under a rateless fountain) with decoder, renderer, camera simulator and `iroha offline petal`. A gain-free tile read keeps lanes `P` and `K` alive under over-exposure, veiling light and shadows. Swift, Kotlin/JVM, JavaScript, Python and C# ports reproduce the shared fixtures and decode the golden captures to the recorded lanes. Three corner blossoms suffice (a thumb, glare or frame edge may hide the fourth) and sessions track the pose between frames (about 4× cheaper per frame). Simulated reads complete a 10,000-byte KAGEMUSHA message in 11.5 s (45 s from lanes `P` and `D` alone). | All evidence is simulated: physical-camera reads on the governed Android/iOS device matrix, lane `K` at 480p or soft focus, and the public guides in `iroha-docs`. |

## Immediate blockers

The proxied pipeline-status JSON accounting mismatch is repaired. Scoped node runs
have passed repeated paid settlements, sixteen-peer settlement with restart and
disjoint lane isolation with restart. The latest paid 4→7→4 attempt halted after
restart at height 66 when an ordinary State publication change was classified as
terminal recovery. Its source-authentication retry repair passes component controls;
fresh daemon qualification remains open. The earlier queued paid transaction after
the certified return boundary at height 72 remains a required liveness regression.
The runtime and trigger permission guards now reject malformed recognized payloads
before delegation; decoder refusals preserve local deferral. Core strict lint still
fails on unused production/resource graphs. The repaired Rust SDK client source
passes its unit suite and strict lint; qualification of the final combined source,
signed RS16 loss/withholding and release remains open. Exact finalized-carrier retries
retain original execution without requeueing or charging again; changed source
requires fresh evidence.

Core/World acquisition retains original frozen readers for verifier, proof-status
and validation-fee indexes. Complete authenticated State/Kura publication remains
unimplemented. The combined test graph, complete resource funding and original
Validate-to-Apply custody remain open. Component repairs do not establish one
retained execution through finality and restart.

The ordinary SoraFS Node library tests pass after correcting fixture permissions,
hedged-encryption assertions and cumulative quarantine decoding. The Core
selection has scoped coverage and Torii's unit-test target compiles. Production
qualification remains open under the
[reliability goals](specs/sorafs/first_release_reliability_goals.md) and
[closure ledger](specs/sorafs/v1_closure_ledger.md).

Native stream-token gateway admission combines protected consensus quotas, leases and
callback history with challenged readbacks, daemon software custody and a final serving
publication fence. Broker-supplied gateway authority and the generic local token-reputation
producer are retired. Native admissions own exact reputation append intents; certified current
readbacks and committed terminal dispositions govern callback completion. The token issuer uses
one bounded private receipt journal over shared Unix/Windows filesystem custody.
Broker observation continuations retain the original reply, socket, admission and
file-configured absolute deadline through publication; combined runtime qualification,
physical allocation ownership and durable recovery remain open. Native Windows
execution and release qualification remain open. Combined candidate
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
`68d8bb58ff4af339aba2229f18da3ed00d58d2e6` and subsequent reviewed repairs.
The latest eighteen-target Core/Kagami build and all 262 selected native
executions pass with unchanged source and artifacts. They account for all original
247 obligations through genuine producer/consumer replacements and paired
identical captures, plus the corrected chronology and FASTPQ context controls.
The genuine producer reproduces all four canonical fixtures with actual genesis
and sequential/parallel parity. Both Mac and Linux scalar-CALL, callable, frame
and history selections pass all 61 ordinary controls on their recorded artifacts;
two heavy frame controls remain unexecuted. The locked, offline workspace all-target check passes on the combined X509,
fixture and arithmetic candidate. All 5,005 affected native tests pass: 35 artifact
admission, 4,633 data-model, two allocation-observer and 335 Primitives controls.
The original harness failure from 17 standard `should panic` annotations remains
retained; independent reconciliation verifies the exact actual native events.
The recorded strict Clippy run still fails on decoder visibility, unused CoreZK
items and one test pattern. Reviewed visibility, projection and receipt-method
repairs are now applied; fresh compiler and native validation remain required. These component
results do not qualify the integrated release. Exact evidence and remaining criteria are in the
[ZK goals](specs/zk_first_release_goals.md).

The X509 padding, private-u32 path-length, mandatory copy-census and canonical
EKU repairs are applied. Fresh Mac and Linux artifacts pass all 218 selected
controls, including all 29 profile fields, signed capacity cases and copy-omission
regressions, with unchanged measured source and artifacts. Authentic proof hashes
match across platforms. The standalone DER proof is 1,527,952 bytes and takes
268.12 seconds on Mac and 268.73 seconds on Linux; both verifier and mutation
suites pass. Its test-only format exposes terminal products and does not
establish complete-credential hiding. The complete maximum result below still
misses the proving-time gate; integrated and cryptographic qualification remain open.

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
open. The current inventory has 106 codec owners and 1,559 nominal identities,
preserving every prior owner; publication and reduced-feature qualification
remain required.

The fresh Linux and Mac optimized privacy artifacts pass all 133 private-dispatch
controls, including all eight direct-jump and eleven CALL descriptor/frame-work
controls, with zero failures or skips. Earlier RFC, FFT, nonce and auxiliary
oracle passes remain scoped to their recorded artifacts. The applied DER/RFC
canonical-zero inversion repairs and expanded public phase counters require fresh
native tests and compiled-caller review. Complete current ordinary privacy
coverage, IVM execution, initialization and finalized-State binding remain open.
RAM-LFE secure encryption, refresh and the full program relation remain unavailable.

The latest Linux maximum X509 proof on the corrected profile is 9,412,944 bytes.
Producer self-check, independent replay, wrong-genesis, corruption and all 32
nonce-byte substitutions pass; a separate fresh public verifier takes 7.977963
seconds. Conservative reported RSS is 7,163,871,232 bytes, within the unchanged
12 GiB cap, and the kernel-enforced hard/soft 32 GiB address-space gate passes.
These observations do not establish an exact physical or address-space peak.
Proving takes 1,437.185689 seconds against 300, so the complete run fails its
performance gate. The retained result and all 65 phase timers are source-bound;
later changes require fresh validation. The preceding Metal maximum also verified
the complete proof but failed its time and Darwin address-space gates. Relation,
adaptive transcript/hiding and whole-prover side-channel review remain open.
No maximum is qualified, and no cap or supported shape is relaxed.

Maximum FASTPQ ordinary and AXT production and fresh verifier replays pass on
their recorded candidate. Current hardware controls pass 57 tests, including all
nine mandatory Metal controls on the M1 Ultra. The four-validator finalized
transcript scenario passes after the exact authenticated query repair, preserving
contiguous genesis-rooted finality, source substitutions and restart checks.
This establishes transcript custody; complete q77/D7 source/spend admission,
concrete cryptographic qualification and other hardware remain open.
The complete-effect ordinary artifact, original-pool funding and move-only
finalized-source lane are now integrated for compiler/native validation. The
ordinary profile and canonical artifact layout changed; preceding transfer-only
maximum results do not qualify this candidate. Durable completion publication,
recovery and automatic dispatch remain unfinished.

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
433 dep-info inputs). The latest focused Privacy validation lists 2,628 tests,
including 2,560 ordinary cases and 68 ignores. All original 65 ignores remain;
three additional ignores are reviewed cost diagnostics. All five corrected
GETGAS/scalar-history regressions pass, with captured source, Git, toolchain,
managed inputs and artifacts unchanged through build and runtime. The earlier
ordinary suite closed naturally with 2,424 passes, 52 failures and 65 ignores,
with concurrent source changes. All 52 failed cases retain ordinary coverage
and require current-source validation. Full current ordinary and merged-source
qualification remain open. The derive library, strict JSON
and UI regressions pass, preserving diagnostics.
SDK Native custody and genuine production proving are mandatory even with SDK
defaults disabled; assembly tools remain explicit
`dev-tools` targets, and FASTPQ uses the existing STARK feature. All 21 configured
dependency boundaries, including exact CoreZK/Halo2 SDK and downstream
profiles, pass locked offline Cargo resolution with unchanged manifests and
lock. The current source budget
passes with exact reviewed declaration costs, without growth headroom or
relaxed ownership denials; current native frontend and proof qualification
remain open. Surface/toolchain workspace lint inheritance and Surface public
Rustdoc still require correction.
The compiler and proof owners have a recorded clean default Core ZK frontend
check; focused strict lint and current repaired Privacy regressions remain open.
The canonical Wallet Selection schema expectation now matches its owner. The
expanded prospective Wallet registry contains 107 ordinary tests, including
15 custody controls. Its latest build passes, but dependency source changes
prevented runtime validation. Feature hygiene passes all 65 controls and its
guard command. The compiler source guard seals 305 fixture includes and
605 test names, and all 43 Python source-reader controls pass. Executable
metadata freshness
passes on its recorded source cut. Current workspace and merged-candidate
qualification remain open.

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
- **Privacy/crypto:** an earlier joint X509 maximum proof passes verification,
  size, RSS and enforced address-space checks but fails the 300-second limit.
  The corrected RFC profile and authentic component fixtures require a fresh
  complete proof. Complete relation, adaptive transcript/hiding and side-channel
  qualification remain open. Maximum q77 ordinary/AXT proofs, local
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
