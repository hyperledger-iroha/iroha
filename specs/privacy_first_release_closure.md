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
| ZK-ACE | Dedicated masked STARK; six identity lanes and six replay lanes, 8x LDE, Fp4 FRI and 136 distinct queries. Activation unavailable. | Verify corrected AIR next-row stride and witness erasure; complete qROM reduction, multi-target accounting, independent implementation review and final artifacts. |
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
| Generic native STARK | Explicit initial degree/verified blowup geometry and authenticated bounded-degree terminal polynomial for private AIR. Binary fold consistency alone does not prove this. Unsupported BFV public-padding verification remains unavailable. |
| FASTPQ | Resolve the single verifier architecture: a justified bounded-opening quotient/degree proof or an explicit full-replay artifact. Complete protocol-specific qROM analysis and independently reproduce the permutation and six-lane construction. |
| AXT | Bind exact finalized/QC source state and transaction set, issuer intent, proof and effective amount before admitting remote spends. |
| BFV/Soracloud and MKHE | Full BFV-RNS and one atomic 40-limb source/materialization/packing/cross-field/padding verifier, full-size/eight-party KAT, resource measurements and governed noise/qROM evidence. Unavailable stages cannot issue receipts. |
| Confidential assets and private settlement | Regenerated canonical proofs/keys, authority/amount/conservation adversaries, complete SDK routes and deployment evidence described by the owning settlement/asset specifications. |
| SoraFS | Complete the V1 closure ledger, live four-voter/multi-provider/dual-gateway L1, resilience/load/24-hour soak, all 17 summaries and ordered L2 promotion evidence. |
| Kaigi | Signed participant-authority/replay-bound roster proof, exact keys/schema, suite-tagged HPKE, bounded long-session accounting and authenticated relay recovery. |
| Elections | Complete private ballot/deadline/retry, finalized-beacon, rollback/restore and independent timed-OVN/threshold-BLS review on four validators. |
| SDKs and fixtures | Rust, Kotlin/Java consumers, Swift, JavaScript, Python and C# use the same final canonical bytes and native admission; signed same-source native packages and target-platform execution. Structural parser/source tests alone cannot qualify an SDK. |
| Hardware | Compile native Metal/CUDA and execute CPU/NEON/SIMD/Metal/CUDA parity, fault quarantine, side-channel and measured memory/throughput on qualified hardware. A feature build is not device execution. |
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
