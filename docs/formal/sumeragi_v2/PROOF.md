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

Every signature binds the chain, protocol, height-context ID, height, view,
phase, and subject through its canonical preimage. Correct validators obey the
durable rules enforced by the reducer and WAL:

1. they sign at most one Prepare and one Commit per `(context, view, phase)`;
2. they store and deterministically validate the exact body before persisting
   and signing Prepare;
3. they atomically persist a PrepareQC lock and Commit intent before signing
   Commit;
4. a durable timeout intent prevents later Proposal or Prepare signing and
   ordinary creation of a new Commit intent in that view; after a TC durably
   promotes its selected PrepareQC to the exact active lock, local durable body
   validation may persist that one historical Commit only if no higher
   conflicting-subject local Prepare intent or known PrepareQC exists; an
   already-durable Commit may be re-signed or retransmitted only while its
   round and subject remain the exact active lock;
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

**Lemma 6 (a formed TC either directly protects or intersects durable
installed-TC authorization for every still-formable old CommitQC).** Consider
a subject `x` at view `r` whose durable Commit-intent signers form a dual quorum
`C`. Let a TC for a view `v >= r` have signer union `T`.

By the quorum lemmas, `C ∩ T` contains a correct validator `h`. If `h`'s
timeout vote strictly protects its Commit intent, the TC's maximum semantic
high-QC selection lifts that local fact to direct protection of `(r, x)`: its
selected rank is at least `r`, and equality binds subject `x`. If the timeout
does not strictly protect the Commit, the Commit can only be the narrow
late-created historical-lock exception. The durable provenance invariant then
supplies an installed TC whose selected PrepareQC exactly authorizes `h`'s
Commit round and subject. The authorizing installed TC is not required to be
later than the formed TC being examined. The timeout signer sets are disjoint
and their union is the quorum `T`, so grouping does not change the
intersection argument.

An old CommitQC already formed before the TC remains directly decisive.
Ordinary Commit-intent growth remains timeout-fenced. The sole late-growth
case is the TC's exact selected PrepareQC: installation durably records that
provenance and promotes the matching lock, body validation terminates, and the
node has no higher conflicting-subject Prepare intent or known PrepareQC.
Higher same-subject reproposals are harmless. The grouped-timeout set kernel is
machine-checked for intents already present at timeout. The temporal induction
which retains installed-TC authorization for the late exact intent, and the
dependent direct-or-installed-authorization wrapper, are also ledgered
`tlaps_proved` after their current strict proof closure succeeded.

## Agreement and chain prefix

**Theorem 1 (agreement).** Two valid CommitQCs in one height context cannot
certify different subjects.

Assume for contradiction that `CommitQC(x, r)` and `CommitQC(y, s)` exist with
`x != y`; choose such a pair with the smallest later view `s`, and take
`r <= s`. Lemma 3 rules out `r = s`. The earlier CommitQC gives a dual quorum
of durable locks on `x` at rank `r`. Lemma 6 does not falsely infer that every
older TC's own high protects a Commit created later. It instead preserves the
two proof routes required by the durable lineage invariant: direct high-QC
protection, or an honest intersection signer whose exact late Commit remains
authorized by an installed TC selecting the same Prepare lock.

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
the live transition. Prepare replay remains current-view and timeout-fenced.
Commit replay may cross that timeout or a later installed TC only for the exact
durable vote at the active lock's round and subject; it cannot authorize an
unlocked historical Commit intent.

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

The claim is conditional and per height: after GST, with a responsive dual
quorum and terminating local work, every height eventually decides and every
responsive validator eventually applies it. No claim covers an unbounded
partition, loss of either quorum threshold, or local work that never returns.

**Lemma 7 (generation- and consumer-scoped locked-vote delivery).** Clearing a
volatile vote pool cannot orphan the exact durable locked Commit intent.

Authenticated vote history is immutable and distinct from the volatile receipt
pool. An exact vote already present in that pool is suppressed. Persisting a TC
clears only the installing node's pool and advances its reducer generation
(until the finite model's generation bound). At that acknowledgement boundary,
the reducer compares the resulting lock with its durable Commit intents. An
exact match is queued for re-signing. A newly TC-promoted lock without an
intent never signs merely from installation. Its exact body recovery and
current-generation validation schedule `BeginLockCommit`; only the matching
historical WAL acknowledgement may then release the signature, and only when
no higher conflicting-subject local Prepare intent or known PrepareQC exists.
Completing the signature inserts the vote directly into the installing node's
new pool, then broadcasts only to the other voters because the P2P broadcast
does not deliver to its sender.

Every Commit vote, including one from the current view, is admissible only when
its round and subject equal the active durable Prepare lock. A premature
current-view Commit is a recoverable ignore. When the matching
`LockAndCommit` acknowledgement applies a newer lock, the reducer then prunes
the superseded historical pool before releasing the local Commit signature;
pruning earlier would orphan the old reconstruction source if persistence
failed. The adapter advances the locked-Commit consumer epoch at that boundary,
so the ignored exact vote may enter once after authority exists. Thus the
bounded live set is current Prepare plus either the historical locked Commit or
the current locked Commit, never all three.

Retained signed Commit control remains the peer-delivery source, but cannot by
itself witness local reconstruction because broadcast excludes its sender. A
retransmitted historical Commit which matches the exact lock is classified as
protected progress and consumes each peer's new pool epoch exactly once.
Prepare votes and unrelated old Commit votes remain inadmissible. If a crash
happened after the Commit intent became durable but before its signature or
broadcast, `ResumeVote` reconstructs that same exact locked Commit even after
the timeout or TC. The distinct validation path above may create the missing
exact intent, but replay never invents one without its preceding installed-TC
lock and acknowledged `LockAndCommit` frame.

**Lemma 8 (bounded scheduler and transport service).** A Byzantine flood cannot
starve an authenticated responsive source or an admitted progress/completion
command.

Each recipient/source lane is bounded and the ready queue rotates every
non-empty source. Within the selected source, ingress removes the oldest
currently admissible entry; an auxiliary request waiting for I/O capacity
therefore cannot hide later consensus or certified-body progress, and every
earlier blocked entry remains in its original order. Exact authenticated
envelopes coalesce while an equal occurrence remains owned anywhere in the
scheduler, including deferred, causal, runtime, ready-completion, I/O, or
outstanding-work ownership. Servicing that owner therefore cannot leave an
equal replacement at the same logical rank. A pinned mutation demonstrates the
old deferred-owner replacement lasso and the scheduler-wide coalescing repair.
Transport packets and ingress entries retain occurrence-specific service
arguments instead of being folded vacuously into that candidate rank. Timeout
delivery also establishes membership in the full canonical envelope record set
before consuming authorization fields, excluding augmented records that merely
expose a well-typed selected view.

Local admission alternates a producer-completion source with the causal-work
source. A producer selection while causal work waits records sticky causal
debt and advances the source cursor; only causal selection clears that debt.
Once the causal head is admissible, the debt makes it the deterministic
preference under fair `RunNode` service, so repeated producer replenishment
cannot reset the causal owner's position.

An individual signed Vote cannot establish its own execution-commitment
authority. It is serviceable only after a local validated receipt, verified WAL
replay, or quorum-authenticated QC binds the exact round and subject; an unbound
Vote is rejected recoverably until that binding arrives. Body-availability
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
rank obligation remains explicit proof debt.
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
during the round's timeout, after which the next responsive correct leader
proposes the selected safe subject. The responsive quorum obtains, stores, and
validates the exact body before Prepare; forms and disseminates PrepareQC;
atomically locks and persists Commit intents; and forms CommitQC within the
timeout. Each node persists the decision, fetches the certified manifest and
chunks if needed, reconstructs and validates the exact body, applies it, and
advances its own height. None of these local transitions waits for every other
correct node.

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
modules with pinned TLAPM commit `763bf3c`, requires a positive
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
theorem have checked proof bodies. The type-closure wrapper nevertheless
remains ledgered `specified_unproved` because its induction consumes the
still-unproved concrete runner scheduler-preservation leaf. The timeout durability, signing,
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
`EffectiveLockAcquisitionModelObligation` is still `specified_unproved`;
exhaustive bounded TLC is counterexample search, not a deductive discharge.
Ordinary-Rust map/hash/service projection, worker/request ownership, and
post-GST fairness remain separately in
`EffectiveLockBodyAcquisitionProductionRefinementObligation`, also ledgered
`specified_unproved`. The exact enabled-`RunNode` result is only a scheduler
lemma: post-GST deadlock freedom requires an enabled `AsyncNext` step that grows
current-height protocol evidence, strictly consumes a concrete deadline debt,
or decreases/exits a protected candidate or Serve-occurrence rank. Repeated
clock or view-change steps alone do not satisfy that productive obligation.
Stage-2/3/6, packet-admission, and zero-deadline cases therefore remain explicit
proof debt. Runner preservation and starvation now have source
proof bodies, but remain `specified_unproved`; the runner still needs a fresh
pinned strict proof and starvation depends on the still-unproved service-rank
theorem. Durable witness,
rank decrease, and the remaining stable-suffix liveness theorems are exact
universally quantified declarations recorded as `specified_unproved`. The
argument above does not upgrade any of those statuses. The concrete genesis
chain product's first-successor handoff also has a source proof body but remains
ledgered `specified_unproved`. The chain
refinement now contains the authoritative indexed successor-instance product
and its exact `SumeragiV2ChainEpochRefinement!HeightLivenessObligation`. Its
source proof body composes instance activation, exact-action fairness,
exact historical recovery for responsive validators absent from an old roster,
and finite-height temporal induction, but the ledger remains
`specified_unproved` until the whole theorem passes a fresh pinned strict proof
after rotating-leader, application liveness, and the separately ledgered
`SuccessorActivationStarvationFreedomObligation` and
`SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation`.
The former pins the minimal `0..19` queue-to-publication rank for each
responsive validator, including the one-time pre-failure `+9` offset. It makes
no progress claim for an honest validator outside `Responsive`, which may stop
with pre-GST local work still queued. The latter maps production startup and
block-sync traces to the model. Historical recovery is an exact Async reducer path rather than a
second consensus or receipt relation: an authenticated current voter serves an
already canonical exact CommitQC, the Core imports that envelope into ordinary
certificate delivery, and the reducer performs decision persistence, body
recovery, store, validation, and application. Nonterminal
application queues successor startup and does not join; only exact Applied or
Recovered publication joins. Recovered publication requires absent
process-visible predecessor ownership plus durable complete-tip authority and
never writes a fictitious predecessor `Complete`. At terminal `MaxHeight`, a
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
closure ahead of runner scheduler preservation, post-GST deadlock freedom ahead
of async type closure, starvation freedom ahead of service-rank progress, or
genesis handoff or indexed height liveness ahead of rotating-leader,
application liveness, successor-activation starvation freedom, and the
production successor/exact-recovery refinement seam.
The
conditional liveness premises remain visible as trusted contracts rather than
being restated as the theorem to prove.

Each of the 18 actions named by `AsyncFairnessAt` now carries its complete
category-specific outer frame. `AsyncFairActionAt` repeats the exact quantified
inventory. `AsyncFairActionsRefineAsyncNext` is the typed source claim, while
`SumeragiV2AsyncFairnessRefinementProofs!AsyncFairActionsRefineAsyncNextObligation`
is its deductive theorem. The decomposed proof projects typed command execution,
all 18 fair actions, and their runner, non-runner, and recovery outer frames;
the exact release invocation proves 1,143/1,143 obligations. The proof checker
rejects a missing action, changed quantifier domain, misclassified frame,
weakened claim or theorem, unreviewed helper theorem, duplicate TLC-only
variable tuple, or alternate TLC fairness relation. The complete Core `Next`
relation is not embedded in every `WF` target, because that redundant search
causes TLC to test unrelated conflicting Core branches during `ENABLED`.
This promotes only `async-fair-action-refinement` to `tlaps_proved`; the
46-entry ledger still contains 15 `specified_unproved`, 6 `trusted_contract`,
and 1 `out_of_scope` entries, so `machine_checked_completion` remains false.

TLC runs exhaustive constant checks and bounded asynchronous counterexample
searches. It cannot upgrade a proof status. The scheduler corridor runs the original eight
mutation/repair pairs: equal-value replacement/coalescing, deferred-owner
replacement/scheduler-wide coalescing, strict/cyclic deferred-class selection,
Busy Completion requeue without/with cursor advance, head-only/indexed ingress,
aggregate-only/per-lane ingress capacity, conflated/separate work and
completion capacity, and producer-first/causal-debt alternating local
admission. In the last pair, the producer-first model has the pinned
three-state fair lasso and TLC status 13, while the repaired sticky-debt/cursor
model exhausts seven bounded states without error and returns status 0. These
are followed by the causal-capacity refill matrix, blind-successor/coalesced
replacement, in-runner/independent Commit-discovery, and all-I/O/Consensus-only
index mutations. An exhaustive one-validator configuration checks the logical
ownership invariant through 42,817 generated states, 6,208 distinct states,
and depth 45. The larger graph covers separate non-timeout-progress and
TimeoutVote ingress reservations. These are bounded regression witnesses, not deductive proof and not a reason to
promote a ledger entry. Two additional seam models make the remaining temporal
gap executable: an unprotected Normal proposal/Prepare candidate starves, and
a dynamic delivery-class mutation loses a stored CommitVote after a TC, while
the frozen constructor inventory closes both cases; separately, a
scheduler-only deadlock claim accepts a bare tick, whereas the productive claim
rejects it until a concrete deadline, evidence, or rank repair exists. The
productive release obligation remains `specified_unproved`. The production trace replayer and adversarial
simulations exercise the exact reducer sources, while the pinned Verus harness
proves the source-linked reducer/WAL and scheduler kernels. The
remaining cryptographic, deterministic-execution, operating-system durability,
post-GST transport, and host-service premises are listed explicitly in the
ledger and formal README.

The current pre-network release inventory names 166 tests across fourteen Rust
modules. It includes exact completion ownership, body-owner binding and
rebind, byte retirement, three-class production arbitration, the exact
`3N+1` ingress and `2N+3` deferred partitions, successor activation/recovery,
authenticated exact historical recovery, and watchdog classification. Cargo
discovery found all
162 then-required names among 6,744 tests with no missing or ignored release
test before the four replay-FIFO/refinement regressions raised the inventory to
166; the authoritative ingress module was green at 30/30. Fresh 166-name Cargo
discovery and the clean committed, detached, source-sealed serial release leg
remain pending. An earlier exact one-attempt
four-validator genesis rerun is green at 1/1 in 456.76 seconds. Neither
inventory presence nor regression evidence is a machine proof.
The Core delivery relation and normalized trace replay match exact-lock
admission and post-WAL pruning. The repaired full strict induction discharged
all 7,826 obligations and the downstream Core safety wrapper discharged all
565, promoting only historical TC-lock authorization and timeout protection.
The asynchronous liveness proof-premise repairs remain outstanding. Full
strict proof completion, the release-profile 100,000-height chaos rerun, and
the 24-hour Taira-profile soak remain pending.
