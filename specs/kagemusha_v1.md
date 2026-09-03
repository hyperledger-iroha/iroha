# KAGEMUSHA V1

KAGEMUSHA V1 is the sole first-release hardware-cash protocol. Canonical text
transport is `kgm1:` followed by unpadded base64url of one canonical Norito
value. V1 has one decoder and no legacy protocol selector, alias, migration,
dual-write path, or deprecated message kind.

## Guarantee

Correctly formed device-to-device payments remain usable regardless of how many
prior transfers, receipts, origins, folds, or recursive proof steps produced
the value. KAGEMUSHA therefore has no protocol limit on hops, ancestry, fan-in,
note count, origin count, received-fund provenance, or proof depth. A recipient
that accepts 1,000 one-unit credits can fold them into one balance and make one
normal 1,000-unit payment.

Checked `u128` asset arithmetic, finite storage, memory, battery, bandwidth,
processing time, hardware lifetime, and permanent device failure remain physical
bounds. They cannot be turned into history-count admission rules. Backlog may
delay a send or redemption while required credits fold, but a valid accepted
credit cannot become unspendable because of its age, ancestry, or position in a
queue.

## Aggregate monetary state

Each `(network, device lane, asset, asset incarnation)` has one serial,
hardware-controlled private state:

```text
balance: u128
device and hardware-policy binding
hardware epoch and exact-next logical sequence
consumed-credit sparse-Merkle root
state nonce
public state commitment
constant-size recursive proof
```

The balance, replay data, nonce, lane identity, predecessor, and successor
openings remain private. Public commitments, proofs, peer messages, and verifier
work are independent of history length. An authenticated external sparse-tree
database may store replay nodes, but qualified hardware retains and authorizes
the authoritative root. The exact local replay index grows with accepted credit
count; loss or corruption of that index is a recovery fault, not authority to
ignore the committed root.

Every monetary transition consumes exactly one predecessor and creates exactly
one successor. Full sends and full redemptions still create a zero-balance
successor so sequence and replay continuity never disappear. Balance and reserve
arithmetic is checked; overflow, underflow, stale predecessor use, or multiple
successors from one predecessor fails without mutation.

## Fixed recursive operations

KAGEMUSHA uses fixed-shape paired-Pasta recursive relations:

- `Bootstrap` establishes the hardware-bound zero state.
- `MintFold` absorbs one finalized reserve-backed top-up credit.
- `SendSplit` subtracts any positive amount and creates one receiver credit plus
  the sender successor.
- `ReceiveFold` verifies and adds one staged credit, proves credit-ID
  nonmembership, and updates the replay root.
- `RedeemSplit` creates a full or partial redemption voucher plus the successor.
- `Rotate` carries the entire balance and replay root to the exact next hardware
  epoch without an online checkpoint.

Internal terminal authorization proves the actual persisted state candidate and
the normalized hardware guard. The compact outer wrapper recursively consumes
that relation. A peer payment or redemption carries one constant-size paired
proof; no branch path, note inventory, provenance array, hop counter, input
maximum, ancestry witness, or proof-step admission maximum exists.

Incoming credits fold continuously in the background. Before a send or
redemption, the wallet synchronously folds as many pending credits as necessary
for the requested amount. One device lane serializes monetary transitions while
its rollback-resistant inbox may stage many credits concurrently.

## Peer protocol

There are exactly three peer messages. Their full transport contract is defined
in [`peer_transport_v1.md`](peer_transport_v1.md).

1. `KagemushaPaymentRequestV1` binds release and network, asset identity and
   scale, pooled reserve, positive exact amount, recipient account/lane/key,
   required hardware policy, request ID, validity window, and signature. It
   never binds the receiver's balance head.
2. `KagemushaPaymentV1` binds the request and receiver, amount, unique credit ID
   and transition nullifier, sender before/after commitments, encrypted credit
   opening, normalized hardware transition commitment, trusted commit time and
   evidence, and the constant-size proof.
3. `KagemushaAcknowledgementV1` binds the request, payment, credit ID, and
   rollback-resistant inbox receipt after irreversible secure staging.

Any number of requests may be active simultaneously. Distinct valid payments
against the same request are all accepted; invoice satisfaction and overpayment
are application concerns. Exact duplicate delivery returns the byte-identical
durable acknowledgement. Reusing a credit ID with different bytes fails closed.

Request expiry gates only the sender's trusted hardware commit time. A payment
committed within the window remains acceptable and foldable indefinitely,
including after delivery delay, restart, policy refresh, or ordinary verifier
rotation.

## Hardware authority

Offline authority requires an attested non-forking provider implementing all of
the following as one qualified contract:

- exact-next state transitions or one-use successor keys;
- rollback-resistant journal, logical sequence, and accepted-credit inbox;
- trusted commit time or a securely bounded monotonic authorization lease;
- atomic, recoverable transition certificates;
- authenticated durable monetary state and payment retry outbox;
- sealed transition intent and deterministic recovery material;
- authoritative replay-root custody with authenticated sparse-tree recovery;
- rollback-safe hardware-counter rollover and offline epoch rotation; and
- no software fallback.

Raw OEM attestation is checked during enrollment. Governance publishes an
approved hardware profile and compact circuit-verifiable credential binding the
provider/product class, network, lane commitment, transition key, policy,
firmware, epoch/generation, and validity. The recursive relation verifies the
normalized `GuardBundle`; a host-side signature or certificate alone grants no
monetary authority. Stock KeyMint, StrongBox, Secure Enclave, App Attest, or an
equivalent signing service remains online-only unless its complete
journal/counter/inbox/outbox contract is implemented and physically qualified.

The normalized contract is defined in
[`kagemusha_guard_bundle_v1.md`](kagemusha_guard_bundle_v1.md). The optional
mobile ABI is defined in
[`kagemusha_device_bridge_v1.md`](kagemusha_device_bridge_v1.md).

## Atomic sender persistence

The sender performs one recoverable transition:

1. persist sealed transition intent, all proof inputs, randomness, and exact
   canonical-envelope recovery material;
2. reserve durable outbox capacity;
3. construct and locally verify the real candidate relation;
4. commit the hardware predecessor exactly once;
5. recover or generate and verify the terminal proof; and
6. persist the canonical payment envelope before any exposure.

Recovery resumes the same transition and can never recreate a consumed
predecessor, choose another successor, change the receiver, or expose different
bytes. At hardware commit, the transferred amount is irrevocably receiver-bound
and the sender remainder is immediately usable. Missing acknowledgements keep
only the byte-identical retry outbox; they never freeze the wallet. An exposed
credit cannot be cancelled.

The receiver ACKs only after atomic authenticated staging of the exact payment
and opening bytes. Crash recovery at every journal, commit, proof, state,
inbox, outbox, transport, and acknowledgement boundary must conserve value and
reproduce every externally exposed byte.

## Recipient encryption and privacy

The request's recipient key is authenticated and bound into the payment proof.
The credit opening is encrypted only for that key with a release-pinned KEM/AEAD
suite and canonical associated data that excludes the ciphertext itself. The
opening repeats and authenticates the credit ID, amount, asset identity,
receiver binding, and commitments needed by `ReceiveFold`.

Stable credentials, raw attestations, balance openings, replay paths, hardware
counters, and private lane identities are not peer-visible. Proof/output,
ciphertext, request, receiver-key, hardware-transition, and release substitution
must fail before durable acceptance.

## Pooled reserve and settlement

Each `(network, asset, asset incarnation)` has one consensus-accounted reserve:

```text
reserve = total finalized top-ups - total finalized redemptions
```

A top-up atomically debits online funds, credits that reserve, records an
idempotent operation, and emits one unique device-bound mint credit with
circuit-verifiable block finality. Peer transfers and local folds never change
the reserve.

A redemption verifies the recursive aggregate state and hardware voucher,
consumes one unique terminal nullifier, debits the reserve, and credits the
requested account atomically. Full and partial redemption use the same relation.
Duplicate operation IDs are idempotent; conflicting reuse, duplicate or
concurrent nullifier consumption, and reserve underflow reject without partial
mutation.

There are no per-top-up anchors, provenance buckets, provider buckets, claims,
branch paths, or lineage endpoints. Pooling preserves unrestricted mixing of
received funds and deliberately makes approved circuits and hardware providers
one trust domain per asset pool. Admission, phased rollout, issuance controls,
reserve/nullifier accounting, emergency suspension of new issuance/redemption,
and explicit loss policy mitigate that larger compromise blast radius. They do
not restrict peer spending of already committed money.

The generic online routes are:

- `GET /v1/kagemusha/readiness`;
- `POST /v1/kagemusha/top-up`;
- `POST /v1/kagemusha/redeem`; and
- the corresponding operation-status resource.

Their schemas are KAGEMUSHA V1 only. There is no lifecycle route and no
anchor/lineage API.

## Wire and proof bounds

The paired recursive proof is at most 6,528 bytes. A complete raw/text peer
exchange remains approximately 9,211/12,288 bytes. These are current-message
transport bounds, not history bounds. Proof length and verifier work must be
independent of history at depths 8, 64, 1,024, and beyond.

All Rust, Swift, Kotlin, mirrored Java, JavaScript, Python, C#, JNI, QR, and NFC
implementations consume and produce identical canonical Norito values. No SDK
may preserve removed tags, names, decoders, or conversion aliases.

## Recovery and backup consequences

Cloneable secure-state backup would permit double spending and is forbidden.
Backup/restore may recover public configuration and online-only data but cannot
clone a monetary predecessor or roll hardware state backward. Permanent loss of
qualified secure state behaves like loss of physical cash unless a separately
specified online loss policy compensates it from outside the offline balance.

Offline rotation carries the complete balance and replay root into one exact
successor epoch and invalidates the old epoch. Counter rollover is the same
one-successor operation and cannot strand funds.

## Release qualification

KAGEMUSHA is not release-qualified until all of the following evidence is green:

- 1,000 independently funded devices pay one unit; the merchant folds all
  credits, sends 1,000 units once, and the next recipient spends and performs
  full and partial redemption;
- at least 1,024 real recursive handoffs plus longer model/property histories,
  with constant proof size and verification work;
- shuffled concurrent requests, repeated valid payments against one request,
  delayed delivery, duplicate transport, credit replay/conflict, stale state,
  predecessor forks, rollback, counter reuse/skip, forged rotation, overflow,
  and proof/output substitution;
- crash injection at every persistence and exposure boundary, proving value
  conservation and byte-identical recovery;
- reserve underflow, duplicate/concurrent redemption, top-up recovery,
  full/partial redemption, zero-balance continuation, verifier rotation, and
  hardware-counter rollover;
- canonical cross-SDK, JNI, QR, and NFC fixtures and behavior; and
- physical airplane-mode, restart, power-loss, clock-rollback, backup/restore,
  thermal, latency, memory, and throughput qualification for every enabled
  hardware profile.

The implementation remains under integration until those gates are satisfied.
Structural fixtures, mocked recursion, or host-only signatures do not establish
offline monetary authority.
