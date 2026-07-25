# Sumeragi v2 safety and liveness argument

This note gives the deductive protocol argument corresponding to
`SumeragiV2.tla` and `iroha_sumeragi_core::Reducer`. It deliberately separates
the paper argument from its mechanization status: the lemmas below explain the
conditional target, while `SumeragiV2Proofs.tla`, `verus_proofs.rs`, and
`proof_coverage.json` record what is actually machine checked. A theorem is not
recorded as mechanically discharged until the relevant TLAPS or Verus command
succeeds.

## Definitions and assumptions

For one frozen height context let `V` be the voting roster, `n = |V|`, `P` the
total voting power, and `w(S)` the power of signer set `S`. A dual quorum `Q`
satisfies both `3|Q| > 2n` and `3w(Q) > 2P`. The Byzantine set `B` satisfies
`3|B| < n` and `3w(B) < P`.

Every Vote and QC signature binds the chain, protocol, height-context ID,
certification round, immutable `proposal_round`, phase, subject, and execution
commitment through its canonical preimage. Prepare evidence requires
`proposal_round == round`. Commit evidence requires the same context and height
and `proposal_round.view <= round.view`. The reducer lifecycle owner
`(height, view, generation)` is separate local authorization and is not signed
as either wire round. Correct validators obey the durable rules enforced by the
reducer and WAL:

1. they sign at most one Prepare and one Commit per `(context, view, phase)`;
2. they store and deterministically validate the exact body before persisting
   and signing Prepare;
3. they atomically persist a PrepareQC lock and Commit intent before signing
   Commit; the lock retains the PrepareQC origin while the Commit intent may
   use a later finality round;
4. a durable timeout intent prevents later Proposal or Prepare signing and
   ordinary creation of a new Commit intent in that view; after a TC durably
   promotes its selected PrepareQC to the exact active lock, local durable body
   validation may persist one later-finality Commit for that historical origin only if no higher
   proposal-origin local Prepare intent or known PrepareQC exists, including one
   for the same subject bytes; an
   already-durable Commit may be re-signed or retransmitted only while its
   `proposal_round` and subject remain the exact active lock; among durable
   Commit intents for that origin, only the greatest finality round is active;
5. a TC is durable before view entry, and a decision is durable before apply;
   a second TC for the immediately preceding timed-out round may be installed
   without another view advance only when its selected Prepare origin strictly
   exceeds both the installed high and lock;
6. a lock is retained for the same subject and can change subject only through
   a strictly higher PrepareQC; and
7. crash/restart preserves every complete WAL frame and discards volatile
   completions by generation.

Cryptographic authentication, collision resistance, deterministic execution,
faithful durability acknowledgements, and the post-GST delivery and bounded
work assumptions are the trusted contracts stated in the protocol document.
The local-work contract is quantified only over validator hosts admitted by
the first-release storage-platform gate. An unsupported-platform failure is
not successful termination and cannot discharge application or
successor-activation fairness. Revision 3 has no legacy decoder premise: a
Vote, QC, status record, or finality artifact without its canonical signed
proposal origin is rejected.

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
stored and validated at the Prepare `proposal_round` before releasing its
signature. Correct signers retain and serve that body. A Commit vote requires
the same locked proposal origin, validated body, and PrepareQC, so a CommitQC
inherits the claim even when its certification round is later. Thus a node
that records a decision without the body can fetch the exact proposal origin
from certified sources; hash and origin comparison prevent substitution.

**Lemma 5 (lock monotonicity).** For a fixed height, a correct validator's lock
rank never decreases, and its locked subject changes only at a strictly higher
rank.

`LockAndCommit` rejects a lower PrepareQC and rejects an equal-rank different
subject. `InstallTimeout` retains the existing lock unless the TC selects a
strictly higher PrepareQC. All such changes are complete WAL frames. Replay
applies the same checks in sequence, while a height transition creates a new
context rather than mutating the old height's lock.

## Certified timeout protection

**Lemma 6 (a formed TC either directly protects or intersects durable
installed-TC authorization for every still-formable old CommitQC).** Consider
a subject `x` with immutable proposal origin `p` whose durable Commit-intent
signers form a dual quorum `C` at finality round `r >= p`. Let a TC for a view
`v >= r` have signer union `T`.

By the quorum lemmas, `C ∩ T` contains a correct validator `h`. If `h`'s
timeout vote strictly protects its Commit intent, the TC's maximum semantic
high-QC selection lifts that local fact to direct protection of `(p, x)`: its
selected rank is at least `p`, and equality binds subject `x`. If the timeout
does not strictly protect the Commit, the Commit can only be the narrow
late-created historical-lock exception. The durable provenance invariant then
supplies an installed TC whose selected PrepareQC exactly authorizes `h`'s
Commit proposal origin and subject. The authorizing installed TC is not required to be
later than the formed TC being examined. The timeout signer sets are disjoint
and their union is the quorum `T`, so grouping does not change the
intersection argument.

An old CommitQC already formed before the TC remains directly decisive.
Ordinary Commit-intent growth remains timeout-fenced. The sole late-growth
case is the TC's exact selected PrepareQC: installation durably records that
provenance and promotes the matching lock, body validation terminates, and the
node has no higher proposal-origin Prepare intent or known PrepareQC, including
one for the same subject bytes. The
node signs that origin directly in its active finality round; it does not
re-propose the same bytes. The grouped-timeout set kernel is
machine-checked for intents already present at timeout. The earlier temporal
induction for installed-TC authorization and its dependent wrapper was proved
for the superseded transition relation. The stronger higher-origin fence and
strict same-round TC lock-upgrade action have not received a fresh strict run,
so the current ledger records this obligation as `specified_unproved`.

A local proposal after TC installation is now explicitly tied to that TC's
selected high subject. `LocalProposalJustification` projects the selected rank
and subject, `LocalProposalReproposesJustifiedHigh` rejects a different subject
whenever the rank is nonempty, and `BeginLocalProposal` enforces the predicate
after wire validation. The focused inductive theorem proves this exact action
postcondition. Production realizes the same boundary by promoting the TC high
PrepareQC into the durable WAL lock, exporting that exact lock to the runner,
loading and checking the canonical body subject, and refusing fresh candidate
assembly while a lock exists. This is a safe-value/source-refinement fact; it
does not by itself establish the eventual locked-body reproposal theorem.

## Agreement and chain prefix

**Theorem 1 (agreement).** Two valid CommitQCs in one height context cannot
certify different subjects.

Assume for contradiction that `CommitQC(x, r, p)` and `CommitQC(y, s, q)` exist
with `x != y`, where `r` and `s` are finality rounds and `p <= r`, `q <= s` are
their proposal origins. Order the pair by proposal rank and choose a
counterexample with the smallest later origin `q`, taking `p <= q`. If
`p = q`, the underlying PrepareQCs conflict in one proposal round, contradicting
Lemma 3. The `x` CommitQC gives a dual quorum of durable locks on `x` at rank
`p`. Lemma 6 does not falsely infer that every older TC's own high protects a
Commit created later. It instead preserves the two proof routes required by
the durable lineage invariant: direct high-QC protection, or an honest
intersection signer whose later-finality Commit remains authorized by an
installed TC selecting the same proposal-origin lock.

For a correct validator from the intersection of the earlier Commit quorum and
the Prepare quorum at origin `q` underlying `CommitQC(y, s, q)` to Prepare `y`,
its justification must carry a PrepareQC for `y` strictly above its `x` lock.
Select the first
such conflicting PrepareQC. Its own correct intersection validator can change
from `x` only using an even earlier strictly higher conflicting PrepareQC.
That is a smaller conflicting proposal view above `p`, contradicting minimality.
Therefore no conflicting PrepareQC, and hence no conflicting CommitQC, exists.

**Corollary 1 (decision agreement).** Persisting a CommitQC cannot create two
different decisions at one height. The WAL additionally rejects a second
decision whose semantic certificate reference has another subject.

**Corollary 2 (chain-prefix safety).** Finalized chains of correct nodes are
prefix comparable.

The canonical slot `h + 1` is certified only from the exact durable decision
carrying a valid CommitQC for the canonical height-`h` context. Its identity
binds the chain, epoch, roster, lane/DA inputs, semantic parent, and immutable
proposal origin. Different valid CommitQC finality views, aggregate signatures,
or signer subsets for that same proposal-origin decision intentionally produce
the same next context; another proposal origin or parent produces a different
context ID. Each correct validator advances only
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
the live transition. Prepare replay remains current-view and timeout-fenced.
Commit replay may cross that timeout or a later installed TC only for the
newest durable vote whose `proposal_round` and subject equal the active lock;
its finality round may be later. It cannot authorize an unlocked historical
Commit intent.

The executable refinement gate does not authorize this by comparing only
effect-shaped values. Its pending projection binds the WAL record's primary
proposal origin, and records with embedded Prepare evidence also bind that
certificate's auxiliary proposal origin. Begin and acknowledgement boundaries,
the requested persistence capability, and the independently reconstructed
grant all match those fields to the pending record. A correlated mutation of
requested and granted origins therefore still violates refinement.

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
serviced within its declared bound; and every non-crashing responsive
validator participating in the active-height or exact historical-recovery
corridor has an advancing local monotonic clock, regains a serialized height-
runner turn within the declared bound after every finite wait, and completes
admitted fsync, signature, body-transfer, deterministic-validation, proposal-
construction, and application work within finite declared bounds. Also assume
that the complete successful-round rank is representable below the maximum
runtime duration. The immutable view-zero timeout is positive and view `v` receives
`base * (v + 1)` (saturating only at that representation limit), so the model
derives rather than assumes a later view whose timeout exceeds the rank.

The claim is conditional and per height: after GST, with a responsive dual
quorum and terminating local work, every height eventually decides and every
responsive validator eventually applies it. No claim covers an unbounded
partition, loss of either quorum threshold, or local work that never returns.
The model's per-node service-deadline vector is ghost bookkeeping for this
trusted runtime contract, not shared production state. Production source seals
establish the narrower structural facts—one serialized height loop, bounded
service batches, watchdog polling, and finite `IDLE_POLL` waits—but do not turn
host scheduling or operation latency into a code-proved proposition.

**Lemma 7 (generation- and consumer-scoped locked-vote delivery).** Clearing a
volatile vote pool cannot orphan the exact durable locked Commit intent.

Authenticated vote history is immutable and distinct from the volatile receipt
pool. An exact vote already present in that pool is suppressed. Persisting a TC
clears only the installing node's pool and advances its reducer generation
(until the finite model's generation bound). At that acknowledgement boundary,
the reducer compares the resulting lock with its durable Commit intents. An
exact proposal-origin/subject match selects the greatest durable Commit
finality round and queues only that intent for re-signing. A newly TC-promoted
lock without an intent never signs merely from installation. Its exact body
recovery and current-generation validation schedule `BeginLockCommit` for the
active finality round while retaining the old proposal origin; only the
matching WAL acknowledgement may then release the signature, and only when no
higher proposal-origin local Prepare intent or known PrepareQC exists, including
one for the same subject bytes.
Completing the signature inserts the vote directly into the installing node's
new finality pool, then broadcasts only to the other voters because the P2P
broadcast does not deliver to its sender.

Every Commit vote, including one from the current view, is admissible only when
its `proposal_round` and subject equal the active durable Prepare lock. A
premature current-view Commit is a recoverable ignore. When the matching
`LockAndCommit` acknowledgement makes a later-finality same-origin intent
durable, the reducer prunes every older finality pool before releasing the
local Commit signature; pruning earlier would orphan the old reconstruction
source if persistence failed. The adapter advances the locked-Commit consumer
epoch at that boundary, so the ignored exact vote may enter once after
authority exists. Thus the bounded live set is current Prepare plus only the
latest durable Commit finality pool for the locked origin.

Retained signed Commit control remains the peer-delivery source, but cannot by
itself witness local reconstruction because broadcast excludes its sender. A
retransmitted Commit whose proposal origin matches the exact lock is classified as
protected progress and consumes each peer's new pool epoch exactly once.
Prepare votes, unrelated proposal origins, and superseded Commit finality votes
remain inadmissible. If a crash
happened after the Commit intent became durable but before its signature or
broadcast, `ResumeVote` reconstructs that same exact locked Commit even after
the timeout or TC. The distinct validation path above may create the missing
later-finality intent, but replay never invents one without its preceding
installed-TC lock and acknowledged `LockAndCommit` frame. Equal body bytes at a
later proposal origin are rejected; progress commits the installed origin
directly rather than re-proposing it.

The source-shared `locked_commit_progress_witness_body!` kernel covers the
validation/timeout serialization race explicitly. A validated undecided lock
has one of three exact owners: an active durable Commit with its signature or
local-pool owner, the pending matching `LockAndCommit`, or an acknowledged
timeout for the current view when the lock origin is historical. The timeout
branch is recovery-only and cannot authorize a post-timeout Commit. Its Rust
mutation regression rejects stale, wrong-signer, volatile-only, and non-exact
projections; `locked_commit_progress_witness_is_valid` mirrors the same
expression in Verus.

**Lemma 8 (bounded scheduler and transport service).** A Byzantine flood cannot
starve an authenticated responsive source or an admitted progress/completion
command.

Each recipient/source lane is bounded and the ready queue rotates every
non-empty source. Within the selected source, ingress removes the oldest
currently admissible entry; an auxiliary request waiting for I/O capacity
therefore cannot hide later consensus or certified-body progress, and every
earlier blocked entry remains in its original order. In the abstract scheduler,
exact authenticated envelopes coalesce while an equal occurrence remains owned
anywhere in the scheduler, including deferred, causal, runtime,
ready-completion, I/O, or outstanding-work ownership. Servicing that owner
therefore cannot leave an equal replacement at the same logical rank. Production
now uses one generic queued/Busy-deferred ownership bridge for every
authenticated consensus envelope: it compares the complete canonical encoding
with the retained `authenticated_wire_identity`, then rechecks that identity
after authentication. The CommitQC-named wrappers are test-only regression
conveniences, not a reducer-event-only production exception or deductive
evidence. The wider causal/ready/I/O projection remains part of the separately
unproved production-refinement seam. A pinned mutation demonstrates the old
replacement lasso and the scheduler-wide coalescing repair.
Transport packets and ingress entries retain occurrence-specific service
arguments instead of being folded vacuously into that candidate rank. Timeout
delivery also establishes membership in the full canonical envelope record set
before consuming authorization fields, excluding augmented records that merely
expose a well-typed selected view.

The production capacity premise is instantiated with checked exact transport
geometry, not an abstract payload-size allowance. If `F(x)` is canonical
compact-length framing, a frozen layout with `C` chunk hashes has manifest
ceiling `F(8 + C * F(32)) + 228`. The maximal proposal contains that manifest,
one full-QC timeout group per validator, the separately carried highest QC, and
maximum signatures; the recommended 128-validator value is 232,541 bare bytes.
The maximum recommended `PayloadChunk`/`CertifiedBodyResponse` envelope is
16,811,581 bare bytes. Recovery request and
`CommitCertificateResponse` ceilings include maximum QCs, maximum signatures,
the actual chain-id length where present, and an embedded `PeerId` derived from
the protocol-wide 8,258-byte raw public-key payload ceiling. Thus the premise
covers non-roster observers and rotated responders rather than assuming the
active roster's key width.

All live lane-local messages now cross the same fair ingress as V2 messages.
Lane executable payloads and handoffs consume TransportCompletion ownership;
lane votes, proposals, QCs, certificates, and new-view traffic consume Progress
ownership. Production enforces four-MiB and one-MiB exact wire ceilings for
those two groups. The byte-abstract model uses its existing completion and
progress representatives, so the exact concrete class/byte correspondence is
part of `ProgressWitnessProductionRefinementObligation`, not a proved premise.
The concrete resource lane is keyed by the authenticated `via` hop, while
semantic origin remains attached for validation, response routing, and
coalescing. `SumeragiV2ReplyRouteOwnership` now gives each canonical semantic
request a bounded set of authenticated-source attempts. Its actor-global
delivery ordinal is distinct from connection tenure. Exact and later
same-tenure deliveries preserve per-source message/chunk cursors. A reconnect
preserves the source's current cursor while replacing its writer tenure, and a
newly observed alternate source starts at zero.
Tickets bind semantic identity and tenure but not delivery ordinal, and the
round-robin cursor is source isolated. The cursor-reset and alternate-source-
replacement mutations exercise these same transition operators. A
reconnect advances source-scoped tenure once; subsequent deliveries rebind
other retained semantic attempts independently, preserving each newly bound
attempt's cursor while clearing its stale ticket. The liveness formula assumes
eventual stable-responsive service eligibility and requires strict cursor advance or
completion, so route retirement or ticket loss cannot discharge it. This
abstract ownership machine still does not prove the production origin/`via`
projection or runner/worker/sidecar call-path mapping.

That bare envelope is lifted through the exact `BlockMessage::V2`, framed
`BlockMessageWire`, `NetworkMessage::SumeragiBlock`, direct P2P relay, and
header-framed `Message::Data` layouts. The final inequalities require the
plaintext frame to fit its topic cap and the global encrypted cap after the
28-byte AEAD expansion; one queued frame additionally owns its four-byte length
prefix. The wire body may be at most `u32::MAX`, while the deterministic
runtime/configuration ceiling is 2,147,483,643 bytes so prefix plus body fits a
contiguous `i32::MAX`-byte buffer on 32-bit and 64-bit hosts. Daemon validation
rejects a larger cap before binding, and the sender uses checked geometry and
checked conversion before encryption. Both context configuration and ingress
open repeat the count, disjoint source-byte, aggregate-byte, three topic-frame,
and outbound-high checks. Prefix-inclusive outbound charging returns a checked
optional value, so arithmetic failure rejects activation unconditionally rather
than relying on a `usize::MAX` sentinel comparison. These executable checks
establish the finite-capacity premise used by this lemma; they do not prove
post-GST delivery fairness or promote any proof-ledger entry.

The model publishes one abstract packet atomically. Production instead retains
a reliable occurrence through actor admission, encoding, frame and batch
ownership, socket write, and flush; broadcasts also retain their remaining
target cursor. Retirement is authorized only by the matching flush
acknowledgement. That acknowledgement proves one local transport attempt, not
relay-final-target receipt, subscriber consumption, or application. The exact
actor-to-flush trace, later custody transitions, and decreasing service rank
remain unassigned production-refinement propositions, so the abstract packet
action cannot by itself discharge starvation freedom. The Sumeragi-side queue
is bounded by the frozen height roster crossed with all three reliable output
classes plus separate shared capacity. A deterministic matching gives each
retained fanout at most one unique frozen reservation and is recomputed after
partial progress. Non-roster replies and repeated target/class output
therefore cannot spend unopened validator reservations. Concretely, a reliable
broadcast snapshots the actor-accepted relay-aware topology and acquires each
target's ordinary `(target, class)` lane independently. An existing target
child coalesces only a retry with the identical canonical request digest and
the same membership tenure. A distinct payload or direct/broadcast cross-kind
collision retains an exact per-target FIFO ticket with the caller, without a
class-wide parent. Topology removal cancels only the old broadcast tenure, and
remove/re-add creates a new generation; direct-post ownership is not cancelled.
This removes the known local parent-residual obstruction, but no production
refinement or theorem currently establishes remote receipt, downstream
consumption, or broadcast starvation freedom.

Local admission alternates a producer-completion source with the causal-work
source. A producer selection while causal work waits records sticky causal
debt and advances the source cursor; only causal selection clears that debt.
Once the causal head is admissible, the debt makes it the deterministic
preference under fair `RunNode` service, so repeated producer replenishment
cannot reset the causal owner's position.

An individual signed Vote cannot establish its own execution-commitment
authority. It is serviceable only after a local validated receipt, verified WAL
replay, or quorum-authenticated QC binds the exact proposal origin, finality
round, subject, and execution commitment; an unbound Vote is rejected
recoverably until that binding arrives. Body-availability
rebind first requires the installed destination tag and preflights both source
and destination owners. An uninstalled destination is a non-mutating recoverable
caller error, one exact destination coalesces, and conflicting or duplicate
ownership fails closed before mutation. Pipeline and Decision retirement use
the same transactional preflight. Conflicting certificate request/response
transport remains a nonfatal remote-input rejection with retry ownership
preserved.

The runtime queue reserves separate normal, progress, and completion capacity.
The protected candidate vocabulary also includes the canonical
constructor-shaped Normal proposal/Prepare slice: leader `AssembleBody`, the
causal `BeginPrepare` successor, and the frozen Normal delivery shape for
Proposal, PrepareVote, and CommitVote items. Reachable delivery ownership must
originate at authenticated ingress, but protection follows the stored class so
view movement cannot drop an admitted CommitVote when its dynamic classification
becomes historical Progress. Authenticated `TimeoutVote`/`DeliverTimeout` owns
a signer-keyed protected Progress slot. Certified-body and Commit-certificate
recovery requests receive fresh live Serve nonces and occurrence-level FIFO
ranks, so equal request values cannot collapse one another's starvation
witness. These constructor families deliberately over-approximate reachable
provenance; authenticated junk receives no temporal promise, and the composite
rank obligation remains explicit proof debt. A compact Stage-6 mutation pins
the causal arithmetic: doubling the unique FIFO index makes removal of an
earlier head strictly dominate a simultaneous zero-to-one local-source cursor
reset, while multiplier one yields the exact equality counterexample. This is
bounded regression evidence only; it neither opens blocked Completion causal
capacity nor proves the temporal rank obligation. At the production projection
boundary, the source-bound reverse/push-front kernel now has Verus proofs for
continuation-before-tail order, stable first ownership, prior-owner exclusion,
retention of every emitted fresh identity, unique fresh values, and conditional
old-prefix/fresh-tail preservation. Those
sequence theorems assume a faithful identity projection and a complete owner
set; they do not prove the executable effect-to-TLA candidate mapping or the
Completion-capacity product rank, so the ledger entry remains
`specified_unproved`.
The current view's absolute timeout has first priority. A periodic
retransmission may precede already-admitted command work once, after which
command debt gives the class-aware ingress the next non-timeout slot. Thus the
service rank
strictly decreases unless that view's timeout makes the work stale, in which
case the certified next view restarts it under a fresh tag and a strictly larger
deadline.

**Lemma 9 (view progress).** If a height does not decide in view `v`, correct
validators eventually form and install a TC for `v` and enter `v + 1`.

Each responsive correct validator's absolute timer eventually expires and its
durable timeout vote is broadcast to every voter. Responsive correct validators
alone satisfy both quorum thresholds, so their votes form a TC without a
distinguished collector. Persistence precedes `EnterView`, and retransmission
eventually delivers that TC to every responsive validator.

**Lemma 10 (lock convergence).** After GST, a retained lock omitted from one TC
cannot block every later successful round.

The locked validator's timeout vote carries the full PrepareQC while signing
only its semantic identity. Delivery teaches that verified QC to the responsive
quorum. Their later timeout votes report a certificate at least that high, so a
subsequent TC selects it. Certificate uniqueness orders same-view evidence, and
strictly higher certificates safely dominate older locks. Thus responsive
validators converge on one safe proposal subject.

**Theorem 4 (liveness).** Consensus eventually decides and applies the next
block after GST.

By Lemmas 8 and 9, non-deciding views continue to advance. Because the deadline
grows without wraparound throughout the representable proof domain, some view
exceeds the complete finite post-GST service rank. Every later view is at least
as long. The formal rotating-leader property first requires either an early
decision or a view in which the responsive correct scheduled leader itself is
active, then requires that leader state to lead to all responsive decisions.
Deterministic roster rotation supplies such a leader within one complete
rotation.
If an omitted lock prevents that first candidate round, Lemma 10 makes it known
during the round's timeout. A selected PrepareQC installs its immutable
proposal origin on the responsive quorum, which signs Commit for that lock in
the active finality round without creating another proposal. A Timeout-justified
Proposal carrying any selected PrepareQC is structurally invalid; the TC instead
travels through the ordinary durable installation path above. Only if the TC has
no selected PrepareQC may the next responsive correct leader propose while
unlocked. The responsive quorum obtains, stores, and validates the exact body
before Prepare; forms and disseminates PrepareQC; atomically locks and persists
Commit intents; and forms CommitQC within the timeout. Each node persists the
decision, fetches the certified manifest and
chunks if needed, reconstructs and validates the exact body, applies it, and
advances its own height. None of these local transitions waits for every other
correct node.

The mechanization records the missing step under the legacy symbol
`LockedBodyReproposalProgressObligation`. Its first-release obligation is
locked-origin direct-commit progress: a stable available retained lock must
eventually be committed with its original `proposal_round`, be decided, or be
legitimately superseded by a higher certified Prepare lock. Equal bytes cannot
be re-proposed at a later origin. The symbol must be restated and rerun before
formal promotion; it remains `specified_unproved`, and view movement or byte
retention alone is not an outcome.

The application property is quantified per responsive validator: after GST,
one validator's durable decision leads to that validator's application even
if another validator has not decided. A separate aggregate clause composes the
per-node pipeline with successor-height activation; it is not the sole
application guarantee.

For the four-validator Taira regression, the outage begins in a freshly opened
view zero. Its unavailable leader consumes the 10-second base deadline; the
next leader is responsive because the other three validators constitute the
entire live quorum, and view one receives 20 seconds. The 50-second test bound
therefore includes the 30-second protocol envelope plus startup, polling, and
host-scheduling margin. The first timed-out round also disseminates any
previously omitted full PrepareQC, so lock convergence does not add another
complete rotation.

This is necessarily a conditional temporal target: FLP rules out unconditional
termination without post-GST bounds. Without bounded delivery and service, a
responsive dual quorum, terminating validation/fsync/application, and a
representable service bound that some view-indexed timeout can exceed,
deterministic consensus cannot guarantee progress. The paper argument derives
height progress under those premises, including a valid empty heartbeat; it
does not prove transaction inclusion or censorship fairness. It is not a
machine-checked liveness completion while the ledger reports
`machine_checked_completion: false` and retains downstream asynchronous and
multi-height liveness obligations as `specified_unproved`.

## Mechanization ledger

`proof_coverage.json` is the checked-in status declaration, not independent
proof authority. The checker binds it to exact theorem declarations and
structural dependencies. The release runner checks the ordered deductive
modules with pinned TLAPM commit `3ab43c7`, requires a positive
all-obligations-proved result from each backend run, and writes
source/log/tool-bound evidence under `target/formal/sumeragi_v2/`. Checked-in
backend counts are intentionally prohibited because they become stale as
proofs change.

The module set covers quorum algebra, availability, crash recovery,
reconfiguration, compositional safety, agreement, full action induction,
receipt-backed selected-height and indexed chain/epoch refinement, and the
explicit asynchronous scheduler/transport model. Ten arbitrary-context
Core safety wrappers and the receipt-backed chain-prefix and epoch-boundary
wrappers are TLAPS-proved. This includes the historical TC-lock authorization,
the dependent direct-or-installed-authorization timeout wrapper, and the strict
grouped-timeout kernel. The
one-height asynchronous type-closure wrapper and generation-scoped delivery
theorem are now ledgered `tlaps_proved`; the concrete runner scheduler-
preservation prerequisite is proved as well. Fresh hash-guarded strict TLAPS
slices exited 0 for transport/runner closure (186/186 and 204/204), the recovery
execution hierarchy (305/305), its strong caller and bracket (63/63), the exact
type obligation (16/16), and the named always-strong wrapper (10/10). The timeout durability, signing,
view-frontier, and wire-authorization modules additionally prove that every
honest pending, durable, signing, and transported timeout vote is roster-,
context-, height-, and authenticated-high-reference bound with a high rank no
greater than its timeout view. The production Rust/Verus gate now checks the
local EnterView effective-lock selection boundary plus a typed adapter safety
projection: monotonic Fetch/BodyAvailable/Store/Validate identity, strictly
advancing same-body consumer rebind, unique cross-lane completion ownership,
exact supersession byte residuals, and the actual bounded three-class runtime
selector. The corresponding solver queries are isolated in
`effective_lock_verus_proofs.rs`, but instantiate the same production macros;
they are not a separately transcribed protocol. This is not a cross-tool proof
that the independently parsed TLA+
body matches those Rust expressions, nor a temporal theorem that the runtime or
external body service is eventually invoked. The executable height-scoped
acquisition owner now specifies immutable physical identity, mutable consumer
rebind, exact completion classification, certified recovery, and retry. Its
`EffectiveLockAcquisitionModelObligation` composes type closure, acquisition
progress, and stable repeated delivery. A complete pinned strict TLAPS run
proved all 1,258 module obligations, so the ledger records this abstract model
obligation as `tlaps_proved`; exhaustive bounded TLC remains complementary
regression evidence rather than the deductive discharge.
Ordinary-Rust map/hash/service projection, worker/request ownership, and
post-GST fairness remain separately in
`EffectiveLockBodyAcquisitionProductionRefinementObligation`, also ledgered
`specified_unproved`. The exact enabled-`RunNode` result is only a scheduler
lemma: post-GST deadlock freedom requires an enabled `AsyncNext` step that grows
current-height protocol evidence, strictly consumes a concrete deadline debt,
or decreases/exits a protected candidate or Serve-occurrence rank. Repeated
clock or view-change steps alone do not satisfy that productive obligation.
Stage-2/3/6, packet-admission, and zero-deadline cases therefore remain explicit
proof debt. The executable async model now retains an exact deferred handoff
across a Busy retry: matching work clears only its own token, Completion work
may still terminate the current Busy owner, and foreign non-Completion work
cannot create a successor owner. The Stage-2 scratch isolates the remaining
timer/retransmit rearm, finite cursor, and exact-service leaves; the Stage-3
scratch reduces strict FIFO closure to the same-node `PostGstRunNode` induction
and the all-other-action UNLESS induction. Neither scratch has fresh strict
TLAPS evidence, so no ledger status changes. Runner preservation and the dependent async type closure are now
proved. Starvation has conditional precursor proof bodies, while
the release-facing theorem remains proofless and depends on the still-unproved
service-rank theorem. It remains `specified_unproved`. Durable witness,
rank decrease, and the remaining stable-suffix liveness theorems are exact
universally quantified declarations recorded as `specified_unproved`. The
argument above does not upgrade any of those statuses. The concrete genesis
chain product's first-successor handoff also has a source proof body but remains
ledgered `specified_unproved`. The application debt is now isolated at
`ApplicationCompletionProgressObligation`, the proofless per-responsive-node
decision-to-application pipeline. The aggregate `ApplicationLivenessObligation`
is derived from that premise by application monotonicity, a frozen responsive
voter set, and finite validator-prefix induction; this composition introduces
no global application barrier and does not promote the ledger entry. The chain
refinement contains the authoritative indexed successor-instance product;
`SumeragiV2ChainLivenessProofs!HeightLivenessObligation` now lives in a child
of the successor-starvation proof module, so the composition no longer asks a
parent to import a child theorem. Its conditional kernel derives exact chain
recovery from an explicit source-eligibility leadsto, fair target opening, and
two explicit Async temporal properties: an exact historical target eventually
installs its durable Decision, and an outstanding Decision for every responsive
node eventually applies. The eligibility leadsto prevents a joined node with
no certified source from being treated as an enabled historical action. These
properties are named predicates, not assumptions or production-refinement
booleans. `SumeragiV2AsyncHistoricalRecoveryLivenessProofs` now states the two
exact post-GST properties over every `Responsive` validator. Its explicit
historical owner is `HistoricalRecoveryTarget(candidate.node)`, not the
current-voter-only parent owner. The child proves well-founded rank composition
for that owner and the weak-fair CommitQC discovery rule conditional on exact
pending-state preservation. It leaves clock/readiness, preservation across all
unrelated actions, the authenticated request/response-to-Decision corridor,
and the historical Decision/body/application corridor as visible operator
premises. Consequently neither exact property nor the ledger is promoted. The
release-facing theorem remains proofless and ledgered
`specified_unproved` until those prerequisites are discharged and the whole
theorem passes a fresh pinned strict proof
after rotating-leader, application liveness, and the separately ledgered
`SuccessorActivationStarvationFreedomObligation` and
`SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation`.
The former pins a lifecycle-aware `0..21` rank for each responsive validator:
Published predecessor ownership occupies the upper tier by adding `11` to the
`0..10` pipeline distance, while recovered/absent ownership occupies the lower
tier. Thus clean exact-complete-tip restart is a strict descent into the
absent-owner attempt, and snapshot-bootstrap authority is kept distinct from
the complete-tip credential. Applied failure retains Running until restart
and a Recovered attempt may fail repeatedly, so failure history is not a
ranking counter; `IndexedChainSpec` instead includes an explicit eventual
failure-free suffix premise. It makes
no progress claim for an honest validator outside `Responsive`, which may stop
with pre-GST local work still queued. Its release-facing theorem contains a
candidate suffix-local weak-fairness proof, well-founded rank composition, and
an explicit temporal suffix lift, but remains `specified_unproved` until
strict TLAPS discharges that proof. The latter is deliberately not a
theorem about model state alone. Its exact statement conjuncts
`ProductionSuccessorAndExactRecoveryTraceRefinement` with the indexed model
invariant. That source predicate contains six unassigned booleans for Applied
publication, Recovered publication, split startup failure/restart,
authenticated historical-certificate import, the ordinary historical body
pipeline, and the authenticated terminal Apply boundary before successor
construction. The sixth production kernel has no `MaxHeight` input: it proves
that exact context, receipt, artifact, block, and durable-predecessor identity
agree while no successor activation is pending. `MaxHeight` remains only a
finite-horizon projection. Source token/order checks,
adversarial production tests, stale-token mutation tests, and source-manifest
binding constrain those claims, but do not prove any of them. Consequently the
already proved abstract successor invariant cannot discharge this production
seam; only machine-checked cross-tool trace evidence for every claim may add a
proof body or promote its ledger status. Historical recovery is an exact Async
reducer path rather than a second consensus or receipt relation: an
authenticated current voter serves an
already canonical exact CommitQC, the Core imports that envelope into ordinary
certificate delivery, and the reducer performs decision persistence, body
recovery, store, validation, and application. Production may schedule the
first request immediately when startup recovers durable v2 ownership or an
interrupted tip. It carries that urgency across a height only when an
authenticated Commit-certificate response yields a discovered CommitQC which
is admitted to, or coalesced with, serialized reducer ownership; ordinary live
finality clears it. The corresponding outstanding-request `Some`-to-`None`
transition proves neither reducer execution, Decision, durability, nor
historical-Kura provenance. This is a scheduling refinement of the same exact
Async import path, not an alternate certificate, authentication, or consensus
relation. Its concrete runner mapping remains part of the `specified_unproved`
production-refinement seam.

In the abstract indexed model, nonterminal application queues successor
startup and does not join; only exact
Applied or Recovered publication joins. Production's earlier internal State
application maps to that abstract Applied boundary only after canonical lane
completion. If block sync installs that canonical ownership after adapter
construction, production rehydrates its exact bounded proposal at rollover so
the missing lane certificate remains recoverable. Proving that delayed mapping
remains explicit refinement debt.
Recovered publication requires absent
process-visible predecessor ownership plus durable complete-tip authority. For
a tip whose canonical block owns lane payloads, that authority additionally
requires every exact ownership's durable lane certificate and application
receipt; global finality alone cannot activate the successor. An empty
canonical ownership set has no lane debt for a result-bearing genesis or other
external-only block. Recovery reopens
an incomplete tip for exact decided-lane traffic without re-entering global
reducer input and never writes a fictitious predecessor `Complete`. At terminal
`MaxHeight`, a
responsive observer records known application without advancing height or
creating activation state. Its
dormant `InitAt` parent receipts remain private to their one-height instances,
while the global projection contains only exact current-context receipt deltas.
No global asynchronous shadow state, alternate
consensus transition relation, or favourable-network protocol relation may
stand in for that proof.

No top-level assumption, axiom, or unledgered omitted proof can satisfy the
release checker. A proofless release theorem is accepted only when its exact
module and symbol are pinned as `specified_unproved`; it must be discharged by
TLAPS before machine-checked completion can become true. This is a structural
rule even outside release mode. The ledger also prohibits promoting async type
closure ahead of runner scheduler preservation, the production progress-witness
refinement ahead of the pure model witness, productive post-GST deadlock freedom
ahead of both model-witness preservation and protected-rank progress, timeout/
view liveness ahead of productive deadlock freedom, starvation freedom ahead of
service-rank progress, or genesis handoff or indexed height liveness ahead of
rotating-leader, application liveness, and successor-activation starvation
freedom. The successor/exact-recovery production bridge is an independent
safety/refinement seam, not a temporal exact-recovery-progress theorem, so it is
not used as either chain-liveness dependency. Stage-2, Stage-3, and Stage-6 do
not yet have canonical ledger entries; their scratch names therefore are not
encoded as false protected-rank dependencies.
The
conditional liveness premises remain visible as trusted contracts rather than
being restated as the theorem to prove.

Each of the 18 actions named by `AsyncFairnessAt` now carries its complete
category-specific outer frame. `AsyncFairActionAt` repeats the exact quantified
inventory. `AsyncFairActionsRefineAsyncNext` is the typed source claim, while
`SumeragiV2AsyncFairnessRefinementProofs!AsyncFairActionsRefineAsyncNextObligation`
is its deductive theorem. The decomposed proof projects typed command execution,
all 18 fair actions, and their runner, non-runner, and recovery outer frames. A
recorded exact release invocation proved 1,143/1,143 obligations; that
historical submodule result is not current aggregate source-manifest evidence.
The proof checker
rejects a missing action, changed quantifier domain, misclassified frame,
weakened claim or theorem, unreviewed helper theorem, duplicate TLC-only
variable tuple, or alternate TLC fairness relation. The complete Core `Next`
relation is not embedded in every `WF` target, because that redundant search
causes TLC to test unrelated conflicting Core branches during `ENABLED`.
This historically promoted `async-fair-action-refinement` to `tlaps_proved`.
Independently, the post-Decision timeout frontier and exact durable
Commit-Decision recovery lifecycle described below were also proved for that
superseded transition relation.
The abstract protected-rank prerequisites are separately ledgered as
`async-progress-ownership-invariant`,
`protected-service-rank-stage4-ready-causal`,
`protected-service-rank-serve-fifo`, and
`protected-service-rank-stage5-consensus-fifo`. All four were
`tlaps_proved` before the Core/Async transition change. Fresh strict `--nofp`
theorem-range runs under pinned TLAPM
`3ab43c7`, against
`SumeragiV2AsyncLivenessProofs.tla` SHA-256
`2fe00973f8f983fd7682667baadbccbbe422a6c34dec02205cfe9cfe697f95ec`,
proved the complete selected obligations: progress ownership at lines
33324--33348 (15/15), Serve FIFO at 42007--42049 (15/15), Stage 4 at
46313--46342 (9/9), and Stage 5 at 46344--46373 (9/9). Each invocation exited
zero. Declaration-only `--line` attempts selected zero obligations and exited
12; they were rejected rather than counted as evidence. Progress ownership
consumes the proved async type closure; Stage 4 and Stage 5 additionally
consume progress ownership, while all three rank leaves consume their exact
proved fair-action prerequisites. The aggregate
`protected-service-rank` obligation waits for every leaf, while production
admission, runtime, ingress, and actor-to-flush ownership remain outside these
abstract results. The 54-entry ledger contains 33 `tlaps_proved`, 14
`specified_unproved`, 6
`trusted_contract`, and 1 `out_of_scope` entries, so
`machine_checked_completion` remains false.

TLC runs exhaustive constant checks and bounded asynchronous counterexample
searches. It cannot upgrade a proof status. The scheduler corridor runs nine
mutation/repair pairs: equal-value replacement/coalescing, deferred-owner
replacement/scheduler-wide coalescing, strict/cyclic deferred-class selection,
Busy Completion requeue without/with cursor advance, handoff-free equal-rank
re-Busy/exact deferred handoff, head-only/indexed ingress,
aggregate-only/per-lane ingress capacity, conflated/separate work and
completion capacity, and producer-first/causal-debt alternating local
admission. In the last pair, the producer-first model has the pinned
three-state fair lasso and TLC status 13, while the repaired sticky-debt/cursor
model exhausts seven bounded states without error and returns status 0. These
are followed by the causal-capacity refill matrix, blind-successor/coalesced
replacement, in-runner/independent Commit-discovery, and all-I/O/Consensus-only
index mutations. An exhaustive one-validator configuration checks the logical
ownership invariant through 983,041 generated states, 99,328 distinct states,
and depth 49. The larger graph covers separate non-timeout-progress and
TimeoutVote ingress reservations. These are bounded regression witnesses, not
deductive proof and not a reason to promote a ledger entry. Two reply-route
mutations use the production-shared abstract kernel: resetting a cursor across
a new connection tenure and replacing an alternate source both violate
`RouteMutationSafety`, while the fixed prefix also covers exact retry, a later
delivery update, A/B source isolation, and per-semantic rebind after
reconnect. Two additional seam models make the remaining temporal
gap executable: an unprotected Normal proposal/Prepare candidate starves, and
a dynamic delivery-class mutation loses a stored CommitVote after a TC, while
the frozen constructor inventory closes both cases; separately, a
scheduler-only deadlock claim accepts a bare tick, whereas the productive claim
rejects it until a concrete deadline, evidence, or rank repair exists. The
productive release obligation remains `specified_unproved`. The production
trace replayer and adversarial simulations exercise the exact reducer sources,
while an older pinned Verus receipt proves only the source-linked reducer/WAL
and scheduler kernels that it actually hashed. That receipt predates the
proposal-origin changes and was not rerun for the current source. The
remaining cryptographic, deterministic-execution, operating-system durability,
post-GST transport, and host-service premises are listed explicitly in the
ledger and formal README.

The formal gate also seals and executes a dedicated effect-capacity ownership
matrix consisting of 6 models and 28 configurations. Its 10 repaired cases
complete, while all 18 mutants fail at their named invariant or temporal
witness; together they generate 147 states and reach 146 distinct states. The
matrix exercises persisted TimeoutVote-Sign ownership at capacity two,
deterministic Fetch preemption and decided-owner exclusion, fair
non-preemptible retirement, reconstructible full-capacity Fetch rejection, and
bounded retained-effect FIFO behavior. Its certified-request seam separates a
one-entry request bound from two general work slots. Only a `FetchBody` producer
may retain and retry an independent request-capacity rejection, and that
rejection leaves both work and request allocation unchanged. The exact
authenticated `CertifiedBodyResponse` with a still-live matching logical
request registration is transport-only, so it may cross the retained
reducer-effect suffix and atomically retire the old Fetch/request pair. The
durable producer then reconstructs and retransmits the Fetch so it acquires
both owners without an observable partial state.
`CommitCertificateResponse` remains reducer-ordered because its
authenticated CommitQC is submitted to the reducer before discovery ownership
retires. A sixth model selects either a certified response or payload chunk
behind saturated generic outer-ingress count and bytes. Both kinds share one
per-validator TransportCompletion count slot and full-envelope byte reserve;
independent classification mutants reproduce the lasso for each kind. The
model's weak-fairness result is conditional on a responsive certified source
and terminating transport and body work; those are premises, not consequences
of the finite state search. Crash/restart authority is
explicitly delegated to `SumeragiV2CrashReplayMutation`. These are finite TLC
regression witnesses, not a deductive proof or promotion, and therefore do not
alter any proof-coverage status, promote a ledger obligation, or change
`machine_checked_completion`.

A separate source-sealed post-Decision timeout/TC matrix contains one model
and nine configurations. The repaired deterministic trace completes with TLC
status 0; eight single-seam mutants cover `BeginTimeout`, `ResumeTimeout`,
`FormTC`, `BeginInstallTC`, both receive-pool branches, and both
causal-successor branches, returning status 12 at their exact named
invariants. The source proof exposes `DecisionTimeoutFrontierInvariant` and
exact node/generation-bound `DecisionRecoveryAuthority`. The full Core action
induction, including crash and `ResumeTimeout`, brackets every Core-stuttering
scheduler step and lifts through `AsyncNext` and the temporal specification;
it is proved by strict TLAPS independently of `AsyncTypeInvariantObligation`.
The ledger promotes the post-Decision timeout frontier. A separate 270-
obligation strict proof establishes the same-node/same-context durable/
pending frontier, Commit-only recovery authority, generation-free logical
request registration across crash and restart, explicit Prepare rejection,
and the replay reset's exact singleton current-generation `FetchBody` update.
It derives the reachable Core type facts locally instead of consuming the
still-unproved global async type obligation. An independent sentinel keeps the
Rust/Verus-to-TLA durable-owner, scheduler, and application trace mapping
`specified_unproved`; abstract TLAPS success cannot promote production
progress without it. The matrix is regression evidence only.

A separate certified-response registration matrix contains one model and five
configurations. The duplicate, authenticated-restart, and historical-catch-up
repairs complete only when a signed response matches an exact currently live
request registration. Two missing-guard mutants accept a second fan-out
response after retirement or a delayed pre-replay response after restart has
cleared the registration, and both fail their named invariants. This bounded
matrix pins the executable authorization seam and complements, rather than
substitutes for, the strict deductive crash/restart/replay authority proof.

The source-sealed durable Decision lifecycle matrix adds one repaired trace and
eight targeted mutations. It keeps Commit authority and the logical request
registration generation-free while modeling the executor generation as a
separate variable. The mutants independently violate durable Decision
uniqueness, generation-free or recipient-independent registration, replay
clearing, FetchBody reconstruction, current-generation execution, Commit-only
authority, and singleton replay. The nine deterministic graphs total 42
generated/distinct states: the repair completes, and every mutant fails its
named invariant. These finite graphs guard the checked proof vocabulary but do
not discharge the Rust/Verus-to-TLA refinement sentinel.

The production applied-height output handoff is an atomic, source-sealed typed
contract rather than a variant whitelist. It first rechecks every retained
network-message hash. Historical CommitQC, certified-body, and lane-certificate
claims are singleton target-bound identities and independently reread the exact
Kura finality artifact, canonical body, or certified lane artifact at handoff.
The applicable responder/signature, subject/body/manifest, proposal/QCs, and
response hashes are revalidated; missing, wrong-identity, or substituted
sources fail closed. Current-height global V2 claims bind protocol, Decision
context/height, and the exact finality artifact. A winning lane claim requires
the exact durable Kura certificate and application receipt, revalidates
alternate vote/QC/certificate proof variants, and explicitly supersedes
structurally valid same-height non-winning lane output. The winning set comes
from canonical finalized-block ownership, not volatile output. Missing evidence
keeps the same terminal height active; conflicting evidence fails closed; only
the complete Kura-first set permits handoff. Rollover rehydrates bounded
canonical ownership which arrived after adapter construction before it retries
the exact decided-lane certificate path. Startup enforces this at the live tip
only because older lifecycle sidecars may be canonically retired. Native
AMX claims bind
creation scope, embedded round, and message hash; merge-share claims bind scope
and share hash. Certified-sidecar request/chunk claims bind scope, target roles,
transfer identity, and exact request/response hash. Finalized-sidecar pruning
leaves winning data in the committed merge log and supersedes losing pending
work before handoff. Manual or otherwise untyped `Exact` output remains owned
and fails closed. These source contracts do not promote the application,
reconstruction-refinement, or starvation obligations; the added rollover and
tip-recovery regressions remain executable evidence under
`specified_unproved`, not a machine-checked completion claim.

The current pre-network release inventory names 569 tests across thirty-nine Rust
modules. The preceding 298-name inventory arose from the 264-name inventory by
adding 37 positive regressions which
comprise 10 per-target exact-output and historical/current typed-rollover tests,
2 peer-writer flush/old-generation custody tests, 20 exact progress-ticket,
topology, removal, replacement, and identical-retry tests plus
distinct/cross-kind broadcast-residual and subscriber-backlog tests, and 1
runtime/Busy-deferred exact CommitQC coalescing test, plus 4 Nexus lane-relay
ownership/fairness tests. Removing the obsolete adapter cursor alias and two
superseded network broadcast-residual tests yielded the net delta of 34.
Relative to the resulting 298-name inventory, the bounded per-source closure
added 110 exact tests and removed two superseded names, for a net increase of
108 and an exact total of 406. The preceding current-source geometry closure
added 14 exact tests without removing a name, for an exact total of 420. The
route-lifecycle closure added three exact P2P tests without removing a name,
for the historical 423-test checkpoint. Mechanical source reconciliation then
added 16 tests and three owning modules, producing 439 tests across 29 modules;
a renamed lane-relay saturation test replaced its obsolete name without
changing cardinality. The in-flight sidecar redelivery regression raised the
inventory to 440, and three terminal-flush/reconnect worker regressions produced
the historical 443-test, 29-module, 52-leg checkpoint. The subsequent
source-authority, immutable-sidecar, runner-race, daemon-corridor,
shared-byte-budget, cached-Arc-admission, and executable-refinement closure adds
22 exact regressions and moves two peer tests to their actual owning module,
yielding the 465-test, 30-module, 53-leg checkpoint. The authenticated
non-validator source-cap regression and the alternate-route-before-lane-cap
regression add two exact names, yielding the 467-test checkpoint without
adding a module or corridor leg. Three daemon Hold/Release controller
regressions, one layered daemon ownership regression, and two root
configuration geometry regressions add six exact names, yielding the historical
473-test checkpoint without adding a module or corridor leg. One configuration
fingerprint, two historical-recovery kernel, and one shared authenticated
source-credit regression add four exact names, yielding the historical 477-test
checkpoint without adding a module or corridor leg.
They bind each delivery capability to its
original minting tenure across bounded retired-source tombstone churn, reject
a second rehydrated capability instead of overwriting the first, and prove actor
normal-exit/`Drop` teardown retires every actor-owned route while canceling only
that actor's waiters. The geometry additions cover semantic
request identity, independent per-source routes/cursors,
exact-envelope deferred service and semantic-origin separation, writer-flush
identity, sidecar source isolation,
runner/worker route retention, daemon Hold/Release failure handling,
actor-global deferred capabilities, scheduler ownership handoff, and typed
fail-closed ordinal/debt boundaries. They also cover source-swapped response
and chunk rejection, alternate-source runtime/orphan ownership, actor-global
ordinals across tenures, and checked `iroha_config` source/capacity geometry.
The proposal-origin, multi-carrier, and persistence-failure closure adds 41
exact regressions while retiring nine superseded selectors,
yielding the 509-test, 38-module, 61-leg checkpoint. Those regressions
additionally bind reducer and deferred identities, equivocation evidence,
aggregate signatures, finality/header geometry, compact offline QCs, and parent
height-context identity to the signed origin.

The final successor/recovery closure adds six exact regressions without adding
a module. The per-source route-attempt, exact PrepareQC recovery, locked-body
reproposal, runner/worker, sidecar, and daemon closure yields the current
569-test, 39-module inventory. The complete source-sealed pre-network corridor
contains 72 legs. The canonical module/test TSV inventory SHA-256 is
`be314a8e489645cec1d2e141fee08d8bc506bb12e107156f924f9651c83d727e`.
The added boundaries preserve the frozen predecessor CommitQC through
wire-to-core conversion, block rollover until the decided lane session is
durable, reopen a globally finalized tip whose lane evidence is incomplete,
and filter CommitQC discovery and losing current-body requests at terminal
ingress. They also accept canonical view-zero bytes whose first proposal origin
is later and pin a contention-tolerant restart view-zero deadline. The genesis
finality regression's whole-item token SHA-256 is
`bfbd01d093f38fa8c96fb17fe38b6ec1132e6ffbb0d09367a298299394bdce4f`,
and the restart-deadline regression's is
`13c1cd988856a8c4ee4d20cfc176c4111352ba7262d07bb417de5a4056cf8b1f`.
The same four-validator scenario owns sequential missing-height discovery and
catch-up. A diagnostic rerun exposed one full 20-second discovery delay at
every missing height. The fresh exact run of
`sumeragi_v2_runner::authoritative_v2_finalizes_through_validator_restart`
against recovery-scoped eager discovery passed 1/1 in 79.82 seconds; all four
peers shut down gracefully with empty stderr. This focused regression does not
promote any proof-ledger entry. The successor-boundary regression's whole-item
token SHA-256 is
`ee773b00e696822c6d2ba998fb88201bb6e2a06eac749a2c700edec70dbbdf74`;
its extended authenticated-admission companion is sealed at
`1cb4736b2e4b499403c870cc3dd5ab8ccd361d51887efad4178ed7d39a9e0225`.
They are local ownership and reconstruction
contracts, not remote application acknowledgement, relay second-hop
completion, or unbounded broadcast admission. The 264-name baseline added 32
atomic-lane, semantic-origin, P2P source-fairness, daemon-relay, and active-
watchdog regressions. The 232-name baseline already included two exact locked-Commit
progress-witness regressions
and six outer TransportCompletion-corridor regressions. The current
geometry inventories four owners per validator, two owners for every one of
the `H` simultaneously materialized authenticated non-validator lanes, and two
anonymous owners (`4N+2H+2` total), including a roster-origin completion relayed
through an authenticated non-validator hop, and retains the capacity-negative
boundary. It
also adds one four-validator exact PrepareQC count-and-power quorum regression.
The five integration names share a module-filtered leg; the pre-network corridor
now has 65 legs, including separate exact data-model status and atomic
lane-certificate decode contracts, two `iroha_config` geometry modules, three P2P
geometry modules, the daemon genesis module, and source-sealed command-success
legs. Its finality, offline compact-QC, and height-context proposal-origin
modules each use a dedicated `iroha_data_model` leg. Its `iroha_p2p` legs use
the crate's empty default feature set; feature-gated QUIC first-packet geometry
tests are not claimed by the thirty-eight-module, sixty-five-leg corridor. It
includes
exact completion ownership, body-owner binding and
rebind, rejection of future physical completions, durable-recovery retry to the
latest consumer, byte retirement, three-class production arbitration, the exact
`4N+2H+2` ingress and `2N+3` deferred partitions, successor activation/recovery,
authenticated exact historical recovery, retained effect-capacity ownership,
post-decision timeout/TC quiescence, and watchdog classification. It also pins
the adapter's maximum flattened persistence macro-step at five effects within
the reducer's eight-effect bound, services at most one Busy-deferred adapter
macro-step per serialized runtime turn, and forbids terminal readiness while
any Completion, Progress, or Normal deferred queue remains nonempty. The
production-default saturation regression fills all 256 certified-request
owners, 640 Normal ingress slots, and the 128-slot reserved Progress increment
while preserving the 256-slot Completion reserve, then proves that an exact
authenticated `CertifiedBodyResponse` with a still-live matching logical
request registration can retire the old request; durable reducer
retransmission then reconstructs the blocked Fetch and lets it acquire both
owners atomically. The
preceding mutable-source discovery and direct execution evidence covered the
earlier 168-name inventory. Fresh 515-name
discovery/execution and the clean committed, detached, source-sealed serial
release leg remain pending. An
earlier exact one-attempt
four-validator genesis rerun is green at 1/1 in 456.76 seconds. Neither
inventory presence nor regression evidence is a machine proof.
For the current proposal-origin source, the isolated source-shared harness
passed 118/118 reducer/WAL/refinement tests and all 8 model-trace replay tests;
fast-network passed all 9 named simulations. The 2026-07-21 100,000-height
permissioned/NPoS chaos run completed both 50,000-height prefixes, 400,000
validator finalizations, and zero failures in 91.29 seconds. These mutable-tree
results are implementation evidence only. The pinned Verus receipt predates
the proposal-origin changes and was not rerun, so it does not discharge the
changed source-shared obligations.
The Core delivery relation and normalized trace replay match exact-lock
admission and post-WAL pruning. A recorded pre-current-edit strict run
discharged 7,826 induction obligations and 565 downstream Core safety
obligations for the superseded transition relation. The new higher-origin
fence, no-high proposal rule, same-round TC upgrade, and durable-timeout
recovery witness make that receipt historical and current-source-unproved; it
does not promote any changed obligation.
The asynchronous liveness proof-premise repairs remain outstanding. A fresh
pinned strict whole-module aggregate release TLAPS run, the clean source-sealed
release gate, the release-profile 100,000-height chaos rerun, and the 24-hour
Taira-profile soak remain pending.
