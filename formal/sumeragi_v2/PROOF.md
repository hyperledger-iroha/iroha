# Sumeragi v2 safety and liveness argument

## Current proof-ledger status

The checked-in revision-4 ledger declares 44 `tlaps_proved`, 3
`cross_tool_proved`, 0 `specified_unproved`, 6 `trusted_contract`, and 1
`out_of_scope`, with `machine_checked_completion: true`. This declaration is
the byte-exact input to the release proof wave, not proof evidence by itself
and not a deductive proof of revision 4. Release completion still requires
strict TLAPS, pinned Verus, derived cross-tool and production-trace evidence,
the separate revision-4 exact-cardinality TLC/mutation corridor, and
receipt/completion-marker checks against one frozen signed candidate.
Mechanization-status statements below that report the earlier 35/12 split,
call an obligation `specified_unproved`, or set
`machine_checked_completion: false` are historical snapshots retained as
provenance; they do not override the current status declaration.

## Revision-4 argument

Revision 4 is a fresh-genesis protocol. For each frozen height context, let
`V` be the unit-vote validator committee and require

```text
n = |V| = 3f + 1,    1 <= f <= 10,    q = 2f + 1.
```

The protocol therefore admits exactly 4, 7, ..., 31 validators. At most `f`
committee members may be Byzantine. Stake affects election and eligibility,
but it does not weight a Prepare, Commit, or timeout vote. Every certificate
uses `q` distinct validator identities from the one frozen committee.

The broader deductive quorum library deliberately keeps `DualQuorum` as a
monotone sufficiency predicate: signer supersets remain useful in intersection
and liveness arguments. They are not valid serialized certificates. The Core
wire predicates require the exact minimum count, and every QC/TC constructor
projects a raw collected pool to the first `q` eligible identities in frozen
`RosterSequence` order. The projection theorem proves exact count when at least
`q` roster candidates exist; deriving the full dual predicate additionally
retains the explicit power-quorum premise because roster order alone cannot
establish weighted sufficiency.

A height-seeded permutation of `V` is rotated cyclically by view. In each view,
Set A is the first `q` validators, including the leader at its head and the
proxy tail at its end; Set B is the remaining `f` validators. The proxy tail is
the sole fast-path vote collector. There is no backup collector. The first
retransmission deadline activates same-view fallback: it keeps the exact
proposal, view, lock, and quorum rule, but expands full-body recovery and voting
eligibility from Set A to all of `V`. A certificate of `q` timeout votes changes
the view and rotates every role. Installing that certified view change resets
the fallback flag before the new view starts.

Every honest Prepare voter must reconstruct the complete canonical body from
the mandatory Reed-Solomon-16 layout, verify the manifest and body hashes,
durably store the exact body, and deterministically validate it before
persisting its Prepare intent. Commit voting additionally requires the matching
PrepareQC and durable lock. Honest validators sign at most once per
height-context, view, and phase, retain locks across fallback and view changes,
and only accept a proposal justified by the highest certified lock. Signatures
bind the height context, view, phase, proposal identity, body commitment, and
execution commitment. These durability, authentication, deterministic
execution, and safe-proposal checks are trusted production-refinement
assumptions of the compact model.

### Unit-quorum intersection

For any two revision-4 quorums `Q1` and `Q2`,

```text
|Q1 intersect Q2| >= |Q1| + |Q2| - n
                  = 2(2f + 1) - (3f + 1)
                  = f + 1.
```

Because at most `f` validators are Byzantine, the intersection contains an
honest validator. Each quorum also contains at least `f + 1` honest
validators. No stake-power premise is needed or permitted.

### Safety

**Lemma 1 (same-view certificate uniqueness).** Two valid PrepareQCs, or two
valid CommitQCs, for one height context, view, and phase cannot certify
different bodies.

The two signer sets intersect in an honest validator. That validator would have
to violate its durable sign-once rule to sign both bodies.

**Lemma 2 (full-body external validity).** A valid PrepareQC certifies that at
least `f + 1` honest validators durably possess and have deterministically
validated the exact body. A CommitQC inherits the same property.

A QC has `q` distinct signers, at most `f` of whom are Byzantine. The
full-body-before-Prepare gate applies independently to each honest signer;
possession of a manifest or a partial shard set is insufficient. A Commit vote
requires that same full-body authority plus the matching PrepareQC and lock.
Thus a certified body remains recoverable from honest QC signers after a leader
or proxy-tail failure.

**Lemma 3 (fallback preservation).** Entering same-view fallback cannot create
a conflicting certificate.

Fallback changes only which committee members may reconstruct the body and
vote. It does not change the proposal, view, phase, signer identity, sign-once
slot, lock, or `q` threshold. Set B votes are therefore ordinary unit votes in
the same certificate domain, not a second or weaker quorum system. A certified
view change clears this view-scoped eligibility expansion, so stale fallback
authority cannot leak into the rotated view.

**Lemma 4 (certified view-change lock preservation).** A certified view change
cannot replace a locked body with a conflicting body.

A Commit quorum for body `x` contains at least `f + 1` honest locked
validators. Fewer than `q` other validators remain, so a conflicting
PrepareQC cannot be the first higher certificate unless one of those honest
validators abandons its lock. The safe-proposal rule permits such a validator
to move only through a strictly higher certified lock. Choosing the first
conflicting higher PrepareQC yields the same contradiction recursively: its
quorum intersects the protected quorum in an honest validator that cannot
justify the change. Timeout votes bypass the proxy tail, and the certified
view-change value carries the highest safe lock into the rotated view. The
compact revision-4 model represents this by leaving `lockedBody` unchanged in
`ChangeView` and requiring `Propose` to match a nonempty lock.

**Theorem 1 (agreement).** Two valid CommitQCs in one height context cannot
certify different bodies.

Same-view conflict contradicts Lemma 1. For different views, order the
certificates by view and select the earliest conflicting later certificate.
Its PrepareQC contradicts Lemma 4. Same-view fallback is irrelevant to this
ordering by Lemma 3.

**Corollary 1 (chain-prefix safety).** If successor height contexts are created
only from a durably applied revision-4 CommitQC and its exact canonical body,
then finalized histories of honest validators are prefix comparable.

Theorem 1 gives one certified body per height context. Hash-bound parent and
height-context construction then gives the induction step. A validator may
lag, but finalized-output repair for an older height cannot authorize a
different successor.

### Conditional liveness after GST

The liveness claim is conditional, as required by FLP. Assume that after GST:

1. at least `q = n - f` honest validators remain responsive;
2. authenticated message delivery, timer service, full-body reconstruction,
   durable writes, signatures, deterministic validation, and application
   complete within finite bounds;
3. retransmission and view-change deadlines eventually exceed those bounds;
4. an honest leader can recover any certified locked body from honest signers;
5. enough honest members of every unfinished predecessor lane committee remain
   responsive to meet its frozen descriptor threshold until exact durable lane
   completion, even if those members are absent from the successor global
   roster; and
6. a correct proxy tail that receives `q` phase votes durably forms and
   disseminates the corresponding QC.

**Lemma 5 (fallback progress with an honest tail).** In a view with an honest
leader and honest proxy tail, the height either completes on the Set A fast
path or completes after same-view fallback.

Set A has exactly `q` members, so the fast path is intentionally optimistic:
one withholding or unavailable Set A member may prevent its QC. At the first
retransmission deadline, fallback makes every validator eligible for body
recovery and voting without changing the proposal. There are exactly `q`
honest validators in the worst case. After GST they all obtain and validate
the full body, and their `q` Prepare votes, followed by their `q` Commit votes,
reach the honest proxy tail.

**Lemma 6 (eventual usable view).** Repeated certified view changes eventually
select a view with both an honest leader and an honest proxy tail.

Timeout votes do not depend on the proxy tail, so the `q` honest validators can
certify departure from a stalled view. Across one full cyclic rotation, at
most `f` views place a Byzantine validator in the leader position and at most
`f` place one in the proxy-tail position. Their union excludes at most `2f`
of the `3f + 1` views, leaving at least `f + 1` views with both roles honest.
Certified view entry resets fallback, after which that view receives its own
fast-path attempt and, if needed, same-view fallback.

**Theorem 2 (conditional per-height termination).** Under the post-GST
assumptions, every active height eventually obtains a CommitQC, and every
responsive honest validator eventually recovers, validates, durably applies
the exact body, and activates its successor height.

Failed views either decide or collect `q` timeout votes and rotate. Lemma 6
eventually supplies a usable view; Lemma 5 completes both voting phases there.
The full-body gate provides the body needed for global application. Successor
activation additionally depends on the finalized predecessor completing its
lane-durability preflight: late canonical ownership is rehydrated, bounded
historical recovery is serviced, and every winning current-height lane has an
exact Kura certificate plus its ordinary application receipt or autonomous
durable record. Incomplete lane progress remains active-height-owned; the
implementation has no volatile successor-owner alternative. Only then can the
complete durable lane authority drain and seal exact output and transfer the
remaining successor-owned sidecar state.
Those lane obligations retain their predecessor descriptor and committee. A
removed configured validator is an observer for successor global consensus and
has no successor vote, while remaining eligible for a lane vote only when that
exact frozen descriptor explicitly names it. The same rule applies to older
descriptors and independently pinned current-height Nexus lane descriptors;
neither borrows successor-global authority. During applied-height handoff, an
already completed earlier-height lane output may be retired only after Kura
independently rereads its exact certificate and application receipt (or the
record-backed autonomous equivalent).

This theorem does not cover a permanent partition, more than `f` Byzantine or
unresponsive validators, nonterminating local work, exhausted finite counters,
or a scheduler that never services an enabled action. Finalized-output and
lane-durability repair remain required, bounded, retryable predecessor work;
they are not discarded to make successor activation appear live.

### Revision-4 model status

[`SumeragiV2Revision4.tla`](SumeragiV2Revision4.tla) encodes the exact committee
geometry, one finite cyclic A/B rotation, same-view fallback, fallback reset
on `q`-certified `ChangeView`, full-body voting gates, durable sign-once state,
decision agreement, and successor activation independent of finalized-output
debt. Its routing state additionally records these production-facing
obligations:

1. a proposal manifest targets the whole frozen committee;
2. the first body/chunk occurrence targets exactly Set A;
3. same-view fallback expands body/chunk targets to the whole committee;
4. Prepare and Commit votes target only the current proxy tail; and
5. each timeout vote uses committee-wide fanout, so forming a timeout
   certificate does not depend on the proxy-tail route.

[`SumeragiV2Revision4.cfg`](SumeragiV2Revision4.cfg) instantiates `n = 4`,
`f = 1`, `q = 3`, two candidate bodies, and one faulty validator for exhaustive
invariant checking. The finite `Views = 0..(n - 1)` horizon is one complete
role rotation; unlike the former unbounded natural-valued executable view, it
has a finite state graph which TLC can exhaust.

[`SumeragiV2Revision4AdversarialSafety.tla`](SumeragiV2Revision4AdversarialSafety.tla)
separately removes the main compact model's single-proposal and retained-lock
shortcuts from the agreement search. Its bounded round contains four
validators, quorum three, two independent candidate bodies, and one Byzantine
validator which may vote for both. Each honest validator may receive both full
bodies but its durable Commit intent permits at most one vote. Vote, QC, and
decision actions remain enabled after the first QC, so TLC continues searching
for a second, conflicting QC and decision instead of obtaining agreement by
terminating progress. The exhaustive
[`SumeragiV2Revision4AdversarialSafety.cfg`](SumeragiV2Revision4AdversarialSafety.cfg)
checks both conflicting-CommitQC unreachability and decision agreement.

[`SumeragiV2Revision4Liveness.cfg`](SumeragiV2Revision4Liveness.cfg) checks the
same finite geometry with `PostGSTSpec`. That specification makes the
conditional partial-synchrony premises executable: while the current leader
and proxy tail are both honest, the post-GST transition relation suppresses
timeout-certified departure, representing deadlines longer than the finite
service bound. Weak fairness is stated explicitly for honest-leader proposal,
honest full-body service, fallback, honest Prepare and Commit service,
honest-tail QC and decision formation, honest timeout service and certified
view change, exact local decided-body recovery, application, and successor
activation. `RepairFinalizedOutput` is deliberately absent from the fairness
set and from the activation guard.

The temporal configuration checks both:

```text
ConditionalPostGSTProgress
FinalizedOutputDebtDoesNotBlockSuccessor
```

The first reaches decision, local application, and successor activation from
the initial height within the fair post-GST model. The second is a leads-to
property from applied state with outstanding finalized-output debt to an
active successor. `NonblockingSuccessorActivation` remains the separate
enabledness invariant.

`CompleteFullBody` and `RecoverDecidedFullBody` atomically abstract RS16
reconstruction, hash checking, durability, and validation. The compact model
also does not generate the complete set of arbitrary Byzantine network actions
or establish a source-level refinement. Consequently TLC success is bounded
invariant and conditional temporal evidence, not by itself a TLAPS proof of
the paper theorem, a proof of production refinement, or unconditional
liveness.

## Revision-3 archive (historical)

Everything below this heading describes the retired weighted-vote
revision-3 transition relation and its proof ledger. It is preserved as
historical proof-engineering material. It is not revision-4 release evidence,
and its count-and-power assumptions must not be imported into the current
protocol.

This note gives the deductive protocol argument corresponding to
`SumeragiV2Inductive.tla` and `iroha_sumeragi_core::Reducer`. It deliberately separates
the paper argument from its mechanization status: the lemmas below explain the
conditional target, while `SumeragiV2Proofs.tla`, `verus_proofs.rs`, and
`proof_coverage.json` record what is actually machine checked. A theorem is not
recorded as mechanically discharged until the relevant TLAPS or Verus command
succeeds.

### Definitions and assumptions

For one frozen height context let `V` be the voting roster, `n = |V|`, `P` the
total voting power, and `w(S)` the power of signer set `S`. A dual quorum `Q`
satisfies both `3|Q| > 2n` and `3w(Q) > 2P`. The Byzantine set `B` satisfies
`3|B| < n` and `3w(B) < P`.

Every Vote and QC signature binds the chain, protocol, height-context ID,
certification round, immutable `proposal_round`, phase, subject, and execution
commitment through its canonical preimage. Both Prepare and Commit evidence
require `proposal_round == round`; split-round Commit evidence is rejected.
The reducer lifecycle owner
`(height, view, generation)` is separate local authorization and is not signed
as either wire round. Correct validators obey the durable rules enforced by the
reducer and WAL:

1. they sign at most one Prepare and one Commit per `(context, view, phase)`;
2. they store and deterministically validate the exact body before persisting
   and signing Prepare;
3. they atomically persist a PrepareQC lock and Commit intent before signing
   Commit; the PrepareQC, Commit intent, Vote, and CommitQC all name one exact
   proposal/certification round;
4. a durable timeout intent prevents later Proposal or Prepare signing and
   creation of a new Commit intent in that closed view; after a TC durably
   promotes its selected PrepareQC to the exact active lock, an already-durable
   Commit for that old same-round origin may be re-signed while it remains the
   exact active lock, but a node without that intent must wait for an unchanged
   later-view re-proposal, deterministic validation, and a new same-round
   PrepareQC before signing Commit;
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
successor-activation fairness. Revision 4 has no legacy decoder premise: a
Vote, QC, status record, or finality artifact without its canonical signed
proposal origin is rejected.

### Quorum lemmas

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

### Validity, availability, and locks

**Lemma 4 (external validity and decided-body availability).** Every PrepareQC
certifies a deterministically valid body that remains available from a correct
signer; consequently every CommitQC does too.

A PrepareQC contains a dual quorum and therefore at least one correct signer.
By rule 2, each correct Prepare signer had the exact hash-matching body durably
stored and validated at the Prepare `proposal_round` before releasing its
signature. Correct signers retain and serve that body. A Commit vote requires
the same-round locked proposal, validated body, and PrepareQC, so a CommitQC
inherits the claim. An unchanged later-view re-proposal creates a new
same-round manifest and validation authority for the same subject; it never
relabels an old receipt or produces split-round evidence. Thus a node that
records a decision without the body can fetch the exact certified round from
certified sources; hash and origin comparison prevent substitution.

**Lemma 5 (lock monotonicity).** For a fixed height, a correct validator's lock
rank never decreases, and its locked subject changes only at a strictly higher
rank.

`LockAndCommit` rejects a lower PrepareQC and rejects an equal-rank different
subject. `InstallTimeout` retains the existing lock unless the TC selects a
strictly higher PrepareQC. All such changes are complete WAL frames. Replay
applies the same checks in sequence, while a height transition creates a new
context rather than mutating the old height's lock.

### Certified timeout protection

**Lemma 6 (a formed TC protects every still-formable old CommitQC and its safe
re-proposal).** Consider a subject `x` whose same-round durable Commit-intent
signers form a dual quorum `C` at round `r`. Let a TC for a view `v >= r` have
signer union `T`.

By the quorum lemmas, `C ∩ T` contains a correct validator `h`. If `h`'s
timeout vote strictly protects its Commit intent, the TC's maximum semantic
high-QC selection lifts that local fact to direct protection of `(r, x)`: its
selected rank is at least `r`, and equality binds subject `x`. Ordinary
Commit-intent growth is timeout-fenced, so no historical split-round exception
can appear after the closed view. The timeout signer sets are disjoint and
their union is the quorum `T`, so grouping does not change the intersection
argument.

An old CommitQC already formed before the TC remains directly decisive.
An already-durable old-round Commit intent may resume without changing its
signed round. If that quorum does not complete, the TC-selected subject remains
the safe value for a later responsive leader, which may re-propose the
unchanged body under a new same-round origin. The later PrepareQC and CommitQC
then both name that new round. The grouped-timeout set kernel is machine-checked
for intents already present at timeout. The temporal composition from
protected old-round work to either direct Commit or safe later re-proposal is
recorded `tlaps_proved`. Release still requires a fresh strict run over the
final source closure.

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

### Agreement and chain prefix

**Theorem 1 (agreement).** Two valid CommitQCs in one height context cannot
certify different subjects.

Assume for contradiction that `CommitQC(x, r)` and `CommitQC(y, s)` exist with
`x != y`. Each CommitQC has an underlying PrepareQC in the same exact round.
Order the pair by round and choose a counterexample with the smallest later
round `s`, taking `r <= s`. If `r = s`, the underlying PrepareQCs conflict in
one round, contradicting Lemma 3. The `x` CommitQC gives a dual quorum of
durable locks on `x` at rank `r`. Lemma 6 preserves `x` as the safe value until
a strictly higher certified Prepare legitimately supersedes it.

For a correct validator from the intersection of the earlier Commit quorum and
the Prepare quorum at round `s` underlying `CommitQC(y, s)` to Prepare `y`,
its justification must carry a PrepareQC for `y` strictly above its `x` lock.
Select the first
such conflicting PrepareQC. Its own correct intersection validator can change
from `x` only using an even earlier strictly higher conflicting PrepareQC.
That is a smaller conflicting proposal view above `r`, contradicting minimality.
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

`SumeragiV2ChainReceiptAgreementProofs` isolates the corresponding exact
per-slot model claim. It includes Decision and Application receipts at
`context.height + 1`, including the terminal `MaxHeight + 1` slot, and derives
agreement from joined-source ownership plus the joined Core instance's
`DecisionAgreement`. Canonical prefix history (`decidedAt[1..h]`) identifies
the unique joined source context, but neither `decidedAt[h+1]` nor
`CanonicalCommitForSlot` supplies current-slot subject equality.
`IndexedFreshReceiptActionHasProductExtension` separately proves that the
chain product admits every fresh Core receipt step under that independently
established agreement, so the product relation is not filtering conflicts to
manufacture the result. The script is SANY-clean, but it has no fresh strict
TLAPS receipt. Consequently
`IndexedChainSpecEstablishesExactPerSlotReceiptAgreement` remains a
source-bound support theorem rather than independent proof evidence.

### Crash/restart and reconfiguration

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
exact durable same-round vote whose `proposal_round`, certification round, and
subject equal the active lock. A later lifecycle view does not retarget that
vote. Replay cannot authorize an unlocked historical Commit intent.

The source-refinement guard additionally requires the production WAL, its
serviced-candidate sibling, and its leader-wire sibling to retain one opened
post-open directory identity and use bounded descriptor-relative operations.
The two sibling authorities are distinct, move-only, and one-shot. WAL append
revalidates its leaf before write and after synchronization; adjacent atomic
publication revalidates the promoted leaf after directory synchronization.
Rename and final-component symlink substitutions therefore fail closed after
open. This remains an operating-system durability contract rather than a TLA+
theorem, and it does not yet authenticate pre-open path ancestry to an opened
Kura-root handle. On non-Unix targets only basic WAL I/O keeps the legacy path
fallback; the two adjacent-store authority mints fail closed before changing
their one-shot state.

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

The separate `SumeragiV2TerminalIngressLifecycleProofs` model states only
process-lifetime suffix safety: the set
`{TerminalReadOnly, TerminalRetired}` is forward-closed,
`TerminalRetired` is individually absorbing, the history-service owner exists
exactly in the read-only mode, owner exit atomically retires detached and
ingress owners without increasing the successful-admission count, and no
successful admission follows owner loss. Restart creates a fresh instance,
and no eventual-exit fairness or Rust refinement is claimed. The script is
SANY-clean but has no fresh strict TLAPS evidence, so
`TerminalIngressProcessLifetimeAbsorbencyObligation` remains source-bound
support rather than independent proof evidence. It does not discharge the
terminal Rust trace refinement.

### Conditional liveness after GST

Assume after GST that the responsive correct voters independently meet both
strict count and power thresholds; per-source authenticated transport is
serviced within its declared bound; and every non-crashing responsive
validator participating in the active-height or exact historical-recovery
corridor has an advancing local monotonic clock, regains a serialized height-
runner turn within the declared bound after every finite wait, and completes
admitted fsync, signature, body-transfer, deterministic-validation, proposal-
construction, and application work within finite declared bounds. Also assume
that the complete successful-round rank is representable below the maximum
runtime duration. The immutable view-zero timeout is positive and view `v`
receives `min(base * (v + 1), 10 * base)`, so the model requires the complete
post-GST service rank to fit below that finite protocol ceiling.
`AsyncProductionTimingInstantiation` records the exact ten-base production
binding, and `ProductionAdequateViewTimeoutExists` specializes the adequate-view
theorem under that binding. The finite TLC configurations leave the ceiling
symbolic and larger because their service budget counts abstract actions, not
milliseconds; those searches validate the conditional transition argument,
not the operational claim that a particular deployment meets the ten-base
service premise.

The claim is conditional and per height: after GST, with a representative
roster of at least four voting peers, a responsive dual quorum, and
deterministic terminating local work, every height eventually decides and
every responsive validator eventually applies it. No claim covers an
unbounded partition, loss of either quorum threshold, an undersized production
fixture, or local work that never returns.
The model's per-node service-deadline vector is ghost bookkeeping for this
trusted runtime contract, not shared production state. Production source seals
establish the narrower structural facts—one serialized height loop, bounded
service batches, watchdog polling, and finite `IDLE_POLL` waits—but do not turn
host scheduling or operation latency into a code-proved proposition.
The production source seal additionally checks the consuming lifecycle activation
state: fail-stop admission precedes clock arming, status projection,
observer installation, exact ingress/status publication, and readiness release;
CompleteTip publication consumes its retained predecessor retirement. This is
paired with a source-sealed consuming finalization chain which closes readiness
and durable leader-wire ingress, joins exact Kura finality to adapter closure, retains
the safety WAL through the existing durable output handoff, retires it only
after that handoff, then refreshes Serve state and publishes
all-row LedgerV1 retirement through opaque coordinator-owning tokens before
clean shutdown. This is not a new mechanized liveness theorem, and the
serialized runner mints and drives these states only through the atomic cutover.

**Lemma 7 (generation- and consumer-scoped locked-vote delivery).** Clearing a
volatile vote pool cannot orphan the exact durable locked Commit intent.

Authenticated vote history is immutable and distinct from the volatile receipt
pool. An exact vote already present in that pool is suppressed. Persisting a TC
clears only the installing node's pool and advances its reducer generation
when the checked increment is available. Exhaustion is fail-stop liveness debt,
not a temporal premise. At that acknowledgement boundary,
the reducer compares the resulting lock with its durable Commit intents. An
exact proposal-round/subject match may queue that old same-round intent for
re-signing. A newly TC-promoted lock without an intent never signs merely from
installation. Its exact body recovery keeps the safe value available for a
later unchanged re-proposal, whose new same-round validation and PrepareQC may
authorize a new Commit intent.
Completing the signature inserts the vote directly into the installing node's
exact-round pool, then broadcasts only to the other voters because the P2P
broadcast does not deliver to its sender.

Every Commit vote, including one from the current view, is admissible only when
its `proposal_round` equal its certification round and its subject equal the
matching durable Prepare lock. A premature current-view Commit is a recoverable
ignore. The matching `LockAndCommit` acknowledgement makes that exact
same-round intent durable before releasing the local Commit signature. The
adapter advances the locked-Commit consumer epoch at that boundary, so an
ignored exact vote may enter once after authority exists.

Retained signed Commit control remains the peer-delivery source, but cannot by
itself witness local reconstruction because broadcast excludes its sender. A
retransmitted Commit whose proposal origin matches the exact lock is classified as
protected progress and consumes each peer's new pool epoch exactly once.
Prepare votes, unrelated proposal origins, and superseded Commit finality votes
remain inadmissible. If a crash
happened after the Commit intent became durable but before its signature or
broadcast, `ResumeVote` reconstructs that same exact locked Commit even after
the timeout or TC. Replay never retargets it to another round. If old-round
Commit reconstruction does not complete a quorum, the retained bytes may be
re-proposed unchanged in a later view; that path creates a new same-round
Prepare and Commit authority instead of mutating the old intent.

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
evidence. The wider causal/ready/I/O projection remains part of the promoted
production-refinement seam, whose fresh exact-source cross-tool release
evidence remains required. A pinned mutation demonstrates the old
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
classes, one `SidecarTopologyProgress` Lane reservation, and one
`SidecarReplyControl` Lane reservation per frozen target, plus separate shared
capacity. A deterministic matching gives each retained fanout at most one
unique frozen target/class/kind reservation and is recomputed after partial
progress. Parked ordinary same-target output and saturated shared capacity
cannot consume the topology-progress or reply-control opportunity. Alternate
authenticated Hint routes coalesce without multiplying reservations.
Non-roster replies and repeated target/class/kind output therefore cannot spend
unopened validator reservations. Concretely, a reliable
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

Local admission alternates producer-completion and causal-work sources. The
current proof source gives an Init/full-action induction for
`asyncCausalAdmissionOwed[node] => CausalQueueNonempty(node)` on `AsyncSpecAt`
traces; `AsyncStrongTypeInvariant` alone does not imply this fact. Reachable
causal-debt replenishment therefore reduces to the two local metadata setters,
and owed admissible causal work receives deterministic preference under fair
runner service. This does not bound how often distinct causal heads can
replenish the debt, so it is not yet a temporal convergence proof.

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
provenance; authenticated junk receives no temporal promise. The composite
rank obligation is recorded `tlaps_proved`, but fresh exact-source strict
release evidence remains required. A compact Stage-6 mutation pins
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
Completion-capacity product rank by themselves. The promoted progress-witness
refinement still requires fresh cross-tool release evidence.
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
during the round's timeout. A selected PrepareQC installs its immutable safe
subject on the responsive quorum. Existing same-round Commit intents may still
complete the old-round CommitQC. Otherwise a later responsive leader
re-proposes the retained body unchanged under a new same-round origin; the
responsive quorum obtains, stores, and validates that exact manifest, forms
and disseminates its same-round PrepareQC, atomically locks and persists
same-round Commit intents, and forms CommitQC within the timeout. Each node
persists the decision, fetches the certified manifest and
chunks if needed, reconstructs and validates the exact body, applies it, and
advances its own height. None of these local transitions waits for every other
correct node.

The mechanization records the missing step under the historically retained symbol
`LockedBodyReproposalProgressObligation`. Its first-release obligation is that
a stable available retained lock must eventually commit in its old round, be
re-proposed unchanged under a later new same-round origin, or be legitimately
decided or superseded by a higher certified Prepare lock. The symbol must pass
the fresh strict proof for release; it is recorded `tlaps_proved`, and view
movement or byte retention alone is not an outcome.
`SumeragiV2LockedBodyProposalActionProofs` supplies only action-frame
preservation and exact exit helpers for this corridor. It owns no ledger
obligation, and those helper theorems neither prove nor promote the temporal
locked-body target.

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
representative roster of at least four voting peers, a responsive dual quorum,
deterministic terminating validation/fsync/application, and a service bound
below the finite maximum view timeout, deterministic
consensus cannot guarantee progress. The paper argument derives
height progress under those premises when valid transaction, autonomous,
internal, or state-derived time-trigger work exists; it
does not prove transaction inclusion or censorship fairness. The archived
ledger's true completion flag does not make this a revision-4 deductive
liveness proof; revision-4 release acceptance still requires the separate
bounded TLC corridor and same-source release evidence.

### Exact reply-writer boundary

`SumeragiV2ReplyWriterDeadline.tla` is an orthogonal executable abstraction of
one exact-reply actor occurrence. It acquires one absolute adaptively scaled
deadline at first actor dispatch, before bounded peer-writer admission.
Full-queue retry preserves that deadline. Only `PublishPeerWriterFlush` can
establish `writerFlushObserved`, the witnessed observation is monotonic, and an
exact reply is flush-ready only when both the ready bit and that immutable
witness are present. The receipt separately retains the admission timeout
attempt, and the fixed `PollPeerWriterFlush` branch advances only when that
attempt equals the target's current attempt. Timeout therefore cannot
fabricate a successful receipt, and a ready witnessed receipt excludes
timeout, outer-actor closure, and stale route retirement while surviving
connection replacement. Timeout can retire only its accepting connection
occurrence; topology output does not acquire the exact-reply deadline. The
safety induction still enumerates all 15 fixed `Next` branches explicitly.

The proof module keeps local actor termination separate from environmental
writer progress. `ReplyWriterDeadlineSpec` assumes weak fairness for actor
dispatch, the monotone timer, expiry, and receipt polling, but no fairness for
writer-flush publication. `ReplyWriterDeadlineModelObligation` is the
deductive safety and local-termination target. The conditional cursor proof
script consumes the explicit `ResponsiveWriterReceiptAssumption`. The
SANY-clean `ResponsiveStrongFairnessToReceiptResidual` proof script derives
that assumption by retaining outstanding ownership, using weak fairness to
reconnect and first-dispatch parked work, and using strong fairness to admit
and publish across fragmented `WriterPending` intervals.
`ResponsiveReplyWriterCursorLivenessFromStrongFairness` composes that bridge
with the conditional cursor theorem. These source proof bodies are deductively
classified support for the promoted production progress refinement; they are
not independent ledger rows. A fresh strict TLAPS run plus Verus and derived
trace evidence over the final dependency closure remains required for release.
The termination statement is qualitative, not a fixed operational wall-clock
SLA: finite `u8`/`Duration` scaling can produce very long later deadlines. A
recovered responsive writer may nevertheless publish and be polled immediately
before its current deadline expires.

The mutation matrix has eleven state-invariant counterexamples plus one
witness-erasure counterexample checked by an explicit monotonic action
property. The new wrong-attempt case first reaches attempt one, then publishes
an attempt-zero receipt. These bounded checks and the production source seals
constrain the intended abstraction, but they do not prove a Rust-to-TLA
semantic refinement theorem. The earlier 276/276 strict TLAPS receipt predates
the final witness origin, monotonicity, receipt-attempt, and strong-fairness
theorems; a fresh strict run against the current proof source remains pending.

### Typed rollover handoff boundary

`SumeragiV2TypedRolloverHandoff.tla` isolates one changed- or same-roster
handoff with two initial exact-output workers, a move-only service/transport
owner pair, an empty-corridor seal, an exact predecessor receipt, an immediate
successor, retry preservation, and late-callback isolation. Production has
three changed-roster authorities. The ordinary path requires authenticated
predecessor terminality. A move-only handoff issued after the exact-output
corridor is durably sealed, or a private fence issued after complete semantic
V3 restoration, may supersede active predecessor responder state. Both forced
paths require the lifecycle journal, commit the checked successor generation
and empty responder projection before clearing memory, retire responder/output
debt, and do not manufacture requester-authenticated close prefixes.
Same-roster rehydration preserves responder generation and ownership, including
the retained current chunk; a new requester against a full same-roster table
receives fail-atomic `Capacity`. Counter overflow and unauthorized active-state
replacement likewise fail without mutation. No Rust-to-TLA refinement for
that production relation is claimed.

The model records an inductive control partition over the reachable healthy
handoff stages and keeps `NoRolloverFailure` explicit. That structure is a
specification aid, not proof evidence for compaction or eventual restart. Its
conditional liveness declaration is not an independent ledger row; final
persistence and rotating-leader closure are carried by the promoted consumers
and still require fresh release evidence.

The conditional local handoff result is not a proof of the final compaction
relation. It begins after finality validation and does not cover network
delivery, reply-writer flush, recovery after a fail-stop rejection, repeated
rollover, or a Rust-to-TLA semantic refinement. Historical bounded and strict
receipts predate the authority-gated force fence, V3 two-slot persistence,
bootstrap adoption, validation-before-cleanup ordering, and the root trust
boundary. Both typed-handoff declarations remain source-bound support leaves
consumed by the promoted top-level successor-activation production-refinement
target; they are not independent ledger rows. No filesystem refinement claim follows from
the abstract model.

### Mechanization ledger

`proof_coverage.json` is the checked-in status declaration, not independent
proof authority. The checker binds it to exact theorem declarations and
structural dependencies. The release runner checks the ordered deductive
modules with pinned TLAPM commit `3ab43c7`, requires a positive
all-obligations-proved result from each backend run, and writes
source/log/tool-bound evidence under `target/formal/sumeragi_v2/`. Checked-in
backend counts are intentionally prohibited because they become stale as
proofs change.

The reviewed top-level inventory contains exactly 54 obligations: 44
`tlaps_proved`, 3 `cross_tool_proved`, 6 `trusted_contract`, and 1
`out_of_scope`, with no `specified_unproved` rows. Sixteen source-bound
proof/evidence decomposition leaves remain checked transitively through those
top-level obligations and are not additional ledger rows.
`machine_checked_completion` is true for this reviewed revision-4 status
inventory. It is not a deductive proof of revision 4, and the flag is not
independent proof authority: release mode still requires fresh strict TLAPS,
pinned Verus, derived cross-tool evidence, and the separate revision-4
exact-cardinality corridor bound to the exact source and ledger.

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
`EffectiveLockBodyAcquisitionProductionRefinementObligation`, recorded
`cross_tool_proved` subject to fresh release evidence. The exact enabled-`RunNode` result is only a scheduler
lemma: post-GST deadlock freedom requires an enabled `AsyncNext` step that grows
current-height protocol evidence, strictly consumes a concrete deadline debt,
or decreases/exits a protected candidate or Serve-occurrence rank. Repeated
clock or view-change steps alone do not satisfy that productive obligation.
Stage-2/3/6, packet-admission, and zero-deadline cases remain explicit branches
of the reviewed rank and deadlock decompositions; they are not hidden behind
an enabled-action facade. The executable async model retains an exact deferred
handoff across a Busy retry: matching work clears only its own token,
Completion work may still terminate the current Busy owner, and foreign
non-Completion work cannot create a successor owner. The Stage-2 and Stage-3
leaves close timer/retransmit rearm, finite cursor, exact service, same-node
`PostGstRunNode`, and all-other-action UNLESS cases. Together with Stage 6 and
the Completion product rank they feed the ledgered `tlaps_proved` aggregate
protected-service-rank theorem; all of those final-source dependencies still
require fresh strict revalidation after source drift. Runner preservation and
the dependent async type closure are also ledgered proved. Deadlock and
starvation have composed source proof bodies; starvation consumes
protected-service-rank progress and the finite-runner episode closure. Both are
recorded `tlaps_proved`, as are the stable-suffix liveness declarations and
concrete genesis first-successor handoff. Their final dependency cones still
require fresh strict TLAPS for release. The application boundary is isolated at
`AsyncTemporalClosureApplicationCompletionProgressObligation`, the exact
per-responsive-node decision-to-application pipeline. Its source proof
composes the five off-scheduler Decision leaves with protected finite-runner
service. The aggregate application theorem adds application monotonicity, a
frozen responsive voter set, and finite validator-prefix induction without
introducing a global application barrier. The application row is recorded
`tlaps_proved`; release still requires fresh strict verification. The chain
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
premises. The remaining exact historical closure is registered as exactly
three temporal support theorems with source proof bodies:
`IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation`,
`IndexedHistoricalCertificateRankProgressResidualObligation`,
and `IndexedHistoricalDecisionRankProgressResidualObligation`. The separate
`IndexedHistoricalDecisionStageOwnershipResidualObligation` is now a proved
safety support theorem: `IndexedChainSpec` establishes the composition,
Decision-witness, and recovery-dormancy invariants that make its residual
empty. Downstream composition derives that ownership property and consumes the
three temporal theorems. All four historical leaves are deductively classified
support rather than independent ledger rows; checked-in promotion is not fresh
strict evidence. The release-facing height theorem has a source proof body and
is recorded `tlaps_proved`; release still requires the whole theorem to pass a
fresh pinned strict proof
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
ranking counter. Progress preservation now also consumes the chain-epoch
invariant: an exact durable parent application derives that its canonical next
context is admissible from the typed node context, certified valid-subject
prefix, and no-outrun clauses before a failure/restart owner can be retained.
This closes the former arbitrary-witness hole in the local protocol invariant;
the helper, caller chain, and fail-closed mutations are SANY-clean, but still
await strict TLAPS. `IndexedChainSpec` includes an explicit eventual
failure-free suffix premise. It makes
no progress claim for an honest validator outside `Responsive`, which may stop
with pre-GST local work still queued. Its release-facing theorem contains a
candidate suffix-local weak-fairness proof, well-founded rank composition, and
an explicit temporal suffix lift. Its promoted status is accepted for release
only when strict TLAPS discharges that proof. The latter is deliberately not a
theorem about model state alone. Its exact statement conjuncts
`ProductionSuccessorAndExactRecoveryTraceRefinement` with the indexed model
invariant. That source predicate contains six unassigned booleans for Applied
publication, Recovered publication, split startup failure/restart,
authenticated historical-certificate import, the ordinary historical body
pipeline, and the authenticated terminal Apply boundary before successor
construction. The sixth production kernel has no `MaxHeight` input: it proves
that exact context, receipt, artifact, block, and durable-predecessor identity
agree while no successor activation is pending. `MaxHeight` remains only a
finite-horizon projection. A valid checked Running failure latches the local
snapshot; an invalid projection preserves it, while the runner's already
latched process output guard remains authoritative in public status.
Historical body service requires frozen-roster membership, not participation
in the old QC; the QC authenticates the subject and the archive signs the
response. Source token/order checks,
adversarial production tests, stale-token mutation tests, and source-manifest
binding constrain those claims, but do not prove any of them. Consequently the
already proved abstract successor invariant cannot discharge this production
seam. The row is recorded `cross_tool_proved`, and release acceptance still
requires fresh machine-checked cross-tool trace evidence for every claim.
Historical recovery is an exact Async
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
relation. Its concrete runner mapping remains part of the cross-tool
production-refinement seam.

In the abstract indexed model, nonterminal application queues successor
startup and does not join; only exact
Applied or Recovered publication joins. Production's earlier internal State
application maps to that abstract Applied boundary only after canonical lane
completion. If block sync installs that canonical ownership after adapter
construction, production rehydrates its exact bounded proposal while the
finalized predecessor is still active, retransmits it as the certificate
recovery source, and refuses to mint rollover authority until the recovered
certificate and application receipt are durable. That delayed mapping is part
of the promoted cross-tool target and still requires fresh exact-source
release evidence. The concrete effect executor now coalesces
every exact in-flight `Apply` rediscovery and retains the typed Kura finality
receipt, exact artifact, and original reducer tag as a completion tombstone.
An exact post-drain retry is absorbed; tag, subject, context, or CommitQC drift
fails closed. This closes old-stage recreation in source but does not supply
the fresh application or cross-tool release evidence required by the promoted
statuses.
At the enclosing adapter boundary, a zero-to-one semantic candidate publishes
its immutable owner, while a one-to-one retry must present that exact owner.
A different owner for the same semantic lifecycle is rejected before the
effect-to-candidate trace projection is constructed. The separate physical
Fetch/Store/Validate lineage may retain a stronger carrier only after its
distinct phase/commitment identity passes the typed monotone authority gate and
the adopted lineage is re-projected. These source and mutation checks do not
supply the fresh cross-tool release evidence required by the promoted status.
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
rule even outside release mode. The theorem-free structural shard
`SumeragiV2AsyncOutstandingLivenessDebt` and exactly two theorem-bearing
scratch modules are non-release inventory.
`SumeragiV2HistoricalLockedBodyRecoveryBridgeScratch` and
`SumeragiV2ProgressWitnessCrossToolScratch` each recheck one authoritative
bridge. None is release evidence and none can promote a ledger entry.
`SumeragiV2LockedBodyProposalActionProofs` remains helper-only despite being in
the release module order; it owns no ledger target. The ledger also prohibits
promoting async type
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
abstract results. The 54-entry top-level ledger contains 44 `tlaps_proved`, 3
`cross_tool_proved`, 6 `trusted_contract`, and 1 `out_of_scope`, with no
`specified_unproved` rows and machine-checked completion true. Sixteen
decomposition declarations remain source-bound and transitively checked through
their reviewed top-level consumers rather than appearing as extra claims. Fresh
exact-source release evidence remains mandatory.
The aggregate temporal closure now gives
`AdequateLeaderExactClosureResidualObligation` and
`ExactDecisionOffSchedulerResidualConvergenceObligation` pinned source proof
bodies and classifies them as deductively proved support leaves. They are not
independent ledger rows; fresh strict verification and the remaining consumer
dependencies are still required before release accepts the promoted rotating-
leader and application-liveness rows.

The adequate-leader residual is target-local rather than aggregate: another
validator's Decision is not terminal for the indexed target. Its occurrence
rank counts every distinct target/leader owner at the frozen semantic rank,
preventing one serviced owner from hiding another. Equal-count replacement
and count-increasing replenishment remain explicit non-progress cases and
require a prior finite or coalesced producer argument.

The production lifecycle closes the matching final-retirement race through one
coordinator-owned transaction. A current-height Serve remains in fair ingress
until the lifecycle selector authenticates its exact carrier, and capacity
backpressure returns `CapacityPending` without removing that occurrence. A
successful transaction attests the complete Ready census, claims the durable
ledger row, reserves the exact worker target, and only then commits dequeue.
The serialized proposal runner separately claims and settles `ProducerTurn`;
there is no queue-local Serve gate, barrier, reservation, or producer episode.
Rejected replenishment cannot mint either scheduler or logical lifecycle
ordinals. Digest-refreshed mutations bind the coordinator transaction, the
high-water marks, timeout-owner ordering, strict predecessor service, and the
real timeout-certificate/EnterView suffix. This source refinement does not
count replenishment as progress, add fairness, or promote a ledger row.

The exact-Decision producer audit narrows causal replenishment to reachable
local debt setters; Serve-capacity growth to ordinary or historical request
drain, fresh causal Completion admission, or local Control enqueue; and
priority growth to exact network-claim admission or the same archive's normal,
recovery, or historical runner. Each classification is action-local. The five
exact off-scheduler convergence leaves now have source proof bodies over the
immutable owner, finite prefix, admission/coalescing, and nonphysical response
gate. Their composition does not treat replenishment itself as progress, and
fresh strict TLAPS remains required before release acceptance.
The production mirror likewise keeps one Apply work owner through queued,
active, and completion-pending retries. Once Kura returns the exact typed
finality artifact, that durable completion retains the original reducer tag as
the tombstone for the logical request, so exact periodic rediscovery after
drain remains idempotent even after live tag authority is relinquished. Tag
drift or a different CommitQC remains a fail-closed conflict.

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
ownership invariant through 616,705 generated states, 62,464 distinct states,
and depth 37. The bounded checker retains separate non-timeout-progress and
TimeoutVote ingress reservations, closes the inherited acquisition state, and
uses structurally equivalent finite-search definitions for powerset-valued
production carriers. These are bounded regression witnesses, not deductive
proof and not a reason to promote a ledger entry. Two reply-route
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
productive release target is promoted but still requires fresh strict
evidence. The production
trace replayer and adversarial simulations exercise the exact reducer sources,
while an older pinned Verus receipt proves only the source-linked reducer/WAL
and scheduler kernels that it actually hashed. That receipt predates the
proposal-origin changes and was not rerun for the current source. The
remaining cryptographic, deterministic-execution, operating-system durability,
post-GST transport, and host-service premises are listed explicitly in the
ledger and formal README.

The formal gate also seals a dedicated effect-capacity ownership matrix
consisting of 6 models and 33 configurations. Its runner requires 10 repaired
cases to complete and all 23 mutants to fail at their named invariant or
temporal witness. The 22 unchanged cases retain exact aggregate markers for
131 generated and 130 distinct states; the revised eleven-case certified-request
model pins semantic actions and violations rather than carrying forward stale
state totals. The matrix exercises persisted TimeoutVote-Sign ownership at
capacity two, deterministic Fetch preemption and decided-owner exclusion, fair
non-preemptible retirement, reconstructible full-capacity Fetch rejection, and
bounded retained-effect FIFO behavior. Its certified-request seam separates a
one-entry request bound from two general work slots. Only a `FetchBody`
capacity rejection is retryable. The executor retains that exact
task/authority/lifecycle occurrence as one bounded FIFO owner, without partial
pending-work, request, or transport ownership; drop, substitution, duplication,
overtaking, partial P/Q install, and loss of the existing-owner retry barrier
are separate mutants. An exact authenticated
`CertifiedBodyResponse` with a still-live matching logical request registration
is transport-only, so it may cross retained reducer-effect debt and retire the
blocking request. After the required capacity is released, retrying the
retained FIFO head atomically installs both pending-work and certified-request
owners.
A genuinely new Fetch then drains that successful head; an existing ordinary
Fetch retains the exact head as its completion barrier after the authority
upgrade.
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

A separate source-sealed applied-phase admission matrix contains one model and
six configurations. The TLA+ matrix deliberately covers the surviving
evidence-bearing `BodyStored` phase; five mutants allocate a new post-apply
ordinal, retain a physical owner after apply, coalesce conflicting storage
payload or owner evidence, hide a malformed callback behind stale-tag
coalescing, or admit a well-formed stale callback as current. The seal binds those cases to
`preflight_runtime_command_admission`, both serialized enqueue paths, and the
matching Busy-owner regression. Production validates the complete callback
before treating a stale incarnation as a stutter, reports conflicting storage
payload or owner evidence as fail-closed, and suppresses an exact applied
retry before constructing a tagged command or allocating an admission ordinal.
The source seal separately pins a Rust regression for exact post-apply
suppression of `BodyAvailable` and `BodyStored`; it makes no
conflicting-evidence or Busy-owner claim for `BodyAvailable`. Busy, unapplied
`BodyStored` callbacks retain exactly one serviceable owner. This finite model is mutation
evidence only; it does not establish crash/restart refinement or promote a
proof-ledger obligation.

A separate source-sealed durable Validate lifecycle matrix contains one model
and six configurations. The repaired case joins scheduler-owned Ready-to-Waiting
dispatch to worker execution and the turn driver's exact guarded completion;
retains an exact missing-sidecar registration on the same row and ordinal;
requires the rejected-result output reservation before claim; reconstructs
mandatory replay authority for every modeled origin after restart; and stops
on an ambiguous error after LedgerV1 fsync. Five single-cut mutants break those
properties independently and must fail at their named invariants with their
action-coverage witnesses. The source seal binds the finite states to the real
scheduler, worker, turn-driver, sidecar, recovery, replay-authority, and report
publication seams, and the formal CI gate invokes the runner explicitly. This
bounded TLC mutation evidence neither proves the unbounded runtime nor promotes
a deductive proof-ledger obligation.

A separate source-sealed post-Decision timeout/TC matrix contains one model
and ten configurations. The repaired deterministic trace completes with TLC
status 0; nine single-seam mutants cover `BeginTimeout`, `ResumeTimeout`, the
atomic local receipt turn, `BeginInstallTC`, both receive-pool branches, and
all three causal-successor branches, returning status 12 at their exact named
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
global async type obligation. An independent sentinel keeps the
Rust/Verus-to-TLA durable-owner, scheduler, and application trace mapping
dependent on fresh cross-tool evidence; abstract TLAPS success alone cannot
establish production progress. The matrix is regression evidence only.

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
tip-recovery regressions remain executable regression evidence, not independent
proof of the promoted obligations.

The current pre-network release inventory names 864 tests across 43 Rust
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
a module. Crash-safe response handoff and same-delivery retry after transient
capacity pressure add two more sidecar regressions. The lifecycle snapshot also
binds its complete canonical Norito payload with a typed hash; recovery rejects
canonical bytes carrying a stale digest before interpreting any semantic
floor. The per-source
route-attempt, exact PrepareQC recovery, locked-body reproposal, runner/worker,
sidecar, and daemon closure, plus the certified sidecar control-bucket
regression, yielded the 585-test checkpoint. The unsent-request restoration and
fairness-cursor retry regressions yielded the 588-test checkpoint. The durable
semantic-peer-history regression yielded the 589-test checkpoint. Mechanical
source-to-inventory reconciliation then adds 115 net authoritative-ingress,
merge-sidecar, lane-work, runner, worker, P2P-network, and daemon-relay
changes, yielding the 704-test checkpoint. The runner close-prefix
failed-suffix handoff regression adds one exact name, yielding the current
705-test checkpoint. The routed-Hint and crash-safe V3 lifecycle closure adds
26 exact regressions and retires eight obsolete route-free/V2 selectors,
yielding the 723-test checkpoint. Rejecting replay of a proposal superseded by
a same-round lock adds one exact reducer regression, yielding the 724-test
checkpoint. Preserving that replayed proposal's tag, round, and subject through
runner startup adds one exact regression. Two cross-platform lifecycle V3
crash regressions cover state replacement before directory sync and root
replacement before predecessor cleanup, yielding the 732-test checkpoint.
The foreign-context CommitQC Apply rejection adds one exact `v2_effects`
regression, yielding the current
733-test checkpoint. Five exact-Serve lifecycle regressions cover
Pending/Reserved rollback, shutdown rollback, and route-neutral tombstone
replay, plus cached replay after the singular future-slot barrier, yielding the
738-test checkpoint. Another 35 exact CertifiedServe ingress and worker
regressions bind gate ordering, immutable admission ordinals, frozen
predecessors, coalesced retries, durable restart, terminal replay, owner
replacement, and anti-resurrection behavior. One four-peer leader-wire
lifecycle-store regression binds the full origin/phase/chunk slot product and
restart-stable terminal coalescing, yielding the 774-test checkpoint. Eight
runtime/effect/runner regressions now bind Decision/lock retirement,
same-turn terminal consumption across live and recovery capacity retries, and
fail-closed authenticated semantic-only Coalesce defense. Subsequent source
reconciliation and exact-ingress lifecycle, restart, provenance, and
quarantine regressions bind one logical owner across every physical retry,
yielding the 806-test checkpoint. Seven physical-cut, adapter-capability,
aggregate-rebase, and ineligible-driver regressions yield the 813-test
checkpoint. Five admission/coalescing, Busy pre-runtime ownership, and
reconstructed-chunk terminality regressions yield the 818-test checkpoint.
Thirteen exact admission, retry, tombstone, and high-water regressions yield the
831-test checkpoint. Retiring five obsolete peer-genesis protocol regressions
yields the 826-test checkpoint. Replacing one obsolete restart selector with
its separate raw/coalesced crash boundaries and restoring two implemented
certified-ingress regressions yields the 829-test checkpoint. Autonomous-
lifecycle terminal-outcome and startup-recovery coverage plus the final source
reconciliation yield the 837-test, 39-module checkpoint. Ten unignored
deterministic network simulations cover lossy/offline leaders, symmetric and
asymmetric partitions, current-owner QC redelivery, leader crashes, bounded
corrupted-chunk recovery, WAL-intent replay, and divergent Taira views, yielding
the 847-test, 40-module checkpoint. The source-bound terminal-sweep partition
regression yields the 848-test checkpoint. The late-
passive-Fetch direct-predecessor observation and one-turn RAII reopening
regressions yield the 850-test checkpoint. Seven Native AMX finality-bound
merge-projection regressions yield the 857-test, 40-module checkpoint. Three
Kura recovery regressions and the governance-unlock audit yield the 861-test,
41-module checkpoint. The production-adapter activation guard and two deferred-
canonical-carrier completion regressions produced that historical 864-test,
41-module checkpoint. Retiring the duplicate inline network-simulation rows
yields the historical 856-test, 40-module checkpoint. The exact retired-attempt
accessor, mixed-carrier successor, two-link cold-restart hydration,
noncanonical autonomous-output retirement, and the ordinary plus record-backed
autonomous predecessor-durability regressions yield the
current 864-test, 43-module inventory. The complete source-sealed
pre-network corridor
contains 84 legs. Six source-
sealed command legs and the G-SCALE
runner/validator preflight harden that release corridor.
Wire protocol version 1 uses positive `NonZeroU64` responder generation,
requester epoch, and per-stream semantic sequence coordinates. Canonical
request identity binds the version, those coordinates, payload or reference,
and both peers, excluding only cumulative `closed_through`; a monotonic floor
advance on the same occurrence does not rematerialize output. `GenerationHint`
names the observed/current generations and exact triggering Request or Close
hash while retaining that delivery's authenticated reply route. Alternate
sources remain independent attempts. Each frozen target owns a
`SidecarTopologyProgress` Lane reservation for topology-routed Request/Close
and a separate `SidecarReplyControl` Lane reservation for exact-reply
CloseAck/GenerationHint. Same-target ordinary output and saturated shared
capacity cannot consume either reservation; alternate Hint routes coalesce
without multiplying it.
The canonical progress-mutation runner executes the dedicated
`GenerationEpochFixed` trace as well as the cursor, source-isolation, and close
traces. It persists and installs responder generation two, observes an old
generation request, persists a fresh requester epoch before discarding the old
partial identity, and rejects future-generation input without mutation. TLC
exhausts exactly 7 generated and 7 distinct states at depth 7. This is bounded
regression evidence; it does not promote a deductive obligation. The separate
pipeline trace carries that occurrence through enqueue, persisted hint reset,
successor enqueue, and stale-flush rejection. Its complete graph has exactly
11 generated and 10 distinct states at depth 10. The capacity-overflow trace
then keeps active ownership unchanged across rejected nonterminal compaction,
requester-epoch exhaustion, and responder-generation exhaustion; its graph has
5 generated and 5 distinct states at depth 5. The pipeline companion retains
the pending attachment, exact item, and source-owned occurrence across the same
rejections, exhausting 8 generated and 7 distinct states at depth 7.

The machine-checked claim boundary contains exactly 13 ownership, 9 pipeline,
and 11 asynchronous structural theorems. Current pinned strict waves discharge
15/15, 11/11, and 54/54 backend obligations respectively. They prove coordinate
identities, fail-atomic local transitions, exact V2 action projection, both
product brackets, and both composed-spec projections. Whole-spec induction,
successor isolation, local progress, and the asynchronous temporal product
remain plain support operators with no independent proof evidence. Neither
these bounded traces nor the structural theorems establish network
delivery, rotating-leader progress, or another liveness result.

The sole `MergeSidecarLifecycleSnapshotV3` persists geometry,
`next_stream_epoch`, responder generation, requester streams, unified
server streams, request gates, and its root generation. The unified
server-stream table and gate table are bounded independently of P2P reply-source
capacity. The root is durably published before the state directory as a
generation-zero bootstrap sentinel with no snapshot hash; a surviving
generation-one candidate is semantically validated and rechecked before the
root adopts it. Later snapshots alternate between two state slots: the inactive
slot is fsynced before the independent root marker commits it. A later
pre-marker crash therefore restores the predecessor and a post-marker crash
restores the successor. Restore validates the marker-selected candidate,
rechecks the live pair, and validates known temp artifact types before deleting
a temp or unselected slot; unknown or non-regular artifacts fail closed.
Committed recovery re-syncs the selected state and root-marker directories
before cleanup, and filesystem aliases, including Windows reparse-point files
or directories, fail closed. Native atomic replacement is required on Unix and
Windows, and unsupported directory-sync platforms fail closed. The root marker
is the local trust anchor, so marker replacement/rollback—including restoration
of the bootstrap sentinel—and whole-store rollback are outside the guarantee.
V1/V2 are unsupported.

Ordinary generation advance requires a certified changed roster and
authenticated terminality. A sealed durable handoff or semantically validated
restart may force-fence active predecessor responder state after committing the
empty successor projection, without forging close prefixes. Same-roster
rehydration preserves generation and responder ownership; a new requester
against a full same-roster table rejects without mutation.
The canonical module/test TSV inventory SHA-256 is
`23325cb037bc930c7503986845dbb25891ef80af6f08092533b1e0e1d8233fad`.
The separate source-sealed G-UNIT inventory contains 522 focused tests,
including 316 `iroha_core` tests. Its 523-line
canonical TSV has SHA-256
`e83efb1bd375226d379831d9f6e11c4bd4726fda3293849f0d12349f4b7565ea`;
the sealed Native rows cover exact per-route prevote-byte accounting,
empty/hard-cap/overflow pair geometry, and precommit error classification.
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
geometry inventories five owners per validator, three owners for every one of
the `H` simultaneously materialized authenticated non-validator lanes
(`5N+3H` total), including a roster-origin completion relayed
through an authenticated non-validator hop, and retains the capacity-negative
boundary. It
also adds one four-validator exact PrepareQC count-and-power quorum regression.
The four integration names share a module-filtered leg; the pre-network corridor
now has 84 legs, including the governance-unlock audit module, the autonomous
lifecycle-recovery module, separate
exact data-model status and atomic
lane-certificate decode contracts, two `iroha_config` geometry modules, three P2P
geometry modules, and source-sealed command-success legs. Its finality, offline
compact-QC, and height-context proposal-origin
modules each use a dedicated `iroha_data_model` leg. Its `iroha_p2p` legs use
the crate's empty default feature set; feature-gated QUIC first-packet geometry
tests are not claimed by the 43-module, 84-leg corridor. It
includes
exact completion ownership, body-owner binding and
rebind, rejection of future physical completions, durable-recovery retry to the
latest consumer, byte retirement, three-class production arbitration, the exact
`5N+3H` ingress and `2N+3` deferred partitions, successor activation/recovery,
authenticated exact historical recovery, retained effect-capacity ownership,
post-decision timeout/TC quiescence, and watchdog classification. It also pins
the adapter's maximum flattened persistence macro-step at four effects within
the reducer's eight-effect bound, services at most one Busy-deferred adapter
macro-step per serialized runtime turn, and forbids terminal readiness while
any Completion, Progress, or Normal deferred queue remains nonempty. The
production-default saturation regression fills all 256 certified-request
owners, 639 Normal ingress slots, and the 128-slot reserved Progress increment
while preserving the 256-slot Completion reserve and one separately charged
certified-fence slot, then proves that an exact
authenticated `CertifiedBodyResponse` with a still-live matching logical
request registration can retire the old request. The fence slot is one
retained physical credit rather than a limit of one
certificate. Multiple distinct authenticated TCs, CommitQCs, or
CommitQC-carrying recovery responses share that credit; every certificate
after the first consumes ordinary Progress capacity, and the ordinary
reservation is independent of whether the first certificate arrives before
or after ordinary work. There is no response-local phase or resettable
certificate latch. A selected `CertifiedResponse` remains ordinary FIFO work:
a later `TimeoutVote` cannot cross it, while timeout control is a dependency
only when it advances the current Proposal, vote, QC, or TimeoutVote owner. The
claimed-response rank counts the exact direct roots already inside its frozen
prefix and their strictly decreasing trusted causal tail, so later timeout
traffic cannot replenish that prefix. The standalone revision-4 kernel charges
an unpublished `BodyAvailable` token as an ordinary Completion owner and
atomically replaces its conflicting proposal owner without changing physical
occupancy. Production refinement additionally persists a complete Busy-
deferred producer-release batch before removing queue owners, reconciles
obsolete Reserved producers only after restart WAL replay while preserving the
exact protected body pipeline, and retires a restored stage-7 Fetch parent even
when no Completion token was reserved. The fresh post-restart Fetch owner proves
only its exact effect; the adapter resolves the old parent by durable
round/subject coordinates plus exact manifest identity when supplied, and
requires a unique match when the manifest is absent. Body rebind preserves the sole
persistent producer and rejects two persistent roots before mutation. The
generic productive-wire cut is monotone at certified EnterView and durable
Decision: the durable gate prunes exactly obsolete Dormant records before the
fair-ingress mirror, preserves ordinal high-waters, rolls back on store failure,
and rejects below-cut view-scoped admission. A `CertifiedResponse` is not
view-scoped at this boundary: durable Decision may precede recovery of its exact
body, so both cuts preserve the bounded response owner and defer authority to
the outstanding signed request, QC, responder signature, canonical body, and
manifest checks. Proposal chunks remain cut-scoped. The Rust refinement now
preflights Proposal control and canonical chunks as one aggregate exact-output
batch: it plans both FIFO owners without mutation, returns the opaque recovered
authority under capacity pressure or typed abort, and commits both under the
same retained mutex. The recovered authority is also joined to the exact
body-store instance and process output guard; ordinary live Proposal output
uses the same batch planner before consuming its first-send marker. This is a
source-bound crash/refinement contract, not a new deductive theorem. The
already-WAL-ahead recovered Proposal transaction now retains that batch across
one exact LedgerV1 publication of Broadcast plus its independent next Sign;
the assertion-only tail parks Broadcast, leaves Sign Ready, advances the
adapter, retires the worker completion, and enqueues both fanouts. Cold-open
authentication now has a frame-exact pair classifier and an affine
frozen-roster reducer replay which must reproduce both durable children; its
control and phase branches splice both carriers into the complete census, and
the control branch reconstructs Proposal chunks from the exact body-store
owner. The initial `ProposalPrepareWal` transaction now reserves the same
batch, preflights `PrepareIntent -> Sign(Prepare)`, fsyncs that frame, and then
uses the same two-child LedgerV1 publication. Capacity is retryable only before
the WAL append; every later ambiguity is restart-only. The unified lifecycle
Completion-turn driver classifies both recovered Proposal settlement shapes,
and the live lifecycle height driver invokes it with the real borrow-bound
Completion turn before the ordinary completion tail. The cold Proposal path is
therefore reachable without a second scheduler or publication authority. These
are source-bound production-refinement contracts; the existing deductive
asynchronous proof does not by itself prove their Rust persistence ordering,
and no theorem or evidence status is promoted. The executor then retries the
same retained FIFO occurrence and acquires pending-work and request ownership
atomically. A new Fetch removes that head; an existing ordinary Fetch keeps it
as the exact completion barrier after upgrading request authority. The
preceding mutable-source discovery and direct execution evidence covered the
earlier 168-name inventory. The latest fresh discovery checkpoint covered 738
names; the current 864-name tree still requires a clean committed, detached,
source-sealed serial release execution. An
earlier exact one-attempt
four-validator genesis rerun is green at 1/1 in 456.76 seconds. Neither
inventory presence nor regression evidence is a machine proof.
For the preceding proposal-origin source, the isolated source-shared harness
passed 118/118 reducer/WAL/refinement tests and all 8 model-trace replay tests;
the current harness inventories 137 runnable reducer tests and requires a fresh
source-sealed run. Fast-network passed all 9 named simulations. The 2026-07-21 100,000-height
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
Fresh current-source asynchronous-liveness proof evidence remains outstanding.
A fresh
pinned strict whole-module aggregate release TLAPS run, the clean source-sealed
release gate, and the release-profile 100,000-height chaos rerun remain pending.
