# Iroha ZK Cryptographic Audit

Date: 2026-06-02

This report audits Iroha-owned zero-knowledge verifier code and proof-bearing runtime
integrations. Vendored Halo2, curve, hash, encoding, and arithmetic libraries are
treated as dependency assumptions. The audited surfaces are native STARK/FRI,
ZK-ACE AIR, Iroha-owned IPA/Halo2 wrappers, verifier registry policy, proof
envelopes, Torii proof endpoints, IVM host syscalls, Kaigi privacy flows, and FASTPQ
lane proof binding.

## Executive Summary

Normal ledger-grade ZK admission is designed to fail closed. A ledger-accepted proof
must bind an active registered verifying key, backend label, circuit id,
schema/public-input commitment, verifying-key hash, and proof bytes. Registry policy
and runtime guardrails reject trusted-setup and developer-only backend labels before
verifier dispatch.

The native STARK/FRI verifier performs canonical Goldilocks field checks, Merkle
opening checks, Fiat-Shamir query derivation, FRI folding checks, AIR composition
checks, and OpenVerifyEnvelope metadata binding. ZK-ACE adds exact policy/action/
domain binding, transfer digest binding, replay-nullifier deduplication, active
identity/policy checks, and witness nonzero constraints.

The Iroha-owned IPA/Halo2 stack is a transparent IPA verifier wrapper. This audit
does not source-audit vendored Halo2 or curve crates; it audits Iroha's generator
derivation, transcript labels, public-input shape checks, metadata binding, registry
limits, batch dispatch, and runtime guardrails.

FASTPQ lane admission binds the proof to the lane envelope and AXT claim metadata:
dataspace, manifest root, source transaction commitment, effect type, claim digest,
batch seal, transfer witnesses, public I/O, transcript challenges, AIR openings,
Merkle paths, FRI query chains, and proof-size limits.

The principal audit risk is boundary management. Recovery-only trust paths and
diagnostic endpoints must never be usable as fresh ledger-admission proof. This report
records that as the primary finding and models the same class of failure in TLA+.

## Scope and Evidence

Audited code evidence:

- [../../crates/iroha_data_model/src/zk.rs](../../crates/iroha_data_model/src/zk.rs):
  `OpenVerifyEnvelope`, STARK wrapper payloads, ZK-ACE public inputs, witness
  commitments, replay nullifiers, schema hashes, and public-input digests.
- [../../crates/iroha_data_model/src/proof.rs](../../crates/iroha_data_model/src/proof.rs):
  `ProofBox`, `ProofAttachment`, `VerifyingKeyBox`, `VerifyingKeyRecord`, key status,
  and backend/commitment serialization policy.
- [../../crates/iroha_core/src/zk.rs](../../crates/iroha_core/src/zk.rs): verifier
  dispatch, preverify/dedup, backend-label guardrails, envelope metadata checks,
  STARK/Halo2 entry points, and timing/size guardrails.
- [../../crates/iroha_core/src/zk_stark.rs](../../crates/iroha_core/src/zk_stark.rs):
  native Goldilocks STARK/FRI verifier, AIR bindings, ZK-ACE AIR synthesis and
  verification helpers.
- [../../crates/iroha_zkp_halo2/src/lib.rs](../../crates/iroha_zkp_halo2/src/lib.rs):
  IPA verifier wrapper, generator derivation, envelope decoding, limits, and batch
  verification API.
- [../../crates/zk_ace_prover/src/lib.rs](../../crates/zk_ace_prover/src/lib.rs):
  ZK-ACE v0 parameters, VK payload construction, proof attachment construction, and
  witness validation.
- [../../crates/iroha_core/src/smartcontracts/isi/world.rs](../../crates/iroha_core/src/smartcontracts/isi/world.rs):
  verifying-key registry policy, `VerifyProof`, ZK-ACE authorized transfer,
  governance privacy proof checks, and FASTPQ lane relay admission.
- [../../crates/iroha_core/src/smartcontracts/ivm/host.rs](../../crates/iroha_core/src/smartcontracts/ivm/host.rs):
  IVM VK loading, envelope enforcement, verifier syscalls, and batch verification.
- [../../crates/iroha_core/src/smartcontracts/isi/kaigi/privacy.rs](../../crates/iroha_core/src/smartcontracts/isi/kaigi/privacy.rs):
  Kaigi privacy proof metadata and roster-root verification.
- [../../crates/iroha_torii/src/lib.rs](../../crates/iroha_torii/src/lib.rs),
  [../../crates/iroha_torii/src/zk_attachments.rs](../../crates/iroha_torii/src/zk_attachments.rs),
  and [../../crates/iroha_torii/src/zk_prover.rs](../../crates/iroha_torii/src/zk_prover.rs):
  app-facing proof submission, verification, storage, and non-consensus worker
  boundaries.
- [../../crates/fastpq_prover/src/proof.rs](../../crates/fastpq_prover/src/proof.rs),
  [../../crates/fastpq_prover/src/axt_binding.rs](../../crates/fastpq_prover/src/axt_binding.rs),
  and [../../crates/iroha_data_model/src/fastpq.rs](../../crates/iroha_data_model/src/fastpq.rs):
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

Evidence: `SubmitZkAceAuthorizedTransfer` in
[../../crates/iroha_core/src/smartcontracts/isi/world.rs](../../crates/iroha_core/src/smartcontracts/isi/world.rs)
performs local STARK verification and rejects a failed report unless
`state_transaction.trust_committed_execution_results` is set. With that flag set, the
code logs and continues with replay-nullifier insertion and transfer execution.

Impact: if this flag is strictly limited to deterministic replay of already-committed
execution results, it is a recovery mechanism. If it can be enabled during new block
production, fresh transaction admission, or uncommitted ledger execution, an invalid
ZK-ACE proof can authorize a transparent transfer and consume a replay nullifier.

Regression coverage: `zk_ace_rejects_inner_stark_tamper_even_when_committed_result_trust_is_set`
builds a valid ZK-ACE STARK proof, tampers the inner STARK envelope after metadata
binding, enables committed-result trust, and asserts rejection plus unchanged balances
and unconsumed replay nullifier.

Residual recommendation: keep `trust_committed_execution_results` restricted to
committed-block replay/recovery contexts for all other proof-bearing flows, and avoid
adding new proof-authorizing bypasses.

### ZK-AUDIT-02: Diagnostic proof endpoints require an explicit non-ledger contract

Severity: Medium.

Evidence: Torii proof submission, attachment storage, and background prover worker
paths are app-facing and report-only. They do not mutate WSV directly, but a client
can confuse diagnostic success with ledger-grade acceptance if the API contract is not
explicit.

Recommendation: keep these endpoints labeled diagnostic/non-consensus in API docs and
telemetry; never feed diagnostic success into ledger state without re-validating
against the active VK registry, guardrails, proof attachment, and transaction context;
add an integration test that diagnostic success alone does not create a ledger
`ProofRecord::Verified`.

### ZK-AUDIT-03: ZK-ACE v0 STARK/FRI parameters are PoC-scale

Severity: Medium for production deployment; informational for PoC use.

Evidence: `zk_ace_stark_fri_params_v1()` configures `n_log2 = 4`,
`blowup_log2 = 1`, `queries = 2`, binary folding, binary Merkle paths, and SHA-256.
The verifier binds these parameters exactly, but the repo does not contain a
quantitative reduction or production soundness proof for this parameter set.

Recommendation: document a production security target and derive `n_log2`, blowup,
query count, hash mode, AIR degree, and grinding bounds from that target. Treat
parameter changes as governance-visible VK changes.

### ZK-AUDIT-04: BN254/Halo2 naming must remain segregated from ledger-grade IPA policy

Severity: Low.

Evidence: `iroha_zkp_halo2` has raw decoding/verification support for multiple curve
identifiers, while registry/runtime policy rejects trusted-setup and non-IPA labels.

Recommendation: keep production backend identifiers narrow and explicit, for example
`halo2/ipa/pallas`; continue rejecting KZG/Groth16/SRS/PTAU-style labels in registry
and runtime dispatch.

## Dependency Assumptions

- SHA-256 and Blake2 are collision resistant and implemented correctly.
- Poseidon2 use is domain-separated as claimed by Iroha-owned call sites.
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

`OpenVerifyEnvelope` carries backend tag, circuit id, VK hash, public inputs, proof
bytes, and auxiliary bytes. Admission validation rejects unsupported backends, empty
circuit ids, zero VK hashes, empty public inputs, empty proofs, oversized fields, and
nonempty aux unless explicitly allowed.

`ProofAttachment` rejects inconsistent backend fields and legacy inline VK fields.
`VerifyingKeyRecord` binds circuit id, backend, curve, schema hash, commitment, size
limits, gas schedule id, status, and optional inline key. Ledger admission accepts
active registered VK records only.

Registry and runtime guardrails reject trusted-setup and developer-only labels.
`preverify_with_budget()` checks active VK status, budget, VK commitment, envelope
metadata, and dedup cache keys. `verify_backend_with_timing_guardrails()` enforces
backend enablement and maximum envelope/proof sizes before dispatch.

## Native STARK/FRI and ZK-ACE AIR

The native STARK verifier uses Goldilocks modulus `2^64 - 2^32 + 1`, rejects
noncanonical field elements, validates verifier parameters, verifies Merkle openings,
derives Fiat-Shamir query indices from bound transcript material, checks FRI folds,
and binds AIR trace/composition/public digests.

STARK OpenVerifyEnvelope dispatch additionally checks backend tag, VK hash, backend
profile hash mode, normalized circuit ids, inner STARK params, derived domain tag,
and ZK-ACE public-input digest. This binds registry record, wrapper envelope, inner
STARK statement, transcript, and public inputs.

ZK-ACE AIR binds identity commitment, replay nullifier, transaction digest, chain id,
domain tag, action class, policy hash, transfer participants, asset id, amount, and
verifier key id. Witness validation rejects zero identity root, zero identity
blinding, zero replay secret, zero public commitment/nullifier/policy hash, and empty
action/domain strings. Runtime admission additionally requires canonical ZK-ACE
action/domain constants, active identity/policy, allowed source account, exact public
input bytes, exact schema hash, canonical backend, and active canonical VK.

The AIR places witness limbs in a private row and tries up to 256 deterministic
blinding attempts to avoid opening that row under transcript-derived queries. Existing
tests assert safe openings do not recover witness material and reject tampered AIR or
public-input bindings. The privacy claim remains a proof obligation for production.

## Parameter Security

ZK-ACE v0 uses `n_log2 = 4`, `blowup_log2 = 1`, `fold_arity = 2`, `queries = 2`,
`merkle_arity = 2`, SHA-256, and domain tag `iroha:zk-ace:stark-fri:v0`. The code
binds these values correctly through VK payloads, envelope metadata, inner params,
transcript challenge derivation, and domain tags.

Security interpretation: SHA-256 Merkle and Fiat-Shamir binding is a strong
dependency assumption, but two FRI queries over a domain of 16 with blowup 2 is not
enough to infer production-grade soundness from code review alone. The 256-attempt
blinding loop limits privacy-row grinding, but its security depends on transcript
query distribution and AIR shape.

## IPA/Halo2 Verification

The Iroha-owned IPA wrapper uses deterministic generator derivation under an Iroha
domain separation tag, transcript label limits, curve/parameter consistency checks,
public instance shape checks, proof round count checks, final verifier equality, batch
helpers, and envelope decoding limits.

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
runner at [../../scripts/formal/zk_tlc.sh](../../scripts/formal/zk_tlc.sh). The model
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
| ZK-ACE replay rejected | Satisfied; verifier-failure trust bypass remediated | replay nullifier WSV checks; see ZK-AUDIT-01 |
| ZK-ACE privacy strength | Proof obligation | private-row avoidance and tests exist; production proof absent |
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

Add audit-driven regression tests for confirmed gaps: fresh ZK-ACE trust-flag bypass,
diagnostic success not creating ledger proof records, continued backend-label
rejection, and VK/domain binding on parameter changes.

## Conclusion

Iroha's normal ZK verifier admission architecture is sound at the control-plane
binding layer. Active VK registry policy, envelope metadata, backend guardrails,
public-input binding, and proof dispatch are consistently tied together. Native
STARK/FRI and FASTPQ verification include meaningful malformed-proof rejection and
statement-binding checks.

The remaining work is parameter proof and boundary hardening. ZK-ACE v0 parameters
are PoC-scale and need a production soundness analysis before production claims.
Recovery-only trust and diagnostic endpoints must remain outside fresh ledger
admission, with tests proving that separation.
