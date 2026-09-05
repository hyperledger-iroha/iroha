# KAGEMUSHA V1 peer transport

KAGEMUSHA has one transport-neutral three-message exchange:

1. Receiver: signed `KagemushaPaymentRequestV1`.
2. Sender: `KagemushaPaymentV1`, containing the committed receiver-bound
   credit, its recoverable hardware transition certificate, and constant-size
   paired recursive `KagemushaPaymentProofV1`.
3. Receiver: `KagemushaAcknowledgementV1`, only after irreversible secure
   staging in the rollback-resistant accepted-credit inbox.

IPM1 kinds are exactly Request (`1`), Payment (`2`), and Acknowledgement (`3`).
There are no acceptance-intent, acceptance-ticket, precommit, cancellation, or
compatibility message kinds. Binary values use canonical Norito; text is exactly
`kgm1:` plus unpadded base64url of one canonical value.

## Semantics

The request binds release, network, exact asset/incarnation/scale, pooled
reserve, recipient account and device lane, recipient encryption key, required
hardware policy, request ID, positive exact amount, validity window, and
signature. It never contains the receiver's current balance head.

The sender validates the request and prepares one receiver-bound credit. Sender
hardware commits an exact-next balance transition once within the request
window. The resulting payment binds the request digest and amount, unique credit
ID and transition nullifier, sender before/after commitments, receiver/request
binding, recipient-encrypted credit opening and ciphertext commitment,
normalized hardware transition commitment, trusted commit time and evidence,
and the constant-size recursive proof. Host signatures or certificates alone
grant no monetary authority: the proof verifies the normalized hardware guard
bundle and monetary transition.

Request expiry gates only the sender hardware commit time. A credit committed
inside the window remains acceptable, foldable, spendable, and redeemable
indefinitely. Sender funds become an irrevocable receiver-bound credit at
hardware commit. The sender successor, including a zero-balance successor after
a full spend, is immediately usable. A missing acknowledgement retains only the
byte-identical retry outbox and never freezes the remainder. Exposed credits
cannot be cancelled.

Receiver hardware verifies the complete payment and atomically stages its exact
canonical bytes before signing the acknowledgement. The acknowledgement binds
the request, payment, credit ID, and rollback-resistant inbox receipt. Exact
duplicate delivery returns the same durable acknowledgement; reuse of one
credit ID with different bytes fails closed.

Any number of receiver requests may be live simultaneously. Distinct valid
payments made against the same request are all accepted. The protocol deduplicates
credits, not invoices; detecting an already-satisfied or overpaid invoice is an
application concern and cannot make valid money unspendable.

The accepted-credit inbox may stage many credits while one monetary lane
serializes balance transitions. Credits fold continuously in the background.
Before sending or redeeming, the wallet synchronously folds whatever pending
credits are required. Backlog can add processing latency, but neither a count nor
history-depth limit may reject a valid credit or prevent its later use.

## Bounds

These limits protect one message parser and physical transport. They never bound
cumulative history, received-note count, fan-in, origins, hops, ancestry, or
recursive proof depth.

| Value | Canonical binary maximum | `kgm1:` text maximum |
| --- | ---: | ---: |
| `KagemushaPaymentRequestV1` | 928 bytes | 1,243 bytes |
| `KagemushaPaymentV1` | 7,552 bytes | 10,075 bytes |
| `KagemushaAcknowledgementV1` | 256 bytes | 347 bytes |
| Complete exchange hard gate | 9,211 bytes | 12,288 bytes |
| Paired recursive proof | 6,528 bytes | Included only in the payment |

Every frame counts exactly once. Decoders check the applicable byte cap before
allocation, require canonical V1 framing, reject trailing bytes, and validate
the complete context. Synthetic maximum-size proof fixtures establish transport
bounds, not cryptographic or hardware qualification.

A smaller implementation-specific limit cannot be advertised as an
offline-capable KAGEMUSHA profile. Finite storage, memory, battery, bandwidth,
hardware lifetime, and `u128` arithmetic remain physical bounds; protocol
admission cannot impose history-count limits.

## QR, NFC, and local radio

A static QR carries an independently framed `kgm1:` value only if it fits.
Animated QR frames include stream ID, kind, total length, offset, payload, and
checksum. Reassembly accepts any ordering, deduplicates identical overlap,
rejects conflicting overlap, and decodes only a complete bounded value. Frame
count depends on the current bounded message, never payment history.

NFC, Nearby, Multipeer, and equivalent links carry the same canonical binary
values with explicit kind and length, using the same overlap/conflict rules.
Transport encryption never replaces the recipient-only credit envelope,
recursive proof, or hardware authorization. Staging must finish before ACK;
plaintext metadata must not expose balance openings, replay paths, stable
credential audits, or journals.

## Conformance and qualification

Rust, Swift, Kotlin, mirrored Java, JavaScript, Python, C#, JNI, QR, and NFC must
re-encode identical canonical fixtures. Tests cover concurrent and shuffled
requests, multiple valid payments against one request, delayed post-expiry
delivery, restart, exact retries, conflicting credit-ID reuse, stale state,
forked successors, rollback, hardware counter reuse/skip, epoch rotation,
overflow, proof/output substitution, truncation, framing, and transport bounds.

Codec conformance is necessary but not sufficient. Release qualification also
requires real recursive proofs across long histories, pooled-reserve settlement,
crash recovery at every durable boundary, multi-peer tests, and physical
airplane-mode, restart, power-loss, clock-rollback, backup/restore, thermal,
latency, memory, and throughput evidence for every approved hardware profile.
