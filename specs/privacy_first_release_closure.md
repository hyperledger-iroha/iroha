# First-release privacy and ZK closure

This ledger records the implementation and qualification work for the final V1
privacy stack. It is not an audit certificate or permission to activate an
unqualified protocol. Source observations below were checked on 2026-09-06–09 in a
shared working tree; they do not identify a sealed release candidate.

The [2026-09-09 validation checkpoint](../docs/history/2026-09-09/privacy-validation-checkpoint.md)
records the retained native23 four-validator failure, completed SDK20 Apple
build, failed whole Swift suite and integrated corrections awaiting execution.

## Final interface contracts

- One ordered Exact12 catalog, one V1 proof envelope, exact protocol/proof-system/
  engine tuples, and one weakest-composition security model per protocol.
- Native STARK commitments use the shared six-lane Goldilocks Poseidon-x7
  construction, canonical 48-byte digests, and role-separated transcripts.
  Fp4 values have four canonical coefficients and an exact 32-byte encoding.
  Public metadata hashes remain the hashes explicitly specified by their owning
  protocol; they are not alternative native STARK commitment backends.
- The local compiled-profile catalog conveys build metadata. Production
  availability additionally requires committed activation and the complete
  signed release/deployment qualification matching that activation.
- The final privacy-intent validator requires the statement network to equal the
  transaction network domain, even after both intent and statement digests are
  recomputed. Genesis-only transactions cannot carry a final privacy submission.
- The capability archive keeps its 256 KiB outer limit. Nested decode budgets
  must admit the required 48 stage receipts, 54 proof artifacts, audit records,
  signatures, and four-validator deployment record while retaining explicit
  sequence, allocation, field, and depth ceilings. Catalog cardinality is checked
  separately from the nested sequence budget.
- The C bridge has six privacy symbols: compiled-profile getter and validator,
  Exact12 fixture getter and validator, capability-manifest validator, and buffer
  free. The capability validator accepts supplied bytes and returns the shared
  Rust validation status. It does not obtain or synthesize network state.
  Swift and C# must call it before projecting a manifest or issuing admission;
  an old library without the symbol is unavailable. No managed validation
  fallback can establish production qualification.
- IPA supports only the binding Pallas and BN254 commitment groups. The additive
  Goldilocks test module, public types and optional features are removed from
  the proof crate and all Core/IVM/Torii consumers. Goldilocks remains the STARK
  field identity; IPA decoding rejects it before group arithmetic.
- Retired layouts, aliases, signatures over obsolete payloads, and old proof
  artifacts cannot be accepted as alternate V1 paths.

See [proof envelopes](zk_envelopes.md), [FASTPQ](fastpq_plan.md), and
[audit dispositions](zk_cryptographic_audit.md) for implementation details.

## Exact12 implementation map

An implemented engine or available compiled profile in this table is not a
production-qualified protocol. All rows still require same-candidate release,
independent audit, SDK, hardware, and deployment evidence.

| Protocol | Current source observation | Remaining outcome |
| --- | --- | --- |
| ZK-ACE | Dedicated masked STARK; six identity lanes and six replay lanes, 8x LDE, Fp4 FRI and 136 distinct queries. Activation unavailable. | Complete qROM reduction, multi-target accounting, independent implementation review and final artifacts. |
| Anonymous PGC | Native P-256 bootstrap and payment relations, including twisted-ElGamal legality. | Full maximum-shape, malicious-party, resource and release qualification. |
| VeRange | Native P-256 range profile and typed component surface. | Same-candidate range, composition, resource and release qualification. |
| ZK-AMS | Admission/provisioning structure exists; composite MKHE readiness unavailable. | Complete resource, wire, malicious-party, decryption-share, phase-2/3 and full-size release-KAT gates. |
| Vega | Credential relation and Figure 9 key-install machinery exist; compiled profile unavailable. | Full-shape governed keys, independent proof vector and complete Figure 9 qualification. |
| ZK-X509 | Native certificate relation exists; compiled profile unavailable. | Narrow or recursively compose the relation to fit the 9 MiB limit. The current shared-geometry projection is 16,447,808 bytes, with 12,235,648 bytes of raw trace openings alone. Regenerate artifacts and measure the real implementation. |
| Jindo | Native Figures 2–7 implementation with 32 signed-monomial repetitions. | Reviewed qROM extractor certificate, exact adversarial/max-shape evidence and production qualification. |
| Bootle/Lantern | Native lattice anonymous credential and Falcon issuer implementation. | Independent arithmetic/sampling/custody review, issuer lifecycle, maximum-shape and release qualification. |
| Orchard | Sole Orchard/PostNu6_3 profile with two-pass preparation and authorization. | Audited parameter/proof provenance and full native/SDK/network qualification. |
| FCMP++ | Native membership, generalized Bulletproofs, ranges, linkability, conservation and wallet paths. | Complete maximum-shape, cross-authority, resource and release qualification. |
| IVM private note | Native profile and lifecycle integration exist. | Shared STARK soundness/qualification, program/authority adversaries, SDK and network evidence. |
| PQ-MASP | Native note/AIR/profile and lifecycle integration exist. | Shared STARK soundness/qualification, full action/asset conservation, SDK and network evidence. |

The compiled-profile owner is
[`privacy_profiles.rs`](../crates/iroha_core/src/privacy_profiles.rs). Engine
implementation markers must not be substituted for the qualification record in
[`release_manifest.rs`](../crates/iroha_data_model/src/privacy/release_manifest.rs).

## Coupled stack requirements

| Component | Completion criterion still required |
| --- | --- |
| Generic native STARK | Current Binding and Explicit paths reconstruct the complete public trace/composition roots and enforce a zero terminal value. A future hidden-trace AIR still needs a verified initial degree/proximity argument; binary fold consistency alone is insufficient. Standalone public-padding verification remains unavailable, and BFV/Soracloud callers must complete explicit material replay. |
| FASTPQ | The sole offline compact V1 owner now compiles with six-lane commitments, complete typed context and fixed field tapes; accepted compact SHAKE/prototype selectors are removed. Complete actual full-domain proofs and fresh artifacts, replace the production replay representation, and finish the bounded-opening AIR/FRI and witness-privacy arguments within unchanged production proof limits. Complete protocol-specific qROM analysis and independent permutation/construction reproduction. |
| AXT | Both Core execution pipelines now commit exact ordered canonical transaction wires; missing block-owned commitments cannot be synthesized from transcript identities. Anchor-bound proof verification checks ordered wire membership, exact public roots and context, and mandatory expiry. Consensus witness roots and transfer-batch trees still commit subsets, which cannot substitute for full persisted WSV roots. Complete successful-execution/transfer binding, rooted state witnesses, immutable anchor resolution and durable spend nonces. |
| BFV/Soracloud and MKHE | Full BFV-RNS and one atomic 40-limb source/materialization/packing/cross-field/padding verifier, full-size/eight-party KAT, resource measurements and governed noise/qROM evidence. Unavailable stages cannot issue receipts. |
| Confidential assets and private settlement | Regenerated canonical proofs/keys, authority/amount/conservation adversaries, complete SDK routes and deployment evidence described by the owning settlement/asset specifications. |
| SoraFS | Complete the V1 closure ledger, live four-voter/multi-provider/dual-gateway L1, resilience/load/24-hour soak, all 17 summaries and ordered L2 promotion evidence. |
| Kaigi | Final 31-row authorization and 25-row usage proofs; retained original-account participation, exact keys/schema, suite-tagged HPKE, bounded accounting and authenticated relay recovery. |
| Elections | Complete private ballot/deadline/retry, finalized-beacon, rollback/restore and independent timed-OVN/threshold-BLS review on four validators. |
| SDKs and fixtures | Rust, Kotlin/Java consumers, Swift, JavaScript, Python and C# use the same final canonical bytes and native admission; signed same-source native packages and target-platform execution. Structural parser/source tests alone cannot qualify an SDK. |
| Hardware | Final FASTPQ six-lane hashes and polynomial derivation currently execute on CPU. Old scalar-permutation/FFT preflights cannot qualify final V1 GPU proofs. The allocation-free typed frame now shares CPU, streaming and hardware input framing. Dedicated Metal digest dispatch and cleanup/quarantine tests pass locally; CUDA compilation/device evidence and full proof routing remain outstanding. Complete real mode propagation; then execute CPU/NEON/SIMD/Metal/CUDA parity, fault quarantine and measured memory/throughput. A feature build or selected mode is not device execution. |
| Release and deployment | Clean signed source/lock/toolchain identity, complete independent audit classes and finding dispositions, real 48-stage/54-artifact evidence, exact four-validator quorum, staged restart/canary/convergence and authenticated endpoint readback. |

## Verification discipline

Core protocol fixtures bind the runtime `NetworkId` to their synthetic committed
genesis. Native payment and FCMP state tests exercise the shared submission
preparation and execution stages with an exact compiled activation; the shipping
instruction handler separately requires registered Exact12 qualification before
entering execution. A missing-qualification regression checks that an active
profile alone cannot mutate payment state or budgets. X.509 governance coverage
keeps block overlays in separate lifecycle stages to bound the default test-stack
footprint while retaining the complete atomicity and terminal-state assertions.

Focused regressions distinguish malformed geometry, layout-dependent transcripts,
noncanonical field representatives, invalid AIR stride, incomplete evidence and
missing native authority from honest canonical inputs. Synthetic signed manifests
test validation logic only; they never represent deployed validators, actual
proof runs, independent auditors or hardware measurements.

The full workspace/feature, SDK/native, audit and four-validator qualification
matrix remains open until it passes against one immutable candidate. Local tests
and extracted arithmetic harnesses are reported with their actual scope. Failed,
blocked, ignored, timed-out or unavailable runs cannot be recorded as passes.


## Scoped local verification, 2026-09-06–07

- Direct IPA owner suite: 73 passes after removing the additive-field backend.
  Cargo feature ownership: 19 passes, including resurrection rejection.
- Final FASTPQ mode admission: six regressions pass; explicit GPU requests fail
  before witness work, and Auto reports actual CPU execution. Seven observer and
  three ambient-layout regressions pass. Final GPU execution remains unavailable.
- Shared six-lane framing passes all 39 primitive tests, including exact lane
  substitution, byte/rate boundaries, atomic output rejection, private-state
  diagnostic redaction and the unchanged digest KAT. A separate Python integer/SHAKE256 reference reproduces the complete
  parameter asset, frame bytes and six digest lanes; three adversarial controls
  pass. This is independent arithmetic implementation evidence, not an audit.
- AXT ordered-wire model tests pass five cases and the signed-spend mutation
  regression passes. All 76 FASTPQ binding tests pass, including six anchor-bound
  cases. Seven Core producer/capture/lane/sealed-reveal regressions pass against
  the refreshed Core executable; fabricated anchor tests do not establish finality.
- Dedicated six-lane GPU primitive: nine host staging/readiness/quarantine/
  cleanup tests pass; the separately required Metal dispatch test passes on the
  M1 Ultra. An independently compiled Metal kernel matches Python reference
  outputs for 12 frames and all 72 lanes. These are digest conformance checks;
  CUDA device execution and complete GPU proofs remain unqualified.
- The current-source Merkle executor passes three batching tests, one injected
  dispatch-failure test and five preprocessing regressions. Required Metal
  multi-level/root parity passes across all tree roles, FRI rounds, empty,
  single and odd trees, and forced two-frame dispatch boundaries. Six full-proof
  admission regressions still require explicit GPU proofs to remain unavailable.
  The normal Cargo executable also passes ten device staging/readiness/quarantine/
  cleanup cases, both Metal Merkle/failure cases, five preprocessing cases and
  seven observers. The raw64 proof fixture regenerates byte-identically.
- Generalized Bulletproof secret cleanup: 36 passes after isolating byte counters
  per test thread and serializing every shared tracking-suite counter user.
- FASTPQ raw64 fixture: 1,136,406 bytes, reproduced across unoptimized and optimized
  primitive builds. The bounded diagnostic verifier accepts its explicit 2 MiB
  budget; the unchanged 512 KiB production limit rejects it. This is not a
  production batch-size or qROM certificate.
- Network-corrected Exact12 fixture publication passes the Rust generator's exact
  check. JavaScript fixture/capability/network tests pass 23 cases, Python passes
  94, Kotlin passes 16 and C# pure fixture tests pass 13, without skipped cases.
- The rebuilt native capability FFI passes both pointer/resource and forged
  qualification regressions. The shared C header and all 25 drift controls pass
  with six privacy exports.
  Swift runtime qualification still requires the rebuilt XCFramework; syntax
  and source checks do not replace it.
- SDK source admission checks pass 90 regressions, including immutable origin,
  exact expected-network propagation, one-use JS transport receipts, Kotlin
  wire-payload snapshots, and final Swift envelope/cache controls. The strict
  audit retains its explicit source-prerequisite evidence label.
- The subsequent SDK authority review found offline-archive promotion in Kotlin,
  Python and the JS admission helper, plus missing deployment-network comparisons
  in managed SDK projections. The final origin/network fixes now bind admission to configured client networks
  and reject offline inspection objects at construction. Focused tests pass:
  JavaScript 62, Kotlin 63, Python 103 and Swift 35. The actual C#
  native privacy/verifier selection passes 143 tests against the rebuilt ABI-23
  host bridge; it is local artifact conformance, not signed package qualification. Swift also rejects
  the retired 11-field envelope and checks all twelve final statement contexts.
  C# passes 11 source/native-validator mutation controls. Swift execution used
  an isolated source-only package; native packaging remains unqualified.
  Kotlin captures caller-provided wire payloads before admission and encoding;
  Swift capability requests explicitly bypass caching. Native JNI/PyO3/N-API
  unit selections pass 4/7/2 tests respectively. Actual Kotlin and Java host
  runtime consumers pass 37 tests. The captured Python wheel passes canonical
  installation verification and 169 installed-package tests, plus actual native
  authenticated-transport replay/network controls. N-API package runtime
  execution remains open. JS private request/auth/response methods prevent
  mutable public helper overrides from minting transport receipts.
- The final 50,264-byte model intent KAT is reproduced through the public model
  API and independent Python BLAKE3 framing. The model privacy selection passes
  127 tests with three explicitly ignored generators.
- MKHE section codecs pass ten tests, including transport-integrity rejection
  and five resealed context substitutions that reach final binding rejection.
- The explicitly selected MKHE tail transport gates pass both cases: 176
  sequential object publications and construction of the 3,520-entry provider
  allowlist. Peak process RSS was 17,186,816 bytes. This checks the bounded
  transport components; the live phase-2/3 owner and full proof resource gates
  remain unavailable.
- Three explicitly selected full-size MKHE arithmetic tests pass: canonical
  common-a streaming, staged/native NTT boundary-pattern parity and sparse
  subtraction parity across all 38 release limbs. The isolated process used
  116,129,792 bytes peak RSS and completed in 20.25 seconds. This is arithmetic
  conformance, not full-proof or eight-party resource qualification.
- The explicitly selected real T256 membership smoke proves and verifies one
  16,384-coefficient ternary chunk, including nonzero commitments and secret
  cleanup. It passes in 263.59 seconds with 50,659,328 bytes peak RSS. The full
  eight-chunk membership KAT also passes in 1,758.52 seconds with 69,140,480
  bytes peak RSS. CPK relation linkage remains unavailable.
- Three explicitly selected Vega/Figure 9 circuit gates pass the authenticated
  input/statement mutation matrix, independently signed calendar boundaries and
  exact split metadata/step/core-row checks. Peak RSS was 121,241,600 bytes.
  Governed key installation and independent full-proof qualification remain open.
- The remaining three observed MKHE regressions pass focused reruns: compact
  ciphertext diagnostics, the standalone qPCS unavailability assertion, and
  malformed fold-history rejection before and at prover admission.

The canonical five-target Apple builder ran from isolated snapshot
`57baf6390a036f2979dd4ac93e84e844c30d009b4c3dac376f637ab97bc96bc1`.
That dependency-closure fingerprint precedes subsequent main-tree fixes; a
successful artifact can qualify only that captured source and executed targets.
Its first iOS target compiled, but archive export failed on duplicate PQCrypto
symbols; no XCFramework or Swift native runtime pass was produced. The cause
was the bridge redundantly bundling three transitive PQClean archives. Removing
those bridge link attributes preserves dependency ownership and passes complete-
archive consumer linking on ARM macOS, ARM iOS and x86-64 macOS in an isolated
control reproducer; the ARM host SHA3 known-answer test passes. The unchanged
control reproduces the duplicate-symbol failure on all three targets. The actual
bridge source is corrected. A subsequent complete five-target build started from
immutable overlay manifest `c1e13661ade2148661fa620425a66846cd18af41ec31a53b7ab2043df5c37983`.
It precedes the subsequent Swift request-cache change and Python dependency
owner correction; it cannot qualify the later moving checkout. Its first iOS
target finished in 64m49s; after the prior process stopped, the same frozen
build resumed using its existing target cache. The later host restart deleted
that temporary snapshot and build cache. It produced no complete XCFramework or
Swift native runtime pass. A new durable shared source capture and official
five-target build are required.

The previous complete optimized proof-crate run finished with 1,294 passes,
four failures and 13 explicitly ignored resource/generator tests. All four
failures now pass focused reruns (ten section-codec and three additional MKHE
cases). The corrected complete proof-crate run passes 1,299 tests with zero
failures and 13 explicitly ignored tests in 3,085.08 seconds. Ten of the
ignored component gates pass the separate explicit runs described above; those
do not qualify the remaining full-proof resource gates. Earlier cleanup, independently
reproduced MKHE KAT and four split pending-owner regressions also pass.

The refreshed Core native selection completed with 54 passes and eight BFV
failures at a stale 64-character domain-tag limit. The final six-lane tag is 96
hexadecimal characters; the bound and independent BFV KAT are corrected and
its Crypto domain-tag regression passes. The subsequent actual Core selection
passes all 63 native tests, including full-material BFV verification and the
previous eight failures. Both 737,089-byte BFV conformance proofs reproduce
byte-identically in a supplementary current-module probe.
The current shared-framing ACE selection passes all 26 tests, including the 2,131,222-byte
canonical roundtrip, randomized proof verification, public-relation replay
rejection, stride and witness erasure. The BFV conformance-material generator and audited-
wrapper rejection tests pass both cases; they do not supply external noise or
qROM qualification. These observations are from a moving shared checkout, not one signed
immutable release candidate.

The final canonical Kaigi model passes 33 tests, including independent six-lane
identity vectors, complete multisig identities, layout invariance, mandatory
retained original-account ownership, sequence consumption and strict unmarked
Pasta scalar bytes. Retired artifact hints are rejected. After deleting all old
circuit and seed APIs, a fresh post-restart authorization/usage run passes all
20 tests in 44.66s, including real k13/k12 IPA proofs, every changed 31/25-row
public input, range/action/role constraints and independent C/N/A/U framing
vectors. The current proofs contain 3,264 authorization bytes and 3,136 usage
bytes. The test binary SHA256 is
`786540fdd7b9c270c5bb172862374fd3da81b1629364f1f96cac167105272ede`.
Its log, owned-source hashes and result are retained under the ignored
`target/privacy-release-evidence/2026-09-07-recovery/` directory.

The fresh combined build completed successfully in 564.62s. All three actual
Kaigi integration tests pass, exercising the final native authorization/usage
backend and lifecycle. The scoped Core unit selection passes 96 of 97 tests;
the remaining test used generic error display instead of asserting the typed
registry-capacity error. Its assertion is corrected, with update, rejection,
rollback and above-cap retirement checks retained; verification awaits the
next combined capture. This build records 23 unrelated SoraFS-node source
changes and does not claim an immutable whole-workspace candidate. The earlier
host restart cleared temporary logs and the frozen Apple build; those lost
artifacts cannot qualify this candidate.

The final wire previously passed 15 exact Rust/Python/Kotlin vectors and three
modulus-negative archives. Fresh post-restart account decoding passes 42 tests
in 0.65s, including every declared Norito layout. The shared Rust-owned fixture
contains 16 complete controller positives (all eleven algorithms, full weighted
and mixed policies, and 256 members) and seven malformed policy negatives. Its
SHA256 is `054f16109e6525d06565ef55d26a39b9291fad6831a39bcdd7d18cb1e232ffa6`.
Kotlin/Java previously passed 40 scoped wire tests; C# and Swift managed
diagnostics passed 5,690 and 80 cases. These predate the malformed-key correction:
C# accepted 11 of 12 malformed vectors, Swift/JavaScript/Kotlin accepted nine,
and both Python address implementations accepted seven. Public address and key
admission now requires the Rust cryptographic owner. Kotlin removes global
curve switches and duplicate parser APIs; its updated Kotlin/Java sources
compile with JDK 8 API enforcement. Three boundary tests pass, including a
fresh JVM checking eight missing-native failures and a signed-u64 multihash
length regression. Current native positives and package qualification remain
pending. Python's transport-independent `iroha-native` wheel owns the extension;
the pure full SDK depends on it and Torii account operations require it. Its
Rust test-target check passes in 2m44s, but actual two-wheel installation and
native fixture execution remain required. Signing restrictions remain at
actual signing operations instead of rejecting generic account identities.

Node authorization and usage generators use the final 31/25-row circuits,
retain only public proving material between calls, and self-verify generated
canonical envelopes through Core. Supplied mutable blinding views are cleared
on success and failure. The actual Rust JS-host suite passes 21 tests in 772.99s,
including both real proofs, all 31/25 row mutations, context and wipe controls.
Its unchanged executable SHA256 is
`7f1341545c01be00b72df7adb1a7f25d1694d1b4b0059c12b0d6b230e93d2ce0`.
The frozen official addon and package tests remain pending. None of these
results attests deployment, relay transport or release readiness.

The pre-merge FASTPQ balance-key migration passed 181 focused model/prover tests, including
four account display discriminants crossed with all ten Norito layouts. The
sole `FastpqBalanceKeyV1` canonical frame replaces display-text key material;
it binds the full asset identity and account controller. All 11 Core integration
tests passed on that earlier source. The merged raw64 transcript now verifies
and regenerates identical bytes in its focused replay test; its 1,831,049 bytes
have SHA256 `bd53a7c7bcfdf4a1529e92e7466636b588f8a99dbce786b5241d900a76a76de2`.
It exceeds both the production 512-KiB cap and the AXT 1-MiB cap; its 2-MiB
diagnostic budget does not qualify production admission.
The coupled JSON CLI, Torii recovery batch producer and Core persisted-proof
writer now use the sole canonical public model frames and bounded encoders;
these corrections now pass the rebuilt JSON CLI's seven tests and Core's 116
selected FASTPQ tests. Torii recovery now passes all seven tests: its repaired
pagination fixture first proves that an unanchored sidecar is rejected, then
stores the exact canonical block before checking pagination and byte budgets.
The combined retry passes after the 14 CLI compilation repairs, with all 8,714
captured inputs unchanged. The current FASTPQ integration target passes 20 tests
with four diagnostic cases ignored; its separate raw transcript replay passes.

The earlier merged public `offline_compact` verifier used SHAKE256 despite its
48-byte carrier. The current six-lane owner replaces that implementation and its
prototype selectors; the current build and execution evidence below supersede
that source finding. Production ingress still rejects compact frames, and
offline mathematical verification supplies no authenticated ledger authority.
Resource limits, witness privacy and independent security review remain open.
The original affected-source inventory is retained in ignored
`target/privacy-release-evidence/2026-09-07-recovery/merged-privacy-audit/fastpq-hash-cutover-open.json`.
The current compact preflight requires at least 375 complete 342-field rows and
375 pairs of Fp4 mixed/quotient values. Their raw values alone require
`375 * (342 * 8 + 2 * 32) = 1,050,000` bytes, exceeding even the AXT 1-MiB cap
before indices, frames, roots, frontiers or FRI data. The six-lane hash cutover
therefore cannot by itself close the production proof-size requirement.

A fresh merged-source independent Python/Metal diagnostic compiles the production
six-lane kernel and passes all 204 output words across 23 canonical frames,
four arithmetic boundary frames and seven invalid descriptors on the M1 Ultra.
Twelve extra dispatched threads preserve the 96-byte output guard. Source
hashes remain unchanged throughout the run; input, shader, compiler, executable,
output and log hashes are retained under
`target/privacy-release-evidence/2026-09-07-recovery/metal-merged-20260908/`.
The evidence manifest SHA256 is
`7ac3687cf29d14f4d6ce194f30630006fe9eb6bfd217a4a618d68dbe5b1b6c5b`.
This run corrects the old diagnostic's repeated field tag: three multi-field
cases in `metal-current/` proved arbitrary-wordstream parity, not canonical
framing. The original evidence is retained with an explicit correction record;
the new encoder uses tag `12 + field_index` and checks the complete reference KAT.
Separately, the pre-merge captured Metal-enabled Rust prover passed 68 device/runtime
tests: ten digest tests, two native Merkle tests and 56 Metal runtime tests,
including staging, allocation, cleanup and injected failures. The executable
SHA256 is `dd5b547358846978f03ea350afd9105173a5ce80abf2942f6a70ac1678490b2c`.
Neither selection qualifies a complete GPU proof, CUDA hardware or the release
qualification workflow.

The merged native capture also passes all three Core Kaigi integration tests
and all 21 Rust JS-host Kaigi tests, including real proofs and the 31/25-row
mutation controls. The Kaigi unit suite now passes all 97 tests after four
sample-account calls use the domain's name as the single-label seed; domain
state and all existing assertions are retained. Its earlier typed capacity-error
assertion also passes. All 29 restored Torii
MCP security tests and both Rust JNI account-controller tests pass. These are
Rust bridge tests; installed JNI and addon packages remain unqualified.
Two explicitly enabled Metal continuation tests execute the actual device on
heterogeneous frames and independent reference vectors; both pass. Every run
retains unchanged source and binary hashes under
`target/privacy-release-evidence/2026-09-07-recovery/core-retry-5/`;
the repaired Kaigi and Torii results are in `core-retry-6/`. The latter combined
build passes in 135 seconds with no source drift; the 97-test and seven-test
runs finish in 279 seconds and 17 seconds, respectively. Previous failed
captures remain available and do not count as passes.

The seventh combined capture passes in 136.49 seconds with all 8,715 inputs
unchanged. Core FASTPQ passes 116 tests in 19.83 seconds and Torii passes seven
in 9.12 seconds against the rebuilt storage code; both retain unchanged source
and binary hashes. SoraFS passes 95 focused and 1,468 ordinary tests, followed
by both explicitly enabled real local Kubo tests. These cover canonical PoR
outbox publication and authenticated IPNS pin recovery, with test-only signer
providers; they do not qualify regional deployment or hardware custody.
All five storage accounting regressions pass on the eighth capture, which builds
in 86.52 seconds with no source drift. Its only two Rust changes correct fixture
setup: establish configured lane markers before publishing the baseline and bind
the signed payload's exact incarnation before publishing its catalog journal.
Real authority checks and all assertions remain intact; the failed seventh-run
records are preserved. The JavaScript extraction passes 167 focused tests, 19
bundle tests, nine SoraFS archive tests and two native-absence tests. Bundle and
lint gates pass, all 91 browser exports remain present, and the instruction-builder
source budget is reduced from 5,803 to its current 5,353 lines. These structural
and absence results do not qualify an installed native addon.

The captured four-validator harness compiles, but its first runtime attempt
stopped before peer startup on a stale X509 status expectation: the current
profile reports `ProfileInitializationFailed` for invalid shared STARK
geometry, rather than `EngineUnavailable`. This is not governance availability
or four-validator execution evidence. The per-profile assertions now track
that exact diagnostic and the harness uses explicit 32-MiB parent and worker
stacks; those revisions still await compilation and execution.

## Current compact execution evidence

The 2026-09-08 coordinated locked/offline build passes all 16 selected test
artifacts without source drift. All 27 exact pre-proof checks pass, covering
fixed geometry, six-lane context/tape known answers, canonical coordinates,
permanent sampler abort, explicit decode budgets and unchanged AIR equations.
The primitive library passes 60 tests (one timing diagnostic ignored), and the
ordinary offline-consumer target passes all eight tests. These are scoped local
runs, not a complete workspace or release qualification.

The actual ordinary and AXT full-domain quantity-transfer proof tests both pass.
Retained proofs contain 3,994,619 and 4,015,551 bytes; proving takes 295.71 and
329.91 seconds, bounded verification 23.84 and 22.82 seconds, and each run peaks
at about 2.49 GB resident memory. Both verify 375 AIR openings and one terminal
degree check after private data is dropped; altered quantities, authority,
identity, truncation and allocation limits are rejected. Source and executable
hashes remain unchanged. The actual ordinary two-segment bundle also passes
with a separately retained 7,986,384-byte artifact: proving takes 608.46 seconds,
verification 43.84 seconds, and peak resident memory is 2.57 GB. Both child
transcripts, 750 AIR openings and two terminal degree checks are verified under
the bundle's own identity. The separate AXT bundle passes with 8,011,999 bytes,
560.55 seconds proving, 45.99 seconds verifying and 2.57 GB peak resident memory.
It checks its own two transcripts, 750 AIR openings and two terminal degree
checks. Fresh transport fixtures remain pending; single proofs cannot substitute
for bundle-bound artifacts.
The complete-row format exceeds the 512 KiB production and 1 MiB AXT limits:
even the necessary raw row openings account for 1,050,000 bytes. The default
32 MiB decode allocation budget is unchanged; a synthetic largest-shape test
uses an explicit 64 MiB diagnostic scope and proves default-budget rejection.
Neither that scope nor mathematical profile checks authorize production use.

The three mathematical checker entry points now share 28 exact executable,
specification and implementation inputs, with pre/post equality and seven
changed/omitted-input controls. Typed, all 128 bundle-size and historical theorem
checks pass privately; the live typed check reproduces the identical report.
Independent comparison preserves the reviewed arithmetic. These are conditional
model calculations and source provenance, not concrete cryptographic qualification.

Four saved-proof regression tests now pass in 136.85 seconds, checking each
retained single/bundle hash, canonical frame, ordered roots and exact work. They
also require precise rejection at the unchanged production byte/allocation
limits. Their individually emitted binary comes from the failed expanded
SoraFS build; the overall compilation failure remains recorded separately.

The final ordinary and AXT context owners each pass 1,635 actual Metal hash
comparisons, including all six lanes, every oracle level and all 931 field-tape
blocks. Three descriptor, thread-isolation and cleanup controls also pass.
The M1 Ultra runs take 9.63 and 15.00 seconds with 28.9 and 29.9 MB peak RSS.
Inputs are deterministic full-width payloads under the real statement contexts;
these are hash parity tests, not complete GPU proof or privacy qualification.
Both source and executable remain unchanged during execution. Evidence is in
`core-retry-14/retained-kats-result.json` and
`core-retry-15/compact-metal-result.json` under the recovery evidence directory.

The new [Kaigi lifecycle gate](kaigi_four_validator_lifecycle_v1.md) submits real
governed-key, authorization and usage instructions to four validators and checks
typed rejection, retained participation and two cold restarts. It is implemented
but the network harness and ordinary daemon still await a successful composed
build and execution. The retained Core fixture test passes with both governed
keys, all five generated authorization/usage proofs, and the malformed-envelope
control. This is local verifier evidence, not a deployment result.
## Subsequent composed checks, 2026-09-08

Combined retry 18 ends with two denied trait-object casts in a Torii test fixture
after 702.22 seconds. All 8,809 captured native inputs remain unchanged, and 27
test artifacts plus six ordinary runtime companions are retained. The narrow
cast correction is applied but has not yet compiled. The captured Core/config
selection passes 197 tests and fails 11 resource-accounting cases; all failures
remain recorded for correction. This is not a successful combined build.

The same capture passes all 11 Metadata tests, including canonical roundtrips,
duplicate rejection and modeled tree-allocation admission. All 20 composed
incentives tests pass. The real Kaigi fixture test passes in 94.57 seconds.
Each selection retains its exact original and copied executable hashes and
unchanged captured source. Evidence is under `core-retry-18/`, the SoraFS
`hardware-stream-token-native-retry18/` directory, and the multilane
`core-kura-direct-208-retry18-v2/` directory. Subsequent source changes require
fresh affected native execution.

The applied JVM correction migrates the old bridge test into a Java consumer of
the canonical Kotlin API, checks exact compiled JNI declarations without SDK
reflection, and uses explicit JDK tools without replacing the authenticated
Cargo environment. Its 126 source/negative controls pass in 65.74 seconds.
The earlier frozen native phase built JNI and passed 129 Kotlin-selected tests
plus seven retained Java tests, but the whole phase failed on the retired Java
consumer and cleanup environment. Those failures remain failures. Source guards
and Java compilation cannot replace the fresh complete native qualification.

A private, unregistered scalar-PCS reference slice passes 13 arithmetic/transcript
tests plus 11 unchanged field tests and 36 independent Fp4 vectors. The bounded
mathematical review reproduces the sealed audit and dual-basis controls. Full
partial-word AIR/PCS composition, malformed unopened leaves, masked AIR degree,
generic-witness privacy, native codec/resources and concrete hash qualification
remain open. The reference slice is not a shipping verifier or an alternate V1
proof path, and its projected byte counts are not measured production proofs.


Combined retry 19 ends with the daemon's denied inline-state size lint after
523.54 seconds, with all 8,812 inputs unchanged. Its Core/configuration selection
passes 206 tests and fails two signed-snapshot finalizer cases at static-policy
authentication; its Manifest selection passes 957 and fails two independent
wire-oracle cases. Those original failures and artifacts remain immutable.

Retry 20 passes that daemon lint boundary and stops at newly reached CLI, daemon
test and integration-client compilation errors after 141.65 seconds. All 8,820
native source inputs remain unchanged; 32 test binaries and seven ordinary
companions are retained. The individually emitted Core binary passes both
corrected signed-snapshot finalizer tests and the existing wrong-signature
negative. The other 206 Core/configuration results remain retry-19 evidence.
The selected BFV audited-prover test fails before the modified mode helper because
its shared bootstrap fixture still requested two refresh rounds against the
mandatory limit of one. The reviewed paired fixture correction is applied, and
both direct mode-helper tests pass in retry 21 alongside two Kaigi reason/proof
controls. The original audited full-execution fixture still requires production
qualification; the direct controls do not exercise a complete bootstrap.

The full Manifest selection now passes all 959 tests, including every corrected
prepared-window oracle and missing-field negative. Torii's complete 33-test
hardware-token selection also passes. Original and retained binaries and the full
captured source stay unchanged throughout these executions. See `core-retry-20/`,
`core-retry-20/core-policy-tests/`, and the
[SoraFS checkpoint](sorafs/v1_closure_ledger.md). These 992 executions retain
retry-20 provenance; matching later binary hashes are not new test executions.

Retry 21 passes the complete selected native build, retaining 37 test binaries
and seven companions with all 8,821 source inputs unchanged. Its CLI cohort
finishes 41 passed and 16 failed; the daemon stream-token cohort finishes 57
passed and five failed. The reviewed retry-22 correction fixes the unique Clap
group, actual metadata-value sizing and canonical decoder allocation policy,
valid gateway-role acceptance, blocking socket fixture and issuer trust interval.
Retry 22's same selected build passes in 35.51 seconds, with all 8,821 inputs
unchanged and all 44 executables retained. Its exact CLI selection passes all
61 tests, including four additive workload boundaries; its exact daemon
stream-token selection passes all 62 tests, including all five earlier failures
and twelve new gateway mutations inside the original positive. Earlier failures
remain retained under their own attempts.

The ordinary shipping-feature daemon also builds successfully from that same
retry-22 source in 31.76 seconds, without nonshipping verifier helpers. The first
Kaigi attempt in retry 21 fails before any daemon launches because the test
repeats Alice's baseline read permission. After removing only that duplicate,
the retry-22 exact ignored lifecycle test starts four validators but fails the
test-owned genesis startup deadline after 267.04 seconds. Nodes exchange Timeout
and Prepare certificates, but block one does not commit; no Kaigi lifecycle
transaction executes. The source, harness and daemon bytes remain unchanged,
and 256 private peer artifacts are retained with hashes. The production cause
remains under investigation; no timeout, quorum or RS16 requirement is waived.
See `core-retry-22/`, `daemon-retry-22/` and
`deployment/kaigi-r22-runtime/failure-review.json` in the recovery evidence.
Neither the successful unit cohorts nor this failed network run qualify deployment.

The applied JVM owner-retirement packet removes duplicate Java privacy owners
and nineteen duplicate Android privacy JNI exports, retaining canonical Kotlin
consumers and all original assertions. Its 176 live source controls pass; the
previous failed frozen native phase still requires a fresh final-source replay.
The reviewed orphan confidential-witness producer retirement is also applied.
Independent controls verify compiled Swift test-source discovery, fail-closed
directory traversal and all shipping Swift source targets. Legacy encoder
assertions remain in test-only fixtures. A new isolated capture contains 19,462
source entries; all 284 source controls pass with zero capture drift. Its original
Cargo provisioning verifies, and the complete prepared JNI phase builds a fresh
native library and passes the exact eighteen-class Kotlin/Java selection's 152
JUnit tests plus seven Java proof-attachment consumer tests. The standalone Java
utility separately passes all fifteen groups with assertions enabled. All 159
JUnit cases have zero failures, errors or skips. Independent inspection binds
all 2,531 shipping JAR classes to the actual class-contract output, verifies
JDK-8 class versions and excludes retired producers and test-only fixtures.
The frozen source and original provisioning/launcher/lock inputs remain intact;
135 retained review artifacts include the native library, JAR and all twenty
JUnit XML reports. See `sdk20-jvm-postvalidation-review/review-capsule.json`.
All six original release gates remain unmet: two reject dirty source and four
lack qualified manifests. Full Swift, physical Android and signed-source release
qualification remain open; these host JNI executions do not establish them.

The private PCS fold and full quadratic-sumcheck operators pass an independent
recompile and all 36 arithmetic/field tests. Their coefficient and Boolean bases,
complete canonical words and terminal arithmetic are reviewed; authenticated
openings, full masked AIR, source/compiler mapping and a private-witness simulator
remain unfinished. A separate conditional partial-word state argument and finite
adversarial controls now make the outer AIR-to-PCS handoff explicit. Neither
private packet is registered as a proof-acceptance or fallback path. BFV review
confirms that exact/refresh owners still require a full chain-native replacement;
the applied internal mandatory-mode cleanup removes only a dead retry branch.
