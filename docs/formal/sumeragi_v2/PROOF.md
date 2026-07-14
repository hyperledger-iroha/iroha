# Sumeragi v2 safety and liveness argument

This note gives the deductive protocol argument corresponding to
`SumeragiV2.tla` and `iroha_sumeragi_core::Reducer`. It deliberately separates
the mathematical result from its mechanization status: the lemmas below are
the review proof, while `SumeragiV2Proofs.tla` and `verus_proofs.rs` are the
machine-checked ledgers. A theorem is not recorded as mechanically discharged
until the relevant TLAPS or Verus command succeeds.

## Definitions and assumptions

For one frozen height context let `V` be the voting roster, `n = |V|`, `P` the
total voting power, and `w(S)` the power of signer set `S`. A dual quorum `Q`
satisfies both `3|Q| > 2n` and `3w(Q) > 2P`. The Byzantine set `B` satisfies
`3|B| < n` and `3w(B) < P`.

Every signature binds the chain, protocol, height-context ID, height, view,
phase, and subject through its canonical preimage. Correct validators obey the
durable rules enforced by the reducer and WAL:

1. they sign at most one Prepare and one Commit per `(context, view, phase)`;
2. they store and deterministically validate the exact body before persisting
   and signing Prepare;
3. they atomically persist a PrepareQC lock and Commit intent before signing
   Commit;
4. a durable timeout intent prevents later Proposal, Prepare, or Commit signing
   in that view;
5. a TC is durable before view entry, and a decision is durable before apply;
6. a lock is retained for the same subject and can change subject only through
   a strictly higher PrepareQC; and
7. crash/restart preserves every complete WAL frame and discards volatile
   completions by generation.

Cryptographic authentication, collision resistance, deterministic execution,
faithful durability acknowledgements, and the post-GST delivery and bounded
work assumptions are the trusted contracts stated in the protocol document.

## Quorum lemmas

**Lemma 1 (count intersection).** Any two count quorums intersect in a correct
validator.

Let `A` and `C` be count quorums. Inclusion-exclusion gives
`|A ∩ C| >= |A| + |C| - n > n/3`. Since `|B| < n/3`, their intersection cannot
be contained in `B`; therefore it contains a correct validator.

**Lemma 2 (power intersection).** Any two power quorums intersect in correct
voting power.

Because power is additive over the frozen roster,
`w(A ∩ C) >= w(A) + w(C) - P > P/3`. Byzantine power is strictly below `P/3`,
so the intersection contains at least one correct validator. Lemmas 1 and 2
apply simultaneously to every QC and TC signer union.

**Lemma 3 (certificate uniqueness in one round).** Two valid PrepareQCs, or two
valid CommitQCs, in the same context, height, view, and phase cannot certify
different subjects.

Their signer sets are dual quorums and therefore intersect in a correct
validator. That validator would have signed two subjects in the same durable
sign-once slot, contradicting rule 1.

## Validity, availability, and locks

**Lemma 4 (external validity and decided-body availability).** Every PrepareQC
certifies a deterministically valid body that remains available from a correct
signer; consequently every CommitQC does too.

A PrepareQC contains a dual quorum and therefore at least one correct signer.
By rule 2, each correct Prepare signer had the exact hash-matching body durably
stored and validated before releasing its signature. Correct signers retain
and serve that body. A Commit vote requires the same validated body and its
PrepareQC, so a CommitQC inherits the claim. Thus a node that records a
decision without the body can fetch the exact body from certified sources;
hash comparison prevents substitution.

**Lemma 5 (lock monotonicity).** For a fixed height, a correct validator's lock
rank never decreases, and its locked subject changes only at a strictly higher
rank.

`LockAndCommit` rejects a lower PrepareQC and rejects an equal-rank different
subject. `InstallTimeout` retains the existing lock unless the TC selects a
strictly higher PrepareQC. All such changes are complete WAL frames. Replay
applies the same checks in sequence, while a height transition creates a new
context rather than mutating the old height's lock.

## Certified timeout protection

**Lemma 6 (a TC protects every still-formable old CommitQC).** Consider a
subject `x` at view `r` whose durable Commit-intent signers form a dual quorum
`C`. Let a TC for a view `v >= r` have signer union `T`. The TC's selected
PrepareQC has rank at least `r`; if its rank is exactly `r`, it certifies `x`.

By the quorum lemmas, `C ∩ T` contains a correct validator `h`. Its Commit
intent was atomically recorded with a PrepareQC for `x` at `r`. By lock
monotonicity, the highest PrepareQC in `h`'s timeout vote has rank at least
`r`. If its rank remains `r`, certificate uniqueness makes its subject `x`.
The TC selects the maximum semantic high-QC reference across all groups, so its
rank is no lower than `h`'s. If the selected rank is `r`, it has subject `x`;
if greater, it is precisely the higher certificate allowed to supersede the
old lock. The timeout signer sets are disjoint and their union is the quorum
`T`, so grouping does not change the argument.

An old CommitQC already formed before the TC remains directly decisive. If the
old Commit-intent set was not a quorum, honest timeout fences prevent it from
growing after a TC quorum has closed that view.

## Agreement and chain prefix

**Theorem 1 (agreement).** Two valid CommitQCs in one height context cannot
certify different subjects.

Assume for contradiction that `CommitQC(x, r)` and `CommitQC(y, s)` exist with
`x != y`; choose such a pair with the smallest later view `s`, and take
`r <= s`. Lemma 3 rules out `r = s`. The earlier CommitQC gives a dual quorum
of durable locks on `x` at rank `r`. Every TC used to reach a later view
protects that potential commit by Lemma 6.

For a correct validator from the intersection of the earlier Commit quorum and
the Prepare quorum underlying `CommitQC(y, s)` to Prepare `y`, its proposal
must carry a PrepareQC for `y` strictly above its `x` lock. Select the first
such conflicting PrepareQC. Its own correct intersection validator can change
from `x` only using an even earlier strictly higher conflicting PrepareQC.
That is a smaller conflicting view above `r`, contradicting minimality.
Therefore no conflicting PrepareQC, and hence no conflicting CommitQC, exists.

**Corollary 1 (decision agreement).** Persisting a CommitQC cannot create two
different decisions at one height. The WAL additionally rejects a second
decision whose semantic certificate reference has another subject.

**Corollary 2 (chain-prefix safety).** Finalized chains of correct nodes are
prefix comparable.

The canonical slot `h + 1` is certified only from the exact durable decision
carrying a valid CommitQC for the canonical height-`h` context. Its identity
binds the chain, epoch, roster, lane/DA inputs, and semantic parent. Different
valid CommitQC views, aggregate signatures, or signer subsets for that same
semantic parent intentionally produce the same next context; a different
parent produces a different context ID. Each correct validator advances only
after its own exact application receipt. It may lag, but another validator's
receipt cannot advance it. Theorem 1 plus induction over these receipt-backed
slots therefore makes all correct local histories prefix comparable without a
global all-node application barrier.

## Crash/restart and reconfiguration

**Theorem 2 (crash/restart safety).** A crash at any effect boundary cannot
cause a correct validator to equivocate, lower a lock, enter an uncertified
view, or apply an undecided block.

Before a safety-relevant signature or state transition is exposed, its WAL
frame is flushed and synchronised. A crash before acknowledgement leaves no
authorized continuation; an incomplete final frame is discarded. A crash
after acknowledgement replays the intent, lock, TC, or decision before ingress
opens. Earlier corruption, a broken hash chain, or a chain/protocol/key mismatch
fails closed. Generation tags reject completions from the crashed incarnation.
The replay transition checks the same uniqueness and monotonicity predicates as
the live transition.

**Theorem 3 (epoch-boundary safety).** Certificates from one epoch cannot vote
in another, and a roster/power change cannot invalidate an earlier decision.

Every vote and certificate binds a height-context ID. The context contains the
finalized epoch roster and powers, so verification always uses that immutable
snapshot. A validator constructs its new epoch context only after its local
receipt proves the exact preceding decision and body durable and applied. Old
signatures therefore fail the new context-ID check, while Theorem 1 remains
valid under the old snapshot. Validators may cross the boundary at different
times and still remain on one certified prefix.

## Conditional liveness after GST

Assume after GST that the responsive correct voters independently meet both
strict count and power thresholds; per-source authenticated transport is
serviced within its declared bound; the monotonic clock and serialized run loop
continue; fsync, signature, body transfer, deterministic validation, proposal
construction, and application terminate within finite declared bounds; and the
complete successful-round rank is representable below the maximum runtime
duration. The immutable view-zero timeout is positive and view `v` receives
`base * (v + 1)` (saturating only at that representation limit), so the model
derives rather than assumes a later view whose timeout exceeds the rank.

**Lemma 7 (bounded scheduler and transport service).** A Byzantine flood cannot
starve an authenticated responsive source or an admitted progress/completion
command.

Each recipient/source lane is bounded and the transport cursor visits every
lane in roster order. Its exact remaining rank is the number of source slots
left in the current recipient plus one full source roster for each remaining
recipient; alternating ingress and transport contributes a factor of two. The
runtime queue reserves separate normal, progress, and completion capacity.
The current view's absolute timeout has first priority. A periodic
retransmission may precede an already-admitted FIFO command once, after which
FIFO debt gives that command the next non-timeout slot. Thus the service rank
strictly decreases unless that view's timeout makes the work stale, in which
case the certified next view restarts it under a fresh tag and a strictly larger
deadline.

**Lemma 8 (view progress).** If a height does not decide in view `v`, correct
validators eventually form and install a TC for `v` and enter `v + 1`.

Each responsive correct validator's absolute timer eventually expires and its
durable timeout vote is broadcast to every voter. Responsive correct validators
alone satisfy both quorum thresholds, so their votes form a TC without a
distinguished collector. Persistence precedes `EnterView`, and retransmission
eventually delivers that TC to every responsive validator.

**Lemma 9 (lock convergence).** After GST, a retained lock omitted from one TC
cannot block every later successful round.

The locked validator's timeout vote carries the full PrepareQC while signing
only its semantic identity. Delivery teaches that verified QC to the responsive
quorum. Their later timeout votes report a certificate at least that high, so a
subsequent TC selects it. Certificate uniqueness orders same-view evidence, and
strictly higher certificates safely dominate older locks. Thus responsive
validators converge on one safe proposal subject.

**Theorem 4 (liveness).** Consensus eventually decides and applies the next
block after GST.

By Lemmas 7 and 8, non-deciding views continue to advance. Because the deadline
grows without wraparound throughout the representable proof domain, some view
exceeds the complete finite post-GST service rank. Every later view is at least
as long, and deterministic roster rotation selects a responsive correct leader
within one complete rotation.
If an omitted lock prevents that first candidate round, Lemma 9 makes it known
during the round's timeout, after which the next responsive correct leader
proposes the selected safe subject. The responsive quorum obtains, stores, and
validates the exact body before Prepare; forms and disseminates PrepareQC;
atomically locks and persists Commit intents; and forms CommitQC within the
timeout. Each node persists the decision, fetches the certified manifest and
chunks if needed, reconstructs and validates the exact body, applies it, and
advances its own height. None of these local transitions waits for every other
correct node.

For the four-validator Taira regression, the outage begins in a freshly opened
view zero. Its unavailable leader consumes the 10-second base deadline; the
next leader is responsive because the other three validators constitute the
entire live quorum, and view one receives 20 seconds. The 50-second test bound
therefore includes the 30-second protocol envelope plus startup, polling, and
host-scheduling margin. The first timed-out round also disseminates any
previously omitted full PrepareQC, so lock convergence does not add another
complete rotation.

This is necessarily a conditional temporal result: FLP rules out unconditional
termination without post-GST bounds. Without bounded delivery and service, a
responsive dual quorum, terminating validation/fsync/application, and a
representable service bound that some view-indexed timeout can exceed,
deterministic consensus cannot guarantee progress. The result proves height
progress, including a valid empty heartbeat; it does not prove transaction
inclusion or censorship fairness.

## Mechanization ledger

`proof_coverage.json` is the only checked-in status authority. The release
runner checks the ordered deductive modules with pinned TLAPM commit `763bf3c`,
requires a positive all-obligations-proved result from each backend run, and
writes source/log/tool-bound evidence under `target/formal/sumeragi_v2/`.
Checked-in counts are intentionally prohibited because they become stale as
proofs change.

The module set covers quorum algebra, availability, crash recovery,
reconfiguration, compositional safety, agreement, full action induction,
receipt-backed chain/epoch refinement, stable-suffix liveness, and the explicit
asynchronous scheduler/transport proof. No top-level assumption, axiom,
omitted proof, or favourable-network protocol relation can satisfy the release
checker. The conditional liveness premises remain visible as trusted contracts
rather than being restated as the theorem to prove.

TLC runs exhaustive constant checks and bounded asynchronous counterexample
searches. It cannot upgrade a proof status. The production trace replayer and
adversarial simulations exercise the exact reducer sources, while the pinned
Verus harness proves the source-linked reducer/WAL and scheduler kernels. The
remaining cryptographic, deterministic-execution, operating-system durability,
post-GST transport, and host-service premises are listed explicitly in the
ledger and formal README.
