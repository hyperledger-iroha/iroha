# Iroha ZK Cryptographic Audit

Date: 2026-08-23

This report audits Iroha-owned zero-knowledge verifier code and proof-bearing runtime
integrations. Vendored Halo2, curve, hash, encoding, and arithmetic libraries are
treated as dependency assumptions. The audited surfaces are native STARK/FRI,
ZK-ACE AIR, Iroha-owned IPA/Halo2 wrappers, verifier registry policy, proof
envelopes, Torii proof endpoints, IVM host syscalls, Kaigi privacy flows, and FASTPQ
lane proof binding.

## Executive Summary

Normal ledger-grade ZK admission is designed to fail closed. A
registry-dispatched proof must bind a registered verifying key that is active at
the execution block height, backend label, circuit id, schema/public-input
commitment, verifying-key hash, and proof bytes. A typed privacy proof instead binds an exact compiled protocol
activation, statement schema, verifier and engine digests, signed transaction
intent, and proof bytes. Both surfaces reject unsupported or substituted
artifacts before verifier dispatch.

The native STARK/FRI verifier performs canonical Goldilocks field checks,
Merkle opening checks, Fiat--Shamir query derivation, FRI folding checks, and
AIR composition checks. Generic circuits bind `OpenVerifyEnvelope` metadata.
The separate typed `PrivacyProofEnvelopeV1` path adds compiled-profile,
governed-policy, signed-transaction-intent, trusted-genesis, transfer, and
replay-nullifier binding. ZK-ACE activation is nevertheless unavailable: its
current four-word public commitment has only a one-field, roughly 32-bit
generic collision ceiling.

The Iroha-owned IPA/Halo2 stack is a transparent IPA verifier wrapper. Production
IPA bases are independently mapped with domain-separated hash-to-curve; the
additive Goldilocks compatibility backend is rejected by runtime verification.
This audit does not source-audit vendored Halo2 or curve crates; it audits Iroha's
generator derivation, transcript labels, public-input shape checks, metadata
binding, registry limits, batch dispatch, and runtime guardrails.

FASTPQ lane admission binds the proof to the lane envelope and AXT claim metadata:
dataspace, manifest root, source transaction commitment, effect type, claim digest,
optional committed amount, batch seal, transfer witnesses, public I/O, transcript
challenges, AIR openings, Merkle paths, FRI query chains, and proof-size limits.

The principal audit risk is boundary management. Recovery-only trust paths and
diagnostic endpoints must never be usable as fresh ledger-admission proof. This report
records that as the primary finding and models the same class of failure in TLA+.

## Scope and Evidence

Audited code evidence:

- [../crates/iroha_data_model/src/zk.rs](../crates/iroha_data_model/src/zk.rs):
  `OpenVerifyEnvelope`, generic STARK wrapper payloads,
  `ZkAcePrivacyPublicInputsV1`, replay-nullifier derivation, and canonical
  public-input hashing.
- [../crates/iroha_data_model/src/privacy.rs](../crates/iroha_data_model/src/privacy.rs):
  the closed privacy protocol registry, compiled-artifact bindings, governed
  ZK-ACE policy records, typed statements, and proof envelopes.
- [../crates/iroha_data_model/src/proof.rs](../crates/iroha_data_model/src/proof.rs):
  `ProofBox`, `ProofAttachment`, `VerifyingKeyBox`, `VerifyingKeyRecord`, key status,
  and backend/commitment serialization policy.
- [../crates/iroha_core/src/zk.rs](../crates/iroha_core/src/zk.rs): verifier
  dispatch, preverify/dedup, backend-label guardrails, envelope metadata checks,
  STARK/Halo2 entry points, and timing/size guardrails.
- [../crates/iroha_core/src/zk_stark.rs](../crates/iroha_core/src/zk_stark.rs):
  generic native Goldilocks STARK/FRI verifier and AIR bindings; the generic
  boundary explicitly rejects the retired ZK-ACE relation.
- [../crates/iroha_core/src/privacy_engines/zk_ace.rs](../crates/iroha_core/src/privacy_engines/zk_ace.rs)
  and
  [../crates/iroha_core/src/privacy_engines/zk_ace_stark.rs](../crates/iroha_core/src/privacy_engines/zk_ace_stark.rs):
  the private zeroizing witness, compiled profile, dedicated masked AIR,
  theorem-bound DEEP/FRI prover, and native verifier.
- [../crates/iroha_core/src/privacy_engines/transparent_stark.rs](../crates/iroha_core/src/privacy_engines/transparent_stark.rs),
  [../crates/iroha_core/src/privacy_engines/aggregate_stark.rs](../crates/iroha_core/src/privacy_engines/aggregate_stark.rs),
  and
  [../crates/iroha_core/src/privacy_engines/proof_managed_note_stark.rs](../crates/iroha_core/src/privacy_engines/proof_managed_note_stark.rs):
  shared field, transcript, Merkle, aggregate DEEP/FRI, exact proof-codec, and
  proof-managed relation-profile boundaries.
- [../crates/iroha_zkp_halo2/src/lib.rs](../crates/iroha_zkp_halo2/src/lib.rs):
  IPA verifier wrapper, generator derivation, envelope decoding, limits, and batch
  verification API.
- [../crates/zk_ace_prover/src/lib.rs](../crates/zk_ace_prover/src/lib.rs):
  governed transfer construction, typed proof-envelope assembly, exact
  transaction-intent binding, and signed `SubmitPrivacyProofV1` creation.
- [../crates/iroha_core/src/smartcontracts/isi/privacy.rs](../crates/iroha_core/src/smartcontracts/isi/privacy.rs):
  privacy activation and ZK-ACE policy governance, typed proof verification,
  atomic transfer effects, and replay-nullifier consumption.
- [../crates/iroha_core/src/smartcontracts/isi/world.rs](../crates/iroha_core/src/smartcontracts/isi/world.rs):
  generic verifying-key registry policy, `VerifyProof`, governance proof
  checks, and FASTPQ lane relay admission.
- [../crates/iroha_core/src/smartcontracts/ivm/host.rs](../crates/iroha_core/src/smartcontracts/ivm/host.rs):
  IVM VK loading, envelope enforcement, verifier syscalls, and batch verification.
- [../crates/iroha_core/src/smartcontracts/isi/kaigi/privacy.rs](../crates/iroha_core/src/smartcontracts/isi/kaigi/privacy.rs):
  Kaigi privacy proof metadata and roster-root verification.
- [../crates/iroha_torii/src/lib.rs](../crates/iroha_torii/src/lib.rs),
  [../crates/iroha_torii/src/zk_attachments.rs](../crates/iroha_torii/src/zk_attachments.rs),
  and [../crates/iroha_torii/src/zk_prover.rs](../crates/iroha_torii/src/zk_prover.rs):
  app-facing proof submission, verification, storage, and non-consensus worker
  boundaries.
- [../crates/fastpq_prover/src/proof.rs](../crates/fastpq_prover/src/proof.rs),
  [../crates/fastpq_prover/src/axt_binding.rs](../crates/fastpq_prover/src/axt_binding.rs),
  and [../crates/iroha_data_model/src/fastpq.rs](../crates/iroha_data_model/src/fastpq.rs):
  FASTPQ public I/O, transcript arithmetic, AIR/FRI verification, AXT packaging,
  lane claim binding, and transfer transcripts.

Out of scope: source audit of vendored Halo2, curve, finite-field, SHA-2, Blake2,
Poseidon2, and Norito implementations; full algebraic proof of STARK/FRI or IPA
soundness; runtime API, schema, or wire-format changes.

## Findings

### ZK-AUDIT-01: Recovery trust flag can bypass local ZK-ACE verifier failure

Severity: High if reachable during fresh transaction admission; otherwise a
replay-boundary risk.

Status: Remediated. ZK-ACE authorization now rejects local verifier failure even
when committed-result trust is set; the flag may log the replay condition but no
longer authorizes transfer execution, replay-nullifier consumption, or balance
movement for a failed ZK-ACE proof.

Evidence: the direct `SubmitZkAceAuthorizedTransfer` wire is retired. The
canonical `zk_ace_prover` path does not expose a caller-selected backend,
verifier key, proof attachment, or generic `OpenVerifyEnvelope`, and now fails
with `CompiledProfileUnavailable` before proof construction. If the candidate
is eventually requalified, `SubmitPrivacyProofV1` execution in
[../crates/iroha_core/src/smartcontracts/isi/privacy.rs](../crates/iroha_core/src/smartcontracts/isi/privacy.rs)
validates the signed transaction intent, compiled activation, governed policy,
native proof, and replay state before committing the transfer effects. The
native verifier does not consult committed-result trust and currently returns
`EngineUnavailable` before proof parsing.

Impact: the original bypass could authorize a transparent transfer and consume a
replay nullifier if the flag were enabled during new block production, fresh
transaction admission, or uncommitted ledger execution. The remediated ZK-ACE path
no longer depends on that operational boundary.

Regression coverage:
`zk_ace_production_dispatch_has_no_activatable_profile` and
`zk_ace_submit_has_no_activatable_compiled_profile` pin verifier and state
transition fail-closure. The four-peer exact-12 lifecycle tests also reject
ZK-ACE activation and proof submission before and after restart.

Residual recommendation: keep `trust_committed_execution_results` restricted to
committed-block replay/recovery contexts for all other proof-bearing flows, and avoid
adding new proof-authorizing bypasses.

### ZK-AUDIT-02: Diagnostic proof endpoints require an explicit non-ledger contract

Severity: Medium.

Status: Closed. The pre-release decode-only `/v1/zk/verify` and
`/v1/zk/submit-proof` routes, their Rust client helpers, and their CLI commands were
removed. Ledger proof records can be created only by a signed transaction containing
`VerifyProof` or another proof-bearing instruction that reaches the guarded core
verifier.

Evidence: the remaining `POST /v1/zk/verify-batch` route performs bounded
cryptographic verification of its standalone IPA diagnostic format and is documented
as non-ledger-equivalent. Attachment storage and the background prover worker remain
report-only and do not return a ledger-acceptance result.

The standalone IPA envelope no longer accepts inline commitment bases. Its
`IpaParams` field is only a `(version, curve, n)` selector; the verifier derives
the sole V1 generator set for that selector and binds the complete derived
parameter fingerprint into the transcript before producing challenges. Retired
wire layouts carrying `g`, `h`, or `u` are non-canonical.

Regression coverage: `zk_subrouter_smoke` asserts that both retired routes return
`404 Not Found`; the existing `zk_verify_batch_*` integration suites cover the
remaining bounded diagnostic verifier.

### ZK-AUDIT-03: ZK-ACE v0 commitment does not meet its compiled security target

Severity: Critical if activated; fail-closed in the current tree.

Status: Disabled pending redesign. The algebraic candidate uses a 4,096-row
masked trace, 65,536-row low-degree extension, quartic Goldilocks challenges,
108 unique FRI queries, binary folding and SHA-256 Merkle paths. Its public
commitment, however, exposes four sequential `state[0]` values from one
rate-two, capacity-one Goldilocks sponge. Multiple outputs do not raise the
generic binding above the one-field capacity, so the end-to-end profile cannot
inherit the STARK layer's theorem-derived 128-bit classical-ROM bound.

`ZK_ACE_FULL_ENGINE_AVAILABLE_V1` is therefore false. Proving, verification,
and compiled-profile activation return `EngineUnavailable` before processing
proof material. The candidate relation and fixed 1,341,142-byte proof wire stay
testable, but are not an executable privacy protocol.

Required remediation: derive the four commitment words from independent
domain-separated invocations, or replace the commitment with a construction
having at least 128-bit collision binding. The resulting wider AIR schedule,
trace domain, masking geometry, FRI profile, profile digest, fixtures, and
security certificate must all be regenerated and requalified before activation.

### ZK-AUDIT-04: BN254/Halo2 naming must remain segregated from ledger-grade IPA policy

Severity: Low.

Evidence: `iroha_zkp_halo2` has raw decoding/verification support for multiple curve
identifiers, while registry/runtime policy rejects trusted-setup and non-IPA labels.

Recommendation: keep production backend identifiers narrow and explicit, for example
`halo2/ipa/pallas`; continue rejecting KZG/Groth16/SRS/PTAU-style labels in registry
and runtime dispatch.

### ZK-AUDIT-05: Fixed binding-AIR residual weights admitted a public kernel

Severity: High for generic native STARK metadata binding.

Status: Remediated. The generic binding AIR previously compressed twelve row
residuals with the public fixed coefficients `3, 5, ..., 25`. A malicious prover
could add the non-zero residual vector `(1, -2, 1)` to both opened rows while
preserving a zero composition value because `3 - 2·5 + 7 = 0` and the analogous
next-row coefficients also cancel.

The verifier now compares every coordinate of each transcript-sampled current
and next row with the verifier-owned deterministic binding row. Sampling alone
would still miss a sparse mutation with high probability, so the verifier also
reconstructs the canonical public trace root with a streaming Merkle accumulator
and requires exact equality. It likewise reconstructs the all-zero composition
tree root and matches it exactly; a sparse nonzero composition layer can no
longer pass merely because every sampled opening is zero. Generic binding proofs
and verifying keys are capped at `n_log2 = 12`; larger domains fail closed
instead of forcing unbounded verification work. The regression
`binding_air_rejects_fixed_coefficient_cancellation_rows` constructs the former
collision, while `binding_air_rejects_unsampled_row_via_exact_trace_root` places
a mutation outside every sampled current/successor opening and requires the
exact-root check to reject it. A separate sparse-composition regression pins the
exact zero-root rule.

### ZK-AUDIT-06: FASTPQ accepted alternate Goldilocks representatives

Severity: Medium; proof malleability and canonical-wire boundary failure.

Status: Remediated. FASTPQ proof fields are serialized as `u64` or 32-byte
field containers, while field arithmetic and Poseidon reduce modulo the
Goldilocks prime. The verifier could therefore treat a Merkle sibling encoded as
`p` as the same field value as canonical zero.

After resource limits, and before semantic or transcript work, verification now
rejects every noncanonical proof-carried field scalar, Merkle sibling, AIR/FRI
opening, Poseidon root, and permission-field hash. Opaque public hash bytes are
not misclassified as field elements. Public and container-completeness
regressions pin the preflight and the `value < p` 32-byte decoder rule.

### ZK-AUDIT-07: BN254 radix-2 transforms omitted input bit reversal

Severity: High for BN254 FFT/LDE correctness.

Status: Remediated in the CPU, CUDA, and Metal paths. Their iterative
decimation-in-time butterflies consumed coefficient-order input without first
applying the required bit-reversal permutation, so even a degree-one polynomial
produced the wrong evaluation vector. GPU parity could not expose the error
because its CPU reference implemented the same ordering bug.

All three transforms now bit-reverse after canonical-to-Montgomery conversion
and coset scaling, before the butterfly stages. CPU regressions compare FFT and
coset-LDE output with independent direct Horner evaluation; Metal parity reuses
that tested oracle, and the CUDA benchmark reference follows the corrected
ordering. Hardware qualification remains part of release evidence.

### ZK-AUDIT-08: Native Poseidon Merkle paths admitted a noncanonical field alias

Severity: Medium; proof malleability and transcript ambiguity.

Status: Remediated. Native STARK Poseidon digests occupy one Goldilocks field
element in a 32-byte container. The decoder previously required only that the
upper 24 bytes were zero, so the low word `p` entered hashing as the same field
element as canonical zero. Digest decoding now also requires `value < p`.
Value-leaf and AIR-row Merkle regressions authenticate a zero sibling and reject
the otherwise equivalent sibling encoded as `p`. Because a one-field root has
only about 32 bits of generic collision binding, canonical ledger verifier keys
now require SHA-256. Hash selector `2` remains available only to the raw
compatibility verifier and is not ledger-grade.

### ZK-AUDIT-09: Native FRI used the wrong point for bit-reversed pairs

Severity: High for the claimed polynomial-fold relation.

Status: Fold equation remediated; degree-bound qualification remains open. FRI
layers store adjacent `(x, -x)` evaluations in bit-reversed order, but the prover
and verifier both used `x = omega^j`. They now use
`x = omega^bit_reverse(j, log2(layer_size) - 1)`. An independent `N = 8`,
`f(X) = X` regression proves that pair `j = 1` folds to `beta` at `omega^2` and
that the former `omega` calculation does not.

The generic native verifier still does not turn `blowup_log2` into an explicit
initial degree bound or authenticate a bounded-degree terminal polynomial.
Consequently this audit does not assign the advertised proximity/security claim
to private explicit-AIR generic proofs. The verifier-owned Binding profile is
separately protected by exact canonical trace-root reconstruction.

### ZK-AUDIT-10: FASTPQ FRI folded contiguous chunks without domain points

Severity: High for the claimed Reed--Solomon/low-degree argument.

Status: Fold and terminal-degree relation remediated; release security profile
remains open.
The previous fold `sum(y_j * beta^j)` neither opened multiplicative cosets nor
used their domain points. Proving and verification now open strided cosets
`i + k*m`, recover residue-class polynomials with the inverse subgroup DFT and
`x^-j` factors, evaluate them at `beta`, advance the generator and offset by the
round arity, and use a real smaller final subgroup instead of repeat-last
padding. The old schedule then folded all the way to one value, under which any
committed function has a valid terminal scalar and the degree check is vacuous.
The prover now stops with the complete terminal domain still present
(`2^19 -> ... -> 2` for balanced and `2^20 -> ... -> 2^4` for latency). The
verifier derives the conservative exclusive bound `2 * N_trace` from the
quadratic V1 residue ledger, reduces it alongside each fold, authenticates the
single terminal leaf, inverse-interpolates all terminal evaluations, and rejects
every coefficient at or above that bound. Independent constant/linear,
Lagrange-interpolation, schedule, and high-terminal-degree regressions pin both
parts of the relation.

The current FASTPQ verifier also reconstructs the complete batch trace, checks
its base constraints, and recomputes its trace/AIR commitments. That current
boundary prevents the FRI layer from becoming the sole semantic batch check.
A commitment/transcript design with at least 128-bit security and an independent
quantitative analysis of the implemented FRI profile remain release blockers.
The catalogue now honestly declares a base-field challenge, zero grinding, and
a 32-bit target instead of claiming unimplemented Fp2/grinding.

### ZK-AUDIT-11: Exact Halo2 production labels bypassed the outer envelope

Severity: High availability failure for registered production circuits.

Status: Remediated. Registry dispatch previously ran before the Halo2 outer
envelope handler. Exact IVM, Kaigi, transfer, top-up, and unshield labels were
therefore sent to the legacy raw decoder and rejected even when their canonical
`OpenVerifyEnvelope` was valid. Every production Pasta/IPA label now enters the
same authenticated outer-envelope boundary first. Valid-proof regressions cover
all seven exact labels. Production verification now accepts only the strict ZK1
inner carrier. The legacy binary carrier's caller-controlled `n_in`, `n_out`,
and lookup flags were not transcript-authenticated, so it has been retired from
production dispatch; unknown flag bits also remain rejected by its standalone
parser.

### ZK-AUDIT-12: IVM verifier helpers panicked or accepted impossible control flow

Severity: Medium; malformed-witness denial of service and verifier/runtime drift.

Status: Remediated. Constraint trace indices, secp256k1 scalars, heap allocation
arithmetic, memory-region limits, and instruction fetches now use checked
conversion/arithmetic and return verification errors rather than panicking or
truncating. VM trace fetch accepts only an aligned in-range instruction or the
exact end-of-code padding row. Jump and branch helpers preserve the runtime's
four-byte PC alignment class, and `JALR` applies the same relative `!3` mask as
runtime execution. No-panic and half-word-target regressions cover the former
paths.

### ZK-AUDIT-13: FASTPQ's `x^5` S-box was not a Goldilocks permutation

Severity: Critical for FASTPQ and every shared digest using that permutation.

Status: Algebraic collision removed; affected proof systems remain
unqualified. For the Goldilocks prime, `gcd(5, p - 1) = 5`, so `x -> x^5` is
five-to-one rather than a permutation. A chosen pair of distinct first sponge
words reaches the same post-S-box state and produces an identical hash. More
importantly, two valid ZK-ACE identity roots with the same nonzero blinding were
found to produce the same legacy identity commitment.

CPU, CUDA, Metal, public-data digests, and the ZK-ACE AIR now use `x^7`, for
which the exponent is coprime to `p - 1`. The current FASTPQ parameter versions
are `5` and `6`; regenerated domain roots/coset offsets and the coherent
trace/LDE relation prevent old proofs from being reinterpreted, and
known-answer plus legacy-collision regressions pin the new construction.
Descriptors now call it dense-MDS Goldilocks Poseidon `x^7`; it is not
Poseidon2.

This change removes the concrete non-permutation collision but does not upgrade
a one-Goldilocks-element root to 128-bit collision resistance. FASTPQ remains
release-blocked, and ZK-ACE remains unavailable under ZK-AUDIT-03.

### ZK-AUDIT-14: BFV public-padding sampling did not bind hidden trace columns

Severity: Critical if the public-only verifier is treated as an execution proof.

Status: Fail-closed. The BFV public-padding entry point authenticated sampled
public rows and composition openings but did not prove low degree for the
unobserved private trace columns. A prover could therefore decouple a hidden
trace from the zero composition values seen at sampled public rows.

Both public-padding-only entry points now reject unconditionally. The governed
full-material verifier retains the structural checks and then deterministically
reconstructs and exactly matches every trace row, composition value, trace root,
and composition root. Re-enabling public-only verification requires separately
committing every hidden trace polynomial, proving its degree bound, and binding
those commitments into the sampled composition relation.

### ZK-AUDIT-15: Secret STARK buffers used optimizable ordinary overwrites

Severity: Medium for prover-side witness confidentiality; malformed-input panic
was a low-severity availability issue.

Status: Remediated. Replayable trace masks, aggregate base/FP4 columns, and
proof-managed-note IFFT coefficients previously used ordinary assignment or
`fill` immediately before deallocation. An optimizer is allowed to remove such
dead stores. They now route each field limb through the `zeroize` crate's
hardened overwrite. The shared masked-trace helper also rejects noncanonical
mask residues before field subtraction, avoiding a debug underflow/panic on
malformed internal input.

### ZK-AUDIT-16: Halo2 outer schema bytes were not authenticated by generic dispatch

Severity: High metadata-substitution risk for direct production verifier calls.

Status: Remediated. Halo2 authenticates its concrete instance columns, but it
does not absorb the surrounding data-model `OpenVerifyEnvelope.public_inputs`
field. Generic `verify_backend` therefore accepted the same valid proof after
those nonempty outer schema bytes were replaced. Registry-owned call sites often
performed a separate schema-hash comparison, but the public production verifier
did not make that invariant universal.

All seven admitted production circuit ids now normalize through one closed map
to exactly one authoritative schema descriptor. Preverification, timing
guardrails, final verification, and strict verifying-key record preparation
require exact descriptor bytes or their Iroha-hash commitment. Generic and exact
backend aliases select the same entry, and unknown/unmapped circuits fail closed.
Valid IVM proof regressions mutate the outer schema under both generic and exact
labels; Kaigi exports its two owner-crate schema constants so fixtures and
registry policy cannot drift.

### ZK-AUDIT-17: Generic FRI challenges did not bind their round

Severity: Medium transcript-domain-separation weakness.

Status: Remediated. The generic native-STARK prover and verifier derived each
FRI beta from the parameter set, transcript label, and current layer root, but
omitted the layer number. Equal roots at two depths therefore reused the same
challenge. The shared derivation now absorbs the exact little-endian `u32`
round in proof construction, shape validation, and final verification. The
`fri_challenges_bind_the_exact_round` regression pins both determinism within a
round and separation across rounds.

### ZK-AUDIT-18: Aggregate domain validation omitted fixed core roles across layers

Severity: Medium for a misconfigured or future aggregate profile.

Status: Remediated. `AggregateStarkDomainsV1::validate` rejected collisions
among caller-supplied domains and fixed DEEP labels, but did not include the
three fixed FRI-mask leaf, node, and root labels in that uniqueness set. A
profile could therefore pass validation while aliasing masked and unmasked
commitments or transcript frames. The proof-managed relation layer also checked
its own labels against only the fifteen caller-supplied aggregate domains, not
the five fixed DEEP and FRI-mask roles.

The aggregate core now owns one closed five-role set and exposes a collision
predicate to relation layers. Aggregate and proof-managed validation both check
the complete cross-layer set. `aggregate_domains_cannot_alias_fixed_core_roles`
and `malformed_trace_profile_and_entropy_never_emit_a_proof` iterate the
authoritative set so future fixed-role additions cannot silently lose coverage.

### ZK-AUDIT-19: Batch inversion accepted empty and noncanonical inputs

Severity: Low internal invariant weakness.

Status: Remediated. The shared Goldilocks batch inversion helper treated an
empty slice as a successful inversion and allowed noncanonical field wrappers
to enter modular multiplication. It now rejects empty input and every residue
outside the canonical field range before computing prefixes. The regression
`batch_inversion_rejects_empty_and_noncanonical_inputs` pins both cases.

### ZK-AUDIT-20: Kaigi scalar commitments lost one field bit in `Hash`

Severity: Medium public-input ambiguity and honest-proof availability failure.

Status: Remediated. Kaigi previously placed a canonical Pasta scalar directly
inside `Hash::prehashed`. The `Hash` constructor unconditionally sets bit 248,
so scalars that differed only in that bit produced the same instruction
artifact, and roughly half of honest circuit outputs decoded as a different
public input. Kaigi now packs all 255 scalar bits injectively by shifting the
seven high representation bits left, inserting the required marker at bit 248,
and reversing that operation before canonical field decoding. The JS-facing
byte helpers and Rust `Hash` helpers share this encoding. The regression
`scalar_hash_encoding_does_not_overwrite_field_bit_248` demonstrates the old
two-to-one carrier and round-trips both formerly colliding values.

### ZK-AUDIT-21: Kaigi roster proofs did not bind the signed participant

Severity: Critical authorization and proof-replay failure.

Status: Contained by fail-closed production admission. The roster circuit binds
an arbitrary private `account` witness to commitment and nullifier outputs, but
none of its public inputs is derived from the account that signed the
`JoinKaigi` instruction. A non-interactive proof is transferable: an observer
could copy a pending proof and artifacts, name their own signed account as the
participant, and satisfy the former outer authorization check. The roster root
does not repair that missing authority relation.

Production `ZkRosterV1` join verification now rejects before proof dispatch
until a versioned circuit, public-input schema, and deterministic key bind the
canonical signed participant. Transparent Kaigi and usage commitment proofs are
unaffected. Host create/end remains separately usable because every end now
requires the stored host's transaction signature; host-create nullifiers are
recorded and cannot be replayed as end nullifiers. Public JS/native roster
builders fail with the same unavailable status; candidate construction remains
test-only and uses the canonical schema bytes, `CID1` key carrier, nonzero
verifier-key commitment, and canonical Norito outer envelope.

### ZK-AUDIT-22: FASTPQ trace and LDE generators disagreed on the AIR row stride

Severity: High protocol-domain mismatch.

Status: Remediated. Both canonical roots had their advertised exact subgroup
orders, but they were generated independently. Consequently
`lde_root^blowup_factor != trace_root`, while the AIR prover and verifier opened
the alleged next trace row at `index + blowup_factor`. That index was not the
evaluation point obtained by multiplying the current point by the trace
generator, so the transition composition was formed over the wrong row pair.

The trace roots are now derived as `lde_root^blowup_factor` for both catalogue
entries, and `Planner::new` fails fast if individually primitive roots do not
satisfy that relation. Parameter versions advanced from `3/4` to `5/6`, and
proof/transcript fixtures were regenerated. Catalogue tests independently pin
exact orders, outside-subgroup cosets, and the cross-domain equality. Canonical
version admission now compares the complete parameter record instead of only
its name, and rejects a same-name mutation before trace planning.

The same audit found that the reusable Merkle verifier ignored index bits above
the authentication-path depth, allowing `i + k * 2^depth` to reuse the path for
`i`. Proof verification already derived in-range indices from its transcript,
but the helper now also requires all residual high bits to be zero; an
adversarial-index regression pins the rejection.

### ZK-AUDIT-23: Zero collapsed the aggregate DEEP current/next geometry

Severity: Low-probability verifier-soundness invariant gap.

Status: Remediated. The shared DEEP admissibility predicate excluded trace,
evaluation, query, and translated next-row domains, but accepted zero because
zero lies outside every multiplicative domain. At zero, however,
`z * omega_H == z` for every native trace group. The proof still carried and
mixed independent current and next openings, so the sampled point could
collapse the distinct-point geometry assumed by that relation.

Zero is now explicitly inadmissible. Prover and verifier call the same
`derive_deep_point_v1` after absorbing the FRI-mask roots, so both apply the
same predicate and transcript schedule. Rejection sampling is deterministic and
bounded to sixteen framed attempts; a rejected candidate is not absorbed, and
exhaustion leaves the transcript unchanged and fails closed. The regressions
`deep_point_exclusion_covers_trace_evaluation_query_and_next_domains` and
`goldilocks_fp4_transcript_and_rng_sampling_fail_closed` pin zero rejection,
the exact retry counter/attempt sequence, and bounded exhaustion.

### ZK-AUDIT-24: Oversized Merkle domains reached an infallible hash assertion

Severity: Low profile-misconfiguration availability risk.

Status: Remediated. SHA-256 transcript frames encode their domain length as a
`u16`, but Merkle tree construction and path/multiproof verification formerly
checked only that their node domain was nonempty. A statically configured domain
longer than 65,535 bytes could therefore reach `sha256_merkle_node_v1` and panic
at its infallible framing assertion instead of returning a profile or proof
error.

Aggregate and proof-managed domain validation now rejects every empty or
oversized framed role. Full-tree, streaming-tree, single-path, and canonical
multiproof entry points independently enforce the same bound before hashing.
`layout_domains_and_frontiers_are_fail_closed`,
`malformed_trace_profile_and_entropy_never_emit_a_proof`, and
`merkle_paths_bind_domain_index_leaf_order_and_depth` cover the configuration,
construction, and verification boundaries.

### ZK-AUDIT-25: Unanchored FASTPQ became generic AXT authorization

Severity: Critical authorization-boundary failure.

Status: Contained for standalone IVM admission; handle-backed authorization
remains release-blocked. FASTPQ's current catalogue honestly declares a
roughly 32-bit commitment ceiling. Its transfer verifier also reconstructs the
complete caller-carried batch, transcript, SMT witness, trace, lookup material,
and commitments. That deterministic replay checks transfer arithmetic and root
chaining, but it does not make caller-supplied `old_root`, `new_root`, or
transaction-set context an authoritative finalized source-state statement.
Several trace fields—including key/asset identity, path-node, running-counter,
and permission columns—are enforced by that replay rather than by every column
appearing in the sampled AIR residue vector. The replay is therefore mandatory
for current correctness and does not upgrade the commitment's security level.

Production CoreHost formerly let `AXT_VERIFY_DS_PROOF` expose successful FASTPQ
verification to a contract and record/cache the proof without matching its
roots and transaction set to a finalized/QC-backed source anchor. A valid
caller-carried witness could therefore be mistaken for authorization at only
the proof system's approximately 32-bit binding strength. Non-null standalone
admission now returns `PermissionDenied` with `AxtRejectReason::Proof` before
recording proof state or touching an existing verified-proof cache entry. A
zero pointer remains an explicit proof-clear operation. The adversarial
`axt_verify_ds_proof_rejects_unanchored_fastpq_without_state_or_cache_mutation`
regression preloads a valid cache sentinel, submits a fully valid FASTPQ proof,
and proves the proof map, cache contents, and cache slot remain unchanged.

Specialized callsites have distinct trust analyses. Verified lane-relay
registration independently matches the proven roots and transaction set to a
finalized lane execution commitment. Fee-sponsor vault allocation is checked
against authenticated owner/delegation and current authoritative vault/policy
state. Those paths do not derive authority from generic syscall success. The
issuer-signed asset handle path is narrower but not release-qualified: its
signature covers capability and asset identity, not the
`RemoteSpendIntent`, proof bytes, or effective amount. Exact intent and amount
still rely on FASTPQ metadata and the approximately 32-bit commitment. The
handle path must remain outside production release authorization until those
facts have at least 128-bit binding or are independently matched to an
authoritative finalized source-state statement.

## Dependency Assumptions

- SHA-256 and Blake2 are collision resistant and implemented correctly.
- Implementations actually identified as Poseidon2 are domain-separated and
  apply delimiter framing before field-sponge padding. FASTPQ and ZK-ACE use a
  separately versioned dense-MDS Goldilocks `x^7` construction and receive no
  Poseidon2 security assumption from this audit.
- Vendored Halo2 and curve crates implement advertised group, scalar-field, and
  transcript APIs correctly.
- Norito encoding is deterministic and rejects malformed payloads according to its
  API contract.
- `trust_committed_execution_results` remains a replay/recovery compatibility flag;
  ZK-ACE no longer relies on that assumption because verifier failure is always
  rejected.
- Mock privacy features such as Kaigi privacy mock modes are not enabled in
  production.

## Binding and Guardrails

`OpenVerifyEnvelope` carries backend tag, circuit id, VK hash, public inputs,
proof bytes, and auxiliary bytes for the generic proof surface. Admission
validation rejects unsupported backends, empty circuit ids, zero VK hashes,
empty public inputs, empty proofs, oversized fields, and nonempty aux unless
explicitly allowed. ZK-ACE identifiers are reserved at this generic boundary;
the generic STARK prover and verifier reject them and direct callers to
`SubmitPrivacyProofV1`.

`ProofAttachment` rejects inconsistent backend fields and legacy inline VK fields.
`VerifyingKeyRecord` binds circuit id, backend, curve, schema hash, commitment, size
limits, gas schedule id, status, and optional inline key. Ledger admission accepts
active registered VK records only.

Registry and runtime guardrails reject trusted-setup and developer-only labels.
`preverify_with_budget()` checks active VK status, budget, VK commitment, envelope
metadata, and dedup cache keys. Its result is advisory only: it is not persisted
as an authoritative acceptance decision and ledger execution always invokes the
guarded cryptographic verifier. `verify_backend_with_timing_guardrails()` enforces
backend enablement and maximum envelope/proof sizes before dispatch.

## Native STARK/FRI and ZK-ACE AIR

The native STARK verifier uses Goldilocks modulus `2^64 - 2^32 + 1`, rejects
noncanonical field elements, validates verifier parameters, verifies Merkle openings,
derives Fiat-Shamir query indices from bound transcript material, binds every
FRI folding challenge to its exact round, checks FRI folds, and binds AIR
trace/composition/public digests. Generic binding-AIR verification
checks every coordinate of each sampled current/next row against the
verifier-owned row instead of compressing residuals with public fixed weights,
then reconstructs and matches the complete canonical public trace root within
the bounded `2^12` generic Binding domain. It also requires the exact Merkle
root of an all-zero composition vector. Explicit full-material verification
recomputes both roots from the complete caller-independent material.

The reserved ZK-ACE ledger shape carries
`PrivacyProofEnvelopeV1::ZkAcePqAuthorizationV0`. Its public input is
`ZkAcePrivacyPublicInputsV1`: the exact typed
`ZkAcePqAuthorizationStatementV1` plus the trusted genesis hash. The statement
binds the chain, action index, transaction-intent digest, compiled artifact
digests, governed policy id and digest, authorization epoch, identity
commitment, transfer participants, asset, atomic amount, and replay nullifier.
The low-level AIR projection is internal to the dedicated prover and verifier.

`ZkAcePrivacyWitnessV1` owns the identity root, identity blinding, and replay
secret behind private fields. It is non-serializable, non-cloneable, and
zeroized on drop; construction rejects an all-zero component. Runtime admission
requires the exact active compiled protocol activation, a valid active governed
policy, an allowlisted source, the signed transaction-intent binding, trusted
genesis, matching statement and policy epochs, a valid native proof, and an
unused replay nullifier. The compiled profile is currently unavailable, so this
validation shape cannot activate or execute a ZK-ACE proof.

The disabled candidate prover independently masks the execution trace and the
full FRI batching space before transcript challenges, links the AIR at a
quartic-extension DEEP point, and self-verifies each produced proof. Adversarial
tests mutate typed public bindings, witness relations, mask geometry, DEEP
openings, query schedules, and FRI paths.

## Parameter Security

ZK-ACE v0 fixes its candidate algebraic profile in the compiled engine
descriptor rather than a caller- or registry-supplied VK payload. The descriptor
commits the Goldilocks base and quartic extension, degree-two AIR, 4,096-row
trace, 65,536-row LDE, trace and FRI masks, one DEEP point, 108 unique queries,
twelve binary FRI rounds, SHA-256 domains, fixed proof wire, and
work-normalized classical-ROM STARK bound. It also records the sequential
one-field-output commitment and disabled activation state.

Security interpretation: the theorem-derived certificate covers the candidate
STARK/FRI geometry, not the weaker public commitment that defines its statement.
The complete ZK-ACE system therefore has only the commitment's roughly 32-bit
binding ceiling and is unavailable. No qROM claim is made.

## IPA/Halo2 Verification

The Iroha-owned IPA wrapper uses deterministic transparent generator derivation
at circuit-fixed degrees, transcript label limits, curve/parameter consistency
checks, exact compiled verifier-key equality, public instance shape checks,
proof round count checks, final verifier equality, batch helpers, and envelope
decoding limits. Persisted proving keys use bounded canonical Norito archives;
their processed Halo2 payloads are structurally preflighted before the vendored
reader and must re-encode canonically.

Ledger integration adds active VK requirements, backend-label policy, envelope backend
tag checks, VK hash/commitment checks, circuit/schema/public-input metadata checks,
and disabled-backend fail-closed behavior. External Halo2 circuit soundness is an
assumption.

## Torii, IVM, Kaigi, and FASTPQ

Torii ZK endpoints are diagnostic/report surfaces. IVM host verification enforces
backend enablement, active VK maps, namespace binding, envelope size limits, circuit
id matching, inline VK hash matching, owner manifest matching, curve/backend
allowance, schema hash matching, proof caps, and batch limits. Kaigi usage and
host-proof verification enforce configured active VKs, roster-root binding, an
injective canonical Pasta-scalar carrier, envelope backend/circuit/VK-hash
metadata, proof registration, and guardrailed dispatch. Production
`ZkRosterV1` joins fail closed until the roster statement binds the signed
participant authority; see ZK-AUDIT-21.

FASTPQ `verify_with_limits()` checks protocol/parameter versions, batch consistency,
proof size/shape, trace commitment, expected public I/O, nonzero LDE domain, transcript
challenges, lookup/AIR coefficients, sampled query indices, Merkle paths, AIR row
widths, next-row openings, FRI roots, folded values, and query chains. A bounded
preflight first rejects noncanonical representations in every proof-carried
Goldilocks scalar and field container.

AXT/FASTPQ binding checks canonical binding normalization, dataspace, manifest root,
payload size, batch parameter, batch public dataspace, concrete execution batch,
source transaction commitment, embedded binding metadata, claim digest, witness and
policy commitments, source receipt id, target dataspaces, effect type, corridor, batch
seal, transfer transitions, and transfer transcripts. `RegisterVerifiedLaneRelay`
then checks lane envelope verification, proof payload digest, height/expiry, source
dataspace, effect type, lane relay claim digest, and FASTPQ proof result before
recording a verified lane relay.

## Formal Model

The audit adds a TLA+ control-plane model under [../formal/zk](../formal/zk) and a
runner at [../scripts/formal/zk_tlc.sh](../scripts/formal/zk_tlc.sh). The model
asserts active matching VK requirements, backend/circuit/schema/VK-hash/domain/public
input non-swappability, disabled/trusted/developer/oversized/decode-only fail-closed
behavior, diagnostic endpoint separation, ZK-ACE replay rejection, and FASTPQ claim
binding. Mutation configs enable one fail-open bug each and must produce TLC
counterexamples.

## Claim Matrix

| Claim | Result | Evidence |
| --- | --- | --- |
| Active VK required | Satisfied for normal ledger paths | `VerifyingKeyRecord::is_active`, registry resolution, `preverify_with_budget`, `VerifyProof` |
| VK hash cannot be swapped | Satisfied | `OpenVerifyEnvelope.vk_hash`, `hash_vk`, registry commitment checks |
| Backend cannot be swapped | Satisfied | `ProofAttachment`, attachment validation, `verify_backend`, guardrails |
| Circuit/schema/public inputs cannot be swapped | Satisfied for production registry and direct dispatch | normalized circuit checks, closed Halo2 schema map, schema hash checks, public-input digest checks; see ZK-AUDIT-16 |
| STARK domain tag is bound | Satisfied | derived STARK domain tag checked against inner envelope |
| Malformed STARK proof rejection | Satisfied by code shape and tests | decode, parameter, Merkle, AIR, and FRI checks |
| ZK-ACE replay rejected | Satisfied; verifier-failure trust bypass remediated | signed transaction intent, governed policy, and replay-nullifier checks; see ZK-AUDIT-01 |
| ZK-ACE privacy strength | Unavailable; candidate commitment has a roughly 32-bit binding ceiling | fail-closed engine flag, independent-lane remediation descriptor, legacy-collision regression; see ZK-AUDIT-03 and ZK-AUDIT-13 |
| IPA metadata binding | Satisfied for Iroha-owned wrapper | generator DST, transcript limits, shape checks, canonical outer schema, strict ZK1 carrier, VK/envelope checks |
| Trusted setup fail-closed | Satisfied in audited policy | registry and runtime label rejection |
| Diagnostic endpoint not ledger-grade | Satisfied in code; documentation risk | Torii attachment/prover worker are report-only; see ZK-AUDIT-02 |
| FASTPQ transfer replay and lane claim binding | Satisfied only with mandatory full verifier replay; standalone IVM admission unavailable | `ensure_public_io_matches`, transcript/SMT replay, `verify_batch_matches_binding`, finalized lane claim checks; see ZK-AUDIT-25 |
| AXT remote-spend authorization strength | Unavailable for release; generic proof admission fails closed and handle intent/amount binding remains roughly 32 bits | non-mutating `AXT_VERIFY_DS_PROOF` rejection, inline authenticated-handle tests, release blocker; see ZK-AUDIT-25 |

## Verification Plan

```bash
scripts/formal/zk_tlc.sh fast
scripts/formal/zk_tlc.sh mutations
cargo test -p iroha_core --features zk-stark --lib zk_stark::tests::
cargo test -p iroha_core --features zk-stark --lib zk_ace
cargo test -p iroha_core --features zk-halo2-ipa --lib zk::
cargo test -p iroha_zkp_halo2
cargo test -p fastpq_prover
```

The broad `zk::` library slice keeps heavyweight Kagemusha non-native
MockProver subtests behind `#[ignore]`; run those explicitly with `--ignored`
when circuit-synthesis evidence is required. The default slice still exercises
the fast builder, preflight, public-input substitution, transcript, range, and
metadata-binding negative paths.

Audit-driven regression coverage includes the retired ZK-ACE trust-flag
bypass, diagnostic success not creating ledger proof records, continued
backend-label rejection, compiled-profile substitution, typed-statement
mutation, governed-policy drift, and malformed dedicated STARK proofs.
Additional trust-boundary regressions cover tampered Kagemusha and
confidential-transfer proofs when committed-result trust is set. Add further
regressions only for newly confirmed gaps.

## Conclusion

Iroha's normal ZK verifier admission architecture is sound at the control-plane
binding layer. Active VK registry policy, envelope metadata, backend guardrails,
public-input binding, and proof dispatch are consistently tied together. Native
STARK/FRI and FASTPQ verification include meaningful malformed-proof rejection and
statement-binding checks.

ZK-ACE now carries a quantitative compiled-profile certificate and fixed proof
wire instead of the historical caller-selected PoC parameters. Independent
review must be repeated if that profile changes, and the current certificate is
strictly a classical random-oracle claim rather than a qROM claim.
Recovery-only trust and diagnostic endpoints must remain outside fresh ledger
admission, with tests proving that separation across proof-bearing flows.
