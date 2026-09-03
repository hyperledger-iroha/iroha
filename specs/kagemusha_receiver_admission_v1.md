# KAGEMUSHA V1 receiver admission

Receiver admission accepts a committed `KagemushaPaymentV1` into a
rollback-resistant hardware inbox and returns one durable
`KagemushaAcknowledgementV1`. There is no intent, ticket, request-local budget,
reservation message, invoice counter, or cancellation protocol.

## Admission contract

For each delivered payment, Core and qualified receiver hardware verify:

- canonical KAGEMUSHA V1 framing and the Request=1/Payment=2 message kinds;
- the complete signed request, positive exact amount, recipient account/lane,
  recipient encryption key, hardware policy, request ID, and validity window;
- trusted sender hardware commit occurred within that window;
- direct payment/request/amount/receiver binding;
- unique transition nullifier and request-derived credit ID;
- sender before/after state commitments and constant-size paired recursive
  proof;
- normalized sender `GuardBundle`, actual terminal candidate, recoverable
  hardware commit certificate, and hardware terminal commitment;
- ciphertext and opening commitments plus recipient-only decryption and
  authenticated opening fields; and
- exact canonical payment bytes used for replay identity and recovery.

The sender hardware profile may differ from the receiver profile. Both must be
authenticated under the payment's release. A host signature or certificate
without recursive guard verification grants no monetary authority.

Request expiry applies only to trusted sender commit time. Once committed in
window, a valid payment remains acceptable after arbitrary delivery delay,
device restart, request expiry, ordinary verifier rotation, or policy refresh.

## Atomic staging and acknowledgement

Receiver hardware performs one atomic, recoverable inbox transition:

1. authenticate the current inbox journal/root and available physical storage;
2. validate the complete request, payment, proof, certificate, and opening;
3. persist the exact canonical payment and opening bytes plus credit-ID replay
   identity in rollback-resistant storage;
4. commit the next inbox revision once; and
5. return the exact durable `KagemushaInboxReceiptV1`.

Core signs and exposes `KagemushaAcknowledgementV1` only after step 4. The ACK
binds request digest, payment digest, credit ID through the receipt, and the
rollback-resistant inbox revision. A lost response recovers the same receipt
and byte-identical ACK without staging twice.

Exact duplicate delivery of the same credit ID and bytes returns the durable
ACK. The same credit ID with different request, payment, ciphertext, opening,
or proof bytes is a conflict and must not mutate storage. Distinct credit IDs
against the same request are independent valid payments and all stage. Invoice
satisfaction, overpayment, and business-level deduplication are application
concerns and cannot invalidate money.

## Capacity and backlog

Finite storage can apply backpressure before a new payment is accepted. It
cannot authorize dropping, cancelling, or refusing a credit after durable
staging. No admission decision depends on hop count, ancestry, origins, proof
depth, prior receipt count, current request count, or how many other valid
payments used the same request.

The hardware inbox can stage many payments concurrently while a monetary lane
serializes aggregate balance transitions. Background processing folds one
staged credit at a time with `ReceiveFold`. Before a send or redemption, the
wallet synchronously folds whatever pending credits are required for the amount.
Backlog may increase latency but never makes a staged credit unspendable.

Folding changes the private balance and consumed-credit sparse-Merkle root; it
does not delete the durable replay identity or reopen an invoice allowance.
Compaction must preserve authenticated exact duplicate/conflict behavior.

## Recovery invariants

Snapshot and journal recovery authenticate every inbox record and its exact
canonical-byte digest, credit ID, receipt, revision, and fold state. Comparing
only byte totals or counters is insufficient. Recovery cannot:

- roll back a committed inbox revision;
- manufacture or discard an accepted credit;
- acknowledge bytes that were not durably staged;
- fold one credit more than once;
- change the payment/opening bytes behind an existing receipt; or
- advance the monetary state before the corresponding `ReceiveFold` proof and
  hardware transition commit.

Corrupt or missing external sparse-tree nodes are a recovery failure, not
authority to ignore the hardware-owned replay root. Cloneable secure-state
backup is forbidden because it would enable double spending.

## Qualification

Focused admission tests cover shuffled concurrent requests, multiple distinct
payments per request, delivery after expiry, duplicate transport, credit-ID
conflicts, wrong receiver lane/key/policy, proof and certificate substitution,
ciphertext/opening substitution, stale inbox state, restart, and byte-identical
lost-response recovery.

Crash injection is required before and after validation, storage reservation,
payment persistence, opening persistence, inbox commit, receipt creation, ACK
persistence, and ACK exposure. Full-capacity and long-backlog tests must show
that accepted value is conserved and remains foldable/spendable. Physical
airplane-mode, power-loss, clock-rollback, backup/restore, thermal, latency,
memory, and throughput qualification is required for every hardware profile.

The proof limit remains 6,528 bytes and the complete three-message exchange
remains within the 9,211 raw / 12,288 text gates. These are current-message
transport limits and never cumulative-history limits.
