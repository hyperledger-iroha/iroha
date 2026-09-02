# Offline Cash V1 normalized GuardBundle

`GuardBundleV1` is the sole hardware-authority input to Offline Cash V1 state
and commit-wrapper proofs. A raw platform attestation, host signature,
keystore flag, precommit proof, or prepare receipt has no monetary authority.
The final proof accepts only a qualified credential and a recoverable terminal
hardware commit over the exact persisted candidate.

## Profile and credential

`OfflineCashHardwareProfileV1` (`HardwareProfileV1`) is an immutable
governance object with this canonical body:

```text
version = 1
protocol_version = 1
hardware_profile_id
provider_id
platform_class
product_class_digest
firmware_policy_digest
enrollment_attestation_verifier_digest
attestation_trust_roots_digest
allowed_suite_commitment
policy_epoch
governance_credential_public_key
capability_mask
qualification_report_digest
valid_from_ms and expires_at_ms
```

The exact `0xffff` mask makes all sixteen service properties independently
mandatory: exact-next predecessor consumption, one-use successor authorization,
rollback-resistant counter and journal, sealed transition recovery, one-use
acceptance tickets, durable inbox reservation, authenticated inbound staging,
authoritative replay-root recovery, sender outbox reservation, authenticated
durable retry outbox, atomic verified-candidate commit, recoverable terminal
commit certificate, trusted time or monotonic leases, offline epoch rotation,
rollback-safe counter rollover, and no software fallback. The qualification
report commits the enrollment verifier, attestation roots, storage and recovery
semantics, and physical evidence. A governed registry separately marks a profile
`proposed`, `active`, `suspended`, `verification_only`, or `retired`; changing
status does not change its ID.

Raw OEM/platform evidence is checked only by the governed online enrollment
verifier. Successful enrollment issues this compact circuit input:

```text
OfflineCashHardwareCredentialV1 {
  version = 1
  credential_id
  network_id
  hardware_profile_id
  suite_id
  firmware_policy_digest
  policy_epoch
  lane_commitment
  hardware_epoch_id and hardware_epoch_generation
  device_public_key and device_key_reference
  issued_at_ms and expires_at_ms
  governance_signature
}
```

`credential_id` commits every unsigned field. At credential issuance and for a
new offline commit, the issuer must be the one in the active profile, the
firmware policy and policy epoch must match, and the authenticated commit time
or lease must precede credential expiry. A later transition to `suspended`,
`verification_only`, or `retired` does not invalidate a terminally committed
credit: historical verification and the governed online redemption/recovery
corridor remain available. Raw certificate chains are neither peer payloads nor
circuit witnesses. Rotation uses a newly issued credential and proves
continuity from the old lane commitment without revealing the lane.

An online top-up request carries this complete credential plus a
proof-bearing `OfflineCashMintAuthorizationV1`. Before any payer debit or
reserve increment, Core resolves the exact authenticated release and verifies
both parities of that authorization against the active profile, credential,
recipient key, pre-ID commitments, derived issuance/credit identifiers, and
exact ciphertext digest. The resulting public
`OfflineCashMintCreditStatementV1` contains only a fresh randomized credential
commitment and the authorization context/complete-authorization digests; the
exact credential and its opening remain private helper witnesses. Credential
ID, lane, hardware epoch, and device key therefore cannot become public
mint-credit pseudonyms.

The mint relations follow one acyclic construction order: sample the
recipient-credential and credit-opening commitments and X25519 recipient key;
fix their pre-ID authorization context; derive `issuance_commitment`; derive
`credit_id` without ciphertext or proof bytes; and only then encrypt. The final
authorization binds the AEAD digest. The mint-helper relation recursively
verifies that same authorization plus reserve finality and the lifecycle
`ciphertext_digest` before the exact encrypted credit is admitted.

## Private state relation and public projection

The bundle has a fixed operation tag:

```text
Bootstrap | MintFold | SendSplit | ReceiveFoldBatch |
RedeemSplit | Rotate | SuiteUpgrade
```

Its private witness binds:

```text
network_id, protocol_version, release_id, suite_id, vk_digest
asset identity, exact AxtAssetIncarnationV1 token, scale,
and pooled-reserve identity
operation kind
OfflineCashHardwareProfileV1 and OfflineCashHardwareCredentialV1 membership
lane commitment and hardware epoch
predecessor and successor state commitments/openings/nonces
predecessor and successor private balances and replay roots
exact-next logical sequence, hardware counter, and journal revision
request, proof-bearing OfflineCashAcceptanceIntentAuthorizationV1,
its compact OfflineCashAcceptanceIntentV1, exact-amount
OfflineCashAcceptanceTicketV1, private request-ledger decision, and
reserved-byte record when applicable
credit/nullifier IDs and ciphertext digest
exact public policy deadline plus private commit time, clock evidence, or
monotonic lease identity/window/counter
sealed-input, recovery-record, candidate, inbox, and outbox digests
```

The predecessor and successor, sequence, lane, hardware epoch, replay paths,
journal values, raw trusted time, lease identity/window/counter, and clock
authority never become payment public inputs. The payment projection is limited
to the transition nullifier/credit ID, acceptance-intent/request/exact-amount
ticket bindings, recipient one-time key, amount/ciphertext commitment and
ciphertext digest, hardware profile and policy epoch, an opaque time/lease
evidence commitment, compact commit certificate, and fixed release framing.
Both Pasta parities decide the same projection and lifecycle bindings.
Every paired proof uses fresh statement-scoped rerandomized credential audits
and history accumulators. Those proof fields are not reusable credential,
profile, lane, or device pseudonyms; parity equality or cross-statement reuse
fails the released relation.

Every non-bootstrap relation requires exact successor sequence, hardware
counter, and journal revision. Non-rotation operations preserve the lane and
hardware epoch. `Rotate` advances the hardware epoch/counter generation exactly
once and carries balance and replay root unchanged. `SuiteUpgrade` recursively
verifies the old suite and carries the same balance, replay root, and lane into
the named new suite, but a generic suite-upgrade measurement does not authorize
an exact from/to bridge. Activation requires separately governed bridge
evidence; otherwise the old verifier remains available. Checked arithmetic and
canonical empty values are enforced inside the relation.

The relation also proves global liability conservation against the single
`u128` pooled reserve. Because balances and staged credits are non-negative, a
valid staged credit plus its recipient balance is bounded by that reserve and
cannot overflow during a fold. No ticket-value headroom, provenance count, or
historical receipt limit is substituted for this invariant.

There is no cumulative relation limit based on hops, ancestry, origins,
receipts, fan-in, proof depth, provenance, or historical transitions. Fixed
wire values, checked `u128` arithmetic, processing time, and physical capacity
remain real bounds, but they may stop only a new ticket or prepare operation;
they do not invalidate an already committed credit.

`ReceiveFoldBatch` contains exactly sixteen slots and `active_count` in
`1..=16`. Each active slot verifies a distinct final credit, its one-use ticket
and commit wrapper, proves nonmembership against the root resulting from the
previous slot, and adds its private amount. Padding slots are all-zero and have
no effect. The final root and checked sum are installed by one successor.

Peer and mint credits use the one encrypted-credit contract defined in
[`offline_cash_v1.md`](offline_cash_v1.md): canonical
`OfflineCashEncryptedCreditEnvelopeV1`, X25519, HKDF-SHA256, and
XChaCha20-Poly1305 over the exact canonical AAD. The ID-independent mint or peer
opening commitment and pre-ID context are fixed before the credit ID; neither
ciphertext nor proof bytes feed back into those identifiers. The recipient
plaintext is the fixed `OfflineCashCreditOpeningV1`. The `k = 16` proof does not
arithmetize the KEM or AEAD; it constrains the qualified hardware authority,
opening commitments, exact public projections, and semantic digests while the
audited native/hardware boundary performs seal and open.

## Candidate and terminal wrapper

The transition proof is deliberately split around irreversible hardware
commit:

1. Hardware creates a sealed prepared record that locks one exact predecessor,
   reserves every required durable byte, and fixes all inputs and proof seeds.
2. Core produces and durably persists a canonical sealed precommit candidate:

   ```text
   version, protocol/release/suite/vk identity
   operation kind and permitted public projection
   encrypted output and ciphertext digest
   recursive transition proof
   artifact-manifest digest
   ```

3. Core derives proof authority from an `OfflineCashAuthenticatedReleaseV1`,
   cryptographically verifies both proof parities under its pinned verifier
   keys and size policy, and derives
   `candidate_envelope_digest = H(domain || canonical_candidate_bytes)`. This
   produces an internal authenticated-candidate typestate; canonical bytes or a
   data-model shape check cannot produce commit authority.
4. Core constructs the hardware commit command only from that
   authenticated-candidate capability. Hardware atomically consumes the
   predecessor, installs the sole successor, binds
   `candidate_envelope_digest` into its
   `OfflineCashOutboxReservationV1`/terminal record, and emits one recoverable
   `OfflineCashCommitCertificateV1`.
5. `OfflineCashCommitWrapperProofV1` recursively verifies the candidate
   transition proof, profile and credential, hidden complete terminal
   statement, and hardware authentication over that exact candidate digest. It
   publishes only the permitted projection and a compact certificate. Core
   cryptographically verifies the recovered wrapper through the same
   authenticated release before installing the final envelope.

Hardware first commits a self-free `OfflineCashHardwareTerminalBodyV1` in this
order: candidate digest, lifecycle digest, transition nullifier, outbox
reservation commitment, opaque commit evidence, profile and policy epoch,
private successor commitment, private journal commitment, and private recovery
commitment. That body contains no certificate ID, certificate digest, wrapper
digest, or final-envelope digest. Its canonical commitment is then included in
the certificate-ID preimage, fixing the acyclic order terminal body → terminal
commitment → certificate ID → wrapper proof → final envelope.

The compact certificate binds `candidate_envelope_digest`, the lifecycle
binding digest, transition nullifier, a hiding outbox-reservation commitment,
hardware profile, policy epoch, `OfflineCashCommitEvidenceV1`, and that hiding
hardware-terminal commitment. Lane, epoch, credential, journal, successor
authorization, predecessor, and successor remain hidden and are proven by the
wrapper. The evidence variant carries only an opaque trusted-time or lease
commitment; the actual commit time, exact checked deadline, clock authority,
lease identity/window, and authorization counter stay private. The final paired
wrapper proof, not the candidate proof, is transported in `PaymentV1` or the
redemption voucher.

The candidate is not money: peers, receivers, and chain admission reject it.
The wrapper's prover randomness is derived from the sealed prepared record, so
recovery after hardware commit reproduces the identical canonical final
envelope rather than another valid encoding.

Data-model canonical decode, `validate_shape*`, digest, and signature helpers
are non-authoritative structural checks. Only Core can authenticate the release,
profile, credential, helper/transition/wrapper proofs, and hardware terminal
binding and then advance the internal typestates used for monetary admission.

## Journal, capacity, and recovery

The hardware journal has monotonic
`empty -> prepared -> committed -> wrapped -> exposed` states. `prepared` may
be abandoned only before a terminal commit and then unlocks the unchanged
predecessor. `committed` is irreversible and stores or authenticates the sealed
inputs, candidate-envelope digest, sole successor authorization, terminal
certificate, and reserved output location. Recovery must resume that record;
it cannot cancel it or choose new inputs, output, randomness, certificate, or
successor.

Ticket issuance accepts one
`OfflineCashAcceptanceIntentAuthorizationV1`. Before hardware persists the
compact intent or mutates its request ledger, Core verifies both proof parities
through the exact authenticated release and proves hidden enabled-profile
membership, qualified sender hardware, sufficient balance, and a one-use
predecessor authorization for the exact request and amount. A compact
`OfflineCashAcceptanceIntentV1` or shape-valid proof bytes alone have no
authority. Hardware then atomically records the exact intent-digest decision
and reserves its declared inbox bytes while applying the request's private
ledger. `SingleExact` consumes its one exact slot;
`PartialUntilTotal` checked-adds the exact ticket amount to its cumulative
issued amount; `BoundedMultiPayment` increments its issued count and checks its
amount interval; and `OpenReceive` checks the interval without imposing a
cumulative count or amount bound. Every ticket fixes one exact amount. The same
intent cannot issue twice, and expiry never subtracts an issued amount or
count. Resolved `OpenReceive` decision entries may compact into an
authenticated exact-decision accumulator with externally paged nodes; duplicate
and conflict queries remain exact and the accumulator cannot become a
historical admission bound. Governed relocation preserves the ledger decision and an equivalent
delivery slot rather than reclaiming either budget. Only an authenticated
no-commit closure may remove an unresolved ticket and release its capacity,
after proving that its exact authorized predecessor and sender intent never
reached terminal hardware commit. Consumed tickets remain counted.

Staging the exact valid payment consumes that reservation and produces one
durable receipt. Delivery after ticket expiry, profile rotation, or ordinary
suite rotation is admitted when the proven sender commit was valid; later
traffic may not take its bytes. Expiry alone cannot release an allocation that
might cover a delayed in-window commit. Governed recovery may relocate an
unused reservation only while preserving an equivalent durable delivery slot.
Exact duplicate bytes return the same receipt; a different digest for the same
ticket or credit ID is a conflict.

The acknowledgement exposes only request and payment digests, the credit ID,
an opaque signed receipt commitment, and the receiver signature. Receiver lane,
hardware epoch, exact-next inbox sequence, persistence time, and raw durable
record remain private beneath that commitment.

Sender preparation similarly succeeds only after reserving the worst-case
candidate, proof, terminal certificate, canonical envelope, and retry metadata.
No post-commit step may report capacity exhaustion. If local authenticated
storage or its external replay database is damaged, recovery or the governed
online recovery corridor is invoked; committed value is never reclassified as
invalid because storage is full or unavailable.

## Qualification

This section defines promotion gates. The present schema, formal model,
evidence records, and evidence verifier are not proof that production circuits
or any physical profile have passed them; Offline Cash remains unqualified
until the complete matrix is independently satisfied.

A backend is offline-capable only after physical-device evidence exercises
every capability and every crash edge, including power loss immediately before
and after terminal commit, delayed ticket delivery, counter rollover, clock
rollback, outbox/inbox exhaustion, rotation, backup/restore fork attempts, and
byte-identical recovery. The software matrix additionally rejects missing,
forged, replayed, and cross-release sender/mint authorizations plus low-order
X25519, zero-DH, AAD/ciphertext substitution, and nonce/key reuse. Generic
KeyMint, StrongBox, Secure Enclave, App Attest,
software signing, or an independently implemented SDK proof path cannot satisfy
this contract.

Release evidence is a closed, canonical matrix: one record for each of the
seven aggregate-state relations, the release-wide
`AcceptanceIntentAuthorization` relation, the separate commit-wrapper circuit,
and the `MintAuthorization`, `MintCredit`, `PlatformCredential`, and
`GuardBundle` helper circuits; all batch
occupancies 1--16; depths 8, 64, 1,024, and greater than 1,024; every enabled
hardware profile; and every required crash/capacity/lifecycle/privacy/reserve/
transport case. The enabled profile set in the manifest and physically
qualified profile set must be identical. A maximum, count, or non-zero report
digest alone does not mark a matrix cell complete.

A qualifying build has one audited native cryptographic core that owns
authenticated release selection, hashing, encryption, credential and
certificate verification, proof creation and verification, and canonical
fixture generation. Every SDK must cross the same versioned native boundary;
SDKs provide codecs, storage, transport, and orchestration only. Independent
SDK cryptography or fixtures cannot close a qualification cell.
