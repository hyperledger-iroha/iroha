# Iroha ZK Cryptographic Audit

Date: 2026-07-30

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
ZK-ACE uses the separate typed `PrivacyProofEnvelopeV1` path and adds compiled
profile, governed policy, signed transaction intent, trusted genesis, transfer,
and replay-nullifier binding.

The Iroha-owned IPA/Halo2 stack is a transparent IPA verifier wrapper. Production
IPA bases are independently mapped with domain-separated hash-to-curve; the
additive Goldilocks compatibility backend is rejected by runtime verification.
This audit does not source-audit vendored Halo2 or curve crates; it audits Iroha's
generator derivation, transcript labels, public-input shape checks, metadata
binding, registry limits, batch dispatch, and runtime guardrails.

FASTPQ lane admission binds the proof to the lane envelope and AXT claim metadata:
dataspace, manifest root, source transaction commitment, effect type, claim digest,
batch seal, transfer witnesses, public I/O, transcript challenges, AIR openings,
Merkle paths, FRI query chains, and proof-size limits.

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
canonical `zk_ace_prover` path selects an active governed
`PrivacyZkAcePolicyRecordV1`, builds a typed statement and native proof, wraps
them in exactly one signed `SubmitPrivacyProofV1`, and does not expose a
caller-selected backend, verifier key, proof attachment, or generic
`OpenVerifyEnvelope`. `SubmitPrivacyProofV1` execution in
[../crates/iroha_core/src/smartcontracts/isi/privacy.rs](../crates/iroha_core/src/smartcontracts/isi/privacy.rs)
validates the signed transaction intent, compiled activation, governed policy,
native proof, and replay state before committing the transfer effects. The
native verifier does not consult committed-result trust.

Impact: the original bypass could authorize a transparent transfer and consume a
replay nullifier if the flag were enabled during new block production, fresh
transaction admission, or uncommitted ledger execution. The remediated ZK-ACE path
no longer depends on that operational boundary.

Regression coverage:
`zk_ace_production_dispatch_derives_exact_effects_and_rejects_adversarial_binding`
rejects proof and typed-statement substitution, while
`zk_ace_submit_atomically_transfers_and_records_replay_nullifier` exercises the
governed transfer and one-shot replay effect. Transaction-intent tests reject
missing, stale, substituted, and consumed bindings before effects.

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

### ZK-AUDIT-03: ZK-ACE v0 STARK/FRI profile must remain compiled and soundness-bound

Severity: Medium.

Status: Hardened. ZK-ACE uses one compiled native profile: a 4,096-row masked
trace, 65,536-row low-degree extension, quartic Goldilocks challenges, 108
unique FRI queries, binary folding and Merkle paths, and SHA-256 transcript
binding. The fixed proof wire is 1,341,142 bytes and the compiled certificate
claims 128 work-normalized bits in the classical random-oracle model; it does
not claim a quantum-random-oracle reduction. The profile digest binds the
parameter, verifier, statement-schema, and engine-manifest digests used by
privacy activation and proof admission.

Residual recommendation: maintain the quantitative security certificate and
rederive its AIR degree, masking, DEEP, FRI, query, and Fiat--Shamir bounds for
any profile change. A changed profile requires a new protocol identity and data
model release; callers cannot substitute verifier parameters through governance
or the proof wire.

Regression coverage: `zk_ace_profile_is_deterministic_complete_and_bounded` and
`zk_ace_compiled_profile_rejects_every_binding_mismatch` pin the activation
tuple. The native STARK tests
`air_degree_mask_and_work_security_substitution_fail_closed`,
`fri_theorem_precondition_substitutions_fail_closed`, and
`theorem_backed_fp4_soundness_budget_clears_128_bits` pin the compiled security
geometry.

### ZK-AUDIT-04: BN254/Halo2 naming must remain segregated from ledger-grade IPA policy

Severity: Low.

Evidence: `iroha_zkp_halo2` has raw decoding/verification support for multiple curve
identifiers, while registry/runtime policy rejects trusted-setup and non-IPA labels.

Recommendation: keep production backend identifiers narrow and explicit, for example
`halo2/ipa/pallas`; continue rejecting KZG/Groth16/SRS/PTAU-style labels in registry
and runtime dispatch.

## Dependency Assumptions

- SHA-256 and Blake2 are collision resistant and implemented correctly.
- Poseidon2 use is domain-separated as claimed by Iroha-owned call sites, and its
  byte API applies delimiter framing before field-sponge padding.
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
derives Fiat-Shamir query indices from bound transcript material, checks FRI folds,
and binds AIR trace/composition/public digests.

The ZK-ACE ledger path instead carries
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
unused replay nullifier.

The prover independently masks the execution trace and the full FRI batching
space before transcript challenges, links the AIR at a quartic-extension DEEP
point, and self-verifies each produced proof. Adversarial tests mutate typed
public bindings, witness relations, mask geometry, DEEP openings, query
schedules, and FRI paths.

## Parameter Security

ZK-ACE v0 fixes its profile in the compiled engine descriptor rather than a
caller- or registry-supplied VK payload. The descriptor commits the Goldilocks
base and quartic extension, degree-two AIR, 4,096-row trace, 65,536-row LDE,
trace and FRI masks, one DEEP point, 108 unique queries, twelve binary FRI
rounds, SHA-256 domains, fixed proof wire, and work-normalized classical-ROM
bound. Admission requires the activation record to match the compiled profile
exactly.

Security interpretation: SHA-256 Merkle and Fiat--Shamir binding remains a
dependency assumption. The implementation carries a theorem-derived
classical-ROM certificate and fail-closed geometry tests; this audit does not
independently reproduce that reduction, and no qROM claim is made.

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
allowance, schema hash matching, proof caps, and batch limits. Kaigi privacy
verification enforces configured active VKs, roster-root binding, envelope
backend/circuit/VK-hash metadata, proof registration, and guardrailed dispatch.

FASTPQ `verify_with_limits()` checks protocol/parameter versions, batch consistency,
proof size/shape, trace commitment, expected public I/O, nonzero LDE domain, transcript
challenges, lookup/AIR coefficients, sampled query indices, Merkle paths, AIR row
widths, next-row openings, FRI roots, folded values, and query chains.

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
| Circuit/schema/public inputs cannot be swapped | Satisfied for registry-bound paths | normalized circuit checks, schema hash checks, public-input digest checks |
| STARK domain tag is bound | Satisfied | derived STARK domain tag checked against inner envelope |
| Malformed STARK proof rejection | Satisfied by code shape and tests | decode, parameter, Merkle, AIR, and FRI checks |
| ZK-ACE replay rejected | Satisfied; verifier-failure trust bypass remediated | signed transaction intent, governed policy, and replay-nullifier checks; see ZK-AUDIT-01 |
| ZK-ACE privacy strength | Compiled classical-ROM certificate; qROM not claimed | independent trace/FRI masking, FP4 DEEP/FRI, 128-bit work-normalized bound, adversarial geometry tests |
| IPA metadata binding | Satisfied for Iroha-owned wrapper | generator DST, transcript limits, shape checks, VK/envelope checks |
| Trusted setup fail-closed | Satisfied in audited policy | registry and runtime label rejection |
| Diagnostic endpoint not ledger-grade | Satisfied in code; documentation risk | Torii attachment/prover worker are report-only; see ZK-AUDIT-02 |
| FASTPQ public I/O and lane claim binding | Satisfied | `ensure_public_io_matches`, transcript checks, `verify_batch_matches_binding`, lane claim digest checks |

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
