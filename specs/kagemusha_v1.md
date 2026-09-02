# Kagemusha V1

Kagemusha V1 is the sole first-release kagemusha protocol. Canonical text
transport is `kgm1:` followed by unpadded base64url of the canonical Norito
value; V1 has one decoder and no protocol selector or migration surface.

## Guarantee and monetary state

V1 has no cumulative protocol limit based on hops, ancestry, origins,
receipts, fan-in, proof depth, received-fund provenance, or the number of prior
state transitions. Fixed wire sizes, checked `u128` arithmetic, finite
processing time, and physical storage remain unavoidable. Resource exhaustion
may stop a receiver from issuing a new acceptance ticket or stop a sender
before preparation; it must never reject, cancel, or strand money that was
validly committed against an existing reservation.

Each `(network, device lane, asset, exact AxtAssetIncarnationV1 token)` has one
serial, hardware-controlled aggregate state:

```text
(private balance: u128,
 hardware epoch and exact-next sequence,
 consumed-credit sparse-Merkle root,
 state nonce,
 recursive state proof)
```

The balance, replay root, state openings, logical sequence, and hardware epoch
remain private. `PaymentV1` publishes only opaque sender-before and
sender-after state commitments so every verifier can enforce one predecessor
and one successor without learning the balance or replay tree. An authenticated
external sparse-tree database may retain replay nodes, but qualified hardware
retains and authorizes the authoritative root. Corrupt or missing external
nodes are a recovery fault, not grounds for rejecting an already-staged credit.

The fixed relations are:

- `Bootstrap`: establish a hardware-bound zero balance;
- `MintFold`: add one finalized reserve-backed mint credit;
- `SendSplit`: subtract a positive receiver-bound credit;
- `ReceiveFold`: fold one staged credit;
- `RedeemSplit`: subtract a positive terminal voucher; and
- `Rotate`: carry the whole state into the exact next hardware epoch and,
  when governed bridge evidence is present, install the successor verifier
  suite in that same transition.

Every non-bootstrap relation consumes exactly one predecessor and creates
exactly one successor, including a zero-balance successor. Additions,
subtractions, sequences, and counters use checked arithmetic. Any pre-commit
failure leaves the predecessor spendable and unchanged.

`ReceiveFold` verifies one complete credit and terminal commit wrapper, proves
replay nonmembership, updates the replay root, and performs one checked balance
addition. Wallets repeat the same constant-shape transition in the background
and synchronously drain enough staged value before a send or redemption. There
is no count-based backlog rejection or cumulative receipt limit.

## Hardware admission

Offline authority requires a qualified non-forking service, not merely a
hardware-backed signing key. Governance publishes
`KagemushaHardwareProfileV1` (`HardwareProfileV1`). Its stable
`hardware_profile_id` binds the provider and platform/product class, firmware
policy, governed credential issuer, physical qualification report, validity,
and the complete required capability set:

1. exact-next state transitions or one-use successor keys;
2. rollback-resistant journal and accepted-credit inbox;
3. trusted commit time;
4. atomic, recoverable transition certificates;
5. authenticated durable state and byte-identical payment outbox;
6. authoritative replay-root and external sparse-tree recovery;
7. offline hardware-epoch rotation and counter rollover; and
8. no software fallback.

Raw OEM or platform attestation is verified during online enrollment.
Governance then issues a compact, circuit-friendly
`KagemushaHardwareCredentialV1` (`HardwareCredentialV1`) binding the device
transition key, network, lane commitment, profile, firmware policy, policy
epoch, hardware epoch/generation, issuance time, and expiry.
The credential and profile-membership proof, not the raw certificate chain, are
used by offline circuits. Stock KeyMint, StrongBox, Secure Enclave, or App
Attest remains online-only unless an OEM/secure-element service implements and
qualifies the complete profile. There is no software downgrade.

The normalized circuit and recovery contract is defined in
[`kagemusha_guard_bundle_v1.md`](kagemusha_guard_bundle_v1.md); the
optional mobile ABI is defined in
[`kagemusha_device_bridge_v1.md`](kagemusha_device_bridge_v1.md).

## Requests, payments, acknowledgements, and capacity

The peer protocol has exactly three public messages:
`KagemushaPaymentRequestV1`, `KagemushaPaymentV1`, and
`KagemushaAcknowledgementV1`. There is no acceptance intent, ticket,
no-commit closure, preflight message, or alternate decoder.

`PaymentRequestV1` is a signed exact-payment invitation. It binds the
authenticated release, network, exact `AxtAssetIncarnationV1` token and scale,
pooled-reserve identity, positive amount, recipient account, stable receiver
lane, recipient one-time encryption key, complete hardware policy/credential,
request ID, and validity window. It never binds the receiver's current balance
head. Any number of independently valid payments may use one request; invoice
deduplication is an application concern and never monetary admission policy.

`PaymentV1` binds one unique credit ID, the signed request and receiver lane,
the sender's opaque before/after state commitments, the positive amount, the
literal trusted sender-commit time, an encrypted credit opening, a normalized
hardware-transition commitment, and one constant-size paired recursive proof.
The sender commit time must be inside the request window. Delivery time is not
checked: an in-window committed credit remains acceptable and foldable
indefinitely.

Receiver hardware atomically stages the exact canonical request/payment bytes,
credit ID, replay decision, and rollback-resistant inbox receipt before an ACK
may be emitted. Exact duplicate delivery returns the byte-identical durable
ACK. Reuse of a credit ID with different bytes fails. Distinct payments against
the same request are staged independently. Credits are folded continuously in
the background; before a send or redemption, the wallet synchronously folds
whatever pending credits are required. Backlog may add latency but cannot
cause a count-, history-, origin-, or depth-based rejection.

Finite storage remains a physical bound, not a protocol quota. If a receiver
cannot durably stage immediately, it does not reject or invalidate the credit;
the sender's authenticated outbox retains the byte-identical payment for later
delivery. Before hardware commit, a sender must reserve enough durable outbox
and recovery bytes for the transition witness, proof, canonical envelope, and
retry record. After commit the remainder state is immediately spendable and a
missing ACK only retains that retry entry. Exposed credits cannot be cancelled.

## Prepare, prove, commit, and delivery

`SendSplit` and `RedeemSplit` use the same recoverable state machine:

1. **Stage intent.** Hardware verifies the credential and request where
   applicable, reserves the complete outbox budget, locks the exact predecessor,
   and seals every transition input plus deterministic proof/randomness seeds.
2. **Commit once.** Hardware atomically consumes that predecessor, installs its
   only successor, and emits a recoverable transition certificate. For a send,
   the amount is now an irrevocable receiver-bound credit and cannot be
   cancelled.
3. **Prove and recover.** The audited native core recursively proves the exact
   committed transition and normalized hardware guard under the authenticated
   release. A crash resumes the same certificate and sealed seeds; it cannot
   recreate or consume the predecessor again.
4. **Persist.** Core verifies both proof parities, then atomically persists the
   successor state, proof, canonical envelope, and retry-outbox bytes.
5. **Expose.** Only the installed canonical `PaymentV1` or redemption voucher
   may leave the device. Recovery always reproduces the same exposed bytes.

For `SendSplit` and `RedeemSplit`, the transition proof statement's
`effect_digest` equals the canonical semantic digest of the exact prepared
public projection, including its opaque commit evidence. Core checks this
binding before candidate persistence, so changing commit evidence is rejected
even when the amount, lifecycle, and transition nullifier still match.

A crash before step 2 may abandon the staged intent without consuming the
predecessor. A crash during or after step 2 must recover the one committed
successor and complete proof/envelope persistence. Proof generation is
deterministic from the sealed seeds. Neither a staged intent nor a bare
hardware certificate is accepted as money.

The data-model `validate_shape*`, canonical decode, digest, and signature
helpers reject malformed or inconsistently bound wire values only. Monetary
admission is a Core operation that derives authority from an
`KagemushaAuthenticatedReleaseV1`, verifies the governed profile and
credential, runs the native recursive/helper/wrapper proof verifiers, and
passes non-forgeable internal typestates between stages. Callers cannot replace
those checks with self-described release, profile, verifier-key, or proof bytes.

Canonical Norito remains the sole transport encoding, but a transport archive
is not a circuit transcript. Commit-wrapper semantic hashing uses explicit
fixed-width V1 layouts for the request binding, state commitments, trusted
commit time, hardware transition commitment, outbox reservation, and terminal
certificate. Integers and enum tags in those layouts are unsigned little-endian
values; fixed digests are copied as their raw 32 bytes in data-model field
order. Each outer semantic digest or certificate ID is
`SHA256(domain || 0x00 || u64_le(transcript_length) || transcript)`. SDKs must
consume the same fixture vectors and may not substitute a Norito header,
padding, or checksum for any of these circuit inputs. This separation does not
create a second transport or decoder.

The receiver validates the complete final wrapper before staging. It ACKs only
after the exact request/payment bytes, credit ID, replay decision, and inbox
receipt are rollback-resistently persisted. Delivery time does not replace the
authenticated sender commit time.

## Public transcript and privacy

Apart from fixed framing and the signed request context, `PaymentV1` exposes:

- one unique transition nullifier/credit ID;
- sender-before and sender-after aggregate commitments;
- the request digest, recipient lane/key binding, and positive amount;
- the literal trusted sender-commit time;
- ciphertext commitment/digest and encrypted credit opening;
- one normalized hardware-transition commitment; and
- the constant-size paired recursive proof.

The wrapper proves that the trusted time came from the qualified hardware
provider and fell inside the request window. Raw clock authority, hardware
counter, state openings, balance, replay path, device key, and certificate
remain private witnesses. Publishing before/after commitments intentionally
makes exact state succession visible; it is the V1 tradeoff for simple public
non-fork detection and does not make proof size depend on history.

Every `KagemushaPairedProofV1` uses fresh statement-scoped rerandomized
credential audits and history accumulators in its two parity components. Those
values are neither reusable credential pseudonyms nor stable profile, lane, or
device identifiers; equality across parity roles or reuse across statements is
rejected by the release relation.

Balances, state nonces, sequence, hardware epoch, replay paths, and received-fund
provenance remain private witnesses. Two successors from one before commitment
or reuse of a transition nullifier is a public conflict. The hardware-transition
commitment exposes no stable device key, credential ID, counter, or epoch; its
opening and authority are verified recursively.

`KagemushaAcknowledgementV1` exposes only the request digest, payment digest,
an `KagemushaInboxReceiptV1 { credit_id, receipt_commitment }`, and the
receiver signature. The receipt commitment hides the persisted lane, hardware
epoch, exact-next inbox sequence, acknowledgement time, payment, and credit.
Those raw values are private hardware/Core witnesses and are not transport
fields or stable receiver pseudonyms.

## Encrypted credit envelope

Every peer or mint `encrypted_credit` field is the canonical Norito encoding
of `KagemushaEncryptedCreditEnvelopeV1 { version = 1,
ephemeral_x25519_public_key, nonce, ciphertext_and_tag }`. The ephemeral key is
exactly 32 bytes, the XChaCha20-Poly1305 nonce is exactly 24 bytes, the combined
ciphertext ends with its 16-byte tag, and the complete encoded envelope is at
most 384 bytes. Low-order or all-zero X25519 public keys and an all-zero shared
secret are rejected. A fresh ephemeral secret and nonce are generated for each
envelope; nonce freshness is a qualified-service property, not a heuristic
decoder check.

The authenticated plaintext is the canonical fixed-size
`KagemushaCreditOpeningV1 { version = 1, credit_id, amount,
credit_commitment_opening, recipient_binding_opening, recovery_nonce }`, at
most 256 bytes. Its three openings/nonces are non-zero, and qualified hardware
rejects any `credit_id` or amount that does not exactly equal the public
statement.

The recipient key in a signed request or mint-authorization context is an
X25519 public key. For recipient key `R`, ephemeral public key `E`, and raw
X25519 shared secret `DH`, derive the 32-byte XChaCha key with HKDF-SHA256:

```text
salt = SHA256("iroha:kagemusha:v1:credit-envelope-salt\0" || R || E)
IKM  = DH
info = "iroha:kagemusha:v1:credit-envelope-key\0" ||
       SHA256(canonical KagemushaEncryptedCreditAadV1)
```

The canonical AAD is exactly `{ version = 1, purpose = Mint | Peer,
context_digest, issuance_or_transition_commitment, credit_id, amount }`. Mint
uses the pre-ID mint-authorization context digest and issuance commitment. Peer
uses a pre-ID context binding the exact request, receiver lane/key, sender state
commitments, trusted commit time, and lifecycle fields plus the ID-independent
credit-opening/transition commitment.
Ciphertext, ciphertext digest, and proof bytes never enter the AAD or any
pre-ID commitment. XChaCha20-Poly1305 authenticates the canonical AAD and
stores ciphertext followed by the tag.

The `k = 16` recursive circuits do not arithmetize X25519, HKDF, or AEAD.
Sealing and opening must be performed through one audited native core and a
qualified hardware service; the proofs constrain hardware authority,
credential and opening commitments, public projections, and their exact
semantic digests.

## Lifecycle and release binding

Every V1 credit carries an `KagemushaLifecycleBindingV1` with `network_id`,
`protocol_version`, `suite_id`, `vk_digest`, `release_id`, asset identity, the
exact typed `AxtAssetIncarnationV1`, scale, pooled-reserve identity,
`hardware_profile_id`, `policy_epoch`, operation kind, request/ticket/credit
IDs, and ciphertext
digest. The wrapper additionally binds lane commitment and hardware epoch as
private witnesses. Those private lifecycle fields are not transported.

`Rotate` may retain the current verifier or install a successor verifier using
separately governed exact from/to bridge evidence. A verifier-changing rotation
recursively verifies the old state and carries the entire private balance and
replay root into the new hardware epoch under the new suite. A generic circuit
measurement does not authorize that bridge. A suite may become
verification-only, but its verifier cannot be removed while delayed credits
still require it.
Credential or policy-epoch rotation likewise cannot invalidate a payment whose
terminal commit satisfied its ticket and lease.

Emergency compromise may suspend new ticket issuance, top-ups, and offline
commits for an affected profile. Suspension never invalidates a previously
committed ticket-backed payment. Historical verification and an online
redemption/recovery corridor remain available for legitimate credits, using
terminal-nullifier deduplication and the issuer loss policy where compromise
creates excess claims. Normal and emergency controls operate on profiles and
issuance, never on the provenance of received funds.

## Pooled reserve

Each `(network, asset, exact AxtAssetIncarnationV1 token)` has one deterministic
liability pool and one consensus-accounted reserve:

```text
reserve = total finalized top-ups - total finalized redemptions
```

The reserve and every non-negative live liability fit `u128`. Every peer
transition preserves their sum, so a valid staged credit plus its recipient's
current balance is bounded by that same reserve and remains foldable without
arithmetic overflow. This conservation argument, rather than a cumulative
receipt or provenance bound, is part of the circuit relation and release
qualification.

A top-up first verifies a proof-bearing `KagemushaMintAuthorizationV1`, then
atomically debits its payer, increments that reserve, and fixes one immutable
recipient-bound mint intent. Its pre-ID context binds the operation, exact
authenticated release and artifacts, network/asset/incarnation/scale/pool,
amount, payer and recipient, authenticated hardware credential/profile/policy,
fresh recipient-credential and credit-opening commitments, and an X25519
recipient key. Its final statement binds the derived issuance commitment,
credit ID, and exact ciphertext digest. Core must resolve the authenticated
release, verify both authorization proof parities, and validate the active
profile and credential before reserve mutation; shape validation or a missing,
self-described, or forged authorization has no debit authority. Finality
attaches the exact mint credit and circuit-verifiable receipt without changing
the reserve a second time.
Peer payments and folds do not touch reserve state. Redemption verifies the
final commit wrapper, consumes one terminal nullifier, decrements the same
reserve, and credits the beneficiary in one atomic transaction. Duplicate
operation IDs are idempotent; reserve underflow or a conflicting terminal
nullifier rejects without mutation, including under concurrent redemption.

The online top-up request carries the complete compact hardware credential and
the complete mint authorization. Its public
`KagemushaMintCreditStatementV1` is deliberately
compact: it carries the `MintFold` lifecycle, a fresh per-mint randomized
credential commitment, authorization-context and mint-authorization digests,
amount, issuance commitment, recipient, credit commitment, and committed-ledger
time. The stable credential ID, lane, hardware
epoch, device key, and raw credential do not appear in the mint credit. They
remain in the authenticated online request and private mint-helper witness,
which proves that the randomized commitment opens to that exact credential and
recursively verifies the same pre-debit authorization digest.

Mint construction is an acyclic DAG. The randomized recipient-credential
commitment binds operation ID, exact credential ID, and its fresh private
opening. The pre-ID credit commitment binds network, asset identity and
incarnation, scale, liability pool, amount, recipient, recipient X25519 key,
and fresh credit-opening randomness; it excludes authorization context,
issuance commitment, credit ID, ciphertext, and proof. The context digest is
then fixed, the issuance commitment is derived, and the credit ID is derived
without ciphertext or authorization-proof bytes. AEAD is created last with the
credit ID in its plaintext and AAD. The final authorization binds the exact
ciphertext digest, and the mint helper recursively binds that authorization,
reserve-finality proof, lifecycle ciphertext digest, and encrypted bytes before
the mint credit is admitted.

There are no ancestry, provider, risk, or received-fund reserve buckets. Each
`(network, asset, exact AxtAssetIncarnationV1 token)` has exactly one pool; this
preserves fungibility within that pool and deliberately makes all admitted
profiles one global trust domain per pool. Mitigations are strict profile
admission, phased provider rollout, top-up issuance controls, emergency suspension, redemption
monitoring, exact reserve/nullifier accounting, and an explicit issuer loss
policy--not restrictions on spending received funds.

## Release gates

These are promotion gates, not a statement that the current implementation is
release-qualified. The current schema, formal model, evidence formats, and
verification tooling do not by themselves prove that production circuits or a
physical hardware profile passed them.

Release requires real production circuits. Placeholder proofs, mock recursive
verification, or a silent increase of a bound fail the gate. The complete
paired commit-wrapper proof is at most 6,528 bytes. The terminal
request/payment/acknowledgement trio is at most 9,211 raw bytes and 12,288
separately framed `kgm1:` text bytes; those terminal-trio caps do not silently
omit the pre-ticket proof exchange. Request, proof-bearing sender
authorization, and issued ticket target at most 8,960 raw bytes, while all five
transported messages target at most 16,384 raw bytes and must also satisfy their
18,171-byte composed absolute cap: the 9,211-byte terminal-trio cap plus the
independently bounded authorization and ticket. The
release report records exact proof/envelope bytes, circuit rows, verifier-key
size, proving and verification latency, peak memory, energy, and sustained
folding under thermal throttling at depths 8, 64, 1,024, and beyond.
`MintAuthorization` and `MintCredit` report one canonical paired wire proof and
remain subject to the 6,528-byte ceiling. `PlatformCredential` and
`GuardBundle` instead report separate raw Eq and Ep internal-proof samples:
each length must exactly match its authenticated compiled protocol and each
evidence file is resource-capped at 64 MiB. That evidence ceiling grants no
additional wire capacity.

The signed release receipt contains one strictly ordered typed measurement for
all seven aggregate-state relations, the release-wide
`AcceptanceIntentAuthorization` relation, the distinct final commit-wrapper
circuit, and the `MintAuthorization`, `MintCredit`, `PlatformCredential`, and
`GuardBundle` helper circuits;
the four required recursive-depth fixtures; every enabled hardware profile;
and every closed acceptance case.
The manifest's enabled profile list must exactly equal the receipt's qualified
profile list. Aggregate maxima, non-zero report digests, or a profile count are
not substitutes for those complete records; the release tool verifies every
referenced proof, fixture, transcript, and evidence-file digest before signing.

The acceptance suite includes:

- 1,000 independent payments folded and spent as one payment;
- at least 1,024 real recursive handoffs;
- receiver-inbox and sender-outbox capacity exhaustion;
- missing, forged, replayed, and cross-release sender/mint authorization;
- every crash boundary in prepare/prove/commit/wrap/recovery;
- delayed delivery across ordinary suite and credential rotation;
- clock rollback, lease expiry, hardware-epoch rotation, and counter rollover;
- shuffled concurrent exact payments against one reusable request, including
  delayed delivery and application-level invoice deduplication;
- transcript unlinkability and predecessor double-successor conflict detection;
- X25519 low-order/zero-DH rejection, AEAD/AAD substitution, and deterministic
  injected-randomness seal/open KATs;
- reserve underflow and concurrent redemption;
- a four-validator settlement corridor covering finalized top-up, peer
  transfer, full and partial redemption, terminal-nullifier deduplication, and
  reserve/liability conservation;
- animated-QR loss/reordering recovery, with static QR only for messages that
  genuinely fit its bound; and
- physical qualified-hardware tests in airplane mode covering restart, power
  loss, backup/restore rejection, memory, latency, energy, and thermal behavior.

A qualifying release has one audited native cryptographic core that owns
hashing, encryption, proof creation and verification, authenticated-release/
profile/credential admission, certificate verification, and generation of
canonical fixtures. Swift, Kotlin, Java, JavaScript, Python, C#, JNI, QR, and
NFC must cross one versioned native boundary for those operations and provide
only codecs, storage, transport, and orchestration around it. Independent SDK
cryptography or SDK-generated "canonical" fixtures fail the release gate.

The executable safety oracle is
[`formal/kagemusha_v1/KagemushaV1.tla`](../formal/kagemusha_v1/KagemushaV1.tla).
Finite TLC sets bound only model exploration and are never protocol limits.
