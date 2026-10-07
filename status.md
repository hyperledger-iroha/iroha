# Status

Reviewed 2026-10-07. Iroha 3 remains under implementation and qualification.
Component checks cover substantial portions of the system, but the combined
source has not passed the complete workspace, SDK, hardware and release gates.
The [roadmap](roadmap.md) lists outstanding outcomes; the linked specifications
hold detailed acceptance criteria. Routine repair receipts belong in PRs and CI.

## Current implementation

| Area | Current state | Remaining qualification |
| --- | --- | --- |
| Sumeragi | The sans-IO core, simulator and node driver are integrated; the previous consensus runtime is removed. Certified results bind World roots and ordered events. | Current-candidate simulator/mutation gates, consumers, network faults and authenticated accelerated restoration. |
| Lanes and dataspaces | Fixed/elastic lanes run as Sumeragi instances; the global chain merges certified lane blocks. The daemon can host an independent signed dataspace root with its own State, Kura, allocation pool and native context archive. Lifecycle/restart have component and node coverage. | Production AMX bootstrap, complete outbound proof custody, durable validator relaying and current-source network isolation, scale and restart. |
| Storage and execution | `lanes::LaneRunner` and `SumeragiLaneMerge` are the production path. Kura owns a shared fail-stop gate and authenticated native tips/journals. | Original funded execution custody through acquisition, certification, publication, replay and retained-generation reclamation. |
| Configuration and DPN | Private dataspace definitions are separate from validator settings. `iroha dataspace plan/apply/status` derives artifacts and retains once-only transactions under one budget. Kagami has an isolated BPNG catalog/paid-namespace genesis preset. | Profile-based generator closure, four-daemon paid deployment/readback and physical isolation. BPNG local contracts, fee authority and application provisioning remain incomplete. Owner-node provisioning is outside this path. |
| Kagami/Mochi developer experience | Shared native localnet generation, process ownership and workspace contexts are implemented in `iroha_deploy`; Musubi exposes source/artifact/package deployment with exact retained recovery. Installed-runtime fixtures cover config-free startup, all three deployment inputs, restart recovery and four-parent/four-private attachment. | Obsolete test-network load-authorizer setup is removed after the Kagemusha role retirement. Focused compilation and the seven new history parsing/retained-handle regressions pass; the remaining regression run and current peer-configuration controls are pending. Concurrent changes outside the guarded DevEx sources remain outside that qualification. The prior candidate passed the filesystem suite and selected Core, snapshot and funding regressions, but its original bounded 64-history and parent recovery tests exceeded their deadlines; the parent test did not reproduce the earlier stack abort. Matching normal-package and installed-runtime validation, combined paid provisioning, anchoring, payload isolation, provider renewal, and publication/frontend handoff remain required. Cold registry publication, the approved committed Taira installation profile and recurring signed checkpoint publication, native desktop interaction, signed native OS matrices and reference-host latency remain open. Release bundling refuses without that approved public profile. See the [developer goals](specs/kagami_mochi_devex_goals.md). |
| Rust client | Immutable account contexts, owned async transport, explicit blocking capabilities and typed fee quoting are implemented. | Remaining capability/consumer migration, unified errors and network cancellation/finality/authorization coverage. |
| Kotlin/JVM | Kotlin owns the SDK, HTTP/SSE/WebSocket, attestation tools and JNI API; Java consumers exercise that API. Host coverage includes native/confidential operations. Android Keystore alias existence is decided only by keystore2 `getKey` (API 31+); a Keystore error is never read as absence. | Remaining Java/publication retirement, signed packages, CUDA hardware and Android/device qualification. |
| Other SDKs | Shared prepared-operation, signing, account and native checkpoint contracts are being migrated across Swift, JavaScript, Python and C#. | Same-source native artifacts, complete fixtures/consumers and release OS/architecture matrices. |
| Torii collection queries | Seventeen collections and eleven explorer feeds share the [query contract](specs/torii/collection_queries.md) across Rust, Kotlin/Java, Swift, JavaScript, Python, C#, CLI and MCP. Identity-ordered reads seek and stream; authenticated history checkpoints bound deep reads. Global-state collections execute once per read for exact totals and aggregates. History and explorer feeds explicitly reject unsupported controls. Signed query selectors have one feature-independent layout; client network context is explicit. | Current-candidate live multi-dataspace paging and full SDK/native delivery qualification remain open. The genuine ABI-25 Swift framework, full native Swift package suites, ordinary SwiftPM Release consumers and checked-in iOS demo simulator suite pass; physical-device and signed public-release qualification remain separate. JS and Python native suites require matching authenticated artifacts. See the [completion goals](specs/torii/query_completion_goals.md). |
| Norito | Declared identities own canonical frames; payload serialization/reconstruction and explicit JSON key contracts are integrated. | Consumer/feature closure, fallible allocation ownership, physical model extraction and workspace lint/runtime coverage. |
| IVM/Kotodama | IVM is the sole VM with ABI V1 and program header 1.1; header 1.0 is rejected. Compiler separation and state-free proof owners reduce normal dependency graphs. Source bundles support declaration includes and explicit module exports; authenticated error-message catalogs preserve nominal schemas. FASTPQ Metal uses one embedded-bundle admission owner; runtime source/path loading is removed. Ordinary Linux/Windows daemon dependencies include driver-loaded IVM CUDA; genuine bundle absence permits CPU build/startup, and supplied unapproved material is rejected. Original-pool idle-runtime rows and scoped committed/frozen account-rekey, trigger-contract and all four trigger-action captures are integrated; fresh compilation and runtime validation remain pending. | Lifecycle/custody closure, complete State/Kura publication, native execution proofs, anchored private invocation/AXT, coherent SDK regeneration and hardware validation. FASTPQ has no approved Metal bundle. IVM CUDA approval is `None`; genuine signed ten-family artifacts, driverless runtime checks and physical qualification remain open. |
| ZK delivery plan | The [plan](specs/zk_delivery_plan.md) and its [task graph](specs/zk_delivery_graph.json) own the ZK, privacy, authenticated-State and FHE work. Landed as source-checked contracts and inventories: plan and graph checks; the zk-X509 presentation interval with shared vectors; the FASTPQ `air` interface with the shared-backend inventory; the IVM proof-coverage inventory; the first-release history and cutover contract with a pinned history; the FHE ownership inventory; the RAM-LFE V1 policy, receipt, opening, class and query-limit contract with a cleartext reference; the State table and root inventory with the keyed-commitment contract (`specs/sumeragi.md` §16); and the unified resource contract. Two shared crates exist: `iroha_fhe` (exact RNS, NTT, basis-conversion and rounding arithmetic used by BFV, ZK-AMS, Jindo and Bootle-Lantern) and `iroha_measurement` (phase and resource records, with `scripts/zk_resource_harness.py`). | No new proof relation, FHE construction, native ISI or SDK operation exists yet, and nothing consumes the RAM-LFE V1 types. The certified State root is still the World-only accumulator (inventory defects G1-D1 to G1-D11). The resource contract records eleven relations violated today, so a maximum zk-X509 proof transaction cannot be committed under current defaults. AVX2 on a physical x86-64 CPU, Metal/CUDA parity, Linux `RLIMIT_AS` enforcement, a complete X509 proof under the harness and four-validator evidence are unexecuted. |
| SoraFS | Software signing, canonical manifests and storage/billing/publication ownership are implemented; the ordinary Node library tests pass. | Matched daemon/harness, provider resilience and L1/L2 promotion. |
| KAGEMUSHA | G1 revision4, core33/rest8, fixed64 quotas, Request-recorded blacklist and time controls are implemented. Native PIPA-R/PIPA-AS and recursive operation components have genuine proof coverage. Shared wallet components retain durable folds, exact replay and descriptor-relative custody. Ordinary Load execution/finality adapters are implemented; dedicated mint-finality, generic consensus attestation and superseded monetary engines are removed. See the [candidate evidence](specs/kagemusha_evidence_gate.md#8-recorded-results). | Full protocol qualification remains open. Removing circuit-fixed scheme identity resolves the key-generation cycle; genuine Bootstrap tests accept two schemes under identical keys and reject a correctly signed foreign-scheme certificate. Affected A/W/Omega artifacts require re-keying; earlier captures retain their recorded source scope. Compiled source intake and a source-bound ABI25 host bridge/SDK capture pass their component checks. Complete authenticated producer catalog, ordinary-finality Load and full Archive proofs, NativeProofs and typed lifecycle integration, complete SDK delivery and real-proof A→B→C→unload remain unfinished. M3 has correctness and oracle coverage but no accepted real-chip performance qualification. Structural Payment size is9,979B (1,723B overhead+3,456B sigma+4,800B Omega); actual full-catalog acceptance must establish the10,000B gate. Two-second p95 durable completion and physical-phone qualification remain unmet. |
| Petal Stream | `iroha_petal` implements the [Petal Stream](specs/petal_stream.md) animated optical transport (`天` orientation field, katakana, tile polarity and ring dots as three Reed–Solomon lanes under a rateless fountain) with decoder, renderer, camera simulator and `iroha offline petal`. A gain-free tile read keeps lanes `P` and `K` alive under over-exposure, veiling light and shadows. Swift, Kotlin/JVM, JavaScript, Python and C# ports reproduce the shared fixtures and decode the golden captures to the recorded lanes. Three corner blossoms suffice (a thumb, glare or frame edge may hide the fourth) and sessions track the pose between frames (about 4× cheaper per frame). Simulated reads complete a 10,000-byte KAGEMUSHA message in 11.5 s (45 s from lanes `P` and `D` alone). | All evidence is simulated: physical-camera reads on the governed Android/iOS device matrix, lane `K` at 480p or soft focus, and the public guides in `iroha-docs`. |

## Immediate blockers

The Nexus proposal/status repairs, publisher-custody fixture and original invocation
binding have fresh native functional coverage: the rebuilt daemon and harness passed
ten fresh sixteen-validator paid settlements and the original serial all-seat
restart/readback diagnostic. The
independent disjoint-lane control also passed stopped-committee progress, recovery
without resubmission and all-process restart. Subsequent AMX custody and harness
repairs require fresh affected checks. Full fault/leakage campaigns and the
complete original funded execution graph remain open.

Native ceremony handoff preserves overlapping descriptors and read-only phase pipes.
The rebuilt paid committee attempt authenticated seven-seat activation at height 129
and its E+3 four-seat election, completed the corrected reward and exit-scheduling
checks, then failed during four-seat return preparation when the original ceremony
transport deadline expired. The original overall runtime limit was also exceeded.
Full paid 4→7→4, withdrawal, return-boundary liveness, restart and slashing qualification
remain open. Lane retirement component controls now cover original reads, publications,
historical opening joins, final-reader release and authentication/refusal propagation.
Physical retirement and complete retained-history dependency closure remain open;
certified disk frames remain retained for global replay.

Actual original-library regressions reproduce generic snapshot undo loss: a present
previous null value is serialized indistinguishably from no previous value in Cell and
Storage. Writers and readers now use explicit present-value undo framing; current-source
qualification of the coordinated repair remains open.
Strict startup still rejects nonempty snapshot caches and uses certified replay;
accelerated complete-State restoration remains open.

The runtime and trigger permission guards reject malformed recognized payloads before
delegation; decoder refusals preserve local deferral. Core strict lint still fails on
unused production/resource graphs, and blanket Clippy allowances leave entire lint
groups unqualified. Dependency and test-network lint failures also remain open until
their repairs pass the actual affected commands. The repaired Rust SDK client source
passes its unit suite and strict lint; qualification of the final combined source, signed RS16
loss/withholding and release remains open. Exact finalized-carrier retries retain
original execution without requeueing or charging again; changed source requires fresh
evidence.

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
Current-source happy-day settlements and serial restart have functional coverage;
fault, leakage, performance and release campaigns remain open. Earlier source or
component observations do not qualify the final combined candidate.
See the [protocol](specs/private_settlement.md).

Focused runs on the combined ZK-plan tree on 2026-10-05 pass `cargo check` for
`iroha_core`, `iroha_torii` and `irohad`; the workspace gates were not run, and
these failures remain. Torii lib tests: 106 of 5,437 fail repeatably and at
least three more intermittently in a full parallel run; all five
`address_parsing::explorer` tests of `torii_core_routes` and 15 of 54
`agent_alias` tests of `torii_protocols` fail. `irohad_lib`: a full lib-test run
aborts on a stack overflow in `beacon_bootstrap::seat_attempt::aggregate_tests`
after five `durable::tests` failures, and two
`musubi_publication_service::finality` tests fail. `iroha_core`:
`sumeragi::node::tests::every_committed_block_contains_work_before_and_after_restart`
(second transaction not committed within 30 s), three `sumeragi::executor`
archive, publication and replay tests and `state` reserve-account tests fail.
`fastpq_prover`: three lib tests and the
four-quadrant tree capacity test (124 retained nodes against 127) fail. Four
`ivm` Metal vector tests fail intermittently in a loaded full run and pass
alone. Four repository guards are red for committed changes that predate the
ZK-plan work: the trusted release-surface digest, the
panic-recovery boundary inventory, the environment-toggle inventory and the
generated-source registry (`crates/iroha_petal/src/glyph_templates.rs` has no
owner).

## Deployment state

Taira's four validators serve the e3766fdb daemon build and the fresh signed
genesis `277902D32673C29F56D4AA104063347F290881909ECB838B5038E39ABE11C15F`.
October 4 direct checks found height 9, three peers and an empty queue on every
validator. Public status, text readiness and faucet policy return HTTP 200;
twenty-two transactions are approved and none rejected. Idle chains create no
empty blocks. Shared HTTP defaults are 1,000,000 requests per second,
60,000,000 per minute and a 10,000,000-token burst. Native amendments applied
26 explicit request-budget fields to each serving validator; installed-daemon
config checks and service-unit checks passed before a serial restart. All four
new process config bindings match the native amendment receipts. The live
amendment excludes optional recipient lookup because the installed daemon's
schema cannot accept its new source budget. The deployed native client's basic
doctor passes all fifteen checks, including MCP server discovery and tools list.

Current source initializes fresh safety records before first startup and retires
completed execution after successful replay before strict native archive
attachment. Deployment preparation, transfer and the routine updater accept an
authenticated build-only candidate without a full regression gate. On-chain
governance owns deployment policy; no fixed 24-hour fault test is a prerequisite
for testnet or production. Release qualification remains open.

The previous 7c77fd3c network's public accounts regression completed 240
requests with HTTP 200 and no HTTP 429, but sustained only 3.01 requests per
second against the required 20.
The deployed fanout admission reserves its entire 48 MB execution pool for each
read; the current single-World collection redesign still requires bounded row
allocation and encoding before it can use a smaller concurrent reservation.
Current-candidate release qualification and authenticated deployment completion
remain open.
Private DPN, paid `dpn`/`admin@dpn` and clean-client completion remain open.
The serving bundle lacks the standard `iroha3d` sibling and an installed native
network profile. Source now prepares independently signed checkpoint/profile
artifacts and provisions the dataspace and owner alias under one retained quote;
component validation and a matching complete bundle remain required before
those paths can qualify live deployment.
Validators run in a Linux guest on MacStadium in Dublin; use the approved
deployment tooling. Retained incident records describe the
[previous readiness failure](docs/incidents/2026-09-30-taira-readiness.md).


BPNG retained-history qualification, validator catch-up, additive catalog
activation and API22/FE17 application commissioning remain open. Basic acceptance
does not close crash/concurrency or advanced guest work. Infrastructure changes
require the explicitly approved OVH target.

## Build and release qualification

The current `optimizations` checkout includes uncommitted handoff repairs.
Fresh combined Core, node, workspace and SDK qualification remains open.
The repaired daemon and Nexus harness build with the original optimized compiler
settings. Their observed Core compiler invocations stay below the unchanged 13 GiB
kernel-measured ceiling, but both builds exceed the 20-minute target. Other feature
combinations and the complete release remain unqualified. Complete Core unit tests
and fresh native network consumers still require the same repaired candidate.
The earlier eighteen-target Core/Kagami build and all 262 selected native
executions passed with their recorded source and artifacts; they do not qualify
the changed combined candidate. They account for all original
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

The old KAGEMUSHA component controls (journal recovery, issuer keys, enrollment
floors, expired preparations, Guard generations and claim carriers) were deleted
with their code. Primary-owner Privacy library strict lint passes
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
SDK custody and genuine proving qualification follow the canonical G1, Advance
and PIPA owners; their bridge, SDK and ledger integration remains open. FASTPQ
uses the existing STARK feature. The recorded signed optimization validation baseline is
`4486aa78661b8f289d21238e034fc2ece10600d8`. Recorded static source-graph
discovery lists 124 workspace members; current resolved Cargo metadata remains
unqualified. Source-cost and feature-hygiene checks passed on that recorded cut
with every cost limit exact.
Recorded dependency boundaries, legacy-codec and checker controls retain their
input cuts. Shipping Oracle denials and the aggregate-model test exception
remain intact. Core retains consensus ownership after the unused direct daemon
Sumeragi edge was removed.

ABI argument-record and static numeric encoding belong to `ivm_abi`; consumers
import the owner directly. IVM retains decoding, gas and memory custody.
Compiler/toolchain, all 14 ABI/IVM phases, direct SDK consumers and workspace
qualification remain open. Core ZK requires current strict default/no-default
libraries, test-feature library/group coverage and canonical proof-owner and
consumer checks. Its retired real-proof-harness feature is not a target.
FASTPQ's ordinary runtime and raw-field regression remain pending. Privacy's
previous failures remain ordinary tests and require current regressions followed
by the full ordinary suite with stock ignores retained.

The generic Norito array correction is applied and formatted, preserving explicit
raw-field layouts and charging actual owned allocations once. Complete
Primitives/P2P/Manifest and Norito/derive/Crypto suites passed on the recorded
cut with stable source.
The latter Cargo run succeeds; its original outer failure from a changed Git
baseline is preserved. Independent comparison now joins both native Source/Git
captures and the reviewed application to the genuine formatter baseline at that
recorded cut. Subsequent commits and selected-input changes require fresh
qualification. Broader native tool, artifact and consumer qualification remains
open. Earlier
Model and Python passes retain their
original inputs; full current requalification remains pending. Drifted script
runs remain diagnostic and interrupted unknown exits remain unresolved.
Journal's recorded selected checks require requalification after selected input
changes. Wallet rejects unknown native material before decoding, signing or
HTTP; its expanded registry preserves prior cases and custody/frame controls,
with Wallet and SDK native qualification pending. Compiler fixture/source-reader
and CI controls retain their recorded scopes; compiled bytecode and actual CI
execution remain unqualified. Executable metadata freshness passes on its
recorded cut. Current workspace, merged-candidate, release and O7 timing
qualification remain open. The
[optimization goals](specs/compile_bloat_optimization_goals.md) retain all seven
completion criteria and separate pinned-runner resource requirements.

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

- **Parliament:** the [final requirements and launch decision](specs/parliament_private_ballot_design.md)
  requires PQ private ballots without decryption custodians and with potentially
  small electorates. No construction satisfying accepted-voter dropout is
  selected; this is a construction blocker, with no pending owner decision.
  Binding-governance mainnet launch is no-go until qualification. The fast pause
  panel is seated per epoch independently of attempts. Current timed-OVN ballots and consensus-mandatory Parliament pulse/
  custody checks do not satisfy that target; construction and availability
  isolation remain blockers.
- **Services:** Musubi publication/paid contracts, standalone elections,
  SoraNet/Linux helpers, SCCP live corridors and Inrou Linux/AArch64/KVM isolation
  remain unqualified.
- **Offline money/devices:** the [single KAGEMUSHA target](specs/kagemusha_single_design_proposal.md)
  uses a released wallet on a stock uncompromised OS, hardware-backed keys,
  a durable software provider and recursive proofs. Its Send is irreversible;
  recovery replays exact Payment bytes to the receiver, with no refund path.
  The old per-operation online-control implementation was deleted on
  2026-10-05; the G1 canonical objects (with cross-language vectors) and the G2
  `Advance` provider are the only implementation. Measured Pasta IPA proof
  sizes (≈ 358 bytes per advice column at k=16) mean a 10,000-byte Payment
  needs a narrow large-k outer proof layer; that architecture and the 2 s
  target await owner decisions. Complete proofs, the provider's bridge
  integration, load/unload and device recovery measurements remain open.
  The [checklist](specs/kagemusha_evidence_gate.md) records verification without
  an approval gate; software markers claim no protection against OS takeover.

First-release contracts remain canonical APIs, domainless `AccountId`, Norito
wire formats and deterministic ABI V1. Sumeragi uses exact `3f + 1` global
committees, exactly `n - f` votes, signed RS16 availability and work-driven blocks.
Ordinary signing supports authenticated software custody. The first production
KAGEMUSHA target requires platform enrollment, genuine monetary proofs and
durable local state/replay authority under its stock-OS assumption; it has no
custom applet or one-use-key prerequisite.
