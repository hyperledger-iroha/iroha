# Offline Cash V1

Offline Cash V1 is the sole first-release offline-cash protocol. Canonical text
transport is `oc1:` followed by unpadded base64url of the canonical Norito
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

The balance, replay root, state openings, lane identity, predecessor, and
successor are private. The constant-size state commitment is local proof input;
it is not published in `PaymentV1`. An authenticated external sparse-tree
database may retain replay nodes, but qualified hardware retains and authorizes
the authoritative root. Corrupt or missing external nodes are a recovery fault,
not grounds for rejecting an already-staged credit.

The fixed relations are:

- `Bootstrap`: establish a hardware-bound zero balance;
- `MintFold`: add one finalized reserve-backed mint credit;
- `SendSplit`: subtract a positive receiver-bound credit;
- `ReceiveFold`: fold one staged credit;
- `RedeemSplit`: subtract a positive terminal voucher;
- `Rotate`: carry the whole state into the exact next hardware epoch; and
- `SuiteUpgrade`: carry the whole state through a recursively verified
  proof-suite bridge.

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
`OfflineCashHardwareProfileV1` (`HardwareProfileV1`). Its stable
`hardware_profile_id` binds the provider and platform/product class, firmware
policy, governed credential issuer, physical qualification report, validity,
and the exact `0xffff` required-capability mask:

1. exact-next predecessor consumption;
2. one-use successor authorization;
3. rollback-resistant counter and journal;
4. sealed transition inputs and deterministic recovery seeds;
5. one-use acceptance-ticket issuance;
6. durable inbox byte reservation;
7. authenticated inbound staging, exact deduplication, and paging;
8. authoritative replay-root and external sparse-tree recovery;
9. sender outbox byte reservation before predecessor lock;
10. authenticated durable retry outbox;
11. atomic commit bound to a Core-verified candidate digest;
12. recoverable terminal commit certificate;
13. trusted time or a secure monotonic authorization lease;
14. offline hardware-epoch rotation;
15. rollback-safe counter rollover; and
16. no software fallback.

Raw OEM or platform attestation is verified during online enrollment.
Governance then issues a compact, circuit-friendly
`OfflineCashHardwareCredentialV1` (`HardwareCredentialV1`) binding the device
transition key, network, lane commitment, profile, firmware policy, policy
epoch, hardware epoch/generation, issuance time, and expiry.
The credential and profile-membership proof, not the raw certificate chain, are
used by offline circuits. Stock KeyMint, StrongBox, Secure Enclave, or App
Attest remains online-only unless an OEM/secure-element service implements and
qualifies the complete profile. There is no software downgrade.

The normalized circuit and recovery contract is defined in
[`offline_cash_guard_bundle_v1.md`](offline_cash_guard_bundle_v1.md); the
optional mobile ABI is defined in
[`offline_cash_device_bridge_v1.md`](offline_cash_device_bridge_v1.md).

## Requests, tickets, and capacity

`PaymentRequestV1` is a signed exact-payment invitation. It binds the
authenticated release, network, the exact `AxtAssetIncarnationV1` token and
asset scale, pooled-reserve identity, recipient account, complete compact
hardware credential, one positive exact amount, request ID, and validity
window. It does not bind the receiver's current aggregate-state head and does
not carry a request mode, cumulative total, payment-count ceiling, or amount
interval. Any number of distinct valid payments may use the same request; an
application may separately decide whether those payments satisfy an invoice.

Before requesting receiver capacity, the sender creates a compact canonical
`OfflineCashAcceptanceIntentV1`. It binds the exact signed request digest, a
fresh random intent ID, one positive exact amount, and a randomized
`sender_one_time_commitment`. The sender transports that intent inside an
`OfflineCashAcceptanceIntentAuthorizationV1` whose statement also binds the
exact authenticated release, suite, verifying-key digest, and artifact
manifest. Both proof parities hide the sender profile and credential while
proving membership in that release's enabled-profile set, sufficient private
balance, and a qualified one-use predecessor authorization for the exact
request and amount. The receiver's authenticated native verifier must verify
that proof before hardware persists an intent or reserves inbox bytes. The
compact intent alone has no reservation authority.

The final wrapper opens `sender_one_time_commitment` only as a private witness
and proves that it authorizes the exact private predecessor consumed by sender
hardware. Neither authorization nor payment reveals a sender credential, key,
lane, epoch, counter, predecessor, or successor. The later payment embeds only
the compact intent whose digest is signed by the ticket; the proof-bearing
authorization is a distinct pre-ticket message.

Receiver hardware processes the verified authorization and ticket issuance as
one atomic private-ledger operation. It checks that the intent amount equals
the request amount and records the terminal intent-to-ticket decision so the
same intent cannot issue twice or resolve to different ticket bytes. Distinct
intents against the same request are independent and never consume a
request-local count or amount budget. Resolved decisions may be compacted into
an authenticated exact-decision accumulator, with paged nodes outside
hardware, but compaction must preserve duplicate/conflict answers and cannot
introduce a historical count bound. A ticket's physical inbox reservation is
not reclaimed merely because the ticket expires or appears unused. Relocation
and compaction preserve the same decision. Only a separate authenticated
no-commit closure may release an unresolved physical reservation, after
proving that the exact authorized predecessor and bound sender intent never
reached terminal hardware commit.

Before a sender can prepare, receiver hardware atomically applies that private
request ledger, allocates physical inbox space, and issues one one-use,
exact-amount `OfflineCashAcceptanceTicketV1`. The ticket binds:

```text
request_id and request digest
acceptance_ticket_id
exact AcceptanceIntentV1 digest and exact amount
asset identity, exact AxtAssetIncarnationV1 token, and scale
reserved_inbox_bytes
recipient one-time encryption key
hardware profile and policy epoch
public sender-commit deadline
receiver-hardware signature under the credential carried by the request
```

The reservation covers the maximum canonical payment, staging metadata, and
acknowledgement record. Hardware stops issuing tickets before free capacity
falls below that allocation. Ticket expiry is only the last valid sender-commit
time; it does not prove that no in-window commit occurred and therefore does
not release the reservation by itself. An apparently unused allocation may be
moved only by governed online recovery that preserves an equivalent durable
delivery slot, and relocation alone does not restore a request amount or count
slot. An authenticated no-commit closure may release both only when it proves
that the exact sender intent never reached terminal commit; otherwise neither
is reclaimed. A
payment carrying valid terminal commit evidence is accepted into that
allocation even after delivery delay, receiver rotation, ticket expiry, or
later traffic. Exact duplicate bytes recover the byte-identical receipt; the
same ticket or credit ID with different bytes is corruption.

The sender applies the same rule. Before locking a predecessor it reserves
enough durable outbox and recovery space for sealed inputs, both proof stages,
the terminal certificate, canonical envelope, and retry metadata. If that
reservation cannot be made, preparation does not start. The reservation is an
`OfflineCashOutboxReservationV1`. After hardware commit,
the reservation cannot be reclaimed until the canonical envelope is installed
and its delivery acknowledgement or terminal online submission is durably
recorded. A missing acknowledgement does not roll back the successor or stop it
from being spent; it only retains the retry record and can prevent a later send
from starting when physical capacity is exhausted.

`OfflineCashDurableCapacityV1` rejects lane configuration below 298,640 inbox
bytes or 90,274 outbox bytes. The inbox value covers one complete recoverable
receive operation. The outbox value is Core's implementation-storage floor for
one maximum live payment or redemption slot, including typed state and its
byte-identical retry encoding; it is not the public `reserved_outbox_bytes`
value. Additional concurrent live records require additional storage, and
neither floor is a protocol history limit.

Staging and folding convert, rather than reacquire, precommitted capacity.
Core derives the complete pending-credit, retained-receipt, and consumed-index
projection, releases the folded ticket on a private ticket-book successor,
recomputes the exact meters once against the pre-fold committed ceiling, and
only then installs that successor. Candidate persistence, hardware commit, wrapper
installation, exposure, and retry likewise consume the sender's original
reservation; no terminal stage performs a second capacity admission. A failed
unrelated admission leaves the canonical durable bytes unchanged.

## Prepare, prove, commit, and delivery

`SendSplit` and `RedeemSplit` use the same recoverable state machine:

1. **Prepare.** Hardware verifies the credential, request and ticket where
   applicable, reserves the complete outbox budget, locks the exact predecessor,
   and seals every transition input and every proof/randomness seed. It does not
   yet consume the predecessor or expose money.
2. **Prove.** The audited native core uses the exact authenticated release to
   create a sealed precommit candidate envelope containing the permitted public
   projection, encrypted output, and recursive transition proof. The candidate
   is not a payment or voucher.
3. **Persist and verify.** Core durably stores the exact canonical candidate,
   cryptographically verifies both proof parities under the release-pinned
   verifier keys and size policy, and derives its canonical candidate digest.
   Success advances an authenticated-release-backed persisted-candidate
   typestate; canonical bytes or a successful data-model shape check cannot
   create that authority.
4. **Commit.** Core constructs the hardware commit call only from that internal
   candidate capability. Hardware atomically consumes the predecessor, installs
   its sole successor, converts the reservation into a retry-outbox entry bound
   to the candidate digest, and emits a recoverable
   `OfflineCashCommitCertificateV1`.
5. **Wrap.** From the sealed candidate, terminal certificate, and sealed prover
   seed, Core creates or recovers the final
   `OfflineCashCommitWrapperProofV1`, cryptographically verifies it under the
   same authenticated release, and durably installs the canonical `PaymentV1`
   or redemption voucher in the outbox.
6. **Expose.** Only the final installed envelope may be sent or submitted.

For `SendSplit` and `RedeemSplit`, the transition proof statement's
`effect_digest` equals the canonical semantic digest of the exact prepared
public projection, including its opaque commit evidence. Core checks this
binding before candidate persistence, so changing commit evidence is rejected
even when the amount, lifecycle, and transition nullifier still match.

A crash before step 4 can resume or abandon the prepared record without
consuming the predecessor. A crash during or after step 4 recovers the one
terminal certificate and the sealed candidate, regenerates byte-identical
wrapper/envelope bytes, and never burns the balance or authorizes a second
successor. Proof generation is deterministic from the sealed seeds for this
purpose. A precommit candidate or prepare certificate is never accepted as
money.

The data-model `validate_shape*`, canonical decode, digest, and signature
helpers reject malformed or inconsistently bound wire values only. Monetary
admission is a Core operation that derives authority from an
`OfflineCashAuthenticatedReleaseV1`, verifies the governed profile and
credential, runs the native recursive/helper/wrapper proof verifiers, and
passes non-forgeable internal typestates between stages. Callers cannot replace
those checks with self-described release, profile, verifier-key, or proof bytes.

Canonical Norito remains the sole transport encoding, but a transport archive
is not a circuit transcript. CommitWrapper-bound semantic hashing uses explicit
fixed-width V1 layouts: acceptance intent (114 bytes), intent authorization
statement (244), no-commit statement (498), outbox reservation (56), the commit
evidence fragment embedded in a certificate (36), commit-certificate ID
preimage (238), and commit certificate (270). Integers and enum tags in those
layouts are unsigned little-endian values; fixed digests are copied as their
raw 32 bytes in the field order defined by the data model. Each outer semantic
digest or certificate ID is
`SHA256(domain || 0x00 || u64_le(transcript_length) || transcript)`. SDKs must
consume the same fixture vectors and may not substitute a Norito header,
padding, or checksum for any of these circuit inputs. This separation does not
create a second transport or decoder.

The receiver validates the complete final wrapper and ticket before staging,
but uses the ticket's already-allocated bytes. It ACKs only after the exact
payment bytes, ticket ID, and credit ID are rollback-resistently persisted.
Delivery time does not replace the authenticated sender commit time.

## Public transcript and privacy

Apart from fixed framing and the referenced request/ticket context,
`PaymentV1` exposes only:

- one pseudorandom transition nullifier/credit ID;
- the canonical acceptance intent, request, and exact-amount ticket bindings;
- the recipient one-time key;
- the amount/ciphertext commitment and ciphertext digest;
- the qualified hardware profile and policy epoch;
- an opaque trusted-time or lease evidence commitment; and
- the compact commit certificate and constant-size paired commit-wrapper proof.

The payment embeds the complete `OfflineCashAcceptanceTicketV1`, and its amount
must equal the ticket amount. It also embeds the exact
`OfflineCashAcceptanceIntentV1`; the ticket signs its intent digest and the
wrapper privately opens the intent's sender commitment.
`OfflineCashCommitEvidenceV1` is exactly
`TrustedTime { time_evidence_commitment }` or
`MonotonicLease { lease_evidence_commitment }`. The signed request/ticket policy
deadline remains public, but the terminal proof's exact deadline copy, actual
commit time, lease identity and window, authorization counter, clock authority,
and raw clock/lease evidence are private witnesses committed by that opaque
value. A public counter, lease ID, or clock epoch must not become a
cross-payment pseudonym.

Every `OfflineCashPairedProofV1` uses fresh statement-scoped rerandomized
credential audits and history accumulators in its two parity components. Those
values are neither reusable credential pseudonyms nor stable profile, lane, or
device identifiers; equality across parity roles or reuse across statements is
rejected by the release relation.

The predecessor and successor commitments, balances, state nonces, lane,
sequence, hardware epoch, replay paths, and received-fund provenance remain
private witnesses. The nullifier is deterministic for one consumed predecessor,
so competing successors conflict, but honest consecutive transitions are not
linkable. Neither a public sender conflict key nor a hash that permits matching
one payment's successor to another payment's predecessor is emitted. The
compact certificate exposes no stable device key, credential ID, lane, or
hardware epoch; its hidden hardware authentication is verified by the wrapper.

`OfflineCashAcknowledgementV1` exposes only the request digest, payment digest,
an `OfflineCashInboxReceiptV1 { credit_id, receipt_commitment }`, and the
receiver signature. The receipt commitment hides the persisted lane, hardware
epoch, exact-next inbox sequence, acknowledgement time, payment, and credit.
Those raw values are private hardware/Core witnesses and are not transport
fields or stable receiver pseudonyms.

## Encrypted credit envelope

Every peer or mint `encrypted_credit` field is the canonical Norito encoding
of `OfflineCashEncryptedCreditEnvelopeV1 { version = 1,
ephemeral_x25519_public_key, nonce, ciphertext_and_tag }`. The ephemeral key is
exactly 32 bytes, the XChaCha20-Poly1305 nonce is exactly 24 bytes, the combined
ciphertext ends with its 16-byte tag, and the complete encoded envelope is at
most 384 bytes. Low-order or all-zero X25519 public keys and an all-zero shared
secret are rejected. A fresh ephemeral secret and nonce are generated for each
envelope; nonce freshness is a qualified-service property, not a heuristic
decoder check.

The authenticated plaintext is the canonical fixed-size
`OfflineCashCreditOpeningV1 { version = 1, credit_id, amount,
credit_commitment_opening, recipient_binding_opening, recovery_nonce }`, at
most 256 bytes. Its three openings/nonces are non-zero, and qualified hardware
rejects any `credit_id` or amount that does not exactly equal the public
statement.

The recipient key in a signed ticket or mint-authorization context is an
X25519 public key. For recipient key `R`, ephemeral public key `E`, and raw
X25519 shared secret `DH`, derive the 32-byte XChaCha key with HKDF-SHA256:

```text
salt = SHA256("iroha:offline-cash:v1:credit-envelope-salt\0" || R || E)
IKM  = DH
info = "iroha:offline-cash:v1:credit-envelope-key\0" ||
       SHA256(canonical OfflineCashEncryptedCreditAadV1)
```

The canonical AAD is exactly `{ version = 1, purpose = Mint | Peer,
context_digest, issuance_or_transition_commitment, credit_id, amount }`. Mint
uses the pre-ID mint-authorization context digest and issuance commitment. Peer
uses a pre-ID context binding the exact request, compact intent, ticket, and
lifecycle fields plus the ID-independent credit-opening/transition commitment.
Ciphertext, ciphertext digest, and proof bytes never enter the AAD or any
pre-ID commitment. XChaCha20-Poly1305 authenticates the canonical AAD and
stores ciphertext followed by the tag.

The `k = 16` recursive circuits do not arithmetize X25519, HKDF, or AEAD.
Sealing and opening must be performed through one audited native core and a
qualified hardware service; the proofs constrain hardware authority,
credential and opening commitments, public projections, and their exact
semantic digests.

## Lifecycle and release binding

Every V1 credit carries an `OfflineCashLifecycleBindingV1` with `network_id`,
`protocol_version`, `suite_id`, `vk_digest`, `release_id`, asset identity, the
exact typed `AxtAssetIncarnationV1`, scale, pooled-reserve identity,
`hardware_profile_id`, `policy_epoch`, operation kind, request/ticket/credit
IDs, and ciphertext
digest. The wrapper additionally binds lane commitment and hardware epoch as
private witnesses. Those private lifecycle fields are not transported.

An ordinary suite rotation first retains the old verifier for offline use or
activates a `SuiteUpgrade` relation with separately governed exact from/to
bridge evidence. `SuiteUpgrade` recursively verifies the old state, carries the
entire private balance and replay root, and creates one state under the new
suite. A generic suite-upgrade circuit measurement does not authorize that
bridge. A suite may become verification-only, but its verifier cannot be
removed while unbridged credits can exist.
Credential or policy-epoch rotation likewise cannot invalidate a payment whose
terminal commit satisfied its ticket and lease.

Emergency compromise may suspend new ticket issuance, top-ups, and offline
commits for an affected profile. Suspension never invalidates a previously
committed ticket-backed payment. Historical verification and an online
redemption/recovery corridor remain available for legitimate credits, using
terminal-nullifier deduplication and the issuer loss policy where compromise
creates excess claims. Normal and emergency controls operate on profiles and
issuance, never on the provenance of received funds.

## Mint-finality genesis trust root

The final signed genesis carries
`ConsensusHandshakeMetadata.offline_cash_mint_finality` as
`OfflineCashMintFinalityGenesisParametersV1`. It contains one mandatory epoch-zero
`OfflineCashMintFinalityEpochRosterTemplateV1` and an optional epoch-one template; the latter is
required only when height one is the epoch-zero boundary and is rejected otherwise. These are
networkless templates because the final `NetworkId` does not exist until the canonical hash of the
final signed genesis is known. A template must never contain, guess, or derive a provisional
network identity.

Core authenticates the signed genesis first, derives
`NetworkId = hash(final signed genesis)`, and only then binds the templates into
`OfflineCashMintFinalityEpochRosterV1` values used by the first `HeightContext` and any authenticated
epoch-one successor snapshot. The closed first-release genesis profile contains exactly four
validators. Its template must match the frozen Sumeragi roster in exact order and `PeerId` identity
at all four positions; a reordered, missing, additional, or substituted validator fails closed.
Each validator's Eq/Fp helper key must decode as a canonical non-identity Pallas point and its Ep/Fq
helper key as a canonical non-identity Vesta point. Zero, duplicate, non-canonical, and identity
encodings have no authority.

Provisioning derives the two public keys from a separately provisioned validator-local seed, the
epoch, and the validator's canonical `PeerId`, with a distinct parity domain for each curve. It
never derives a Pasta key from the validator's BLS key, and it does not add `NetworkId` to public-key
derivation because that would reintroduce the genesis fixed point. Every validator must use a seed
unique to that validator and deployment; deployments must not reuse seeds across networks.

Only the roster templates carried by signed genesis are networkless. A bound runtime roster
contains the final `NetworkId`, and its `finality_epoch_id` hashes that network, epoch, exact
ordered identities, and both public keys. `HeightContext` carries the complete bound roster and
identifier. Every mint-finality seal message binds the same `NetworkId`, `HeightContextId`, block
height, subject, and execution commitment, while deterministic Schnorr nonce derivation also
includes `NetworkId`. Consequently a bound roster, seal, or nonce from another deployment cannot
be replayed.

The templates are deliberately excluded from the secondary Sumeragi consensus-parameters
fingerprint: their exact bytes are already authenticated beside that fingerprint by the final
signed genesis. Snapshot reconstruction preserves only the stable three-field
`SumeragiV2GenesisContextParameters` (`da_layout`, `nexus_amx_context_hash`, and
`execution_policy_hash`); an authenticated snapshot restores the full network-bound
`HeightContext` rather than reconstructing networkless genesis templates.

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

A top-up first verifies a proof-bearing `OfflineCashMintAuthorizationV1`, then
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
`OfflineCashMintCreditStatementV1` is deliberately
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
separately framed `oc1:` text bytes; those terminal-trio caps do not silently
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
[`formal/offline_cash_v1/OfflineCashV1.tla`](../formal/offline_cash_v1/OfflineCashV1.tla).
Finite TLC sets bound only model exploration and are never protocol limits.
