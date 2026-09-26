# ZK Audit Matrix

This matrix began as a record of the 2026-04-02 and 2026-05-16 ZK hardening
passes. Historical risk labels are not current release qualification. The
standalone-election and Parliament entries below reflect their current source
owners; this correction does not constitute a new independent audit or
revalidate the other entries. Release completion is tracked in the
[first-release closure](privacy_first_release_closure.md).

## Matrix

| Surface | Backend family | Runtime criticality | Outer binding checks | Backend verifier used | Residual risk after patch |
| --- | --- | --- | --- | --- | --- |
| Standalone ZK election ballot / tally | Closed semantic Halo2/Pasta/IPA registry; no admitted ballot or tally circuit | Separate consensus-critical election product, implementation incomplete | Active VK, exact circuit role, VK/schema/envelope and contextual host checks remain mandatory; they cannot supply the missing semantic statement | `world::voting_circuit_matches` and the closed `HALO2_IPA_PRODUCTION_CIRCUIT_IDS_V1` reject the current unsupported vote roles before an accepting proof path | Unqualified. No production ballot/tally relation or canonical key owner; toy vote-bool and generic STARK Binding AIR are rejected. See the required bindings below. |
| Parliament private body ballots / tally | Fixed timed-OVN over BLS12-381 with threshold-BLS release; separate from the Halo2 registry | Consensus-critical Parliament lifecycle | Exact session, registered participant, frozen survivor corpus, release identity, phase/deadline and finalized-release bindings; complete ordered ballot corpus and aggregate count checks | `iroha_crypto::timed_ovn`; Core `governance::timed_ovn`, `tle_release` and `parliament::reducer_ballot` | Real protocol and lifecycle owners exist. Independent timed-OVN/threshold-BLS review, signer/custody qualification and source-bound four-validator release evidence remain required; implementation presence is not an audit result. |
| Confidential transfer / unshield | Registry-backed verifier path (current default `halo2/ipa`, STARK family where configured) | Consensus-critical | Policy/VK resolution, `vk_hash`, schema hash, proof-size caps, backend allowlist, wrapper/header checks | `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. Wrapper and registry binding stay stronger than the standalone helper path. |
| `IvmProved` admission | Registry-backed `halo2/ipa` or `stark/fri/*` | Consensus-critical | `vk_hash`, canonical `ivm-execution` schema hash, circuit id, namespace / manifest match, curve / `k` caps, payload header validation | `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. Admission remains pinned to the guarded runtime verifier. |
| Kaigi privacy join / usage | Registry-backed `halo2/ipa` | Consensus-critical for Kaigi privacy-mode flows | VK registry record, `vk_hash`, schema hash, canonical circuit id, active status, exact commitment/nullifier/root public-input binding | `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. The roster join uses domain-separated constrained Poseidon; on-chain privacy-mode leave remains disabled until it has a dedicated membership circuit. |
| RAM-LFE execution receipts | Resolver signature or policy-published `halo2/ipa` verifier metadata, as required by policy | Non-consensus helper / application-facing | Policy/backend/mode binding; native backend-registry admission; canonical envelope; circuit, schema, and verifier-key hashes; public instance bound to the execution payload hash; runtime enablement and envelope/proof byte caps | Signature verification or `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. Proof-mode receipts use the same node-configured guardrails as other native backend verification paths. |
| Identifier receipts | Signed or policy-published Halo2 RAM-LFE execution attestation plus a signed output opening | Consensus-critical claim admission / application-facing verification | Policy/program linkage, output-opening signature, derived opaque identifier and receipt hash, plus the guarded RAM-LFE proof binding above | Signature verification or the shared guarded RAM-LFE verifier | Low. Consensus and stateless verification now share one proof-validation path, preventing policy or resource-limit drift. |
| Lane relay / FASTPQ | Native FASTPQ prover/verifier | Safety-critical for lane proof checking | Rebuilt transition batch from binding, full `PublicIO` equality (`dsid`, `slot`, roots, hashes), transcript already seeded with `public_io` | `fastpq_prover::verify` | Medium-low. Claims are now checked field-for-field; remaining risk is in FASTPQ arithmetic/circuit correctness rather than omitted public claims. |
| Torii `POST /v1/zk/verify-batch` | Standalone native IPA poly-open helper | Diagnostic only, not ledger-equivalent | Configured total-body cap before decode; finite batch/envelope/curve-`k`/label caps; the wire selects only curve/`n` and the verifier derives the deterministic V1 generators; transcript-bound statement (`transcript_label`, complete derived parameter fingerprint, curve/`n`, `z`, `t`, `p_g`, optional metadata); proof-round shape checks | `iroha_zkp_halo2::batch::verify_open_batch_with_limits` | Low for the standalone primitive. Callers cannot encode alternate generator relations, the embedding surface has no unbounded handler, and resource use is bounded, but the diagnostic endpoint intentionally lacks ledger VK registry / circuit/schema policy enforcement. |
| IVM batch syscall (`SYSCALL_ZK_VERIFY_BATCH`) | Registry-backed `halo2/ipa` and `stark/fri/*` verifier on `CoreHost`; disabled on `DefaultHost` | Runtime helper with ledger-grade binding on the node host | Outer `OpenVerifyEnvelope` header checks, VK registry lookup, circuit/schema/manifest/curve/`max_k` or STARK profile enforcement, then backend verification with guardrails | `iroha_core::smartcontracts::ivm::host::CoreHost` -> `iroha_core::zk::verify_backend_with_timing_guardrails` | Low on the runtime host. `DefaultHost` intentionally returns `ERR_DISABLED`, so the remaining risk is misuse of a non-runtime host rather than a standalone verifier bypass. |

## Election statement completion

[Standalone referenda](governance_pipeline.md#standalone-referendum-boundary)
remain a separate product with PLAIN and proof-backed ZK ballots. They cannot
produce Parliament body results or authorize `GovernanceCertificateV1`.
The missing ZK implementation does not withdraw that product requirement.
The [retired vote fixture](governance_vote_tally.md) supplies neither an
approved semantic circuit nor a production key; changing its label cannot
satisfy the closed registry.

Public PLAIN arithmetic now uses an explicit pre-vote frozen smallest-unit
context, funded escrow even at a zero minimum, immutable-choice monotonic
updates, and a durable closed result independent of released locks. Core and
Torii share the checked public tally. The exact integer-square-root and capped
conviction multiplier live in `conviction_weight_from_units_v1`, a pure checked
reference taking the asset's smallest units. A future private ballot relation
must prove that same equation against its confidential bond. This is a
public-account correctness repair;
it supplies neither anonymous credentials nor a confidential position/ballot or
sound private tally proof, and does not open any registry admission gate.

Core's retained standalone ZK election state now has one bounded, ordered
sequence of fixed 32-byte `(nullifier, commitment)` pairs. Snapshot and restore
validation preserve the exact pair and admission order, reject duplicate
nullifiers, and refuse the retired split-field layout. This is only durable
corpus structure: the current nullifier is still derived from a public
commitment, and no credential, confidential bond, choice-preserving update, or
dropout-resilient closed-corpus tally relation has been qualified. Ballot and
tally production admission remain closed. Both ballot routes still clone the
retained election before the bounded append; whole-owner allocation admission
for that clone and its publication remains a separate resource gate.
Create, Submit and Finalize now declare the same whole-election scheduler key
because each replaces the entire retained record; field-suffixed election hint
keys are rejected rather than treated as independent writes.

Core's current `ballot_inputs_from_columns` reads only commitment and eligible
root. `SubmitBallot` requires the supplied ciphertext to equal those commitment
bytes, and `derive_ballot_nullifier` hashes that public commitment with domain,
network and election selector. `tally_from_columns` reads only one `u64` count
per option. These shapes do not define ballot encryption, credential-based
uniqueness or a tally relation over the actual accepted corpus.

The standalone ZK ballot and final-tally execution routes now retain an explicit
fail-closed semantic admission guard after verifying-key role resolution and
before proof verification or accepted-state mutation. A future key-registration
change alone cannot open these routes. The guard must remain until reviewed
relations prove credential-linked anonymous authority, a confidential bond
weighted in the asset's frozen smallest units, choice-preserving conviction
updates, and the exact closed accepted corpus. Public PLAIN ballots continue to
use their separate frozen-scale arithmetic and funded escrow protocol. The
current closed key registry still rejects vote roles before this guard can be
reached by an instruction; the guard is independent defense against a later
registry or role change and does not qualify standalone ZK elections.

The [standalone protocol contract](standalone_election_protocol_contract.md)
requires anonymous, aggregate-only, committee-free, non-reconstructing dropout
completion. Its construction remains unresolved. The retained standalone
contract therefore needs a reviewed semantic design
that specifies and enforces the following bindings at the circuit and host
boundaries before any production registry/key admission:

- Exact network, election selector, nullifier domain, eligibility snapshot,
  option count and ballot policy. Eligibility membership and the
  election-scoped nullifier must follow one authenticated credential relation;
  a hash of a freely changed public commitment is not that relation.
- A valid choice and its authorized weight, with the actual conviction-lock
  owner, amount and duration where applicable. Duplicate, replacement and
  lock-extension rules must agree with the retained state transition;
  caller-provided hints alone do not prove them.
- The exact admitted ciphertext/commitment and its well-formed relation to the
  choice and confidential bond. Specify any public parameters and tally-opening
  relation required by the construction without a decryption committee, master
  decryption key, voter-secret reconstruction, omitted accepted ballots or
  additional subset tallies. No encryption scheme or recovery construction is
  selected by this matrix.
- The exact closed accepted ballot corpus, its order/root/count, election and
  eligibility/key/policy context, including replacement and duplicate handling.
  Tally counts must be derived from that corpus and respect option, weight and
  arithmetic bounds. Comparing public counts to submitted counts does not
  prove tally correctness or bind another election's result.

Dynamic context may be bound through a precisely specified canonical statement;
its representation, circuit relation and proof/key parameters still require
review. Parliament's three-choice, linkable-participation timed-OVN statement
is not a substitute for this standalone relation. Dedicated election snapshot
reads validate stored shape, not ballot proofs, finality or Parliament
certificate authority. No accepting alias, fallback or new qualification is
introduced by this documentation correction.

## Notes

- The strongest OtterSec-style risk in this repo was the standalone native IPA
  helper, because it previously derived Fiat-Shamir challenges without binding
  the full public statement. That helper now binds `transcript_label`,
  backend/domain size, the exact deterministic parameter fingerprint, `z`,
  `t`, `p_g`, and any optional metadata carried in `OpenVerifyEnvelope`.
- FASTPQ already seeded Fiat-Shamir with `public_io`, so its issue was not the
  same bug class. The hardening here closes the verifier-side claim-validation
  gap by requiring field-for-field `PublicIO` equality before accepting the
  proof.
- Production ledger verification remains centered on the guarded
  `iroha_core::zk::verify_backend_with_timing_guardrails` path and should stay
  the reference implementation for future proof-bearing features.
- STARK `ivm-execution-v1` dispatch uses a dedicated binding AIR context after
  authenticating the exact circuit, VK, schema and public-input digest. It
  reconstructs the full deterministic trace and zero-composition commitments
  within the exact reconstruction domain cap; auxiliary composition is rejected.
  The generic AIR verifier still rejects reserved IVM circuits, and `IvmProved`
  admission always replays the VM to check execution semantics.
- The pre-release decode-only `/v1/zk/verify` and `/v1/zk/submit-proof`
  routes were removed instead of retaining success responses that could be
  confused with cryptographic or ledger acceptance.
- Block headers and peer handshakes now include a `zk_policy_hash` in the
  confidential feature digest, so peers commit to the consensus-relevant ZK
  verifier policy instead of trusting node-local timeout or worker settings.
- Generic `VerifyProof` is registry-only: it requires `vk_ref`, rejects inline
  VKs, and enforces active VK record, circuit/version, schema, namespace, gas
  schedule, and commitment binding before calling the backend verifier.
- Local verifier elapsed time is reported for telemetry but is no longer a
  consensus rejection condition. Runtime limits that can change block validity
  are sourced from the committed ZK policy.
- The standalone Halo2 helper's wire carries only `(version, curve, n)` and the
  verifier derives the one deterministic V1 parameter set for that selector.
  Canonical sets live in a bounded process-local cache and Fiat-Shamir
  challenges come from a running transcript state. This removes the
  caller-chosen generator surface entirely, as well as the previously noted
  unbounded-cache and quadratic-history rough edges, without changing its
  non-ledger status.
