---- MODULE SumeragiV2LockedBodyReproposalProgressProofs ----
EXTENDS SumeragiV2ExactLeaderSemanticRanks,
        SumeragiV2LockedBodyProposalActionProofs

(***************************************************************************
Direct retained-lock decomposition.

This leaf sits between timeout/view progress and rotating-leader progress.
It therefore does not use rotating-leader Decision convergence to satisfy the
terminal disjunct of `LockedBodyReproposalOutcome`.  Instead, one fixed
`(node, lockedRound, subject)` source is followed through the production
corridors which can actually discharge it:

  1. the node's durable old-round Commit owner may finish at exactly
     `(lockedRound, subject)`;
  2. timeout/TC installation either preserves that exact Prepare subject for
     a later self-leader turn or exposes a higher certified Prepare transfer;
  3. the retained body is acquired/rebound, stored, and validated at one
     frozen later proposal origin;
  4. that same origin runs Assemble -> BeginProposal -> PersistProposal ->
     replay-safe SignProposal; or
  5. a higher Prepare/lock or durable Decision legitimately supersedes it.

The frozen `originView` below is intentionally retained across rank handoffs.
An old-round Commit rank cannot be combined with a later-round Commit rank,
and proposal stages from different views cannot be spliced into one progress
witness.  A later Commit corridor is classified only as higher-lock transfer,
never as `LockedBodyCommittedInOldRound`.

Three temporal properties remain explicit at the bottom:

  * timeout-fed source exposure to an exact later self-leader/Prepare-transfer
    frontier;
  * activation of the exact producer origin at that frontier; and
  * exact same-origin semantic-rank handoff.

They are premises of a deductive reduction, not asserted theorems.  In
particular, this module does not replace the source-exposure/leader-turn gap
with broad responsive Decision convergence.
***************************************************************************)

(***************************************************************************
Generic convergence vocabulary remains visible to the higher rotating-leader
leaf.  It is not used anywhere in the locked-body reduction below.
***************************************************************************)

ResponsiveDecisionConvergenceProperty(specification) ==
  specification
    => (gst /\ ~ResponsiveNodesDecide) ~> ResponsiveNodesDecide

RetainedLockModeSource(node, lockedRound, subject) ==
  StableAvailableRetainedLock(node, lockedRound, subject)

RetainedLockModeGoal(node, lockedRound, subject) ==
  LockedBodyReproposalOutcome(node, lockedRound, subject)

RetainedLockModeActive(node, lockedRound, subject) ==
  /\ RetainedLockModeSource(node, lockedRound, subject)
  /\ ~RetainedLockModeGoal(node, lockedRound, subject)

(***************************************************************************
Exact timeout/TC frontiers.

`LockedBodyProposalAttemptStableFrame` fixes the retained source node as the
honest leader of one later view, requires that view's preceding TC to be
installed, and requires the proposal subject selected by the exact installed
high Prepare evidence to remain the retained subject.  The alternative
certified-transfer frontier is the action proof's exact higher PrepareQC exit;
if its lock has already been installed, the release goal is already true.
***************************************************************************)

RetainedLockExactLeaderTurn(
    node, lockedRound, subject, originView) ==
  /\ originView \in Views
  /\ originView > lockedRound
  /\ LockedBodyProposalAttemptStableFrame(
       node, originView, lockedRound, subject)

RetainedLockCertifiedPrepareTransfer(
    node, lockedRound, subject, originView) ==
  /\ originView \in Views
  /\ originView = highestRank[node]
  /\ ~RetainedLockModeGoal(node, lockedRound, subject)
  /\ LockedBodyProposalCertifiedHighExit(
       node, lockedRound, subject)

(***************************************************************************
One exact producer origin.

Every candidate below is current-consumer protected work owned by the retained
source node.  Besides `(context, height, originView, subject)`, the three
adapter identity fields must still name the exact subject carried by the
candidate.  This excludes a fabricated same-kind candidate and prevents a
later rank handoff from silently changing body/manifest/commitment origin.
***************************************************************************)

RetainedLockFrozenCandidateOrigin(candidate, node, originView) ==
  /\ candidate.node = node
  /\ candidate.consumerContext = context
  /\ candidate.height = context.height
  /\ candidate.view = originView
  /\ candidate.subject \in Subjects
  /\ candidate.bodyIdentity = candidate.subject
  /\ candidate.manifestIdentity = candidate.subject
  /\ candidate.commitmentIdentity = candidate.subject

RetainedLockProposalIngressOrigin(candidate, originView, subject) ==
  \/ candidate.kind
       \notin {"DeliverProposal", "DeliverChunk"}
  \/ /\ candidate.kind = "DeliverProposal"
        /\ candidate.item.kind = "Proposal"
        /\ candidate.item.envelope.recipient = candidate.node
        /\ candidate.item.envelope.proposal.context = context
        /\ candidate.item.envelope.proposal.height = context.height
        /\ candidate.item.envelope.proposal.view = originView
        /\ candidate.item.envelope.proposal.subject = subject
        /\ candidate.item.envelope.proposal.proposer =
             Leader(context, originView)
  \/ /\ candidate.kind = "DeliverChunk"
        /\ candidate.item.kind = "Chunk"
        /\ candidate.item.envelope.recipient = candidate.node
        /\ candidate.item.envelope.height = context.height
        /\ candidate.item.envelope.view = originView
        /\ candidate.item.envelope.subject = subject

RetainedLockOldRoundCommitCandidateRank(
    node, lockedRound, subject, originView, candidate, rank) ==
  /\ originView = lockedRound
  /\ RetainedLockFrozenCandidateOrigin(candidate, node, originView)
  /\ candidate.subject = subject
  /\ ExactLeaderCommitRank(candidate, rank)

RetainedLockLaterProposalKinds ==
  {"AssembleBody", "BeginProposal", "PersistProposal", "SignProposal",
   "DeliverProposal", "DeliverChunk", "FetchBody", "RebindRetainedBody",
   "FetchCertifiedBody", "StoreBody", "ValidateBody"}

RetainedLockLaterProposalCandidateRank(
    node, lockedRound, subject, originView, candidate, rank) ==
  /\ originView > lockedRound
  /\ RetainedLockFrozenCandidateOrigin(candidate, node, originView)
  /\ candidate.subject = subject
  /\ candidate.kind \in RetainedLockLaterProposalKinds
  /\ RetainedLockProposalIngressOrigin(candidate, originView, subject)
  /\ ExactLeaderProposalRank(candidate, rank)

RetainedLockHigherPrepareCandidateRank(
    node, lockedRound, originView, candidate, rank) ==
  /\ originView > lockedRound
  /\ RetainedLockFrozenCandidateOrigin(candidate, node, originView)
  /\ ExactLeaderPrepareRank(candidate, rank)

RetainedLockHigherLockCandidateRank(
    node, lockedRound, originView, candidate, rank) ==
  /\ originView > lockedRound
  /\ RetainedLockFrozenCandidateOrigin(candidate, node, originView)
  /\ ExactLeaderCommitRank(candidate, rank)

RetainedLockDecisionCandidateRank(
    node, originView, candidate, rank) ==
  /\ RetainedLockFrozenCandidateOrigin(candidate, node, originView)
  /\ ExactLeaderDecisionRank(candidate, rank)

RetainedLockExactCandidateRank(
    node, lockedRound, subject, originView, candidate, rank) ==
  /\ ExactLeaderCandidateRank(candidate, rank)
  /\ \/ RetainedLockOldRoundCommitCandidateRank(
          node, lockedRound, subject, originView, candidate, rank)
     \/ RetainedLockLaterProposalCandidateRank(
          node, lockedRound, subject, originView, candidate, rank)
     \/ RetainedLockHigherPrepareCandidateRank(
          node, lockedRound, originView, candidate, rank)
     \/ RetainedLockHigherLockCandidateRank(
          node, lockedRound, originView, candidate, rank)
     \/ RetainedLockDecisionCandidateRank(
          node, originView, candidate, rank)

RetainedLockCandidateRankFrontier(
    node, lockedRound, subject, originView, rank) ==
  /\ RetainedLockModeActive(node, lockedRound, subject)
  /\ originView \in Views
  /\ rank \in ExactLeaderSemanticRankCarrier
  /\ \E candidate \in AsyncCandidateSet:
       RetainedLockExactCandidateRank(
         node, lockedRound, subject, originView, candidate, rank)

RetainedLockRankedOriginFrontier(
    node, lockedRound, subject, originView) ==
  \E rank \in ExactLeaderSemanticRankCarrier:
    RetainedLockCandidateRankFrontier(
      node, lockedRound, subject, originView, rank)

RetainedLockRankedFrontier(node, lockedRound, subject) ==
  \E originView \in Views:
    RetainedLockRankedOriginFrontier(
      node, lockedRound, subject, originView)

RetainedLockSourceExposureFrontier(node, lockedRound, subject) ==
  \E originView \in Views:
    \/ RetainedLockExactLeaderTurn(
         node, lockedRound, subject, originView)
    \/ RetainedLockCertifiedPrepareTransfer(
         node, lockedRound, subject, originView)
    \/ RetainedLockRankedOriginFrontier(
         node, lockedRound, subject, originView)

(***************************************************************************
Static safety anchors.
***************************************************************************)

THEOREM RetainedLockLaterProposalIsSafeForDurableLock ==
  \A node \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects:
    \A proposal:
      /\ StableAvailableRetainedLock(node, lockedRound, subject)
      /\ proposal.view > lockedRound
      /\ proposal.subject = subject
      => DurableProposalSafeForLock(node, proposal)
BY Isa
   DEF StableAvailableRetainedLock,
       DurableProposalSafeForLock

THEOREM OldRoundCommitFrontierRejectsSplitRoundCommit ==
  \A node \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, originView \in Views:
    \A candidate, rank:
      /\ originView # lockedRound
      => ~RetainedLockOldRoundCommitCandidateRank(
            node, lockedRound, subject, originView, candidate, rank)
BY DEF RetainedLockOldRoundCommitCandidateRank

(***************************************************************************
Reuse of the lower exact action-preservation proof.

While one exact retained-body leader frame is active, a post-GST asynchronous
step preserves the same frame, changes that exact node's view, reaches a
legitimate release outcome, or exposes the authenticated higher PrepareQC
frontier.  This is an action theorem only; it does not claim that a leader
frame is eventually reached or that a producer is eventually scheduled.
***************************************************************************)

THEOREM PostGstAsyncNextPreservesRetainedLockLeaderOriginOrExits ==
  \A node \in ValidatorIds, originView \in Views,
     lockedRound \in Views, subject \in Subjects:
    /\ StrongInductiveInvariant
    /\ RetainedLockExactLeaderTurn(
         node, lockedRound, subject, originView)
    /\ AsyncNext
    => \/ RetainedLockExactLeaderTurn(
            node, lockedRound, subject, originView)'
       \/ LockedBodyProposalAttemptViewExit(node, originView)'
       \/ RetainedLockModeGoal(node, lockedRound, subject)'
       \/ LockedBodyProposalCertifiedHighExit(
            node, lockedRound, subject)'
BY PostGstAsyncNextPreservesLockedBodyProposalAttemptOrExits, Isa
   DEF RetainedLockExactLeaderTurn,
       RetainedLockModeGoal,
       LockedBodyReproposalOutcome

(***************************************************************************
Explicit temporal proof boundaries.

Timeout/view progress is consumed first.  It guarantees each individual
undecided responsive view eventually advances or decides, but does not by
itself prove that the retained source reaches a later view which selects that
same node as leader while preserving the exact TC-selected subject.  The first
property names precisely that remaining source-exposure/leader-turn bridge.
It is deliberately an operator declaration, not a theorem.
***************************************************************************)

RetainedLockTimeoutFedSourceExposureLeaderTurnProperty(specification) ==
  TimeoutViewProgressProperty(specification)
    => (specification
          => \A node \in ValidatorIds,
                lockedRound \in Views,
                subject \in Subjects:
               RetainedLockModeSource(node, lockedRound, subject)
                 ~> (RetainedLockModeGoal(
                       node, lockedRound, subject)
                      \/ RetainedLockSourceExposureFrontier(
                           node, lockedRound, subject)))

\* TODO: prove the timeout-fed source-exposure/leader-turn property by
\* composing exact per-node view advancement, cyclic leader selection,
\* retained-lock preservation, and TC-selected Prepare ownership.

RetainedLockLeaderTurnProducerOriginProperty(specification) ==
  specification
    => \A node \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          originView \in Views:
         (RetainedLockExactLeaderTurn(
             node, lockedRound, subject, originView)
           \/ RetainedLockCertifiedPrepareTransfer(
                node, lockedRound, subject, originView))
           ~> (RetainedLockModeGoal(node, lockedRound, subject)
                \/ RetainedLockRankedOriginFrontier(
                     node, lockedRound, subject, originView))

\* TODO: prove exact producer-origin activation for old Commit ownership,
\* retained-body acquire/rebind/store/validate, and the later-view
\* Assemble/Begin/Persist/safe-Sign Proposal corridor.

RetainedLockRankHandoffProperty(specification) ==
  specification
    => \A node \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          originView \in Views,
          rank \in ExactLeaderSemanticRankCarrier:
         RetainedLockCandidateRankFrontier(
           node, lockedRound, subject, originView, rank)
           ~> (RetainedLockModeGoal(node, lockedRound, subject)
                \/ \E lowerRank \in
                       SetLessThan(
                         rank,
                         ExactLeaderSemanticRankOrdering,
                         ExactLeaderSemanticRankCarrier):
                     RetainedLockCandidateRankFrontier(
                       node, lockedRound, subject,
                       originView, lowerRank))

\* TODO: prove the exact same-origin semantic handoff from protected
\* candidate exit, causal successor, and durable/wire milestone theorems.

(***************************************************************************
Deductive rank closure.

The finite lexicographic carrier closes a supplied exact same-origin handoff
property.  These theorems do not supply any of the three temporal properties
above; they only show that those narrower premises are sufficient for the
release-facing locked-body property.
***************************************************************************)

THEOREM RetainedLockSemanticRankOrderingWellFounded ==
  IsWellFoundedOn(
    ExactLeaderSemanticRankOrdering,
    ExactLeaderSemanticRankCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset,
   WFLexPairOrdering, SMT
   DEF ExactLeaderSemanticRankOrdering,
       ExactLeaderSemanticRankCarrier

THEOREM RetainedLockRankHandoffClosesExactOrigin ==
  \A initialContext:
    RetainedLockRankHandoffProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A node \in ValidatorIds,
                  lockedRound \in Views,
                  subject \in Subjects,
                  originView \in Views,
                  rank \in ExactLeaderSemanticRankCarrier:
                 RetainedLockCandidateRankFrontier(
                   node, lockedRound, subject, originView, rank)
                   ~> RetainedLockModeGoal(
                        node, lockedRound, subject))
BY RetainedLockSemanticRankOrderingWellFounded,
   WellFoundedLeadsTo
   DEF RetainedLockRankHandoffProperty

THEOREM RetainedLockRankHandoffClosesRankedFrontier ==
  \A initialContext:
    RetainedLockRankHandoffProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A node \in ValidatorIds,
                  lockedRound \in Views,
                  subject \in Subjects:
                 RetainedLockRankedFrontier(
                   node, lockedRound, subject)
                   ~> RetainedLockModeGoal(
                        node, lockedRound, subject))
BY RetainedLockRankHandoffClosesExactOrigin, PTL
   DEF RetainedLockRankedFrontier,
       RetainedLockRankedOriginFrontier

THEOREM DirectRetainedLockDecompositionClosesLockedBodyReproposal ==
  \A initialContext:
    /\ TimeoutViewProgressProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockTimeoutFedSourceExposureLeaderTurnProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockLeaderTurnProducerOriginProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockRankHandoffProperty(
         AsyncLiveSpecAt(initialContext))
    => LockedBodyReproposalProgressProperty(
         AsyncLiveSpecAt(initialContext))
BY RetainedLockRankHandoffClosesRankedFrontier, PTL
   DEF RetainedLockTimeoutFedSourceExposureLeaderTurnProperty,
       RetainedLockLeaderTurnProducerOriginProperty,
       RetainedLockSourceExposureFrontier,
       RetainedLockRankedFrontier,
       RetainedLockModeSource,
       RetainedLockModeGoal,
       LockedBodyReproposalProgressProperty

=============================================================================
