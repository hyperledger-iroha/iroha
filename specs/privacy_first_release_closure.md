# First-release privacy and ZK closure

This ledger records the implementation and qualification work for the final V1
privacy stack. It is not an audit certificate or permission to activate an
unqualified protocol. Source observations below were checked on 2026-09-06 in a
shared working tree; they do not identify a sealed release candidate.

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
| FASTPQ | Resolve the single verifier architecture: a justified bounded-opening quotient/degree proof or an explicit full-replay artifact. Complete protocol-specific qROM analysis and independently reproduce the permutation and six-lane construction. |
| AXT | Both Core execution pipelines now commit exact ordered canonical transaction wires; missing block-owned commitments cannot be synthesized from transcript identities. Anchor-bound proof verification checks ordered wire membership, exact public roots and context, and mandatory expiry. Consensus witness roots and transfer-batch trees still commit subsets, which cannot substitute for full persisted WSV roots. Complete successful-execution/transfer binding, rooted state witnesses, immutable anchor resolution and durable spend nonces. |
| BFV/Soracloud and MKHE | Full BFV-RNS and one atomic 40-limb source/materialization/packing/cross-field/padding verifier, full-size/eight-party KAT, resource measurements and governed noise/qROM evidence. Unavailable stages cannot issue receipts. |
| Confidential assets and private settlement | Regenerated canonical proofs/keys, authority/amount/conservation adversaries, complete SDK routes and deployment evidence described by the owning settlement/asset specifications. |
| SoraFS | Complete the V1 closure ledger, live four-voter/multi-provider/dual-gateway L1, resilience/load/24-hour soak, all 17 summaries and ordered L2 promotion evidence. |
| Kaigi | Signed participant-authority/replay-bound roster proof, exact keys/schema, suite-tagged HPKE, bounded long-session accounting and authenticated relay recovery. |
| Elections | Complete private ballot/deadline/retry, finalized-beacon, rollback/restore and independent timed-OVN/threshold-BLS review on four validators. |
| SDKs and fixtures | Rust, Kotlin/Java consumers, Swift, JavaScript, Python and C# use the same final canonical bytes and native admission; signed same-source native packages and target-platform execution. Structural parser/source tests alone cannot qualify an SDK. |
| Hardware | Final FASTPQ six-lane hashes and polynomial derivation currently execute on CPU. Old scalar-permutation/FFT preflights cannot qualify final V1 GPU proofs. The allocation-free typed frame now shares CPU, streaming and hardware input framing. Dedicated Metal digest dispatch and cleanup/quarantine tests pass locally; CUDA compilation/device evidence and full proof routing remain outstanding. Complete real mode propagation; then execute CPU/NEON/SIMD/Metal/CUDA parity, fault quarantine and measured memory/throughput. A feature build or selected mode is not device execution. |
| Release and deployment | Clean signed source/lock/toolchain identity, complete independent audit classes and finding dispositions, real 48-stage/54-artifact evidence, exact four-validator quorum, staged restart/canary/convergence and authenticated endpoint readback. |

## Verification discipline

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
- SDK source admission checks pass 86 regressions, including immutable origin,
  exact expected-network propagation, one-use JS transport receipts, Kotlin
  wire-payload snapshots, and final Swift envelope/cache controls. The strict
  audit retains its explicit source-prerequisite evidence label.
- The subsequent SDK authority review found offline-archive promotion in Kotlin,
  Python and the JS admission helper, plus missing deployment-network comparisons
  in managed SDK projections. The final origin/network fixes now bind admission to configured client networks
  and reject offline inspection objects at construction. Focused tests pass:
  JavaScript 37, Kotlin 63, Python 103, C# 17 and Swift 35. Swift also rejects
  the retired 11-field envelope and checks all twelve final statement contexts.
  C# passes 11 source/native-validator mutation controls. Swift execution used
  an isolated source-only package; native packaging remains unqualified.
  Kotlin captures caller-provided wire payloads before admission and encoding;
  Swift capability requests explicitly bypass caching. Native JNI/PyO3/N-API
  test rebuilds remain in progress after stale test/import owners were corrected.
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
  eight-chunk membership KAT is running; CPK relation linkage remains unavailable.
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
bridge source is corrected. A fresh complete five-target build is running from
immutable overlay manifest `c1e13661ade2148661fa620425a66846cd18af41ec31a53b7ab2043df5c37983`.
It precedes the subsequent Swift request-cache change and Python dependency
owner correction; it cannot qualify the later moving checkout.

The previous complete optimized proof-crate run finished with 1,294 passes,
four failures and 13 explicitly ignored resource/generator tests. All four
failures now pass focused reruns (ten section-codec and three additional MKHE
cases). The corrected complete proof-crate run passes 1,299 tests with zero
failures and 13 explicitly ignored tests in 3,085.08 seconds. Nine of the
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
