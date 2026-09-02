# Offline Cash V1 formal model

`OfflineCashV1.tla` is the finite-state safety oracle for the revised,
clean-slate aggregate recursive-balance protocol. It intentionally models one
asset and one pooled reserve. No state or message contains cumulative hops,
ancestry, origins, receipts, fan-in, transition history, or proof depth.

## What the model covers

- **Qualified hardware admission.** `HardwareQualified` and
  `NoSoftwareFallback` abstract successful online verification and governance
  issuance of `HardwareCredentialV1`. Exact-next predecessor locks,
  one-successor consumption, rollback-resistant sequence/epoch state, durable
  candidates, reserved retry outboxes, and recoverable terminal commit states
  model the safety-relevant hardware contract. These assumptions do not claim
  that TLC can qualify a real secure element.
- **Recoverable send and redemption.** Both operations move through
  `Prepared`, `Candidate`, `HardwareCommitted`, `WrapperGenerated`,
  `Installed`, `Exposed`, and explicit `Recovery` states. Prepare seals the
  private predecessor, inputs, and randomness. Candidate persistence happens
  before the monetary hardware commit. Crashes at wrapper generation,
  canonical installation, and exposure resume the same candidate-bound
  artifact; they cannot spend the predecessor again or create another
  successor.
- **Intent, exact ticket, then payment.** Before ticket issuance, the sender
  creates one unique `AcceptanceIntent` containing one exact positive amount
  and a freshly randomized commitment to the private sender opening. A
  separate proof-bearing `AcceptanceIntentAuthorization` then binds the intent
  to the sender profile, suite, policy epoch, and release. This gate uses the
  sender authorization profile, not the receiver ticket profile. Only an
  authorized intent can receive a ticket. The ticket binds that intent digest
  and exactly the same amount; payment cannot choose a value from an interval.
  Its candidate privately opens the committed sender and binds the exact
  predecessor. The logical exchange is request, intent, sender authorization,
  exact signed ticket, payment, then acknowledgement.
- **Pre-debit mint authorization.** `TopUp` requires a verified
  `MintAuthorization` before it can debit online value. Its exact statement
  binds recipient account, typed asset incarnation, amount/context,
  randomized recipient-credential commitment, ID-independent credit
  commitment, recipient KEM, and release-pinned profile/suite/policy fields.
  The recursive mint-helper binding carries the same authorization digest and
  exact credit commitment. Derived credit IDs and ciphertext fields are absent
  from the authorization preimage, preserving an acyclic pre-ID construction.
- **Exact reusable requests and atomic capacity.** Every request commits one
  exact positive amount. Each distinct valid sender intent against that request
  may receive its own one-use, capacity-backed ticket for exactly that amount;
  there is no invoice-level amount, payment-count, or request-use ledger.
  Ticket issuance reserves receiver bytes atomically. Send and redemption
  preparation also reserve sender outbox bytes. Staging atomically exchanges
  ticket bytes for staged-inbox bytes. Folding or acknowledgement frees the
  corresponding physical slot without weakening one-use ticket evidence.
- **No expiry reclaim.** Observing ticket expiry changes only evidence. It
  cannot release capacity. A separate
  authenticated no-commit recovery first enters `RecoveryPending`, preserving
  both reservations, and may close only while the bound predecessor remains
  unconsumed and no terminal payment commit exists. Only that closure releases
  physical capacity; the intent/ticket identity remains permanently closed.
  Relocation alone has no reclaim transition.
- **Fixed-shape receive folding.** Every `FoldReceive` consumes exactly one
  staged credit, proves replay nonmembership against the current root, and
  installs one exact successor. Wallets drain arbitrary backlogs by repeating
  this constant-shape transition. The real implementation may store
  sparse-tree nodes externally while hardware retains the root.
- **Lifecycle behavior.** Verifier rotation moves the previous active suite to
  `Retained`; it is never silently discarded. Hardware profile suspension
  stops new admission and uncommitted hardware commits, but does not gate
  post-commit recovery, receiver staging/ACK, or online application of a
  committed redemption. Captured policy epochs remain attached to prepared
  artifacts across later policy rotation.
- **Binding and structural privacy.** Candidate, terminal-certificate, and
  canonical-envelope records bind network/protocol, suite and verifier-key
  digest, asset identity/incarnation/scale, hardware profile and policy epoch,
  credential expiry, lane and hardware epoch, operation/request/ticket/credit
  IDs, and ciphertext digest. The exact public projection contains only the
  intent commitment, exact intent-bound ticket, ciphertext commitment, opaque
  terminal commit evidence, proof, and certificate fields. Raw sender,
  predecessor, deadline/time, lease, counter, lane, hardware epoch, and
  successor state are private witnesses. ACK durability is abstracted by an
  opaque credit-ID evidence set; no private ACK epoch, sequence, or time enters
  the public payment projection.
- **Self-free terminal construction.** The model fixes the canonical terminal
  body field order, constructs the body without any certificate ID, hashes that
  exact body, and only then derives the terminal certificate ID from the body
  digest. The canonical envelope embeds that resulting certificate.
- **Global value accounting.** Top-ups move online value into one pooled
  reserve; live aggregate balances, in-flight credits, and pending redemption
  vouchers exactly equal that reserve. Applying a terminal-nullifier-protected
  redemption moves value back online.

Private intent openings and predecessor records are specification ghosts used
to check commitment opening and exact-next consumption. They are not public
`PaymentV1` fields. Public candidates, commit evidence, ACK evidence, and
canonical envelopes are represented by opaque IDs.

## Checked invariants

The TLC configuration is configured to check:

- reserve, offline-liability, and total-value conservation;
- durable ACK/inbox relationships and one-use replay/nullifier state;
- unique intent-to-ticket binding, exact positive ticket/payment amounts, and
  sender-commitment opening before terminal commit;
- proof-bearing sender-authority release/profile/suite/policy binding before
  ticket issue, independently of the receiver profile;
- pre-debit mint authorization, exact recursive credit-commitment binding, and
  absence of derived IDs/ciphertext from the authorization preimage;
- self-free terminal body-to-digest-to-certificate-ID construction;
- deadline checks only at sender hardware commit;
- exact reusable request amounts, distinct one-use intent/ticket pairs, and no
  invoice-level payment-count admission ceiling;
- expiry observation never reclaiming capacity state, plus
  authenticated no-commit recovery preserving both until distinct closure;
- receiver inbox and combined sender outbox byte-reservation bounds;
- conservation-derived value-foldability of every staged exact-amount credit;
- singular receive-fold shape with no count-based admission limit;
- rejection state for a different envelope digest presented with the same
  credit and acceptance-ticket IDs;
- uniqueness of exact-next predecessors across committed operations;
- recovery of hardware-committed sends/redemptions from durable candidates;
- unconditional stageability/acknowledgeability after sender commit;
- canonical envelopes only after hardware commit;
- complete lifecycle bindings, byte-identical artifact identity across every
  post-commit recovery phase, and the exact public-transcript field boundary;
- qualified, no-fallback hardware admission; and
- exactly one active suite while older suites remain retained.

`CommittedPaymentsRemainReceivable` is a safety formulation of the key money
availability guarantee. An exact-ticket hardware-committed payment is either
recoverable from its durable candidate, immediately stageable using its
still-locked ticket, or already durably staged and acknowledged. It deliberately
does not depend on current free bytes, ticket expiry, current suite, profile
suspension, or current policy epoch. Expiry cannot send a terminal payment into
no-commit recovery.
Once staged, pooled-reserve conservation proves that the credit can enter an
ordered singleton fold without an amount overflow.

## Exploration bounds and scope

Every finite constant in `OfflineCashV1.cfg`—devices, intent/ticket/credit IDs,
amounts, byte
capacities, counters, and epochs—exists only to make TLC exploration finite.
The four configured intent/ticket/credit pairs all target the same exact
request, so the sample run explores independent valid payments against one
request and repeated singular folds. More pairs can be supplied through an
alternate configuration to explore a larger concurrent or sequential set.
Their finite count, the configured capacities, and every other TLC constant
exist only to bound state exploration; none is a protocol limit or an
authorization rule, and none may be copied into admission logic.

The growing evidence sets are specification ghosts for checking prior
transitions, not protocol history fields. The production design requires
authenticated compaction/replay accumulators with aggregate commitments and
constant-size roots, so released physical slots do not require cumulative
public history. The current in-memory implementation still retains several
exact maps; durable compaction, restart, paging, and physical-exhaustion
qualification therefore remain open. Physical storage, fixed integer widths,
and runtime remain engineering constraints rather than cumulative money-usage
rules.

This is a safety model, not a randomness proof or a proof of cryptographic
soundness, transcript unlinkability, circuit size, envelope bytes, fairness,
eventual network delivery, hardware durability, or thermal performance. TLC
treats the sampled sender commitment and cryptographic evidence as abstract
opaque values. Real circuits and qualified-hardware acceptance evidence remain
separate release gates.

Run SANY and TLC against the exact model revision before treating it as release
evidence. A prior bounded exploration was stopped after more than 90 million
generated states; that partial run is neither an invariant result nor release
evidence for this revision.

Run from this directory with a local TLA+ installation:

```sh
tlc2.TLC -config OfflineCashV1.cfg OfflineCashV1.tla
```

For release evidence, retain the TLA+/TLC version, configuration, state count,
search depth, invariant results, and source digest. Increase exploration bounds
through alternate configuration files; never reinterpret those bounds as
protocol limits.
